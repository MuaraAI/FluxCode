#!/bin/sh
# Install the FluxCode Live Share relay on this VPS.
#
# Idempotent: running it again is safe. The secret is never overwritten, so
# rotation only happens when you do it yourself (see README.md).
#
#   sudo ./install.sh
#
# What it changes on the system:
#   /usr/local/bin/bore                      relay binary
#   /etc/bore-relay/env                      BORE_SECRET, mode 0640
#   /etc/systemd/system/bore-server.service
#   /etc/systemd/system/budget-guard.{service,timer}
#   /usr/local/lib/fluxcode-relay/           budget guard script
#   /var/www/fluxcode-relay/                 the /fluxcode page (when ports 80/443 are free)
set -eu

BORE_VERSION="0.6.0"
# From the official ekzhang/bore v0.6.0 release. The script refuses to install a
# binary whose checksum does not match.
SHA_X86_64="e484d1e3acba77169b773f31a5bfb34192d4b660f44a094a658a2522cd2270f7"
SHA_AARCH64="ffc4515f3617420b243758cf36ed6a63208d7dba76b2ec3e90d1f476a9742951"

RELAY_DIR="$(cd "$(dirname "$0")" && pwd)"
MIN_PORT=20000
MAX_PORT=20049
CONTROL_PORT=7835
BUDGET_GB="${FLUXCODE_RELAY_BUDGET_GB:-190}"

say()  { printf '  %s\n' "$*"; }
step() { printf '\n== %s\n' "$*"; }
die()  { printf '\n!! %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "run this with sudo"

step "Checking the architecture"
case "$(uname -m)" in
    x86_64)        ARCH="x86_64";  SHA="$SHA_X86_64" ;;
    aarch64|arm64) ARCH="aarch64"; SHA="$SHA_AARCH64" ;;
    *) die "architecture $(uname -m) is not supported yet (x86_64 or aarch64)" ;;
esac
say "$ARCH"

step "Installing bore $BORE_VERSION"
if [ -x /usr/local/bin/bore ] && /usr/local/bin/bore --version 2>/dev/null | grep -q "$BORE_VERSION"; then
    say "already installed"
else
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    URL="https://github.com/ekzhang/bore/releases/download/v${BORE_VERSION}/bore-v${BORE_VERSION}-${ARCH}-unknown-linux-musl.tar.gz"
    say "downloading $URL"
    curl -fsSL --max-time 120 "$URL" -o "$TMP/bore.tgz" || die "download failed"
    GOT="$(sha256sum "$TMP/bore.tgz" | cut -d' ' -f1)"
    [ "$GOT" = "$SHA" ] || die "checksum mismatch: $GOT"
    tar xzf "$TMP/bore.tgz" -C "$TMP"
    install -m 0755 "$TMP/bore" /usr/local/bin/bore
    say "installed, checksum matched"
fi

step "Creating the borerelay system user"
if id borerelay >/dev/null 2>&1; then
    say "already exists"
else
    useradd --system --no-create-home --shell /usr/sbin/nologin borerelay
    say "created"
fi

step "Preparing the secret"
# Owned by the group the service runs as, so `EnvironmentFile` can read it while
# nobody else can.
install -d -m 0750 -o root -g borerelay /etc/bore-relay
if [ -s /etc/bore-relay/env ]; then
    say "already present, left alone (rotate by hand, see README.md)"
else
    SECRET="$(openssl rand -hex 32)"
    printf 'BORE_SECRET=%s\n' "$SECRET" > /etc/bore-relay/env
    say "generated"
fi
chown root:borerelay /etc/bore-relay/env
chmod 0640 /etc/bore-relay/env

if [ -n "${FLUXCODE_RELAY_BUDGET_GB:-}" ]; then
    printf 'FLUXCODE_RELAY_BUDGET_GB=%s\n' "$FLUXCODE_RELAY_BUDGET_GB" > /etc/bore-relay/budget
    chmod 0644 /etc/bore-relay/budget
    say "budget ceiling set to ${FLUXCODE_RELAY_BUDGET_GB} GB/month"
fi

step "Checking the bandwidth meter"
if command -v vnstat >/dev/null 2>&1; then
    say "vnstat already installed"
else
    # Without vnstat the budget guard is blind, and that is worse than no guard:
    # the timer stays green while the bill runs on its own.
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y vnstat >/dev/null || die "could not install vnstat"
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y vnstat >/dev/null || die "could not install vnstat"
    elif command -v pacman >/dev/null 2>&1; then
        pacman -S --noconfirm vnstat >/dev/null || die "could not install vnstat"
    else
        die "vnstat is missing and no known package manager is available; install it by hand and run this again"
    fi
    say "vnstat installed"
fi
systemctl enable --now vnstat >/dev/null 2>&1 || say "WARNING: the vnstat service could not be enabled"

step "Installing the systemd units"
install -m 0644 "$RELAY_DIR/bore-server.service" /etc/systemd/system/bore-server.service
install -d -m 0755 /usr/local/lib/fluxcode-relay
install -m 0755 "$RELAY_DIR/budget-guard.sh" /usr/local/lib/fluxcode-relay/budget-guard.sh
install -m 0644 "$RELAY_DIR/budget-guard.service" /etc/systemd/system/budget-guard.service
install -m 0644 "$RELAY_DIR/budget-guard.timer" /etc/systemd/system/budget-guard.timer
systemctl daemon-reload
say "installed"

step "Opening the ports in the local firewall"
if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw allow "${CONTROL_PORT}/tcp" >/dev/null
    ufw allow "${MIN_PORT}:${MAX_PORT}/tcp" >/dev/null
    say "ufw: ${CONTROL_PORT}/tcp and ${MIN_PORT}-${MAX_PORT}/tcp opened"
elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port="${CONTROL_PORT}/tcp" >/dev/null
    firewall-cmd --permanent --add-port="${MIN_PORT}-${MAX_PORT}/tcp" >/dev/null
    firewall-cmd --reload >/dev/null
    say "firewalld: ports opened"
else
    say "no active local firewall found, nothing to open here"
fi

step "Starting the relay"
systemctl enable --now bore-server.service
sleep 1
systemctl is-active --quiet bore-server.service && say "bore-server is running" || die "bore-server failed to start, see: journalctl -u bore-server -n 50"

step "Scheduling the budget guard (${BUDGET_GB} GB/month)"
systemctl enable --now budget-guard.timer
say "budget-guard.timer is active"

step "Information page"
if command -v caddy >/dev/null 2>&1; then
    if ss -tlnH 2>/dev/null | awk '{print $4}' | grep -qE ':(80|443)$'; then
        say "ports 80/443 are taken, skipping the information page"
    else
        install -d -m 0755 /var/www/fluxcode-relay
        install -m 0644 "$RELAY_DIR/public/fluxcode.html" /var/www/fluxcode-relay/index.html
        install -d -m 0755 /etc/caddy
        # The Caddyfile may hold the operator's own sites, so it is appended to,
        # never replaced, and only when our block is not in it yet.
        if grep -q "/var/www/fluxcode-relay" /etc/caddy/Caddyfile 2>/dev/null; then
            say "the Caddyfile already serves fluxcode-relay, leaving it alone"
        else
            # The operator's last line may lack a trailing newline, and without one
            # our comment would merge into it and break the whole config.
            if [ -s /etc/caddy/Caddyfile ] && [ -n "$(tail -c 1 /etc/caddy/Caddyfile)" ]; then
                printf '\n' >> /etc/caddy/Caddyfile
            fi
            cat "$RELAY_DIR/caddy/Caddyfile" >> /etc/caddy/Caddyfile
            say "fluxcode block appended to /etc/caddy/Caddyfile"
        fi
        # Validate before reloading: a bad config would otherwise take the
        # operator's other sites down with it.
        if caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1; then
            systemctl reload caddy 2>/dev/null || systemctl restart caddy
            say "information page served at /fluxcode"
        else
            say "WARNING: /etc/caddy/Caddyfile did not validate; caddy was not reloaded"
        fi
    fi
else
    say "caddy is not installed, skipping the information page"
fi

step "Final check"
if ss -tlnH 2>/dev/null | awk '{print $4}' | grep -q ":${CONTROL_PORT}$"; then
    say "listening on :${CONTROL_PORT}"
else
    say "WARNING: nothing is listening on :${CONTROL_PORT}"
fi

cat <<EOF

Done. Check these once in the VPS provider's panel:

  1. Inbound TCP ${CONTROL_PORT} and ${MIN_PORT}-${MAX_PORT} are open in the
     security group / firewall. install.sh only opens the local firewall,
     which cannot see (and must not change) the provider's rules.
  2. The DNS record for the relay host is DNS-only in Cloudflare (grey cloud).
     Behind the proxy, ${CONTROL_PORT} and ${MIN_PORT}-${MAX_PORT} are dead.

Then test from another machine:

  nc -vz <relay host> ${CONTROL_PORT}
  nc -vz <relay host> ${MIN_PORT}

The relay secret printed above goes into the GitHub Actions secret
FLUXCODE_RELAY_SECRET, which builds bake into the client:

  GitHub -> the FluxCode repo -> Settings -> Secrets and variables -> Actions

Rotating it later, and the bandwidth budget, are covered in relay/README.md.
EOF

if [ -s /etc/bore-relay/env ]; then
    printf '\nThe relay secret (also in /etc/bore-relay/env):\n\n  %s\n' "$(sed -n 's/^BORE_SECRET=//p' /etc/bore-relay/env)"
fi

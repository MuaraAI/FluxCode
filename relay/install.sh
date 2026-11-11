#!/bin/sh
# Pasang relay FluxCode Live Share di VPS ini.
#
# Idempoten: menjalankan ulang aman. Secret tidak pernah ditimpa, jadi rotasi
# hanya terjadi kalau kamu melakukannya sendiri (lihat README).
#
#   sudo ./install.sh
#
# Yang diubah pada sistem:
#   /usr/local/bin/bore                      binary relay
#   /etc/bore-relay/env                      BORE_SECRET, mode 0600
#   /etc/systemd/system/bore-server.service
#   /etc/systemd/system/budget-guard.{service,timer}
#   /usr/local/lib/fluxcode-relay/           skrip penjaga anggaran
#   /var/www/fluxcode-relay/                 halaman /fluxcode (kalau port 80/443 bebas)
set -eu

BORE_VERSION="0.6.0"
# Diambil dari rilis resmi ekzhang/bore v0.6.0. Skrip menolak memasang binary
# yang checksum-nya tidak cocok.
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

[ "$(id -u)" -eq 0 ] || die "jalankan dengan sudo"

step "Memeriksa arsitektur"
case "$(uname -m)" in
    x86_64)        ARCH="x86_64";  SHA="$SHA_X86_64" ;;
    aarch64|arm64) ARCH="aarch64"; SHA="$SHA_AARCH64" ;;
    *) die "arsitektur $(uname -m) belum didukung (x86_64 atau aarch64)" ;;
esac
say "$ARCH"

step "Memasang bore $BORE_VERSION"
if [ -x /usr/local/bin/bore ] && /usr/local/bin/bore --version 2>/dev/null | grep -q "$BORE_VERSION"; then
    say "sudah terpasang"
else
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    URL="https://github.com/ekzhang/bore/releases/download/v${BORE_VERSION}/bore-v${BORE_VERSION}-${ARCH}-unknown-linux-musl.tar.gz"
    say "mengunduh $URL"
    curl -fsSL --max-time 120 "$URL" -o "$TMP/bore.tgz" || die "unduhan gagal"
    GOT="$(sha256sum "$TMP/bore.tgz" | cut -d' ' -f1)"
    [ "$GOT" = "$SHA" ] || die "checksum tidak cocok: $GOT"
    tar xzf "$TMP/bore.tgz" -C "$TMP"
    install -m 0755 "$TMP/bore" /usr/local/bin/bore
    say "terpasang, checksum cocok"
fi

step "Membuat user sistem borerelay"
if id borerelay >/dev/null 2>&1; then
    say "sudah ada"
else
    useradd --system --no-create-home --shell /usr/sbin/nologin borerelay
    say "dibuat"
fi

step "Menyiapkan secret"
install -d -m 0750 /etc/bore-relay
if [ -s /etc/bore-relay/env ]; then
    say "sudah ada, dibiarkan (rotasi manual, lihat README)"
else
    SECRET="$(openssl rand -hex 32)"
    printf 'BORE_SECRET=%s\n' "$SECRET" > /etc/bore-relay/env
    say "dibuat baru"
fi
chown root:borerelay /etc/bore-relay/env
chmod 0640 /etc/bore-relay/env

if [ -n "${FLUXCODE_RELAY_BUDGET_GB:-}" ]; then
    printf 'FLUXCODE_RELAY_BUDGET_GB=%s\n' "$FLUXCODE_RELAY_BUDGET_GB" > /etc/bore-relay/budget
    chmod 0644 /etc/bore-relay/budget
    say "batas anggaran disetel ke ${FLUXCODE_RELAY_BUDGET_GB} GB/bulan"
fi

step "Memasang unit systemd"
install -m 0644 "$RELAY_DIR/bore-server.service" /etc/systemd/system/bore-server.service
install -d -m 0755 /usr/local/lib/fluxcode-relay
install -m 0755 "$RELAY_DIR/budget-guard.sh" /usr/local/lib/fluxcode-relay/budget-guard.sh
install -m 0644 "$RELAY_DIR/budget-guard.service" /etc/systemd/system/budget-guard.service
install -m 0644 "$RELAY_DIR/budget-guard.timer" /etc/systemd/system/budget-guard.timer
systemctl daemon-reload
say "terpasang"

step "Membuka port di firewall lokal"
if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw allow "${CONTROL_PORT}/tcp" >/dev/null
    ufw allow "${MIN_PORT}:${MAX_PORT}/tcp" >/dev/null
    say "ufw: ${CONTROL_PORT}/tcp dan ${MIN_PORT}-${MAX_PORT}/tcp dibuka"
elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port="${CONTROL_PORT}/tcp" >/dev/null
    firewall-cmd --permanent --add-port="${MIN_PORT}-${MAX_PORT}/tcp" >/dev/null
    firewall-cmd --reload >/dev/null
    say "firewalld: port dibuka"
else
    say "tidak ada ufw/firewalld aktif, lewati"
fi

step "Menjalankan relay"
systemctl enable --now bore-server.service
sleep 1
systemctl is-active --quiet bore-server.service && say "bore-server aktif" || die "bore-server gagal start, lihat: journalctl -u bore-server -n 50"

step "Menjadwalkan penjaga anggaran (${BUDGET_GB} GB/bulan)"
systemctl enable --now budget-guard.timer
say "budget-guard.timer aktif"

step "Halaman informasi"
if command -v caddy >/dev/null 2>&1; then
    if ss -tlnH 2>/dev/null | awk '{print $4}' | grep -qE ':(80|443)$'; then
        say "port 80/443 sudah dipakai, lewati halaman info"
    else
        install -d -m 0755 /var/www/fluxcode-relay
        install -m 0644 "$RELAY_DIR/public/fluxcode.html" /var/www/fluxcode-relay/index.html
        install -d -m 0755 /etc/caddy
        install -m 0644 "$RELAY_DIR/caddy/Caddyfile" /etc/caddy/Caddyfile
        systemctl reload caddy 2>/dev/null || systemctl restart caddy
        say "halaman info dipasang di /fluxcode"
    fi
else
    say "caddy tidak terpasang, lewati halaman info"
fi

step "Pemeriksaan akhir"
if ss -tlnH 2>/dev/null | awk '{print $4}' | grep -q ":${CONTROL_PORT}$"; then
    say "mendengarkan di :${CONTROL_PORT}"
else
    say "PERINGATAN: tidak ada yang mendengarkan di :${CONTROL_PORT}"
fi

cat <<EOF

Selesai. Yang perlu dicek sekali dari panel provider VPS:

  * port ${CONTROL_PORT}/tcp terbuka
  * port ${MIN_PORT}-${MAX_PORT}/tcp terbuka
  * DNS live.muaraai.com -> IP VPS ini, Proxy OFF (grey cloud)

Status:   systemctl status bore-server
Log:      journalctl -u bore-server -f
Uji:      nc -vz live.muaraai.com ${CONTROL_PORT}
Anggaran: cat /etc/bore-relay/env   (nilai BORE_SECRET untuk GitHub Actions)

EOF

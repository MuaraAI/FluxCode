# FluxCode Live Share relay

The TCP relay that makes FluxCode Live Share work across networks, without port
forwarding on anyone's PC.

One VPS runs the relay. Host and guest both connect **out** to it, so no PC needs
a public address.

```text
Guest ──┐
        ├── TCP 7835 / 20000-20049 ──► relay VPS ──► Host PC
Host ───┘        (ciphertext only)
```
Session content is encrypted with the key in the invite code. The relay only sees
ciphertext: it cannot read files, text, or cursor positions.

## Two relays, one client

FluxCode knows two relays, and they are not the same thing:

| | MuaraAI relay (`live.muaraai.com`) | Public relay (`bore.pub`) |
|---|---|---|
| Run by | you, on this VPS | the `bore` project by ekzhang, free |
| When it is used | **always tried first** | automatically, only when our relay fails |
| Authentication | HMAC-SHA256 with the secret | none |
| Limit | 190 GB/month, enforced by `budget-guard` | no guarantee, can change any day |
| Name in the client UI | "Relay (MuaraAI)" | "Relay (public)" |

The order lives in `endpoints()` (`src/collab/reach.rs`): this relay first,
`bore.pub` as the spare. "This relay is down" needs no manual switch: if the VPS
is off, port 7835 is blocked, or the secret does not match, the client moves to
`bore.pub` on its own and the session keeps working.

A build without a secret has no primary endpoint at all: `bore.pub` only. That is
deliberate, so a PR build sends no traffic to this VPS.

To disable the spare entirely (for example while testing the uninstall):
`FLUXCODE_RELAY_FALLBACK=0`.

---

## 1. VPS requirements

| Item | Requirement |
|---|---|
| Distro | anything with `systemd` and `sh` (tested on Ubuntu 22.04) |
| Access | `root` or `sudo` |
| Address | **public and static**. A dynamic address breaks DNS when it changes |
| RAM | 64 MB is enough. The relay uses about 2 MB idle |
| Disk | under 5 MB |
| Bandwidth | default budget 190 GB/month (see section 7) |

No GPU, database, Docker, or language runtime.

---

## 2. Ports that must be open

| Port | Protocol | For |
|---|---|---|
| `7835` | TCP | relay control (handshake and port assignment) |
| `20000-20049` | TCP | data; one port per Live Share session |
| `80`, `443` | TCP | optional, only for the `/fluxcode` page |

`install.sh` opens the ports in the **local** firewall (`ufw` or `firewalld`). The
**provider** firewall must be opened by hand, because it cannot be automated from
inside the VPS.

Typical provider steps:

- **Tencent Cloud** — CVM → Security Group → Add Rule → TCP, `7835`, `20000-20049`, source `0.0.0.0/0`
- **AWS** — EC2 → Security → Security groups → Inbound rules → Custom TCP, port range `20000-20049`
- **DigitalOcean** — Networking → Firewalls → Inbound Rules → Custom, TCP `20000-20049`
- **Hetzner** — Firewalls → Add rule → TCP, port `20000-20049`
- **VPS with no provider firewall** — the local firewall is all there is; `install.sh` handles it

If `20000-20049` cannot be opened as one range, open a smaller one and adjust
`--min-port`/`--max-port` in `/etc/systemd/system/bore-server.service`. One port
is one concurrent session.

---

## 3. Install

```bash
git clone https://github.com/MuaraAI/FluxCode.git
cd FluxCode/relay
sudo ./install.sh
```

The script is idempotent: running it twice gives the same result.

What it changes on the system:

| Path | Contents |
|---|---|
| `/usr/local/bin/bore` | relay binary v0.6.0, checksum verified |
| `/etc/bore-relay/env` | `BORE_SECRET`, mode `0640`, **never overwritten** |
| `/etc/systemd/system/bore-server.service` | the relay unit |
| `/etc/systemd/system/budget-guard.{service,timer}` | the budget guard |
| `/usr/local/lib/fluxcode-relay/budget-guard.sh` | the guard script |
| `/var/www/fluxcode-relay/index.html` | the `/fluxcode` page (when ports 80/443 are free) |
| `/etc/caddy/Caddyfile` | appended to, never replaced |

No package is removed and no other configuration is overwritten.

---

## 4. Verify after install

```bash
systemctl status bore-server --no-pager
ss -tlnp | grep bore
```

It must be listening on `0.0.0.0:7835`.

Test from **another machine** (not the VPS itself), because that is the only way to
test the provider firewall:

```bash
nc -vz live.muaraai.com 7835
nc -vz live.muaraai.com 20000
```

Both must report `succeeded`. A `timed out` means the provider security group still
blocks the port.

Test the secret handshake (it must return a Challenge):

```bash
printf '{"Hello":0}\0' | timeout 3 nc live.muaraai.com 7835 | xxd | head -3
```

A reply containing `Challenge` means the relay is up and does ask for the secret.

---

## 5. DNS

In Cloudflare:

```
Type   : A
Name   : live
Content: <VPS IP>
Proxy  : OFF  (grey cloud)
TTL    : Auto
```

The proxy **must be OFF**. Cloudflare only forwards HTTP/HTTPS on the standard
ports; `7835` and `20000-20049` die completely behind the proxy.

Check propagation:

```bash
dig +short live.muaraai.com
# must return the VPS address, not Cloudflare's 104.x / 172.x
```

---

## 6. Rotating the secret

The secret lives in two places: the server and the client build. Both must change.

**a. On the relay**

```bash
sudo sh -c 'printf "BORE_SECRET=%s\n" "$(openssl rand -hex 32)" > /etc/bore-relay/env'
sudo chown root:borerelay /etc/bore-relay/env
sudo chmod 0640 /etc/bore-relay/env
sudo systemctl restart bore-server
sudo cat /etc/bore-relay/env     # copy the value
```

After the restart, old clients are refused immediately. A host in the middle of a
session has to start a new one.

**b. In the client**

GitHub → the FluxCode repo → Settings → Secrets and variables → Actions →
`FLUXCODE_RELAY_SECRET` → enter the new value → rebuild and release.

If the rotation is because of abuse, do (a) and (b) back to back within minutes,
then tell users that older builds can no longer use Live Share.

---

## 7. Bandwidth budget

The relay stops itself at **190 GB/month** (out of a 200 GB allowance). The ceiling
sits deliberately below the allowance so there is no surprise bill.

```bash
vnstat -m                              # monthly usage, human format
vnstat --oneline b                     # what the guard reads: field 11 = this month, bytes
systemctl list-timers budget-guard.timer
journalctl -u budget-guard -n 20
```

`install.sh` installs `vnstat` when it is missing. A budget guard without vnstat is
only green in `systemctl` while the bill runs on its own. The script fails hard
(`exit 1`) when vnstat is absent or its output is not recognised, so the failure
shows in `systemctl --failed`.

`--oneline` accepts only mode `b`; `m` is rejected with an error and a non-zero
exit. Mode `b` already reports raw bytes, with no unit to parse.

Raise the ceiling:

```bash
sudo sh -c 'printf "FLUXCODE_RELAY_BUDGET_GB=250\n" > /etc/bore-relay/budget'
sudo systemctl restart budget-guard.timer   # the unit already reads that EnvironmentFile
sudo systemctl start budget-guard.service
```

A relay stopped by the guard does not come back on its own. Start it after the
month rolls over:

```bash
sudo systemctl start bore-server
```

Rough arithmetic: 200 GB/month is 6.6 GB/day. A typical Live Share session uses
hundreds of kilobytes to a few megabytes, so hundreds of sessions a day still fit.

---

## 8. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `nc` to 7835 times out | provider security group blocks the port | open `7835/tcp` in the provider panel |
| `nc` to 20000 times out | the data range is not open | open `20000-20049/tcp` |
| `bore-server` keeps restarting | the secret cannot be read | `journalctl -u bore-server -n 50` |
| Client says "did not ask for a secret" | the relay runs without `--secret` | restart it with a secret |
| Client fails on every path | DNS is behind the Cloudflare proxy | set the record to DNS-only |
| `/fluxcode` returns 404 | Caddy is missing or the ports are taken | `systemctl status caddy` |
| `dig` returns `104.x` / `172.x` | the Cloudflare proxy is still on | turn the proxy off |
| Relay stopped by itself | the 190 GB budget was reached | see section 7 |

Logs:

```bash
journalctl -u bore-server -n 100 --no-pager
journalctl -u budget-guard -n 50 --no-pager
```

---

## 9. Uninstall

```bash
sudo systemctl disable --now bore-server budget-guard.timer
sudo rm -f /etc/systemd/system/bore-server.service \
           /etc/systemd/system/budget-guard.service \
           /etc/systemd/system/budget-guard.timer
sudo systemctl daemon-reload
sudo rm -rf /usr/local/lib/fluxcode-relay /etc/bore-relay /var/www/fluxcode-relay
sudo rm -f /usr/local/bin/bore
sudo userdel borerelay
```

If `install.sh` wrote to `/etc/caddy/Caddyfile`, read it before deleting anything:
the file may hold other sites.

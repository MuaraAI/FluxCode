# Relay Live Share FluxCode

Relay TCP yang membuat Live Share FluxCode bekerja antar jaringan, tanpa port
forwarding di PC siapa pun.

Satu VPS menjalankan relay. Host dan guest sama-sama menyambung **keluar** ke
relay ini, jadi tidak ada PC yang perlu IP publik.

```text
Guest ──┐
        ├── TCP 7835 / 20000-20049 ──► VPS relay ──► Host PC
Host ───┘        (ciphertext saja)
```

Isi sesi dienkripsi dengan kunci yang ada di kode undangan. Relay hanya melihat
byte terenkripsi: tidak bisa membaca berkas, teks, maupun posisi kursor.

---

## 1. Prasyarat VPS

| Item | Syarat |
|---|---|
| Distro | apa pun dengan `systemd` dan `sh` (diuji di Ubuntu 22.04) |
| Akses | `root` atau `sudo` |
| IP | **publik dan statis**. IP dinamis akan memutus DNS saat berubah |
| RAM | 64 MB cukup. Relay memakai ~2 MB saat idle |
| Disk | < 5 MB |
| Bandwidth | anggaran default 190 GB/bulan (lihat bagian 7) |

Tidak perlu GPU, database, Docker, atau runtime bahasa apa pun.

---

## 2. Port yang Harus Terbuka

| Port | Protokol | Untuk |
|---|---|---|
| `7835` | TCP | kontrol relay (handshake + penugasan port) |
| `20000-20049` | TCP | data; satu port per sesi Live Share |
| `80`, `443` | TCP | opsional, hanya untuk halaman `/fluxcode` |

`install.sh` membuka port pada firewall **lokal** (`ufw` atau `firewalld`).
Firewall **provider** harus dibuka manual, karena tidak bisa diotomasi dari
dalam VPS.

Contoh langkah di provider yang umum dipakai:

- **Tencent Cloud** — CVM → Security Group → Add Rule → TCP, `7835`, `20000-20049`, source `0.0.0.0/0`
- **AWS** — EC2 → Security → Security groups → Inbound rules → Custom TCP, port range `20000-20049`
- **DigitalOcean** — Networking → Firewalls → Inbound Rules → Custom, TCP `20000-20049`
- **Hetzner** — Firewalls → Add rule → TCP, port `20000-20049`
- **VPS tanpa firewall provider** — cukup firewall lokal, `install.sh` yang urus

Kalau `20000-20049` tidak bisa dibuka sekaligus, buka rentang lebih kecil dan
sesuaikan `--min-port`/`--max-port` di `/etc/systemd/system/bore-server.service`.
Satu port = satu sesi bersamaan.

---

## 3. Instalasi

```bash
git clone https://github.com/MuaraAI/FluxCode.git
cd FluxCode/relay
sudo ./install.sh
```

Skrip ini idempoten: dijalankan dua kali hasilnya sama.

Yang diubah pada sistem:

| Path | Isi |
|---|---|
| `/usr/local/bin/bore` | binary relay v0.6.0, checksum diverifikasi |
| `/etc/bore-relay/env` | `BORE_SECRET`, mode `0640`, **tidak ditimpa** |
| `/etc/systemd/system/bore-server.service` | unit relay |
| `/etc/systemd/system/budget-guard.{service,timer}` | penjaga anggaran |
| `/usr/local/lib/fluxcode-relay/budget-guard.sh` | skrip penjaga |
| `/var/www/fluxcode-relay/index.html` | halaman `/fluxcode` (kalau port 80/443 bebas) |
| `/etc/caddy/Caddyfile` | hanya kalau `caddy` sudah ada dan port bebas |

Tidak ada paket yang dihapus, tidak ada konfigurasi lain yang ditimpa.

---

## 4. Verifikasi Setelah Install

```bash
systemctl status bore-server --no-pager
ss -tlnp | grep bore
```

Harus terlihat mendengarkan di `0.0.0.0:7835`.

Uji dari **mesin lain** (bukan VPS-nya), karena itu satu-satunya cara menguji
firewall provider:

```bash
nc -vz live.muaraai.com 7835
nc -vz live.muaraai.com 20000
```

Keduanya harus `succeeded`. Kalau `timed out`, berarti security group provider
masih menutup port.

Uji handshake dengan secret (harus mengembalikan Challenge):

```bash
printf '{"Hello":0}\0' | timeout 3 nc live.muaraai.com 7835 | xxd | head -3
```

Balasan berisi `Challenge` berarti relay siap dan memang meminta secret.

---

## 5. DNS

Di Cloudflare:

```
Type   : A
Name   : live
Content: <IP VPS>
Proxy  : OFF  (grey cloud)
TTL    : Auto
```

Proxy **wajib OFF**. Cloudflare hanya meneruskan HTTP/HTTPS di port standar;
port `7835` dan `20000-20049` akan mati total kalau di-proxy.

Cek propagasi:

```bash
dig +short live.muaraai.com
# harus mengembalikan IP VPS, bukan 104.x / 172.x milik Cloudflare
```

---

## 6. Rotasi Secret

Secret ada di dua tempat: server dan build klien. Keduanya harus berubah.

**a. Di relay**

```bash
sudo sh -c 'printf "BORE_SECRET=%s\n" "$(openssl rand -hex 32)" > /etc/bore-relay/env'
sudo chown root:borerelay /etc/bore-relay/env
sudo chmod 0640 /etc/bore-relay/env
sudo systemctl restart bore-server
sudo cat /etc/bore-relay/env     # salin nilainya
```

Setelah restart, klien lama langsung ditolak. Host yang sedang sesi harus
memulai sesi baru.

**b. Di klien**

GitHub → repo FluxCode → Settings → Secrets and variables → Actions →
`FLUXCODE_RELAY_SECRET` → isi nilai baru → build dan rilis ulang.

Kalau rotasi karena penyalahgunaan, lakukan (a) dan (b) berurutan dalam
hitungan menit, lalu umumkan ke pengguna bahwa versi lama tidak bisa Live Share.

---

## 7. Anggaran Bandwidth

Relay berhenti sendiri pada **190 GB/bulan** (dari anggaran 200 GB). Batasnya
dipasang sengaja di bawah anggaran supaya tidak ada tagihan kejutan.

```bash
vnstat -m                        # pemakaian per bulan
systemctl list-timers budget-guard.timer
journalctl -u budget-guard -n 20
```

Menaikkan batas:

```bash
sudo sh -c 'printf "FLUXCODE_RELAY_BUDGET_GB=250\n" > /etc/bore-relay/budget'
sudo systemctl edit budget-guard.service   # tambahkan EnvironmentFile=/etc/bore-relay/budget
sudo systemctl start budget-guard.service
```

Relay yang berhenti karena anggaran tidak akan menyala sendiri. Nyalakan ulang
setelah bulan berganti:

```bash
sudo systemctl start bore-server
```

Hitungan kasar: 200 GB/bulan = 6,6 GB/hari. Sesi Live Share tipikal memakai
ratusan kilobyte sampai beberapa megabyte, jadi ratusan sesi per hari masih
masuk anggaran.

---

## 8. Troubleshooting

| Gejala | Penyebab | Tindakan |
|---|---|---|
| `nc` ke 7835 timeout | security group provider menutup port | buka `7835/tcp` di panel provider |
| `nc` ke 20000 timeout | rentang data belum dibuka | buka `20000-20049/tcp` |
| `bore-server` restart terus | secret tidak terbaca | `journalctl -u bore-server -n 50` |
| Klien bilang "did not ask for a secret" | relay jalan tanpa `--secret` | start ulang dengan secret |
| Klien gagal semua jalur | DNS di-proxy Cloudflare | set record ke DNS-only |
| Halaman `/fluxcode` 404 | Caddy tidak terpasang atau port dipakai lain | `systemctl status caddy` |
| `dig` mengembalikan `104.x` / `172.x` | proxy Cloudflare masih aktif | matikan proxy |
| Relay mati sendiri | anggaran 190 GB tercapai | lihat bagian 7 |

Log:

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

Bila `install.sh` yang menulis `/etc/caddy/Caddyfile`, periksa dulu isinya
sebelum menghapus: mungkin ada situs lain di berkas yang sama.

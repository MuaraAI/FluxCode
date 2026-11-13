# FluxCode Web Downloader: Design System & Architecture Specification

Dokumen spesifikasi desain, arsitektur UI/UX, dan mekanisme direct download untuk situs resmi pengunduh **FluxCode** (editor kode native berbasis Rust & GPUI dari MuaraAI).

---

## 1. Filosofi & Identitas Produk

### 1.1 Karakter Produk
FluxCode bukan sekadar editor kode biasa, melainkan editor native bertenaga GPU yang mengutamakan jendela yang tampil dalam 0,22 detik, memori idle 1,4 MB, dan tanpa telemetri, serta integrasi panel agen AI berstandar ACP (*Agent Client Protocol*).

Prinsip desain web pengunduh ini:
- **Jujur & Berbasis Bukti (No AI Slop / No Fluff)**: Setiap klaim performa (0,22 detik buka jendela, 1,4 MB idle RAM, 16 MB disk) diambil langsung dari benchmark terukur pada repositori resmi `MuaraAI/FluxCode`.
- **Eksplisit & Praktis**: Menghindari kata-kata hampa. Menjelaskan secara gamblang apa yang dilakukan perangkat lunak dan cara memasangnya.
- **Direct First (Langsung Mengunduh)**: Pengunjung datang untuk mengunduh software. Tombol unduh tidak boleh melempar pengguna ke halaman rilis GitHub yang rumit dengan daftar aset panjang, melainkan langsung memicu pengunduhan berkas binary yang tepat (*one-click direct download*) sesuai sistem operasi pengguna.

### 1.2 Motif Visual: Meander
Logo FluxCode mengadopsi motif **Meander** (dinamai dari sungai Maeander di Anatolia barat; salah satu meander tertua yang diketahui terukir pada gelang gading mamut yang ditemukan di Mezin, Ukraina, lebih dari 15.000 tahun lalu):
- Satu garis tanpa putus yang hanya berbelok tegak lurus dan tidak pernah memotong dirinya sendiri.
- Garisnya dibelah di tengah menjadi dua kait berbentuk sama, diputar 180 derajat dan saling mengunci, sehingga lambang tetap sama walau dibalik.
- Strukturnya seperti jalannya kode: satu jalur yang berbelok masuk ke sebuah blok lalu kembali keluar, seperti pemanggilan yang kembali ke tempat asalnya.
- Palet warna meander: gading (*ivory* `#ebe4d6`) dan tanah liat bakar/oker kuning (`#F0D55B` / `#b98a5e`) di atas latar tinta gelap (`#141416`).

---

## 2. Palet Warna & Token Semantik

| Token | Nilai Hex / HSL | Penggunaan |
| :--- | :--- | :--- |
| `--bg-canvas` | `#101012` | Latar belakang dasar halaman (*deep canvas*) |
| `--bg-surface` | `#17181c` | Permukaan kartu, mockup editor, dan panel |
| `--bg-card-hover` | `#202227` | Status interaksi hover pada kartu |
| `--border-subtle` | `#2d3038` | Garis batas struktural panel dan kontainer |
| `--border-focus` | `#F0D55B` | Garis batas status aktif, fokus aksesibilitas |
| `--text-primary` | `#f4f4f6` | Teks judul dan konten utama (kontras tinggi) |
| `--text-muted` | `#a0a5b2` | Teks deskripsi, label pembantu, metadata |
| `--accent-gold` | `#F0D55B` | Warna aksen utama FluxCode (kuning oker khas) |
| `--accent-gold-hover` | `#f5df7a` | State hover tombol primer |
| `--accent-gold-glow` | `rgba(240, 213, 91, 0.15)` | Efek glow halus pada tombol dan hero spotlight |
| `--accent-ivory` | `#ebe4d6` | Aksen sekunder meander dan highlight kode |
| `--accent-cyan` | `#5eead4` | Aksen penanda RAM / efisiensi pada grafik |
| `--accent-emerald` | `#4ade80` | Indikator status live, build aktif, dan verifikasi |

---

## 3. Tipografi

Sistem tipografi menggunakan dua jenis huruf yang mendukung nuansa ketelitian rekayasa piranti lunak (*engineering precision*):

1. **Bricolage Grotesque** (Font Display & UI):
   - Karakter huruf memiliki sudut visual tegas, kontemporer, dan berkarakter kuat tanpa terkesan kaku.
   - Digunakan pada logo, navigasi, *headings* (H1, H2, H3), tombol aksi, dan metrik angka benchmark.
2. **JetBrains Mono** (Font Monospace Teknis):
   - Digunakan pada mockup editor, cuplikan kode, tag versi build, hash aset, ukuran berkas, dan indikator terminal.

### Skala Hierarki:
- **Display Title (H1)**: `clamp(2.4rem, 5.5vw, 4.2rem)` · `line-height: 1.08` · `weight: 800`
- **Section Heading (H2)**: `clamp(1.8rem, 3.8vw, 2.8rem)` · `line-height: 1.15` · `weight: 800`
- **Card Heading (H3)**: `1.25rem` – `1.4rem` · `line-height: 1.25` · `weight: 700`
- **Body Lead**: `1.125rem` – `1.25rem` · `line-height: 1.6` · `weight: 400`
- **Body / Label**: `0.95rem` – `1.0rem` · `line-height: 1.55` · `weight: 400/500`
- **Micro Technical**: `0.8rem` – `0.85rem` · Monospace · `weight: 500`

---

## 4. Arsitektur Komponen & UX Flow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Header: Logo Meander + Navigasi + Status Build + CTA     │
├─────────────────────────────────────────────────────────────┤
│ 2. Hero Section:                                            │
│    - Badge Rilis Dev Aktif                                  │
│    - Headline Native Rust/GPUI & AI                         │
│    - Tombol Direct Download Otomatis Sesuai OS Pengguna     │
│    - Selector Cepat Sistem Operasi Lain                     │
├─────────────────────────────────────────────────────────────┤
│ 3. Mockup Editor Interaktif FluxCode:                       │
│    - Activity Bar (Explorer, Git, ACP Agent, Live Share)    │
│    - Editor Tab (main.rs / agent_prompt.flux)               │
│    - Panel Agen AI Terintegrasi (OpenCode 2 / Mura)         │
│    - Status Bar (1.4 MB idle RAM, 0.22s boot)               │
│    - Tab Interaktif Switcher Mode Fitur                     │
├─────────────────────────────────────────────────────────────┤
│ 4. Visual Benchmark: FluxCode vs Zed vs VS Code             │
│    - Waktu buka jendela (221 ms vs 818 ms vs 290 ms)        │
│    - Konsumsi RAM saat idle (1.4 MB vs 66.2 MB vs 929.2 MB) │
│    - Jumlah proses & ukuran disk                            │
├─────────────────────────────────────────────────────────────┤
│ 5. Grid Fitur Utama (Rust, ACP Agent, P2P Live Share, DLL)  │
├─────────────────────────────────────────────────────────────┤
│ 6. Hub Pengunduhan Langsung (#unduh):                       │
│    - Windows 10/11 x64 Setup (.exe)                         │
│    - Windows Portable (.zip)                                │
│    - macOS Universal (.dmg - Apple Silicon & Intel)         │
│    - Linux x64 (.tar.gz - Vulkan & glibc 2.35+)             │
│    - Catatan Pemasangan & Verifikasi Hash                   │
├─────────────────────────────────────────────────────────────┤
│ 7. Footer: Informasi Lisensi GPL-3.0, MuaraAI & MyKineID    │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Mekanisme Direct Download & Resolusi Aset

Tantangan utama yang diselesaikan dalam perombakan ini: **pengguna tidak diarahkan ke halaman rilis GitHub yang berbelit-belit, melainkan langsung mengunduh binary yang sesuai.**

### 5.1 Alur Deteksi Sistem Operasi (Client-Side OS Sniffing)
1. Periksa `navigator.userAgentData?.platform` dan `navigator.userAgent` / `navigator.platform`.
2. Identifikasi:
   - **Windows**: Pilih target aset `windows-x64-setup.exe`.
   - **macOS**: Pilih target aset `macos-universal.dmg` (arsip universal untuk Apple Silicon M1-M4 dan Mac berbasis Intel).
   - **Linux**: Pilih target aset `linux-x64.tar.gz`.
3. Sediakan tombol peralihan jika pengguna ingin mengunduh format alternatif (misalnya Windows Portable `.zip` atau mengunduh untuk perangkat lain).

### 5.2 Integrasi Dinamis GitHub Releases API & Fallback Statis
Supaya unduhan tetap berjalan kalau API tidak bisa dihubungi:
- **Fallback Statis Siaga**: URL binary rilis stabil terbaru (`BuildDev-2026-11-13-23-10-32`) sudah tertanam secara *hardcoded* di dalam halaman. Jika koneksi GitHub API terblokir, *rate-limited*, atau pengguna sedang *offline*, fungsi unduh tetap berjalan seketika.
- **Sinkronisasi Asinkron (Live Update)**: Saat halaman dimuat, skrip menghubungi:
  `https://api.github.com/repos/MuaraAI/FluxCode/releases`
- Begitu respon diterima, skrip secara otomatis:
  1. Memperbarui label badge versi build (misal `BuildDev-YYYY-MM-DD-HH-MM-SS`).
  2. Menyesuaikan tautan `browser_download_url` untuk masing-masing berkas (`.exe`, `.zip`, `.dmg`, `.tar.gz`).
  3. Memperbarui ukuran berkas aktual dalam megabyte (MB).
  4. Menampilkan tanggal terbit rilis terbaru.

### 5.3 Eksekusi Pengunduhan (Download Triggering)
Saat pengguna menekan tombol unduh:
```javascript
function initiateDirectDownload(downloadUrl, filename) {
  const anchor = document.createElement('a');
  anchor.href = downloadUrl;
  if (filename) anchor.download = filename;
  anchor.target = '_self';
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);

  showDownloadFeedback(filename, downloadUrl);
}
```
Tindakan ini:
1. Memicu dialog unduh berkas peramban (*browser native download manager*) tanpa mengarahkan pengguna menjauhi situs.
2. Menampilkan *toast notification* yang mengonfirmasi bahwa pengunduhan sedang berjalan, lengkap dengan tautan darurat cadangan (*"Jika unduhan belum dimulai otomatis, klik di sini"*).

---

## 6. Aksesibilitas & Responsivitas

- **Keyboard Navigable**: Setiap tombol dan kartu interaktif memiliki indikator fokus `:focus-visible` yang jelas dengan aksen kuning oker `#F0D55B`.
- **Prefeers Reduced Motion**: Animasi typing, pulse, dan transisi pergerakan dimatikan bagi pengguna dengan setelan `prefers-reduced-motion: reduce`.
- **Responsive Layout**:
  - Desktop (>1024px): Tampilan layout 2 kolom berdampingan untuk Hero dan Mockup, grid 4 kolom untuk platform download.
  - Tablet (768px - 1024px): Mockup bertumpuk proporsional dengan skala visual adaptif.
  - Mobile (<768px): Navigasi ringkas, tombol unduh berukuran penuh dengan sentuhan nyaman (*thumb friendly min 48px*), tabel benchmark dapat digulir horizontal secara mulus.
- **High Contrast Dark Mode**: Rasio kontras teks terhadap latar belakang dipertahankan di atas 7:1 untuk teks normal dan di atas 4.5:1 untuk teks penjelas sesuai pedoman WCAG AAA.

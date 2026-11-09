# Catatan perubahan FluxCode (edisi GPUI)

Edisi GPUI ada di folder `gpui-app/`. Ini yang berubah di tiap versi, ditulis apa adanya:
apa yang bisa dilakukan sekarang, apa bedanya dengan sebelumnya, dan apa yang belum dites.

## 0.9.1-dev (9 November 2026)

### Sebelum dan sesudah

| | Sebelum (0.9.0) | Sesudah (0.9.1) |
| --- | --- | --- |
| File yang diubah agent | tidak kelihatan, kecuali yang ditulis lewat FluxCode | kartu Perubahan: tiap file dengan +baris -baris, diff, dan undo |
| Scroll roda mouse di chat agent | loncat | meluncur seperti di editor |
| Langkah scroll halus | timer tiap 8 ms | satu langkah per frame layar |
| Pesan error di status bar | path lengkap dan pesan sistem, tidak pernah hilang | kalimat pendek, hilang sendiri setelah 6 detik |
| File yang sedang dipakai program lain (misalnya `LOCK` milik server) | "The process cannot access the file..." | "LOCK sedang dipakai program lain (misalnya server yang jalan)..." |
| Placeholder kotak chat | terpotong di panel sempit | pendek: "Tanya OpenCode..." |
| Harness | OpenCode, Mira, Mura | OpenCode, Mura |
| Breadcrumb file di luar workspace | path lengkap dari C:\ | nama file saja |
| Source Control dengan ribuan file baru | ribuan baris digambar, terasa berat | maksimal 200 baris per bagian, plus daftar folder untuk diabaikan |
| Membatalkan git init | tidak bisa | Hapus Git dari Folder Ini, .git ke Recycle Bin |
| Pengaturan agent | daftar panjang, harus scroll | satu layar: model jadi dropdown, sisanya deretan tombol |

### Yang berubah

- Kartu Perubahan di atas kotak pesan: daftar file yang diubah agent di giliran ini, dengan
  jumlah baris yang ditambah dan dihapus. Klik nama untuk membuka file, tombol diff untuk
  melihat perubahannya di tab baru, tombol undo untuk mengembalikan file itu. Batalkan semua
  dan Simpan semua ada di kepala kartu. FluxCode membaca isi file tepat sebelum agent mengedit
  (atau memakai teks lama dari diff yang dikirim agent), jadi edit yang dilakukan agent sendiri
  juga tercatat. Dites langsung: OpenCode mengedit Main.kt, kartu menunjukkan +2 -1, diff dan
  isi editor ikut berubah.
- Scroll halus sekarang juga di chat agent, dan langkahnya mengikuti frame layar.
- Status bar: pesan baru hilang sendiri setelah 6 detik, dan file yang dikunci program lain
  dijelaskan dengan kalimat biasa.
- Mira (`omm`) dihapus. Pengaturan lama yang memilih Mira sekarang memakai Mura.
- Placeholder kotak chat dipendekkan supaya tidak terpotong.
- Source Control tidak lagi berat saat ada ribuan file: tiap bagian menggambar paling banyak
  200 baris lalu menulis berapa sisanya. Kalau file yang belum dilacak lebih dari 200, muncul
  daftar folder yang paling banyak isinya dengan tombol Abaikan (ditambahkan ke .gitignore).
  Dites dengan 2.801 file: panel langsung terbuka, Abaikan libraries/ menurunkan jadi 1.302.
- Hapus Git dari folder ini (menu ... di Source Control): kebalikan dari Buat Repository.
  Ada konfirmasi yang menyebut repository mana, folder .git dipindah ke Recycle Bin dan file
  project tidak berubah.
- Pengaturan OpenCode/Mura jadi ringkas dan muat tanpa scroll: satu baris per pengaturan
  (label kiri, pilihan kanan, membungkus kalau panel sempit). Model jadi satu dropdown dengan
  model gratis di grup paling atas, mode jadi deretan tombol dengan deskripsi mode yang dipilih
  di bawahnya, lalu effort, akses dan mati-saat-nganggur.

## 0.9.0-dev (9 November 2026)

### Sebelum dan sesudah

Angka di bawah diukur di laptop yang sama (Windows 11), build release, folder dan file
yang sama, nilai tengah dari 3 kali buka. Sebelum = 0.8.2-dev (build pagi ini), sesudah = 0.9.0-dev.

| | Sebelum (0.8.2) | Sesudah (0.9.0) |
| --- | --- | --- |
| Ukuran FluxCode.exe | 28,6 MB | 15,1 MB |
| Dari klik sampai jendela muncul | 231 ms | 226 ms (sama, beda wajar) |
| Memori saat diam (working set) | 120 MB | 123 MB (sama) |
| CPU saat diam | 0,9 % | 0,9 % |
| Bahasa yang diwarnai di editor | 19 | sekitar 200 |
| File biner (`.klog`, `.kchk`, `.exe`, ...) | pesan error, tidak bisa dibuka | tampilan hex, hanya baca |
| Word, Excel, PowerPoint, PDF | tidak bisa dibuka | isi teksnya tampil, hanya baca |
| ZIP, JAR, database SQLite | tidak bisa dibuka | daftar isi ZIP; tabel, SQL dan jumlah baris SQLite |
| File kosong (misalnya `LOCK`) | editor kosong tanpa keterangan | ada keterangan bahwa file-nya kosong |
| Scroll pakai roda mouse | loncat 3 baris sekaligus | meluncur ke tujuan dalam sekitar 0,1 detik |
| Tarik folder ke jendela | tidak bisa | ditanya: tambah ke workspace atau pindah ke folder itu |
| Bahasa tampilan | Inggris saja | Inggris atau Bahasa Indonesia |
| Ganti model / effort / mode agent | harus lewat OpenCode sendiri | dari panel agent |
| Pesan yang sudah terkirim | tidak bisa diubah | bisa diedit, jawaban diulang dari situ |
| Izin agent | auto-approve nyala atau mati | Hanya baca, Baca & tulis, atau Akses penuh |
| Catatan memori di chat | kotak besar setiap agent menyimpan atau memakai catatan | satu baris tipis |
| Memori agent | selalu jalan | bisa dimatikan; agent diminta tidak menjalankan skrip memori Python |
| Harness agent | OpenCode, Mira | OpenCode, Mira, Mura |

Ukuran exe turun karena pewarnaan kode sekarang pakai syntect dengan kumpulan grammar
dari bat, menggantikan 19 grammar tree-sitter yang masing-masing dikompilasi ke dalam
program. Kecepatan buka dan pemakaian memori tidak berubah.

### Editor dan file

- Pewarnaan untuk sekitar 200 bahasa, dipilih dari nama file (`Dockerfile`, `Gemfile`,
  `.env.local`, `requirements-dev.txt`), ekstensi, atau baris `#!` di skrip tanpa ekstensi.
  Nama bahasanya tampil di status bar.
- File yang tidak bisa dibuka sebagai teks sekarang dibuka hanya baca, dengan satu baris
  keterangan di atasnya:
  - file biner tampil sebagai hex (64 KB pertama),
  - Word, Excel, PowerPoint dan OpenDocument tampil sebagai teks (sheet jadi baris berpemisah tab),
  - PDF tampil sebagai teks kalau teksnya bisa diambil,
  - ZIP dan sejenisnya (jar, whl, apk) tampil sebagai daftar file dengan ukurannya,
  - database SQLite tampil sebagai daftar tabel, SQL-nya dan jumlah barisnya.
- File teks UTF-16 (dengan BOM) terbuka sebagai teks.
- Scroll roda mouse di editor meluncur halus. Beberapa putaran cepat dijumlahkan.
  Touchpad tetap pakai scroll bawaan editor.
- Titik penanda spasi sekarang mati secara default karena bikin baris panjang terlihat
  ramai. Bisa dinyalakan lagi di Settings, Whitespace dots.

### Workspace

- Folder bisa ditarik ke jendela. Kalau belum ada folder yang terbuka, folder itu langsung
  dibuka. Kalau sudah ada, FluxCode bertanya: tambahkan ke workspace (folder baru tampil
  sebagai root sendiri di explorer) atau pindah ke folder itu. File yang ditarik dibuka di tab.
- Go to File, Find in Files, `@` di chat dan agent ikut mencari di semua folder workspace.
  Folder tambahan bisa dikeluarkan lewat klik kanan, dan workspace dibuka lagi saat start.

### Bahasa tampilan

- Settings, Bahasa: Otomatis (mengikuti Windows), English, atau Bahasa Indonesia.
  Menu, panel, panel agent dan pesan langsung berganti tanpa restart.

### Panel agent

- Saat sesi baru dimulai, agent dapat ringkasan workspace: folder, jenis project dari
  file seperti `package.json`, `Cargo.toml`, `requirements.txt`, isi awal README, struktur
  folder, jumlah file per bahasa, branch git dan perubahan yang belum di-commit, dan tab
  yang terbuka. Chip Workspace muncul saat ringkasan itu ikut terkirim.
- Pengaturan agent (menu judul panel atau tombol model di bawah kotak pesan): mode agent
  beserta deskripsinya, model (model gratis OpenCode Zen dikelompokkan paling atas) dan
  effort. Pilihan disimpan dan dipakai lagi di sesi berikutnya.
- Pesan yang sudah terkirim bisa diedit lewat ikon pensil. Enter mengirim versi baru dan
  jawaban diulang dari pesan itu, dengan percakapan sebelumnya tetap jadi konteks.
- Akses agent: Hanya baca (edit dan perintah ditolak, agent diberi tahu), Baca & tulis
  (edit file jalan, perintah tanya dulu), Akses penuh (tidak ada yang ditanya).
- Kotak pesan dirapikan: tombol lampiran, `@`, model, akses dan kirim/stop sama ukurannya
  dan sejajar. Status Mengerjakan pindah ke baris bawah kotak pesan, jadi area chat tidak
  terpotong di atas.
- Chip konteks punya tombol x di kiri. Klik namanya membuka file, baris error atau terminal
  yang dimaksud.
- Ctrl+L membuka dan menutup panel agent.
- Kalau model gagal karena kredensial atau kuota, panel menyarankan pilih model lain.

### Mura

- Mura (repo MuaraAI/Mura-Harness, satu program Rust) bisa dipilih di menu judul panel,
  di samping OpenCode dan Mira. Mura tetap program terpisah: build dengan
  `cargo build --release -p mura-cli`, taruh `mura.exe` di PATH atau `~/.local/bin`,
  lalu jalankan `mura setup` sekali untuk provider dan API key-nya.

### Memori

- Tampilan Memori dirapikan: dikelompokkan per project dan semua project, kekuatan catatan
  ditulis sebagai kata (Kuat, Normal, Memudar), ada berapa kali catatan dipakai, dan
  path file dipendekkan.
- Memori bisa dimatikan. Saat mati, tidak ada yang dibaca, disimpan atau dikirim ke agent.
- File memori dibaca sekali dan disimpan di RAM sampai file-nya berubah. Memori FluxCode
  ditulis dalam Rust dan tidak menjalankan proses di belakang. Agent diminta tidak
  menjalankan skill memori Python (`ant_memory.py`) karena FluxCode sudah mengurusnya.

### Belum dites langsung

- Mode Hanya baca saat agent benar-benar minta izin mengedit.
- Pilihan "Pindah ke folder ini" setelah menarik folder.
- Apakah agent selalu menuruti permintaan untuk tidak menjalankan `ant_memory.py`.
  Itu instruksi ke model, bukan pemblokiran.
- Mura menjawab pesan: Mura belum punya API key di PC pengujian, jadi yang dites baru
  koneksi dan pengaturannya.

## 0.6.2 sampai 0.8.2 (8 sampai 9 November 2026)

- Terminal Python: error tampil sebagai kartu dengan file dan baris yang bisa diklik,
  salah ketik seperti `pip i` dibetulkan otomatis, warna dirapikan, path panjang dibungkus.
- Source Control: commit, push, sync, pull request lewat `gh`, dan menjalankan GitHub Actions.
- Go to File (Ctrl+P) di tengah jendela dengan latar diredupkan.
- Penampil gambar, explorer dengan ukuran ala VS Code, tombol jendela yang sekali klik.
- Panel agent: `@file`, chip konteks, riwayat dan melanjutkan sesi OpenCode, memori ala
  AoC Memory, perintah `/`, agent mati sendiri saat nganggur lalu lanjut sesi yang sama.
- Kotak pesan agent baru: lampiran gambar, PDF dan dokumen Office, antrean pesan saat agent
  masih menjawab (bisa dikirim langsung), dan ganti harness tanpa kehilangan chat.

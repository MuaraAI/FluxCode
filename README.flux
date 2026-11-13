// README.flux: FluxCode's README, written in Flux (docs/flux.flux has the syntax).
//
// Two files are made from it, one per language. English is the default, because
// that is what GitHub shows first:
//   fluxcode --flux-to-md README.flux README.md    --lang en
//   fluxcode --flux-to-md README.flux README.id.md --lang id
//
// Every section is written twice, `@en` then `@id`, so a change to one is visible
// next to the other. Blocks with no tag show in both languages: the badges, the
// images that have no words in them, and code.

# FluxCode

@en
**English** · [Bahasa Indonesia](README.id.md)
@id
[English](README.md) · **Bahasa Indonesia**

@en
A native code editor for Windows, macOS and Linux, written in Rust and drawn by the GPU.
One small program with an AI agent panel, a Python-friendly terminal, built-in Source
Control, and read-only views for files other editors refuse to open. No telemetry.

A [MuaraAI](https://github.com/MuaraAI) project, built by a first-semester university
student in Indonesia ([MyKineID](https://github.com/MyKineID)). Open source, and still in
development.
@id
Editor kode native untuk Windows, macOS, dan Linux, ditulis dalam Rust dan digambar oleh
GPU. Satu program kecil dengan panel agent AI, terminal yang ramah Python, Source Control
bawaan, dan tampilan baca-saja untuk file yang tidak mau dibuka editor lain. Tanpa
telemetri.

Proyek [MuaraAI](https://github.com/MuaraAI), dibangun oleh mahasiswa semester satu di
Indonesia ([MyKineID](https://github.com/MyKineID)). Open source, masih dalam
pengembangan.

[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

@en
@image assets/screenshots/agent.png width=100% link=assets/screenshots/agent.png alt="FluxCode with main.py open. In the agent panel on the right, OpenCode has added a docstring to greet(); the Changes card lists main.py with one added line, and a prompt asks before the agent runs a Python command."

The agent added a docstring (line 23), the Changes card lists the file with Undo, and
because access is set to Read & write, it has to ask before running a command.
@id
@image assets/screenshots/agent.png width=100% link=assets/screenshots/agent.png alt="FluxCode dengan main.py terbuka. Di panel agent sebelah kanan, OpenCode menambahkan docstring ke greet(); kartu Changes mendaftar main.py dengan satu baris tambahan, dan ada prompt yang bertanya sebelum agent menjalankan perintah Python."

Agent menambahkan docstring (baris 23), kartu Changes mendaftar file itu beserta Undo,
dan karena akses disetel ke Read & write, agent harus bertanya sebelum menjalankan
perintah.

@en
## Benchmark

On one Windows 11 PC, FluxCode put its window on screen in **0.22 s** and sat at **1.4 MB**
in Task Manager once idle, in one process. Zed took 0.82 s and 66 MB; VS Code 0.29 s and
929 MB over nine processes.
@id
## Benchmark

Di satu PC Windows 11, FluxCode menampilkan jendelanya dalam **0,22 detik** dan menetap
di **1,4 MB** di Task Manager saat idle, dalam satu proses. Zed 0,82 detik dan 66 MB; VS
Code 0,29 detik dan 929 MB di sembilan proses.

@image assets/benchmarks/benchmark-light.svg dark=assets/benchmarks/benchmark-dark.svg width=100% alt="Six bar charts, lower is better in each. Window on screen after launch: FluxCode 221 ms, Zed 818 ms, VS Code 290 ms. Memory in Task Manager when idle: FluxCode 1.4 MB, Zed 66.2 MB, VS Code 929.2 MB. Memory reserved: FluxCode 72 MB, Zed 154 MB, VS Code 1,315 MB. CPU while idle: FluxCode 0.05%, Zed 1.46%, VS Code 5.89% of one core. Processes: FluxCode 1, Zed 2, VS Code 9. Size on disk: FluxCode 16 MB, Zed 261 MB, VS Code 1,044 MB."

@en
*Median of 5 runs each on a PC with an AMD Ryzen 5 7535HS and 15 GB of RAM, 10 November 2026. Every
app started from a fresh, empty profile with telemetry and update checks off and no folder
open.*
@id
*Median dari 5 pengukuran di PC dengan AMD Ryzen 5 7535HS dan 15 GB RAM, 10 November 2026.
Semua aplikasi dijalankan dari profil baru yang kosong, dengan telemetri dan pemeriksaan
update dimatikan serta tanpa folder terbuka.*

@en
@details Exact numbers, method, and what this does not measure

Median, with the lowest and highest of the runs in brackets:

| | FluxCode BuildDev-2026-11-10-04-41-34 | Zed 1.27.0 | VS Code 1.145.2 |
| --- | --- | --- | --- |
| Window on screen after launch (ms) | 221 (216 to 232) | 818 (387 to 1,322) | 290 (268 to 328) |
| Memory in Task Manager, idle (MB) | 1.4 (1.0 to 4.8) | 66.2 (65.2 to 71.9) | 929.2 (867.7 to 985.4) |
| Memory reserved (MB) | 72 (72 to 72) | 154 (153 to 156) | 1,315 (1,249 to 1,399) |
| CPU while idle (% of one core) | 0.05 (0.00 to 0.68) | 1.46 (1.15 to 2.60) | 5.89 (5.57 to 8.12) |
| Processes running | 1 | 2 | 9 |
| Size on disk (MB) | 16 | 261 | 1,044 |

- **Window**: from launch to the first visible window, runs 2 to 5 (run 1 reads the program
  from disk cold). VS Code shows its window before the workbench inside it has loaded.
- **Memory in Task Manager**: the private working set, 9 s after launch, summed over every
  process of the app. FluxCode hands memory it isn't using back to Windows 6 s after start,
  so this is its idle figure; it is about 5 MB after a click and about 30 MB while the agent
  install animation runs. Zed and VS Code keep theirs.
- **Memory reserved**: private bytes (commit), the memory the app has claimed whether or
  not it is in RAM right now.
- **CPU while idle**: CPU time from 9 s to 39 s after launch, as a share of one core.
  Windows counts it in 15.6 ms steps, so FluxCode's 0.05% is at the limit of what it can
  tell apart from zero.
- **Size on disk**: the install folder; for FluxCode, its one program file.
- **Not measured here**: typing latency, opening very large files, working in a big
  project, or anything with extensions and language servers loaded. Zed and VS Code do
  more in those areas (see the comparison table below: FluxCode has no debugger,
  extensions or language servers yet).
- **Reproduce**: close all three apps, then run
  `powershell -ExecutionPolicy Bypass -File tools/bench/bench.ps1 -FluxExe <path to fluxcode.exe>`
  and `python tools/bench/chart.py`. The raw runs are in
  [`tools/bench/results.json`](tools/bench/results.json).

@end
@id
@details Angka lengkap, metodenya, dan apa yang tidak diukur di sini

Median, dengan nilai terendah dan tertinggi tiap pengukuran di dalam tanda kurung:

| | FluxCode BuildDev-2026-11-10-04-41-34 | Zed 1.27.0 | VS Code 1.145.2 |
| --- | --- | --- | --- |
| Jendela tampil setelah diluncurkan (ms) | 221 (216 sampai 232) | 818 (387 sampai 1.322) | 290 (268 sampai 328) |
| Memori di Task Manager, idle (MB) | 1,4 (1,0 sampai 4,8) | 66,2 (65,2 sampai 71,9) | 929,2 (867,7 sampai 985,4) |
| Memori yang direservasi (MB) | 72 (72 sampai 72) | 154 (153 sampai 156) | 1.315 (1.249 sampai 1.399) |
| CPU saat idle (% satu core) | 0,05 (0,00 sampai 0,68) | 1,46 (1,15 sampai 2,60) | 5,89 (5,57 sampai 8,12) |
| Proses berjalan | 1 | 2 | 9 |
| Ukuran di disk (MB) | 16 | 261 | 1.044 |

- **Jendela**: dari diluncurkan sampai jendela pertama terlihat, pengukuran ke-2 sampai
  ke-5 (pengukuran pertama membaca program dari disk dalam keadaan dingin). VS Code
  menampilkan jendelanya sebelum workbench di dalamnya selesai dimuat.
- **Memori di Task Manager**: private working set, 9 detik setelah diluncurkan, dijumlahkan
  dari semua proses aplikasi. FluxCode mengembalikan memori yang tidak dipakai ke Windows
  6 detik setelah start, jadi angka ini adalah kondisi idle-nya; sekitar 5 MB setelah satu
  klik dan sekitar 30 MB saat animasi pemasangan agent berjalan. Zed dan VS Code menyimpan
  memorinya.
- **Memori yang direservasi**: private bytes (commit), memori yang sudah diklaim aplikasi
  entah sedang berada di RAM atau tidak.
- **CPU saat idle**: waktu CPU dari detik ke-9 sampai ke-39 setelah diluncurkan, sebagai
  bagian dari satu core. Windows menghitungnya dalam langkah 15,6 ms, jadi 0,05% milik
  FluxCode sudah di batas kemampuan pengukuran untuk membedakannya dari nol.
- **Ukuran di disk**: folder pemasangan; untuk FluxCode, satu file programnya.
- **Yang tidak diukur di sini**: latensi mengetik, membuka file sangat besar, bekerja di
  proyek besar, atau apa pun dengan extension dan language server termuat. Zed dan VS Code
  lebih jauh di area itu (lihat tabel perbandingan di bawah: FluxCode belum punya debugger,
  extension, maupun language server).
- **Cara mengulang**: tutup ketiga aplikasi, lalu jalankan
  `powershell -ExecutionPolicy Bypass -File tools/bench/bench.ps1 -FluxExe <path ke fluxcode.exe>`
  dan `python tools/bench/chart.py`. Data mentahnya ada di
  [`tools/bench/results.json`](tools/bench/results.json).

@end

@en
## Download

Get the newest build from [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| System | File | Install |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Run it. No admin rights: it installs for your account, with a Start Menu and a Desktop shortcut. |
| macOS 11+, Apple silicon and Intel | `FluxCode-BuildDev-...-macos-universal.dmg` | Open it and drag FluxCode to Applications. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Extract, run `./install.sh`. No root. |
| Windows, portable | `FluxCode-BuildDev-...-windows-x64.zip` | Extract; `FluxCode.exe` runs from the folder (`Install.cmd` installs it). |

The macOS build isn't notarized by Apple yet (that takes an Apple Developer ID), so the
first open asks once: on macOS 15 and newer, open it, then System Settings > Privacy &
Security > Open Anyway; on older ones, right-click FluxCode > Open. Uninstall: Settings >
Apps on Windows, the Trash on macOS, `./install.sh --uninstall` on Linux.

There are no version numbers yet: every change merged into `main` becomes a build named
after the moment it was made, like `BuildDev-2026-11-09-18-19-54`. Installed copies offer
the new one in a card and update themselves in one click. Settings > Updates turns the
check off.
@id
## Unduh

Ambil build terbaru dari [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| Sistem | File | Pemasangan |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Jalankan. Tanpa hak admin: dipasang untuk akunmu, lengkap dengan pintasan Start Menu dan Desktop. |
| macOS 11+, Apple silicon dan Intel | `FluxCode-BuildDev-...-macos-universal.dmg` | Buka, lalu tarik FluxCode ke Applications. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Ekstrak, jalankan `./install.sh`. Tanpa root. |
| Windows, portable | `FluxCode-BuildDev-...-windows-x64.zip` | Ekstrak; `FluxCode.exe` langsung jalan dari foldernya (`Install.cmd` memasangnya). |

Build macOS belum dinotarisasi Apple (butuh Apple Developer ID), jadi pembukaan pertama
bertanya sekali: di macOS 15 ke atas, buka aplikasinya, lalu System Settings > Privacy &
Security > Open Anyway; di versi lama, klik kanan FluxCode > Open. Uninstall: Settings >
Apps di Windows, Trash di macOS, `./install.sh --uninstall` di Linux.

Belum ada nomor versi: setiap perubahan yang masuk ke `main` menjadi build yang dinamai
menurut waktu pembuatannya, seperti `BuildDev-2026-11-09-18-19-54`. Salinan yang sudah
terpasang menawarkan build baru lewat kartu dan memperbarui dirinya dengan satu klik.
Settings > Updates mematikan pemeriksaannya.

@en
## A closer look
@id
## Lebih dekat

@gallery
@image assets/screenshots/python-error.png width=49% link=assets/screenshots/python-error.png alt="A Python run fails with ModuleNotFoundError. The Output panel shows a card with the file and line, the code line, a hint, and a Fix button that runs py -m pip install tabulate."
@image assets/screenshots/sqlite.png width=49% link=assets/screenshots/sqlite.png alt="A SQLite database opened as text: each table and view with its CREATE statement and row count, under a line saying the view is read-only."
@end

@en
A Python error becomes a card with the file, the line and a hint; for a missing module, Fix
installs it. A SQLite database opens as its tables, SQL and row counts, like Word, Excel,
PDF and ZIP files do, instead of an error.
@id
Error Python menjadi kartu berisi file, baris, dan petunjuk; kalau modulnya tidak ada, Fix
memasangnya. Database SQLite terbuka sebagai daftar tabel, SQL, dan jumlah barisnya, sama
seperti file Word, Excel, PDF, dan ZIP, bukan sebagai error.

@gallery
@image assets/screenshots/source-control.png width=49% link=assets/screenshots/source-control.png alt="Source Control after git init in a game server folder: 2801 untracked files, with the folders holding most of them listed first, each with an Ignore button."
@image assets/screenshots/go-to-file.png width=49% link=assets/screenshots/go-to-file.png alt="Go to File in the FluxMono theme: typing wbench finds workbench.rs in the src folder."
@end

@en
Source Control stays fast with thousands of new files and offers to ignore the folders
that hold them. Go to File (`Ctrl+P`) finds a file from a few letters of its name.
@id
Source Control tetap cepat dengan ribuan file baru dan menawarkan untuk mengabaikan folder
yang menampungnya. Go to File (`Ctrl+P`) menemukan file dari beberapa huruf namanya.

@gallery
@image assets/screenshots/themes.png width=49% link=assets/screenshots/themes.png alt="Settings: six color themes as cards (FluxDark, FluxCream, FluxMono, FluxRed, FluxMuara, FluxMuara Dark), then language and interface size."
@image assets/screenshots/update-indonesian.png width=49% link=assets/screenshots/update-indonesian.png alt="The FluxCream theme with the interface in Bahasa Indonesia. A card in the corner offers a newer build, with its notes and a button to update and restart."
@end

@en
Six color themes, three interface sizes and your own background image. The interface
also comes in Bahasa Indonesia, shown here with the update card in the FluxCream theme.
@id
Enam tema warna, tiga ukuran antarmuka, dan gambar latar pilihanmu sendiri. Antarmukanya
juga tersedia dalam Bahasa Indonesia, terlihat di sini bersama kartu update pada tema
FluxCream.

@en
## What makes it different

- **Laid out and driven like VS Code.** Activity bar, side bar, tabs, panel and status
  bar sit where VS Code puts them, and the keys are VS Code's: `Ctrl+Shift+P` opens the
  Command Palette with every command and its shortcut, `Ctrl+P` finds a file, `Ctrl+G`
  goes to a line, `Ctrl+/` comments, `Alt+Up` moves a line. If you know VS Code, you
  already know where things are. It is not a VS Code clone: there is no debugger yet, and
  the extensions are build tasks rather than a catalog (see the table below).
- **Light like Zed.** The whole editor is one Rust program drawn by the GPU with GPUI,
  the framework Zed is built on. In the [benchmark](#benchmark) it opens its window in
  0.22 s, reserves 72 MB (Zed: 154 MB), and once idle shows 1.4 MB in Task Manager,
  because it hands the memory it isn't using back to Windows.
- **Built for AI agents.** The agent panel speaks the Agent Client Protocol and lists
  every agent in the ACP registry, each with its own icon. Missing ones install with one
  click into FluxCode's own folder. You choose the model, effort, mode and permissions
  under the message box; the agent can ask you multiple-choice questions and has to ask
  before it runs a command or changes a file; each turn's file changes can be undone; the
  agent's instruction files are one click away in Settings.
- **For beginners and experts.** A beginner can pick an agent from a list, answer its
  questions by clicking, and keep it read-only. An expert gets the palette, the shortcuts,
  per-agent instruction files and every editor option in Settings: font, size, tab size,
  word wrap, whitespace, line numbers, folding, bracket closing, smart indent, cursor
  padding, scrolling past the end, interface size, themes and a background image.
@id
## Yang membedakannya

- **Tata letak dan kendali seperti VS Code.** Activity bar, side bar, tab, panel, dan
  status bar berada di tempat yang sama dengan VS Code, dan tombolnya juga punya VS Code:
  `Ctrl+Shift+P` membuka Command Palette berisi semua perintah dan pintasannya, `Ctrl+P`
  mencari file, `Ctrl+G` menuju baris, `Ctrl+/` mengomentari, `Alt+Up` memindah baris.
  Kalau kamu sudah kenal VS Code, kamu sudah tahu letak semuanya. Ini bukan tiruan VS
  Code: belum ada debugger, dan extension-nya berupa task build, bukan katalog (lihat tabel
  di bawah).
- **Ringan seperti Zed.** Seluruh editor adalah satu program Rust yang digambar GPU lewat
  GPUI, framework yang dipakai Zed. Di [benchmark](#benchmark) jendelanya muncul dalam
  0,22 detik, mereservasi 72 MB (Zed: 154 MB), dan saat idle menampilkan 1,4 MB di Task
  Manager, karena memori yang tidak dipakai dikembalikan ke Windows.
- **Dibuat untuk agent AI.** Panel agent berbicara lewat Agent Client Protocol dan
  mendaftar semua agent di registri ACP, masing-masing dengan ikonnya sendiri. Yang belum
  terpasang bisa dipasang dengan satu klik ke folder milik FluxCode. Model, effort, mode,
  dan izin dipilih di bawah kotak pesan; agent bisa mengajukan pertanyaan pilihan ganda dan
  harus bertanya sebelum menjalankan perintah atau mengubah file; perubahan file tiap
  giliran bisa dibatalkan; file instruksi agent berjarak satu klik di Settings.
- **Untuk pemula dan untuk yang sudah ahli.** Pemula bisa memilih agent dari daftar,
  menjawab pertanyaannya dengan klik, dan menjaganya tetap read-only. Yang sudah ahli
  mendapat palette, pintasan, file instruksi per agent, dan semua opsi editor di Settings:
  font, ukuran, lebar tab, word wrap, spasi, nomor baris, lipatan, penutup bracket, indent
  pintar, padding kursor, scroll melewati akhir baris, ukuran antarmuka, tema, dan gambar
  latar.

@en
## Live share

Code together in real time, with your PC as the server. The people icon in the activity
bar opens the Live share page in the editor area: its cards sit two to a row on a wide
window and one under the other on a narrow one.

1. The host opens a folder, clicks **Start hosting** and sends the invite code
   (`FLUX1-...`) to the others, in a chat or by mail.
2. Each guest pastes it and clicks **Join**, either into their own clone of the same
   repository or into a new folder that fills as they open files.
3. Everyone types in the same files at once. The host's PC orders the edits
   (operational transformation), so everyone ends with the same text.

The panel lists who is in the session (by GitHub name when signed in) and which file and
line each person is on. Its activity list shows each person's edits with lines added and
removed and a diff; an edit made by an AI agent carries an **AI** label with the agent,
model, effort and mode it ran with, so you can see which model wrote what.

Edits go between the host's PC and each guest, encrypted with ChaCha20-Poly1305 under
the secret in the invite code; no server of ours is involved, and nobody needs a VPN or a
rented server. When hosting starts, FluxCode opens every way in it can and the panel
shows each one, its address, or why it isn't open:

| Way in | Who can use it | What it needs |
| --- | --- | --- |
| Same network | anyone on the host's Wi-Fi or LAN | nothing (Windows asks once to let FluxCode through its firewall) |
| Router port (UPnP) | anyone on the internet | a router with UPnP on, like the ones game servers open ports on; not possible when the provider shares one public address between customers (carrier-grade NAT), which the panel detects and says |
| Public IPv6 | guests with IPv6 | an IPv6 address from the provider, and a router that lets connections in |
| Relay (MuaraAI) | anyone on the internet | nothing: it works behind any router and carrier-grade NAT. Shows as Relay (public) when the fallback below carries the session |

The invite code carries every address that opened. The guest tries them all at once, the
same network first, and uses the first host that proves it holds the code's secret.
The relay is a TCP tunnel run by MuaraAI: it passes bytes that are already encrypted, so
it can see that two PCs are talking, but not what they send. It costs a little speed (each
keystroke travels to `live.muaraai.com` and back). When MuaraAI's relay cannot be reached,
FluxCode falls back to the public [bore.pub](https://github.com/ekzhang/bore) tunnel and
the panel relabels the row **Relay (public)**, so you can tell which one carries the session.
Unticking **Let people join over the internet** before hosting keeps the session on the
network only. The router's port is closed again when the session ends.

Running your own relay is one command on a VPS: see [`relay/README.md`](relay/README.md).

- Only files someone has open travel, so a large project (a game, a Laravel or Android
  project) costs no more than the files being worked on. Files over 16 MB are not shared.
  Environment files, private keys, and build folders like `node_modules` or `target` are
  never shared, even if a guest asks for them by name.
- A guest cannot flood the host: joins and opens are rate limited, an edit is charged by
  how much it changes, and the number of guests is capped.
- Not yet: other people's carets drawn inside the editor (the panel shows their line),
  separate undo per person (`Ctrl+Z` can undo a change someone else made on your screen),
  and syncing files an agent edits without them being open.
@id
## Live share

Ngoding bersama secara real time, dengan PC-mu sebagai servernya. Ikon orang di activity
bar membuka halaman Live share di area editor: kartunya tersusun dua per baris di jendela
lebar dan bertumpuk satu per satu di jendela sempit.

1. Host membuka folder, menekan **Start hosting**, lalu mengirim kode undangan
   (`FLUX1-...`) ke yang lain, lewat chat atau email.
2. Tiap tamu menempelkan kodenya dan menekan **Join**, entah ke klon repositori yang sama
   miliknya sendiri atau ke folder baru yang terisi saat file dibuka.
3. Semua orang mengetik di file yang sama sekaligus. PC host yang menentukan urutan
   perubahan (operational transformation), jadi teks di semua sisi berakhir sama.

Panel mendaftar siapa saja yang ada di sesi (dengan nama GitHub kalau sudah masuk) beserta
file dan baris tempat masing-masing berada. Daftar aktivitasnya menampilkan perubahan tiap
orang dengan jumlah baris yang ditambah dan dihapus serta diff-nya; perubahan dari agent AI
membawa label **AI** berisi agent, model, effort, dan mode yang dipakai, jadi kelihatan
model mana yang menulis apa.

Perubahan mengalir antara PC host dan tiap tamu, dienkripsi ChaCha20-Poly1305 dengan
rahasia dari kode undangan; tidak ada server kami yang terlibat, dan tidak ada yang perlu
VPN atau server sewaan. Saat hosting dimulai, FluxCode membuka setiap jalur masuk yang
bisa, dan panel menampilkan tiap jalur, alamatnya, atau alasan kenapa tidak terbuka:

| Jalur masuk | Siapa yang bisa memakai | Syaratnya |
| --- | --- | --- |
| Jaringan yang sama | siapa pun di Wi-Fi atau LAN host | tidak ada (Windows sekali bertanya untuk mengizinkan FluxCode lewat firewall) |
| Port router (UPnP) | siapa pun di internet | router dengan UPnP aktif, seperti yang dipakai server game; tidak mungkin kalau provider membagi satu alamat publik ke banyak pelanggan (carrier-grade NAT), dan panel mendeteksi serta menjelaskannya |
| IPv6 publik | tamu yang punya IPv6 | alamat IPv6 dari provider, dan router yang mengizinkan koneksi masuk |
| Relay MuaraAI | siapa pun di internet | tidak ada: jalan di balik router apa pun dan carrier-grade NAT. Berubah jadi Relay publik kalau cadangan di bawah yang membawa sesi |

Kode undangan memuat semua alamat yang terbuka. Tamu mencobanya sekaligus, jaringan yang
sama lebih dulu, dan memakai host pertama yang membuktikan bahwa ia memegang rahasia kode
tersebut.
Relay-nya adalah tunnel TCP yang dijalankan MuaraAI: yang lewat sudah terenkripsi, jadi
relay tahu ada dua PC yang sedang bicara, tapi tidak tahu isinya. Sedikit mengurangi
kecepatan (tiap ketikan menempuh perjalanan ke `live.muaraai.com` dan kembali). Kalau
relay MuaraAI tidak bisa dihubungi, FluxCode beralih ke tunnel publik
[bore.pub](https://github.com/ekzhang/bore) dan panel mengubah barisnya menjadi
**Relay publik**, jadi kelihatan mana yang membawa sesi. Menghapus centang **Let people
join over the internet** sebelum hosting membuat sesi hanya di jaringan lokal. Port router
ditutup kembali saat sesi berakhir.

Menjalankan relay sendiri cukup satu perintah di VPS: lihat
[`relay/README.md`](relay/README.md).

- Hanya file yang sedang dibuka seseorang yang dikirim, jadi proyek besar (game, proyek
  Laravel atau Android) hanya sebesar file yang sedang dikerjakan. File di atas 16 MB tidak
  dibagikan. File environment, kunci privat, dan folder build seperti `node_modules` atau
  `target` tidak pernah dibagikan, walau tamu memintanya dengan nama yang tepat.
- Tamu tidak bisa membanjiri host: join dan open dibatasi lajunya, perubahan dihitung
  sebesar yang diubah, dan jumlah tamu dibatasi.
- Belum ada: kursor orang lain yang digambar di dalam editor (panel hanya menunjukkan
  barisnya), undo terpisah per orang (`Ctrl+Z` bisa membatalkan perubahan orang lain di
  layarmu), dan sinkronisasi file yang diubah agent tanpa file itu dibuka.

@en
## Extensions and FluxCompile

The Extensions view (`Ctrl+Shift+X`) lists what FluxCode can build, run, pack and preview:
Paper plugins, Bedrock add-ons, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ (CMake
and Make), .NET and the HTML preview. An extension is one small JSON file: the files that
mark its projects, the tasks it adds, the tools it needs (with a winget id to install
them) and notes for the AI agent. Nothing runs until you click a task, so installing one
costs a few KB. More can be installed from an `extension.json` file.

Once an extension is installed, the Run view lists the folder's projects with their tasks,
and the agent is told what each project is and how it builds.

- **Paper plugins (Minecraft Java).** Build, Build & deploy, Start server, Test, Clean. The
  project's name is its plugin's name, read from the `plugin.yml` inside the built jar. Deploy
  copies the jar into the test server's `plugins` folder as `<Name>-<version>.jar` and
  removes older builds of the same plugin (found by the name inside each jar). The server is
  the folder above the plugin with a `plugins` folder and a `paper-*.jar`. New plugin writes
  a Gradle project for that server's version, with the Gradle wrapper.
- **Bedrock add-ons.** Check manifests (UUIDs, versions, module types, script entries,
  dependencies between the packs), Deploy to Minecraft (into the game's development pack
  folders) and Pack `.mcaddon`, which FluxCode zips itself. New add-on writes a behavior
  and a resource pack that depend on each other.
- **FluxCompile** is how FluxCode runs a Java build. It uses the JDK the project asks for
  (a Gradle toolchain or Maven `release`, found in the usual install folders or a
  `tools/jdk-*` folder next to the project), Gradle's local build cache unless the project
  turns it off, and the project's own settings for parallel builds and the configuration
  cache. It folds Gradle's task lines into one progress line and ends with what was built.
  A compiler error opens the file at its line. Forcing the configuration cache or parallel
  builds from outside can break a build, so FluxCompile doesn't; we checked how 28 large
  plugins (LuckPerms, EssentialsX, WorldEdit, Geyser, ViaVersion and others) build before
  choosing.
- Measured on a multi-module Paper plugin (55 Gradle tasks, a 609 KB jar): the first
  build took 14.6 s (40 tasks run, 15 from the cache), a build with nothing changed 1.9 s
  with Gradle's daemon warm.
@id
## Extension dan FluxCompile

Tampilan Extensions (`Ctrl+Shift+X`) mendaftar apa yang bisa dibangun, dijalankan, dikemas,
dan dipratinjau FluxCode: plugin Paper, add-on Bedrock, Gradle, Maven, Node.js, Rust,
Python, Go, C/C++ (CMake dan Make), .NET, dan pratinjau HTML. Satu extension adalah satu
file JSON kecil: file yang menandai proyeknya, task yang ditambahkannya, alat yang
dibutuhkannya (dengan id winget untuk memasangnya), dan catatan untuk agent AI. Tidak ada
yang berjalan sampai kamu menekan sebuah task, jadi memasangnya hanya memakai beberapa KB.
Extension lain bisa dipasang dari file `extension.json`.

Setelah extension terpasang, tampilan Run mendaftar proyek di folder itu beserta task-nya,
dan agent diberi tahu proyek itu apa serta bagaimana ia dibangun.

- **Plugin Paper (Minecraft Java).** Build, Build & deploy, Start server, Test, Clean. Nama
  proyeknya adalah nama plugin-nya, dibaca dari `plugin.yml` di dalam jar hasil build.
  Deploy menyalin jar ke folder `plugins` server uji sebagai `<Nama>-<versi>.jar` dan
  membuang build lama plugin yang sama (dikenali dari nama di dalam tiap jar). Servernya
  adalah folder di atas plugin yang punya folder `plugins` dan `paper-*.jar`. Plugin baru
  menulis proyek Gradle untuk versi server itu, lengkap dengan Gradle wrapper.
- **Add-on Bedrock.** Check manifest (UUID, versi, tipe modul, entri script, dependensi
  antar pack), Deploy to Minecraft (ke folder development pack milik gamenya), dan Pack
  `.mcaddon` yang di-zip sendiri oleh FluxCode. Add-on baru menulis behavior pack dan
  resource pack yang saling bergantung.
- **FluxCompile** adalah cara FluxCode menjalankan build Java. Ia memakai JDK yang diminta
  proyek (toolchain Gradle atau `release` Maven, dicari di folder pemasangan biasa atau
  folder `tools/jdk-*` di sebelah proyek), build cache lokal Gradle kecuali proyeknya
  mematikannya, serta pengaturan proyek sendiri untuk build paralel dan configuration
  cache. Baris task Gradle dilipat jadi satu baris progres dan ditutup dengan apa yang
  berhasil dibangun. Error kompilasi membuka file di barisnya. Memaksa configuration cache
  atau build paralel dari luar bisa merusak build, jadi FluxCompile tidak melakukannya;
  kami memeriksa bagaimana 28 plugin besar (LuckPerms, EssentialsX, WorldEdit, Geyser,
  ViaVersion, dan lainnya) dibangun sebelum memutuskan.
- Diukur pada plugin Paper multi-modul (55 task Gradle, jar 609 KB): build pertama 14,6
  detik (40 task jalan, 15 dari cache), build tanpa perubahan 1,9 detik dengan daemon
  Gradle sudah panas.

@en
## Flux documents

This README is written in Flux (`README.flux`); the `README.md` GitHub shows is made from
it. A `.flux` file is Markdown's text plus one-line blocks for what Markdown leaves to
HTML: image width and dark-theme versions, images side by side, video, flowcharts,
callouts, collapsible sections, columns, a table of contents and sections per language.
FluxCode opens it drawn, with Preview and Source above it.

- **Light at any size**: one pass over the text, nothing copied, only the rows on screen
  drawn, and a large document's text kept packed. A 1.3-million-line document (31.8 MB, a
  200,000-row table in it) shows its first screen after 0.08 ms of reading, opens in
  0.3 s, and shows 16 MB in Task Manager (its text kept in 2.3 MB).
- **Readable on GitHub**: `fluxcode --flux-to-md README.flux README.md` (or **Save as
  Markdown** in FluxCode) writes GitHub Markdown; flowcharts become Mermaid and callouts
  GitHub's alerts. Add `--lang id` for the Indonesian sections, which ship as
  `README.id.md`. The tests fail when any of those files disagree.
@id
## Dokumen Flux

README ini ditulis dalam Flux (`README.flux`); `README.md` yang tampil di GitHub dibuat
dari situ. File `.flux` adalah teks Markdown ditambah blok satu baris untuk hal yang
biasanya diserahkan Markdown ke HTML: lebar gambar dan versi tema gelap, gambar
berdampingan, video, flowchart, callout, bagian yang bisa dilipat, kolom, daftar isi, dan
bagian per bahasa. FluxCode membukanya dalam bentuk tergambar, dengan Preview dan Source
di atasnya.

- **Ringan di ukuran apa pun**: satu kali pembacaan teks, tanpa salinan, hanya baris yang
  terlihat yang digambar, dan teks dokumen besar disimpan dalam bentuk padat. Dokumen
  1,3 juta baris (31,8 MB, berisi tabel 200.000 baris) menampilkan layar pertamanya setelah
  0,08 ms pembacaan, terbuka dalam 0,3 detik, dan menampilkan 16 MB di Task Manager
  (teksnya disimpan dalam 2,3 MB).
- **Terbaca di GitHub**: `fluxcode --flux-to-md README.flux README.md` (atau **Save as
  Markdown** di FluxCode) menulis Markdown untuk GitHub; flowchart jadi Mermaid dan
  callout jadi alert milik GitHub. Tambahkan `--lang id` untuk bagian Indonesianya, yang
  tersedia sebagai `README.id.md`. Test gagal kalau file-file itu tidak sama.

@en
@tip The whole syntax
[docs/flux.md](docs/flux.md) describes every block, written in Flux itself
([docs/flux.flux](docs/flux.flux)).
@end
@id
@tip Seluruh sintaksnya
[docs/flux.id.md](docs/flux.id.md) menjelaskan semua blok, ditulis dalam Flux itu sendiri
([docs/flux.flux](docs/flux.flux)).
@end

@en
## What else it does

- Syntax colors for about 200 languages, picked by file name, extension or `#!` line.
- Multi-root workspaces: drag a folder onto the window to add it or switch to it.
- The agent panel (`Ctrl+L`) runs any agent in the [ACP registry](https://agentclientprotocol.com),
  such as OpenCode 2, Claude Agent, Codex, Gemini CLI, GitHub Copilot or MuaraAI's Mura. An
  agent that isn't on your PC has a download button in the agent menu (with the agent's own
  icon from the registry); FluxCode puts it in
  `~/.fluxcode/agents` (npm packages too, not in your global npm) and starts it only while
  you use the panel. Model, effort, mode and permissions are buttons under the message
  box; each opens a list right above it with a search box, and the agent menu says which
  agent is on. GitHub Copilot, which offers no models over ACP, gets its model list from
  Copilot itself and restarts with the one you pick, in the same chat. Edit a sent message
  and the answer restarts from it; the agent gets an overview of the workspace. Memory is
  the agent's own (FluxCode adds none of its own).
- **Effort and models, with what they cost.** Effort is a slider (low to max) that starts at
  medium: drag it, or click a level. Each model shows how many tokens it reads at once and
  writes in one answer, and free models are marked Free (from the model list OpenCode keeps
  on your PC). Claude Agent's models are grouped by generation, newest first, each with its
  price per million tokens, since the newer ones do the same work with fewer tokens.
- **Usage without typing `/usage`.** A meter under the message box shows how full the
  context is; it, the gauge button or typing `/usage` opens a panel with the plan's limits
  (the 5-hour and weekly ones), when each resets, the context window, the session's cost and
  what uses the limits most, all read from the agent itself. Limits reset by themselves;
  Request usage credits asks first, then asks the plan for more and shows the answer.
- **Web searches you can follow.** When the agent searches the web or reads a page, the
  chat shows the query and its sources, each one click away.
- **What the agent is doing, as it does it.** Thinking, reading, editing, running a command
  and searching are steps on one line: a running step has a thin moving line under it, and
  when it ends its label turns from "Reading" to "Read" and shows how long it took.
- **A browser for every agent.** FluxCode hands each agent session a browser it can
  drive: the Chrome, Brave or Edge already on your PC (Chrome when it is there), through
  Google's [chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp)
  over the Chrome DevTools Protocol. The agent opens pages, clicks and types, reads the
  console and network requests, takes screenshots and accessibility snapshots, and records
  performance traces. Nothing is downloaded but the 15 MB tool, installed once into
  `~/.fluxcode/tools/browser`; the browser gets a profile of its own, never your cookies
  or passwords, and Google's usage statistics are turned off. Agent settings > Browser
  picks the browser, a window or none, or turns it off.
- Source Control commits, pushes, pulls and syncs, shows a commit graph, opens pull
  requests and GitHub Actions runs through `gh`, and Remove Git undoes a `git init`.
- Source Control tells you when the remote has commits you don't. It runs a quiet
  `git fetch` every five minutes (and when you come back to the window), never asking
  for a password, and an Incoming section at the top lists the new commits: title and
  author, then the files each one changed, and a file's diff in the Output panel. Pull
  brings them in, or put them aside until more arrive. A blue dot on the Source Control
  icon says they are there.
- After an update, a "What's new" page opens once with the notes since your last build
  (from [`changelog.flux`](changelog.flux), in the interface language). Help > What's New
  opens it again.
- OpenCode 2 is the registry's native OpenCode build from opencode.ai, which FluxCode
  installs and checks against its SHA-256. A copy of OpenCode on your PATH is no longer
  used, and an existing OpenCode choice moves to OpenCode 2 with its settings.
- The color theme's name in the status bar opens the six themes, each shown as a small
  editor in its colors.
- Settings > About FluxCode links to this repository, the issue tracker and the changelog.
@id
## Kemampuan lainnya

- Pewarnaan sintaks untuk sekitar 200 bahasa, dipilih dari nama file, ekstensi, atau baris
  `#!`.
- Workspace multi-root: tarik folder ke jendela untuk menambahkannya atau berpindah ke
  situ.
- Panel agent (`Ctrl+L`) menjalankan agent apa pun di [registri ACP](https://agentclientprotocol.com),
  seperti OpenCode 2, Claude Agent, Codex, Gemini CLI, GitHub Copilot, atau Mura dari
  MuaraAI. Agent yang belum ada di PC-mu punya tombol unduh di menu agent (lengkap dengan
  ikonnya sendiri dari registri); FluxCode menaruhnya di `~/.fluxcode/agents` (termasuk
  paket npm, tidak ke npm global-mu) dan hanya menjalankannya selama panel dipakai. Model,
  effort, mode, dan izin berupa tombol di bawah kotak pesan; masing-masing membuka daftar
  tepat di atasnya dengan kotak pencarian, dan menu agent menunjukkan agent mana yang
  aktif. GitHub Copilot, yang tidak menawarkan model lewat ACP, mengambil daftar modelnya
  langsung dari Copilot dan restart dengan model pilihanmu, di chat yang sama. Sunting
  pesan yang sudah terkirim dan jawabannya dimulai ulang dari situ; agent mendapat
  ringkasan workspace. Memori adalah milik agent itu sendiri (FluxCode tidak menambahkan
  memorinya sendiri).
- **Effort dan model, beserta harganya.** Effort berupa slider (low sampai max) yang mulai
  di medium: tarik, atau klik satu tingkat. Tiap model menunjukkan berapa token yang
  dibacanya sekaligus dan ditulisnya dalam satu jawaban, dan model gratis ditandai Free
  (dari daftar model yang disimpan OpenCode di PC-mu). Model Claude Agent dikelompokkan per
  generasi, terbaru lebih dulu, masing-masing dengan harga per juta token, karena yang
  lebih baru mengerjakan pekerjaan sama dengan token lebih sedikit.
- **Penggunaan tanpa mengetik `/usage`.** Meter di bawah kotak pesan menunjukkan seberapa
  penuh konteksnya; meter itu, tombol gauge, atau mengetik `/usage` membuka panel berisi
  batas paket (yang 5 jam dan mingguan), kapan masing-masing direset, context window, biaya
  sesi, dan apa yang paling banyak memakai batas, semuanya dibaca dari agent itu sendiri.
  Batasnya direset sendiri; Request usage credits bertanya dulu, lalu meminta tambahan ke
  paket dan menampilkan jawabannya.
- **Pencarian web yang bisa diikuti.** Saat agent mencari di web atau membaca halaman,
  chat menampilkan kueri dan sumbernya, masing-masing berjarak satu klik.
- **Apa yang sedang dilakukan agent, saat melakukannya.** Thinking, reading, editing,
  menjalankan perintah, dan searching adalah langkah-langkah dalam satu baris: langkah yang
  berjalan punya garis tipis bergerak di bawahnya, dan saat selesai labelnya berubah dari
  "Reading" menjadi "Read" beserta lamanya.
- **Browser untuk setiap agent.** FluxCode memberi tiap sesi agent sebuah browser yang
  bisa dikendalikannya: Chrome, Brave, atau Edge yang sudah ada di PC-mu (Chrome kalau
  tersedia), lewat [chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp)
  dari Google melalui Chrome DevTools Protocol. Agent membuka halaman, mengklik dan
  mengetik, membaca console dan permintaan jaringan, mengambil screenshot dan snapshot
  aksesibilitas, serta merekam trace performa. Tidak ada yang diunduh selain alat 15 MB
  itu, dipasang sekali ke `~/.fluxcode/tools/browser`; browsernya memakai profil sendiri,
  tidak pernah cookie atau password-mu, dan statistik penggunaan Google dimatikan. Agent
  settings > Browser memilih browsernya, berupa jendela atau tanpa jendela, atau
  mematikannya.
- Source Control melakukan commit, push, pull, dan sync, menampilkan graf commit, membuka
  pull request dan GitHub Actions lewat `gh`, serta Remove Git untuk membatalkan `git init`.
- Source Control memberi tahu kalau remote punya commit yang belum kamu miliki. Ia
  menjalankan `git fetch` senyap tiap lima menit (dan saat kamu kembali ke jendela), tanpa
  pernah meminta password, dan bagian Incoming di atas mendaftar commit baru: judul dan
  penulisnya, lalu file yang diubah tiap commit, dan diff sebuah file di panel Output.
  Pull menariknya masuk, atau sisihkan sampai ada yang lain datang. Titik biru di ikon
  Source Control menandakan ada yang baru.
- Setelah update, halaman "What's new" terbuka sekali berisi catatan sejak build terakhir
  (dari [`changelog.flux`](changelog.flux), dalam bahasa antarmuka). Help > What's New
  membukanya lagi.
- OpenCode 2 adalah build native OpenCode dari opencode.ai di registri, yang dipasang
  FluxCode dan diperiksa terhadap SHA-256-nya. Salinan OpenCode di PATH tidak lagi dipakai,
  dan pilihan OpenCode yang sudah ada berpindah ke OpenCode 2 beserta pengaturannya.
- Nama tema warna di status bar membuka keempat tema, masing-masing digambar sebagai editor
  kecil dalam warnanya sendiri.
- Settings > About FluxCode menautkan ke repositori ini, issue tracker, dan changelog.

@en
## FluxCode, Zed and VS Code

All three are good editors made for different people. This is where each stands today.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Built with | Rust, GPUI (GPU-drawn) | Rust, GPUI (GPU-drawn) | TypeScript on Electron (Chromium + Node.js) |
| Systems | Windows, macOS, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| What you download | one program, about 16 MB | an installer | an installer |
| Layout and keys | VS Code's layout and VS Code's keys for the commands it has | its own layout; a VS Code key map is available | the original |
| Start, memory, CPU, size | 0.22 s, 1.4 MB idle, 1 process, 16 MB ([benchmark](#benchmark)) | 0.82 s, 66 MB, 2 processes, 261 MB | 0.29 s, 929 MB, 9 processes, 1 GB |
| Telemetry | none | on by default, can be turned off | on by default in Microsoft's build, can be turned off |
| Language smarts (go to definition, rename, diagnostics) | not yet: colors and word completion | built in, through language servers | through extensions |
| Debugger | not yet (Run only) | yes | yes |
| Extensions | 11 built in (build, run, deploy and pack tasks, as manifests), more from a file | yes | the largest catalog |
| AI agent | any ACP agent, three access levels, per-turn changes with undo | its own agent and ACP agents | GitHub Copilot and extensions |
| Files that are not text | hex, PDF, Office, ZIP, SQLite (read-only) | images | images; more through extensions |
| Real-time collaboration | built in, through the host's PC (no account or server) | built in, through Zed's servers | Live Share extension, through Microsoft's servers |
| Remote development | no | SSH | SSH, containers, WSL |
| Interface languages | English, Bahasa Indonesia | English | many, through language packs |
| Maturity | development builds | stable releases | stable, widely used |
| License | GPL-3.0-or-later | GPL-3.0 (the editor) | MIT for Code - OSS; Microsoft's build has its own license |

Pick FluxCode if you want a small, quiet editor on Windows or Linux with an agent you can
keep on a short leash, and you open logs, databases and documents next to your code.
Pick Zed for macOS, language servers out of the box and pair programming.
Pick VS Code when you need a specific extension, a debugger or remote development.
@id
## FluxCode, Zed, dan VS Code

Ketiganya editor bagus yang dibuat untuk orang berbeda. Berikut posisi masing-masing hari
ini.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Dibangun dengan | Rust, GPUI (digambar GPU) | Rust, GPUI (digambar GPU) | TypeScript di Electron (Chromium + Node.js) |
| Sistem | Windows, macOS, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| Yang diunduh | satu program, sekitar 16 MB | sebuah installer | sebuah installer |
| Tata letak dan tombol | tata letak VS Code dan tombol VS Code untuk perintah yang ada | tata letaknya sendiri; tersedia key map VS Code | yang aslinya |
| Start, memori, CPU, ukuran | 0,22 detik, 1,4 MB idle, 1 proses, 16 MB ([benchmark](#benchmark)) | 0,82 detik, 66 MB, 2 proses, 261 MB | 0,29 detik, 929 MB, 9 proses, 1 GB |
| Telemetri | tidak ada | aktif secara bawaan, bisa dimatikan | aktif secara bawaan di build Microsoft, bisa dimatikan |
| Kecerdasan bahasa (go to definition, rename, diagnostik) | belum ada: warna dan penyelesaian kata | bawaan, lewat language server | lewat extension |
| Debugger | belum ada (hanya Run) | ada | ada |
| Extension | 11 bawaan (task build, run, deploy, dan pack, berupa manifest), lebih banyak dari file | ada | katalog terbesar |
| Agent AI | agent ACP apa pun, tiga tingkat akses, perubahan per giliran dengan undo | agent sendiri dan agent ACP | GitHub Copilot dan extension |
| File yang bukan teks | hex, PDF, Office, ZIP, SQLite (baca-saja) | gambar | gambar; lebih banyak lewat extension |
| Kolaborasi real-time | bawaan, lewat PC host (tanpa akun atau server) | bawaan, lewat server Zed | extension Live Share, lewat server Microsoft |
| Pengembangan jarak jauh | tidak ada | SSH | SSH, container, WSL |
| Bahasa antarmuka | Inggris, Bahasa Indonesia | Inggris | banyak, lewat language pack |
| Kematangan | build pengembangan | rilis stabil | stabil, dipakai luas |
| Lisensi | GPL-3.0-or-later | GPL-3.0 (editornya) | MIT untuk Code - OSS; build Microsoft punya lisensi sendiri |

Pilih FluxCode kalau kamu ingin editor kecil dan tenang di Windows, macOS, atau Linux
dengan agent yang bisa dijaga ketat, dan kamu membuka log, database, serta dokumen di
samping kode.
Pilih Zed untuk language server yang langsung jalan dan pair programming.
Pilih VS Code kalau kamu butuh extension tertentu, debugger, atau pengembangan jarak jauh.

@en
## Privacy

FluxCode sends nothing about you. The only request it makes by itself is the update
check: a plain `GET` of the release list from github.com, with no build ID, machine ID or
cookie, at start and every 6 hours (Settings > Updates turns it off). Everything else goes
out only when you start it: the agent you run, `git`, GitHub when you sign in, and the new
build's `changelog.flux` when you click What's new on the update card. The
agent menu reads the public ACP agent list from cdn.agentclientprotocol.com, at most once
a day and only when you open the menu. Hosting a Live share session asks your router to
open its port (UPnP) and opens a tunnel at `live.muaraai.com`, unless you untick **Let
people join over the internet**; the relay then sees your public IP address and encrypted
bytes. If `live.muaraai.com` cannot be reached, FluxCode falls back to the public bore.pub
tunnel, which sees the same two things.
The first time an agent starts, FluxCode installs the agents' browser tool
(chrome-devtools-mcp) from the npm registry; Agent settings > Browser > Off skips it.
Extensions download nothing by themselves: Add Gradle wrapper asks services.gradle.org for
the current version and fetches the wrapper from Gradle's GitHub repository, and an
Install button on an extension's page runs `winget` in the Terminal, only when you click.
@id
## Privasi

FluxCode tidak mengirim apa pun tentangmu. Satu-satunya permintaan yang dibuat sendiri
adalah pemeriksaan update: `GET` biasa ke daftar rilis di github.com, tanpa build ID,
machine ID, atau cookie, saat start dan tiap 6 jam (Settings > Updates mematikannya).
Sisanya keluar hanya saat kamu memulainya: agent yang kamu jalankan, `git`, GitHub saat
kamu masuk, dan `changelog.flux` build baru saat kamu menekan What's new di kartu update.
Menu agent membaca daftar agent ACP publik dari cdn.agentclientprotocol.com, paling banyak
sekali sehari dan hanya saat menu itu dibuka. Menjalankan sesi Live share meminta routermu
membuka portnya (UPnP) dan membuka tunnel di `live.muaraai.com`, kecuali kamu menghapus
centang **Let people join over the internet**; relay lalu melihat alamat IP publikmu dan
byte terenkripsi. Kalau `live.muaraai.com` tidak bisa dihubungi, FluxCode beralih ke tunnel
publik bore.pub, yang melihat dua hal yang sama.
Saat agent pertama kali dijalankan, FluxCode memasang alat browser untuk agent
(chrome-devtools-mcp) dari registri npm; Agent settings > Browser > Off melewatinya.
Extension tidak mengunduh apa pun sendiri: Add Gradle wrapper menanyakan versi terbaru ke
services.gradle.org dan mengambil wrapper dari repositori GitHub Gradle, dan tombol Install
di halaman sebuah extension menjalankan `winget` di Terminal, hanya saat kamu menekannya.

@en
## Shortcuts

VS Code's keys on Windows, macOS and Linux. `Ctrl+Shift+P` (or `F1`) lists every command
with its keys, and Settings > Keyboard shortcuts shows the same list.

| Action | Keys |
| --- | --- |
| Command Palette | `Ctrl+Shift+P` / `F1` |
| Go to File / Go to Line | `Ctrl+P` / `Ctrl+G` |
| Find / Replace / Find in Files | `Ctrl+F` / `Ctrl+H` / `Ctrl+Shift+F` |
| Open File / Open Folder | `Ctrl+O` / `Ctrl+K Ctrl+O` |
| Save / Save All | `Ctrl+S` / `Ctrl+K S` |
| Close tab / Close all / Reopen closed | `Ctrl+W` / `Ctrl+K Ctrl+W` / `Ctrl+Shift+T` |
| Next / previous tab | `Ctrl+Tab` or `Ctrl+PageDown` / `Ctrl+Shift+Tab` or `Ctrl+PageUp` |
| Toggle line comment | `Ctrl+/` |
| Move line up / down | `Alt+Up` / `Alt+Down` |
| Copy line up / down | `Shift+Alt+Up` / `Shift+Alt+Down` |
| Add cursor above / below | `Ctrl+Alt+Up` / `Ctrl+Alt+Down` |
| Delete line | `Ctrl+Shift+K` |
| New line below / above | `Ctrl+Enter` / `Ctrl+Shift+Enter` |
| Indent / outdent | `Ctrl+]` / `Ctrl+[` |
| Word wrap | `Alt+Z` |
| Editor font bigger / smaller / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Side bar / Panel / Terminal | `Ctrl+B` / `Ctrl+J` / ``Ctrl+` `` |
| Explorer / Source Control / Run / Extensions | `Ctrl+Shift+E` / `Ctrl+Shift+G` / `Ctrl+Shift+D` / `Ctrl+Shift+X` |
| Run file / Stop | `F5` / `Shift+F5` |
| Agent panel | `Ctrl+L` |
| Settings / Next theme | `Ctrl+,` / `Ctrl+K Ctrl+T` |

`Ctrl+L` opens the agent (as in Cursor) where VS Code would select a line.
@id
## Pintasan

Tombol VS Code di Windows, macOS, dan Linux. `Ctrl+Shift+P` (atau `F1`) mendaftar semua
perintah beserta tombolnya, dan Settings > Keyboard shortcuts menampilkan daftar yang sama.

| Aksi | Tombol |
| --- | --- |
| Command Palette | `Ctrl+Shift+P` / `F1` |
| Go to File / Go to Line | `Ctrl+P` / `Ctrl+G` |
| Find / Replace / Find in Files | `Ctrl+F` / `Ctrl+H` / `Ctrl+Shift+F` |
| Open File / Open Folder | `Ctrl+O` / `Ctrl+K Ctrl+O` |
| Save / Save All | `Ctrl+S` / `Ctrl+K S` |
| Tutup tab / Tutup semua / Buka yang ditutup | `Ctrl+W` / `Ctrl+K Ctrl+W` / `Ctrl+Shift+T` |
| Tab berikutnya / sebelumnya | `Ctrl+Tab` atau `Ctrl+PageDown` / `Ctrl+Shift+Tab` atau `Ctrl+PageUp` |
| Komentar baris | `Ctrl+/` |
| Pindah baris ke atas / ke bawah | `Alt+Up` / `Alt+Down` |
| Salin baris ke atas / ke bawah | `Shift+Alt+Up` / `Shift+Alt+Down` |
| Tambah kursor di atas / di bawah | `Ctrl+Alt+Up` / `Ctrl+Alt+Down` |
| Hapus baris | `Ctrl+Shift+K` |
| Baris baru di bawah / di atas | `Ctrl+Enter` / `Ctrl+Shift+Enter` |
| Indent / outdent | `Ctrl+]` / `Ctrl+[` |
| Word wrap | `Alt+Z` |
| Font editor lebih besar / kecil / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Side bar / Panel / Terminal | `Ctrl+B` / `Ctrl+J` / ``Ctrl+` `` |
| Explorer / Source Control / Run / Extensions | `Ctrl+Shift+E` / `Ctrl+Shift+G` / `Ctrl+Shift+D` / `Ctrl+Shift+X` |
| Jalankan file / Stop | `F5` / `Shift+F5` |
| Panel agent | `Ctrl+L` |
| Settings / Tema berikutnya | `Ctrl+,` / `Ctrl+K Ctrl+T` |

`Ctrl+L` membuka agent (seperti di Cursor) di tempat VS Code memilih satu baris.

@en
## Build from source

Rust stable, then:

```sh
cargo build --release          # target/release/fluxcode(.exe)
cargo test --bin fluxcode
```

- Windows: HLSL shaders need `fxc.exe` from the Windows SDK, or set `GPUI_FXC_PATH` to
  `tools/fxc-shim` built with `cargo build --release --manifest-path tools/fxc-shim/Cargo.toml`.
  With the GNU toolchain, use a `CARGO_TARGET_DIR` without spaces (MinGW `windres`).
  `packaging/windows/build.ps1 -Install` builds, packs and installs like a release.
- Linux: `pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
  libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev libvulkan-dev` (Debian
  and Ubuntu names).
- macOS: Xcode (for the Metal shaders). `packaging/macos/bundle.sh BUILD BINARY OUT` packs
  a binary as `FluxCode.app`, a `.dmg` and the update archive, like a release.
- Builds name themselves from `FLUXCODE_BUILD` (`BuildDev-YYYY-MM-DD-HH-MM-SS`); without
  it, from the current UTC time.
@id
## Build dari sumber

Rust stable, lalu:

```sh
cargo build --release          # target/release/fluxcode(.exe)
cargo test --bin fluxcode
```

- Windows: shader HLSL butuh `fxc.exe` dari Windows SDK, atau setel `GPUI_FXC_PATH` ke
  `tools/fxc-shim` yang dibangun dengan
  `cargo build --release --manifest-path tools/fxc-shim/Cargo.toml`. Dengan toolchain GNU,
  pakai `CARGO_TARGET_DIR` tanpa spasi (`windres` MinGW). `packaging/windows/build.ps1
  -Install` membangun, mengemas, dan memasang seperti rilis.
- Linux: `pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
  libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev libvulkan-dev` (nama paket
  Debian dan Ubuntu).
- macOS: Xcode (untuk shader Metal). `packaging/macos/bundle.sh BUILD BINARY OUT` mengemas
  sebuah binary jadi `FluxCode.app`, `.dmg`, dan arsip update, seperti rilis.
- Build menamai dirinya dari `FLUXCODE_BUILD` (`BuildDev-YYYY-MM-DD-HH-MM-SS`); tanpa itu,
  dari waktu UTC saat itu.

@en
## Contributing

FluxCode is open source, and issues are open to everyone: report a bug or suggest an idea
through [Issues](https://github.com/MuaraAI/FluxCode/issues/new/choose). A bot checks each
new issue for earlier reports of the same problem and for an outdated build.

Pull requests are welcome from anyone. Fork the repository, branch from `main`, and open
a pull request; for a new feature or a new dependency, open an issue first so the idea
can be agreed on before the code is written.

Every pull request is checked automatically, and every push is checked again:

| Check | What it looks at | Fails when |
| --- | --- | --- |
| **Safety scan** | Every added line and file, read without running anything: malware patterns (download-and-run, reverse shells, miners, credential theft, process injection), leaked tokens and keys, invisible or right-to-left characters, encoded blobs, committed executables, dependencies from outside crates.io | Anything malicious or secret turns up, or someone outside the team changes CI, build or packaging files |
| **Performance** | The pull request and `main`, built the same way on the same runner and run 40 times each: executable size, start-up time, Markdown conversion time, peak memory, crate count | Size grows more than 10%, a run gets more than 25% slower, or memory grows more than 40% (smaller changes are flagged as warnings) |
| **Code quality** | Only the lines the pull request adds: clippy, rustfmt, `unwrap()`, `dbg!`, `thread::sleep`, very long lines and files | A clippy `correctness`, `suspicious` or `perf` lint, a compile error or a leftover `dbg!` |
| **Dependencies** | The RustSec advisory database, through cargo-deny | A dependency brings in an advisory `main` does not already have |
| **Build** | Windows, Linux and macOS builds, unit tests, install and start | Anything does not build, a test fails, or the app does not start |

The limits are read from `main`, so a pull request cannot loosen its own. The checks
help; they do not merge anything: a maintainer reads every pull request first. The full
list, how to run them locally and what each warning means are in
[CONTRIBUTING.md](CONTRIBUTING.md). What changed in each build:
[CHANGELOG.md](CHANGELOG.md).
@id
## Berkontribusi

FluxCode open source, dan issue terbuka untuk siapa saja: laporkan bug atau usulkan ide
lewat [Issues](https://github.com/MuaraAI/FluxCode/issues/new/choose). Bot memeriksa tiap
issue baru terhadap laporan lama dengan masalah yang sama dan terhadap build yang sudah
usang.

Pull request terbuka untuk siapa saja. Fork repositori ini, buat branch dari `main`, lalu
buka pull request; untuk fitur baru atau dependency baru, buka issue dulu supaya idenya
disepakati sebelum kodenya ditulis.

Tiap pull request diperiksa otomatis, dan tiap push diperiksa ulang:

| Pemeriksaan | Yang dilihat | Gagal kalau |
| --- | --- | --- |
| **Safety scan** | Tiap baris dan file yang ditambahkan, cuma dibaca tanpa dijalankan: pola malware (unduh-lalu-jalankan, reverse shell, miner, pencurian kredensial, injeksi proses), token dan kunci yang bocor, karakter tak terlihat atau kanan-ke-kiri, blob ter-encode, file executable, dependency dari luar crates.io | Ada yang berbahaya atau rahasia, atau orang di luar tim mengubah file CI, build, atau packaging |
| **Performa** | Pull request dan `main`, dibangun dengan cara yang sama di runner yang sama dan dijalankan 40 kali masing-masing: ukuran executable, waktu start, waktu konversi Markdown, memori puncak, jumlah crate | Ukuran naik lebih dari 10%, sebuah run lebih lambat lebih dari 25%, atau memori naik lebih dari 40% (perubahan lebih kecil ditandai sebagai peringatan) |
| **Kualitas kode** | Hanya baris yang ditambahkan pull request: clippy, rustfmt, `unwrap()`, `dbg!`, `thread::sleep`, baris dan file yang terlalu panjang | Ada lint clippy `correctness`, `suspicious`, atau `perf`, error kompilasi, atau `dbg!` yang tertinggal |
| **Dependency** | Database advisory RustSec, lewat cargo-deny | Sebuah dependency membawa advisory yang belum ada di `main` |
| **Build** | Build Windows, Linux, dan macOS, unit test, pasang dan jalankan | Ada yang tidak bisa di-build, test gagal, atau aplikasinya tidak mau jalan |

Batas-batasnya dibaca dari `main`, jadi pull request tidak bisa melonggarkan batasnya
sendiri. Pemeriksaan ini membantu, tapi tidak me-merge apa pun: maintainer membaca tiap
pull request dulu. Daftar lengkapnya, cara menjalankannya di lokal, dan arti tiap
peringatan ada di [CONTRIBUTING.md](CONTRIBUTING.md). Apa yang berubah di tiap build:
[CHANGELOG.md](CHANGELOG.md).

@en
## The icon
@id
## Ikonnya

@image assets/icon.png width=96 align=right alt="The FluxCode icon: a square meander, one ivory hook and one ochre hook locked together, on a near-black tile."

@en
The mark is a meander: one unbroken line that turns only at right angles and never
crosses itself. The pattern is named after the river Maeander in western Anatolia (the
Büyük Menderes today). The Greek geographer Strabo wrote that it winds so much that
every winding course came to be called after it. One of the oldest known meanders is
carved into mammoth-ivory bracelets found at Mezin, in Ukraine, more than 15,000 years
ago; Greek potters of the Geometric period later painted it around whole vases.

FluxCode takes three things from it. The name: a flux is a flow, and the meander is a
river drawn as a rule. The structure: code also runs as one path that turns into a
block and comes back out, the way a call returns to where it started. The colors: ivory
for the Mezin carvings and ochre for fired clay, on ink. The line is split at its center
into two hooks of the same shape, turned 180 degrees and holding each other, so the mark
looks the same upside down. The app's FluxDark and FluxCream themes use the same three
colors.
@id
Lambangnya adalah meander: satu garis tanpa putus yang hanya berbelok tegak lurus dan
tidak pernah memotong dirinya sendiri. Motif ini dinamai dari sungai Maeander di Anatolia
barat (kini Büyük Menderes). Geografer Yunani Strabo menulis bahwa sungai itu berkelok
sedemikian rupa sampai setiap alur yang berkelok ikut dinamai menurutnya. Salah satu
meander tertua yang diketahui terukir pada gelang gading mamut yang ditemukan di Mezin,
Ukraina, lebih dari 15.000 tahun lalu; pembuat tembikar Yunani periode Geometrik kemudian
melukisnya mengelilingi seluruh vas.

FluxCode mengambil tiga hal darinya. Namanya: flux adalah aliran, dan meander adalah
sungai yang digambar sebagai kaidah. Strukturnya: kode juga berjalan sebagai satu jalur
yang berbelok masuk ke sebuah blok lalu keluar lagi, seperti pemanggilan yang kembali ke
tempat asalnya. Warnanya: gading untuk ukiran Mezin dan oker untuk tanah liat bakar, di
atas tinta. Garisnya dibelah di tengah menjadi dua kait berbentuk sama, diputar 180
derajat dan saling mengunci, sehingga lambang itu tetap sama walau dibalik. Tema FluxDark
dan FluxCream di aplikasi memakai tiga warna yang sama.

@en
## License

[GPL-3.0-or-later](LICENSE). Third-party components: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
@id
## Lisensi

[GPL-3.0-or-later](LICENSE). Komponen pihak ketiga: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

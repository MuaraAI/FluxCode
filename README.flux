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
**English** · [Bahasa Indonesia](README.id.md) · [Roadmap (2026-2027)](ROADMAP.md) · [Discussions](https://github.com/MuaraAI/FluxCode/discussions)
@id
[English](README.md) · **Bahasa Indonesia** · [Roadmap (2026-2027)](ROADMAP.md) · [Diskusi](https://github.com/MuaraAI/FluxCode/discussions)

@en
FluxCode is a native, GPU-rendered code editor for Windows, macOS, and Linux written in
Rust. It ships as a single 16 MB binary with an integrated AI agent workspace, a built-in
Rust WizTree disk analyzer wired into AI context, a multi-language error-aware terminal,
built-in Git, editable Minecraft Bedrock/Java NBT support, and instant views for binary,
archive, database, and Office files. It opens a window in ~50 ms, idles at 1.4 MB of RAM
in one process ([benchmark](#benchmark)), and sends zero telemetry.

Built at [MuaraAI](https://github.com/MuaraAI) by engineers with deep Rust systems
experience and over six years of building and running production SaaS products. Open source
under GPL-3.0-or-later and actively developed in the open, with continuous builds from
`main` and a public [2026-2027 Roadmap](ROADMAP.md) (security hardening, opt-in LSP, and
native Android in 2027).
@id
FluxCode adalah editor kode native yang dirender GPU untuk Windows, macOS, dan Linux,
ditulis dalam Rust. Dikemas sebagai satu binary 16 MB dengan workspace agent AI terintegrasi,
penganalisis disk WizTree Rust yang terhubung langsung ke konteks AI, terminal multibahasa
dengan klik error langsung ke baris kode, Git bawaan, editor NBT Minecraft Bedrock/Java,
serta tampilan instan untuk file binary, arsip, database, dan Office. Jendelanya terbuka
dalam ~50 ms, hanya memakai 1,4 MB RAM saat idle dalam satu proses ([benchmark](#benchmark)),
dan tanpa telemetri sama sekali.

Dibangun di [MuaraAI](https://github.com/MuaraAI) oleh tim engineer berpengalaman di
sistem Rust yang telah membangun dan menjalankan produk SaaS produksi sejak 6 tahun lalu
hingga sekarang. Open source di bawah lisensi GPL-3.0-or-later dan terus dikembangkan
secara aktif lewat rilis berkelanjutan dari `main` serta [Roadmap 2026-2027](ROADMAP.md)
(pengamanan inti, LSP ringan, hingga rilis native Android di 2027).

@all
[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![Last commit](https://img.shields.io/github/last-commit/MuaraAI/FluxCode?label=last%20commit)](https://github.com/MuaraAI/FluxCode/commits/main)
[![Build](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

@en
@image assets/screenshots/hero.jpg width=100% link=assets/screenshots/agent-workspace.png alt="A 16 MB code editor with AI agents built in: the title over a dusk sky, and below it FluxCode with the agent filling the window, rising out of a bank of clouds. Saved chats on the left, the conversation in the middle, the files it changed on the right."

Full-screen agent workspace: saved threads on the left, active conversation in the center,
and live file diffs on the right, floating over a GPU-drawn background. When you're deep
in code, hit `Ctrl+L` to dock the agent as a compact side panel.
@id
@image assets/screenshots/hero.id.jpg width=100% link=assets/screenshots/agent-workspace.png alt="Editor kode 16 MB dengan agen AI di dalamnya: judul di atas langit senja, dan di bawahnya FluxCode dengan agen memenuhi jendela, muncul dari balik awan. Daftar chat di kiri, percakapan di tengah, file yang diubahnya di kanan."

Workspace layar penuh milik agent: daftar chat di kiri, percakapan aktif di tengah, dan
diff file secara langsung di kanan, melayang di atas latar yang digambar GPU. Saat fokus
menulis kode, tekan `Ctrl+L` untuk menaruh agent sebagai panel ringkas di samping.

@en
## Benchmark

On a standard Windows 11 laptop, FluxCode puts its first frame on screen in **~50 ms** and
drops to **1.4 MB** in Task Manager at idle, all inside a single process. On the same
machine, Zed takes 818 ms and 66.2 MB; VS Code takes ~3 s and 929.2 MB across nine
processes.

@image assets/benchmarks/benchmark-light.svg dark=assets/benchmarks/benchmark-dark.svg width=100% alt="Six bar charts, lower is better in each. Window on screen after launch: FluxCode about 50 ms (timed by hand), Zed 818 ms, VS Code 3,000 ms. Memory in Task Manager when idle: FluxCode 1.4 MB, Zed 66.2 MB, VS Code 929.2 MB. Memory reserved: FluxCode 72 MB, Zed 154 MB, VS Code 1,315 MB. CPU while idle: FluxCode 0.05%, Zed 1.46%, VS Code 5.89% of one core. Processes: FluxCode 1, Zed 2, VS Code 9. Size on disk: FluxCode 16 MB, Zed 261 MB, VS Code 1,044 MB."

The difference shows up where your editor spends most of its life: sitting open in the
background while you think, read docs, or run builds. FluxCode idles at 1.4 MB of RAM
(vs. 66.2 MB for Zed and 929.2 MB for VS Code) and takes just 16 MB on disk. Lower is
better across every chart above.
@id
## Benchmark

Di laptop Windows 11 standar, FluxCode menampilkan jendela pertamanya dalam **~50 ms** dan
turun ke **1,4 MB** di Task Manager saat idle, semuanya dalam satu proses. Di mesin yang
sama, Zed butuh 818 ms dan 66,2 MB; VS Code butuh ~3 detik dan 929,2 MB di sembilan proses.

@image assets/benchmarks/benchmark-light.svg dark=assets/benchmarks/benchmark-dark.svg width=100% alt="Six bar charts, lower is better in each. Window on screen after launch: FluxCode about 50 ms (timed by hand), Zed 818 ms, VS Code 3,000 ms. Memory in Task Manager when idle: FluxCode 1.4 MB, Zed 66.2 MB, VS Code 929.2 MB. Memory reserved: FluxCode 72 MB, Zed 154 MB, VS Code 1,315 MB. CPU while idle: FluxCode 0.05%, Zed 1.46%, VS Code 5.89% of one core. Processes: FluxCode 1, Zed 2, VS Code 9. Size on disk: FluxCode 16 MB, Zed 261 MB, VS Code 1,044 MB."

Perbedaannya paling terasa saat editor dibiarkan terbuka seharian sambil kamu berpikir,
membaca dokumentasi, atau menjalankan build: FluxCode hanya memakai 1,4 MB RAM saat idle
(dibanding 66,2 MB di Zed dan 929,2 MB di VS Code) dan hanya memakan 16 MB di disk. Makin
kecil angkanya, makin baik.

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
| Window on screen after launch (ms) | about 50, timed by hand (bench.ps1: 221) | 818 (387 to 1,322) | 3,000 |
| Memory in Task Manager, idle (MB) | 1.4 (1.0 to 4.8) | 66.2 (65.2 to 71.9) | 929.2 (867.7 to 985.4) |
| Memory reserved (MB) | 72 (72 to 72) | 154 (153 to 156) | 1,315 (1,249 to 1,399) |
| CPU while idle (% of one core) | 0.05 (0.00 to 0.68) | 1.46 (1.15 to 2.60) | 5.89 (5.57 to 8.12) |
| Processes running | 1 | 2 | 9 |
| Size on disk (MB) | 16 | 261 | 1,044 |

- **Window**: from launch to the first visible window, runs 2 to 5 (run 1 reads the program
  from disk cold). VS Code's figure is the time until the workbench is usable, about
  3 s: its window shows before the workbench inside it has loaded.
- **FluxCode's window time** is about 50 ms, timed by hand by the maintainer. `bench.ps1`
  logs 221 ms for it, but its PowerShell loop calls `Get-Process` on every poll, so that
  figure includes the script's own time. Zed's and VS Code's figures come from the same
  script.
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
| Jendela tampil setelah diluncurkan (ms) | sekitar 50, diukur manual (bench.ps1: 221) | 818 (387 sampai 1.322) | 3.000 |
| Memori di Task Manager, idle (MB) | 1,4 (1,0 sampai 4,8) | 66,2 (65,2 sampai 71,9) | 929,2 (867,7 sampai 985,4) |
| Memori yang direservasi (MB) | 72 (72 sampai 72) | 154 (153 sampai 156) | 1.315 (1.249 sampai 1.399) |
| CPU saat idle (% satu core) | 0,05 (0,00 sampai 0,68) | 1,46 (1,15 sampai 2,60) | 5,89 (5,57 sampai 8,12) |
| Proses berjalan | 1 | 2 | 9 |
| Ukuran di disk (MB) | 16 | 261 | 1.044 |

- **Jendela**: dari diluncurkan sampai jendela pertama terlihat, pengukuran ke-2 sampai
  ke-5 (pengukuran pertama membaca program dari disk dalam keadaan dingin). Angka VS Code
  adalah waktu sampai workbench siap dipakai, sekitar 3 detik: jendelanya tampil sebelum
  workbench di dalamnya selesai dimuat.
- **Waktu jendela FluxCode** sekitar 50 ms, diukur manual oleh pengelola proyek.
  `bench.ps1` mencatat 221 ms untuknya, tetapi loop PowerShell-nya memanggil `Get-Process`
  di setiap polling, jadi angka itu ikut memuat waktu skrip sendiri. Angka Zed dan VS Code
  berasal dari skrip yang sama.
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

Grab the latest build from [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| System | File | Install |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Run the installer. No admin rights required: installs per-user with Start Menu and Desktop shortcuts. |
| macOS 11+, Apple Silicon and Intel (preview) | `FluxCode-BuildDev-...-macos-universal.dmg` | Open the DMG and drag FluxCode into Applications. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Extract and run `./install.sh` (no root required). |
| Windows, portable | `FluxCode-BuildDev-...-windows-x64.zip` | Extract anywhere and run `FluxCode.exe` directly (`Install.cmd` is included if you want to install later). |

The macOS build is currently a preview and not yet notarized by Apple, so macOS will ask
once on first launch (on macOS 15+, open it and click **System Settings > Privacy &
Security > Open Anyway**; on older versions, right-click FluxCode > **Open**). If you hit
any macOS-specific quirks, [open an issue](https://github.com/MuaraAI/FluxCode/issues/new).

We ship continuous `BuildDev-YYYY-MM-DD-HH-MM-SS` builds straight from `main`. Installed
copies notify you when a new build lands and update in one click (you can turn update
checks off anytime under **Settings > Updates**).
@id
## Unduh

Ambil build terbaru dari [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| Sistem | File | Pemasangan |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Jalankan installer. Tanpa hak admin: terpasang di akunmu lengkap dengan pintasan Start Menu dan Desktop. |
| macOS 11+, Apple Silicon dan Intel (preview) | `FluxCode-BuildDev-...-macos-universal.dmg` | Buka DMG lalu tarik FluxCode ke Applications. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Ekstrak lalu jalankan `./install.sh` (tanpa root). |
| Windows, portable | `FluxCode-BuildDev-...-windows-x64.zip` | Ekstrak di mana saja dan jalankan `FluxCode.exe` langsung (`Install.cmd` juga tersedia bila ingin dipasang). |

Build macOS saat ini masih berstatus preview dan belum dinotarisasi Apple, jadi macOS akan
meminta konfirmasi sekali saat pertama dibuka (di macOS 15+, buka aplikasinya lalu pilih
**System Settings > Privacy & Security > Open Anyway**; di versi lama, klik kanan
FluxCode > **Open**). Kalau menemukan kendala di Mac, silakan
[buka issue](https://github.com/MuaraAI/FluxCode/issues/new).

Kami merilis build `BuildDev-YYYY-MM-DD-HH-MM-SS` secara berkelanjutan setiap ada perubahan
yang masuk ke `main`. Aplikasi yang sudah terpasang akan menampilkan kartu notifikasi saat
build baru tersedia dan bisa diperbarui dalam satu klik (bisa dimatikan kapan saja lewat
**Settings > Updates**).

@en
## A closer look
@id
## Lebih dekat

@all
@gallery
@image assets/screenshots/token-saver.png width=49% link=assets/screenshots/token-saver.png alt="The token saver panel over the chat: Caveman on at the Full level, Lean code off, Quiet tools on, about 36 percent fewer output tokens." caption="Token saver" caption-id="Hemat token"
@image assets/screenshots/usage-limit.png width=49% link=assets/screenshots/usage-limit.png alt="A banner above the message box: Usage limit reached, resets 15.45 in 2 hours, with a Resume at reset checkbox and a Continue now button." caption="When a usage limit is reached" caption-id="Saat limit pemakaian tercapai"
@end

@en
**Token saver** (the leaf icon under the prompt box) trims verbose agent prose, favors
focused diffs over full-file dumps, and keeps tool calls quiet. And when you hit a provider's
rate or quota cap, FluxCode parses the reset window so you can either **Continue now** or
tick **Resume at reset** and let it pick right back up automatically.
@id
**Hemat token** (ikon daun di bawah kotak pesan) memangkas jawaban agent yang bertele-tele,
mengutamakan diff dibanding menulis ulang seluruh file, dan menjaga pemakaian tool tetap
efisien. Saat kuota atau rate limit tercapai, FluxCode membaca jadwal reset-nya sehingga
kamu bisa klik **Continue now** atau centang **Resume at reset** agar pekerjaan dilanjutkan
otomatis begitu kuota kembali.

@all
@gallery
@image assets/screenshots/side-bars-search.png width=49% link=assets/screenshots/side-bars-search.png alt="Search moved to the right side bar, with the Explorer on the left: typing right_view lists 23 results in 4 files as you type, each match tinted, long files folded to six lines." caption="Search in either side bar" caption-id="Search di side bar mana pun"
@image assets/screenshots/extensions-marketplace.png width=49% link=assets/screenshots/extensions-marketplace.png alt="The Extensions view searching the VS Code Marketplace for prettier: 256 results with icons, installs, ratings and Install buttons, and Prettier's page with its details and README." caption="The whole VS Code Marketplace" caption-id="Seluruh VS Code Marketplace"
@end

@en
Right-click any activity icon to dock that view on the left or the right. Project-wide
search streams results as you type, and the Extensions view searches and installs directly
from the VS Code Marketplace.
@id
Klik kanan ikon mana pun di activity bar untuk menaruh panelnya di kiri atau di kanan.
Pencarian proyek menampilkan hasil secara instan saat kamu mengetik, dan panel Extensions
terhubung langsung ke seluruh katalog VS Code Marketplace.

@all
@image assets/screenshots/backgrounds.png width=100% link=assets/screenshots/backgrounds.png alt="The twelve animated backgrounds in the FluxDark theme, drawn live: Clouds at dusk, a moonlit Ocean, Mountains in fog, an Aurora over snowy ridges, a pine Forest with fireflies, a Lake between hills, a Beach at sunset, Dunes under a low sun, a City skyline with lit windows, Rain, Snow, and a Galaxy with a ringed planet."

@gallery
@image assets/screenshots/settings-backgrounds.png width=49% link=assets/screenshots/settings-backgrounds.png alt="Settings, Appearance: Animated background as a grid of twelve live previews (Clouds, Ocean, Mountains, Aurora, Forest, Lake, Beach, Dunes, City, Rain, Snow, Galaxy), each drawn in the FluxSky theme's colors, with Clouds chosen; Background animation and Background visibility below." caption="Twelve backgrounds, previewed live" caption-id="Dua belas latar, dengan pratinjau langsung"
@image assets/screenshots/backgrounds-themes.png width=49% link=assets/screenshots/backgrounds-themes.png alt="The Mountains background in four themes: FluxDark, FluxRed and FluxMuara Dark at night, FluxCream as a pale day scene." caption="One scene in four themes" caption-id="Satu scene dalam empat tema"
@end

@en
Twelve procedural scenes (clouds, ocean, mountains, aurora, forest, lake, beach, dunes,
city, rain, snow, galaxy) rendered straight from your active theme palette, with zero
external assets to download. By default they stay static at 0% CPU; if you flip on
**Background animation**, FluxCode benchmarks its own frame cost on your hardware first and
tells you honestly how light or heavy it runs.
@id
Dua belas latar prosedural (awan, laut, pegunungan, aurora, hutan, danau, pantai, bukit
pasir, kota, hujan, salju, galaksi) digambar langsung menggunakan palet warna tema aktifmu
tanpa perlu mengunduh aset tambahan. Secara default latar ini diam (0% CPU); saat
**Background animation** dinyalakan, FluxCode mengukur beban CPU di mesinmu terlebih dulu
dan memberi tahu apakah animasinya ringan atau berat untuk perangkatmu.

@en
## What makes it different

- **Muscle-memory compatible with VS Code.** Same layout (activity bar, sidebars, tabs,
  bottom panel, status bar) and the same keybindings you already use (`Ctrl+Shift+P`,
  `Ctrl+P`, `Ctrl+G`, `Ctrl+/`, `Alt+Up/Down`). It isn't a bloated Electron clone: there's
  no Node runtime in the editor process, and VS Code color themes & grammars load natively
  in Rust without running extension JavaScript.
- **Systems-level efficiency.** Built in Rust on top of GPUI (the GPU UI framework behind
  Zed) with custom Windows D3D11 swap-chain & path-texture optimizations and incremental
  per-line syntax highlighting. In our [benchmark](#benchmark), it opens a window in ~50 ms,
  reserves 72 MB, and trims its idle working set down to 1.4 MB.
- **First-class AI agent workflow (ACP).** Speaks the Agent Client Protocol out of the box
  and supports every agent in the ACP registry (OpenCode, Claude Code, Codex, Gemini CLI,
  Copilot, Cursor, Qwen, Command Code, and more). Install missing agents in one click, pick
  models and effort levels inline, review file diffs per turn, or roll changes back with
  one click.
- **GitHub Copilot ghost-text completions.** Talks to the official
  `@github/copilot-language-server` over stdio in a separate process so the editor stays
  tiny and responsive. Install once with `npm install -g @github/copilot-language-server`
  and sign in under **Settings > AI agents and GitHub**.
- **Crafted by seasoned Rust & SaaS engineers.** Designed by developers who have written
  Rust in production and shipped commercial SaaS platforms for the past 6+ years: approachable
  enough for a beginner with one-click setup and read-only safety modes, yet tunable down to
  every editor, font, keybinding, and per-agent instruction file for veterans.
@id
## Yang membedakannya

- **Familiar seperti VS Code.** Tata letak (activity bar, side bar, tab, panel bawah, status
  bar) dan shortcut utamanya (`Ctrl+Shift+P`, `Ctrl+P`, `Ctrl+G`, `Ctrl+/`, `Alt+Up/Down`)
  sama persis dengan yang biasa kamu pakai di VS Code. Bedanya, tidak ada runtime Electron
  atau Node di dalam editor: tema warna dan grammar VS Code dimuat secara native di Rust
  tanpa mengeksekusi JavaScript berat.
- **Efisiensi tingkat sistem.** Ditulis dalam Rust di atas GPUI dengan optimasi D3D11
  swap-chain, tekstur on-demand, dan syntax highlighting inkremental per baris. Dalam
  [benchmark](#benchmark), jendela terbuka dalam ~50 ms, mereservasi 72 MB, dan memangkas
  pemakaian RAM idle hingga 1,4 MB.
- **Alur kerja AI agent kelas satu (ACP).** Mendukung Agent Client Protocol secara penuh
  untuk seluruh agent di registri ACP (OpenCode, Claude Code, Codex, Gemini CLI, Copilot,
  Cursor, Qwen, Command Code, dan lainnya). Pasang agent dalam satu klik, atur model dan
  level effort langsung di bawah kotak chat, tinjau diff per giliran, atau batalkan perubahan
  file kapan saja.
- **Inline completion GitHub Copilot.** Terhubung ke `@github/copilot-language-server`
  resmi di proses terpisah agar editor utama tetap ringan. Cukup pasang dengan
  `npm install -g @github/copilot-language-server` lalu masuk lewat
  **Settings > AI agents and GitHub**.
- **Dirancang oleh engineer Rust & SaaS berpengalaman.** Dibuat oleh developer yang sudah
  menangani pemrograman sistem Rust dan membangun produk SaaS sejak 6+ tahun lalu hingga
  sekarang: ramah bagi pemula lewat setup satu klik dan mode aman baca-saja, sekaligus
  lengkap bagi engineer senior dengan kustomisasi penuh.

@en
## Extensions and FluxCompile

The Extensions view (`Ctrl+Shift+X`) is the VS Code Marketplace: all of it, loaded a page
at a time as you scroll, searched as you type (an earlier search comes back instantly),
sorted by relevance, installs, rating, last update or name, and filtered by category.
Install puts the extension's VSIX into `~/.fluxcode/vscode-extensions`; Update and
Uninstall are on its page, and the folder button installs a local `.vsix` file. VS Code's
JavaScript extension host isn't part of FluxCode, so an extension's own code doesn't run
here; its page says what it declares (themes, languages, snippets).

FluxCode's own project support is always on, with nothing to install: Paper plugins,
Bedrock add-ons, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ (CMake and Make), .NET and
the HTML preview. Each is one small JSON manifest: the files that mark its projects, the
tasks it adds, the tools it needs (with a winget id to install them) and notes for the AI
agent. The Run view lists the folder's projects with their tasks, and the agent is told
what each project is and how it builds.

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

Tampilan Extensions (`Ctrl+Shift+X`) adalah VS Code Marketplace: seluruhnya, dimuat per
halaman sambil kamu menggulir, dicari sambil kamu mengetik (pencarian sebelumnya kembali
seketika), diurutkan menurut relevansi, jumlah pasang, rating, update terakhir, atau nama,
dan disaring per kategori. Install menaruh VSIX extension itu di
`~/.fluxcode/vscode-extensions`; Update dan Uninstall ada di halamannya, dan tombol folder
memasang file `.vsix` lokal. Extension host JavaScript milik VS Code bukan bagian dari
FluxCode, jadi kode extension itu sendiri tidak berjalan di sini; halamannya menyebutkan apa
yang dideklarasikannya (tema, bahasa, snippet).

Dukungan proyek milik FluxCode sendiri selalu aktif, tanpa perlu dipasang: plugin Paper,
add-on Bedrock, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ (CMake dan Make), .NET, dan
pratinjau HTML. Masing-masing satu manifest JSON kecil: file yang menandai proyeknya, task
yang ditambahkannya, alat yang dibutuhkannya (dengan id winget untuk memasangnya), dan
catatan untuk agent AI. Tampilan Run mendaftar proyek di folder itu beserta task-nya, dan
agent diberi tahu proyek itu apa serta bagaimana ia dibangun.

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
  such as OpenCode 2, Claude Agent, Codex, Gemini CLI or GitHub Copilot. An
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
  installs and checks against its SHA-256. An OpenCode 2 you already installed (npm's
  `@opencode/cli`, or the opencode.ai installer) is used as it is instead; OpenCode 1 is
  not. An existing OpenCode choice moves to OpenCode 2 with its settings.
- Agents already on your PC are found the way Windows finds programs: on the PATH as
  Windows has it now and in the folders installers use (npm, pnpm, bun, Scoop, WinGet,
  `~/.local/bin`), so nothing is downloaded twice.
- The color theme's name in the status bar opens the seven themes, each shown as a small
  editor in its colors.
- Markdown files draw themselves: a README opens as text, and an eye icon above it
  turns it into the drawn page GitHub shows (headings, tables, lists, local images,
  GitHub's `> [!NOTE]` callouts), with Source to go back. Flux documents keep their
  own Preview and Source.
- Settings > About FluxCode links to this repository, the issue tracker and the changelog.
- **Side bars on either side.** Explorer, Search, Source Control, Run, Extensions and the
  agent can each live in the left or the right side bar, as in VS Code's secondary side
  bar: right-click an activity bar icon to move it. The right side bar's header switches
  between the views that live there.
- **Find in files while you type.** Results update after a short pause in typing, each
  match is tinted, and a file with many matches folds to its first six lines with an
  "N more matches" row. In a git repository only the files git tracks or would not ignore
  are read, on several threads.
- **The agent's full-screen workspace.** The full-screen button in the agent's header turns
  it into three columns: chats, the conversation, and the files it changed with their
  diffs and undo. The columns step aside on narrow windows.
- **Token saver.** Caveman (Lite, Full or Ultra) asks for shorter answers, Lean code for
  diffs instead of whole files, Quiet tools for reading and printing only what the task
  needs. The guidance goes out with a message when the packs change and every few turns,
  so it costs almost nothing itself.
- **Usage limit reached.** A banner above the message box shows when the limit resets.
  Continue now sends the cut-off message again; Resume at reset does it by itself once the
  limit window has reset.
- **Disk Usage (WizTree) for AI agents.** Built-in Rust disk tree and treemap analyzer
  that feeds a compact folder hierarchy (largest, smallest, waste hotspots, extensions,
  and largest files) directly to the agent via `@wiztree`, **Ask Agent**, or
  `fluxcode --wiztree`, so agents inspect workspace storage without manual directory scans.
- **Minecraft Bedrock data.** Open and edit binary `level.dat`, `level.dat_old`,
  `.mcstructure`, and `.nbt` files as clean SNBT with lossless binary save, plus read-only
  inspection of LevelDB `db/*.log` and `db/*.ldb` files.
- **Multi-language terminal.** Parses errors and stack traces across Python, JavaScript,
  TypeScript, Rust, Go, Java, Kotlin, C/C++, C#, Zig, PHP, Ruby, Lua, Dart, Swift, Shell,
  and PowerShell with one-click file:line:col jump links and inline Tab cycling.
- **Liquid glass and animated backgrounds.** Panels can turn translucent over the system's
  blur, with floating glass cards. Twelve animated backgrounds play behind them, drawn
  live in the theme's colors (no video and no download). They stand still by default;
  Background animation makes them move 24 frames a second while the window is in front
  (about 10 to 15% of one CPU core) and a few behind other windows, and turning it on
  first measures what your PC can take.
@id
## Kemampuan lainnya

- Pewarnaan sintaks untuk sekitar 200 bahasa, dipilih dari nama file, ekstensi, atau baris
  `#!`.
- Workspace multi-root: tarik folder ke jendela untuk menambahkannya atau berpindah ke
  situ.
- Panel agent (`Ctrl+L`) menjalankan agent apa pun di [registri ACP](https://agentclientprotocol.com),
  seperti OpenCode 2, Claude Agent, Codex, Gemini CLI, atau GitHub Copilot. Agent yang belum ada di PC-mu punya tombol unduh di menu agent (lengkap dengan
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
  FluxCode dan diperiksa terhadap SHA-256-nya. OpenCode 2 yang sudah kamu pasang (lewat npm
  `@opencode/cli`, atau installer opencode.ai) langsung dipakai; OpenCode 1 tidak. Pilihan
  OpenCode yang sudah ada berpindah ke OpenCode 2 beserta pengaturannya.
- Agent yang sudah ada di PC kamu ditemukan seperti Windows menemukan program: dari PATH
  Windows saat itu dan dari folder yang dipakai installer (npm, pnpm, bun, Scoop, WinGet,
  `~/.local/bin`), jadi tidak ada yang diunduh dua kali.
- Nama tema warna di status bar membuka tujuh tema, masing-masing digambar sebagai editor
  kecil dalam warnanya sendiri.
- File Markdown menggambar dirinya sendiri: README terbuka sebagai teks, dan ikon mata di
  atasnya mengubahnya menjadi halaman tergambar seperti di GitHub (judul, tabel, daftar,
  gambar lokal, callout `> [!NOTE]` milik GitHub), dengan Source untuk kembali. Dokumen
  Flux tetap punya Preview dan Source sendiri.
- Settings > About FluxCode menautkan ke repositori ini, issue tracker, dan changelog.
- **Side bar di kedua sisi.** Explorer, Search, Source Control, Run, Extensions, dan agent
  masing-masing bisa ditaruh di side bar kiri atau kanan, seperti secondary side bar VS
  Code: klik kanan ikon di activity bar untuk memindahkannya. Header side bar kanan
  berpindah di antara view yang ada di sana.
- **Find in files sambil mengetik.** Hasil diperbarui setelah jeda singkat saat mengetik,
  tiap kecocokan diberi warna, dan file dengan banyak kecocokan dilipat ke enam baris
  pertamanya dengan baris "N hasil lagi". Di repositori git, hanya file yang dilacak git
  atau tidak diabaikan yang dibaca, dengan beberapa thread.
- **Workspace layar penuh untuk agent.** Tombol layar penuh di header agent mengubahnya
  menjadi tiga kolom: daftar chat, percakapan, dan file yang diubahnya beserta diff dan
  undo. Kolom-kolomnya menyingkir di jendela yang sempit.
- **Hemat token.** Caveman (Lite, Full, atau Ultra) meminta jawaban lebih pendek, Lean code
  meminta diff alih-alih seluruh file, Quiet tools meminta membaca dan mencetak seperlunya.
  Panduannya ikut terkirim bersama pesan saat paketnya berubah dan tiap beberapa giliran,
  jadi panduan itu sendiri hampir tidak memakan token.
- **Limit pemakaian tercapai.** Banner di atas kotak pesan menunjukkan kapan limit reset.
  Continue now mengirim ulang pesan yang terpotong; Resume at reset melakukannya sendiri
  setelah jendela limitnya reset.
- **Penggunaan Disk (WizTree) untuk agent AI.** Penganalisis pohon disk dan treemap
  bawaan berbasis Rust yang menyuplai ringkasan struktur folder (terbesar, terkecil, titik
  boros, ekstensi, dan file terbesar) langsung ke agent lewat `@wiztree`, **Tanya Agent**,
  atau `fluxcode --wiztree`, sehingga agent paham kondisi penyimpanan tanpa memindai manual.
- **Data Minecraft Bedrock.** Buka dan edit file biner `level.dat`, `level.dat_old`,
  `.mcstructure`, dan `.nbt` sebagai SNBT bersih dengan penyimpanan biner lossless, plus
  inspeksi baca-saja untuk LevelDB `db/*.log` dan `db/*.ldb`.
- **Terminal multibahasa.** Membaca error dan stack trace lintas Python, JavaScript,
  TypeScript, Rust, Go, Java, Kotlin, C/C++, C#, Zig, PHP, Ruby, Lua, Dart, Swift, Shell,
  dan PowerShell dengan klik langsung ke file:baris:kolom dan cycling Tab inline.
- **Liquid glass dan latar beranimasi.** Panel bisa menjadi tembus pandang di atas blur
  sistem, dengan kartu kaca yang melayang. Dua belas latar beranimasi bergerak di
  belakangnya, digambar langsung dengan warna tema (tanpa video dan tanpa unduhan). Secara
  default scene-nya diam; Background animation membuatnya bergerak 24 frame per detik saat
  jendela di depan (sekitar 10 sampai 15% dari satu core CPU) dan beberapa frame saat di
  belakang jendela lain, dan saat dinyalakan FluxCode lebih dulu mengukur apakah PC kamu
  sanggup.

@en
## What FluxCode includes out of the box (that VS Code and Zed don't)

Standard VS Code and Zed are great editors, especially for deep LSP ecosystems and
debugging. FluxCode focuses on a different set of capabilities shipped directly inside its
single 16 MB native binary with zero extensions to configure:

### 1. Extreme resource efficiency & zero telemetry
- **~50 ms cold-to-screen launch, 1.4 MB idle RAM, 1 process, 16 MB binary.** VS Code runs
  Chromium + Node.js across ~9 processes (~929 MB idle, ~1 GB on disk); Zed uses ~66 MB
  idle and ~261 MB on disk. FluxCode actively trims unused pages back to Windows when idle.
- **Zero telemetry by design.** No account login required, no analytics IDs, no hidden
  background sync.

### 2. Built-in Rust WizTree disk analyzer wired directly into AI agents
- **Interactive treemap & disk tree (`Ctrl+Shift+Y`).** Scans an entire workspace or any
  folder in milliseconds using a flat parallel Rust arena (zero background polling when
  closed), with squarified treemap zoom, extension breakdown, and top 100 largest files.
- **Direct AI agent storage context (`@wiztree`, Ask Agent, `fluxcode --wiztree`).**
  Instead of burning tokens and CPU running manual `find`/`du`/`ripgrep` scans, agents
  receive a single-pass compact map of largest folders, lightweight folders, tiny files,
  and cache/bloat hotspots immediately. Neither VS Code nor Zed includes a native disk
  analyzer connected to AI context.

### 3. Production-grade ACP Agent Queue, Force Send, & Background Tray Execution
- **Full FIFO Queue + Context-Aware Force Send (`Ctrl+Enter` / `Now`).** Queue multiple
  tasks with Pause/Resume, reorder (Up/Down), Edit, Remove, and Clear. Force-sending a new
  request while the agent is busy captures the paused task's progress (prompt, plan, tools,
  modified files, partial output), handles your urgent request first, and resumes the
  unfinished task automatically without redoing completed work.
- **Isolated Windows Job Object + Close-to-Tray.** Each AI agent runs in an isolated
  process group & Windows Job Object while retaining full access to your laptop's `PATH`,
  SDKs, and files. Closing FluxCode while an agent is actively running minimizes cleanly to
  the Windows System Tray (reopen from tray or by launching FluxCode again); closing once
  stopped shuts down FluxCode and atomically terminates every agent subprocess with zero
  zombie processes.
- **Built-in Token Saver, Quota Auto-Resume, Per-Turn Undo, & Browser DevTools MCP.**
  Cut agent output/tool bloat with Caveman / Lean code / Quiet tools, view live 5-hour and
  weekly usage meters with automatic **Resume at reset**, review and 1-click undo every
  file edited per turn, auto-discover existing local agents (`PATH`, npm, pnpm, bun, Scoop,
  WinGet) or auto-install SHA-256-verified Node.js/`uv` runtimes into `~/.fluxcode/runtime/`,
  and give every agent a dedicated isolated Chrome/Edge/Brave DevTools browser profile.

### 4. Native binary, Office, database, archive, & Minecraft Bedrock/Java data tools
- **Editable Minecraft Bedrock & Java NBT (zero plugins).** Open `level.dat`,
  `level.dat_old` (old and modern formats), `.mcstructure`, and `.nbt` directly as clean
  SNBT with live syntax diagnostics and lossless binary save, plus read-only **LevelDB**
  (`db/*.log`, `db/*.ldb` with pure-Rust Snappy/Zlib decompression) inspection and built-in
  Paper / Bedrock add-on project tasks.
- **Instant viewers for everyday non-text files.** Read **SQLite** tables, **PDF**,
  **Word (`.docx`)**, **Excel (`.xlsx`)**, **PowerPoint (`.pptx`)**, **ZIP/JAR** archives,
  and raw **Hex** right beside your code without installing third-party extensions.

### 5. Multi-language error-aware terminal with 1-click jump to code
- **Understands 15+ languages out of the box.** Multi-tab shell with typo autocorrect,
  inline directory-aware `Tab` completion cycling, structured error cards with 1-click
  fixes (`pip install`, `npm install`), and clickable `file:line:col` links across Python,
  JS/TS (`tsc`), Rust, Go, Java, Kotlin, C/C++ (GCC/Clang/MSVC), C#/.NET, Zig, PHP, Ruby,
  Lua, Dart, Swift, Shell, and PowerShell that jump straight to the exact line and column.

### 6. Native Markdown, interactive `.flux` docs, fast WSL2 helper, & adaptive UI
- **GPU-rendered Markdown & `.flux` docs.** Preview GitHub Markdown natively without
  Chromium, author bilingual `.flux` docs with flowcharts/annotations and compile to
  Markdown via `--flux-to-md`, and preview live HTML side-by-side.
- **Built-in WSL2 fast disk helper.** Avoids Windows `\\wsl.localhost` 9p bottlenecks by
  reading/writing inside the distro through a tiny built-in Linux helper.
- **Native VS Code theme import with contrast safety + dockable sidebars.** Import
  Marketplace themes (Dracula, One Dark Pro, etc.) with automatic 4.5:1 contrast lifting,
  dock any view to Left or Right sidebar, navigate globally with Mouse Back/Forward
  buttons, and optionally render GPU-drawn theme-tinted live backgrounds (0% CPU when still).

@id
## Keunggulan bawaan FluxCode (yang tidak ada di VS Code dan Zed standar)

VS Code dan Zed standar tetap unggul untuk ekosistem LSP mendalam dan debugger penuh.
FluxCode berfokus pada serangkaian kemampuan praktis yang langsung tertanam di satu file
binary native 16 MB tanpa perlu memasang banyak extension:

### 1. Sangat ringan & nol telemetri
- **Terbuka ~50 ms, 1,4 MB RAM saat idle, 1 proses, ukuran 16 MB.** VS Code menjalankan
  Chromium + Node.js di ~9 proses (~929 MB idle, ~1 GB di disk); Zed memakai ~66 MB idle
  dan ~261 MB di disk. FluxCode otomatis mengembalikan memori tak terpakai ke Windows saat
  diam.
- **Tanpa telemetri sejak awal.** Tidak butuh login akun, tanpa ID analitik, dan tanpa
  sinkronisasi tersembunyi di latar belakang.

### 2. WizTree penganalisis disk Rust bawaan yang langsung dipahami AI agent
- **Treemap & pohon disk interaktif (`Ctrl+Shift+Y`).** Memindai seluruh workspace atau
  folder mana saja dalam hitungan milidetik memakai arena paralel Rust (nol beban latar
  belakang saat ditutup), lengkap dengan zoom treemap, rincian ekstensi, dan 100 file
  terbesar.
- **Konteks penyimpanan langsung ke AI agent (`@wiztree`, Tanya Agent, `fluxcode --wiztree`).**
  Agent tidak perlu boros token dan CPU menjalankan `find`/`du`/`ripgrep` manual, karena
  langsung menerima peta ringkas folder terbesar, folder paling ringan, file mungil, serta
  titik boros/cache dalam sekali baca. Baik VS Code maupun Zed standar tidak memiliki fitur
  ini.

### 3. Sistem Queue Agent ACP lengkap, Force Send ber-konteks, & Background Tray
- **Antrean FIFO penuh + Force Send ber-konteks (`Ctrl+Enter` / `Sekarang`).** Antrekan
  beberapa tugas sekaligus dengan tombol Jeda/Lanjut, ubah urutan (Naik/Turun), Edit,
  Hapus, dan Hapus Semua. Saat memaksa kirim pesan baru di tengah pekerjaan agent, FluxCode
  merekam progres tugas yang dijeda (prompt awal, plan, tool, file diubah, output terakhir),
  mengerjakan permintaan baru lebih dulu, lalu otomatis melanjutkan tugas sebelumnya sampai
  selesai tanpa mengulang pekerjaan yang sudah beres.
- **Isolasi Windows Job Object + Close-to-Tray.** Setiap AI agent berjalan dalam process
  group dan Windows Job Object tersendiri sambil tetap mendeteksi `PATH`, SDK, dan file
  laptop. Menutup FluxCode saat agent masih bekerja akan menyembunyikan jendela ke Windows
  System Tray (klik ikon tray atau buka ulang FluxCode untuk kembali); menutup saat agent
  sudah berhenti akan menutup total FluxCode sekaligus mematikan seluruh subprocess agent
  tanpa proses zombie.
- **Hemat Token, Resume Otomatis saat Limit Reset, Undo per Giliran, & Browser DevTools MCP.**
  Pangkas pemborosan token lewat Caveman / Lean code / Quiet tools, pantau kuota 5 jam dan
  mingguan dengan **Lanjut saat reset** otomatis, tinjau diff dan undo satu klik per giliran,
  deteksi otomatis agent lokal atau pasang runtime Node.js/`uv` berverifikasi SHA-256 ke
  `~/.fluxcode/runtime/`, plus browser DevTools terisolasi untuk setiap agent.

### 4. Pembaca file biner, Office, database, arsip, & data Minecraft Bedrock/Java bawaan
- **Edit NBT Minecraft Bedrock & Java tanpa plugin.** Buka `level.dat`, `level.dat_old`
  (format lama maupun baru), `.mcstructure`, dan `.nbt` langsung sebagai SNBT bersih lengkap
  dengan diagnostik sintaks dan penyimpanan balik ke biner secara lossless, ditambah viewer
  baca-saja **LevelDB** (`db/*.log`, `db/*.ldb` dengan dekompresi Snappy/Zlib pure Rust)
  serta dukungan task proyek Paper dan Bedrock add-on bawaan.
- **Viewer instan untuk file non-teks harian.** Baca tabel **SQLite**, **PDF**, **Word
  (`.docx`)**, **Excel (`.xlsx`)**, **PowerPoint (`.pptx`)**, arsip **ZIP/JAR**, dan
  **Hex** langsung di sebelah kode tanpa extension tambahan.

### 5. Terminal multibahasa dengan pemahaman error & klik langsung ke baris kode
- **Memahami 15+ bahasa pemrograman langsung dari awal.** Terminal multi-tab dengan koreksi
  typo, cycling `Tab` inline (khusus folder untuk `cd`), kartu error terstruktur dengan
  tombol perbaikan satu klik (`pip install`, `npm install`), serta tautan klik
  `file:baris:kolom` lintas Python, JS/TS (`tsc`), Rust, Go, Java, Kotlin, C/C++
  (GCC/Clang/MSVC), C#/.NET, Zig, PHP, Ruby, Lua, Dart, Swift, Shell, dan PowerShell yang
  langsung melompat ke baris dan kolom kode.

### 6. Pratinjau Markdown native, dokumen `.flux`, helper WSL2 cepat, & UI adaptif
- **Render Markdown & dokumen `.flux` langsung di GPU.** Pratinjau Markdown GitHub tanpa
  Chromium, tulis dokumen bilingual `.flux` lengkap dengan flowchart/anotasi dan ekspor ke
  Markdown lewat `--flux-to-md`, plus pratinjau HTML langsung berdampingan.
- **Helper disk WSL2 bawaan.** Menghindari lambatnya share 9p `\\wsl.localhost` dengan
  baca/tulis langsung di dalam distro lewat helper Linux mini bawaan.
- **Impor tema VS Code dengan jaminan kontras + side bar fleksibel.** Pasang tema
  Marketplace (Dracula, One Dark Pro, dll.) dengan penyesuaian kontras otomatis minimal
  4,5:1, pindah panel mana pun ke side bar kiri/kanan, navigasi global dengan tombol
  Back/Forward mouse, serta pilihan latar hidup bertema di GPU (0% CPU saat diam).

@en
## FluxCode, Zed and VS Code

All three are good editors made for different people. This is where each stands today.

FluxCode is the lightest of the three by a wide margin, and the gap grows the longer the
editor stays open. It uses 1.4 MB idle against 66.2 MB for Zed and 929.2 MB for VS Code,
and takes 16 MB on disk against 261 MB and 1,044 MB. It opens in about 50 ms, against 818 ms
and about 3 s. What it gives up is clear in the table: no debugger, no language servers,
and VS Code extensions install but their code doesn't run.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Built with | Rust, GPUI (GPU-drawn) | Rust, GPUI (GPU-drawn) | TypeScript on Electron (Chromium + Node.js) |
| Systems | Windows, macOS, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| What you download | one program, about 16 MB | an installer | an installer |
| Layout and keys | VS Code's layout and VS Code's keys for the commands it has | its own layout; a VS Code key map is available | the original |
| Start, memory, CPU, size | about 50 ms, 1.4 MB idle, 1 process, 16 MB ([benchmark](#benchmark)) | 0.82 s, 66 MB, 2 processes, 261 MB | about 3 s, 929 MB, 9 processes, 1 GB |
| Telemetry | none | on by default, can be turned off | on by default in Microsoft's build, can be turned off |
| Language smarts (go to definition, rename, diagnostics) | not yet: colors and word completion | built in, through language servers | through extensions |
| Debugger | not yet (Run only) | yes | yes |
| Extensions | the VS Code Marketplace to browse and install (their code doesn't run), plus 11 built-in project manifests | yes | the largest catalog |
| AI agent | any ACP agent, FIFO queue + context-aware force send, Job Object isolation, close-to-tray, Token Saver, quota auto-resume, Browser MCP, per-turn undo; Copilot ghost text | its own agent and ACP agents | GitHub Copilot and extensions |
| Built-in disk analyzer (WizTree) | built in (Rust treemap + `@wiztree` AI context + CLI) | not built in | through extensions only |
| Files that are not text | hex, PDF, Office, ZIP, SQLite, LevelDB (read-only); Bedrock/Java NBT (editable) | images | images; more through extensions |
| Remote development | WSL2 (Windows) | SSH | SSH, containers, WSL |
| Interface languages | English, Bahasa Indonesia | English | many, through language packs |
| Maturity | development builds | stable releases | stable, widely used |
| License | GPL-3.0-or-later | GPL-3.0 (the editor) | MIT for Code - OSS; Microsoft's build has its own license |

Pick FluxCode if you want a small, quiet editor on Windows or Linux with an agent you can
keep on a short leash, your code can live in a WSL2 distro, and you open logs, databases
and documents next to your code.
Pick Zed for macOS, language servers out of the box and pair programming.
Pick VS Code when you need a specific extension, a debugger or remote development beyond WSL.

## Other editors at a glance

These editors were not in the benchmark, so this table has no measured numbers for
them. It compares only what each project documents: what it is built with, where it
runs, and what ships built in.

| Editor | Built with | Systems | Language servers | Debugger | Extensions | AI agent |
| --- | --- | --- | --- | --- | --- | --- |
| Sublime Text | C++ | Windows, macOS, Linux | through the LSP package, not built in | none built in | Python plugins, through Package Control | none built in |
| Notepad++ | C++ (Win32, Scintilla) | Windows only | through plugins, not built in | none built in | plugins, through Plugin Admin | none built in |
| Lapce | Rust | Windows, macOS, Linux | built in | none built in | WASI plugins | none built in |
| Helix | Rust | terminal, on Windows, macOS and Linux | built in | built in, through DAP (experimental) | none yet | none built in |
| Neovim | C, with Lua | terminal, on Windows, macOS and Linux | built in | through plugins, not built in | Lua plugins, a large ecosystem | none built in, through plugins |

FluxCode sits closest to Lapce and Helix in spirit: a small Rust editor that stays out
of the way. What sets it apart from them is the graphical interface, the VS Code
layout and keys, and the agent panel. None of the five editors above was measured in
the benchmark, so do not read this table as a speed ranking.

@id
## FluxCode, Zed, dan VS Code

Ketiganya editor bagus yang dibuat untuk orang berbeda. Berikut posisi masing-masing hari
ini.

FluxCode adalah yang paling ringan dari ketiganya dengan selisih yang besar, dan selisih
itu makin terasa makin lama editor dibiarkan terbuka. Ia memakai 1,4 MB saat idle lawan
66,2 MB untuk Zed dan 929,2 MB untuk VS Code, dan memakan 16 MB di disk lawan 261 MB dan
1.044 MB. Ia terbuka dalam sekitar 50 ms, lawan 818 ms dan sekitar 3 detik. Yang ia korbankan
terlihat jelas di tabel: belum ada debugger maupun language server, dan extension VS Code
bisa dipasang tetapi kodenya tidak berjalan.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Dibangun dengan | Rust, GPUI (digambar GPU) | Rust, GPUI (digambar GPU) | TypeScript di Electron (Chromium + Node.js) |
| Sistem | Windows, macOS, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| Yang diunduh | satu program, sekitar 16 MB | sebuah installer | sebuah installer |
| Tata letak dan tombol | tata letak VS Code dan tombol VS Code untuk perintah yang ada | tata letaknya sendiri; tersedia key map VS Code | yang aslinya |
| Start, memori, CPU, ukuran | sekitar 50 ms, 1,4 MB idle, 1 proses, 16 MB ([benchmark](#benchmark)) | 0,82 detik, 66 MB, 2 proses, 261 MB | sekitar 3 detik, 929 MB, 9 proses, 1 GB |
| Telemetri | tidak ada | aktif secara bawaan, bisa dimatikan | aktif secara bawaan di build Microsoft, bisa dimatikan |
| Kecerdasan bahasa (go to definition, rename, diagnostik) | belum ada: warna dan penyelesaian kata | bawaan, lewat language server | lewat extension |
| Debugger | belum ada (hanya Run) | ada | ada |
| Extension | VS Code Marketplace untuk dijelajah dan dipasang (kodenya tidak berjalan), plus 11 manifest proyek bawaan | ada | katalog terbesar |
| Agent AI | agent ACP apa pun, antrean FIFO + force send ber-konteks, isolasi Job Object, close-to-tray, Hemat Token, auto-resume kuota, Browser MCP, undo per giliran; Copilot inline | agent sendiri dan agent ACP | GitHub Copilot dan extension |
| Penganalisis disk bawaan (WizTree) | bawaan (treemap Rust + konteks AI `@wiztree` + CLI) | tidak bawaan | hanya lewat extension |
| File yang bukan teks | hex, PDF, Office, ZIP, SQLite, LevelDB (baca-saja); NBT Bedrock/Java (bisa diedit) | gambar | gambar; lebih banyak lewat extension |
| Pengembangan jarak jauh | WSL2 (Windows) | SSH | SSH, container, WSL |
| Bahasa antarmuka | Inggris, Bahasa Indonesia | Inggris | banyak, lewat language pack |
| Kematangan | build pengembangan | rilis stabil | stabil, dipakai luas |
| Lisensi | GPL-3.0-or-later | GPL-3.0 (editornya) | MIT untuk Code - OSS; build Microsoft punya lisensi sendiri |

Pilih FluxCode kalau kamu ingin editor kecil dan tenang di Windows, macOS, atau Linux
dengan agent yang bisa dijaga ketat, kodemu bisa tinggal di distro WSL2, dan kamu membuka
log, database, serta dokumen di samping kode.
Pilih Zed untuk language server yang langsung jalan dan pair programming.
Pilih VS Code kalau kamu butuh extension tertentu, debugger, atau pengembangan jarak jauh
selain WSL.

## Editor lain sekilas

Editor-editor ini tidak ikut dalam benchmark, jadi tabel ini tidak berisi angka ukur
untuk mereka. Tabel ini hanya membandingkan apa yang didokumentasikan masing-masing
proyek: dibuat dengan apa, berjalan di mana, dan apa yang sudah terpasang bawaan.

| Editor | Dibangun dengan | Sistem | Language server | Debugger | Extension | Agent AI |
| --- | --- | --- | --- | --- | --- | --- |
| Sublime Text | C++ | Windows, macOS, Linux | lewat paket LSP, bukan bawaan | tidak ada bawaan | plugin Python, lewat Package Control | tidak ada bawaan |
| Notepad++ | C++ (Win32, Scintilla) | Windows saja | lewat plugin, bukan bawaan | tidak ada bawaan | plugin, lewat Plugin Admin | tidak ada bawaan |
| Lapce | Rust | Windows, macOS, Linux | bawaan | tidak ada bawaan | plugin WASI | tidak ada bawaan |
| Helix | Rust | terminal, di Windows, macOS, dan Linux | bawaan | bawaan, lewat DAP (eksperimental) | belum ada | tidak ada bawaan |
| Neovim | C, dengan Lua | terminal, di Windows, macOS, dan Linux | bawaan | lewat plugin, bukan bawaan | plugin Lua, ekosistem besar | tidak ada bawaan, lewat plugin |

FluxCode paling dekat dengan Lapce dan Helix secara semangat: editor Rust kecil yang
tidak menghalangi. Yang membedakannya dari mereka adalah antarmuka grafis, tata letak
dan tombol VS Code, serta panel agent. Kelima editor di atas tidak diukur dalam
benchmark, jadi jangan baca tabel ini sebagai peringkat kecepatan.

@en
## Privacy

FluxCode sends nothing about you. The only request it makes by itself is the update
check: a plain `GET` of the release list from github.com, with no build ID, machine ID or
cookie, at start and every 6 hours (Settings > Updates turns it off). Everything else goes
out only when you start it: the agent you run, `git`, GitHub when you sign in, and the new
build's `changelog.flux` when you click What's new on the update card. The
agent menu reads the public ACP agent list from cdn.agentclientprotocol.com, at most once
a day and only when you open the menu.
The first time an agent starts, FluxCode installs the agents' browser tool
(chrome-devtools-mcp) from the npm registry; Agent settings > Browser > Off skips it.
Extensions download nothing by themselves: Add Gradle wrapper asks services.gradle.org for
the current version and fetches the wrapper from Gradle's GitHub repository, and an
Install button on an extension's page runs `winget` in the Terminal, only when you click.
The Extensions view asks marketplace.visualstudio.com for the list while it is open, and
downloads an extension's VSIX only when you click Install.
@id
## Privasi

FluxCode tidak mengirim apa pun tentangmu. Satu-satunya permintaan yang dibuat sendiri
adalah pemeriksaan update: `GET` biasa ke daftar rilis di github.com, tanpa build ID,
machine ID, atau cookie, saat start dan tiap 6 jam (Settings > Updates mematikannya).
Sisanya keluar hanya saat kamu memulainya: agent yang kamu jalankan, `git`, GitHub saat
kamu masuk, dan `changelog.flux` build baru saat kamu menekan What's new di kartu update.
Menu agent membaca daftar agent ACP publik dari cdn.agentclientprotocol.com, paling banyak
sekali sehari dan hanya saat menu itu dibuka.
Saat agent pertama kali dijalankan, FluxCode memasang alat browser untuk agent
(chrome-devtools-mcp) dari registri npm; Agent settings > Browser > Off melewatinya.
Extension tidak mengunduh apa pun sendiri: Add Gradle wrapper menanyakan versi terbaru ke
services.gradle.org dan mengambil wrapper dari repositori GitHub Gradle, dan tombol Install
di halaman sebuah extension menjalankan `winget` di Terminal, hanya saat kamu menekannya.
Tampilan Extensions meminta daftar ke marketplace.visualstudio.com selama tampilan itu
terbuka, dan mengunduh VSIX sebuah extension hanya saat kamu menekan Install.

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
## Roadmap

FluxCode is under active, continuous development with hard CI budgets on binary size,
startup speed, and memory. Here is where we are headed (full checklist and technical scope
in [ROADMAP.md](ROADMAP.md)):

| Phase | Target | Focus |
| --- | --- | --- |
| **Foundation** | Shipped (2026) | Single-process Rust + GPUI editor (~50 ms startup, 1.4 MB idle RAM), ACP agent workspace, Copilot ghost text, Git, WizTree AI context, Bedrock NBT, multi-language terminal, WSL2 helper, Markdown/`.flux` preview with zoom & annotations |
| **Phase 1: Hardening** | Late 2026 - Q1 2027 | Workspace trust model, SHA-256 verified self-updates, OS credential store for tokens, ACP path containment, dirty-tab close guards |
| **Phase 2: Language Intelligence** | Q1 - Q2 2027 | Opt-in lightweight LSP client (`rust-analyzer`, `pyright`, `tsserver`, `gopls`), inline diagnostics & go-to-definition, multi-cursor & split editors, notarized macOS builds |
| **Phase 3: FluxCode for Android** | 2027 | Native `aarch64-linux-android` Vulkan build (zero WebView), touch & Android IME ergonomics, remote desktop/SSH + ACP agent offload, signed APK / F-Droid / Play Store release ([#117](https://github.com/MuaraAI/FluxCode/discussions/117)) |
| **Phase 4: Extensibility & v1.0** | Late 2027+ | Sandboxed WASM extensions, first-class Remote SSH workspaces, semantic `v1.0` release channels |

@id
## Roadmap

FluxCode dikembangkan secara aktif dan berkelanjutan dengan batas CI yang ketat pada ukuran
binary, kecepatan startup, dan pemakaian memori. Berikut arah pengembangan kami (daftar
lengkap dan detail teknisnya ada di [ROADMAP.md](ROADMAP.md)):

| Fase | Target | Fokus |
| --- | --- | --- |
| **Fondasi** | Rilis (2026) | Editor satu proses Rust + GPUI (~50 ms startup, 1,4 MB RAM idle), workspace agent ACP, inline Copilot, Git, konteks AI WizTree, NBT Bedrock, terminal multibahasa, helper WSL2, pratinjau Markdown/`.flux` dengan zoom & corat-coret |
| **Fase 1: Pengamanan** | Akhir 2026 - Q1 2027 | Model workspace trust, verifikasi SHA-256 untuk auto-update, penyimpanan token di credential store OS, isolasi path ACP, pengaman tab belum tersimpan |
| **Fase 2: LSP & Editor Pro** | Q1 - Q2 2027 | Klien LSP ringan opt-in (`rust-analyzer`, `pyright`, `tsserver`, `gopls`), diagnostik & go-to-definition, multi-cursor & split editor, notarisasi macOS |
| **Fase 3: FluxCode untuk Android** | 2027 | Build native `aarch64-linux-android` Vulkan (tanpa WebView), ergonomi layar sentuh & IME Android, koneksi remote desktop/SSH + AI agent ACP, rilis APK / F-Droid / Play Store ([#117](https://github.com/MuaraAI/FluxCode/discussions/117)) |
| **Fase 4: Ekstensibilitas & v1.0** | Akhir 2027+ | Extension WASM ter-sandbox, workspace Remote SSH penuh, rilis versi stabil `v1.0` |

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

Copyright (C) 2026 MuaraAI and contributors.

[GPL-3.0-or-later](LICENSE): everyone is free to use, study, change and share
FluxCode, and every copy carries the same freedom. Third-party components:
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
@id
## Lisensi

Copyright (C) 2026 MuaraAI dan kontributor.

[GPL-3.0-or-later](LICENSE): semua orang bebas memakai, mempelajari, mengubah, dan
membagikan FluxCode, dan setiap salinannya membawa kebebasan yang sama. Komponen
pihak ketiga: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

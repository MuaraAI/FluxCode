// changelog.flux: the release notes FluxCode shows after an update.
//
// FluxCode compiles this file in and opens it as a page the first time a new build
// starts, showing every release newer than the build that ran before. The page
// follows Settings > Language.
//
// Format: Markdown inside a few directives, each on its own line.
//   @release YYYY-MM-DD      starts a release (newest first; builds are named by date)
//   @title en: ...           its title in English (and `@title id:` in Indonesian)
//   @en / @id                the Markdown that follows is English / Indonesian
// Lines starting with // before the first @release are comments.

@release 2026-11-25
@title en: A WSL folder no longer freezes the window at startup
@title id: Folder WSL tidak lagi membekukan jendela saat mulai
@en
With **Read WSL folders through a helper in the distro** on, the first launch after an update installed the helper on the window's own thread: the window stopped responding until the ~13 MB download finished, and a slow connection could keep it there for minutes. FluxCode now opens at once, reading the folder through the shared path while the helper installs in the background. The download has a connect and a total timeout, and an install that fails is not retried on every call.
@id
Dengan **Baca folder WSL lewat helper di dalam distro** aktif, peluncuran pertama setelah pembaruan memasang helper di thread jendela: jendela berhenti merespons sampai unduhan ~13 MB selesai, dan koneksi lambat bisa menahannya bermenit-menit. Sekarang FluxCode langsung terbuka, membaca folder lewat jalur bersama sementara helper dipasang di latar belakang. Unduhan punya batas waktu koneksi dan total, dan pemasangan yang gagal tidak dicoba ulang di setiap pemanggilan.

@release 2026-11-25
@title en: WizTree AI & Disk Analyzer, Smart Agent Queue, Interactive Tables, Editable Bedrock NBT, Multi-Terminal, and Native UI Precision
@title id: WizTree AI & Penganalisis Disk, Queue Agent Cerdas, Tabel Interaktif, Edit NBT Bedrock, Multi-Terminal, dan Presisi UI Native
@en
**Built-in Rust WizTree & Zero-Overhead AI Disk Context**: FluxCode ships a pure-Rust **WizTree disk usage analyzer** (`Ctrl+Shift+Y` or the drive icon in the Activity Bar) with a bounded parallel flat-arena indexer, interactive squarified treemap, expandable size-sorted tree, extension breakdown, top 100 largest files, synchronized tree/treemap hover, directional view transitions, aligned columns, and an animated **WizTree Settings (`⚙`)** popover (treemap toggle, hidden files, skip build caches, sorting, minimum size filter, treemap height). WizTree feeds a single-pass compact disk map (largest/smallest folders, cache/waste hotspots, extensions, and top files) directly to AI agents via `@wiztree`, **Ask Agent**, automatic disk-intent detection, workspace context, or `fluxcode --wiztree`. Fixed prompt context extraction so `@wiztree`, `@file`, terminal output, and error blocks are always read from the actual sent message instead of the cleared composer input, letting AI agents answer disk queries in **0-1 tool calls** instead of running multi-step shell scripts.

**Rebuilt Agent Queue, Context-Aware Force Send, & Interactive Markdown Tables**: queued prompts now execute one-by-one in strict FIFO order with a full Queue panel (**Pause / Resume**, **Clear**, and per-item **Send Now**, **Move Up / Down**, **Edit**, and **Remove**). Pressing **Ctrl+Enter** or **Now** while the agent is busy snapshots the active task's progress (original prompt, plan, tools, edited files, and partial output), interrupts cleanly, handles the urgent request first, and resumes the paused task until complete (skipping continuation when already finished). Agent replies now render Markdown tables with a **responsive, interactive native table widget**: non-wrapping right-aligned numeric/byte-size columns, generous scrollable path/text columns with automatic horizontal scroll on narrow panels, click-header smart sorting (byte sizes, percentages, numbers, and text with pinned `Total` rows), click-cell to open workspace files or copy values, and one-click **Copy Markdown** / **Copy CSV**. Fixed the startup `Invalid params: effort not found: medium` error on non-reasoning models by deferring `thought_level` until a saved model switch updates valid choices and silencing background startup option mismatches.

**Isolated Background Agent Lifecycle & Safe Runtime Auto-Install**: every AI agent runs inside an isolated Windows process group and **Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) while keeping full access to your laptop's `PATH`, SDKs, and workspace. Closing FluxCode while an agent is actively running hides the window to the **Windows System Tray** (with status/completion alerts, `Open FluxCode`, `Stop AI Agent & Exit`, and single-instance wake on relaunch); closing once agents are stopped shuts down FluxCode and terminates every agent process tree with zero zombie processes. Agent runtime setup also fixes the Windows registry `Path` NUL-byte bug (`npm could not start`) and automatically installs **SHA-256-verified** Node.js/npm or `uv` atomically into `~/.fluxcode/runtime/`.

**Full Editable Minecraft Bedrock Data & Multi-Language Multi-Terminal**: binary Minecraft Bedrock files (`level.dat`, `level.dat_old` old and modern formats, `.mcstructure`, `.nbt`, plus Java NBT) now open as clean editable SNBT with live syntax diagnostics and lossless binary saving, alongside read-only **LevelDB** (`db/*.log`, `db/*.ldb` with Snappy/Zlib decompression) inspection and Bedrock pack/world context. The terminal panel now supports **multiple terminal tabs** (`+` or `Ctrl+Shift+\``) with independent working directory, output, scroll, draft command, and exit routing per tab; parses errors and stack traces across Python, JS/TS (`tsc`), Rust, Go, Java, Kotlin, C/C++, C#/.NET, Zig, PHP, Ruby, Lua, Dart, Swift, Shell, and PowerShell with one-click `file:line:col` editor jump links; and fixes the empty-prompt `Tab` directory-spam bug with inline completion cycling.

**Precision Native UI, Navigation, & Editor Ergonomics**: tightened oversized rounded radii (`8-14 px` / pills) across the Agent panel and glass overlays to crisp `2-6 px` native geometry; removed the redundant assistant speaker label above `Worked`; aligned the activity timeline circles and 1 px connector rails to exact odd-pixel centers; fixed the Windows frameless window top border offset so the top of the title bar is never clipped above the screen and centered the `Run` / `Minimize` / `Maximize` / `Close` title bar icons precisely. Added **Explorer pixel-accurate hover tracking** at any scroll depth, global **Mouse Back/Forward** (`Alt+Left`/`Alt+Right` + Title Bar & WizTree `←`/`→` buttons), **Settings > Editor > Wheel scroll speed** (`50%` to `300%`, [#112](https://github.com/MuaraAI/FluxCode/issues/112)), narrow-width responsive fixes, live hover-preview theme dropdown, smooth gliding toggles, smooth Agent file chip jump, and auto-scrolling tab bar. Completely removed discontinued Live Share (`src/collab/`, `relay/`, and 4 unused crates) and Mura ACP entries.
@id
**WizTree Rust Bawaan & Konteks Disk AI Tanpa Beban**: FluxCode kini membawa **penganalisis penggunaan disk WizTree** berbasis Rust murni (`Ctrl+Shift+Y` atau ikon drive di Activity Bar) dengan indekser paralel flat-arena yang hemat memori, treemap interaktif, pohon folder terurut ukuran, rincian ekstensi, daftar 100 file terbesar, sinkronisasi hover pohon/treemap, transisi terarah, kolom tabel sejajar, serta popover **Pengaturan WizTree (`⚙`)** beranimasi (toggle treemap, file tersembunyi, lewati cache build, urutan, filter ukuran minimum, tinggi treemap). WizTree menyuplai peta disk ringkas satu-pass (folder terbesar/terkecil, titik boros/cache, ekstensi, dan file terbesar) langsung ke AI agent lewat `@wiztree`, **Tanya Agent**, deteksi otomatis pertanyaan disk, konteks workspace, atau `fluxcode --wiztree`. Bug pembacaan konteks saat pesan dikirim telah diperbaiki sehingga `@wiztree`, `@file`, output terminal, dan error selalu dibaca dari teks pesan yang dikirim (bukan kotak input yang sudah dikosongkan), membuat AI agent menjawab analisis disk dalam **0-1 tool call** tanpa skrip shell berulang.

**Sistem Queue Agent Baru, Force Send Ber-konteks, & Tabel Markdown Interaktif**: antrean pesan kini dijalankan satu per satu secara berurutan (FIFO) dengan panel Queue lengkap (**Jeda / Lanjut**, **Hapus semua**, serta per-item **Kirim Sekarang**, **Naikkan / Turunkan**, **Edit**, dan **Hapus**). Menekan **Ctrl+Enter** atau **Sekarang** saat agent sibuk merekam progres tugas aktif (prompt awal, plan, tool, file diubah, dan output terakhir), membatalkan giliran secara rapi, mengerjakan pesan prioritas lebih dulu, lalu otomatis melanjutkan tugas sebelumnya sampai tuntas (dan melewati kelanjutan bila sudah selesai). Balasan agent kini merender tabel Markdown dengan **widget tabel native yang responsif dan interaktif**: kolom angka/ukuran rata kanan tanpa patah baris, kolom teks/path lega dengan scroll horizontal otomatis di panel sempit, klik header untuk urutkan cerdas (ukuran byte, persen, angka, dan teks dengan baris `Total` tetap di bawah), klik sel untuk langsung buka file workspace atau salin nilai, serta tombol satu klik **Salin Markdown** dan **Salin CSV**. Error startup `Invalid params: effort not found: medium` pada model non-reasoning diperbaiki dengan menunggu opsi model baru selesai dimuat sebelum menyetel `thought_level` dan membisukan mismatch opsi otomatis saat startup.

**Lifecycle Agent Latar Belakang Terisolasi & Pasang Runtime Otomatis yang Aman**: setiap AI agent berjalan dalam process group dan **Windows Job Object** tersendiri (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) sambil tetap mendeteksi `PATH`, SDK, dan workspace laptop. Menutup FluxCode saat agent masih berjalan menyembunyikan jendela ke **Windows System Tray** (dengan notifikasi status/selesai, `Buka FluxCode`, `Hentikan AI Agent & Keluar`, dan bangun otomatis saat `fluxcode.exe` dibuka ulang); menutup setelah agent berhenti langsung mematikan penuh sistem dan seluruh proses anak agent tanpa proses zombie. Pemasangan environment agent juga memperbaiki bug NUL-byte registry `Path` Windows (`npm could not start`) dan otomatis mengunduh serta memasang runtime resmi **Node.js/npm** atau **uv** berverifikasi **SHA-256** secara atomik di `~/.fluxcode/runtime/`.

**Dukungan Penuh Edit Data Minecraft Bedrock & Multi-Terminal Multibahasa**: file biner Minecraft Bedrock (`level.dat`, `level.dat_old` format lama maupun baru, `.mcstructure`, `.nbt`, serta NBT Java) kini terbuka sebagai SNBT bersih yang bisa diedit dengan diagnostik langsung dan disimpan kembali ke biner secara lossless, dilengkapi viewer baca-saja **LevelDB** (`db/*.log`, `db/*.ldb` dengan dekompresi Snappy/Zlib) serta konteks dunia/pack Bedrock. Panel terminal kini mendukung **banyak tab terminal** (`+` atau `Ctrl+Shift+\``) dengan direktori kerja, output, scroll, draf perintah, dan routing exit masing-masing; memahami error dan stack trace lintas Python, JS/TS (`tsc`), Rust, Go, Java, Kotlin, C/C++, C#/.NET, Zig, PHP, Ruby, Lua, Dart, Swift, Shell, dan PowerShell dengan klik langsung ke `file:baris:kolom`; serta memperbaiki bug tombol `Tab` kosong dengan cycling kandidat inline.

**Presisi UI Native, Navigasi, & Ergonomi Editor**: sudut membulat berlebihan (`8-14 px` / pill) di panel Agent dan glass overlay dirapikan menjadi geometri native tajam `2-6 px`; label pembicara agent berulang di atas `Worked` dihapus; lingkaran dan garis vertikal `1 px` timeline aktivitas diratakan persis ke tengah piksel ganjil; offset bingkai atas jendela Windows diperbaiki agar bagian atas title bar tidak pernah terpotong ke atas layar sekaligus menyejajarkan ikon `Run` / `Minimize` / `Maximize` / `Close` tepat di tengah. Ditambahkan **sorotan hover Explorer presisi** di semua posisi scroll, navigasi global **tombol samping Mouse Back/Forward** (`Alt+Left`/`Alt+Right` + tombol `←`/`→` di Title Bar & WizTree), **Pengaturan > Editor > Kecepatan gulir roda** (`50%` sampai `300%`, [#112](https://github.com/MuaraAI/FluxCode/issues/112)), perbaikan tata letak panel sempit, dropdown tema pratinjau langsung saat hover, switch meluncur halus, lompat halus dari chip file Agent, dan tab bar auto-scroll. Fitur Live Share (`src/collab/`, `relay/`, dan 4 crate tak terpakai) serta pilihan agent Mura ACP telah dihapus sepenuhnya.

@release 2026-11-21
@title en: Saving in a WSL folder no longer hangs
@title id: Menyimpan di folder WSL tidak macet lagi
@en
Saving a file through the WSL helper hung the editor: the helper answers a write with one JSON frame, and FluxCode waited for a second frame that a write never sends, so the save never came back and the window stopped responding instead of falling back to the shared folder. A write now reads only the answer it gets and returns. A test with a mock helper keeps it that way without needing a WSL distro.
@id
Menyimpan file lewat helper WSL membuat editor macet: helper menjawab penulisan dengan satu frame JSON, sementara FluxCode menunggu frame kedua yang tidak pernah dikirim untuk penulisan, jadi penyimpanan tidak pernah kembali dan jendela berhenti merespons alih-alih beralih ke folder bersama. Sekarang penulisan hanya membaca jawaban yang dikirim lalu selesai. Tes dengan helper tiruan menjaganya tetap begitu tanpa perlu distro WSL.

@release 2026-11-19
@title en: Unmatched brackets underlined as you type
@title id: Kurung tanpa pasangan digarisbawahi saat mengetik
@en
A bracket that has no partner now gets a red underline a moment after you stop typing, so an extra `}` or a missing `)` shows up before you run anything ([#105](https://github.com/MuaraAI/FluxCode/issues/105)). Hover it to read what is wrong: unmatched, not closed, or closing the wrong kind. It works in Python, Rust, JavaScript, TypeScript, C, C++, C#, Java, Kotlin, Go, Swift, Dart, PHP, JSON and CSS, and skips brackets inside strings and comments. FluxCode checks the brackets only, not the rest of the syntax, and adds nothing to its size for it.
@id
Kurung yang tidak punya pasangan sekarang diberi garis bawah merah sesaat setelah kamu berhenti mengetik, jadi `}` yang kelebihan atau `)` yang kurang langsung kelihatan sebelum kamu menjalankan apa pun ([#105](https://github.com/MuaraAI/FluxCode/issues/105)). Arahkan kursor ke garisnya untuk membaca masalahnya: tanpa pasangan, belum ditutup, atau menutup jenis kurung yang salah. Fitur ini jalan di Python, Rust, JavaScript, TypeScript, C, C++, C#, Java, Kotlin, Go, Swift, Dart, PHP, JSON, dan CSS, dan melewati kurung di dalam string dan komentar. FluxCode hanya memeriksa kurungnya, bukan seluruh sintaks, dan ukurannya tidak bertambah karenanya.

@release 2026-11-18
@title en: Agents you already have, found
@title id: Agent yang sudah ada, langsung ketemu
@en
FluxCode now finds the agents already on your PC the way Windows does: on the PATH as Windows has it right now (an agent installed while FluxCode is open shows up the next time you open the agent list) and in the folders installers use, such as npm, pnpm, bun, Scoop, WinGet and `~/.local/bin`. **OpenCode 2** installed with npm is used as it is, without a second download, and a download button for an agent that is already there just switches to it. When **Claude Code** or **Codex** is installed, the list says so: only their small ACP bridge is added. The agent list also fits a narrow panel now: the maker's name goes first, the badges shrink to a star, and nothing runs into the state or the download button.
@id
FluxCode sekarang menemukan agent yang sudah ada di PC kamu seperti cara Windows mencarinya: dari PATH Windows saat itu juga (agent yang dipasang saat FluxCode terbuka muncul begitu daftar agent dibuka lagi) dan dari folder yang dipakai installer, seperti npm, pnpm, bun, Scoop, WinGet dan `~/.local/bin`. **OpenCode 2** yang dipasang lewat npm langsung dipakai, tanpa diunduh lagi, dan tombol unduh untuk agent yang ternyata sudah ada cukup beralih ke agent itu. Kalau **Claude Code** atau **Codex** sudah terpasang, daftar memberitahunya: tinggal jembatan ACP-nya yang kecil yang ditambahkan. Daftar agent juga pas di panel sempit: nama pembuat disembunyikan dulu, badge mengecil jadi bintang, dan tidak ada lagi yang menabrak status atau tombol unduh.

@release 2026-11-18
@title en: Twelve live backgrounds, and FluxSky
@title id: Dua belas latar hidup, dan FluxSky
@en
The animated backgrounds are new: **twelve scenes drawn live** instead of pixel videos. Clouds at dusk, a moonlit ocean, mountains in fog, the northern lights, a pine forest with fireflies, a lake, a beach at sunset, dunes, a city with its windows going on and off, rain, snow and a galaxy. They take their colors from your theme, so changing the theme recolors the scene at once (light themes turn it to day), they fit any window size, and nothing is downloaded or decoded. Settings > Appearance shows each one as a live preview in your theme. **Background animation is now off by default**: the scene stands still and costs no CPU. Turned on, the scene moves (about 10 to 15% of one CPU core while FluxCode is in front), and FluxCode first measures what your PC can take and tells you whether it is light, fine or heavy there. The new **FluxSky** theme (violet night with a peach accent, from the website's dusk) brings the clouds with it, and the background you had comes back when you pick another theme. Changing theme now fades smoothly from the old colors to the new ones.
@id
Latar beranimasi sekarang baru: **dua belas scene yang digambar langsung**, bukan video piksel lagi. Awan senja, laut di bawah bulan, pegunungan berkabut, aurora, hutan pinus dengan kunang-kunang, danau, pantai saat matahari terbenam, bukit pasir, kota dengan jendela yang menyala dan padam, hujan, salju, dan galaksi. Warnanya diambil dari tema kamu, jadi ganti tema langsung mewarnai ulang scene-nya (tema terang mengubahnya jadi siang), pas di ukuran jendela apa pun, dan tidak ada yang diunduh atau di-decode. Pengaturan > Tampilan menampilkan masing-masing sebagai pratinjau hidup dalam tema kamu. **Animasi latar sekarang mati secara default**: scene-nya diam dan tidak memakai CPU. Kalau dinyalakan, scene bergerak (sekitar 10 sampai 15% dari satu core CPU saat FluxCode di depan), dan FluxCode lebih dulu mengukur apakah PC kamu sanggup, lalu memberi tahu apakah ringan, cukup, atau berat di PC itu. Tema baru **FluxSky** (ungu malam dengan aksen persik, dari senja di website) membawa awannya sendiri, dan latar yang kamu pakai sebelumnya kembali saat memilih tema lain. Ganti tema sekarang berpindah warna dengan halus, bukan melompat.

@release 2026-11-16
@title en: Open with FluxCode
@title id: Buka dengan FluxCode
@en
Files open from **Windows Explorer** now: right-click a file or folder and pick **Open with FluxCode**, or double-click a code file. The installer has this as an option (on by default) and **Settings > Windows** turns it on and off. It adds FluxCode to Open with for .js, .ts, .json, .html, .css, .py and other code and text files; types no app opens yet open in FluxCode right away, and for the rest Windows lets you pick FluxCode once and tick Always. A file opened this way joins the FluxCode window that is already open instead of starting a second one, and a fresh start opens it on top of your last session.
@id
File sekarang bisa dibuka dari **Windows Explorer**: klik kanan file atau folder lalu pilih **Open with FluxCode**, atau klik ganda file kode. Installer menyediakannya sebagai opsi (aktif secara default) dan **Pengaturan > Windows** bisa menyalakan atau mematikannya. FluxCode ditambahkan ke Open with untuk file .js, .ts, .json, .html, .css, .py, dan file kode atau teks lainnya; jenis file yang belum punya aplikasi langsung terbuka di FluxCode, dan untuk yang lain Windows membiarkanmu memilih FluxCode sekali lalu mencentang Always. File yang dibuka begini masuk ke jendela FluxCode yang sudah terbuka, bukan membuka jendela kedua, dan kalau FluxCode belum jalan, file itu dibuka di atas sesi terakhirmu.

@release 2026-11-16
@title en: VS Code themes in FluxCode
@title id: Tema VS Code di FluxCode
@en
A color theme you install from the VS Code Marketplace, such as **Dracula** or **One Dark Pro**, now shows up in the theme menu in the status bar and in Settings, and recolors the interface and the code. Text that would be hard to read on the theme's surfaces is lifted to a readable contrast. File types an extension adds (like `.svelte`) open as its language, and the extension's page lists what FluxCode applied, with buttons to switch to its themes.
@id
Tema warna yang kamu pasang dari Marketplace VS Code, misalnya **Dracula** atau **One Dark Pro**, sekarang muncul di menu tema di bilah status dan di Pengaturan, lalu mewarnai antarmuka dan kode. Teks yang sulit dibaca di permukaan tema dinaikkan kontrasnya supaya tetap terbaca. Jenis file yang ditambahkan ekstensi (seperti `.svelte`) dibuka sebagai bahasanya, dan halaman ekstensi menampilkan apa saja yang dipakai FluxCode, dengan tombol untuk beralih ke temanya.

@release 2026-11-16
@title en: Command Code, over ACP
@title id: Command Code, lewat ACP
@en
The agent panel now lists **Command Code**, the coding agent for open models. FluxCode doesn't download it: install it once with `npm i -g command-code`, run `cmd login` in a terminal, and FluxCode finds it on PATH and starts it over the Agent Client Protocol when you pick it. Its logo ships with the app, so the panel shows the mark even though the registry lists no icon.
@id
Panel agent sekarang menampilkan **Command Code**, coding agent untuk model terbuka. FluxCode tidak mengunduhnya: pasang sekali dengan `npm i -g command-code`, jalankan `cmd login` di terminal, dan FluxCode menemukannya di PATH lalu menjalankannya lewat Agent Client Protocol saat kamu memilihnya. Logo-nya dibawa bareng aplikasi, jadi panel menampilkan mark-nya meskipun registry tidak menyediakan ikonnya.

@release 2026-11-15
@title en: One-click Copilot setup
@title id: Setup Copilot satu klik
@en
Setting up the built-in **Copilot ghost text** no longer needs a terminal:
when the language server is missing, Settings shows an **Install server**
button that installs it in the background (a **Get Node.js** button opens the
download page when Node.js itself is missing). While suggestions cannot work
yet, the status bar shows a small **Copilot** chip that says what is missing
and opens Settings on click; it disappears once suggestions are ready.
@id
Menyiapkan **ghost text Copilot** bawaan tidak lagi butuh terminal: kalau
language server-nya belum ada, Settings menampilkan tombol **Pasang server**
yang memasangnya di latar belakang (tombol **Unduh Node.js** membuka halaman
unduhan kalau Node.js-nya sendiri belum ada). Selama saran belum bisa jalan,
status bar menampilkan chip **Copilot** kecil yang menyebutkan apa yang
kurang dan membuka Settings saat diklik; chip itu hilang sendiri setelah
saran siap.

@title en: HTML preview says it runs a WebView
@title id: Pratinjau HTML bilang ia menjalankan WebView
@en
Opening the HTML preview now shows a notice first: the preview runs a WebView (WebView2), not FluxCode's native Rust UI. The preview only starts after you press Continue; Cancel opens nothing. A "Do not show again" box keeps the notice off from then on.
@id
Membuka pratinjau HTML sekarang menampilkan pemberitahuan dulu: pratinjau menjalankan WebView (WebView2), bukan UI Rust native FluxCode. Pratinjau baru berjalan setelah kamu menekan Lanjutkan; Batal tidak membuka apa pun. Kotak "Jangan tampilkan lagi" membuat pemberitahuan tidak muncul lagi.

@title en: File links for WSL and network folders
@title id: Tautan file untuk folder WSL dan jaringan
@en
Files in a WSL workspace or on a network share are now sent to Copilot and the
agents as proper `file://` links, the same form VS Code uses. A WSL path written
with forward slashes or mixed slashes is recognised as the same WSL folder, and
a `file://wsl.localhost/...` link printed in the terminal opens the file.
@id
File di workspace WSL atau di folder jaringan sekarang dikirim ke Copilot dan
agen sebagai tautan `file://` yang benar, bentuk yang sama dengan yang dipakai
VS Code. Path WSL yang ditulis dengan garis miring biasa atau campuran dikenali
sebagai folder WSL yang sama, dan tautan `file://wsl.localhost/...` yang
tercetak di terminal membuka filenya.

@title en: What's new shows only what is new
@title id: What's new hanya menampilkan yang baru
@en
What's new used to show the same entry after every update. Two things caused it:
the release notes it reads had not been updated with the latest changes, and when
nothing looked new it showed the newest entry again instead of nothing. Both are
fixed: the notes now carry every change in the changelog, and an entry is only
shown when the build that carries it is newer than the one before. If an update
brings no new entry, the page says there are no notes for this build.
@id
What's new dulu menampilkan entri yang sama setelah setiap update. Dua hal jadi
penyebabnya: catatan rilis yang dibacanya belum diperbarui dengan perubahan
terbaru, dan saat tidak ada yang tampak baru ia menampilkan lagi entri terbaru
bukannya tidak menampilkan apa-apa. Keduanya sudah diperbaiki: catatan sekarang
memuat setiap perubahan di changelog, dan sebuah entri hanya ditampilkan kalau
build yang membawanya lebih baru dari build sebelumnya. Kalau sebuah update tidak
membawa entri baru, halaman menyatakan tidak ada catatan untuk build ini.

@release 2026-11-15
@title en: Settings page: two panes, one section at a time
@title id: Halaman Settings: dua panel, satu seksi sekali tampil
@en
Settings is now two panes: a narrow navigation on the left lists Appearance,
Editor, WSL, AI agents and GitHub, Agent instructions, Keyboard shortcuts,
Updates, and About FluxCode, and the main area shows one section at a time.
Every setting is still there, with the same controls. Each section reads as one
bordered card: one row per setting, its title and a short description on the
left, the control at the right edge.

Switching sections fades the new one in over 160 ms, opacity only, so it costs
no layout pass per frame. Only the active section is built. A section whose
settings differ from the defaults carries a small dot on its nav row, so a
changed setting stays findable. Under the system's reduce-motion setting there
is no motion at all.
@id
Settings sekarang dua panel: navigasi sempit di kiri menampilkan Appearance,
Editor, WSL, AI agents and GitHub, Agent instructions, Keyboard shortcuts,
Updates, dan About FluxCode, dan area utama menampilkan satu seksi setiap kali.
Setiap pengaturan tetap ada, dengan kontrol yang sama. Setiap seksi dibaca sebagai
satu kartu berbingkai: satu baris per pengaturan, judul dan deskripsi singkat di
kiri, kontrol di tepi kanan.

Berpindah seksi memunculkan seksi baru dengan fade 160 ms, hanya opacity, jadi
tidak butuh layout pass per frame. Hanya seksi aktif yang dibangun. Seksi yang
pengaturannya berbeda dari bawaan membawa titik kecil di baris nav-nya, jadi
pengaturan yang berubah tetap mudah ditemukan. Di bawah pengaturan reduce-motion
sistem, tidak ada gerakan sama sekali.

@release 2026-11-14
@title en: Explorer context menu, inline names, compact Extensions
@title id: Menu konteks Explorer, nama inline, Extensions ringkas
@en
Right-click a file or folder in the Explorer opens a context menu, like VS Code:
New File, New Folder, Rename, Delete, Copy Path, Copy Relative Path, Reveal in
File Explorer. Right-clicking a row selects it first, so the highlight and the
menu act on the same row. New File, New Folder, and Rename no longer open a box
at the top of the Explorer: the name is typed in a row inside the tree itself.
Enter confirms, Esc cancels.

The Extensions list is compact: a smaller icon, name and version on one line,
the description on one line that ends with an ellipsis, and a small Install
button at the right edge. A new empty file opens clean, without the bar that said
the file is empty. Clicking a file in the sidebar while Settings is open now
shows that file right away.
@id
Klik kanan file atau folder di Explorer membuka menu konteks, seperti VS Code:
New File, New Folder, Rename, Delete, Copy Path, Copy Relative Path, Reveal in
File Explorer. Klik kanan sebuah baris memilihnya dulu, jadi sorotan dan menunya
menindak baris yang sama. New File, New Folder, dan Rename tidak lagi membuka
kotak di atas Explorer: namanya diketik di baris di dalam tree itu sendiri.
Enter mengonfirmasi, Esc membatalkan.

Daftar Extensions lebih ringkas: ikon lebih kecil, nama dan versi dalam satu
baris, deskripsi dalam satu baris yang berakhir dengan elipsis, dan tombol Install
kecil di tepi kanan. File baru yang kosong terbuka bersih, tanpa bar yang menyatakan
file itu kosong. Mengklik file di sidebar saat Settings terbuka kini langsung
menampilkan file itu.

@release 2026-11-14
@title en: GitHub Copilot inline completions
@title id: Inline completion GitHub Copilot
@en
The editor now suggests the next lines of code as ghost text, from GitHub
Copilot through the official Copilot language server. A suggestion appears dimmed
after the cursor a moment after you stop typing; Tab accepts it, Esc dismisses
it. Turn it on or off in **Settings > Editor > Copilot suggestions**, and sign
in in **Settings > AI agents and GitHub > GitHub Copilot**.

FluxCode stays a thin client: the server runs as its own process and does the
model work, so the editor stays native and light. Install it once with
`npm install -g @github/copilot-language-server` (needs Node.js 20.8 or later),
then sign in with a Copilot plan. Only the open file is sent to the server, as
the document sync the protocol requires.
@id
Editor sekarang menyarankan baris kode berikutnya sebagai ghost text, dari GitHub
Copilot lewat server bahasa Copilot resmi. Saran muncul redup setelah kursor
sesaat setelah kamu berhenti mengetik; Tab menerima, Esc menolak. Nyalakan atau
matikan di **Settings > Editor > Copilot suggestions**, dan masuk di
**Settings > AI agents and GitHub > GitHub Copilot**.

FluxCode tetap klien tipis: server berjalan sebagai prosesnya sendiri dan
mengerjakan model, jadi editor tetap native dan ringan. Pasang sekali dengan
`npm install -g @github/copilot-language-server` (butuh Node.js 20.8 atau lebih
baru), lalu masuk dengan paket Copilot. Hanya file yang terbuka dikirim ke server,
sebagai sinkronisasi dokumen yang dibutuhkan protokol.

@release 2026-11-14
@title en: WSL2 workspaces: commits keep their message
@title id: Workspace WSL2: commit menyimpan pesannya
@en
Committing from Source Control in a WSL2 folder no longer fails with "Aborting
commit due to empty commit message" when the helper is on: the message now
reaches git through the helper, the same way it already did through `wsl.exe`.
@id
Commit dari Source Control di folder WSL2 tidak lagi gagal dengan "Aborting
commit due to empty commit message" saat helper menyala: pesan sekarang sampai
ke git lewat helper, sama seperti sebelumnya lewat `wsl.exe`.

@release 2026-11-14
@title en: Code of Conduct and Security Policy
@title id: Code of Conduct dan Security Policy
@en
The repository now has a Code of Conduct and a Security Policy
(`CODE_OF_CONDUCT.md`, `SECURITY.md`). Security problems are reported privately
through GitHub Security Advisories, not in a public issue.
@id
Repositori sekarang punya Code of Conduct dan Security Policy
(`CODE_OF_CONDUCT.md`, `SECURITY.md`). Masalah keamanan dilaporkan secara pribadi
lewat GitHub Security Advisories, bukan di issue publik.

@release 2026-11-13
@title en: Pull requests open to everyone, with automatic checks
@title id: Pull request terbuka untuk semua, dengan pemeriksaan otomatis
@en
Pull requests from outside the MuaraAI organization are no longer closed. Anyone
can fork, change and open one; CONTRIBUTING.md says how. Every pull request gets
a safety scan that only reads the diff (malware patterns, leaked secrets, Trojan
Source characters, committed executables, dependencies from outside crates.io,
outside changes to CI and build files), and a verdict that compares it with main
on the same runner: executable size, run time and peak memory, clippy and rustfmt
on the added lines, and new RustSec advisories.

Changes to CI, build, packaging, updater and network code ask the maintainers for
a review automatically (`.github/CODEOWNERS`). Each check leaves one comment that
updates on every push.
@id
Pull request dari luar organisasi MuaraAI tidak lagi ditutup. Siapa saja bisa
fork, mengubah dan membukanya; CONTRIBUTING.md menjelaskan caranya. Setiap pull
request mendapat safety scan yang hanya membaca diff (pola malware, secret yang
bocor, karakter Trojan Source, executable yang di-commit, dependensi dari luar
crates.io, perubahan luar pada file CI dan build), dan verdict yang
membandingkannya dengan main di runner yang sama: ukuran executable, waktu jalan
dan memori puncak, clippy dan rustfmt pada baris yang ditambah, dan advisory
RustSec baru.

Perubahan pada kode CI, build, packaging, updater dan network meminta review dari
maintainer secara otomatis (`.github/CODEOWNERS`). Setiap pemeriksaan meninggalkan
satu komentar yang diperbarui setiap push.

@release 2026-11-13
@title en: WSL2 workspaces read through a helper in the distro
@title id: Workspace WSL2 membaca lewat helper di dalam distro
@en
A folder inside a **WSL2 distro** now reads and writes through a small helper
running **inside the distro**, instead of reaching it over Windows' 9p share.
Opening a folder, walking the tree, opening and saving a file, find-in-files and
git all go through it, so a home folder with 70,000 cache entries no longer takes
seconds to list. It is off by default: turn it on in **Settings > WSL**.

The helper is FluxCode itself, built for Linux, from the release of the build that
is running: the two are always the same version and nothing extra ships. It is
installed once per build in `~/.fluxcode/helper/<build>/`. If it cannot be
installed or started, everything falls back to the `\\wsl.localhost\...` path, so
the folder still opens and saves.
@id
Folder di dalam **distro WSL2** sekarang membaca dan menulis lewat helper kecil
yang berjalan **di dalam distro**, bukan lewat share 9p Windows. Membuka folder,
menelusuri tree, membuka dan menyimpan file, find-in-files, dan git semuanya lewat
helper itu, jadi home folder dengan 70.000 entri cache tidak lagi butuh beberapa
detik untuk didaftar. Fitur ini mati secara bawaan: nyalakan di **Settings > WSL**.

Helper-nya adalah FluxCode sendiri, dibangun untuk Linux, dari rilis build yang
sedang berjalan: keduanya selalu versi yang sama dan tidak ada yang perlu diunduh
terpisah. Ia dipasang sekali per build di `~/.fluxcode/helper/<build>/`. Kalau
helper gagal dipasang atau dijalankan, semuanya kembali ke path
`\\wsl.localhost\...`, jadi folder tetap terbuka dan tersimpan.

@release 2026-11-13
@title en: WSL2 workspaces
@title id: Workspace WSL2
@en
A folder that lives inside a **WSL2 distro** opens like any other. Files are read and
written through `\\wsl.localhost\...`, while the **terminal**, **Run** and **git** run
inside the distro, so the paths, line endings and toolchains are the Linux ones. The
status bar says `WSL: Ubuntu`, and **File > Open Folder in WSL...** lists the distros
that are installed.

What it does not do yet: no language server (FluxCode has none), no interactive terminal
(the terminal runs one line at a time), and no file watcher (there is none). A folder
under `/mnt/c` works but is slow, so keep the repo in the Linux filesystem.
@id
Folder yang ada di dalam **distro WSL2** terbuka seperti folder biasa. File dibaca dan
ditulis lewat `\\wsl.localhost\...`, sementara **terminal**, **Run**, dan **git** berjalan
di dalam distro, jadi path, line ending, dan toolchain yang dipakai adalah milik Linux.
Status bar menunjukkan `WSL: Ubuntu`, dan **File > Buka Folder di WSL...** menampilkan
distro yang terpasang.

Yang belum: belum ada language server (FluxCode memang belum punya), terminal belum
interaktif (terminal menjalankan satu baris sekaligus), dan belum ada file watcher
(memang belum ada). Folder di `/mnt/c` bisa dibuka tapi lambat, jadi simpan repo di
filesystem Linux.

@release 2026-11-13
@title en: Big files no longer freeze the window
@title id: File besar tidak lagi membekukan jendela
@en
Opening or saving a file over 256 KB now happens on a background thread, so the window
keeps drawing while a large file, a network share or a slow disk is read. Small files
still open the moment you click them, and a save that finishes late no longer hides
newer unsaved text.
@id
Membuka atau menyimpan file di atas 256 KB sekarang berjalan di thread latar, jadi
jendela tetap menggambar saat file besar, folder jaringan, atau disk lambat dibaca.
File kecil tetap terbuka begitu diklik, dan simpan yang selesai terlambat tidak lagi
menyembunyikan teks baru yang belum disimpan.

@release 2026-11-13
@title en: Markdown files draw themselves
@title id: File Markdown menggambar dirinya sendiri
@en
A **Markdown** file (README.md first of all) opens as text with a small bar above it:
**Preview** draws it the way GitHub shows it (headings, tables, lists, local images,
GitHub's `> [!NOTE]` callouts), **Source** goes back to editing. The editor keeps the
real text, so edits land in the drawing when you switch back. Flux documents keep their
own Preview and Source, unchanged.
@id
File **Markdown** (README.md yang pertama) terbuka sebagai teks dengan satu bar kecil di
atasnya: **Preview** menggambarnya seperti yang GitHub tampilkan (judul, tabel, daftar,
gambar lokal, callout `> [!NOTE]` milik GitHub), **Source** kembali ke penyuntingan.
Editornya tetap memegang teks aslinya, jadi editanmu muncul di gambar saat berpindah
kembali. Dokumen Flux tetap punya Preview dan Source sendiri, tidak berubah.

@release 2026-11-13
@title en: Two new color themes, FluxMuara and FluxMuara Dark
@title id: Dua tema warna baru, FluxMuara dan FluxMuara Dark
@en
**FluxMuara** joins the color themes, with a dark companion. The light one is the Muara
AI brand palette: a mist water canvas, white surfaces, riverbed ink text and one river
current blue accent (`#0369A1`). **FluxMuara Dark** is the same river at night: riverbed
ink surfaces with the accent lifted so it reads on the dark canvas. Pick either from
the theme menu in the status bar, cycle to them with `Ctrl+K Ctrl+T`, or choose their
cards in Settings. Every text on both passes the contrast the other themes are held
to, and a test now keeps every theme to that bar.
@id
**FluxMuara** bergabung ke tema warna, bersama versi gelapnya. Yang terang adalah palet
merek Muara AI: kanvas air berkabut, permukaan putih, teks tinta dasar sungai, dan satu
aksen biru arus sungai (`#0369A1`). **FluxMuara Dark** adalah sungai yang sama di malam
hari: permukaan tinta dasar sungai dengan aksen dinaikkan agar terbaca di kanvas gelap.
Pilih salah satu dari menu tema di status bar, siklus dengan `Ctrl+K Ctrl+T`, atau
kartunya di Settings. Setiap teks di keduanya lolos kontras yang dipegang tema lain,
dan sebuah test sekarang menjaga setiap tema di standar itu.
@release 2026-11-11
@title en: Our own Live Share relay, and a README in two languages
@title id: Relay Live Share sendiri, dan README dalam dua bahasa
@en
**Live Share** now goes through MuaraAI's own relay at `live.muaraai.com` before it falls
back to the free public one. The panel says which is in use: **Relay (MuaraAI)** when it is
ours, **Relay (public)** when the fallback carries the session. Ours is faster and has a
bandwidth ceiling, so it stops rather than runs up a bill.

Shared sessions now refuse to send what should never leave a PC: environment files,
private keys, `credentials*`, `.envrc`, and build folders like `node_modules`, `target`
and `.git`, even when a guest asks for one by name on Windows paths that try to escape the
shared folder.

The **README** and the Flux guide come in English and Indonesian (`README.id.md`,
`docs/flux.id.md`), with English first because that is what a first-time reader needs.
The README link at the top switches between them.
@id
**Live Share** sekarang lewat relay milik MuaraAI di `live.muaraai.com` sebelum beralih
ke relay publik gratis. Panelnya menunjukkan mana yang dipakai: **Relay MuaraAI** saat
memakai milik kita, **Relay publik** saat cadangannya yang membawa sesi. Yang punya kita
lebih cepat dan punya batas bandwidth, jadi ia berhenti alih-alih menagih.

Sesi berbagi sekarang menolak mengirim hal yang tidak boleh keluar dari PC: file
environment, kunci privat, `credentials*`, `.envrc`, dan folder build seperti
`node_modules`, `target`, serta `.git`, bahkan saat tamu memintanya dengan nama yang tepat
lewat path Windows yang mencoba keluar dari folder berbagi.

**README** dan panduan Flux tersedia dalam bahasa Inggris dan Indonesia (`README.id.md`,
`docs/flux.id.md`), dengan bahasa Inggris lebih dulu karena itu yang dibutuhkan pembaca
pertama kali. Tautan di atas README berpindah antara keduanya.

@release 2026-11-11
@title en: The agent asks before it acts
@title id: Agent bertanya sebelum bertindak
@en
Before the agent **runs a command** or changes a file, a card in the chat shows exactly what it
wants to do and it **waits for your answer**. The taskbar button flashes while FluxCode is in
the background. **Full access** is one switch in the agent's new **settings dialog** (the gear
in the panel's title row).

The message box fits a **narrow panel**, the model list **lines up** in columns, and free
models say so under Usage. Text in the **terminal can be selected and copied**, and clicking a
menu over the chat no longer selects its text.
@id
Sebelum agent **menjalankan perintah** atau mengubah file, kartu di chat menunjukkan persis apa
yang mau dilakukan dan agent **menunggu jawabanmu**. Tombol taskbar berkedip saat FluxCode di
belakang. **Akses penuh** cukup satu switch di **dialog pengaturan** agent yang baru (gear di
baris judul panel).

Kotak pesan muat di **panel sempit**, daftar model **rapi** dalam kolom, dan model gratis
ditandai di Usage. Teks di **terminal bisa diblok dan disalin**, dan klik menu di atas chat
tidak lagi memblok teksnya.

@release 2026-11-10
@title en: A Windows installer and a macOS build
@title id: Installer Windows dan build macOS
@en
Windows releases have an installer: `FluxCode-<build>-windows-x64-setup.exe`. It
installs for your account without admin rights in `%USERPROFILE%\.fluxcode\app`
(where FluxCode updates itself), adds Start Menu and Desktop shortcuts and an
entry in Settings > Apps with a working Uninstall, and offers to open FluxCode
when it is done. The ZIP stays for a portable copy (and for the built-in
updater).

FluxCode runs on macOS 11 and newer: `FluxCode-<build>-macos-universal.dmg`, one
app for Apple silicon and Intel. It updates itself like the other builds. It is
signed ad hoc, not notarized, so the first open asks once (Privacy & Security >
Open Anyway).
@id
Rilis Windows punya installer: `FluxCode-<build>-windows-x64-setup.exe`. Ia
terpasang untuk akunmu tanpa hak admin di `%USERPROFILE%\.fluxcode\app` (tempat
FluxCode memperbarui dirinya), menambah shortcut Start Menu dan Desktop dan entri
di Settings > Apps dengan Uninstall yang berfungsi, dan menawarkan membuka
FluxCode setelah selesai. ZIP tetap ada untuk salinan portable (dan untuk updater
bawaan).

FluxCode berjalan di macOS 11 ke atas: `FluxCode-<build>-macos-universal.dmg`,
satu app untuk Apple silicon dan Intel. Ia memperbarui diri seperti build lain.
Ia ditandatangani secara ad hoc, bukan notarized, jadi pembukaan pertama bertanya
sekali (Privacy & Security > Open Anyway).

@release 2026-11-10
@title en: The terminal reads like a normal terminal
@title id: Terminal terbaca seperti terminal biasa
@en
The Terminal tab has no separate input bar any more: the prompt (`folder ❯ `)
is the line right after the output, like Windows Terminal or VS Code. A click
anywhere in the terminal puts the caret at the prompt (links to errors still open
the file). Each command stays in the scrollback as the prompt it was typed at,
so it is clear where one command starts.

The folder in the prompt is cut from the left (`...\Project\app`, the whole path
on hover), only the folder's name on a narrow panel. The prompt is never cut off
at the bottom: the log shrinks above it, and a resized panel stays scrolled to
the newest lines.
@id
Tab Terminal tidak lagi punya bar input terpisah: prompt (`folder ❯ `) adalah
baris tepat setelah output, seperti Windows Terminal atau VS Code. Klik di mana
saja di terminal menaruh caret di prompt (tautan ke error tetap membuka file).
Setiap perintah tetap di scrollback sebagai prompt tempat ia diketik, jadi jelas
di mana satu perintah mulai.

Folder di prompt dipotong dari kiri (`...\Project\app`, path lengkap saat hover),
hanya nama folder di panel sempit. Prompt tidak pernah terpotong di bawah: log
menyusut di atasnya, dan panel yang diubah ukurannya tetap menggulir ke baris
terbaru.

@release 2026-11-10
@title en: The taskbar shows the new icon; lighter debug builds
@title id: Taskbar menampilkan ikon baru; build debug lebih ringan
@en
After an update or a reinstall, Windows kept showing the old blue icon on the
taskbar and in Start, because the shell caches a shortcut's icon by the exe's
path. The updater now tells the shell the exe changed (and again on the first
start after an update), and the installer refreshes the icon cache.

Debug builds keep line numbers for FluxCode's own code and no debug info for
the dependencies: the local build cache went from 48 GB to 2.5 GB. Release
builds are unchanged.
@id
Setelah update atau install ulang, Windows terus menampilkan ikon biru lama di
taskbar dan Start, karena shell menyimpan cache ikon shortcut berdasarkan path
exe. Updater sekarang memberi tahu shell bahwa exe berubah (dan lagi saat start
pertama setelah update), dan installer menyegarkan cache ikon.

Build debug menyimpan nomor baris untuk kode FluxCode sendiri dan tanpa info debug
untuk dependensi: cache build lokal menyusut dari 48 GB menjadi 2,5 GB. Build
rilis tidak berubah.

@release 2026-11-10
@title en: A calmer chat, an effort slider you can drag
@title id: Chat lebih tenang, slider effort yang bisa digeser
@en
Scroll up while the agent answers and the chat **stays where you put it**; scroll back down
and it follows again. Opened thoughts stay on the timeline, and every step's time lines up.

**Drag** the effort slider, see each model's **token limits**, and spot **free** models at a
glance. Attached images show a **preview**, and a long row of attachments scrolls sideways.
@id
Scroll ke atas saat agent menjawab dan chat **tetap di posisimu**; scroll ke bawah lagi dan
chat kembali mengikuti. Thought yang dibuka tetap di garis langkah, dan waktu tiap langkah
sejajar.

**Geser** slider effort, lihat **batas token** tiap model, dan kenali model **gratis** sekilas.
Gambar yang dilampirkan punya **preview**, dan deretan lampiran yang panjang bisa digeser ke
samping.

@release 2026-11-10
@title en: Watch the agent work, step by step
@title id: Lihat agent bekerja, langkah demi langkah
@en
What the agent does now reads as one line of steps: **thinking**, **reading** a file,
**editing**, **running** a command, **searching**. The step in progress moves; when it ends it
says what it did and how long it took.

The **Run** view, the **Output** panel and **Live share** are redesigned: one main button per
project, clearer build results, and two simple choices to host or join a session.
@id
Apa yang dikerjakan agent sekarang terbaca sebagai satu garis langkah: **berpikir**,
**membaca** file, **mengedit**, **menjalankan** perintah, **mencari**. Langkah yang sedang
berjalan bergerak; begitu selesai, ia menyebut apa yang dikerjakan dan berapa lama.

View **Run**, panel **Output** dan **Live share** didesain ulang: satu tombol utama per proyek,
hasil build yang lebih jelas, dan dua pilihan sederhana untuk host atau gabung ke sesi.

@release 2026-11-10
@title en: See your usage, and pick effort with a slider
@title id: Lihat pemakaianmu, dan atur effort dengan slider
@en
The agent panel shows how full the chat's context is under the message box. Click it (or
the gauge button) to see your plan's limits, when they reset, and what this session cost,
without typing `/usage`.

Effort is now a **slider** that starts at **medium**, and Claude's models are grouped by
generation with their price, newest first. When the agent searches the web, you see what it
searched and the sources it found.
@id
Panel agent menampilkan seberapa penuh konteks chat di bawah kotak pesan. Klik (atau tombol
gauge) untuk melihat limit paketmu, kapan reset, dan biaya sesi ini, tanpa mengetik `/usage`.

Effort sekarang berupa **slider** yang mulai dari **medium**, dan model Claude dikelompokkan
per generasi beserta harganya, dari yang terbaru. Saat agent mencari di web, kamu bisa melihat
apa yang dicari dan sumber yang ditemukan.

@release 2026-11-10
@title en: Extensions, and Minecraft plugins built in one click
@title id: Ekstensi, dan plugin Minecraft di-build sekali klik
@en
The new **Extensions** view (**Ctrl+Shift+X**) adds what your projects need: Paper
plugins, Bedrock add-ons, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ and .NET. Install
one and the **Run** view gets its buttons: Build, Build & deploy, Start server, Pack, and
more. Paper plugins build with the Java version the project asks for, deploy under their own
name, and replace their older copy in the server's plugins folder.

The agent list has **OpenCode 2**, and the theme name in the status bar now opens the themes.
@id
View **Ekstensi** yang baru (**Ctrl+Shift+X**) menambahkan apa yang dibutuhkan proyekmu:
plugin Paper, add-on Bedrock, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ dan .NET.
Pasang satu, dan view **Run** mendapat tombolnya: Build, Build & deploy, Nyalakan server,
Pack, dan lainnya. Plugin Paper di-build dengan versi Java yang diminta proyeknya, dideploy
dengan namanya sendiri, dan menggantikan salinan lamanya di folder plugins server.

Daftar agent sekarang punya **OpenCode 2**, dan nama tema di status bar bisa diklik untuk memilih tema.

@release 2026-11-10
@title en: See new commits before you pull
@title id: Lihat commit baru sebelum pull
@en
**Source Control** now checks the remote every few minutes. When it has commits you
don't, an **Incoming** section lists them: open one to see the files it changed, and a file
to see its diff. **Pull** brings them in. A blue dot on the Source Control icon tells you
there is something new.

The update notice is smaller, and if you close it, an **Update** button waits in the title bar.
This page now shows the **build** each change came in.
@id
**Source Control** sekarang mengecek remote setiap beberapa menit. Kalau ada commit yang
belum kamu punya, bagian **Masuk** menampilkannya: buka satu commit untuk melihat file yang
diubah, lalu buka file untuk melihat diff-nya. **Pull** menariknya. Titik biru di ikon Source
Control menandakan ada yang baru.

Notifikasi update lebih kecil, dan kalau ditutup, tombol **Update** tetap ada di title bar.
Halaman ini sekarang menampilkan **build** tempat setiap perubahan masuk.

@release 2026-11-10
@title en: Agents can use a browser
@title id: Agent bisa memakai browser
@en
Agents can now **open web pages** in the Chrome, Brave or Edge on your PC: click, type,
read the console and network, and take screenshots. It uses its own profile, so your
cookies and passwords stay yours. Choose the browser in **Agent settings > Browser**.

The message box is tidier: a context chip you close goes away (a **+** brings it back),
and the chips stay on one line.
@id
Agent sekarang bisa **membuka halaman web** di Chrome, Brave atau Edge di PC-mu: klik,
mengetik, membaca console dan jaringan, serta mengambil screenshot. Browser memakai profil
tersendiri, jadi cookie dan password-mu tetap aman. Pilih browsernya di **Pengaturan agent >
Browser**.

Kotak pesan lebih rapi: chip konteks yang ditutup langsung hilang (tombol **+**
mengembalikannya), dan chip tetap dalam satu baris.

@release 2026-11-10
@title en: Flux documents open drawn
@title id: Dokumen Flux terbuka langsung ter-render
@en
Open a `.flux` file and it shows **drawn**: images with a size and a dark-theme version,
images side by side, videos, **flowcharts**, callouts, collapsible sections, columns and
a table of contents. **Source** shows the text; **Save as Markdown** writes a copy GitHub
can show.

Long documents stay light: only what is on screen is drawn, so a million lines with a
200,000-row table opens in a third of a second.
@id
Buka file `.flux` dan isinya langsung tampil **ter-render**: gambar dengan ukuran dan versi
tema gelap, gambar berdampingan, video, **flowchart**, callout, bagian yang bisa dilipat,
kolom, dan daftar isi. **Sumber** menampilkan teksnya; **Simpan sebagai Markdown** menulis
salinan yang bisa ditampilkan GitHub.

Dokumen panjang tetap ringan: hanya yang tampil di layar yang digambar, jadi satu juta baris
dengan tabel 200.000 baris terbuka dalam sepertiga detik.

@release 2026-11-10
@title en: Live share has its own page
@title id: Live share punya halaman sendiri
@en
The people icon now opens **Live share** as a full page in the editor, with room for the
invite code, every way in, the people in the session and their edits. On a wide window
the cards sit side by side; on a narrow one they stack.
@id
Ikon orang sekarang membuka **Live share** sebagai halaman penuh di editor, cukup luas untuk
kode undangan, semua jalur masuk, orang-orang di sesi dan edit mereka. Di jendela lebar
kartunya berdampingan; di jendela sempit bertumpuk.

@release 2026-11-10
@title en: Tidier agent menus, and models for GitHub Copilot
@title id: Menu agent lebih rapi, dan model untuk GitHub Copilot
@en
The agent, model, mode and permission menus now share one look and stay inside the
agent panel: the agent list opens under its title, the others right above the message
box. Long lists have a **search box**, and the arrow keys, Enter and Esc work.

- The agent list says which agent is **On** and which are **Off**, one line each.
- **GitHub Copilot** now has a model menu: pick one and Copilot restarts with it, in the
  same chat.
@id
Menu agent, model, mode dan izin sekarang tampil seragam dan tetap di dalam panel agent:
daftar agent terbuka di bawah judulnya, yang lain tepat di atas kotak pesan. Daftar yang
panjang punya **kotak pencarian**, dan tombol panah, Enter serta Esc bisa dipakai.

- Daftar agent menunjukkan agent mana yang **Nyala** dan mana yang **Mati**, satu baris per agent.
- **GitHub Copilot** sekarang punya menu model: pilih satu dan Copilot dimulai ulang
  dengan model itu, di chat yang sama.

@release 2026-11-10
@title en: Live share works over the internet now
@title id: Live share sekarang bisa lewat internet
@en
Friends outside your network can join your session, with no VPN and no server to rent.
When you click **Start hosting**, FluxCode opens every way in it can and shows each one in
the panel:

- **Same network**: your Wi-Fi or LAN.
- **Router port (UPnP)**: asks your router to open the port, like a game server does.
- **Public IPv6**: when your provider gives your PC one.
- **Relay (bore.pub)**: a free public tunnel that works behind any router. It only passes
  bytes already encrypted with the invite code's secret.

The invite code holds every address that opened, and the guest's FluxCode tries them all
at once. Untick **Let people join over the internet** to keep a session on your network.
@id
Teman di luar jaringanmu bisa ikut sesi, tanpa VPN dan tanpa sewa server. Saat kamu klik
**Mulai jadi host**, FluxCode membuka semua jalur masuk yang bisa dibuka dan menampilkan
masing-masing di panel:

- **Jaringan yang sama**: Wi-Fi atau LAN-mu.
- **Port router (UPnP)**: meminta router membuka port, seperti server game.
- **IPv6 publik**: kalau provider memberi PC-mu alamat IPv6.
- **Relay (bore.pub)**: tunnel publik gratis yang tembus router apa pun. Relay hanya
  meneruskan data yang sudah terenkripsi dengan rahasia di kode undangan.

Kode undangan memuat semua alamat yang terbuka, dan FluxCode tamu mencoba semuanya sekaligus.
Hapus centang **Izinkan bergabung lewat internet** supaya sesi hanya di jaringanmu.

@release 2026-11-10
@title en: What's new before you update
@title id: Lihat yang baru sebelum update
@en
**What's new** on the update card now opens here: every build between yours and the new
one, what each changed, and *Update and restart* on the same page.
@id
**Yang baru** di kartu update sekarang terbuka di sini: semua build antara punyamu dan yang
baru, apa yang berubah di tiap build, dan *Perbarui dan buka ulang* di halaman yang sama.

@release 2026-11-10
@title en: Live share: code together, with your PC as the server
@title id: Live share: ngoding bareng, PC kamu jadi servernya
@en
Open **Live share** (the people icon in the activity bar), click **Start hosting**, and
send the invite code. Whoever pastes it joins your folder and you type in the same files
at the same time; edits merge so everyone ends with the same text.

- The panel shows who is in the session and the file and line each person is on.
- **Activity** lists every person's edits with a diff. An edit made by an AI agent says
  so, with the agent, model, effort and mode, so you can tell which model wrote what.
- Edits go straight between your PC and theirs, encrypted with the code's secret. Over the
  internet, forward TCP port 47615 on your router to your PC.
@id
Buka **Live share** (ikon orang di activity bar), klik **Mulai jadi host**, lalu kirim
kode undangannya. Siapa pun yang menempelkan kode itu masuk ke folder kamu dan kalian
mengetik di file yang sama secara bersamaan; edit digabung sehingga teksnya sama di semua
PC.

- Panelnya menampilkan siapa saja di sesi dan file serta baris yang sedang dibuka
  masing-masing.
- **Aktivitas** memuat edit tiap orang beserta diff-nya. Edit dari agent AI ditandai,
  lengkap dengan agent, model, effort dan mode-nya, jadi kelihatan model mana yang menulis
  apa.
- Edit dikirim langsung antara PC kamu dan PC mereka, terenkripsi dengan rahasia di
  kodenya. Lewat internet, teruskan port TCP 47615 di router ke PC kamu.

@release 2026-11-10
@title en: Less memory: 72 MB reserved, about 1 MB in Task Manager when idle
@title id: Memori lebih kecil: 72 MB dipesan, sekitar 1 MB di Task Manager saat idle
@en
FluxCode now **reserves 72 MB instead of 121 MB**, and once idle Task Manager shows about
**1 MB** (about 5 MB after a click). On the same PC Zed 1.27 reserves 153 MB and shows 69 MB.

- The window-sized textures GPUI keeps for drawing curved shapes (one of them 4x
  multisampled) are now made only while something curved is on screen, and the window
  keeps two frame buffers instead of three.
- 6 s after start, and after 15 s in the background, FluxCode hands the memory it isn't
  using back to Windows. It comes back, a few milliseconds' worth, when needed.
@id
FluxCode sekarang **memesan 72 MB, bukan 121 MB**, dan saat idle Task Manager menunjukkan
sekitar **1 MB** (sekitar 5 MB setelah satu klik). Di PC yang sama Zed 1.27 memesan 153 MB
dan menunjukkan 69 MB.

- Tekstur seukuran jendela yang disiapkan GPUI untuk menggambar bentuk lengkung (salah
  satunya 4x multisample) sekarang hanya dibuat saat ada bentuk lengkung di layar, dan
  jendela memakai dua frame buffer, bukan tiga.
- 6 detik setelah dibuka, dan setelah 15 detik di belakang jendela lain, FluxCode
  mengembalikan memori yang tidak dipakai ke Windows. Memorinya kembali saat dibutuhkan,
  hanya beberapa milidetik.

@release 2026-11-10
@title en: A calmer look, any ACP agent in one click, agents that ask, and VS Code's shortcuts
@title id: Tampilan lebih tenang, semua agent ACP sekali klik, agent yang bisa bertanya, dan shortcut VS Code
@en
### A new icon and calmer colors

The icon is a **meander**: one line that only turns at right angles and never crosses
itself, split into two hooks that hold each other. It is named after the river Maeander;
one of the oldest known is carved in mammoth ivory from Mezin, Ukraine. README > The icon
tells the whole story.

FluxDark is now ink and ivory with one muted ochre accent, instead of crimson on every
button. FluxCream uses umber on paper, and FluxRed became a quieter oxblood. Every text
color still passes WCAG AA (4.5:1).

### Agents: the whole ACP registry

- The agent menu lists every agent in the [ACP registry](https://agentclientprotocol.com):
  Claude Agent, Codex, Gemini CLI, GitHub Copilot, Cursor, Qwen Code, goose and more,
  each with **its own icon** from the registry.
- An agent that is not on your PC has a **download button**. FluxCode installs it into
  `~/.fluxcode/agents`, checks the registry's SHA-256, and starts it only while you use
  the panel. npm agents stay out of your global npm.
- Installing shows a **liquid ring** that fills with the real byte count, then a **box
  that opens** as each file is unpacked. A failed install says why, in plain words, with
  *Try again*.
- **Model, mode and permissions** are dropdowns under the message box. Effort sits in the
  model menu.

### VS Code's shortcuts

| Shortcut | Does |
| --- | --- |
| `Ctrl+Shift+P` / `F1` | Command Palette, every command with its keys |
| `Ctrl+G` | Go to Line (`:12` or `:12:5` in the box) |
| `Ctrl+/` | Toggle line comment |
| `Alt+Up` / `Alt+Down` | Move line |
| `Shift+Alt+Up` / `Down` | Copy line |
| `Ctrl+Shift+K` | Delete line |
| `Ctrl+Enter` / `Ctrl+Shift+Enter` | New line below / above |
| `Ctrl+Shift+T` | Reopen the closed tab |
| `Ctrl+K Ctrl+W` | Close all tabs |
| `Alt+Z` | Word wrap |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Editor font bigger / smaller / reset |
| `Ctrl+PageUp` / `PageDown` | Previous / next tab |

### The agent can ask you

When an agent needs a decision, it asks in a card above the message box: options
lettered **A, B, C, D**, each with its explanation, and **Type my own answer** for
anything else. One question with one choice answers on a click. This is the ACP
*elicitation* request, the same one Claude Agent and OpenCode use for their questions.

- The welcome screen lists the running agent's **skills and commands**; a click puts
  `/name` in the message box.
- Removing an agent asks you to **type its name twice**, so the wrong one can't go.
- Settings > **Agent instructions (soul files)** lists the files each installed agent
  reads for its standing instructions (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md` and so on),
  with Open, or Create when one is missing.
- The agent is told which language the interface is in, so it answers in Indonesian when
  FluxCode is in Indonesian and the message doesn't say otherwise.

### More settings

Editor font, line numbers, code folding, auto-closing brackets, smart indent, cursor
padding and scrolling past the last line are now in Settings > Editor, and Settings >
Keyboard shortcuts lists every command with its keys.

### Start-up, measured

On the PC this was built on, FluxCode shows its window about **0.2 s** after launch and
uses about 95 MB of memory 5 s later, in one process. About 25 ms of that start is
FluxCode's own work; the rest is Windows and GPU set-up. Finding installed agents runs on
a separate thread while the window opens.
@id
### Ikon baru dan warna yang lebih tenang

Ikonnya sebuah **meander**: satu garis yang hanya berbelok siku dan tidak pernah memotong
dirinya sendiri, dibelah jadi dua kait yang saling mengunci. Namanya dari sungai Maeander;
salah satu yang tertua terukir di gading mammoth dari Mezin, Ukraina. Ceritanya lengkap
di README > The icon.

FluxDark sekarang tinta dan gading dengan satu aksen oker yang redup, tidak lagi merah
crimson di setiap tombol. FluxCream memakai cokelat tua di atas kertas, dan FluxRed jadi
oxblood yang lebih kalem. Semua warna teks tetap lolos WCAG AA (4.5:1).

### Agent: seluruh registry ACP

- Menu agent memuat semua agent di [registry ACP](https://agentclientprotocol.com):
  Claude Agent, Codex, Gemini CLI, GitHub Copilot, Cursor, Qwen Code, goose dan lainnya,
  masing-masing dengan **ikonnya sendiri** dari registry.
- Agent yang belum ada di PC punya **tombol unduh**. FluxCode memasangnya ke
  `~/.fluxcode/agents`, mengecek SHA-256 dari registry, dan hanya menjalankannya saat panel
  dipakai. Agent npm tidak masuk ke npm global kamu.
- Saat memasang, muncul **cincin cairan** yang terisi sesuai byte yang benar-benar
  terunduh, lalu **kotak yang terbuka** saat tiap file dibongkar. Kalau gagal, alasannya
  ditulis jelas, dengan tombol *Coba lagi*.
- **Model, mode dan izin** jadi dropdown di bawah kotak pesan. Effort ada di menu model.

### Shortcut VS Code

| Shortcut | Fungsi |
| --- | --- |
| `Ctrl+Shift+P` / `F1` | Command Palette, semua perintah beserta tombolnya |
| `Ctrl+G` | Ke baris tertentu (`:12` atau `:12:5` di kotaknya) |
| `Ctrl+/` | Komentar baris |
| `Alt+Up` / `Alt+Down` | Pindahkan baris |
| `Shift+Alt+Up` / `Down` | Salin baris |
| `Ctrl+Shift+K` | Hapus baris |
| `Ctrl+Enter` / `Ctrl+Shift+Enter` | Baris baru di bawah / di atas |
| `Ctrl+Shift+T` | Buka lagi tab yang ditutup |
| `Ctrl+K Ctrl+W` | Tutup semua tab |
| `Alt+Z` | Word wrap |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Font editor lebih besar / kecil / reset |
| `Ctrl+PageUp` / `PageDown` | Tab sebelumnya / berikutnya |

### Agent bisa bertanya ke kamu

Saat agent butuh keputusan, dia bertanya lewat kartu di atas kotak pesan: pilihan
berhuruf **A, B, C, D**, masing-masing dengan penjelasannya, dan **Ketik jawaban sendiri**
untuk yang lain. Satu pertanyaan dengan satu pilihan langsung terjawab sekali klik. Ini
permintaan *elicitation* dari ACP, sama dengan yang dipakai Claude Agent dan OpenCode.

- Layar sambutan memuat **skill dan perintah** dari agent yang sedang jalan; klik untuk
  menaruh `/nama` di kotak pesan.
- Menghapus agent memintamu **mengetik namanya dua kali**, supaya tidak salah hapus.
- Pengaturan > **Instruksi agent (file soul)** memuat file yang dibaca tiap agent untuk
  instruksi tetapnya (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md` dan lainnya), dengan tombol
  Buka, atau Buat kalau belum ada.
- Agent diberi tahu bahasa antarmuka, jadi dia menjawab dalam Bahasa Indonesia saat
  FluxCode memakai Bahasa Indonesia dan pesannya tidak meminta bahasa lain.

### Pengaturan tambahan

Font editor, nomor baris, code folding, tutup kurung otomatis, indentasi pintar, jarak
kursor dan scroll melewati baris terakhir sekarang ada di Pengaturan > Editor, dan
Pengaturan > Shortcut keyboard memuat semua perintah beserta tombolnya.

### Waktu buka, diukur

Di PC tempat build ini dibuat, jendela FluxCode muncul sekitar **0,2 detik** setelah
dibuka dan memakai sekitar 95 MB memori 5 detik kemudian, dalam satu proses. Sekitar
25 ms dari waktu itu adalah kerja FluxCode sendiri; sisanya persiapan Windows dan GPU.
Pencarian agent yang terpasang berjalan di thread terpisah selama jendela dibuka.

@release 2026-11-09
@title en: Update card polish
@title id: Kartu update lebih rapi
@en
The update card names the new build on its own line and the installed one below
it. Its notes show the first sentence of each point instead of cutting lines
mid-sentence.
@id
Kartu update menuliskan nama build baru di barisnya sendiri dan yang terpasang
di bawahnya. Catatannya menampilkan kalimat pertama tiap poin, bukan memotong
baris di tengah kalimat.

@release 2026-11-09
@title en: About section, title bar fix and the first public build
@title id: Bagian About, perbaikan title bar, dan build publik pertama
@en
- FluxCode is public and builds for **Windows x64 and Linux x64** on every change.
- A card offers each new build; one click downloads it, swaps the program and restarts.
- Settings > About FluxCode links to the repository, the issue form and the changelog.
- The title bar search box no longer covers the Help menu in Indonesian.
@id
- FluxCode sekarang publik dan di-build untuk **Windows x64 dan Linux x64** di setiap
  perubahan.
- Kartu update menawarkan build baru; sekali klik mengunduh, mengganti program dan
  membuka ulang.
- Pengaturan > Tentang FluxCode berisi tautan ke repository, form issue dan changelog.
- Kotak pencarian di title bar tidak lagi menutupi menu Bantuan dalam Bahasa Indonesia.

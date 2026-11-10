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

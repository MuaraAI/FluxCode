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

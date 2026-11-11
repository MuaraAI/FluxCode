// docs/flux.flux: the Flux document format, written in Flux.
//
// Two files are made from it, one per language:
//   fluxcode --flux-to-md docs/flux.flux docs/flux.md    --lang en
//   fluxcode --flux-to-md docs/flux.flux docs/flux.id.md --lang id
//
// Every section is written twice, `@en` then `@id`. Blocks with no tag show in both
// languages: code samples and images.

@title Flux documents
@style accent=#d0a070

@en
A `.flux` file is a text document: everything Markdown writes, plus blocks for what
Markdown hands to HTML or doesn't have at all. FluxCode opens it drawn, and turns it into
GitHub Markdown when it has to be read on GitHub (FluxCode's own `README.md` is made
from `README.flux` this way).
@id
File `.flux` adalah dokumen teks: semua yang ditulis Markdown, ditambah blok untuk hal
yang diserahkan Markdown ke HTML atau memang tidak ada padanannya. FluxCode membukanya
dalam bentuk tergambar, dan mengubahnya jadi Markdown GitHub saat harus dibaca di GitHub
(`README.md` milik FluxCode sendiri dibuat dari `README.flux` dengan cara ini).

@all
@toc

@en
## Why another format

Markdown is good at paragraphs and lists. Everything else (an image's width, a picture
for dark themes, a collapsible section, a callout, a flowchart, two images side by side)
means writing HTML inside it, and HTML is harder to read, to write and to keep in shape,
for people and for AI models alike. Flux gives each of those a one-line directive.

| | Markdown | Flux |
| --- | --- | --- |
| Text, headings, lists, code, links | yes | the same syntax |
| Tables | pipe tables | pipe tables, or `@table` with alignment and column widths |
| Image width, alignment, dark-theme version | HTML | `@image ... width= align= dark=` |
| Images side by side | HTML | `@gallery` |
| Video | a link | `@video` with a poster; FluxCode plays it in the system's player |
| Flowcharts | Mermaid on GitHub only | `@flow`, drawn by FluxCode; Mermaid on GitHub |
| Callouts | GitHub's `> [!NOTE]` | `@note`, `@tip`, `@warn`, `@danger` |
| Collapsible sections | HTML `<details>` | `@details` |
| Columns | no | `@columns` ... `@col` ... `@end` |
| Table of contents | by hand | `@toc` |
| Keys, badges, colored text | HTML or no | `{kbd Ctrl+S}`, `{badge New}`, `{color=#d0a070 text}` |
| One file, several languages | no | `@en`, `@id`, `@all` sections |
| Look of the page | the viewer's | `@style accent= width= size= font=` |
@id
## Kenapa format lain

Markdown bagus untuk paragraf dan daftar. Sisanya (lebar gambar, gambar untuk tema gelap,
bagian yang bisa dilipat, callout, flowchart, dua gambar berdampingan) berarti menulis
HTML di dalamnya, dan HTML lebih sulit dibaca, ditulis, dan dijaga bentuknya, baik oleh
manusia maupun model AI. Flux memberi masing-masing hal itu satu direktif sebaris.

| | Markdown | Flux |
| --- | --- | --- |
| Teks, heading, daftar, kode, tautan | ada | sintaks yang sama |
| Tabel | tabel pipa | tabel pipa, atau `@table` dengan perataan dan lebar kolom |
| Lebar gambar, perataan, versi tema gelap | HTML | `@image ... width= align= dark=` |
| Gambar berdampingan | HTML | `@gallery` |
| Video | sebuah tautan | `@video` dengan poster; FluxCode memutarnya di player sistem |
| Flowchart | Mermaid hanya di GitHub | `@flow`, digambar FluxCode; Mermaid di GitHub |
| Callout | `> [!NOTE]` milik GitHub | `@note`, `@tip`, `@warn`, `@danger` |
| Bagian yang bisa dilipat | HTML `<details>` | `@details` |
| Kolom | tidak ada | `@columns` ... `@col` ... `@end` |
| Daftar isi | ditulis tangan | `@toc` |
| Tombol, badge, teks berwarna | HTML atau tidak ada | `{kbd Ctrl+S}`, `{badge New}`, `{color=#d0a070 text}` |
| Satu file, beberapa bahasa | tidak ada | bagian `@en`, `@id`, `@all` |
| Tampilan halaman | bawaan viewer | `@style accent= width= size= font=` |

@en
## How it stays light

- **One pass, nothing copied.** Each block keeps where its text is in the file (16 bytes a
  block); bold, links and keys are worked out only for the blocks being drawn.
- **Only what is on screen is drawn.** Every table row and list item is a block of its
  own, and the window holds the blocks in small groups (a heading starts one), laying out
  only the groups in view.
- **First screen at once.** A document over 256 KB shows its first 32 KB straight away;
  the rest is read on another thread and takes its place.
- **Large text kept packed.** Over 1 MB, the text is kept DEFLATE-packed in 64 KB frames,
  and drawing unpacks only the frames on screen (six stay unpacked).
- **No second copy.** The editor behind **Source** is filled the first time Source is
  shown, so a document over 2 MB opens without an editor holding its text too.

Measured with `cargo test --release --bin fluxcode million_lines -- --ignored --nocapture`
on an AMD Ryzen 5 7535HS, for a document of 1,300,003 lines (31.8 MB: headings, lists, a
200,000-row table, paragraphs):

| Step | Time | Memory |
| --- | ---: | ---: |
| First part (31 KB, 1,264 blocks), drawn at once | 0.083 ms | |
| The whole document read | 49 ms (647 MB/s) | 900,001 blocks, 14.4 MB |
| ... and its text packed | 102 ms in all, off screen | 31.8 MB of text kept in 2.3 MB |
| Bold, links and keys for one screen (40 blocks) | 0.04 ms | |
@id
## Kenapa tetap ringan

- **Satu kali lintas, tanpa salinan.** Tiap blok menyimpan letak teksnya di file (16 byte
  per blok); bold, tautan, dan tombol hanya dihitung untuk blok yang sedang digambar.
- **Hanya yang terlihat yang digambar.** Tiap baris tabel dan item daftar adalah blok
  tersendiri, dan jendela menyimpan blok dalam grup kecil (heading memulai satu grup),
  hanya menata grup yang ada di layar.
- **Layar pertama langsung tampil.** Dokumen di atas 256 KB menampilkan 32 KB pertamanya
  seketika; sisanya dibaca di thread lain lalu menggantikan tempatnya.
- **Teks besar disimpan padat.** Di atas 1 MB, teks disimpan terkompresi DEFLATE dalam
  frame 64 KB, dan penggambaran hanya membuka frame yang ada di layar (enam tetap terbuka).
- **Tanpa salinan kedua.** Editor di balik **Source** baru diisi saat Source pertama kali
  ditampilkan, jadi dokumen di atas 2 MB terbuka tanpa editor ikut menyimpan teksnya.

Diukur dengan `cargo test --release --bin fluxcode million_lines -- --ignored --nocapture`
di AMD Ryzen 5 7535HS, untuk dokumen 1.300.003 baris (31,8 MB: heading, daftar, tabel
200.000 baris, paragraf):

| Langkah | Waktu | Memori |
| --- | ---: | ---: |
| Bagian pertama (31 KB, 1.264 blok), langsung digambar | 0,083 ms | |
| Seluruh dokumen terbaca | 49 ms (647 MB/detik) | 900.001 blok, 14,4 MB |
| ... dan teksnya dipadatkan | total 102 ms, di luar layar | 31,8 MB teks tersimpan dalam 2,3 MB |
| Bold, tautan, dan tombol untuk satu layar (40 blok) | 0,04 ms | |

@en
Opened in FluxCode (release build, fresh profile, three runs each):

| Document | Window on screen | Task Manager, 9 s after launch | Memory reserved |
| --- | ---: | ---: | ---: |
| This page | 0.28 to 0.30 s | 5.0 MB | 47.6 MB |
| The 1,300,003-line document, with a `@toc` | 0.28 to 0.29 s | 16 MB | 81.5 MB |
| No document (for comparison) | 0.26 to 0.30 s | 6.3 to 6.9 MB | 51.3 to 51.9 MB |

@note What this does not promise
The packed text above is a repetitive test document; ordinary text packs about three to
four times. The source view is an ordinary editor, so opening Source on a very large
document holds the whole text again. "Memory reserved" is what the process has claimed
(Windows' private bytes); Task Manager shows the part of it in RAM.
@end
@id
Dibuka di FluxCode (build release, profil baru, tiga kali pengukuran):

| Dokumen | Jendela tampil | Task Manager, 9 detik setelah start | Memori direservasi |
| --- | ---: | ---: | ---: |
| Halaman ini | 0,28 sampai 0,30 detik | 5,0 MB | 47,6 MB |
| Dokumen 1.300.003 baris, dengan `@toc` | 0,28 sampai 0,29 detik | 16 MB | 81,5 MB |
| Tanpa dokumen (sebagai pembanding) | 0,26 sampai 0,30 detik | 6,3 sampai 6,9 MB | 51,3 sampai 51,9 MB |

@note Yang tidak dijanjikan di sini
Teks padat di atas adalah dokumen uji yang berulang; teks biasa memadat sekitar tiga
sampai empat kali. Tampilan Source adalah editor biasa, jadi membuka Source pada dokumen
sangat besar menyimpan seluruh teksnya lagi. "Memori direservasi" adalah yang sudah
diklaim proses (private bytes Windows); Task Manager menampilkan bagian yang ada di RAM.
@end

@en
## Text

Paragraphs, headings and lists are written as in Markdown.
@id
## Teks

Paragraf, heading, dan daftar ditulis seperti di Markdown.

```flux
# A heading
## A smaller one

A paragraph can run over several lines.
A blank line ends it.

- a list item
  - nested two spaces in
- [x] a done task
1. a numbered item

> A quote.

---
```

@en
Inline: **bold** `**bold**`, *italic* `*italic*`, `code` with backquotes, ~~struck~~
`~~struck~~`, ==highlighted== `==highlighted==`, [a link](https://github.com/MuaraAI/FluxCode)
`[a link](url)` (a bare `https://...` is a link too), a key {kbd Ctrl+Shift+P}
`{kbd Ctrl+Shift+P}`, a badge {badge New} `{badge New}` and {color=#5fd38d colored text}
`{color=#5fd38d colored text}`. A backslash keeps a character as it is: `\*`.

Lines starting with `//` are comments: they never show.
@id
Sebaris: **bold** `**bold**`, *miring* `*miring*`, `kode` dengan backtick, ~~coret~~
`~~coret~~`, ==sorot== `==sorot==`, [tautan](https://github.com/MuaraAI/FluxCode)
`[tautan](url)` (`https://...` telanjang juga jadi tautan), tombol {kbd Ctrl+Shift+P}
`{kbd Ctrl+Shift+P}`, badge {badge New} `{badge New}`, dan {color=#5fd38d teks berwarna}
`{color=#5fd38d teks berwarna}`. Backslash menjaga karakter tetap apa adanya: `\*`.

Baris yang dimulai dengan `//` adalah komentar: tidak pernah ditampilkan.

@en
## Tables

A pipe table works as on GitHub; the line under the header sets the alignment.
@id
## Tabel

Tabel pipa bekerja seperti di GitHub; baris di bawah header menentukan perataannya.

```flux
| Editor | Start | Memory |
| :--- | ---: | :---: |
| FluxCode | 0.22 s | 1.4 MB |
```

@en
`@table` needs no pipes at the ends and takes options:
@id
`@table` tidak butuh pipa di ujung baris dan menerima opsi:

```flux
@table align=l,r,r widths=3,1,1
Editor | Start | Memory
FluxCode | 0.22 s | 1.4 MB
Zed | 0.82 s | 66 MB
@end
```

@table align=l,r,r widths=3,1,1
Editor | Start | Memory
FluxCode | 0.22 s | 1.4 MB
Zed | 0.82 s | 66 MB
VS Code | 0.29 s | 929 MB
@end

@en
`align=` takes `l`, `c` or `r` per column, `widths=` relative widths, and
`header=none` makes the first row an ordinary one. A table can have any number of rows:
FluxCode draws only those on screen.
@id
`align=` menerima `l`, `c`, atau `r` per kolom, `widths=` lebar relatif, dan `header=none`
membuat baris pertama jadi baris biasa. Tabel boleh punya baris sebanyak apa pun:
FluxCode hanya menggambar yang ada di layar.

@en
## Images, GIFs and video
@id
## Gambar, GIF, dan video

```flux
@image assets/icon.png width=96 alt="The FluxCode icon"
@image chart-light.svg dark=chart-dark.svg width=100% alt="A chart"
@image demo.gif width=60% caption="A GIF plays as it is" link=demo.gif
@video intro.mp4 poster=intro.png alt="A one-minute tour"
```

@en
- `width=` in pixels or as a share of the page (`60%`); `align=right` floats it right.
- `dark=` is shown instead when the theme is dark (on GitHub too, through `<picture>`).
- `link=` is what a click opens; `caption=` goes under it.
- A GIF plays in FluxCode as it is. Images from the web aren't fetched: the document
  stays offline, and shows a link instead.
- `@video` shows the poster with a play button; a click plays the video in the system's
  player (FluxCode has no video decoder, which keeps it at 16 MB). On GitHub it is the
  poster linked to the video.
@id
- `width=` dalam piksel atau sebagai bagian halaman (`60%`); `align=right` membuatnya
  mengambang ke kanan.
- `dark=` ditampilkan sebagai gantinya saat temanya gelap (di GitHub juga, lewat
  `<picture>`).
- `link=` adalah yang terbuka saat diklik; `caption=` tampil di bawahnya.
- GIF diputar di FluxCode apa adanya. Gambar dari web tidak diambil: dokumen tetap
  offline, dan menampilkan tautan sebagai gantinya.
- `@video` menampilkan poster dengan tombol putar; satu klik memutarnya di player sistem
  (FluxCode tidak punya dekoder video, itulah yang menjaganya tetap 16 MB). Di GitHub
  bentuknya poster yang menautkan ke videonya.

@image ../assets/icon.png width=96 alt="The FluxCode icon: a square meander, one ivory hook and one ochre hook locked together, on a near-black tile."

@en
Two or more images side by side:
@id
Dua gambar atau lebih berdampingan:

```flux
@gallery
@image before.png width=49% alt="Before"
@image after.png width=49% alt="After"
@end
```

@en
## Flowcharts
@id
## Flowchart

```flux
@flow down
src([README.flux]) -> read[FluxCode reads it]
read -> view{Where?}
view -in FluxCode-> draw[Drawn, only what is on screen]
view -on GitHub-> md[README.md, flowcharts as Mermaid]
md ..> src
style src fill=#3a2f22 text=#f2e6d0 border=#d0a070
@end
```

@flow down
src([README.flux]) -> read[FluxCode reads it]
read -> view{Where?}
view -in FluxCode-> draw[Drawn, only what is on screen]
view -on GitHub-> md[README.md, flowcharts as Mermaid]
md ..> src
style src fill=#3a2f22 text=#f2e6d0 border=#d0a070
@end

@en
- Nodes: `id[Box]`, `id(Rounded)`, `id([Pill])`, `id{Decision}`, `id((Circle))`. A node
  is declared the first time it has a label; after that its id is enough.
- Edges: `a -> b`, with a label `a -yes-> b`, dashed `a ..> b`; they chain:
  `a -> b -> c`.
- `style id fill=#hex text=#hex border=#hex` colors one node.
- `@flow right` lays it out from left to right instead of top to bottom.
- FluxCode lays it out by itself in layers, and edges that go back up (loops) run along
  the side. On GitHub it becomes a Mermaid flowchart, which GitHub draws.
@id
- Simpul: `id[Kotak]`, `id(Bulat)`, `id([Pil])`, `id{Keputusan}`, `id((Lingkaran))`.
  Simpul dideklarasikan saat pertama kali diberi label; setelah itu cukup id-nya.
- Sisi: `a -> b`, dengan label `a -ya-> b`, putus-putus `a ..> b`; bisa dirangkai:
  `a -> b -> c`.
- `style id fill=#hex text=#hex border=#hex` mewarnai satu simpul.
- `@flow right` menatanya dari kiri ke kanan alih-alih dari atas ke bawah.
- FluxCode menatanya sendiri per lapis, dan sisi yang naik kembali (loop) lewat di sisi.
  Di GitHub bentuknya jadi flowchart Mermaid, yang digambar GitHub.

@en
## Callouts, details and columns
@id
## Callout, details, dan kolom

```flux
@tip Keep it short
One idea per callout.
@end

@details What happens on GitHub
A collapsed section, opened with a click.
@end

@columns
Left column.
@col
Right column.
@end
```

@en
@tip Keep it short
`@note`, `@tip`, `@warn` and `@danger` each have their color and icon. The text after the
directive is the title.
@end
@id
@tip Ringkas saja
`@note`, `@tip`, `@warn`, dan `@danger` masing-masing punya warna dan ikonnya. Teks
setelah direktif adalah judulnya.
@end

@en
@details What happens on GitHub
`@details` becomes `<details><summary>`, and callouts become GitHub's alerts
(`> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, `> [!CAUTION]`).
@end
@id
@details Apa yang terjadi di GitHub
`@details` menjadi `<details><summary>`, dan callout menjadi alert milik GitHub
(`> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, `> [!CAUTION]`).
@end

@en
@columns
**Columns** sit side by side on a wide window and one under the other on a narrow one.
@col
On GitHub, which has no columns, they come one after the other.
@end
@id
@columns
**Kolom** tampil berdampingan di jendela lebar dan bertumpuk di jendela sempit.
@col
Di GitHub, yang tidak punya kolom, keduanya tampil berurutan.
@end

@en
## Languages

One file can hold the same text in several languages. FluxCode shows the sections for
its interface language (Settings > Language); `@all` goes back to text for everyone.

```flux
@en
FluxCode starts in 0.22 s.
@id
FluxCode terbuka dalam 0,22 detik.
@all
```

`changelog.flux`, the release notes FluxCode shows after an update, uses this with
`@release DATE` and `@title en: ...` / `@title id: ...`.
@id
## Bahasa

Satu file bisa memuat teks yang sama dalam beberapa bahasa. FluxCode menampilkan bagian
sesuai bahasa antarmukanya (Settings > Language); `@all` kembali ke teks untuk semua.

```flux
@en
FluxCode starts in 0.22 s.
@id
FluxCode terbuka dalam 0,22 detik.
@all
```

`changelog.flux`, catatan rilis yang ditampilkan FluxCode setelah update, memakai ini
bersama `@release DATE` dan `@title en: ...` / `@title id: ...`.

@en
## The page's look

```flux
@style accent=#d0a070 width=880 size=15 font="Segoe UI"
```

`accent` colors links, list bullets and the title's underline; `width` is the text
column in pixels; `size` the text size; `font` the typeface. Anything not set follows
the FluxCode theme.
@id
## Tampilan halaman

```flux
@style accent=#d0a070 width=880 size=15 font="Segoe UI"
```

`accent` mewarnai tautan, butir daftar, dan garis bawah judul; `width` lebar kolom teks
dalam piksel; `size` ukuran teks; `font` jenis hurufnya. Yang tidak disetel mengikuti tema
FluxCode.

@en
## On GitHub

GitHub only draws a few formats as documents (Markdown, reStructuredText, AsciiDoc, Org
and some older ones). A Flux README therefore comes with the `README.md` made from it:

```text
fluxcode --flux-to-md README.flux README.md
fluxcode --flux-to-md README.flux README.md --check
```

The second line changes nothing and fails when `README.md` is out of date; FluxCode's
tests run the same check, so the two can't drift apart. In FluxCode, **Save as Markdown**
above a Flux document does the same for the file you have open.
@id
## Di GitHub

GitHub hanya menggambar beberapa format sebagai dokumen (Markdown, reStructuredText,
AsciiDoc, Org, dan beberapa yang lebih lama). Karena itu README Flux selalu disertai
`README.md` hasil turunannya:

```text
fluxcode --flux-to-md README.flux README.md
fluxcode --flux-to-md README.flux README.md --check
```

Baris kedua tidak mengubah apa pun dan gagal kalau `README.md` sudah usang; test FluxCode
menjalankan pemeriksaan yang sama, jadi keduanya tidak bisa melenceng. Di FluxCode,
**Save as Markdown** di atas dokumen Flux melakukan hal yang sama untuk file yang sedang
terbuka.

@en
Add `--lang id` to write the Indonesian sections instead (FluxCode's own README ships as
`README.md` and `README.id.md`). English is the default, because that is what a first-time
reader on GitHub needs.
@id
Tambahkan `--lang id` untuk menulis bagian Indonesianya saja (README FluxCode sendiri
tersedia sebagai `README.md` dan `README.id.md`). Bahasa Inggris adalah bawaannya, karena
itu yang dibutuhkan pembaca pertama kali di GitHub.

@en
@warn The .flux name on GitHub
GitHub's language list (Linguist) already has a language called FLUX using `.flux`,
so GitHub labels these files with it. This repository's `.gitattributes` marks `.flux`
as Markdown instead. Linguist only adds a format once it is used widely (at least 2,000
files in a year across many repositories), so Flux can't be listed there yet.
@end
@id
@warn Nama .flux di GitHub
Daftar bahasa GitHub (Linguist) sudah punya bahasa bernama FLUX dengan ekstensi `.flux`,
jadi GitHub memberi label itu pada file-file ini. `.gitattributes` di repositori ini
menandai `.flux` sebagai Markdown sebagai gantinya. Linguist hanya menambah format
setelah dipakai luas (paling sedikit 2.000 file dalam setahun di banyak repositori), jadi
Flux belum bisa terdaftar di sana.
@end

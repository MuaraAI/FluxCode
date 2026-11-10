// docs/flux.flux: the Flux document format, written in Flux.
// docs/flux.md is made from it: fluxcode --flux-to-md docs/flux.flux docs/flux.md

@title Flux documents
@style accent=#d0a070

A `.flux` file is a text document: everything Markdown writes, plus blocks for what
Markdown hands to HTML or doesn't have at all. FluxCode opens it drawn, and turns it into
GitHub Markdown when it has to be read on GitHub (FluxCode's own `README.md` is made
from `README.flux` this way).

@toc

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

## How it stays light

FluxCode reads a document in one pass and copies nothing: each block keeps the position
of its text in the file (20 bytes a block). Every table row and every list item is a
block of its own, and the window draws only the blocks in view, so a long document costs
about what one screen of it costs. Bold, links and keys are worked out only for the
blocks being drawn.

Measured with `cargo test --release --bin fluxcode million_lines -- --ignored --nocapture`
on an AMD Ryzen 5 7535HS:

| Document | Read in | Blocks | Memory for the blocks |
| ---: | ---: | ---: | ---: |
| 1,300,003 lines, 31.8 MB (headings, lists, a 200,000-row table, paragraphs) | 57 ms | 900,001 | 18.9 MB |

Opened in FluxCode (a release build, fresh profile, three runs), the same document with a
`@title` and a `@toc` on top:

| Window on screen | Read in | Task Manager, 9 s after launch | Memory reserved |
| ---: | ---: | ---: | ---: |
| 0.22 to 0.31 s | 73 ms | 18.6 MB | 190 MB (the text itself is 31.8 MB) |

The editor behind **Source** is filled only when Source is first shown: a document over
2 MB opens drawn without an editor holding a second copy of its text.

@note What this does not promise
The text itself still takes its own size in memory (31.8 MB here), and the source view
is an ordinary editor. What stays small is everything on top of the text: the blocks,
and the work of drawing, which only covers what is on screen.
@end

## Text

Paragraphs, headings and lists are written as in Markdown.

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

Inline: **bold** `**bold**`, *italic* `*italic*`, `code` with backquotes, ~~struck~~
`~~struck~~`, ==highlighted== `==highlighted==`, [a link](https://github.com/MuaraAI/FluxCode)
`[a link](url)` (a bare `https://...` is a link too), a key {kbd Ctrl+Shift+P}
`{kbd Ctrl+Shift+P}`, a badge {badge New} `{badge New}` and {color=#5fd38d colored text}
`{color=#5fd38d colored text}`. A backslash keeps a character as it is: `\*`.

Lines starting with `//` are comments: they never show.

## Tables

A pipe table works as on GitHub; the line under the header sets the alignment.

```flux
| Editor | Start | Memory |
| :--- | ---: | :---: |
| FluxCode | 0.22 s | 1.4 MB |
```

`@table` needs no pipes at the ends and takes options:

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

`align=` takes `l`, `c` or `r` per column, `widths=` relative widths, and
`header=none` makes the first row an ordinary one. A table can have any number of rows:
FluxCode draws only those on screen.

## Images, GIFs and video

```flux
@image assets/icon.png width=96 alt="The FluxCode icon"
@image chart-light.svg dark=chart-dark.svg width=100% alt="A chart"
@image demo.gif width=60% caption="A GIF plays as it is" link=demo.gif
@video intro.mp4 poster=intro.png alt="A one-minute tour"
```

- `width=` in pixels or as a share of the page (`60%`); `align=right` floats it right.
- `dark=` is shown instead when the theme is dark (on GitHub too, through `<picture>`).
- `link=` is what a click opens; `caption=` goes under it.
- A GIF plays in FluxCode as it is. Images from the web aren't fetched: the document
  stays offline, and shows a link instead.
- `@video` shows the poster with a play button; a click plays the video in the system's
  player (FluxCode has no video decoder, which keeps it at 16 MB). On GitHub it is the
  poster linked to the video.

@image ../assets/icon.png width=96 alt="The FluxCode icon: a square meander, one ivory hook and one ochre hook locked together, on a near-black tile."

Two or more images side by side:

```flux
@gallery
@image before.png width=49% alt="Before"
@image after.png width=49% alt="After"
@end
```

## Flowcharts

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

- Nodes: `id[Box]`, `id(Rounded)`, `id([Pill])`, `id{Decision}`, `id((Circle))`. A node
  is declared the first time it has a label; after that its id is enough.
- Edges: `a -> b`, with a label `a -yes-> b`, dashed `a ..> b`; they chain:
  `a -> b -> c`.
- `style id fill=#hex text=#hex border=#hex` colors one node.
- `@flow right` lays it out from left to right instead of top to bottom.
- FluxCode lays it out by itself in layers, and edges that go back up (loops) run along
  the side. On GitHub it becomes a Mermaid flowchart, which GitHub draws.

## Callouts, details and columns

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

@tip Keep it short
`@note`, `@tip`, `@warn` and `@danger` each have their color and icon. The text after the
directive is the title.
@end

@details What happens on GitHub
`@details` becomes `<details><summary>`, and callouts become GitHub's alerts
(`> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, `> [!CAUTION]`).
@end

@columns
**Columns** sit side by side on a wide window and one under the other on a narrow one.
@col
On GitHub, which has no columns, they come one after the other.
@end

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

## The page's look

```flux
@style accent=#d0a070 width=880 size=15 font="Segoe UI"
```

`accent` colors links, list bullets and the title's underline; `width` is the text
column in pixels; `size` the text size; `font` the typeface. Anything not set follows
the FluxCode theme.

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

@warn The .flux name on GitHub
GitHub's language list (Linguist) already has a language called FLUX using `.flux`,
so GitHub labels these files with it. This repository's `.gitattributes` marks `.flux`
as Markdown instead. Linguist only adds a format once it is used widely (at least 2,000
files in a year across many repositories), so Flux can't be listed there yet.
@end

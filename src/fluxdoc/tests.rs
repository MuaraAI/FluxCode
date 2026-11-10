use super::{flow, inline, md, parse, text, Align, Block, Kind};

const SAMPLE: &str = "@title Flux
@style accent=#d0a070 width=880
// a comment
Intro **bold** and *it*.
Second line.

## Install

- one
  - nested
- [x] done
3. three

@table align=l,r
Name | Size
FluxCode | 16 MB
Zed | 261 MB
@end

| A | B |
|:--|:-:|
| 1 | 2 |

@note Heads up
Inside a **note**.
@end

@flow down
a([Start]) -> b{Ok?}
b -yes-> c[Ship]
b -no-> a
@end

@image shot.png alt=\"The app\" width=480
```rust
fn main() {}
```
@en
English only.
@id
Hanya Indonesia.
@all
Both.
";

#[test]
fn blocks_in_order() {
    let d = parse(SAMPLE);
    let kinds: Vec<&str> = d
        .blocks
        .iter()
        .map(|b| match b {
            Block::Title(_) => "title",
            Block::Para(_) => "para",
            Block::Heading(..) => "h",
            Block::Item { .. } => "item",
            Block::Row { .. } => "row",
            Block::Boxed { .. } => "box",
            Block::Flow(_) => "flow",
            Block::Image(_) => "image",
            Block::Code { .. } => "code",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["title", "para", "h", "item", "item", "item", "item", "row", "row", "row", "row", "row", "box", "flow", "image", "code", "para", "para", "para"]);
    assert_eq!(text(SAMPLE, match d.blocks[1] { Block::Para(s) => s, _ => unreachable!() }), "Intro **bold** and *it*.\nSecond line.");
    assert_eq!(d.style.accent, Some(0xd0a070));
    assert_eq!(d.style.width, Some(880.));
    assert!(matches!(d.blocks[4], Block::Item { depth: 1, .. }));
    assert!(matches!(d.blocks[5], Block::Item { check: 2, .. }));
    assert!(matches!(d.blocks[6], Block::Item { num: 3, .. }));
    assert_eq!(d.tables[0].align, [Align::Left, Align::Right]);
    assert_eq!(d.tables[1].align, [Align::Left, Align::Center]);
    assert!(matches!(d.blocks[7], Block::Row { header: true, n: 0, .. }));
    assert!(matches!(d.blocks[12], Block::Boxed { kind: Kind::Note, .. }));
    // Languages: the last three paragraphs are en, id, all.
    assert_eq!(&d.lang[16..], [1, 2, 0]);
    assert_eq!(d.visible("en").len(), d.blocks.len() - 1);
}

#[test]
fn inline_marks_and_links() {
    let i = inline("a **b** *c* `d` [e](https://x.y) {kbd Ctrl+S} ~~f~~ snake_case_name");
    assert_eq!(i.text, "a b c d e Ctrl+S f snake_case_name");
    let marks = |t: &str| i.runs.iter().find(|(r, _)| &i.text[r.clone()] == t).map(|(_, m)| *m).unwrap();
    assert!(marks("b").bold);
    assert!(marks("c").italic);
    assert!(marks("d").code);
    assert!(marks("Ctrl+S").kbd);
    assert!(marks("f").strike);
    assert!(!marks(" snake_case_name").italic, "underscores inside words are not italic");
    assert_eq!(i.links, [(8..9, "https://x.y".to_string())]);
    assert_eq!(inline(r"\*not italic\*").text, "*not italic*");
}

#[test]
fn flowchart_layers_and_loops() {
    let d = parse(SAMPLE);
    let f = &d.flows[0];
    assert_eq!(f.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(f.nodes[1].shape, flow::Shape::Diamond);
    assert_eq!(f.nodes.iter().map(|n| n.layer).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(f.edges.iter().any(|e| e.back && e.label == "no"), "b -> a closes a loop");
    // Layers go down and nothing overlaps.
    assert!(f.nodes[0].rect.1 < f.nodes[1].rect.1 && f.nodes[1].rect.1 < f.nodes[2].rect.1);
    let m = flow::to_mermaid(f);
    assert!(m.starts_with("flowchart TD"));
    assert!(m.contains("b -->|yes| c"));
}

#[test]
fn markdown_for_github() {
    let d = parse(SAMPLE);
    let m = md::to_markdown(&d, SAMPLE, "en", None);
    assert!(m.starts_with("# Flux\n\nIntro **bold** and *it*.\nSecond line.\n\n## Install\n\n- one\n  - nested\n- [x] done\n3. three\n"), "{m}");
    assert!(m.contains("| Name | Size |\n| --- | ---: |\n| FluxCode | 16 MB |\n| Zed | 261 MB |"));
    assert!(m.contains("> [!NOTE]\n> **Heads up**\n> Inside a **note**."));
    assert!(m.contains("```mermaid\nflowchart TD"));
    assert!(m.contains("<img src=\"shot.png\" width=\"480\" alt=\"The app\">"));
    assert!(m.contains("English only.") && !m.contains("Hanya Indonesia.") && m.contains("Both."));
    assert_eq!(md::inline_md("{kbd Ctrl+P} ==hi=="), "<kbd>Ctrl+P</kbd> **hi**");
    assert_eq!(md::slug("FluxCode, Zed and VS Code"), "fluxcode-zed-and-vs-code");
}

#[test]
fn changelog_flux_still_reads() {
    let src = "@release 2026-11-10\n@title en: New\n@title id: Baru\n@en\nHello.\n@id\nHalo.\n";
    let d = parse(src);
    assert!(matches!(d.blocks[0], Block::Release(_)));
    assert_eq!(d.lang, [0, 1, 2, 1, 2]);
}

/// README.md and docs/flux.md are made from their .flux files: they must agree (run
/// `fluxcode --flux-to-md README.flux README.md` after editing README.flux).
#[test]
fn markdown_made_from_flux_is_up_to_date() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (flux, md_file) in [("README.flux", "README.md"), ("docs/flux.flux", "docs/flux.md")] {
        let src = std::fs::read_to_string(root.join(flux)).unwrap_or_else(|e| panic!("{flux}: {e}"));
        let want = md::to_markdown(&parse(&src), &src, "en", Some(&md::banner(flux, md_file)));
        let have = std::fs::read_to_string(root.join(md_file)).unwrap_or_default().replace("\r\n", "\n");
        assert!(have == want, "{md_file} is out of date: run `fluxcode --flux-to-md {flux} {md_file}`");
    }
}

/// A million lines (text, lists and a 200,000-row table), parsed in one pass.
/// `cargo test --release --bin fluxcode million_lines -- --ignored --nocapture`
#[test]
#[ignore]
fn million_lines() {
    let mut src = String::with_capacity(48 << 20);
    for i in 0..100_000 {
        src.push_str(&format!("## Section {i}\nSome **bold** text with a [link](https://example.com) and `code`.\n- item one\n- item two\n\n"));
    }
    src.push_str("@table align=l,r,c\nName | Size | State\n");
    for i in 0..200_000 {
        src.push_str(&format!("row {i} | {} KB | ok\n", i * 3));
    }
    src.push_str("@end\n");
    for i in 0..300_000 {
        src.push_str(&format!("Line {i} of plain text that goes on for a while.\n\n"));
    }
    let lines = src.lines().count();
    let t = std::time::Instant::now();
    let d = parse(&src);
    let took = t.elapsed();
    let block_bytes = d.blocks.len() * std::mem::size_of::<Block>() + d.lang.len();
    println!(
        "{lines} lines, {:.1} MB: parsed in {:.1} ms ({:.0} MB/s), {} blocks, {} bytes each, {:.1} MB of blocks",
        src.len() as f64 / 1e6,
        took.as_secs_f64() * 1e3,
        src.len() as f64 / 1e6 / took.as_secs_f64(),
        d.blocks.len(),
        std::mem::size_of::<Block>(),
        block_bytes as f64 / 1e6
    );
    assert!(lines >= 1_000_000);
    // Drawing one screen works out inline marks for about 40 blocks.
    let t = std::time::Instant::now();
    let mut n = 0;
    for b in d.blocks.iter().skip(1_000) {
        if let Block::Para(s) | Block::Heading(_, s) | Block::Item { text: s, .. } = b {
            std::hint::black_box(inline(text(&src, *s)));
            n += 1;
            if n == 40 {
                break;
            }
        }
    }
    assert_eq!(n, 40);
    println!("inline marks for one screen (40 blocks): {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
}

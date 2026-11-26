# gpui-component 0.9.0, as FluxCode builds it

A copy of [gpui-component](https://github.com/longbridge/gpui-kit) 0.9.0 from crates.io
(Apache-2.0, see LICENSE-APACHE) with two FluxCode changes:

1. Incremental syntax highlighting in `src/highlighter/syntect_backend.rs`:

**Syntax colors parse only the lines an edit changes.** Upstream parsed the whole
document again on every key for files up to 64 KB, on the UI thread. Syntect is slow
on bat's JavaScript grammar: a 150-line `.js` file took about 80 ms per parse on a
Ryzen 5 7535HS, so on slower PCs typing fell behind the keys (issue #92). Now the
highlighter keeps the parser state each line starts in, parses from the edited line,
and stops at the first later line that starts in the same state as before. A
keystroke parses one or two lines. Lines that start in the same state share one copy
of it. Files over 64 KB also run incremental `LineParse` updates on the debounced
background thread, lines over 4 KB skip regex tokenization to avoid backtracking
stalls on minified files, and brace-fold scanning stops above 512 KB.

The tests at the end of that file check that the line-by-line result equals a full
parse after many kinds of edits, and that a keystroke parses at most two lines.

2. Markdown HTML-block normalization and fallback in `src/text/html_blocks.rs` and `src/text/compat.rs`:

**Block HTML in Markdown no longer collides on `ElementId("p")`.** Upstream
`gpui-base 0.9.0` lowers `mdast::Node::Html` via `format::html::parse` with `span: None`,
so two or more HTML blocks that enter `InlineFlow` share `GlobalElementId(..., "p")` and
panic on out-of-bounds item indices in `InlineFlow::prepaint`. Non-MDX Markdown now
normalizes common HTML block tags into spanned CommonMark/GFM blocks and guards any
residual `mdast::Node::Html` with a custom fallback block renderer.

To update: copy the new upstream version over this folder (without `Cargo.toml.orig`,
`Cargo.lock` and `.cargo_vcs_info.json`) and apply both changes again unless upstream
fixes them.

# Third-party notices

FluxCode builds on the following components. Each is used under its own license,
which the FluxCode license (GPL-3.0-or-later) does not change. All of them are
compatible with the GPL v3.

| Component | License | Used for |
| --- | --- | --- |
| [GPUI](https://gpui.rs) (from the Zed repository) and [GPUI Fast](https://github.com/longbridge/gpui-fast) | Apache-2.0 | UI framework, GPU rendering; `vendor/gpui-fast-windows` is a modified copy of its Windows renderer (changes listed in its FLUXCODE.md) |
| [GPUI Kit](https://github.com/longbridge/gpui-kit) (`gpui-kit`, `gpui-component`, `gpui-base`) | Apache-2.0 | Widgets and the code editor |
| [Lucide](https://lucide.dev) icons (through `gpui-kit-assets`) | ISC | Icons |
| [syntect](https://github.com/trishume/syntect) and [two-face](https://github.com/CosmicHorrorDev/two-face) | MIT | Syntax highlighting |
| Language grammars from [bat](https://github.com/sharkdp/bat) (through two-face) | each under its own license, listed in two-face | Syntax highlighting for about 200 languages |
| `serde`, `serde_json`, `lsp-types`, `futures`, `anyhow`, `chrono`, `image`, `base64`, `miniz_oxide`, `sha1_smol`, `embed-resource` and the other crates in `Cargo.lock` | MIT or Apache-2.0 | Data formats, archives, build |
| Microsoft Edge WebView2 Loader (only with the optional `embedded-preview` feature, off by default) | Microsoft WebView2 SDK license | In-window web preview |

Not bundled, used when installed on the machine:
[OpenCode](https://opencode.ai) (MIT) and Mura, over the
[Agent Client Protocol](https://agentclientprotocol.com); `git`; `curl` (update
check, GitHub sign-in); the GitHub CLI `gh` (pull requests, Actions).

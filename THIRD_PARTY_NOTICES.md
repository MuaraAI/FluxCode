# Third-party notices

FluxCode includes or builds on the following components. Each is used under
its own license, which the FluxCode license (GPL-3.0-or-later) does not change.
All of them are compatible with the GPL v3.

| Component | License | Used for |
| --- | --- | --- |
| [CodeMirror 6](https://codemirror.net/) (`@codemirror/*`, `@lezer/*`) | MIT | Code editor and language parsers |
| [Tauri](https://tauri.app/) (`tauri`, `@tauri-apps/*`, `tauri-plugin-dialog`) | MIT or Apache-2.0 | Desktop shell, IPC, dialogs |
| [Serde](https://serde.rs/) (`serde`, `serde_json`) | MIT or Apache-2.0 | Rust (de)serialization |
| Other Rust crates resolved in `src-tauri/Cargo.lock` | mostly MIT or Apache-2.0 | Tauri's dependencies |
| [Vite](https://vite.dev/) | MIT | Build tool (development only) |
| [GPUI](https://gpui.rs) (`gpui-pre`, from the Zed repository) | Apache-2.0 | GPUI edition: UI framework |
| [GPUI Kit](https://github.com/longbridge/gpui-kit) (`gpui-kit`, `gpui-component`, `gpui-base`, `gpui-webview`) | Apache-2.0 | GPUI edition: widgets, code editor, optional WebView |
| [Lucide](https://lucide.dev) icons (`gpui-kit-assets`) | ISC | GPUI edition: icons |
| [Tree-sitter](https://tree-sitter.github.io) and its language grammars | MIT | GPUI edition: syntax highlighting |
| `lsp-types`, `futures`, `anyhow`, `embed-resource` and other crates in `gpui-app/Cargo.lock` | MIT or Apache-2.0 | GPUI edition |
| Microsoft Edge WebView2 Loader (`WebView2Loader.dll`) | Microsoft WebView2 SDK license (redistributable) | Loads the system WebView2 runtime |

FluxCode talks to [OpenCode](https://opencode.ai) (MIT) over the
[Agent Client Protocol](https://agentclientprotocol.com) when it is
installed on the machine; it is not bundled. The GitHub sign-in uses the
system `curl.exe` and, optionally, the GitHub CLI (`gh`), neither of which is
bundled either.

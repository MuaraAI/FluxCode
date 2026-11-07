# Third-party notices

FluxCode includes or builds on the following components. Each is used under
its own license, which the FluxCode license does not change.

| Component | License | Used for |
| --- | --- | --- |
| [CodeMirror 6](https://codemirror.net/) (`@codemirror/*`, `@lezer/*`) | MIT | Code editor and language parsers |
| [Tauri](https://tauri.app/) (`tauri`, `@tauri-apps/*`, `tauri-plugin-dialog`) | MIT or Apache-2.0 | Desktop shell, IPC, dialogs |
| [Serde](https://serde.rs/) (`serde`, `serde_json`) | MIT or Apache-2.0 | Rust (de)serialization |
| Other Rust crates resolved in `src-tauri/Cargo.lock` | mostly MIT or Apache-2.0 | Tauri's dependencies |
| [Vite](https://vite.dev/) | MIT | Build tool (development only) |
| Microsoft Edge WebView2 Loader (`WebView2Loader.dll`) | Microsoft WebView2 SDK license (redistributable) | Loads the system WebView2 runtime |

FluxCode talks to [OpenCode](https://opencode.ai) (MIT) over the
[Agent Client Protocol](https://agentclientprotocol.com) when it is
installed on the machine; it is not bundled. The GitHub sign-in uses the
system `curl.exe` and, optionally, the GitHub CLI (`gh`), neither of which is
bundled either.

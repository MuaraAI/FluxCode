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
| [Jersey 15 and Jersey 25](https://github.com/scfried/soft-type-jersey) by The Soft Type Project Authors, via [Fontsource](https://fontsource.org/) | SIL Open Font License 1.1 | Interface fonts |
| Microsoft Edge WebView2 Loader (`WebView2Loader.dll`) | Microsoft WebView2 SDK license (redistributable) | Loads the system WebView2 runtime |

The Jersey fonts are bundled unmodified. Their license (SIL OFL 1.1) is
included in `node_modules/@fontsource/jersey-*/LICENSE` and is available at
https://openfontlicense.org. The fonts may be used, studied, modified and
redistributed freely under that license, but may not be sold on their own.

# FluxCode — GPUI edition

Native FluxCode UI built on [GPUI](https://gpui.rs) + [gpui-component](https://github.com/longbridge/gpui-kit)
(both Apache-2.0). No WebView: one process, drawn by the GPU (DirectX 11 on Windows).
The Tauri app in `src/` + `src-tauri/` is untouched and keeps working while this port grows.

## Build (Windows)

The worktree path contains spaces and MinGW `windres` breaks on them, so build into a
target dir without spaces:

```powershell
$env:CARGO_TARGET_DIR = "C:/Users/User/.fluxcode-target-gpui"
cargo run                      # debug (shaders compiled at runtime)
```

The default build uses [GPUI Fast](https://github.com/longbridge/gpui-fast) (feature
`fast`); `cargo build --release --no-default-features` builds on standard GPUI instead.
GPUI Fast pins a few dependency versions exactly (`taffy 0.15.0`, `unicode-properties
0.1.3`), which `Cargo.lock` already reflects.

Release builds compile HLSL at build time and normally need `fxc.exe` from the Windows SDK.
`tools/fxc-shim` is a tiny stand-in that uses Windows' own `d3dcompiler_47.dll`:

```powershell
cargo build --release --manifest-path tools/fxc-shim/Cargo.toml
$env:GPUI_FXC_PATH = "<target>/release/fxc-shim.exe"
cargo build --release          # -> <target>/release/fluxcode-gpui.exe
```

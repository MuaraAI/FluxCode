# gpui-fast-windows 0.1.13, as FluxCode builds it

A copy of [gpui-fast-windows](https://github.com/longbridge/gpui-fast) 0.1.13
(Apache-2.0, see LICENSE) with seven changes, in `src/directx_renderer.rs`,
`src/directx_atlas.rs`, `src/fast/frame.rs`, `src/window.rs` and `src/events.rs`. Search for "FluxCode" to find them.

1. **Path textures on demand.** Upstream makes two window-sized textures for drawing
   paths when the window opens, one of them 4x multisampled: five window-sized buffers,
   about 44 MB at 1920x1140. An editor's frames rarely draw paths (FluxCode: the install
   animation), so they are now made by the first frame that draws a path and released
   after 240 frames without one, or when the window is resized.
2. **Two swap-chain buffers** instead of three (the flip model's minimum): one
   window-sized buffer less. GPUI draws on demand, so the third bought nothing here.

3. **Acrylic that doesn't flicker while the window moves** (MuaraAI/FluxCode#94). The
   `Blurred` background used the accent policy's acrylic (`SetWindowCompositionAttribute`,
   state 4), which stutters and flickers while the window is dragged. From Windows 11
   22H2 (build 22621) the compositor's own acrylic is used instead
   (`DWMWA_SYSTEMBACKDROP_TYPE` = `DWMSBT_TRANSIENTWINDOW`); on older systems the move/size
   loop swaps acrylic for plain blur (state 3) until the move ends.

4. **`zoom()` restores a maximized window** (MuaraAI/FluxCode#107). Upstream's
   `zoom()` always sends `SW_MAXIMIZE`, which is a no-op on an already-maximized
   window, so FluxCode's custom restore button (and titlebar double-click) could
   never restore. It now toggles `SW_RESTORE`/`SW_MAXIMIZE` on `IsZoomed`, the
   same thing the native caption button does (`HTMAXBUTTON` in `events.rs`).
   `SW_RESTORE` is deliberate, not the native path's `SW_NORMAL`: both undo a
   maximize, but `SW_RESTORE` is the canonical restore command (and what
   `activate()` in `window.rs` uses); do not "fix" it back to `SW_NORMAL`.
   While fullscreen, `zoom()` is a no-op: fullscreen is `toggle_fullscreen`'s
   mode, not a maximize.
5. **Titlebar dragging via `WM_GPUI_START_WINDOW_MOVE`** (MuaraAI/FluxCode#126).
   `PlatformWindow::start_window_move` posts `WM_GPUI_START_WINDOW_MOVE`, which
   releases mouse capture and invokes `DefWindowProcW(WM_NCLBUTTONDOWN, HTCAPTION)`
   without re-entering `handle_nc_mouse_down_msg` or double-counting `ClickState`.
6. **Oversized atlas tile guard** (MuaraAI/FluxCode#121). `DirectXAtlasTextures::allocate`
   rejects tiles above Direct3D 11's 16384px limit before `push_texture` so a tall
   image cannot leak a clamped 16384px texture every frame.
7. **Non-topmost DirectComposition target + focus reclaim for embedded child HWNDs.**
   `DirectComposition::new` calls `CreateTargetForHwnd(hwnd, false)` so `WS_CHILD`
   native WebView2 windows composite cleanly above the GPUI swap chain while preserving
   DWM Acrylic and atomic window moves; `handle_mouse_down_msg` calls `SetFocus(handle)`
   when `GetFocus() != handle` to reclaim keyboard focus when clicking back into GPUI.

To update: copy the new upstream version over this folder and apply the seven changes again.

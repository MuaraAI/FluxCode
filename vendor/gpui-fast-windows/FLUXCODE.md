# gpui-fast-windows 0.1.13, as FluxCode builds it

A copy of [gpui-fast-windows](https://github.com/longbridge/gpui-fast) 0.1.13
(Apache-2.0, see LICENSE) with two changes, both in `src/directx_renderer.rs` and
`src/fast/frame.rs`. Search for "FluxCode" to find them.

1. **Path textures on demand.** Upstream makes two window-sized textures for drawing
   paths when the window opens, one of them 4x multisampled: five window-sized buffers,
   about 44 MB at 1920x1140. An editor's frames rarely draw paths (FluxCode: the install
   animation), so they are now made by the first frame that draws a path and released
   after 240 frames without one, or when the window is resized.
2. **Two swap-chain buffers** instead of three (the flip model's minimum): one
   window-sized buffer less. GPUI draws on demand, so the third bought nothing here.

To update: copy the new upstream version over this folder and apply the two changes again.

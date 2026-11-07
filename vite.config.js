import { defineConfig } from "vite";

// Tauri expects a fixed port in dev and a static build in dist/.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022", // WebView2 is evergreen Chromium; no legacy transpiling needed
    minify: true,
    sourcemap: false,
    cssCodeSplit: false,
    reportCompressedSize: false,
  },
});

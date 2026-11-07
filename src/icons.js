// FluxCode's own icon set, drawn for this project.
//
// UI icons: 24px grid, 1.5px rounded strokes, ~2px corner radius, the shape
// filling the same 18px optical box so they all look the same size.
// File icons: one 16×16 tile for every file type (same size, same corner
// radius, a light tint of the type color) with a simple glyph inside, so names
// line up perfectly in the explorer, tabs and terminal. All inline SVG.

const svg = (body, size = 16, extra = "") =>
  `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"${extra}>${body}</svg>`;

const P = {
  // activity bar
  explorer: '<path d="M9 3.5h5.5L19 8v9.5a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2z"/><path d="M14.5 3.5V8H19"/><path d="M4 7v11.5a2 2 0 0 0 2 2h8"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/>',
  // Run and Debug: a play outline with a bug (like VS Code's Run view icon)
  debug: '<path d="M4.5 5.1v13.8a1 1 0 0 0 1.5.86l5.2-3.2"/><path d="M4.5 5.1a1 1 0 0 1 1.5-.86L12 7.9"/><path d="M17.5 11.5a3 3 0 0 1 3 3v2.2a3 3 0 1 1-6 0v-2.2a3 3 0 0 1 3-3z"/><path d="M16 11.5a1.5 1.5 0 0 1 3 0"/><path d="M13 14.2h1.5M13 17.4h1.5M20.5 14.2H22M20.5 17.4H22"/>',
  // OpenCode agent
  agent: '<rect x="4" y="8" width="16" height="12" rx="2.5"/><path d="M12 8V4.5M9.5 4.5h5"/><path d="M2 14h2M20 14h2"/><path d="M9 13v2M15 13v2"/><path d="M9.5 17.5h5"/>',
  // a real cog (Feather/Lucide "settings"), instantly read as Settings
  settings:
    '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
  // small UI glyphs
  chevron: '<path d="m9.5 6 6 6-6 6"/>',
  menu: '<path d="M4 7h16M4 12h16M4 17h16"/>',
  newFile: '<path d="M13 3.5H8a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h4"/><path d="M13 3.5 18 8.5v3"/><path d="M17.5 14.5v6M14.5 17.5h6"/>',
  newFolder: '<path d="M11 19.5H5a2 2 0 0 1-2-2v-11a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2v2.5"/><path d="M17.5 14v6M14.5 17h6"/>',
  file: '<path d="M14 3.5H7a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5z"/><path d="M14 3.5v5h5"/>',
  refresh: '<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"/><path d="M19.5 4.5v4h-4"/>',
  collapse: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8.5 12h7"/>',
  close: '<path d="m7 7 10 10M17 7 7 17"/>',
  play: '<path d="M7 5.5v13a1 1 0 0 0 1.5.86l10.6-6.5a1 1 0 0 0 0-1.72L8.5 4.64A1 1 0 0 0 7 5.5z" fill="currentColor"/>',
  stop: '<rect x="6.5" y="6.5" width="11" height="11" rx="2.5" fill="currentColor"/>',
  // "clear all": lines being swept away by an ×
  clear: '<path d="M4 6.5h12M4 11.5h8M4 16.5h5"/><path d="m14 13 5.5 5.5M19.5 13 14 18.5"/>',
  trash: '<path d="M4.5 7h15M9.5 7V4.5h5V7M6.5 7l.8 12.6a1 1 0 0 0 1 .9h7.4a1 1 0 0 0 1-.9L17.5 7"/><path d="M10 10.5v6M14 10.5v6"/>',
  sidebar: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M9 4.5v15"/>',
  sidebarOn: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M5.5 6.5h3.5v11H5.5z" fill="currentColor" stroke="none"/><path d="M9 4.5v15"/>',
  panel: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M3.5 14h17"/>',
  panelOn: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M5.5 14h13v3.5h-13z" fill="currentColor" stroke="none"/><path d="M3.5 14h17"/>',
  info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><circle cx="12" cy="7.8" r=".6" fill="currentColor"/>',
  error: '<circle cx="12" cy="12" r="8.5"/><path d="m9 9 6 6M15 9l-6 6"/>',
  warning: '<path d="M10.3 4.5a2 2 0 0 1 3.4 0l7 12.3a2 2 0 0 1-1.7 3H5a2 2 0 0 1-1.7-3z"/><path d="M12 9.5v4"/><circle cx="12" cy="16.8" r=".6" fill="currentColor"/>',
  caseSensitive: '<path d="M3.5 17 7 7.5 10.5 17M4.8 13.5h4.4"/><path d="M17 17v-1.2m0 0c-.6.9-1.5 1.4-2.5 1.4-1.3 0-2.2-.8-2.2-2s1-2 2.6-2h2.1m0 2.6V12.6c0-1.3-.8-2.1-2.2-2.1-.9 0-1.7.4-2.1 1.1"/>',
  wholeWord: '<path d="M3 19h18" opacity=".55"/><path d="M5 15.5v-6m0 3a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0"/><path d="M19 10.5a2.5 2.5 0 1 0 0 4"/><path d="M14.5 15.5V7.5"/>',
  preview: '<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M3 9h18"/><circle cx="6" cy="6.8" r=".5" fill="currentColor"/><circle cx="8" cy="6.8" r=".5" fill="currentColor"/>',
  // preview devices
  desktop: '<rect x="3" y="4.5" width="18" height="12" rx="2"/><path d="M9 20h6M12 16.5V20"/>',
  tablet: '<rect x="5.5" y="3" width="13" height="18" rx="2.5"/><path d="M10.5 18h3"/>',
  phone: '<rect x="7.5" y="3" width="9" height="18" rx="2.5"/><path d="M11 18h2"/>',
  // terminal
  terminal: '<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="m7 10 3 2.5L7 15"/><path d="M12.5 15.5h4.5"/>',
  up: '<path d="M12 19V5.5"/><path d="m6.5 11 5.5-5.5 5.5 5.5"/>',
  back: '<path d="M19 12H5.5"/><path d="m11 6.5-5.5 5.5 5.5 5.5"/>',
  home: '<path d="M4.5 10.5 12 4l7.5 6.5"/><path d="M6.5 9v9.5a1 1 0 0 0 1 1H10v-5h4v5h2.5a1 1 0 0 0 1-1V9"/>',
  workspace: '<path d="M3.5 17.5v-11a2 2 0 0 1 2-2h3.8l2 2.3h7.2a2 2 0 0 1 2 2v8.7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="m9 11.5-1.5 1.5L9 14.5M15 11.5l1.5 1.5-1.5 1.5"/>',
  // an open folder: "show this folder in File Explorer"
  folderOpen: '<path d="M3.5 8v8.5a2 2 0 0 0 2 2h11.8a1.5 1.5 0 0 0 1.4-1l1.8-5.2a1.3 1.3 0 0 0-1.2-1.8H8.6a1.5 1.5 0 0 0-1.4 1l-1.7 5"/><path d="M3.5 8V6.5a2 2 0 0 1 2-2h3.2l2 2.3h5.8a2 2 0 0 1 2 2v1.7"/>',
  list: '<rect x="4" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5"/>',
  plus: '<path d="M12 5.5v13M5.5 12h13"/>',
  copy: '<rect x="8.5" y="8.5" width="11" height="11" rx="2.5"/><path d="M15.5 8.5V6.5a2 2 0 0 0-2-2h-7a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2"/>',
  rerun: '<path d="M4.5 12a7.5 7.5 0 0 0 12.8 5.3"/><path d="M19.5 12A7.5 7.5 0 0 0 6.7 6.7"/><path d="M6.5 3v3.8h3.8M17.5 21v-3.8h-3.8"/>',
  check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
  drive: '<rect x="3" y="7" width="18" height="10" rx="2.5"/><path d="M7 12h5"/><circle cx="16.5" cy="12" r=".8" fill="currentColor"/>',
  reveal: '<path d="M13.5 4.5h6v6"/><path d="M19.5 4.5 11 13"/><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>',
  shield: '<path d="M12 3.5 5 6.5v5c0 4.5 3 7.7 7 9 4-1.3 7-4.5 7-9v-5z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
  // accounts & agent
  github: '<path d="M15 21.5v-3.6a3.3 3.3 0 0 0-.9-2.6c3-.3 6.1-1.5 6.1-6.6a5.1 5.1 0 0 0-1.4-3.5 4.8 4.8 0 0 0-.1-3.5s-1.1-.4-3.7 1.4a12.8 12.8 0 0 0-6.7 0C5.7 1.3 4.6 1.7 4.6 1.7a4.8 4.8 0 0 0-.1 3.5 5.1 5.1 0 0 0-1.4 3.5c0 5.1 3.1 6.3 6.1 6.6a3.3 3.3 0 0 0-.9 2.6v3.6"/><path d="M8.3 18.2c-3.3 1-3.3-1.6-4.7-2"/>',
  user: '<circle cx="12" cy="8.5" r="3.5"/><path d="M5 20a7 7 0 0 1 14 0"/>',
  send: '<path d="m20.5 3.5-6.6 17.3-3.2-7.5-7.5-3.2z"/><path d="m20.5 3.5-9.8 9.8"/>',
  sparkle: '<path d="m10 3 1.6 4.6a2 2 0 0 0 1.2 1.2L17.5 10.5l-4.7 1.7a2 2 0 0 0-1.2 1.2L10 18l-1.6-4.6a2 2 0 0 0-1.2-1.2L2.5 10.5l4.7-1.7a2 2 0 0 0 1.2-1.2z"/><path d="M19 15v4.5M16.75 17.25h4.5"/>',
  tool: '<path d="M14.6 6.4a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.7-3.7a6 6 0 0 1-7.9 7.9l-6.8 6.8a2.1 2.1 0 0 1-3-3l6.8-6.8a6 6 0 0 1 7.9-7.9z"/>',
  edit: '<path d="M4 20h4l10.5-10.5a2 2 0 0 0-4-4L4 16z"/><path d="m13 7 4 4"/>',
  plan: '<path d="M4.5 7h2M4.5 12h2M4.5 17h2"/><path d="M10 7h9.5M10 12h9.5M10 17h9.5"/>',
  // debugging (toolbar and Run menu)
  dbgContinue: '<path d="M5.5 5.5v13"/><path d="M9.5 6.2v11.6a.8.8 0 0 0 1.2.7l8.6-5.8a.8.8 0 0 0 0-1.4l-8.6-5.8a.8.8 0 0 0-1.2.7z" fill="currentColor"/>',
  dbgPause: '<path d="M8.5 5.5v13M15.5 5.5v13" stroke-width="2.6"/>',
  dbgStepOver: '<path d="M4.5 13a7.5 7.5 0 0 1 14.3-3.2"/><path d="M19.5 5.2v4.6h-4.6"/><circle cx="12" cy="18.2" r="2" fill="currentColor"/>',
  dbgStepInto: '<path d="M12 3.5v10.3"/><path d="m7.6 9.6 4.4 4.4 4.4-4.4"/><circle cx="12" cy="19.2" r="2" fill="currentColor"/>',
  dbgStepOut: '<path d="M12 14.2V3.8"/><path d="M7.6 8.2 12 3.8l4.4 4.4"/><circle cx="12" cy="19.2" r="2" fill="currentColor"/>',
  dbgRestart: '<path d="M5.2 12.5a7 7 0 1 0 2-5.4"/><path d="M5 3.8v4.6h4.6"/>',
  dbgStop: '<rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor"/>',
  breakpoint: '<circle cx="12" cy="12" r="5.5" fill="currentColor" stroke="none"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7"/>',
  notes: '<path d="M7 3.5h7.5L19 8v10.5a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-13a2 2 0 0 1 2-2z"/><path d="M14.5 3.5V8H19"/><path d="M8.5 12.5h7M8.5 16h4.5"/>',
  // window controls: 20px glyphs with 1.3px strokes, like Windows 11 but easier to see
  minimize: '<path d="M5.5 12h13" stroke-width="1.3"/>',
  maximize: '<rect x="5.5" y="5.5" width="13" height="13" rx="1.6" stroke-width="1.3"/>',
  restore: '<rect x="5.5" y="8.5" width="10" height="10" rx="1.6" stroke-width="1.3"/><path d="M8.5 5.5h8a2 2 0 0 1 2 2v8" stroke-width="1.3"/>',
  winClose: '<path d="m6.5 6.5 11 11M17.5 6.5l-11 11" stroke-width="1.3"/>',
};

/** 16px UI icons: `icons.close`, `icons.chevron`, … */
export const icons = Object.fromEntries(Object.entries(P).map(([k, body]) => [k, svg(body, 16)]));

/** Larger icons for the activity bar. */
export const abIcon = (name) => svg(P[name], 24);

/** Window caption glyphs (minimize / maximize / restore / close). */
export const winIcon = (name) => svg(P[name], 20);

export const logo = `<svg viewBox="0 0 1024 1024" aria-hidden="true"><path d="M370 330 L210 512 L370 694" fill="none" stroke="currentColor" stroke-width="72" stroke-linecap="round" stroke-linejoin="round"/><path d="M654 330 L814 512 L654 694" fill="none" stroke="currentColor" stroke-width="72" stroke-linecap="round" stroke-linejoin="round"/><path d="M560 250 L430 540 L530 540 L465 774 L640 470 L535 470 L600 250 Z" fill="currentColor"/></svg>`;

// ------------------------------------------------------------- file icons

// Glyphs drawn on a 16px grid, inside the 3–13 area of the tile.
const G = {
  markup: '<path d="M5.7 5.3 3.6 8l2.1 2.7M10.3 5.3 12.4 8l-2.1 2.7M8.9 4.8 7.1 11.2"/>',
  hash: '<path d="M6.8 4.4 6 11.6M10.2 4.4l-.8 7.2M4.6 6.6h7M4.3 9.4h7"/>',
  braces: '<path d="M6.4 4.4c-1 0-1.4.4-1.4 1.3v.9c0 .7-.4 1.2-1.1 1.4.7.2 1.1.7 1.1 1.4v.9c0 .9.4 1.3 1.4 1.3M9.6 4.4c1 0 1.4.4 1.4 1.3v.9c0 .7.4 1.2 1.1 1.4-.7.2-1.1.7-1.1 1.4v.9c0 .9-.4 1.3-1.4 1.3"/>',
  lines: '<path d="M5 5.6h6M5 8h6M5 10.4h3.8"/>',
  sliders: '<path d="M4.5 5.6h7M4.5 10.4h7"/><circle cx="6.6" cy="5.6" r="1.2" fill="currentColor" stroke="none"/><circle cx="9.4" cy="10.4" r="1.2" fill="currentColor" stroke="none"/>',
  image: '<path d="m3.8 11.2 2.7-3 1.9 2 1.4-1.5 2.4 2.5"/><circle cx="10.2" cy="5.8" r=".9" fill="currentColor" stroke="none"/>',
  branch: '<circle cx="5.6" cy="4.9" r="1.1"/><circle cx="5.6" cy="11.1" r="1.1"/><circle cx="10.6" cy="6.1" r="1.1"/><path d="M5.6 6v4M10.6 7.2c0 1.9-2.6 2.1-4.6 3"/>',
  lock: '<rect x="5" y="7.4" width="6" height="4.6" rx="1"/><path d="M6.3 7.4V6.2a1.7 1.7 0 0 1 3.4 0v1.2"/>',
  prompt: '<path d="m4.7 5.6 2.3 2.4-2.3 2.4M8.6 10.6h2.8"/>',
  markdown: '<path d="M3.8 10.6V5.4L5.9 8 8 5.4v5.2M10.6 5.4v5M9.2 9.2l1.4 1.5L12 9.2"/>',
  diamond: '<path d="M8 4.2 11.8 8 8 11.8 4.2 8z"/>',
  table: '<rect x="4" y="4.8" width="8" height="6.4" rx="1"/><path d="M4 8h8M8 4.8v6.4"/>',
  box: '<path d="M8 4 12 6v4L8 12 4 10V6z"/><path d="M4 6l4 2 4-2M8 8v4"/>',
  // FluxCode's own notes format (.flx): the logo's lightning bolt
  bolt: '<path d="M9.3 3.4 5.3 8.9h2.8l-1 3.7 4-5.5H8.3z" fill="currentColor" stroke="none"/>',
};

const letters = (text) => {
  const size = text.length >= 3 ? 5.6 : 7.4;
  return `<text x="8" y="${text.length >= 3 ? 10.1 : 10.7}" text-anchor="middle" font-size="${size}" font-weight="700" font-family="Segoe UI, Arial, sans-serif" fill="currentColor" stroke="none" letter-spacing="-.15">${text}</text>`;
};

// [glyph or letters, color]
const T = {
  python: [letters("Py"), "#4b9fe0"], notebook: [letters("Nb"), "#f37726"],
  js: [letters("JS"), "#e5c33a"], ts: [letters("TS"), "#3b82f6"], jsx: [letters("JSX"), "#38bdf8"], tsx: [letters("TSX"), "#38bdf8"],
  rust: [letters("Rs"), "#e2733f"], go: [letters("Go"), "#2bb3d0"], c: [letters("C"), "#5c8fd6"], cpp: [letters("C++"), "#5c8fd6"],
  cs: [letters("C#"), "#9b6fe0"], java: [letters("Jv"), "#e0614c"], kotlin: [letters("Kt"), "#b07af0"], php: [letters("php"), "#8d93d8"],
  blade: [letters("B"), "#f05340"],
  ruby: [letters("Rb"), "#e0514c"], lua: [letters("Lu"), "#5b7de8"], vue: [letters("V"), "#42b883"], svelte: [letters("S"), "#ff6a33"],
  sql: [letters("SQL"), "#e8a33d"], header: [letters("H"), "#a57ae8"],
  html: [G.markup, "#ef6c3c"], xml: [G.markup, "#f08c4a"], css: [G.hash, "#4c8ef7"], scss: [G.hash, "#e5679f"],
  json: [G.braces, "#d9b43c"], markdown: [G.markdown, "#5aa3f0"], text: [G.lines, "#8b949e"], license: [G.lines, "#d9b43c"],
  config: [G.sliders, "#9aa5b1"], env: [G.sliders, "#d9b43c"], image: [G.image, "#a57ae8"], svg: [G.diamond, "#f0a53a"],
  git: [G.branch, "#f05133"], lock: [G.lock, "#8b949e"], shell: [G.prompt, "#6cc06a"], powershell: [G.prompt, "#3b82f6"],
  batch: [G.prompt, "#c9b85a"], csv: [G.table, "#4caf6a"], docker: [G.box, "#2496ed"], package: [G.box, "#c98a4a"],
  flx: [G.bolt, "#8b7cf6"], swift: [letters("Sw"), "#f05138"], dart: [letters("Dt"), "#2bb7f6"], r: [letters("R"), "#4a8fd6"],
  font: [letters("Aa"), "#e06c9f"], pdf: [letters("PDF"), "#e5534b"], audio: [G.image, "#4caf6a"], video: [G.image, "#e5679f"],
};

const EXT = {
  py: "python", pyw: "python", pyi: "python", ipynb: "notebook",
  js: "js", mjs: "js", cjs: "js", jsx: "jsx", ts: "ts", mts: "ts", cts: "ts", tsx: "tsx",
  rs: "rust", go: "go", c: "c", h: "header", hpp: "header", cpp: "cpp", cc: "cpp", cs: "cs", java: "java", kt: "kotlin",
  php: "php", rb: "ruby", lua: "lua", vue: "vue", svelte: "svelte", sql: "sql",
  html: "html", htm: "html", xml: "xml", css: "css", scss: "scss", sass: "scss", less: "css",
  json: "json", jsonc: "json", webmanifest: "json", md: "markdown", markdown: "markdown",
  txt: "text", log: "text", toml: "config", yml: "config", yaml: "config", ini: "config", cfg: "config", conf: "config",
  env: "env", png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", ico: "image", bmp: "image", svg: "svg",
  gitignore: "git", gitattributes: "git", gitmodules: "git", lock: "lock", sh: "shell", bash: "shell", zsh: "shell",
  ps1: "powershell", bat: "batch", cmd: "batch", csv: "csv", tsv: "csv", dockerfile: "docker", zip: "package", "7z": "package",
  flx: "flx", mdx: "markdown", rst: "text", jsonl: "json", json5: "json", lockb: "lock", xhtml: "html", vue: "vue",
  swift: "swift", dart: "dart", r: "r", kts: "kotlin", scala: "java", groovy: "java", hh: "header", cxx: "cpp",
  tar: "package", gz: "package", tgz: "package", rar: "package", whl: "package", jar: "package",
  ttf: "font", otf: "font", woff: "font", woff2: "font", pdf: "pdf", mp3: "audio", wav: "audio", ogg: "audio",
  mp4: "video", mov: "video", webm: "video", avif: "image", tif: "image", tiff: "image",
  editorconfig: "config", prettierrc: "config", eslintrc: "config", babelrc: "config", npmrc: "config", nvmrc: "config", properties: "config",
  env: "env", example: "env",
};
const NAMES = { ".env.local": "env", ".env.example": "env", ".env.development": "env", ".env.production": "env", makefile: "config", "requirements.txt": "config", "pyproject.toml": "config", "package.json": "package", "tsconfig.json": "config", "jsconfig.json": "config", "launch.json": "config", "settings.json": "config", license: "license", licence: "license", "license.md": "license", "license.txt": "license", "package-lock.json": "lock", "cargo.lock": "lock", "composer.lock": "lock", dockerfile: "docker" };

const tile = (kind, [glyph, color]) =>
  `<svg class="ficon" data-kind="${kind}" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" style="color:${color}" aria-hidden="true"><rect x="1" y="1" width="14" height="14" rx="3.5" fill="currentColor" fill-opacity=".14" stroke="none"/>${glyph}</svg>`;

const cache = new Map();

/** File-type icon (16×16 tile) for a file name. */
export function fileIcon(name) {
  const lower = name.toLowerCase();
  const ext = lower.startsWith(".") && lower.indexOf(".", 1) < 0 ? lower.slice(1) : lower.includes(".") ? lower.split(".").pop() : lower;
  const kind = NAMES[lower] ?? (lower.endsWith(".blade.php") ? "blade" : EXT[ext]) ?? "text";
  let html = cache.get(kind);
  if (!html) {
    html = tile(kind, T[kind]);
    cache.set(kind, html);
  }
  return html;
}

// Well-known folders get their own color and a small glyph on the folder
// (src, tests, docs, assets, .git, .vscode, node_modules, …), so a tree reads
// at a glance; every other folder keeps the theme's folder color.
const FG = {
  code: '<path d="M6.7 8.2 5.2 9.8l1.5 1.6M9.3 8.2l1.5 1.6-1.5 1.6"/>',
  check: '<path d="m5.6 9.9 1.6 1.5 3.2-3.1"/>',
  lines: '<path d="M5.5 8.6h5M5.5 11h3.4"/>',
  image: '<path d="m5 11.6 1.9-2 1.4 1.3 1-1 1.7 1.7"/>',
  branch: '<path d="M6.4 8v3.8M6.4 9.6c1.8 0 3.2-.3 3.2-1.6"/>',
  box: '<path d="M8 7.8 10.5 9v2.3L8 12.5l-2.5-1.2V9z"/>',
  gear: '<circle cx="8" cy="10" r="1.6"/>',
  play: '<path d="M7 8.3v3.4l2.7-1.7z" fill="currentColor"/>',
  arrow: '<path d="M8 7.8v4M6.3 10.2 8 11.9l1.7-1.7"/>',
  py: '<path d="M6.2 11.8V8.4h1.6a1 1 0 0 1 0 2H6.2"/><path d="M9.5 8.4v1.9l1.2 1.5"/>',
};
const FOLDERS = {
  src: ["code", "#4c8ef7"], source: ["code", "#4c8ef7"], lib: ["code", "#4c8ef7"], app: ["code", "#4c8ef7"], core: ["code", "#4c8ef7"],
  components: ["code", "#38bdf8"], pages: ["code", "#38bdf8"], views: ["code", "#38bdf8"], routes: ["code", "#38bdf8"], api: ["code", "#38bdf8"], server: ["code", "#38bdf8"],
  test: ["check", "#4caf6a"], tests: ["check", "#4caf6a"], __tests__: ["check", "#4caf6a"], spec: ["check", "#4caf6a"], e2e: ["check", "#4caf6a"],
  docs: ["lines", "#5aa3f0"], doc: ["lines", "#5aa3f0"], examples: ["lines", "#5aa3f0"], notes: ["lines", "#5aa3f0"],
  assets: ["image", "#a57ae8"], images: ["image", "#a57ae8"], img: ["image", "#a57ae8"], icons: ["image", "#a57ae8"], media: ["image", "#a57ae8"], public: ["image", "#a57ae8"], static: ["image", "#a57ae8"], fonts: ["image", "#a57ae8"],
  styles: ["code", "#e5679f"], css: ["code", "#e5679f"], scss: ["code", "#e5679f"],
  ".git": ["branch", "#f05133"], ".github": ["branch", "#8b949e"],
  ".vscode": ["gear", "#3b82f6"], ".idea": ["gear", "#8b949e"], config: ["gear", "#9aa5b1"], ".config": ["gear", "#9aa5b1"], settings: ["gear", "#9aa5b1"], ".claude": ["gear", "#d97757"],
  node_modules: ["box", "#7cb342"], packages: ["box", "#c98a4a"], vendor: ["box", "#c98a4a"],
  dist: ["arrow", "#e5a33d"], build: ["arrow", "#e5a33d"], out: ["arrow", "#e5a33d"], target: ["arrow", "#e5a33d"], release: ["arrow", "#e5a33d"],
  scripts: ["play", "#6cc06a"], bin: ["play", "#6cc06a"], tools: ["play", "#6cc06a"],
  venv: ["py", "#4b9fe0"], ".venv": ["py", "#4b9fe0"], env: ["py", "#4b9fe0"], __pycache__: ["py", "#8b949e"],
};

const folderCache = new Map();

/** Folder icon: flat folder; known names (src, tests, .git, …) get a color and glyph. */
export function folderIcon(open, name = "") {
  const known = FOLDERS[name.toLowerCase()];
  const key = `${open ? 1 : 0}:${known ? name.toLowerCase() : ""}`;
  let html = folderCache.get(key);
  if (html) return html;
  const style = known ? ` style="color:${known[1]}"` : "";
  const cls = `folder-icon${open ? " open" : ""}${known ? " known" : ""}`;
  const glyph = known ? `<g fill="none" stroke="var(--bg-side, #1e1e1e)" stroke-width="1.15" stroke-linecap="round" stroke-linejoin="round" color="var(--bg-side, #1e1e1e)">${FG[known[0]]}</g>` : "";
  html = open
    ? `<svg class="${cls}"${style} width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M1.5 11.4V4.3c0-.8.6-1.4 1.4-1.4h3.2c.4 0 .7.1 1 .4l1 1h4.5c.8 0 1.4.6 1.4 1.4V7" fill="currentColor" fill-opacity=".55"/><path d="M3.7 7h10.5c.7 0 1.1.7.9 1.3l-1.7 4c-.2.5-.7.9-1.3.9H2.4c-.7 0-1.1-.7-.9-1.3l1.7-4c.2-.5.7-.9 1.3-.9z" fill="currentColor"/>${glyph}</svg>`
    : `<svg class="${cls}"${style} width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M1.5 4.3c0-.8.6-1.4 1.4-1.4h3.2c.4 0 .7.1 1 .4l1 1h4.9c.8 0 1.4.6 1.4 1.4v6.2c0 .8-.6 1.4-1.4 1.4H2.9c-.8 0-1.4-.6-1.4-1.4z" fill="currentColor"/><path d="M1.5 6.4h13" stroke="#000" stroke-opacity=".14" stroke-width="1"/>${glyph}</svg>`;
  folderCache.set(key, html);
  return html;
}

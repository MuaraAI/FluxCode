// FluxCode's own icon set: drawn from scratch for this project (not codicons,
// not Seti). Rounded 1.5px strokes on a 24px grid, inline SVG: no icon font,
// no network requests, ~4 KB total.

const svg = (body, size = 16, extra = "") =>
  `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"${extra}>${body}</svg>`;

const P = {
  // activity bar
  explorer: '<path d="M8.5 3.5h6l4 4v9a2 2 0 0 1-2 2h-8a2 2 0 0 1-2-2v-11a2 2 0 0 1 2-2z"/><path d="M14.5 3.5v4h4"/><path d="M4 7.5v11a2 2 0 0 0 2 2h8"/>',
  search: '<circle cx="10.5" cy="10.5" r="6"/><path d="m15 15 5 5"/><path d="M8 8.5a3 3 0 0 1 2.5-1.4" opacity=".55"/>',
  run: '<path d="M7.5 5.2v13.6a1 1 0 0 0 1.5.86l11-6.8a1 1 0 0 0 0-1.72l-11-6.8a1 1 0 0 0-1.5.86z"/><path d="M3.5 8.5v7"/>',
  settings:
    '<circle cx="12" cy="12" r="7.6" stroke-width="2.8" stroke-dasharray="2.6 3.37" stroke-linecap="butt"/><circle cx="12" cy="12" r="5.6"/><circle cx="12" cy="12" r="2.2"/>',
  // small UI glyphs
  chevron: '<path d="m9 6 6 6-6 6"/>',
  newFile: '<path d="M13 3.5H7.5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2H12"/><path d="M13 3.5l5 5V12"/><path d="M17.5 15v6M14.5 18h6"/>',
  newFolder: '<path d="M11 19.5H5a2 2 0 0 1-2-2v-11a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V11"/><path d="M17.5 14v6M14.5 17h6"/>',
  refresh: '<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"/><path d="M19.5 4v4h-4"/>',
  collapse: '<rect x="4" y="7" width="13" height="13" rx="2.5"/><path d="M8 13.5h5"/><path d="M8 3.5h9.5a3 3 0 0 1 3 3V16"/>',
  close: '<path d="m6.5 6.5 11 11M17.5 6.5l-11 11"/>',
  play: '<path d="M7.5 5.2v13.6a1 1 0 0 0 1.5.86l11-6.8a1 1 0 0 0 0-1.72l-11-6.8a1 1 0 0 0-1.5.86z" fill="currentColor"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="2.5" fill="currentColor"/>',
  clear: '<path d="M4 20h16"/><path d="m6.5 15.5 8-8a2 2 0 0 1 2.8 0l.7.7a2 2 0 0 1 0 2.8l-6.5 6.5H7z"/>',
  sidebar: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M9 4.5v15"/>',
  sidebarOn: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M5 6h4v12H5z" fill="currentColor" stroke="none"/><path d="M9 4.5v15"/>',
  panel: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M3.5 14h17"/>',
  panelOn: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><path d="M5 14h14v4H5z" fill="currentColor" stroke="none"/><path d="M3.5 14h17"/>',
  info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><circle cx="12" cy="7.8" r=".6" fill="currentColor"/>',
  error: '<circle cx="12" cy="12" r="8.5"/><path d="m9 9 6 6M15 9l-6 6"/>',
  warning: '<path d="M10.3 4.5a2 2 0 0 1 3.4 0l7 12.3a2 2 0 0 1-1.7 3H5a2 2 0 0 1-1.7-3z"/><path d="M12 9.5v4"/><circle cx="12" cy="16.8" r=".6" fill="currentColor"/>',
  bolt: '<path d="M13.5 2.5 5.5 13.5h6l-1 8 8-11h-6z" fill="currentColor" stroke="none"/>',
  bell: '<path d="M6.5 16.5V11a5.5 5.5 0 0 1 11 0v5.5l1.5 2h-14z"/><path d="M10 20.5a2 2 0 0 0 4 0"/>',
  caseSensitive: '<path d="M3.5 17 7 7.5 10.5 17M4.8 13.5h4.4"/><path d="M17 17v-1.2m0 0c-.6.9-1.5 1.4-2.5 1.4-1.3 0-2.2-.8-2.2-2s1-2 2.6-2h2.1m0 2.6V12.6c0-1.3-.8-2.1-2.2-2.1-.9 0-1.7.4-2.1 1.1"/>',
  wholeWord: '<path d="M3 19h18" opacity=".55"/><path d="M5 15.5v-6m0 3a2.5 2.5 0 1 0 5 0 2.5 2.5 0 1 0-5 0"/><path d="M19 10.5a2.5 2.5 0 1 0 0 4"/><path d="M14.5 15.5V7.5"/>',
  // terminal
  terminal: '<rect x="3" y="4.5" width="18" height="15" rx="3"/><path d="m7 10 3 2.5L7 15"/><path d="M12.5 15.5h4.5"/>',
  up: '<path d="M12 19V6"/><path d="m6.5 11 5.5-5.5 5.5 5.5"/>',
  back: '<path d="M19 12H6"/><path d="m11 6.5-5.5 5.5 5.5 5.5"/>',
  home: '<path d="M4.5 11 12 4.5l7.5 6.5"/><path d="M6.5 9.5v9a1 1 0 0 0 1 1H10v-5h4v5h2.5a1 1 0 0 0 1-1v-9"/>',
  workspace: '<path d="M3.5 17.5v-11a2 2 0 0 1 2-2h3.8l2 2.3h7.2a2 2 0 0 1 2 2v8.7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="m12.5 10.5-2 5M15 11.5l1.5 1.5L15 14.5M8.5 11.5 7 13l1.5 1.5"/>',
  list: '<rect x="3.5" y="4.5" width="7" height="6.5" rx="1.5"/><rect x="13.5" y="4.5" width="7" height="6.5" rx="1.5"/><rect x="3.5" y="13" width="7" height="6.5" rx="1.5"/><rect x="13.5" y="13" width="7" height="6.5" rx="1.5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  copy: '<rect x="8.5" y="8.5" width="11" height="11" rx="2.5"/><path d="M15.5 8.5V6a2 2 0 0 0-2-2H6.5a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2"/>',
  rerun: '<path d="M4.5 12a7.5 7.5 0 0 0 12.8 5.3"/><path d="M19.5 12A7.5 7.5 0 0 0 6.7 6.7"/><path d="M6.5 3v3.8h3.8M17.5 21v-3.8h-3.8"/>',
  check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
  drive: '<rect x="3" y="7" width="18" height="10" rx="2.5"/><path d="M7 12h5"/><circle cx="16.5" cy="12" r=".8" fill="currentColor"/>',
  reveal: '<path d="M13.5 4.5h6v6"/><path d="M19.5 4.5 11 13"/><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>',
  shield: '<path d="M12 3.5 5 6.5v5c0 4.5 3 7.7 7 9 4-1.3 7-4.5 7-9v-5z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
  // window controls (thin, 10px marks like Windows 11)
  minimize: '<path d="M6.5 12h11" stroke-width="1.1"/>',
  maximize: '<rect x="6.5" y="6.5" width="11" height="11" rx="1.5" stroke-width="1.1"/>',
  restore: '<rect x="6.5" y="8.5" width="9" height="9" rx="1.5" stroke-width="1.1"/><path d="M9 6.5h6.5a2 2 0 0 1 2 2V15" stroke-width="1.1"/>',
  winClose: '<path d="m7 7 10 10M17 7 7 17" stroke-width="1.1"/>',
};

/** 16px UI icons: `icons.close`, `icons.chevron`, … */
export const icons = Object.fromEntries(Object.entries(P).map(([k, body]) => [k, svg(body, 16)]));

/** Larger icons for the activity bar. */
export const abIcon = (name) => svg(P[name], 24);

export const logo = `<svg viewBox="0 0 1024 1024" aria-hidden="true"><path d="M370 330 L210 512 L370 694" fill="none" stroke="currentColor" stroke-width="72" stroke-linecap="round" stroke-linejoin="round"/><path d="M654 330 L814 512 L654 694" fill="none" stroke="currentColor" stroke-width="72" stroke-linecap="round" stroke-linejoin="round"/><path d="M560 250 L430 540 L530 540 L465 774 L640 470 L535 470 L600 250 Z" fill="currentColor"/></svg>`;

// ------------------------------------------------------------ file icons
// FluxCode badges: a tinted rounded tag with a short label in the type color.

const FILE_TYPES = {
  py: ["py", "#4b9fe8"], pyw: ["py", "#4b9fe8"], pyi: ["pyi", "#4b9fe8"], ipynb: ["nb", "#f08a3c"],
  rs: ["rs", "#f2784b"], toml: ["tm", "#9aa5b1"], lock: ["lk", "#7d8590"],
  js: ["js", "#e8c547"], mjs: ["js", "#e8c547"], cjs: ["js", "#e8c547"], jsx: ["jsx", "#5cc8e8"],
  ts: ["ts", "#3d8fd9"], tsx: ["tsx", "#5cc8e8"], vue: ["vue", "#4cc38a"], svelte: ["sv", "#f26b3a"],
  html: ["</>", "#ef7d4c"], htm: ["</>", "#ef7d4c"], css: ["css", "#5b8def"], scss: ["scss", "#e66ba3"],
  json: ["{ }", "#d6b84a"], md: ["md", "#6aa9f5"], txt: ["txt", "#9aa5b1"], log: ["log", "#7d8590"],
  yml: ["yml", "#b48ef0"], yaml: ["yml", "#b48ef0"], xml: ["xml", "#ef7d4c"], svg: ["svg", "#e8a33d"],
  png: ["img", "#a57ae8"], jpg: ["img", "#a57ae8"], jpeg: ["img", "#a57ae8"], gif: ["img", "#a57ae8"], ico: ["ico", "#a57ae8"],
  sh: ["sh", "#7ccf6a"], bat: ["bat", "#c9c06a"], cmd: ["cmd", "#c9c06a"], ps1: ["ps", "#4b9fe8"],
  c: ["c", "#6a9cd8"], h: ["h", "#a57ae8"], cpp: ["c++", "#6a9cd8"], cs: ["c#", "#8b6fd8"],
  go: ["go", "#4cc3d9"], java: ["jv", "#e0614c"], kt: ["kt", "#b07af0"], php: ["php", "#8d93d8"],
  rb: ["rb", "#e0514c"], lua: ["lua", "#5b7de8"], sql: ["sql", "#e8a33d"], env: ["env", "#d6b84a"],
  gitignore: ["git", "#e8664c"], dockerfile: ["dk", "#4b9fe8"], csv: ["csv", "#5cbf7a"],
};

export function fileIcon(name) {
  const lower = name.toLowerCase();
  const ext = lower.startsWith(".") && lower.indexOf(".", 1) < 0 ? lower.slice(1) : lower.includes(".") ? lower.split(".").pop() : lower;
  const [label, color] = FILE_TYPES[ext] ?? ["", "#8b949e"];
  if (!label) {
    // generic document glyph
    return `<span class="ficon generic" style="color:${color}">${svg('<path d="M7 3.5h6.5l4.5 4.5v10.5a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-13a2 2 0 0 1 2-2z"/><path d="M13.5 3.5V8H18"/>', 14)}</span>`;
  }
  return `<span class="ficon" style="--c:${color}">${label}</span>`;
}

export function folderIcon(open) {
  return open
    ? `<span class="folder-icon open">${svg('<path d="M3.5 18V6.5a2 2 0 0 1 2-2h3.8l2 2.3h6.2a2 2 0 0 1 2 2v1.7" fill="currentColor" fill-opacity=".25"/><path d="M3.5 18l2.3-6.2a1.5 1.5 0 0 1 1.4-1h13.6a1 1 0 0 1 .95 1.3L19.8 18a1.5 1.5 0 0 1-1.4 1H5a1.5 1.5 0 0 1-1.5-1z" fill="currentColor" fill-opacity=".45"/>', 16)}</span>`
    : `<span class="folder-icon">${svg('<path d="M3.5 17.5v-11a2 2 0 0 1 2-2h3.8l2 2.3h7.2a2 2 0 0 1 2 2v8.7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" fill="currentColor" fill-opacity=".35"/>', 16)}</span>`;
}

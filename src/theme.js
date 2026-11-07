// FluxCode theme engine.
//
// A theme is just three colors — background, accent and (optionally) text —
// plus font choices. Every other color (panels, borders, selections, syntax,
// terminal ANSI colors, status colors) is derived from them, and with
// "auto contrast" on each derived color is nudged until it meets a WCAG
// contrast ratio against the surface it sits on, so any picked colors stay
// readable. Everything is applied as CSS variables: no re-render, no reload.

const STORAGE_KEY = "fluxcode.appearance";

export const PRESETS = [
  { id: "flux-dark", name: "Flux Dark", bg: "#1e1f22", accent: "#3b8eea" },
  { id: "midnight", name: "Midnight", bg: "#0f1729", accent: "#38bdf8" },
  { id: "graphite", name: "Graphite", bg: "#232323", accent: "#a3e635" },
  { id: "ocean", name: "Ocean", bg: "#0d2129", accent: "#2dd4bf" },
  { id: "rosewood", name: "Rosewood", bg: "#221a20", accent: "#f472b6" },
  { id: "ember", name: "Ember", bg: "#1f1a17", accent: "#fb923c" },
  { id: "flux-light", name: "Flux Light", bg: "#f7f8fa", accent: "#2563eb" },
  { id: "paper", name: "Paper", bg: "#f4efe6", accent: "#b45309" },
];

export const DEFAULTS = {
  preset: "flux-dark",
  bg: "#1e1f22",
  accent: "#3b8eea",
  text: null, // null = automatic
  autoContrast: true,
  uiFont: "jersey", // "jersey" | "system"
  editorFont: "cascadia", // "cascadia" | "consolas" | "jersey"
  editorSize: 14,
};

// ------------------------------------------------------------ color math

const hexToRgb = (hex) => {
  const h = hex.replace("#", "");
  const n = parseInt(h.length === 3 ? h.replace(/./g, "$&$&") : h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const rgbToHex = (rgb) => `#${rgb.map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0")).join("")}`;

/** Linear blend: t = 0 → a, t = 1 → b. */
export const mix = (a, b, t) => {
  const x = hexToRgb(a);
  const y = hexToRgb(b);
  return rgbToHex(x.map((v, i) => v + (y[i] - v) * t));
};

const luminance = (hex) => {
  const [r, g, b] = hexToRgb(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** WCAG contrast ratio (1–21). */
export const contrast = (a, b) => {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

const isDark = (bg) => contrast(bg, "#ffffff") > contrast(bg, "#000000");
const alpha = (hex, pct) => `color-mix(in srgb, ${hex} ${pct}%, transparent)`;

// ---------------------------------------------------------------- palettes

const SYNTAX = {
  dark: {
    keyword: "#569cd6", control: "#c586c0", string: "#ce9178", escape: "#d7ba7d", comment: "#6a9955",
    number: "#b5cea8", function: "#dcdcaa", type: "#4ec9b0", variable: "#9cdcfe", constant: "#4fc1ff",
    punctuation: "#cccccc", tag: "#569cd6", link: "#3794ff", invalid: "#f44747",
  },
  light: {
    keyword: "#0000ff", control: "#af00db", string: "#a31515", escape: "#ee0000", comment: "#008000",
    number: "#098658", function: "#795e26", type: "#267f99", variable: "#001080", constant: "#0070c1",
    punctuation: "#333333", tag: "#800000", link: "#0066bf", invalid: "#cd3131",
  },
};

const ANSI = {
  dark: ["#3b3f47", "#ff6b6b", "#5fd38d", "#e5c07b", "#5ea8ff", "#c792ea", "#4fd1c5", "#cfd2d8",
    "#6b717c", "#ff8a80", "#7ee2a8", "#ffd68a", "#82bfff", "#dab2ff", "#7ee8dc", "#ffffff"],
  light: ["#24292f", "#cf222e", "#116329", "#7d4e00", "#0969da", "#8250df", "#1b7c83", "#57606a",
    "#6e7781", "#a40e26", "#1a7f37", "#633c01", "#218bff", "#a475f9", "#3192aa", "#424a53"],
};

const STATUS = {
  dark: { ok: "#5fd38d", err: "#ff7b72", warn: "#e5b33d", link: "#79c0ff", hit: "#ffb02e", hitFg: "#ffd28a", folder: "#d9a74a",
    tkCmd: "#82bfff", tkBuiltin: "#5fd38d", tkFlag: "#c792ea", tkStr: "#ffb38a", tkPath: "#4fd1c5", tkNum: "#b5cea8", tkOp: "#e5c07b" },
  light: { ok: "#1a7f37", err: "#cf222e", warn: "#9a6700", link: "#0969da", hit: "#d4a72c", hitFg: "#7a4d00", folder: "#b07d1a",
    tkCmd: "#0550ae", tkBuiltin: "#116329", tkFlag: "#8250df", tkStr: "#953800", tkPath: "#1b7c83", tkNum: "#0a6640", tkOp: "#7d4e00" },
};

// ------------------------------------------------------------------- derive

/** Every CSS variable of a theme, derived from the user's few choices. */
export function derive(s) {
  // Some backgrounds (mid grays, saturated mid tones) can't carry 7:1 text in
  // any color. Auto contrast nudges such a background toward black or white
  // just far enough that readable text exists.
  let bg = s.bg;
  if (s.autoContrast) {
    const reach = (c) => Math.max(contrast(c, "#ffffff"), contrast(c, "#000000"));
    const toward = contrast(s.bg, "#ffffff") > contrast(s.bg, "#000000") ? "#000000" : "#ffffff";
    for (let t = 0.05; reach(bg) < 9 && t <= 1.001; t += 0.05) bg = mix(s.bg, toward, t);
  }
  const dark = isDark(bg);
  const up = dark ? "#ffffff" : "#000000"; // toward more contrast
  // Move `fg` toward black/white until it reaches `min` contrast on `on`.
  const ensure = (fg, on, min) => {
    if (!s.autoContrast || contrast(fg, on) >= min) return fg;
    const target = isDark(on) ? "#ffffff" : "#000000";
    for (let t = 0.05; t <= 1.001; t += 0.05) {
      const c = mix(fg, target, t);
      if (contrast(c, on) >= min) return c;
    }
    return target;
  };

  const side = dark ? mix(bg, "#000000", 0.22) : mix(bg, "#000000", 0.035);
  const title = dark ? mix(bg, "#000000", 0.36) : mix(bg, "#000000", 0.07);
  const elevated = dark ? mix(bg, "#ffffff", 0.045) : mix(bg, "#ffffff", 0.7);
  const input = dark ? mix(bg, "#ffffff", 0.07) : mix(bg, "#ffffff", 0.85);
  // The text must read well on every surface it appears on.
  const surfaces = [bg, side, elevated, input];
  const onAll = (c, min) => surfaces.reduce((acc, surf) => ensure(acc, surf, min), c);
  const fgBase = s.text ?? (dark ? "#d4d7dd" : "#24292f");
  const fg = onAll(fgBase, 7);
  const fgStrong = onAll(mix(fg, up, 0.55), 10);
  const fgMuted = ensure(mix(fg, bg, 0.3), side, 4.5);
  const fgDim = ensure(mix(fg, bg, 0.5), side, 3.2);
  const accent = ensure(s.accent, side, 3);
  // Buttons and the status bar put white text on the accent.
  let accentStrong = accent;
  for (let t = 0; contrast("#ffffff", accentStrong) < 4.5 && t <= 1; t += 0.05) accentStrong = mix(accent, "#000000", t);

  const v = {
    "--bg-editor": bg,
    "--bg-side": side,
    "--bg-title": title,
    "--bg-elevated": elevated,
    "--bg-input": input,
    "--bg-input-hover": mix(input, up, 0.05),
    "--bg-badge": mix(side, up, 0.12),
    "--bg-select": mix(bg, accent, dark ? 0.28 : 0.18),
    "--border": mix(bg, up, dark ? 0.08 : 0.1),
    "--border-strong": mix(bg, up, dark ? 0.15 : 0.2),
    "--fg": fg,
    "--fg-strong": fgStrong,
    "--fg-muted": fgMuted,
    "--fg-dim": fgDim,
    "--accent": accent,
    "--accent-strong": accentStrong,
    "--accent-soft": alpha(accent, 16),
    "--watermark": mix(bg, up, 0.06),
    "--line-hl": mix(bg, up, 0.03),
    "--sel": mix(bg, accent, dark ? 0.38 : 0.28),
    "--sel-dim": mix(bg, up, dark ? 0.12 : 0.1),
    "--caret": ensure(accent, bg, 4.5),
  };
  const st = STATUS[dark ? "dark" : "light"];
  for (const [k, c] of Object.entries(st)) {
    const name = k.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
    v[`--${name}`] = k === "hit" ? c : ensure(c, side, k === "folder" ? 2.5 : 4.5);
  }
  for (const [k, c] of Object.entries(SYNTAX[dark ? "dark" : "light"])) {
    v[`--tk-${k}`] = ensure(c, bg, k === "comment" ? 3.8 : 4.5);
  }
  ANSI[dark ? "dark" : "light"].forEach((c, i) => { v[`--ansi${i}`] = ensure(c, side, 3); });

  // Fonts: "jersey" = Jersey 15 (the most legible of the family, also when blurred), Jersey 25 for the wordmark;
  // "system" = the VS Code default (Segoe UI). Code always stays monospaced.
  const jersey = s.uiFont === "jersey";
  v["--font-ui"] = jersey ? '"Jersey 15", "Segoe UI Variable Text", "Segoe UI", sans-serif' : '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif';
  v["--font-title"] = "var(--font-ui)";
  v["--font-display"] = jersey ? '"Jersey 25", var(--font-ui)' : "var(--font-ui)";
  v["--font-small"] = "var(--font-ui)";
  v["--ui-adjust"] = jersey ? "0.53" : "none";
  v["--editor-font"] = {
    cascadia: '"Cascadia Code", Consolas, "Courier New", monospace',
    consolas: 'Consolas, "Cascadia Code", "Courier New", monospace',
    jersey: '"Jersey 15", Consolas, monospace',
  }[s.editorFont] ?? '"Cascadia Code", Consolas, monospace';
  v["--code-adjust"] = s.editorFont === "jersey" ? "0.53" : "none";
  v["--editor-font-size"] = `${s.editorSize}px`;
  v["--editor-line-height"] = `${Math.round(s.editorSize * 1.45)}px`;
  return { vars: v, dark };
}

// ------------------------------------------------------------------- apply

export function loadAppearance() {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") };
  } catch {
    return { ...DEFAULTS };
  }
}

export function saveAppearance(s) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(s)); } catch {}
}

/** Apply a theme to the whole app (CSS variables only). */
export function applyAppearance(s) {
  const { vars, dark } = derive(s);
  const root = document.documentElement;
  for (const [k, val] of Object.entries(vars)) root.style.setProperty(k, val);
  root.style.colorScheme = dark ? "dark" : "light";
  root.dataset.theme = dark ? "dark" : "light";
  return vars;
}

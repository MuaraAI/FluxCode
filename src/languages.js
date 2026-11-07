// Language registry. Each language is loaded lazily (its own small chunk) the
// first time a matching file is opened, so startup only pays for the core.
// Completion data (Emmet, modules, packages, snippets) is loaded with it.
//
// `run` says what F5 does: "python" (python -u), "node" (node), "preview"
// (Live Preview in FluxCode). Adding a language is one entry.

import { completionExtensions } from "./complete/index.js";

const LANGUAGES = [
  {
    id: "python", name: "Python", extensions: ["py", "pyw", "pyi"], indent: 4, run: "python",
    load: () => import("@codemirror/lang-python").then((m) => m.python()),
  },
  {
    id: "javascript", name: "JavaScript", extensions: ["js", "mjs", "cjs"], indent: 2, run: "node",
    load: () => import("@codemirror/lang-javascript").then((m) => m.javascript()),
  },
  {
    id: "jsx", name: "JavaScript JSX", extensions: ["jsx"], indent: 2,
    load: () => import("@codemirror/lang-javascript").then((m) => m.javascript({ jsx: true })),
  },
  {
    // Node 23.6+ runs .ts directly (type stripping).
    id: "typescript", name: "TypeScript", extensions: ["ts", "mts", "cts"], indent: 2, run: "node",
    load: () => import("@codemirror/lang-javascript").then((m) => m.javascript({ typescript: true })),
  },
  {
    id: "tsx", name: "TypeScript JSX", extensions: ["tsx"], indent: 2,
    load: () => import("@codemirror/lang-javascript").then((m) => m.javascript({ jsx: true, typescript: true })),
  },
  {
    id: "html", name: "HTML", extensions: ["html", "htm"], indent: 2, run: "preview",
    load: () => import("@codemirror/lang-html").then((m) => m.html()),
  },
  {
    id: "css", name: "CSS", extensions: ["css", "scss", "less"], indent: 2, run: "preview",
    load: () => import("@codemirror/lang-css").then((m) => m.css()),
  },
  {
    id: "php", name: "PHP", extensions: ["php", "phtml"], indent: 4,
    load: () => import("@codemirror/lang-php").then((m) => m.php()),
  },
  {
    id: "json", name: "JSON", extensions: ["json", "jsonc", "webmanifest"], indent: 2,
    load: () => import("@codemirror/lang-json").then((m) => m.json()),
  },
  {
    id: "markdown", name: "Markdown", extensions: ["md", "markdown"], indent: 2,
    load: () => import("@codemirror/lang-markdown").then((m) => m.markdown()),
  },
  {
    id: "rust", name: "Rust", extensions: ["rs"], indent: 4,
    load: () => import("@codemirror/lang-rust").then((m) => m.rust()),
  },
];

const byExtension = new Map();
for (const lang of LANGUAGES) for (const ext of lang.extensions) byExtension.set(ext, lang);

const loaded = new Map();

/** Language descriptor for a file name, or a plain-text fallback. */
export function languageFor(name) {
  const lower = (name ?? "").toLowerCase();
  const ext = lower.split(".").pop();
  const lang = byExtension.get(ext) ?? { id: "plaintext", name: "Plain Text", indent: 4, load: null };
  return lower.endsWith(".blade.php") ? { ...lang, name: "Blade" } : lang;
}

/**
 * CodeMirror extensions for a language (cached after the first load).
 * `name` (file name) picks file-specific completion, e.g. package.json keys.
 */
export async function loadLanguage(lang, name = "") {
  if (!lang.load) return [];
  const lower = name.toLowerCase();
  const key = lang.id === "json" || lang.id === "php" ? `${lang.id}:${lower.split(/[\\/]/).pop()}` : lang.id;
  if (!loaded.has(key)) {
    loaded.set(key, lang.load().then(async (support) => [support, await completionExtensions(lang.id, support, lower.split(/[\\/]/).pop())]));
  }
  return loaded.get(key);
}

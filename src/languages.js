// Language registry. Each language is loaded lazily (its own small chunk) the
// first time a matching file is opened, so startup only pays for the core.
//
// `run` says what F5 does: "python" (python -u), "node" (node), "preview"
// (Live Preview in FluxCode). Adding a language is one entry.

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
  const ext = (name ?? "").toLowerCase().split(".").pop();
  return byExtension.get(ext) ?? { id: "plaintext", name: "Plain Text", indent: 4, load: null };
}

/** CodeMirror extension for a language (cached after the first load). */
export async function loadLanguage(lang) {
  if (!lang.load) return [];
  if (!loaded.has(lang.id)) loaded.set(lang.id, lang.load());
  return loaded.get(lang.id);
}

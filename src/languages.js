// Language registry. Each language is loaded lazily (its own small chunk) the
// first time a matching file is opened, so startup only pays for the core.
//
// Roadmap: Python (now) -> Rust -> HTML/CSS/JS. Adding a language is one entry.

const LANGUAGES = [
  {
    id: "python",
    name: "Python",
    extensions: ["py", "pyw", "pyi"],
    indent: 4,
    load: () => import("@codemirror/lang-python").then((m) => m.python()),
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

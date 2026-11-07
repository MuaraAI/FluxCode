// Completion for every language: wires the per-language sources (Emmet, tag
// names, modules, packages, snippets) and document words into CodeMirror.
// Each language only loads the data it needs, on first use.

import { completeAnyWord } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";

export { setWorkspaceManifests } from "./workspace.js";

/** Document words (like VS Code's word-based suggestions), ranked below everything else; not inside strings or comments. */
const wordSource = (context) => {
  const node = syntaxTree(context.state).resolveInner(context.pos, -1);
  if (/String|Comment|Template/.test(node.name)) return null;
  const r = completeAnyWord(context);
  if (!r) return null;
  return { ...r, options: r.options.map((o) => ({ ...o, type: "text", boost: -50 })) };
};

/**
 * Extra completion sources for a language id, as language-data extensions
 * for `support.language`. `name` is the file name (lower-cased).
 */
export async function completionExtensions(id, support, name) {
  const data = (obj) => support.language.data.of(obj);
  const list = [];
  const add = (fn) => list.push(data({ autocomplete: fn }));
  switch (id) {
    case "html": {
      const { htmlTextCompletion } = await import("./html.js");
      const { cssEmmetCompletion } = await import("./css.js");
      add(htmlTextCompletion);
      list.push(data({ emmet: "html" }));
      // <style> and <script> blocks inside HTML are the CSS / JS languages: give them their sources too.
      const { cssLanguage } = await import("@codemirror/lang-css");
      list.push(cssLanguage.data.of({ autocomplete: cssEmmetCompletion, emmet: "css" }));
      const { javascriptLanguage } = await import("@codemirror/lang-javascript");
      const { javascriptCompletion } = await import("./javascript.js");
      for (const s of javascriptCompletion(false)) list.push(javascriptLanguage.data.of({ autocomplete: s }));
      break;
    }
    case "css": {
      const { cssEmmetCompletion } = await import("./css.js");
      add(cssEmmetCompletion);
      list.push(data({ emmet: "css" }));
      break;
    }
    case "javascript": case "jsx": case "typescript": case "tsx": {
      const { javascriptCompletion } = await import("./javascript.js");
      for (const s of javascriptCompletion(id.startsWith("t"))) add(s);
      break;
    }
    case "python": {
      const { pythonCompletion } = await import("./python.js");
      for (const s of pythonCompletion()) add(s);
      break;
    }
    case "php": {
      const { phpCompletion } = await import("./php.js");
      for (const s of phpCompletion(name.endsWith(".blade.php"))) add(s);
      break;
    }
    case "json": {
      const { jsonCompletion } = await import("./json.js");
      for (const s of jsonCompletion(name)) add(s);
      break;
    }
  }
  if (id !== "json") add(wordSource);
  return list;
}

/** Tab with no completion open: expand an Emmet abbreviation (HTML text or CSS declaration). */
export async function loadEmmetTab() {
  const [{ expandHtmlAtCursor }, { expandCssAtCursor }] = await Promise.all([import("./html.js"), import("./css.js")]);
  return (view) => {
    const kind = view.state.languageDataAt("emmet", view.state.selection.main.head)[0];
    if (kind === "html") return expandHtmlAtCursor(view);
    if (kind === "css") return expandCssAtCursor(view);
    return false;
  };
}

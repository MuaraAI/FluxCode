// CSS: Emmet abbreviations inside rule blocks (`m10` → margin: 10px;).
// Property and value names come from @codemirror/lang-css.

import { snippet } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { expandCss, previewOf, cssAbbreviations } from "./emmet.js";

const ABBRS = Object.entries(cssAbbreviations).filter(([k]) => /^[a-z]/.test(k));

/** Inside a `{ … }` block, at the start of a declaration (not in a value, string or comment). */
function inDeclarationStart(state, pos) {
  const line = state.doc.lineAt(pos);
  const before = line.text.slice(0, pos - line.from);
  // Only where a property would start: line start or after `;` / `{`, then the abbreviation itself.
  if (!/(^|[;{])\s*[@a-zA-Z][\w:.#%-]*!?$/.test(before)) return false;
  let inBlock = false;
  for (let n = syntaxTree(state).resolveInner(pos, -1); n; n = n.parent) {
    if (n.name === "Comment" || n.name === "StringLiteral") return false;
    if (n.name === "Block") inBlock = true;
  }
  if (inBlock) return true;
  // The block may still be open (no `}` yet): count braces instead.
  const text = state.sliceDoc(Math.max(0, pos - 20000), pos);
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") depth--;
  }
  return depth > 0;
}

export function cssEmmetCompletion(context) {
  const word = context.matchBefore(/[@a-zA-Z][\w:.#%-]*!?$|[@/]$/);
  if (!word) return null;
  if (!inDeclarationStart(context.state, context.pos) && !word.text.startsWith("@")) return null;
  const text = word.text;
  const options = [];
  const tpl = expandCss(text);
  if (tpl) options.push({ label: text, type: "snippet", detail: previewOf(tpl).split("\n")[0], boost: 95, apply: (v, c, from, to) => snippet(tpl)(v, c, from, to) });
  // Alphabetic abbreviations that start the same way ("fx" → flex, flex-direction …).
  if (/^[a-z]/.test(text) && text.length <= 4 && !/[\d.#:!]/.test(text)) {
    for (const [key, val] of ABBRS) {
      if (key === text || !key.startsWith(text)) continue;
      const t = expandCss(key);
      if (!t || t.startsWith("@")) continue;
      options.push({ label: key, type: "snippet", detail: previewOf(val).replace(":", ": "), apply: (v, c, from, to) => snippet(t)(v, c, from, to) });
      if (options.length > 24) break;
    }
  }
  return options.length ? { from: word.from, options, validFor: /^[@a-zA-Z][\w:.#%-]*!?$/ } : null;
}

/** Tab in CSS: expand the abbreviation before the cursor. */
export function expandCssAtCursor(view) {
  const { state } = view;
  const sel = state.selection.main;
  if (!sel.empty) return false;
  const line = state.doc.lineAt(sel.head);
  const m = /[@a-zA-Z][\w:.#%-]*!?$/.exec(line.text.slice(0, sel.head - line.from));
  if (!m || !inDeclarationStart(state, sel.head)) return false;
  const tpl = expandCss(m[0]);
  if (!tpl) return false;
  snippet(tpl)(view, null, sel.head - m[0].length, sel.head);
  return true;
}

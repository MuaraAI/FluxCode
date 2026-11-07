// HTML completion in text context: Emmet abbreviations and tag names.
// (Inside a tag, @codemirror/lang-html already completes tag and attribute names.)

import { snippet } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { expandHtml, looksLikeHtmlAbbr, previewOf } from "./emmet.js";

// tag → short description
const TAGS = {
  html: "root element", head: "document metadata", body: "document body", title: "document title", meta: "metadata", link: "external resource", style: "CSS", script: "JavaScript", base: "base URL", noscript: "fallback without JS",
  header: "introductory content", nav: "navigation links", main: "main content", section: "thematic section", article: "self-contained content", aside: "tangential content", footer: "footer", address: "contact info",
  h1: "heading 1", h2: "heading 2", h3: "heading 3", h4: "heading 4", h5: "heading 5", h6: "heading 6", hgroup: "heading group",
  div: "generic block", span: "generic inline", p: "paragraph", br: "line break", hr: "thematic break", pre: "preformatted text", blockquote: "quotation", ol: "ordered list", ul: "unordered list", li: "list item", dl: "description list", dt: "term", dd: "description", figure: "figure", figcaption: "figure caption", menu: "menu list",
  a: "link", em: "emphasis", strong: "importance", small: "side comment", s: "strikethrough", cite: "citation", q: "inline quote", dfn: "definition", abbr: "abbreviation", data: "machine-readable value", time: "date/time", code: "code", var: "variable", samp: "sample output", kbd: "keyboard input", sub: "subscript", sup: "superscript", i: "italic", b: "bold", u: "underline", mark: "highlight", bdi: "bidi isolate", bdo: "bidi override", wbr: "line-break opportunity", ruby: "ruby annotation", rt: "ruby text", rp: "ruby parenthesis",
  img: "image", picture: "responsive image", source: "media source", video: "video", audio: "audio", track: "text track", map: "image map", area: "map area", iframe: "inline frame", embed: "embedded content", object: "external object", param: "object parameter", canvas: "drawing canvas", svg: "vector graphic",
  table: "table", caption: "table caption", colgroup: "column group", col: "column", thead: "table head", tbody: "table body", tfoot: "table foot", tr: "table row", th: "header cell", td: "data cell",
  form: "form", label: "label", input: "input control", button: "button", select: "drop-down", datalist: "input options", optgroup: "option group", option: "option", textarea: "multi-line input", output: "calculation result", progress: "progress bar", meter: "gauge", fieldset: "form group", legend: "group caption",
  details: "disclosure widget", summary: "disclosure summary", dialog: "dialog box", template: "inert template", slot: "shadow DOM slot",
};
const TAG_NAMES = Object.keys(TAGS);
const KNOWN = new Set(TAG_NAMES);

const applySnippet = (tpl) => (view, completion, from, to) => snippet(tpl)(view, completion, from, to);

/** True when the cursor sits in text (not inside a tag, attribute, comment, script or style). */
function inTextContext(state, pos) {
  const node = syntaxTree(state).resolveInner(pos, -1);
  for (let n = node; n; n = n.parent) {
    const name = n.name;
    if (name === "OpenTag" || name === "CloseTag" || name === "SelfClosingTag" || name === "AttributeValue" || name === "AttributeName" || name === "TagName" || name === "Comment" || name === "ScriptText" || name === "StyleText") return false;
    if (name === "Text" || name === "Element" || name === "Document") return true;
  }
  return true;
}

/**
 * The abbreviation ending at `pos`: no whitespace, `{text}` may contain spaces,
 * and anything belonging to a preceding tag (`<body>ul>li` → `ul>li`) is cut off.
 */
function abbrBefore(state, pos) {
  const line = state.doc.lineAt(pos);
  const before = line.text.slice(0, pos - line.from);
  const m = /(?:\{[^}]*\}|[^\s<{}])+$/.exec(before);
  if (!m) return null;
  let text = m[0];
  let from = pos - text.length;
  if (before[m.index - 1] === "<") {
    const cut = text.indexOf(">");
    if (cut < 0) return null;
    from += cut + 1;
    text = text.slice(cut + 1);
  }
  return text ? { from, text } : null;
}

/** Emmet abbreviations + tag names in text content. */
export function htmlTextCompletion(context) {
  const { state, pos } = context;
  const word = abbrBefore(state, pos);
  if (!word && !context.explicit) return null;
  const text = word?.text ?? "";
  if (!inTextContext(state, pos)) return null;
  const from = word?.from ?? pos;
  const options = [];
  const first = /^[a-zA-Z][\w-]*/.exec(text)?.[0]?.toLowerCase() ?? "";
  const bare = /^[a-zA-Z][\w-]*$/.test(text);
  // 1. The whole abbreviation (if it is one), expanded.
  if (text && looksLikeHtmlAbbr(text, KNOWN) && !(bare && KNOWN.has(text.toLowerCase()) && text.toLowerCase() !== "html")) {
    const tpl = expandHtml(text);
    if (tpl) options.push({ label: text, type: "snippet", detail: "Emmet", boost: 90, info: () => preview(tpl), apply: applySnippet(tpl) });
  }
  // 2. Tag names that start with the typed letters ("ht" → html, header …), each expanding like Emmet.
  if (bare || context.explicit) {
    for (const tag of TAG_NAMES) {
      if (first && !tag.startsWith(first)) continue;
      const tpl = expandHtml(tag);
      options.push({ label: tag, type: "type", detail: TAGS[tag], boost: tag === first ? 80 : 0, apply: applySnippet(tpl), info: () => preview(tpl) });
    }
  }
  if (!text && context.explicit) options.push({ label: "!", type: "snippet", detail: "HTML boilerplate", boost: 99, apply: applySnippet(expandHtml("!")) });
  return options.length ? { from, options, validFor: /^(?:\{[^}]*\}|[^\s<{}])*$/ } : null;
}

function preview(tpl) {
  const el = document.createElement("pre");
  el.className = "cm-emmet-preview";
  el.textContent = previewOf(tpl);
  return el;
}

/** Tab in HTML text: expand the abbreviation before the cursor (like VS Code's Emmet). */
export function expandHtmlAtCursor(view) {
  const { state } = view;
  const sel = state.selection.main;
  if (!sel.empty) return false;
  const word = abbrBefore(state, sel.head);
  if (!word || !inTextContext(state, sel.head)) return false;
  if (!looksLikeHtmlAbbr(word.text, KNOWN)) return false;
  const tpl = expandHtml(word.text);
  if (!tpl) return false;
  snippet(tpl)(view, null, word.from, sel.head);
  return true;
}

export { KNOWN as htmlTags };

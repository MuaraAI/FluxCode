// CodeMirror 6 setup with a VS Code "Dark Modern" look and VS Code keybindings.
// One EditorView is shared by all tabs; each tab keeps its own EditorState.

import {
  EditorView, ViewPlugin, Decoration, keymap, lineNumbers, highlightActiveLineGutter,
  highlightSpecialChars, drawSelection, dropCursor, rectangularSelection, crosshairCursor,
  highlightActiveLine,
} from "@codemirror/view";
import { EditorState, RangeSetBuilder, Compartment, StateEffect, Prec } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab, selectLine } from "@codemirror/commands";
import {
  indentOnInput, bracketMatching, foldGutter, foldKeymap, syntaxHighlighting, HighlightStyle,
  indentUnit, getIndentUnit,
} from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches, gotoLine } from "@codemirror/search";
import {
  autocompletion, closeBrackets, closeBracketsKeymap, acceptCompletion, completionStatus, hasNextSnippetField, nextSnippetField, startCompletion,
} from "@codemirror/autocomplete";
import { tags as t } from "@lezer/highlight";
import { loadEmmetTab } from "./complete/index.js";
import { agentDiffField, agentDiffTheme } from "./agentdiff.js";

// ------------------------------------------------------------------ theme

const fgMix = (pct) => `color-mix(in srgb, var(--fg-strong) ${pct}%, transparent)`;

// Every color is a CSS variable from theme.js, so a theme change restyles the
// editor instantly (no reconfigure, no re-render of the document).
const theme = EditorView.theme({
    "&": { color: "var(--fg)", backgroundColor: "var(--bg-editor)", height: "100%" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": {
      fontFamily: "var(--editor-font)",
      fontSize: "var(--editor-font-size)",
      lineHeight: "var(--editor-line-height)",
      fontVariantLigatures: "none",
    },
    ".cm-content": { caretColor: "var(--caret)", padding: "4px 0" },
    ".cm-line": { padding: "0 0 0 2px" },
    ".cm-cursor, .cm-dropCursor": { borderLeft: "2px solid var(--caret)" },
    // Smooth caret: fade instead of hard blink, and glide between positions.
    "&.cm-focused > .cm-scroller > .cm-cursorLayer": { animation: "ease-in-out cm-blink 1.1s infinite" },
    ".cm-cursor": { transition: "left 70ms ease-out, top 70ms ease-out" },
    ".cm-selectionBackground": { backgroundColor: "var(--sel-dim)" },
    "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground": { backgroundColor: "var(--sel)" },
    ".cm-content ::selection": { backgroundColor: "transparent" },
    ".cm-activeLine": { backgroundColor: "var(--line-hl)" },
    ".cm-gutters": { backgroundColor: "var(--bg-editor)", color: "var(--fg-dim)", border: "none" },
    // Numbers are zero-padded (01, 02 … 10) so every line number has the same width and column.
    ".cm-lineNumbers .cm-gutterElement": { padding: "0 6px 0 18px", minWidth: "44px", fontVariantNumeric: "tabular-nums" },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--fg-strong)" },
    ".cm-foldGutter .cm-gutterElement": { color: "var(--fg-muted)", padding: "0 4px", cursor: "pointer", opacity: "0", transition: "opacity .1s" },
    ".cm-gutters:hover .cm-foldGutter .cm-gutterElement": { opacity: "1" },
    ".cm-foldPlaceholder": { backgroundColor: "transparent", border: "none", color: "var(--fg-muted)", padding: "0 4px" },
    ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--accent) 18%, transparent)" },
    "&.cm-focused .cm-matchingBracket": { backgroundColor: "color-mix(in srgb, var(--ok) 14%, transparent)", outline: "1px solid var(--fg-dim)" },
    "&.cm-focused .cm-nonmatchingBracket": { color: "var(--err)" },
    ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--hit) 22%, transparent)", outline: "1px solid color-mix(in srgb, var(--hit) 45%, transparent)", borderRadius: "2px" },
    ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--hit) 48%, transparent)" },
    ".cm-specialChar": { color: "var(--err)" },
    ".cm-panels": { backgroundColor: "var(--bg-elevated)", color: "var(--fg)" },
    ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--border-strong)", boxShadow: "0 6px 20px rgba(0,0,0,.25)" },
    ".cm-panels.cm-panels-bottom": { borderTop: "1px solid var(--border-strong)" },
    ".cm-panel input, .cm-panel button, .cm-panel label": { fontFamily: "var(--font-ui)", fontSize: "12px" },
    ".cm-panel.cm-search": { padding: "8px 10px", animation: "pop-in .14s cubic-bezier(.2,.7,.2,1)" },
    ".cm-panel.cm-search [name=close]": { color: "var(--fg-muted)", fontSize: "18px", top: "6px", right: "8px" },
    ".cm-textfield": { backgroundColor: "var(--bg-input)", border: "1px solid var(--border-strong)", color: "var(--fg-strong)", borderRadius: "6px", padding: "4px 8px" },
    ".cm-textfield:focus": { borderColor: "var(--accent)", outline: "none", boxShadow: "0 0 0 3px var(--accent-soft)" },
    ".cm-button": { backgroundImage: "none", backgroundColor: "var(--bg-input)", border: "1px solid var(--border-strong)", color: "var(--fg)", borderRadius: "6px", padding: "3px 10px" },
    ".cm-button:hover": { backgroundColor: "var(--bg-input-hover)" },
    ".cm-tooltip": { backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-strong)", color: "var(--fg)", borderRadius: "8px", overflow: "hidden", boxShadow: "0 8px 24px rgba(0,0,0,.3)" },
    // Suggest widget: VS Code-like rows with a type icon, the label, and a muted detail; cheap pop animation.
    ".cm-tooltip-autocomplete": { animation: "cm-suggest-in .09s cubic-bezier(.2,.7,.2,1)" },
    ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--editor-font)", fontSize: "calc(var(--editor-font-size) - 1px)", maxHeight: "13em", minWidth: "280px", maxWidth: "min(560px, 70vw)", padding: "3px" },
    ".cm-tooltip-autocomplete > ul > li": { padding: "2px 8px 2px 6px", borderRadius: "4px", display: "flex", alignItems: "center", gap: "6px", lineHeight: "1.55", overflow: "hidden" },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--bg-select)", color: "var(--fg-strong)" },
    ".cm-completionLabel": { whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", flex: "none", maxWidth: "60%" },
    ".cm-completionDetail": { marginLeft: "auto", paddingLeft: "12px", fontStyle: "normal", color: "var(--fg-dim)", fontSize: "0.86em", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", fontFamily: "var(--font-ui)", flex: "0 1 auto", minWidth: "0" },
    ".cm-completionMatchedText": { textDecoration: "none", color: "var(--accent)", fontWeight: "600" },
    ".cm-completionIcon": { width: "16px", height: "16px", flex: "none", borderRadius: "3px", fontSize: "9px", fontFamily: "var(--font-ui)", fontWeight: "700", display: "grid", placeItems: "center", opacity: "1", padding: "0", boxSizing: "border-box", color: "#fff" },
    ".cm-completionIcon::after": { content: "''" },
    ".cm-completionIcon-keyword": { backgroundColor: "#b180d7" }, ".cm-completionIcon-keyword::after": { content: "'K'" },
    ".cm-completionIcon-variable": { backgroundColor: "#4fa3e3" }, ".cm-completionIcon-variable::after": { content: "'V'" },
    ".cm-completionIcon-function, .cm-completionIcon-method": { backgroundColor: "#b180d7" }, ".cm-completionIcon-function::after": { content: "'F'" }, ".cm-completionIcon-method::after": { content: "'M'" },
    ".cm-completionIcon-class": { backgroundColor: "#ee9d28" }, ".cm-completionIcon-class::after": { content: "'C'" },
    ".cm-completionIcon-type, .cm-completionIcon-interface, .cm-completionIcon-enum": { backgroundColor: "#ee9d28" }, ".cm-completionIcon-type::after": { content: "'T'" }, ".cm-completionIcon-interface::after": { content: "'I'" }, ".cm-completionIcon-enum::after": { content: "'E'" },
    ".cm-completionIcon-property": { backgroundColor: "#4fa3e3" }, ".cm-completionIcon-property::after": { content: "'P'" },
    ".cm-completionIcon-constant": { backgroundColor: "#4fa3e3" }, ".cm-completionIcon-constant::after": { content: "'K'" },
    ".cm-completionIcon-namespace, .cm-completionIcon-module": { backgroundColor: "#4ec9b0" }, ".cm-completionIcon-namespace::after": { content: "'N'" }, ".cm-completionIcon-module::after": { content: "'M'" },
    ".cm-completionIcon-snippet": { backgroundColor: "#3f8f6a" }, ".cm-completionIcon-snippet::after": { content: "'S'" },
    ".cm-completionIcon-text": { backgroundColor: "color-mix(in srgb, var(--fg-dim) 60%, transparent)" }, ".cm-completionIcon-text::after": { content: "'abc'", fontSize: "7px" },
    ".cm-tooltip.cm-completionInfo": { padding: "8px 10px", maxWidth: "min(460px, 60vw)", fontFamily: "var(--font-ui)", fontSize: "12px", marginLeft: "4px" },
    ".cm-emmet-preview": { margin: "0", fontFamily: "var(--editor-font)", fontSize: "12px", whiteSpace: "pre", overflow: "auto", maxHeight: "16em", color: "var(--fg)" },
    ".cm-snippetField": { backgroundColor: "color-mix(in srgb, var(--accent) 16%, transparent)", borderRadius: "2px" },
    ".cm-indent-guides": {
      backgroundImage: `repeating-linear-gradient(to right, ${fgMix(10)} 0 1px, transparent 1px calc(var(--unit) * 1ch))`,
      backgroundSize: "calc(var(--guides) * var(--unit) * 1ch) 100%",
      backgroundRepeat: "no-repeat",
      backgroundPosition: "2px 0",
    },
  });

// Token colors: VS Code Dark+/Light+ hues, contrast-adjusted by theme.js.
const tk = (name) => `var(--tk-${name})`;
const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.definitionKeyword, t.modifier, t.operatorKeyword, t.bool, t.null, t.self, t.atom], color: tk("keyword") },
  { tag: [t.controlKeyword, t.moduleKeyword], color: tk("control") },
  { tag: [t.string, t.special(t.string), t.character, t.attributeValue], color: tk("string") },
  { tag: [t.escape, t.regexp], color: tk("escape") },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: tk("comment") },
  { tag: [t.number, t.integer, t.float], color: tk("number") },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.function(t.definition(t.variableName)), t.macroName, t.meta], color: tk("function") },
  { tag: [t.className, t.definition(t.className), t.typeName, t.namespace], color: tk("type") },
  { tag: [t.variableName, t.propertyName, t.attributeName, t.labelName], color: tk("variable") },
  { tag: [t.constant(t.variableName), t.standard(t.variableName)], color: tk("constant") },
  { tag: [t.operator, t.punctuation, t.separator, t.paren, t.squareBracket, t.brace, t.derefOperator], color: tk("punctuation") },
  { tag: t.tagName, color: tk("tag") },
  { tag: t.heading, color: tk("keyword"), fontWeight: "bold" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: t.link, color: tk("link"), textDecoration: "underline" },
  { tag: t.invalid, color: tk("invalid") },
]);

// ---------------------------------------------------------- indent guides
// Only the visible lines are decorated, so cost does not grow with file size.

const guideDecos = new Map();
function guideDeco(levels, unit) {
  const key = levels * 100 + unit;
  let deco = guideDecos.get(key);
  if (!deco) {
    deco = Decoration.line({ class: "cm-indent-guides", attributes: { style: `--guides:${levels};--unit:${unit}` } });
    guideDecos.set(key, deco);
  }
  return deco;
}

/** Indentation width in columns, or -1 for a blank line. */
function indentColumns(text, tabSize) {
  let col = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    if (ch === 32) col++;
    else if (ch === 9) col += tabSize - (col % tabSize);
    else return col;
  }
  return -1;
}

function buildGuides(view) {
  const builder = new RangeSetBuilder();
  const { doc, tabSize } = view.state;
  const unit = getIndentUnit(view.state) || 4;
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = doc.lineAt(pos);
      let cols = indentColumns(line.text, tabSize);
      if (cols < 0) {
        // Blank line: continue the guides of the next code line (like VS Code).
        cols = 0;
        for (let n = line.number + 1, end = Math.min(doc.lines, line.number + 200); n <= end; n++) {
          const c = indentColumns(doc.line(n).text, tabSize);
          if (c >= 0) { cols = c; break; }
        }
      }
      const levels = Math.floor(cols / unit);
      if (levels > 0) builder.add(line.from, line.from, guideDeco(levels, unit));
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

const indentGuides = ViewPlugin.fromClass(
  class {
    constructor(view) { this.decorations = buildGuides(view); }
    update(u) { if (u.docChanged || u.viewportChanged) this.decorations = buildGuides(u.view); }
  },
  { decorations: (v) => v.decorations },
);

// ------------------------------------------------- search-view highlights

const isWordChar = (c) => !!c && /[\p{L}\p{N}_]/u.test(c);

/** All [start, end) ranges of `q` in `text`, using VS Code's match-case / whole-word rules. */
export function findRanges(text, q, matchCase, wholeWord) {
  if (!q) return [];
  const hay = matchCase ? text : text.toLowerCase();
  const needle = matchCase ? q : q.toLowerCase();
  const out = [];
  for (let i = hay.indexOf(needle); i >= 0; i = hay.indexOf(needle, i + needle.length)) {
    if (!wholeWord || (!isWordChar(text[i - 1]) && !isWordChar(text[i + needle.length]))) out.push([i, i + needle.length]);
  }
  return out;
}

const refreshHighlight = StateEffect.define();
const matchMark = Decoration.mark({ class: "cm-searchMatch" });
let highlightQuery = null;

/** Highlight the Search view's query in whatever file is open (visible lines only). */
const searchViewHighlighter = ViewPlugin.fromClass(
  class {
    constructor(view) { this.decorations = this.build(view); }
    update(u) {
      if (u.docChanged || u.viewportChanged || u.transactions.some((t) => t.effects.some((e) => e.is(refreshHighlight)))) {
        this.decorations = this.build(u.view);
      }
    }
    build(view) {
      const q = highlightQuery;
      if (!q?.text) return Decoration.none;
      const builder = new RangeSetBuilder();
      const { doc } = view.state;
      for (const { from, to } of view.visibleRanges) {
        for (let pos = from; pos <= to; ) {
          const line = doc.lineAt(pos);
          for (const [a, b] of findRanges(line.text, q.text, q.matchCase, q.wholeWord)) builder.add(line.from + a, line.from + b, matchMark);
          pos = line.to + 1;
        }
      }
      return builder.finish();
    }
  },
  { decorations: (v) => v.decorations },
);

/** Set (or clear with null) the query highlighted in the editor. */
export function setSearchHighlight(view, query) {
  highlightQuery = query;
  view.dispatch({ effects: refreshHighlight.of(null) });
}

// ------------------------------------------------------------- extensions

let updateHandler = () => {};
/** Register the callback that receives every editor update. */
export function onEditorUpdate(fn) { updateHandler = fn; }

const chevron = (open) => {
  const el = document.createElement("span");
  el.className = "cm-fold-marker";
  el.innerHTML = open
    ? '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3"><path d="m4 6 4 4 4-4"/></svg>'
    : '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3"><path d="m6 4 4 4-4 4"/></svg>';
  return el;
};

// Tab, like VS Code: accept the suggestion → next snippet field → expand an
// Emmet abbreviation → indent. The Emmet expander loads with the first HTML/CSS file.
let emmetTab = null;
loadEmmetTab().then((fn) => { emmetTab = fn; });
const smartTab = (view) => {
  if (completionStatus(view.state) === "active" && acceptCompletion(view)) return true;
  if (hasNextSnippetField(view.state)) return nextSnippetField(view);
  if (emmetTab?.(view)) return true;
  return false;
};

const vscodeKeys = [
  { key: "Mod-g", run: gotoLine, preventDefault: true },
  { key: "Mod-l", run: selectLine, preventDefault: true },
  { key: "Mod-Space", run: startCompletion },
];

const baseExtensions = [
  lineNumbers({ formatNumber: (n, state) => String(n).padStart(Math.max(2, String(state.doc.lines).length), "0") }),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  history(),
  foldGutter({ markerDOM: chevron }),
  drawSelection(),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  indentOnInput(),
  syntaxHighlighting(highlight),
  bracketMatching(),
  closeBrackets(),
  autocompletion({ icons: true, activateOnTyping: true, maxRenderedOptions: 60, interactionDelay: 0, selectOnOpen: true, closeOnBlur: true }),
  rectangularSelection(),
  crosshairCursor(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  indentGuides,
  searchViewHighlighter,
  Prec.highest(keymap.of([{ key: "Tab", run: smartTab }])),
  keymap.of([
    ...vscodeKeys,
    ...closeBracketsKeymap,
    ...defaultKeymap,
    ...searchKeymap,
    ...historyKeymap,
    ...foldKeymap,
    indentWithTab,
  ]),
  theme,
  agentDiffField,
  agentDiffTheme,
  EditorView.updateListener.of((u) => updateHandler(u)),
];

// Word wrap (Alt+Z) is shared by all tabs through one compartment.
const wrapCompartment = new Compartment();
let wrapOn = false;
export const isWrapOn = () => wrapOn;

/** Turn word wrap on/off for the view and for every tab opened later. */
export function setWrap(view, on) {
  wrapOn = on;
  view.dispatch({ effects: wrapCompartment.reconfigure(on ? EditorView.lineWrapping : []) });
}

/** Make a tab's saved state follow the current word-wrap setting before showing it. */
export function syncWrap(view) {
  const current = wrapCompartment.get(view.state);
  const want = wrapOn ? EditorView.lineWrapping : [];
  if (current !== want) view.dispatch({ effects: wrapCompartment.reconfigure(want) });
}

/** A fresh editor state for one tab. */
export function createState(doc, languageExtension, indent = 4) {
  return EditorState.create({
    doc,
    extensions: [
      baseExtensions,
      EditorState.tabSize.of(indent),
      indentUnit.of(" ".repeat(indent)),
      wrapCompartment.of(wrapOn ? EditorView.lineWrapping : []),
      languageExtension ?? [],
    ],
  });
}

export function createView(parent) {
  return new EditorView({ state: createState(""), parent });
}

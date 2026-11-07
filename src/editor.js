// CodeMirror 6 setup with a VS Code "Dark Modern" look and VS Code keybindings.
// One EditorView is shared by all tabs; each tab keeps its own EditorState.

import {
  EditorView, ViewPlugin, Decoration, keymap, lineNumbers, highlightActiveLineGutter,
  highlightSpecialChars, drawSelection, dropCursor, rectangularSelection, crosshairCursor,
  highlightActiveLine,
} from "@codemirror/view";
import { EditorState, RangeSetBuilder, Compartment, StateEffect } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab, selectLine } from "@codemirror/commands";
import {
  indentOnInput, bracketMatching, foldGutter, foldKeymap, syntaxHighlighting, HighlightStyle,
  indentUnit, getIndentUnit,
} from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches, gotoLine } from "@codemirror/search";
import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { tags as t } from "@lezer/highlight";

// ------------------------------------------------------------------ theme

const theme = EditorView.theme(
  {
    "&": { color: "#d4d4d4", backgroundColor: "#1e1f22", height: "100%" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": {
      fontFamily: "var(--editor-font)",
      fontSize: "var(--editor-font-size)",
      lineHeight: "var(--editor-line-height)",
      fontVariantLigatures: "none",
    },
    ".cm-content": { caretColor: "#c8ccd4", padding: "4px 0" },
    ".cm-line": { padding: "0 0 0 2px" },
    ".cm-cursor, .cm-dropCursor": { borderLeft: "2px solid #c8ccd4" },
    // Smooth caret: fade instead of hard blink, and glide between positions.
    "&.cm-focused > .cm-scroller > .cm-cursorLayer": { animation: "ease-in-out cm-blink 1.1s infinite" },
    ".cm-cursor": { transition: "left 70ms ease-out, top 70ms ease-out" },
    ".cm-selectionBackground": { backgroundColor: "#353840" },
    "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground": { backgroundColor: "#264f78" },
    ".cm-content ::selection": { backgroundColor: "transparent" },
    ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.025)" },
    ".cm-gutters": { backgroundColor: "#1e1f22", color: "#5d636e", border: "none" },
    ".cm-lineNumbers .cm-gutterElement": { padding: "0 6px 0 18px", minWidth: "42px" },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: "#d4d7dd" },
    ".cm-foldGutter .cm-gutterElement": { color: "#c5c5c5", padding: "0 4px", cursor: "pointer", opacity: "0", transition: "opacity .1s" },
    ".cm-gutters:hover .cm-foldGutter .cm-gutterElement": { opacity: "1" },
    ".cm-foldPlaceholder": { backgroundColor: "transparent", border: "none", color: "#c5c5c5", padding: "0 4px" },
    ".cm-selectionMatch": { backgroundColor: "#add6ff26" },
    "&.cm-focused .cm-matchingBracket": { backgroundColor: "#0064001a", outline: "1px solid #888888" },
    "&.cm-focused .cm-nonmatchingBracket": { color: "#f44747" },
    ".cm-searchMatch": { backgroundColor: "rgba(255,176,46,.22)", outline: "1px solid rgba(255,176,46,.4)", borderRadius: "2px" },
    ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "rgba(255,176,46,.5)" },
    ".cm-specialChar": { color: "#f44747" },
    ".cm-panels": { backgroundColor: "#24262b", color: "#cfd2d8" },
    ".cm-panels.cm-panels-top": { borderBottom: "1px solid #3a3d44", boxShadow: "0 6px 20px rgba(0,0,0,.35)" },
    ".cm-panels.cm-panels-bottom": { borderTop: "1px solid #3a3d44" },
    ".cm-panel input, .cm-panel button, .cm-panel label": { fontFamily: "var(--ui-font)", fontSize: "12px" },
    ".cm-panel.cm-search": { padding: "8px 10px", animation: "pop-in .14s cubic-bezier(.2,.7,.2,1)" },
    ".cm-panel.cm-search [name=close]": { color: "#949aa4", fontSize: "18px", top: "6px", right: "8px" },
    ".cm-textfield": { backgroundColor: "#2a2c31", border: "1px solid #3a3d44", color: "#f1f3f6", borderRadius: "6px", padding: "4px 8px" },
    ".cm-textfield:focus": { borderColor: "#3b8eea", outline: "none", boxShadow: "0 0 0 3px rgba(59,142,234,.16)" },
    ".cm-button": { backgroundImage: "none", backgroundColor: "#2a2c31", border: "1px solid #3a3d44", color: "#cfd2d8", borderRadius: "6px", padding: "3px 10px" },
    ".cm-button:hover": { backgroundColor: "#33363c" },
    ".cm-tooltip": { backgroundColor: "#24262b", border: "1px solid #3a3d44", color: "#cfd2d8", borderRadius: "8px", overflow: "hidden", boxShadow: "0 8px 24px rgba(0,0,0,.4)" },
    ".cm-tooltip-autocomplete": { animation: "pop-in .1s cubic-bezier(.2,.7,.2,1)" },
    ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--editor-font)", maxHeight: "15em", padding: "3px" },
    ".cm-tooltip-autocomplete > ul > li": { padding: "2px 8px", borderRadius: "4px" },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "#1d3b5c", color: "#ffffff" },
    ".cm-completionMatchedText": { textDecoration: "none", color: "#ffd28a", fontWeight: "bold" },
    ".cm-completionIcon": { opacity: "0.8" },
    ".cm-indent-guides": {
      backgroundImage: "repeating-linear-gradient(to right, rgba(255,255,255,.09) 0 1px, transparent 1px calc(var(--unit) * 1ch))",
      backgroundSize: "calc(var(--guides) * var(--unit) * 1ch) 100%",
      backgroundRepeat: "no-repeat",
      backgroundPosition: "2px 0",
    },
  },
  { dark: true },
);

// VS Code Dark+ / Dark Modern token colors.
const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.definitionKeyword, t.modifier, t.operatorKeyword, t.bool, t.null, t.self, t.atom], color: "#569cd6" },
  { tag: [t.controlKeyword, t.moduleKeyword], color: "#c586c0" },
  { tag: [t.string, t.special(t.string), t.character, t.attributeValue], color: "#ce9178" },
  { tag: [t.escape, t.regexp], color: "#d7ba7d" },
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: "#6a9955" },
  { tag: [t.number, t.integer, t.float], color: "#b5cea8" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.function(t.definition(t.variableName)), t.macroName, t.meta], color: "#dcdcaa" },
  { tag: [t.className, t.definition(t.className), t.typeName, t.namespace], color: "#4ec9b0" },
  { tag: [t.variableName, t.propertyName, t.attributeName, t.labelName], color: "#9cdcfe" },
  { tag: [t.constant(t.variableName), t.standard(t.variableName)], color: "#4fc1ff" },
  { tag: [t.operator, t.punctuation, t.separator, t.paren, t.squareBracket, t.brace, t.derefOperator], color: "#cccccc" },
  { tag: t.tagName, color: "#569cd6" },
  { tag: t.heading, color: "#569cd6", fontWeight: "bold" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: t.link, color: "#3794ff", textDecoration: "underline" },
  { tag: t.invalid, color: "#f44747" },
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

const vscodeKeys = [
  { key: "Mod-g", run: gotoLine, preventDefault: true },
  { key: "Mod-l", run: selectLine, preventDefault: true },
];

const baseExtensions = [
  lineNumbers(),
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
  autocompletion({ icons: true, activateOnTyping: true }),
  rectangularSelection(),
  crosshairCursor(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  indentGuides,
  searchViewHighlighter,
  keymap.of([
    ...vscodeKeys,
    ...closeBracketsKeymap,
    ...defaultKeymap,
    ...searchKeymap,
    ...historyKeymap,
    ...foldKeymap,
    ...completionKeymap,
    indentWithTab,
  ]),
  theme,
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

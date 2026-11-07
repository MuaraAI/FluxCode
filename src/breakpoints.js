// Editor side of debugging: the breakpoint gutter (left of the line numbers,
// like VS Code: hover shows a faint dot, click toggles), the current-line
// highlight while paused, and value tooltips on hover. Breakpoints live in
// each tab's EditorState, so they move with the text when lines are added or
// removed above them; main.js mirrors them per file for the debugger.

import { StateField, StateEffect, RangeSet, RangeSetBuilder } from "@codemirror/state";
import { EditorView, Decoration, GutterMarker, gutter, hoverTooltip } from "@codemirror/view";

/** Replace a state's breakpoints: an array of 1-based line numbers. */
export const setBreakpointsEffect = StateEffect.define();
const toggleEffect = StateEffect.define(); // a line start position
/** `{ line, top }` while paused in this file (top = innermost frame), or null. */
export const setExecLineEffect = StateEffect.define();

const bpMarker = new (class extends GutterMarker {
  elementClass = "cm-bp-on";
  toDOM() { const el = document.createElement("span"); el.className = "cm-bp-dot"; return el; }
})();

const breakpointField = StateField.define({
  create: () => RangeSet.empty,
  update(set, tr) {
    set = set.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setBreakpointsEffect)) {
        const doc = tr.state.doc;
        const b = new RangeSetBuilder();
        for (const n of [...new Set(e.value)].filter((n) => n >= 1 && n <= doc.lines).sort((a, b) => a - b)) b.add(doc.line(n).from, doc.line(n).from, bpMarker);
        set = b.finish();
      } else if (e.is(toggleEffect)) {
        let has = false;
        set.between(e.value, e.value, () => { has = true; });
        set = has ? set.update({ filter: (from) => from !== e.value }) : set.update({ add: [bpMarker.range(e.value)] });
      }
    }
    return set;
  },
});

/** Sorted, de-duplicated 1-based lines that have a breakpoint. */
export function breakpointLines(state) {
  const lines = new Set();
  const iter = state.field(breakpointField, false)?.iter();
  if (!iter) return [];
  for (; iter.value; iter.next()) lines.add(state.doc.lineAt(iter.from).number);
  return [...lines];
}

/** True when an update changed this file's breakpoints (toggle, set, or lines moved by an edit). */
export function breakpointsChanged(u) {
  if (u.transactions.some((t) => t.effects.some((e) => e.is(setBreakpointsEffect) || e.is(toggleEffect)))) return true;
  if (!u.docChanged) return false;
  const a = u.startState.field(breakpointField, false);
  return !!a && a.size > 0 && breakpointLines(u.startState).join() !== breakpointLines(u.state).join();
}

/** Toggle the breakpoint on the line holding `pos` (F9, gutter click). */
export function toggleBreakpoint(view, pos = view.state.selection.main.head) {
  view.dispatch({ effects: toggleEffect.of(view.state.doc.lineAt(pos).from) });
  return true;
}

// ------------------------------------------------------------- exec line

const execLineDeco = Decoration.line({ class: "cm-execLine" });
const frameLineDeco = Decoration.line({ class: "cm-execLine cm-frameLine" });
const execMarker = new (class extends GutterMarker {
  elementClass = "cm-exec-on";
  toDOM() { const el = document.createElement("span"); el.className = "cm-exec-arrow"; return el; }
})();

const execField = StateField.define({
  create: () => null, // { pos, top }
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setExecLineEffect)) {
        if (!e.value) return null;
        const doc = tr.state.doc;
        return { pos: doc.line(Math.min(Math.max(1, e.value.line), doc.lines)).from, top: e.value.top !== false };
      }
    }
    return value && tr.docChanged ? { ...value, pos: tr.changes.mapPos(value.pos) } : value;
  },
  provide: (f) => EditorView.decorations.from(f, (v) => (v ? Decoration.set([(v.top ? execLineDeco : frameLineDeco).range(v.pos)]) : Decoration.none)),
});

// ---------------------------------------------------------------- gutter

const bpGutter = gutter({
  class: "cm-bp-gutter",
  markers: (view) => {
    const exec = view.state.field(execField);
    const bps = view.state.field(breakpointField);
    if (!exec) return bps;
    return RangeSet.join([bps, RangeSet.of([execMarker.range(exec.pos)])]);
  },
  initialSpacer: () => bpMarker,
  domEventHandlers: {
    mousedown(view, line) {
      toggleBreakpoint(view, line.from);
      return true;
    },
  },
});

// ----------------------------------------------------------------- hover

let hoverProvider = null; // (expression) => Promise<string|null>, set while paused

/** Value tooltips while paused: `fn(expression)` resolves to the text to show (or null). */
export function setDebugHover(fn) { hoverProvider = fn; }

const debugHover = hoverTooltip(async (view, pos) => {
  if (!hoverProvider) return null;
  const line = view.state.doc.lineAt(pos);
  const text = line.text;
  let a = pos - line.from;
  let b = a;
  const word = /[\p{L}\p{N}_$]/u;
  if (!word.test(text[a] ?? "") && !word.test(text[a - 1] ?? "")) return null;
  while (a > 0 && /[\p{L}\p{N}_$.]/u.test(text[a - 1])) a--;
  while (b < text.length && word.test(text[b])) b++;
  const expr = text.slice(a, b).replace(/^\.+|\.+$/g, "");
  if (!expr || /^\d/.test(expr) || /^(def|class|return|import|from|if|else|elif|for|while|in|not|and|or|function|const|let|var|true|false|null|None|True|False)$/.test(expr)) return null;
  const value = await hoverProvider(expr);
  if (value == null) return null;
  return {
    pos: line.from + a,
    end: line.from + b,
    above: true,
    create() {
      const dom = document.createElement("div");
      dom.className = "cm-debug-hover";
      const name = document.createElement("span");
      name.className = "dh-name";
      name.textContent = `${expr}: `;
      const val = document.createElement("span");
      val.className = "dh-value";
      val.textContent = value;
      dom.append(name, val);
      return { dom };
    },
  };
}, { hoverTime: 350 });

const theme = EditorView.theme({
  ".cm-bp-gutter": { width: "16px", cursor: "pointer" },
  ".cm-bp-gutter .cm-gutterElement": { display: "flex", alignItems: "center", justifyContent: "center", position: "relative" },
  ".cm-bp-dot": { width: "9px", height: "9px", borderRadius: "50%", background: "var(--bp, #e51400)", display: "block" },
  ".cm-bp-gutter .cm-gutterElement:not(.cm-bp-on):not(.cm-exec-on):hover::after": {
    content: '""', width: "9px", height: "9px", borderRadius: "50%", background: "var(--bp, #e51400)", opacity: ".38",
  },
  ".cm-exec-arrow": {
    position: "absolute", left: "1px", top: "50%", width: "13px", height: "11px", marginTop: "-5.5px",
    background: "var(--exec, #ffcc00)", clipPath: "polygon(0 15%, 55% 15%, 55% 0, 100% 50%, 55% 100%, 55% 85%, 0 85%)",
  },
  ".cm-bp-on .cm-exec-arrow": { left: "2px" },
  ".cm-execLine": { backgroundColor: "color-mix(in srgb, var(--exec, #ffcc00) 16%, transparent) !important", outline: "1px solid color-mix(in srgb, var(--exec, #ffcc00) 34%, transparent)" },
  ".cm-frameLine": { backgroundColor: "color-mix(in srgb, #3fb950 14%, transparent) !important", outline: "1px solid color-mix(in srgb, #3fb950 30%, transparent)" },
  ".cm-tooltip.cm-tooltip-hover:has(.cm-debug-hover)": { padding: "0" },
  ".cm-debug-hover": { padding: "5px 9px", fontFamily: "var(--editor-font)", fontSize: "12px", maxWidth: "560px", whiteSpace: "pre-wrap", wordBreak: "break-all" },
  ".cm-debug-hover .dh-name": { color: "var(--tk-variable)" },
  ".cm-debug-hover .dh-value": { color: "var(--fg)" },
});

/** Everything the editor needs for debugging (goes before the line numbers). */
export const debugGutter = [breakpointField, execField, bpGutter, debugHover, theme];

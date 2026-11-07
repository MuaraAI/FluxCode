// Inline "what the agent changed" view for the editor, like Cursor: added
// lines are tinted green, removed lines are shown in red above the place
// they were, and every hunk has Keep / Undo. The baseline (the file before the
// agent touched it) lives in a StateField; hunks are recomputed from
// baseline vs document, so nothing goes stale while you keep typing.

import { StateField, StateEffect, RangeSetBuilder } from "@codemirror/state";
import { EditorView, Decoration, WidgetType } from "@codemirror/view";

const splitLines = (s) => (s === "" ? [""] : s.split(/\r\n?|\n/));

/**
 * Line diff of `a` → `b`. Returns hunks: { oldLine, line, removed, added }
 * (0-based line numbers in a / b). Common prefix/suffix are trimmed first;
 * the remaining middle uses an LCS table when small, otherwise one big hunk.
 */
export function diffLines(aText, bText) {
  const a = splitLines(aText);
  const b = splitLines(bText);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length, endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) { endA--; endB--; }
  const ma = a.slice(start, endA), mb = b.slice(start, endB);
  if (!ma.length && !mb.length) return [];
  if (!ma.length || !mb.length || ma.length * mb.length > 250_000) {
    return [{ oldLine: start, line: start, removed: ma, added: mb }];
  }
  // LCS lengths (ma.length+1) x (mb.length+1), then walk back into hunks.
  const n = ma.length, m = mb.length;
  const w = m + 1;
  const L = new Uint16Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      L[i * w + j] = ma[i] === mb[j] ? L[(i + 1) * w + j + 1] + 1 : Math.max(L[(i + 1) * w + j], L[i * w + j + 1]);
    }
  }
  const hunks = [];
  let i = 0, j = 0, cur = null;
  const flush = () => { if (cur && (cur.removed.length || cur.added.length)) hunks.push(cur); cur = null; };
  while (i < n || j < m) {
    if (i < n && j < m && ma[i] === mb[j]) { flush(); i++; j++; continue; }
    if (!cur) cur = { oldLine: start + i, line: start + j, removed: [], added: [] };
    if (j < m && (i >= n || L[i * w + j + 1] >= L[(i + 1) * w + j])) cur.added.push(mb[j++]);
    else cur.removed.push(ma[i++]);
  }
  flush();
  return hunks;
}

export const setAgentBaseline = StateEffect.define();
const hunkActions = { keep: null, undo: null }; // filled below; widgets call these

class HunkWidget extends WidgetType {
  constructor(hunk, index) { super(); this.hunk = hunk; this.index = index; }
  eq(o) { return o.index === this.index && o.hunk.removed.join("\n") === this.hunk.removed.join("\n") && o.hunk.added.length === this.hunk.added.length; }
  toDOM(view) {
    const el = document.createElement("div");
    el.className = "cm-agent-hunk";
    const removed = this.hunk.removed.map((l) => `<div class="cm-agent-del">${l.replace(/[&<>]/g, (c) => `&#${c.charCodeAt(0)};`) || " "}</div>`).join("");
    el.innerHTML = `<div class="cm-agent-actions"><span>${this.hunk.added.length ? `+${this.hunk.added.length}` : ""}${this.hunk.removed.length ? ` −${this.hunk.removed.length}` : ""}</span><button data-h="keep" title="Keep this change">Keep</button><button data-h="undo" title="Undo this change">Undo</button></div>${removed}`;
    el.addEventListener("mousedown", (e) => {
      const b = e.target.closest("button[data-h]");
      if (!b) return;
      e.preventDefault();
      (b.dataset.h === "keep" ? keepHunk : undoHunk)(view, this.index);
    });
    return el;
  }
  ignoreEvent() { return true; }
  get estimatedHeight() { return 22 * (this.hunk.removed.length + 1); }
}

const addLine = Decoration.line({ class: "cm-agent-add" });

function build(state, baseline) {
  if (baseline == null) return { baseline: null, hunks: [], deco: Decoration.none };
  const doc = state.doc;
  const hunks = diffLines(baseline, doc.toString());
  const builder = new RangeSetBuilder();
  hunks.forEach((h, i) => {
    const at = h.line < doc.lines ? doc.line(h.line + 1).from : doc.length;
    builder.add(at, at, Decoration.widget({ widget: new HunkWidget(h, i), block: true, side: -1 }));
    for (let k = 0; k < h.added.length && h.line + k < doc.lines; k++) {
      const ln = doc.line(h.line + k + 1);
      builder.add(ln.from, ln.from, addLine);
    }
  });
  return { baseline, hunks, deco: builder.finish() };
}

/** The agent-diff state: baseline text (or null), current hunks and decorations. */
export const agentDiffField = StateField.define({
  create: () => ({ baseline: null, hunks: [], deco: Decoration.none }),
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setAgentBaseline)) return build(tr.state, e.value);
    if (value.baseline == null) return value;
    if (tr.docChanged) return build(tr.state, value.baseline);
    return value;
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.deco),
});

/** Hunks currently shown (empty when no baseline). */
export const agentHunks = (state) => state.field(agentDiffField, false)?.hunks ?? [];
export const agentBaseline = (state) => state.field(agentDiffField, false)?.baseline ?? null;

/** Keep hunk i: fold it into the baseline, so only the other hunks stay highlighted. */
export function keepHunk(view, i) {
  const { baseline, hunks } = view.state.field(agentDiffField);
  const h = hunks[i];
  if (!h) return;
  const lines = splitLines(baseline);
  lines.splice(h.oldLine, h.removed.length, ...h.added);
  const next = lines.join("\n");
  view.dispatch({ effects: setAgentBaseline.of(hunks.length === 1 ? null : next) });
  hunkActions.keep?.(view, hunks.length === 1);
}

/** Undo hunk i: put the removed lines back in the document in place of the added ones. */
export function undoHunk(view, i) {
  const { hunks } = view.state.field(agentDiffField);
  const h = hunks[i];
  if (!h) return;
  const doc = view.state.doc;
  let from, to, insert;
  if (h.added.length) {
    from = doc.line(h.line + 1).from;
    to = doc.line(Math.min(doc.lines, h.line + h.added.length)).to;
    insert = h.removed.join("\n");
  } else {
    // Pure deletion: re-insert before the line that follows.
    const at = h.line < doc.lines ? doc.line(h.line + 1).from : doc.length;
    from = to = at;
    insert = h.removed.join("\n") + (h.line < doc.lines ? "\n" : "");
    if (h.line >= doc.lines && doc.length) insert = "\n" + h.removed.join("\n");
  }
  view.dispatch({ changes: { from, to, insert } });
  const left = agentHunks(view.state);
  if (!left.length) view.dispatch({ effects: setAgentBaseline.of(null) });
  hunkActions.undo?.(view, !left.length);
}

/** Let the workbench know when the last hunk of a file was kept/undone (to save, refresh lists …). */
export function onHunkSettled(fn) { hunkActions.keep = hunkActions.undo = fn; }

export const agentDiffTheme = EditorView.theme({
  ".cm-agent-add": { backgroundColor: "color-mix(in srgb, var(--ok) 13%, transparent)" },
  ".cm-agent-hunk": { backgroundColor: "color-mix(in srgb, var(--err) 9%, transparent)", borderLeft: "2px solid color-mix(in srgb, var(--err) 60%, transparent)", padding: "0 0 0 8px", margin: "0 0 0 -2px", position: "relative", fontFamily: "var(--editor-font)" },
  ".cm-agent-del": { whiteSpace: "pre", color: "var(--fg-muted)", textDecoration: "line-through", textDecorationColor: "color-mix(in srgb, var(--err) 50%, transparent)" },
  ".cm-agent-actions": { display: "flex", gap: "4px", alignItems: "center", height: "20px", fontFamily: "var(--font-ui)", fontSize: "11px", color: "var(--fg-dim)", position: "sticky", left: "0", width: "max-content", paddingRight: "8px" },
  ".cm-agent-actions button": { padding: "1px 8px", borderRadius: "5px", border: "1px solid var(--border-strong)", background: "var(--bg-elevated)", color: "var(--fg)", fontSize: "11px", cursor: "pointer" },
  ".cm-agent-actions button:hover": { background: "var(--bg-select)", color: "var(--fg-strong)" },
  ".cm-agent-hunk:not(:has(.cm-agent-del))": { backgroundColor: "transparent", borderLeftColor: "color-mix(in srgb, var(--ok) 60%, transparent)" },
});

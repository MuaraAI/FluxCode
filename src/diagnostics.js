// Problems: syntax errors found in the parse tree the editor already keeps for
// highlighting (no second parser, no worker, no network). They show as a wavy
// underline, a red line number, a hover message and a count in the status bar.
// Runs ~300 ms after the last edit and again when the background parse catches up.

import { linter, lintKeymap, setDiagnosticsEffect, forEachDiagnostic, nextDiagnostic, openLintPanel } from "@codemirror/lint";
import { syntaxTree } from "@codemirror/language";
import { StateField, RangeSet, RangeSetBuilder } from "@codemirror/state";
import { EditorView, GutterMarker, gutterLineClass, keymap } from "@codemirror/view";

/** Files above this size are not checked (keeps huge logs/bundles snappy). */
const MAX_LINT_BYTES = 1_500_000;
const MAX_PROBLEMS = 200;

// HTML elements with no closing tag, or that the parser closes on its own.
const NO_CLOSE_TAG = new Set([
  "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr",
  "p", "li", "dt", "dd", "tr", "td", "th", "option", "optgroup", "thead", "tbody", "tfoot", "colgroup", "caption", "html", "head", "body", "rt", "rp",
]);

function unexpected(doc, from, to) {
  if (from === to) return "Syntax error: a token is missing here";
  const text = doc.sliceString(from, Math.min(to, from + 30)).trim();
  return text ? `Syntax error: unexpected '${text}'${to - from > 30 ? "…" : ""}` : "Syntax error";
}

/** Walk the tree once; collect error nodes and HTML tag problems. */
function collectProblems(state) {
  const { doc } = state;
  const tree = syntaxTree(state);
  const out = [];
  let lastErrorLine = -1;
  const cursor = tree.cursor();
  do {
    const type = cursor.type;
    if (type.isError) {
      // A broken statement usually yields a cascade of error nodes; one per line is enough.
      const line = doc.lineAt(cursor.from).number;
      if (line === lastErrorLine) continue;
      lastErrorLine = line;
      out.push({ from: cursor.from, to: cursor.to, severity: "error", message: unexpected(doc, cursor.from, cursor.to) });
    } else if (type.name === "MismatchedCloseTag") {
      out.push({ from: cursor.from, to: cursor.to, severity: "error", message: `Closing tag '${doc.sliceString(cursor.from, cursor.to)}' does not match the element it closes` });
    } else if (type.name === "IncompleteCloseTag") {
      out.push({ from: cursor.from, to: cursor.to, severity: "error", message: "Incomplete closing tag" });
    } else if (type.name === "Element") {
      const el = cursor.node;
      const open = el.firstChild;
      const last = el.lastChild;
      if (open?.name === "OpenTag" && last?.name !== "CloseTag") {
        const nameNode = open.getChild("TagName");
        const name = nameNode ? doc.sliceString(nameNode.from, nameNode.to).toLowerCase() : "";
        if (name && !NO_CLOSE_TAG.has(name) && !name.includes("-")) {
          out.push({ from: nameNode.from, to: nameNode.to, severity: "warning", message: `Missing closing tag </${name}>` });
        }
      }
    }
    if (out.length >= MAX_PROBLEMS) break;
  } while (cursor.next());
  return out;
}

const syntaxLinter = linter(
  (view) => (view.state.doc.length > MAX_LINT_BYTES ? [] : collectProblems(view.state)),
  {
    delay: 300,
    // Re-check when the background parser finishes a part of the file.
    needsRefresh: (u) => syntaxTree(u.state) !== syntaxTree(u.startState),
  },
);

// ---------------------------------------------------- red line numbers

const errorLine = new (class extends GutterMarker { elementClass = "cm-problemLine cm-problemLine-error"; })();
const warningLine = new (class extends GutterMarker { elementClass = "cm-problemLine cm-problemLine-warning"; })();

const problemLines = StateField.define({
  create: () => RangeSet.empty,
  update(set, tr) {
    for (const e of tr.effects) {
      if (e.is(setDiagnosticsEffect)) {
        const builder = new RangeSetBuilder();
        const seen = new Map(); // line.from -> severity
        for (const d of e.value) {
          const line = tr.state.doc.lineAt(d.from);
          if (seen.get(line.from) !== "error") seen.set(line.from, d.severity === "error" ? "error" : "warning");
        }
        for (const [from, sev] of [...seen].sort((a, b) => a[0] - b[0])) builder.add(from, from, sev === "error" ? errorLine : warningLine);
        return builder.finish();
      }
    }
    return tr.docChanged ? set.map(tr.changes) : set;
  },
  provide: (f) => gutterLineClass.from(f),
});

// ------------------------------------------------------------- theme

const problemsTheme = EditorView.theme({
  ".cm-lintRange": { backgroundImage: "none", textDecorationLine: "underline", textDecorationStyle: "wavy", textDecorationSkipInk: "none", textUnderlineOffset: "3px", textDecorationThickness: "1px", paddingBottom: "0" },
  ".cm-lintRange-error": { textDecorationColor: "var(--err)" },
  ".cm-lintRange-warning": { textDecorationColor: "var(--warn)" },
  ".cm-lintRange-info, .cm-lintRange-hint": { textDecorationColor: "var(--accent)" },
  ".cm-lintPoint-error::after": { borderBottomColor: "var(--err)" },
  ".cm-lintPoint-warning::after": { borderBottomColor: "var(--warn)" },
  ".cm-lineNumbers .cm-gutterElement.cm-problemLine-error": { color: "var(--err)", fontWeight: "600" },
  ".cm-lineNumbers .cm-gutterElement.cm-problemLine-warning": { color: "var(--warn)" },
  ".cm-tooltip.cm-tooltip-lint": { padding: "2px 0", fontFamily: "var(--font-ui)", fontSize: "12px" },
  ".cm-diagnostic": { padding: "5px 10px 5px 8px", borderLeftWidth: "3px", marginLeft: "0" },
  ".cm-diagnostic-error": { borderLeftColor: "var(--err)" },
  ".cm-diagnostic-warning": { borderLeftColor: "var(--warn)" },
  ".cm-diagnostic-info, .cm-diagnostic-hint": { borderLeftColor: "var(--accent)" },
  ".cm-panel.cm-panel-lint": { fontFamily: "var(--font-ui)", fontSize: "12px" },
  ".cm-panel.cm-panel-lint ul": { maxHeight: "140px" },
  ".cm-panel.cm-panel-lint ul [aria-selected]": { backgroundColor: "var(--bg-select)", color: "var(--fg-strong)" },
  ".cm-panel.cm-panel-lint ul:focus [aria-selected]": { backgroundColor: "var(--bg-select)" },
  ".cm-panel.cm-panel-lint button[name=close]": { color: "var(--fg-muted)", fontSize: "18px", top: "4px", right: "6px" },
});

/** Everything the editor needs: the check, red line numbers, F8 / Ctrl+Shift+M keys, theme. */
export const problems = [syntaxLinter, problemLines, problemsTheme, keymap.of(lintKeymap)];

/** `{ errors, warnings }` for a state. */
export function problemCounts(state) {
  let errors = 0;
  let warnings = 0;
  forEachDiagnostic(state, (d) => { if (d.severity === "error") errors++; else warnings++; });
  return { errors, warnings };
}

/** True when this editor update carried a new set of problems. */
export const problemsChanged = (u) => u.transactions.some((t) => t.effects.some((e) => e.is(setDiagnosticsEffect)));

export { nextDiagnostic as nextProblem, openLintPanel as openProblems };

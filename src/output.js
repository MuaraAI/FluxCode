// Output panel for "Run": the text area, the stdin row and the process event
// listeners. Loaded on the first Run and mounted only while a program is
// running or its output is still on screen; `dispose()` tears everything down
// (DOM, Tauri listeners, pending frames) so an idle editor pays nothing for it.
//
// Rendering stays cheap however chatty the program is:
//  - chunks are queued and appended once per animation frame, in one fragment;
//  - same-styled chunks merge into the previous <span> (few DOM nodes);
//  - only the last MAX_LINES lines are kept, older text is dropped from the top;
//  - "stick to bottom" is tracked from scroll events, so a flush never has to
//    measure the scroll height just to find out whether to follow the output.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const MAX_LINES = 5_000;
const MAX_CHARS = 400_000;
const STICK_SLACK = 24; // px from the bottom that still counts as "following"

const countLines = (s) => {
  let n = 0;
  for (let i = s.indexOf("\n"); i !== -1; i = s.indexOf("\n", i + 1)) n++;
  return n;
};

/**
 * @param host     the empty `#output-pane` element
 * @param ctx      { icons, openFile(path, line), toast(msg, kind), runId(): number|null, onExit(payload) }
 */
export function createOutput(host, ctx) {
  host.innerHTML =
    '<pre class="out-text" tabindex="0"></pre>' +
    '<div class="stdin-row" hidden><span class="stdin-prompt"></span>' +
    '<input class="stdin" placeholder="Type input for the running program, then Enter" spellcheck="false" autocomplete="off" /></div>';
  const out = host.firstElementChild;
  const stdinRow = out.nextElementSibling;
  const stdin = stdinRow.lastElementChild;
  stdinRow.firstElementChild.innerHTML = ctx.icons.chevron;

  let pending = [];
  let frame = 0;
  let lines = 0;
  let chars = 0;
  let stick = true;
  let disposed = false;

  // ---------------------------------------------------------------- append

  function append(text, cls = "") {
    if (disposed || !text) return;
    pending.push(text, cls);
    if (!frame) frame = requestAnimationFrame(flush);
  }

  function flush() {
    frame = 0;
    if (!pending.length) return;
    const frag = document.createDocumentFragment();
    let last = out.lastElementChild;
    for (let i = 0; i < pending.length; i += 2) {
      const text = pending[i];
      const cls = pending[i + 1];
      if (last && last.className === cls && !last.dataset.linked) last.append(text);
      else {
        last = document.createElement("span");
        if (cls) last.className = cls;
        last.textContent = text;
        frag.append(last);
      }
      lines += countLines(text);
      chars += text.length;
    }
    pending = [];
    if (frag.hasChildNodes()) out.append(frag);
    trim();
    if (stick) out.scrollTop = out.scrollHeight;
  }

  /** Drop the oldest output once over the line/character budget. */
  function trim() {
    while ((lines > MAX_LINES || chars > MAX_CHARS) && out.firstElementChild) {
      const first = out.firstElementChild;
      const text = first.textContent;
      const firstLines = countLines(text);
      const excess = lines - MAX_LINES;
      if (first.dataset.linked || firstLines <= excess || chars - text.length > MAX_CHARS) {
        lines -= firstLines;
        chars -= text.length;
        first.remove();
        continue;
      }
      // Cut only the surplus lines from the first span.
      let at = -1;
      for (let n = 0; n < excess; n++) at = text.indexOf("\n", at + 1);
      const kept = text.slice(at + 1);
      first.textContent = kept;
      lines -= excess;
      chars -= text.length - kept.length;
    }
  }

  function clear() {
    pending = [];
    lines = chars = 0;
    stick = true;
    out.replaceChildren();
  }

  /** Turn Python tracebacks and Node stack frames (`C:\x.js:10:5`) into clickable links. */
  function linkify() {
    const re = /(?:File "([^"]+)", line (\d+))|((?:\/[^\s:()"'<>|]+\.[a-zA-Z0-9]+|[A-Za-z]:\\[^\s:()"'<>|]+\.[a-zA-Z0-9]+)):(\d+)(?::\d+)?/g;
    for (const span of out.querySelectorAll("span.stderr:not([data-linked])")) {
      const text = span.textContent;
      if (!/File "|[A-Za-z]:\\|\//.test(text)) continue;
      const frag = document.createDocumentFragment();
      let at = 0;
      for (const m of text.matchAll(re)) {
        if (m.index > 0 && text.slice(Math.max(0, m.index - 5), m.index).endsWith("://")) continue;
        frag.append(text.slice(at, m.index));
        const a = document.createElement("a");
        a.textContent = m[0];
        a.dataset.file = m[1] ?? m[3];
        a.dataset.line = m[2] ?? m[4];
        frag.append(a);
        at = m.index + m[0].length;
      }
      frag.append(text.slice(at));
      span.replaceChildren(frag);
      span.dataset.linked = "1";
    }
  }

  // ------------------------------------------------------------- listeners

  const onScroll = () => { stick = out.scrollHeight - out.scrollTop - out.clientHeight <= STICK_SLACK; };
  out.addEventListener("scroll", onScroll, { passive: true });

  const onClick = (e) => {
    const a = e.target.closest("a[data-file]");
    if (a) ctx.openFile(a.dataset.file, +a.dataset.line);
  };
  out.addEventListener("click", onClick);

  const onKey = (e) => {
    const id = ctx.runId();
    if (e.key !== "Enter" || id == null) return;
    const value = stdin.value;
    stdin.value = "";
    append(`${value}\n`, "echo");
    invoke("write_stdin", { id, data: `${value}\n` }).catch((err) => ctx.toast(`${err}`, "error"));
  };
  stdin.addEventListener("keydown", onKey);

  const unlisteners = Promise.all([
    listen("proc-output", ({ payload }) => {
      if (payload.id === ctx.runId()) append(payload.data, payload.stream === "stderr" ? "stderr" : "");
    }),
    listen("proc-exit", ({ payload }) => {
      if (payload.id !== ctx.runId()) return;
      flush();
      linkify();
      ctx.onExit(payload);
    }),
  ]);

  // ---------------------------------------------------------------- public

  return {
    append,
    flush,
    clear,
    linkify,
    /** Show the input row while a program runs and focus it, so `input()` just works. */
    setRunning(running) {
      stdinRow.hidden = !running;
      if (running) stdin.focus();
    },
    focus: () => (stdinRow.hidden ? out : stdin).focus(),
    dispose() {
      if (disposed) return;
      disposed = true;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      pending = [];
      out.removeEventListener("scroll", onScroll);
      out.removeEventListener("click", onClick);
      stdin.removeEventListener("keydown", onKey);
      unlisteners.then((fns) => fns.forEach((f) => f()));
      host.replaceChildren();
    },
  };
}

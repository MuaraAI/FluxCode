// Flux Terminal — FluxCode's own terminal, written from scratch.
//
// Not a VT emulator: every command becomes a "block" (command, status, time,
// output) and FluxCode itself tracks the working folder. That means:
//   * no shell process idling in the background, nothing to download, ~0 cost when idle
//   * folders are navigated by clicking the path bar or just typing a folder
//     name (no `cd ..` needed: `..`, `...`, `~`, `-`, `D:` and plain names work)
//   * `ls`/`dir` are native and clickable
//   * colors (ANSI SGR) and progress bars (\r) are rendered
// Limits: full-screen TUI programs (vim, htop) need a real PTY; they are not supported.
//
// This module is loaded on demand the first time the terminal opens.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const SETTINGS_KEY = "fluxcode.terminal";
const HISTORY_KEY = "fluxcode.terminal.history";
const MAX_BLOCKS = 150;
const MAX_BLOCK_CHARS = 300_000;
const BUILTINS = ["cd", "ls", "dir", "clear", "cls", "open", "code", "help", "history", "exit", "pwd"];

const esc = (s) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const sepIdx = (p) => Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/"));
const baseName = (p) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const joinPath = (dir, name) => (/[\\/]$/.test(dir) ? dir + name : `${dir}\\${name}`);
const fmtSize = (n) =>
  n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : n < 1073741824 ? `${(n / 1048576).toFixed(1)} MB` : `${(n / 1073741824).toFixed(2)} GB`;
const fmtTime = (ms) => (ms < 1000 ? `${Math.round(ms)} ms` : ms < 60000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`);

function loadJson(key, fallback) {
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(key) ?? "null") }; } catch { return { ...fallback }; }
}

// ---------------------------------------------------------------- ANSI → DOM

// Flux palette for the 16 ANSI colors (readable on the dark panel).
const ANSI16 = [
  "#3b3f47", "#ff6b6b", "#5fd38d", "#e5c07b", "#5ea8ff", "#c792ea", "#4fd1c5", "#cfd2d8",
  "#6b717c", "#ff8a80", "#7ee2a8", "#ffd68a", "#82bfff", "#dab2ff", "#7ee8dc", "#ffffff",
];
const xterm256 = (n) => {
  if (n < 16) return ANSI16[n];
  if (n >= 232) { const v = 8 + (n - 232) * 10; return `rgb(${v},${v},${v})`; }
  n -= 16;
  const c = (v) => (v ? 55 + v * 40 : 0);
  return `rgb(${c(Math.floor(n / 36))},${c(Math.floor(n / 6) % 6)},${c(n % 6)})`;
};
// CSI (colors, cursor moves), OSC (titles, links) and charset escapes; only SGR ("m") is applied.
const ESC_RE = /\x1b\[([0-9;?]*)([@-~])|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][0-9A-Za-z]|\x1b[=>78]/g;

function applySgr(st, params) {
  const codes = params === "" ? [0] : params.split(";").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < codes.length; i++) {
    const c = codes[i];
    if (c === 0) { st.fg = st.bg = null; st.bold = st.dim = st.italic = st.underline = false; }
    else if (c === 1) st.bold = true;
    else if (c === 2) st.dim = true;
    else if (c === 3) st.italic = true;
    else if (c === 4) st.underline = true;
    else if (c === 22) st.bold = st.dim = false;
    else if (c === 23) st.italic = false;
    else if (c === 24) st.underline = false;
    else if (c >= 30 && c <= 37) st.fg = ANSI16[c - 30];
    else if (c >= 90 && c <= 97) st.fg = ANSI16[c - 82];
    else if (c === 39) st.fg = null;
    else if (c >= 40 && c <= 47) st.bg = ANSI16[c - 40];
    else if (c >= 100 && c <= 107) st.bg = ANSI16[c - 92];
    else if (c === 49) st.bg = null;
    else if (c === 38 || c === 48) {
      let color = null;
      if (codes[i + 1] === 5) { color = xterm256(codes[i + 2] ?? 0); i += 2; }
      else if (codes[i + 1] === 2) { color = `rgb(${codes[i + 2] ?? 0},${codes[i + 3] ?? 0},${codes[i + 4] ?? 0})`; i += 4; }
      if (c === 38) st.fg = color; else st.bg = color;
    }
  }
}

const styleKey = (st) => `${st.fg}|${st.bg}|${st.bold}|${st.dim}|${st.italic}|${st.underline}`;
function styleSpan(st, text) {
  const span = document.createElement("span");
  let css = "";
  if (st.fg) css += `color:${st.fg};`;
  if (st.bg) css += `background:${st.bg};`;
  if (st.bold) css += "font-weight:600;";
  if (st.dim) css += "opacity:.65;";
  if (st.italic) css += "font-style:italic;";
  if (st.underline) css += "text-decoration:underline;";
  if (css) span.style.cssText = css;
  span.dataset.k = styleKey(st);
  span.textContent = text;
  return span;
}

// ------------------------------------------------------------------ terminal

export function createTerminal(host, ctx) {
  const { icons, fileIcon, folderIcon } = ctx;
  const settings = loadJson(SETTINGS_KEY, { shell: "cmd", fontSize: 13, cursor: "bar", smoothCaret: true, showHidden: false });
  let history = [];
  try { history = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]"); } catch {}
  const saveSettings = () => { try { localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings)); } catch {} };
  const saveHistory = () => { try { localStorage.setItem(HISTORY_KEY, JSON.stringify(history.slice(-300))); } catch {} };

  let env = { home: "C:\\", drives: ["C:\\"], shells: ["cmd", "powershell"], user: "", computer: "" };
  const sessions = [];
  let active = null;
  let seq = 0;
  const procs = new Map(); // process id -> block
  const listCache = new Map(); // folder -> { at, entries }

  host.innerHTML = `
    <div class="term-top">
      <button class="tp-drive" title="Switch drive">${icons.drive}<span class="tp-drive-chev">${icons.chevron}</span></button>
      <div class="tp-crumbs"></div>
      <div class="term-tools">
        <button data-t="back" title="Back to previous folder (Alt+Left)">${icons.back}</button>
        <button data-t="up" title="Up one folder (Alt+Up)">${icons.up}</button>
        <button data-t="home" title="Home folder (~)">${icons.home}</button>
        <button data-t="workspace" title="Workspace folder">${icons.workspace}</button>
        <button data-t="list" title="Show this folder (ls)">${icons.list}</button>
        <button data-t="reveal" title="Open in File Explorer">${icons.reveal}</button>
        <span class="tt-sep"></span>
        <div class="term-sessions"></div>
        <button data-t="new" title="New Terminal (Ctrl+Shift+\`)">${icons.plus}</button>
        <button data-t="settings" title="Terminal Settings">${icons.settings}</button>
      </div>
    </div>
    <div class="term-body"></div>
    <div class="term-input">
      <span class="ti-prompt">${icons.chevron}</span>
      <div class="ti-field">
        <div class="ti-mirror"></div>
        <input class="ti-input" spellcheck="false" autocomplete="off" autocorrect="off" />
        <div class="ti-caret"></div>
      </div>
      <button class="ti-shell" title="Shell for commands (click to switch)"></button>
    </div>
    <ul class="term-suggest" hidden></ul>
    <div class="term-pop" hidden></div>`;

  const $ = (s) => host.querySelector(s);
  const body = $(".term-body");
  const input = $(".ti-input");
  const mirror = $(".ti-mirror");
  const caret = $(".ti-caret");
  const suggest = $(".term-suggest");
  const pop = $(".term-pop");

  // --------------------------------------------------------- appearance

  let charW = 7.8;
  function measure() {
    const probe = document.createElement("span");
    probe.className = "ti-probe";
    probe.textContent = "x".repeat(100);
    $(".ti-field").append(probe);
    charW = probe.getBoundingClientRect().width / 100 / (ctx.zoom?.() ?? 1) || charW;
    probe.remove();
  }
  function applySettings() {
    host.style.setProperty("--term-font-size", `${settings.fontSize}px`);
    host.dataset.cursor = settings.cursor;
    host.classList.toggle("smooth", settings.smoothCaret);
    $(".ti-shell").textContent = settings.shell === "powershell" ? "PowerShell" : settings.shell === "pwsh" ? "pwsh" : "cmd";
    requestAnimationFrame(() => { measure(); renderInput(); });
  }

  // ----------------------------------------------------------- sessions

  function newSession(cwd) {
    const s = { id: ++seq, cwd: cwd ?? active?.cwd ?? null, prev: null, el: document.createElement("div"), running: null };
    s.el.className = "term-session";
    sessions.push(s);
    body.append(s.el);
    switchSession(s);
    if (!s.cwd) start(s);
    else { renderPath(); welcome(s); }
    return s;
  }

  function switchSession(s) {
    active = s;
    for (const x of sessions) x.el.hidden = x !== s;
    renderSessions();
    renderPath();
    renderInput();
    scrollBottom(true);
  }

  function closeSession(s) {
    if (s.running) invoke("kill_process", { id: s.running.pid }).catch(() => {});
    const i = sessions.indexOf(s);
    sessions.splice(i, 1);
    s.el.remove();
    if (!sessions.length) newSession(s.cwd);
    else if (active === s) switchSession(sessions[Math.max(0, i - 1)]);
    else renderSessions();
  }

  function renderSessions() {
    const box = $(".term-sessions");
    box.innerHTML = sessions
      .map((s, i) => `<button class="ts-pill${s === active ? " on" : ""}${s.running ? " busy" : ""}" data-s="${s.id}" title="${esc(s.cwd ?? "")}">${icons.terminal}<span>${i + 1}</span>${sessions.length > 1 ? `<i data-close="${s.id}">${icons.close}</i>` : ""}</button>`)
      .join("");
  }

  // --------------------------------------------------- access & navigation

  /** True if the folder may be used; asks the user first when it's new. */
  async function ensureAccess(path) {
    const info = await invoke("access_check", { path });
    if (info.granted) return true;
    const choice = await ctx.modal(
      "Allow Flux Terminal to use this folder?",
      `${path}\n\nCommands run here can read, change and delete files in this folder and its subfolders.`,
      [
        { id: "session", label: "Allow once", primary: true },
        { id: "always", label: "Always allow" },
        { id: "all", label: "Full disk access" },
        { id: "cancel", label: "Cancel" },
      ],
    );
    if (choice === "cancel") return false;
    await invoke("access_grant", { path, scope: choice });
    if (choice === "all") ctx.toast("Full disk access granted. Revoke it in Terminal Settings.", "warning");
    return true;
  }

  /** First open without a workspace: ask before touching the disk at all. */
  function start(s) {
    const home = env.home;
    s.el.innerHTML = `
      <div class="term-gate">
        <div class="tg-icon">${icons.shield}</div>
        <div class="tg-title">Flux Terminal needs your permission</div>
        <div class="tg-text">It runs commands and opens files in the folders you allow. Nothing outside those folders is touched until you say so.</div>
        <div class="tg-actions">
          <button class="btn-primary" data-g="home">Allow ${esc(baseName(home) || home)}</button>
          <button class="btn-secondary" data-g="all">Full disk access</button>
        </div>
      </div>`;
    s.el.querySelector('[data-g="home"]').onclick = async () => {
      await invoke("access_grant", { path: home, scope: "always" });
      s.el.innerHTML = "";
      await navigate(home, { session: s, quiet: true });
      welcome(s);
    };
    s.el.querySelector('[data-g="all"]').onclick = async () => {
      await invoke("access_grant", { path: home, scope: "all" });
      s.el.innerHTML = "";
      await navigate(home, { session: s, quiet: true });
      welcome(s);
    };
    renderPath();
  }

  function welcome(s) {
    if (s.el.querySelector(".tblock")) return;
    const el = document.createElement("div");
    el.className = "tnote";
    el.innerHTML = `${icons.terminal}<span><b>Flux Terminal</b> · type a command, a folder name to go into it, <kbd>..</kbd> to go up, <kbd>ls</kbd> to browse, <kbd>Tab</kbd> to complete, <kbd>help</kbd> for more.</span>`;
    s.el.append(el);
  }

  async function navigate(target, { session = active, quiet = false } = {}) {
    if (!(await ensureAccess(target))) return false;
    if (session.cwd && session.cwd.toLowerCase() === target.toLowerCase()) return true;
    session.prev = session.cwd;
    session.cwd = target;
    try { localStorage.setItem("fluxcode.terminal.cwd", target); } catch {}
    if (!quiet) {
      const note = document.createElement("div");
      note.className = "tnav";
      note.innerHTML = `${folderIcon(true)}<span>${esc(target)}</span>`;
      note.onclick = () => navigate(target);
      session.el.append(note);
      trimBlocks(session);
      scrollBottom(true);
    }
    if (session === active) { renderPath(); renderSessions(); }
    return true;
  }

  async function goto(text) {
    if (!active?.cwd) return;
    try {
      const path = await invoke("term_resolve", { cwd: active.cwd, input: text });
      return navigate(path);
    } catch (e) {
      ctx.toast(`${e}`, "warning");
    }
  }

  // ------------------------------------------------------------ path bar

  function renderPath() {
    const cwd = active?.cwd;
    const crumbs = $(".tp-crumbs");
    if (!cwd) { crumbs.innerHTML = '<span class="tc-empty">No folder</span>'; return; }
    const parts = cwd.split(/[\\/]/).filter(Boolean);
    let acc = "";
    crumbs.innerHTML = parts
      .map((p, i) => {
        acc = i === 0 ? `${p}\\` : joinPath(acc, p);
        const last = i === parts.length - 1;
        return `${i ? `<span class="tc-sep">${icons.chevron}</span>` : ""}<button class="tc${last ? " last" : ""}" data-path="${esc(acc)}">${esc(i === 0 ? p : p)}</button>`;
      })
      .join("") + `<button class="tc-more" title="Go into a subfolder">${icons.chevron}</button>`;
    crumbs.classList.remove("anim");
    void crumbs.offsetWidth;
    crumbs.classList.add("anim");
    crumbs.scrollLeft = crumbs.scrollWidth;
  }

  async function listDir(path, fresh = false) {
    const hit = listCache.get(path.toLowerCase());
    if (!fresh && hit && performance.now() - hit.at < 3000) return hit.entries;
    const entries = await invoke("term_list", { path });
    listCache.set(path.toLowerCase(), { at: performance.now(), entries });
    if (listCache.size > 64) listCache.delete(listCache.keys().next().value);
    return entries;
  }

  function openPop(anchor, html, onPick) {
    pop.innerHTML = html;
    const r = anchor.getBoundingClientRect();
    const hr = host.getBoundingClientRect();
    pop.style.left = `${Math.max(4, Math.min(r.left - hr.left, hr.width - 280))}px`;
    pop.style.top = `${r.bottom - hr.top + 4}px`;
    pop.hidden = false;
    pop.onclick = (e) => {
      const item = e.target.closest("[data-v]");
      if (item) { pop.hidden = true; onPick(item.dataset.v, e); }
    };
  }

  async function subfolderMenu(anchor, dir) {
    let entries;
    try { entries = await listDir(dir); } catch { return; }
    const folders = entries.filter((e) => e.is_dir && (settings.showHidden || !e.hidden));
    openPop(
      anchor,
      folders.length
        ? `<div class="tp-list">${folders.map((f) => `<button data-v="${esc(f.path)}">${folderIcon(false)}<span>${esc(f.name)}</span></button>`).join("")}</div>`
        : '<div class="tp-empty">No subfolders</div>',
      (v) => navigate(v),
    );
  }

  $(".tp-crumbs").addEventListener("click", (e) => {
    const more = e.target.closest(".tc-more");
    if (more && active?.cwd) return subfolderMenu(more, active.cwd);
    const c = e.target.closest(".tc");
    if (!c) return;
    if (c.classList.contains("last")) return subfolderMenu(c, c.dataset.path);
    navigate(c.dataset.path);
  });

  $(".tp-drive").addEventListener("click", (e) => {
    openPop(
      e.currentTarget,
      `<div class="tp-list">${env.drives.map((d) => `<button data-v="${esc(d)}">${icons.drive}<span>${esc(d)}</span></button>`).join("")}</div>`,
      (v) => navigate(v),
    );
  });

  $(".term-tools").addEventListener("click", (e) => {
    const b = e.target.closest("[data-t]");
    const pill = e.target.closest(".ts-pill");
    if (pill) {
      const id = +(e.target.closest("[data-close]")?.dataset.close ?? 0);
      const s = sessions.find((x) => x.id === (id || +pill.dataset.s));
      if (id) closeSession(s); else switchSession(s);
      return focus();
    }
    if (!b) return;
    const t = b.dataset.t;
    if (t === "back" && active?.prev) navigate(active.prev);
    else if (t === "up") goto("..");
    else if (t === "home") navigate(env.home);
    else if (t === "workspace") ctx.workspace() ? navigate(ctx.workspace()) : ctx.toast("Open a folder first.", "warning");
    else if (t === "list") runLine("ls");
    else if (t === "reveal" && active?.cwd) runLine('explorer.exe .', { silentExit: true });
    else if (t === "new") newSession();
    else if (t === "settings") openSettings(b);
    focus();
  });

  $(".ti-shell").addEventListener("click", () => {
    const list = env.shells;
    settings.shell = list[(list.indexOf(settings.shell) + 1) % list.length];
    saveSettings();
    applySettings();
    focus();
  });

  function openSettings(anchor) {
    const seg = (key, opts) =>
      `<div class="seg">${opts.map(([v, label]) => `<button data-v="${key}:${v}" class="${String(settings[key]) === String(v) ? "on" : ""}">${label}</button>`).join("")}</div>`;
    openPop(
      anchor,
      `<div class="tset">
        <div class="tset-title">Terminal Settings</div>
        <label>Shell</label>${seg("shell", env.shells.map((s) => [s, s === "powershell" ? "PowerShell" : s]))}
        <label>Font size</label>${seg("fontSize", [[12, "12"], [13, "13"], [14, "14"], [15, "15"], [16, "16"]])}
        <label>Cursor</label>${seg("cursor", [["bar", "Bar"], ["block", "Block"], ["underline", "Line"]])}
        <label>Smooth caret</label>${seg("smoothCaret", [[true, "On"], [false, "Off"]])}
        <label>Hidden files in ls</label>${seg("showHidden", [[true, "Show"], [false, "Hide"]])}
        <button class="tset-danger" data-v="reset:1">${icons.shield}<span>Revoke all folder permissions</span></button>
      </div>`,
      async (v, e) => {
        const [key, raw] = v.split(":");
        if (key === "reset") {
          await invoke("access_reset");
          if (ctx.workspace()) await invoke("access_grant", { path: ctx.workspace(), scope: "session" });
          ctx.toast("All folder permissions were revoked.");
          return;
        }
        settings[key] = raw === "true" ? true : raw === "false" ? false : /^\d+$/.test(raw) ? +raw : raw;
        saveSettings();
        applySettings();
        openSettings(anchor); // keep the panel open to show the new state
        e.stopPropagation();
      },
    );
  }

  document.addEventListener("mousedown", (e) => {
    if (!pop.hidden && !pop.contains(e.target) && !e.target.closest(".tp-drive, .tc, .tc-more, [data-t=settings]")) pop.hidden = true;
  });

  // ------------------------------------------------------------- blocks

  function trimBlocks(s) {
    while (s.el.children.length > MAX_BLOCKS) s.el.firstElementChild.remove();
  }

  let stick = true; // follow output while the user hasn't scrolled up
  body.addEventListener("scroll", () => { stick = body.scrollHeight - body.scrollTop - body.clientHeight < 40; });
  function scrollBottom(force = false) {
    if (force) stick = true;
    if (stick) body.scrollTop = body.scrollHeight;
  }

  function makeBlock(s, line, cwd) {
    const b = document.createElement("div");
    b.className = "tblock running";
    b.innerHTML = `
      <div class="tb-head">
        <span class="tb-status"><span class="spinner"></span></span>
        <button class="tb-cwd" title="${esc(cwd)}">${esc(baseName(cwd) || cwd)}</button>
        <span class="tb-cmd"></span>
        <span class="tb-meta"></span>
        <span class="tb-actions">
          <button data-a="copy" title="Copy output">${icons.copy}</button>
          <button data-a="rerun" title="Run again">${icons.rerun}</button>
          <button data-a="stop" title="Stop (Ctrl+C)">${icons.stop}</button>
        </span>
      </div>
      <div class="tb-out"><span class="tb-done"></span><span class="tb-live"></span></div>`;
    b.querySelector(".tb-cmd").innerHTML = highlightLine(line);
    b.dataset.cwd = cwd;
    b.dataset.line = line;
    b.out = { st: {}, cr: false, chars: 0, queue: [] };
    s.el.append(b);
    trimBlocks(s);
    scrollBottom(true);
    return b;
  }

  function finishBlock(b, code, ms) {
    b.classList.remove("running");
    b.classList.add(code === 0 ? "ok" : "err");
    b.querySelector(".tb-status").innerHTML = code === 0 ? icons.check : icons.close;
    b.querySelector(".tb-meta").textContent = `${code === 0 ? "" : `exit ${code ?? "killed"} · `}${fmtTime(ms)}`;
    const out = b.querySelector(".tb-out");
    if (!out.textContent.trim()) out.classList.add("empty");
    linkify(out);
  }

  /** Queue output; it is written once per animation frame (cheap for floods of output). */
  let flushQueued = false;
  const dirty = new Set();
  function write(b, text) {
    b.out.queue.push(text);
    dirty.add(b);
    if (!flushQueued) { flushQueued = true; requestAnimationFrame(flush); }
  }
  function flush() {
    flushQueued = false;
    for (const b of dirty) {
      const text = b.out.queue.join("");
      b.out.queue = [];
      render(b, text);
    }
    dirty.clear();
    scrollBottom();
  }

  function render(b, text) {
    const done = b.querySelector(".tb-done");
    const live = b.querySelector(".tb-live");
    const o = b.out;
    let last = 0;
    const emit = (chunk) => {
      for (const part of chunk.split(/(\r\n|\n|\r)/)) {
        if (!part) continue;
        if (part === "\n" || part === "\r\n") {
          o.cr = false;
          while (live.firstChild) done.append(live.firstChild);
          done.append("\n");
          o.chars += 1;
        } else if (part === "\r") {
          o.cr = true; // the next text overwrites the line (progress bars)
        } else {
          if (o.cr) { live.replaceChildren(); o.cr = false; }
          const tail = live.lastChild;
          const k = styleKey(o.st);
          if (tail && tail.dataset?.k === k) tail.textContent += part;
          else live.append(styleSpan(o.st, part));
          o.chars += part.length;
        }
      }
    };
    for (const m of text.matchAll(ESC_RE)) {
      emit(text.slice(last, m.index));
      if (m[2] === "m") applySgr(o.st, m[1]);
      last = m.index + m[0].length;
    }
    emit(text.slice(last));
    while (o.chars > MAX_BLOCK_CHARS && done.firstChild) {
      o.chars -= done.firstChild.textContent.length;
      done.firstChild.remove();
    }
  }

  /** Python tracebacks and `path:line` become clickable. */
  function linkify(out) {
    const re = /File "([^"]+)", line (\d+)|([A-Za-z]:\\[^\s:"'<>|]+\.[A-Za-z0-9]+):(\d+)/g;
    const walker = document.createTreeWalker(out, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) if (/File "|[A-Za-z]:\\/.test(walker.currentNode.nodeValue)) nodes.push(walker.currentNode);
    for (const node of nodes) {
      const text = node.nodeValue;
      const frag = document.createDocumentFragment();
      let last = 0;
      for (const m of text.matchAll(re)) {
        frag.append(text.slice(last, m.index));
        const a = document.createElement("a");
        a.textContent = m[0];
        a.dataset.file = m[1] ?? m[3];
        a.dataset.line = m[2] ?? m[4];
        frag.append(a);
        last = m.index + m[0].length;
      }
      if (!last) continue;
      frag.append(text.slice(last));
      node.replaceWith(frag);
    }
  }

  body.addEventListener("click", (e) => {
    const a = e.target.closest("a[data-file]");
    if (a) return ctx.openFile(a.dataset.file, +a.dataset.line);
    const entry = e.target.closest(".tl-item");
    if (entry) return entry.dataset.dir ? navigate(entry.dataset.path) : ctx.openFile(entry.dataset.path);
    const cwdBtn = e.target.closest(".tb-cwd");
    if (cwdBtn) return navigate(cwdBtn.closest(".tblock").dataset.cwd);
    const act = e.target.closest("[data-a]");
    if (!act) return;
    const b = act.closest(".tblock");
    if (act.dataset.a === "copy") {
      navigator.clipboard.writeText(b.querySelector(".tb-out").innerText.trimEnd()).then(() => ctx.toast("Output copied."), () => {});
    } else if (act.dataset.a === "rerun") {
      navigate(b.dataset.cwd).then((ok) => ok && runLine(b.dataset.line));
    } else if (act.dataset.a === "stop" && b.pid) {
      invoke("kill_process", { id: b.pid }).catch(() => {});
    }
  });

  // ------------------------------------------------------------ builtins

  async function builtinLs(s, arg) {
    const all = /(^|\s)-a\b/.test(arg);
    const target = arg.replace(/(^|\s)-a\b/, "").trim();
    let dir = s.cwd;
    if (target) {
      try { dir = await invoke("term_resolve", { cwd: s.cwd, input: target }); } catch (e) { return note(s, `${e}`, "err"); }
      if (!(await ensureAccess(dir))) return;
    }
    const b = makeBlock(s, `ls${arg ? ` ${arg}` : ""}`, s.cwd);
    const t0 = performance.now();
    let entries;
    try { entries = await listDir(dir, true); } catch (e) { write(b, `${e}\n`); return finishBlock(b, 1, performance.now() - t0); }
    const shown = entries.filter((x) => all || settings.showHidden || !x.hidden);
    const folders = shown.filter((x) => x.is_dir).length;
    const out = b.querySelector(".tb-out");
    out.innerHTML = `<div class="tl-grid">${shown
      .map((x) => `<button class="tl-item${x.hidden ? " hidden-file" : ""}" ${x.is_dir ? 'data-dir="1"' : ""} data-path="${esc(x.path)}" title="${esc(x.name)}${x.is_dir ? "" : ` · ${fmtSize(x.size)}`}">${x.is_dir ? folderIcon(false) : fileIcon(x.name)}<span>${esc(x.name)}</span>${x.is_dir ? "" : `<em>${fmtSize(x.size)}</em>`}</button>`)
      .join("")}</div><div class="tl-sum">${folders} folder${folders === 1 ? "" : "s"}, ${shown.length - folders} file${shown.length - folders === 1 ? "" : "s"}${entries.length > shown.length ? ` · ${entries.length - shown.length} hidden` : ""}</div>`;
    finishBlock(b, 0, performance.now() - t0);
    scrollBottom(true);
  }

  function note(s, text, kind = "") {
    const el = document.createElement("div");
    el.className = `tnote ${kind}`;
    el.innerHTML = `${kind === "err" ? icons.warning : icons.info}<span>${text}</span>`;
    s.el.append(el);
    scrollBottom(true);
  }

  const HELP = [
    ["folder name, path, ..  ...  ~  -  D:", "go to that folder (no cd needed)"],
    ["ls / dir  [-a] [path]", "show a folder as clickable icons"],
    ["open <file>", "open a file in the editor"],
    ["clear / cls  (Ctrl+L)", "clear this terminal"],
    ["history", "show recent commands"],
    ["Tab · → · ↑ ↓", "complete paths · accept suggestion · history"],
    ["Ctrl+C", "stop the running command"],
    ["Alt+↑ · Alt+←", "up one folder · back"],
    ["anything else", `runs in ${settings.shell === "cmd" ? "cmd" : "PowerShell"} (switch with the shell button)`],
  ];

  // --------------------------------------------------------------- running

  async function runLine(line, { silentExit = false } = {}) {
    const s = active;
    if (!s?.cwd) return;
    const text = line.trim();
    if (!text) return;
    const [, cmd = "", rest = ""] = text.match(/^(\S+)\s*(.*)$/) ?? [];
    const lc = cmd.toLowerCase();

    // Built-ins handled natively (instant, no process).
    if (lc === "clear" || lc === "cls") return clear();
    if (lc === "exit") return closeSession(s);
    if (lc === "pwd" || (lc === "cd" && !rest)) return note(s, esc(s.cwd));
    if (lc === "cd" || lc === "chdir") return goto(rest.replace(/^\/d\s+/i, ""));
    if ((lc === "ls" || lc === "dir") && !/(^|\s)\/[a-z]/i.test(rest) && !/-[b-z]/i.test(rest)) return builtinLs(s, rest);
    if (lc === "open" || lc === "code") {
      try {
        const path = rest.replace(/^"|"$/g, "");
        return ctx.openFile(/^[A-Za-z]:[\\/]/.test(path) ? path : joinPath(s.cwd, path));
      } catch { return; }
    }
    if (lc === "help") {
      return note(s, `<div class="thelp">${HELP.map(([k, v]) => `<code>${esc(k)}</code><span>${esc(v)}</span>`).join("")}</div>`);
    }
    if (lc === "history") return note(s, history.slice(-25).map((h, i) => `${String(history.length - 25 + i + 1).padStart(4)}  ${esc(h)}`).join("<br>") || "No history yet.");
    if (lc === "-" ) return s.prev ? navigate(s.prev) : note(s, "No previous folder.");

    // Just a folder? Go there (unless it's also the name of a program).
    if (!rest && !/[|&<>]/.test(cmd)) {
      const pathish = /^(\.{2,}|~|[A-Za-z]:|.*[\\/].*)$/.test(cmd) || cmd === "..";
      if (pathish || !(await invoke("term_is_command", { name: cmd }))) {
        try {
          const path = await invoke("term_resolve", { cwd: s.cwd, input: cmd });
          return navigate(path);
        } catch {
          if (pathish) return note(s, `'${esc(cmd)}' is not a folder.`, "err");
        }
      }
    }

    if (s.running) return note(s, "A command is still running here. Stop it (Ctrl+C) or open a new terminal (+).", "err");
    if (!(await ensureAccess(s.cwd))) return;
    const b = makeBlock(s, text, s.cwd);
    b.start = performance.now();
    try {
      b.pid = await invoke("term_run", { line: text, cwd: s.cwd, shell: settings.shell });
    } catch (e) {
      write(b, `${e === "access-denied" ? "Access to this folder was not allowed." : e}\n`);
      return finishBlock(b, 1, 0);
    }
    b.silentExit = silentExit;
    procs.set(b.pid, { b, s });
    s.running = { pid: b.pid, b };
    renderSessions();
    renderInput();
  }

  listen("proc-output", ({ payload }) => {
    const p = procs.get(payload.id);
    if (p) write(p.b, payload.data);
  });
  listen("proc-exit", ({ payload }) => {
    const p = procs.get(payload.id);
    if (!p) return;
    procs.delete(payload.id);
    flush();
    // `explorer.exe` returns 1 even on success; don't show that as a failure.
    finishBlock(p.b, p.b.silentExit ? 0 : payload.code, performance.now() - p.b.start);
    p.s.running = null;
    listCache.clear(); // the command may have changed files
    renderSessions();
    renderInput();
    scrollBottom();
  });

  function clear() {
    if (!active) return;
    active.el.replaceChildren();
    if (active.running) active.el.append(active.running.b);
  }

  // ------------------------------------------------------------ input line

  /** Colored command line: command, flags, strings, paths, operators. */
  function highlightLine(text, newAt = -1) {
    let out = "";
    let first = true;
    let pos = 0;
    for (const m of text.matchAll(/(\s+)|("[^"]*"?|'[^']*'?)|([|&<>;]+)|([^\s"'|&<>;]+)/g)) {
      const [tok, ws, str, op, word] = m;
      let cls = "";
      if (ws) cls = "";
      else if (str) cls = "tk-str";
      else if (op) { cls = "tk-op"; first = true; }
      else if (word) {
        if (first) cls = BUILTINS.includes(word.toLowerCase()) ? "tk-builtin" : "tk-cmd";
        else if (/^(-{1,2}[\w-]|\/[A-Za-z?]$)/.test(word)) cls = "tk-flag";
        else if (/[\\/]|^~|^\.\.?$|^[A-Za-z]:/.test(word)) cls = "tk-path";
        else if (/^\d+(\.\d+)?$/.test(word)) cls = "tk-num";
        first = false;
      }
      // Wrap characters individually only around the freshly typed one (for its animation).
      if (newAt >= pos && newAt < pos + tok.length) {
        const i = newAt - pos;
        out += `<span class="${cls}">${esc(tok.slice(0, i))}<span class="nw">${esc(tok[i])}</span>${esc(tok.slice(i + 1))}</span>`;
      } else out += cls ? `<span class="${cls}">${esc(tok)}</span>` : esc(tok);
      pos += tok.length;
    }
    return out;
  }

  let ghost = "";
  let lastTyped = -1;
  let histIdx = -1;
  let draft = "";
  let typingTimer = 0;

  async function computeGhost() {
    const v = input.value;
    ghost = "";
    if (!v || input.selectionStart !== v.length) return;
    const h = [...history].reverse().find((x) => x.startsWith(v) && x !== v);
    if (h) ghost = h.slice(v.length);
    else if (!/\s/.test(v) && active?.cwd) {
      // Single word: suggest a folder in the current directory (type + Enter = go there).
      try {
        const entries = await listDir(active.cwd);
        const f = entries.find((e) => e.is_dir && !e.hidden && e.name.toLowerCase().startsWith(v.toLowerCase()) && e.name.length > v.length);
        if (f && input.value === v) ghost = f.name.slice(v.length);
      } catch {}
    }
    renderInput();
  }

  function renderInput() {
    const running = !!active?.running;
    host.classList.toggle("stdin", running);
    $(".ti-prompt").title = running ? "Input goes to the running program" : active?.cwd ?? "";
    input.placeholder = running ? "Send input to the running program…" : active?.cwd ? `Type a command or a folder name…` : "";
    mirror.innerHTML = highlightLine(input.value, lastTyped) + (ghost ? `<span class="tk-ghost">${esc(ghost)}</span>` : "");
    lastTyped = -1;
    mirror.style.transform = `translateX(${-input.scrollLeft}px)`;
    placeCaret();
  }

  function placeCaret() {
    const { selectionStart: a, selectionEnd: b } = input;
    caret.hidden = a !== b || document.activeElement !== input;
    caret.style.transform = `translateX(${a * charW - input.scrollLeft}px)`;
    caret.style.width = settings.cursor === "block" ? `${charW}px` : "";
  }

  input.addEventListener("input", (e) => {
    if (e.inputType === "insertText" && e.data?.length === 1) lastTyped = input.selectionStart - 1;
    histIdx = -1;
    ghost = "";
    host.classList.add("typing");
    clearTimeout(typingTimer);
    typingTimer = setTimeout(() => host.classList.remove("typing"), 550);
    hideSuggest();
    renderInput();
    computeGhost();
  });
  for (const ev of ["keyup", "click", "select", "focus", "blur", "scroll"]) input.addEventListener(ev, () => {
    mirror.style.transform = `translateX(${-input.scrollLeft}px)`;
    placeCaret();
  });

  // ---------------------------------------------------------- completion

  let sugg = { items: [], index: 0, start: 0, end: 0 };
  function hideSuggest() { suggest.hidden = true; sugg.items = []; }

  async function complete() {
    const v = input.value;
    const caretPos = input.selectionStart;
    const before = v.slice(0, caretPos);
    const m = before.match(/("[^"]*|[^\s"]*)$/);
    const token = m[0].replace(/^"/, "");
    const start = caretPos - m[0].length;
    const cut = sepIdx(token);
    const dirPart = cut >= 0 ? token.slice(0, cut + 1) : "";
    const prefix = token.slice(cut + 1).toLowerCase();
    let dir;
    try { dir = dirPart ? await invoke("term_resolve", { cwd: active.cwd, input: dirPart }) : active.cwd; } catch { return; }
    let entries;
    try { entries = await listDir(dir); } catch { return; }
    let items = entries
      .filter((e) => e.name.toLowerCase().startsWith(prefix) && (prefix.startsWith(".") || settings.showHidden || !e.hidden))
      .map((e) => ({ label: e.name, insert: dirPart + e.name + (e.is_dir ? "\\" : ""), dir: e.is_dir }));
    if (start === 0 && !dirPart) items = [...BUILTINS.filter((b) => b.startsWith(prefix)).map((b) => ({ label: b, insert: `${b} `, builtin: true })), ...items];
    if (!items.length) return;
    sugg = { items: items.slice(0, 50), index: 0, start, end: caretPos };
    if (items.length === 1) return acceptSuggestion(0);
    // Extend to the common prefix, then show the choices.
    let common = items[0].insert;
    for (const it of items) while (!it.insert.toLowerCase().startsWith(common.toLowerCase())) common = common.slice(0, -1);
    if (common.length > token.length) replaceRange(start, caretPos, quoteIfNeeded(common));
    showSuggest();
  }

  const quoteIfNeeded = (s) => (/\s/.test(s) ? `"${s}"` : s);

  function replaceRange(a, b, text) {
    input.setRangeText(text, a, b, "end");
    sugg.end = a + text.length;
    renderInput();
  }

  function acceptSuggestion(i) {
    const it = sugg.items[i];
    if (!it) return;
    replaceRange(sugg.start, sugg.end, quoteIfNeeded(it.insert));
    hideSuggest();
    if (it.dir) computeGhost();
  }

  function showSuggest() {
    suggest.innerHTML = sugg.items
      .map((it, i) => `<li class="${i === sugg.index ? "on" : ""}" data-i="${i}">${it.builtin ? icons.terminal : it.dir ? folderIcon(false) : fileIcon(it.label)}<span>${esc(it.label)}</span>${it.builtin ? "<em>built-in</em>" : ""}</li>`)
      .join("");
    const fieldRect = $(".ti-field").getBoundingClientRect();
    const hostRect = host.getBoundingClientRect();
    suggest.style.left = `${fieldRect.left - hostRect.left + Math.max(0, sugg.start * charW - input.scrollLeft)}px`;
    suggest.style.bottom = `${hostRect.bottom - fieldRect.top + 4}px`;
    suggest.hidden = false;
  }

  suggest.addEventListener("mousedown", (e) => {
    const li = e.target.closest("li[data-i]");
    if (li) { e.preventDefault(); acceptSuggestion(+li.dataset.i); }
  });

  input.addEventListener("keydown", async (e) => {
    const k = `${e.ctrlKey ? "ctrl+" : ""}${e.altKey ? "alt+" : ""}${e.shiftKey ? "shift+" : ""}${e.key}`;
    if (!suggest.hidden) {
      if (k === "ArrowDown" || k === "ArrowUp" || k === "Tab" || k === "shift+Tab") {
        e.preventDefault();
        const d = k === "ArrowUp" || k === "shift+Tab" ? -1 : 1;
        sugg.index = (sugg.index + d + sugg.items.length) % sugg.items.length;
        return showSuggest();
      }
      if (k === "Enter") { e.preventDefault(); return acceptSuggestion(sugg.index); }
      if (k === "Escape") { e.preventDefault(); return hideSuggest(); }
    }
    if (k === "Enter") {
      e.preventDefault();
      const line = input.value;
      if (active?.running) {
        write(active.running.b, `\x1b[36m${line}\x1b[0m\n`);
        invoke("write_stdin", { id: active.running.pid, data: `${line}\n` }).catch((err) => ctx.toast(`${err}`, "error"));
        input.value = "";
        return renderInput();
      }
      if (line.trim() && history.at(-1) !== line.trim()) { history.push(line.trim()); saveHistory(); }
      input.value = "";
      ghost = "";
      histIdx = -1;
      renderInput();
      return runLine(line);
    }
    if (k === "Tab") { e.preventDefault(); if (active?.cwd && !active.running) complete(); return; }
    if ((k === "ArrowRight" || k === "End") && ghost && input.selectionStart === input.value.length) {
      e.preventDefault();
      input.value += ghost;
      ghost = "";
      return renderInput();
    }
    if (k === "ArrowUp" || k === "ArrowDown") {
      if (!history.length) return;
      e.preventDefault();
      if (histIdx === -1) { draft = input.value; histIdx = history.length; }
      histIdx = Math.min(Math.max(histIdx + (k === "ArrowUp" ? -1 : 1), 0), history.length);
      input.value = histIdx === history.length ? draft : history[histIdx];
      ghost = "";
      requestAnimationFrame(() => { input.setSelectionRange(input.value.length, input.value.length); renderInput(); });
      return;
    }
    if (k === "ctrl+c" && input.selectionStart === input.selectionEnd && active?.running) {
      e.preventDefault();
      write(active.running.b, "^C\n");
      return invoke("kill_process", { id: active.running.pid }).catch(() => {});
    }
    if (k === "ctrl+l") { e.preventDefault(); return clear(); }
    if (k === "Escape") { e.preventDefault(); input.value = ""; ghost = ""; return renderInput(); }
    if (k === "alt+ArrowUp") { e.preventDefault(); return goto(".."); }
    if (k === "alt+ArrowLeft") { e.preventDefault(); if (active?.prev) navigate(active.prev); }
  });

  // Clicking empty space in the terminal focuses the input (but keep text selection possible).
  host.addEventListener("mouseup", (e) => {
    if (!getSelection().toString() && !e.target.closest("button, a, input, .term-pop, .tl-item")) focus();
  });

  function focus() {
    input.focus();
    placeCaret();
  }

  // ------------------------------------------------------------------ boot

  applySettings();
  const ready = invoke("term_env").then((e) => { env = e; if (!env.shells.includes(settings.shell)) settings.shell = "cmd"; applySettings(); });
  const remembered = (() => { try { return localStorage.getItem("fluxcode.terminal.cwd"); } catch { return null; } })();

  async function boot() {
    await ready;
    const ws = ctx.workspace();
    let cwd = ws;
    if (!cwd && remembered && (await invoke("access_check", { path: remembered })).granted) cwd = remembered;
    newSession(cwd ?? null);
  }
  const booted = boot();

  return {
    focus: () => booted.then(focus),
    newSession: () => booted.then(() => { newSession(); focus(); }),
    clear: () => booted.then(clear),
    stop: () => active?.running && invoke("kill_process", { id: active.running.pid }),
    remeasure: () => { measure(); placeCaret(); },
    /** The workspace changed: new terminals start there. */
    setWorkspace: (path) => booted.then(() => { if (active && !active.cwd && path) { active.el.innerHTML = ""; navigate(path, { quiet: true }).then(() => welcome(active)); } }),
  };
}

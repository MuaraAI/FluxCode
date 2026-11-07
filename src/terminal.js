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
import { prefs } from "./prefs.js";

const HISTORY_KEY = "fluxcode.terminal.history";
const MAX_BLOCKS = 150;
const MAX_BLOCK_CHARS = 300_000;
const BUILTINS = ["cd", "ls", "dir", "clear", "cls", "open", "code", "help", "history", "exit", "pwd"];

const esc = (s) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const sepIdx = (p) => Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/"));
const baseName = (p) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const joinPath = (dir, name) =>
  dir === "/" ? `/${name}` : (/[\\/]$/.test(dir) ? dir + name : (dir.includes("\\") ? `${dir}\\${name}` : `${dir}/${name}`));
const fmtSize = (n) =>
  n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : n < 1073741824 ? `${(n / 1048576).toFixed(1)} MB` : `${(n / 1073741824).toFixed(2)} GB`;
const fmtTime = (ms) => (ms < 1000 ? `${Math.round(ms)} ms` : ms < 60000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`);

// ---------------------------------------------------------------- ANSI → DOM

// The 16 ANSI colors come from the theme (CSS variables, contrast-adjusted per theme).
const ANSI16 = Array.from({ length: 16 }, (_, i) => `var(--ansi${i})`);
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
  // Settings live in the shared store (the Settings panel edits the same values).
  let settings = prefs.get("terminal");
  prefs.on("terminal", (next) => { settings = next; applySettings(); });
  const setSetting = (key, value) => prefs.set("terminal", { [key]: value });
  let history = [];
  try { history = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]"); } catch {}
  const saveHistory = () => { try { localStorage.setItem(HISTORY_KEY, JSON.stringify(history.slice(-300))); } catch {} };

  let env = { home: "/", drives: ["/"], shells: ["bash", "sh"], user: "", computer: "", platform: "linux" };
  const sessions = [];
  let active = null;
  let seq = 0;
  const procs = new Map(); // process id -> block
  const listCache = new Map(); // folder -> { at, entries }

  // The path bar + tools live in the panel header (saves a whole row of height).
  const top = document.createElement("div");
  top.innerHTML = `
    <div class="term-top">
      <button class="tp-drive" title="Switch drive">${icons.drive}<span class="tp-drive-chev">${icons.chevron}</span></button>
      <div class="tp-crumbs"></div>
      <div class="term-tools">
        <button data-t="back" title="Back to previous folder (Alt+Left)">${icons.back}</button>
        <button data-t="up" title="Up one folder (Alt+Up)">${icons.up}</button>
        <button data-t="home" title="Home folder (~)">${icons.home}</button>
        <button data-t="workspace" title="Workspace folder">${icons.workspace}</button>
        <button data-t="list" title="Show this folder (ls)">${icons.list}</button>
        <button data-t="reveal" title="Show this folder in File Explorer">${icons.folderOpen}</button>
        <span class="tt-sep"></span>
        <div class="term-sessions"></div>
        <button data-t="new" title="New Terminal (Ctrl+Shift+\`)">${icons.plus}</button>
        <button data-t="settings" title="Terminal Settings">${icons.settings}</button>
      </div>
    </div>`;
  const topBar = top.firstElementChild;
  (ctx.toolbarHost ?? host).append(topBar);

  host.innerHTML = `
    <div class="term-body"></div>
    <div class="term-input">
      <span class="ti-prompt">${icons.chevron}</span>
      <div class="ti-field">
        <div class="ti-mirror"></div>
        <input class="ti-input" spellcheck="false" autocomplete="off" autocorrect="off" />
        <div class="ti-caret"></div>
      </div>
      <button class="ti-shell" title="Shell for commands (click to switch)"></button>
    </div>`;

  const $ = (s) => host.querySelector(s) ?? topBar.querySelector(s);
  const body = $(".term-body");
  const input = $(".ti-input");
  const mirror = $(".ti-mirror");
  const caret = $(".ti-caret");
  // Floating UI lives on <body> with fixed positioning, clamped to the window.
  const suggest = document.createElement("ul");
  suggest.className = "term-suggest";
  suggest.hidden = true;
  const pop = document.createElement("div");
  pop.className = "term-pop";
  pop.hidden = true;
  document.body.append(suggest, pop);

  /** Place a fixed popup next to `rect`, flipping above and clamping so it never leaves the window. */
  function placeFloating(el, rect, { prefer = "below", alignRight = false } = {}) {
    el.style.maxHeight = "";
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = alignRight ? rect.right - w : rect.left;
    left = Math.max(8, Math.min(left, vw - w - 8));
    const below = vh - rect.bottom - 12;
    const above = rect.top - 12;
    const showBelow = prefer === "below" ? h <= below || below >= above : !(h <= above || above >= below);
    const room = showBelow ? below : above;
    if (h > room) el.style.maxHeight = `${room}px`;
    const top = showBelow ? rect.bottom + 4 : rect.top - Math.min(h, room) - 4;
    el.style.left = `${left}px`;
    el.style.top = `${Math.max(8, top)}px`;
  }

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
    if (s.el.querySelector(".tblock, .tnote")) return;
    try {
      if (localStorage.getItem("fluxcode.terminal.welcomed")) return;
      localStorage.setItem("fluxcode.terminal.welcomed", "1");
    } catch {}
    const el = document.createElement("div");
    el.className = "tnote";
    el.innerHTML = `${icons.terminal}<span><b>Flux Terminal</b> · type a command or a folder name · <kbd>..</kbd> up · <kbd>ls</kbd> browse · <kbd>Tab</kbd> complete · <kbd>help</kbd></span>`;
    s.el.append(el);
  }

  async function navigate(target, { session = active, quiet = false } = {}) {
    if (!(await ensureAccess(target))) return false;
    if (session.cwd && session.cwd.toLowerCase() === target.toLowerCase()) return true;
    session.prev = session.cwd;
    session.cwd = target;
    try { localStorage.setItem("fluxcode.terminal.cwd", target); } catch {}
    if (!quiet) {
      // A small "moved to" pill; consecutive moves update the same pill instead of stacking.
      const last = session.el.lastElementChild;
      const note = last?.classList.contains("tnav") ? last : document.createElement("div");
      note.className = "tnav";
      const parent = target.slice(0, Math.max(0, sepIdx(target.replace(/[\/]$/, ""))));
      note.innerHTML = `<span class="tnav-pill" title="${esc(target)}">${icons.chevron}${folderIcon(true)}<b>${esc(baseName(target) || target)}</b></span><span class="tnav-parent">${esc(parent)}</span>`;
      note.onclick = () => navigate(target);
      if (note !== last) session.el.append(note);
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
    const isWin = env.platform === "windows" || /^[A-Za-z]:/.test(cwd);
    const parts = cwd.split(/[\\/]/).filter(Boolean);
    let acc = "";
    const items = [];
    if (!isWin && cwd.startsWith("/")) {
      acc = "/";
      items.push({ path: "/", label: "/" });
    }
    for (let i = 0; i < parts.length; i++) {
      const p = parts[i];
      if (isWin && i === 0) {
        acc = `${p}\\`;
      } else {
        acc = joinPath(acc, p);
      }
      items.push({ path: acc, label: p });
    }
    crumbs.innerHTML = items
      .map((item, i) => {
        const last = i === items.length - 1;
        return `${i ? `<span class="tc-sep">${icons.chevron}</span>` : ""}<button class="tc${last ? " last" : ""}" data-path="${esc(item.path)}">${esc(item.label)}</button>`;
      })
      .join("") + `<button class="tc-more" title="Go into a subfolder">${icons.chevron}</button>`;
    crumbs.classList.remove("anim");
    void crumbs.offsetWidth;
    crumbs.classList.add("anim");
    crumbs.querySelectorAll(".tc").forEach((el, i, all) => { el.style.animationDelay = `${Math.max(0, all.length - 1 - i) * 18}ms`; });
    fitCrumbs();
  }

  /** Long paths: fold the middle folders into a "…" menu so the drive and current folder stay visible. */
  function fitCrumbs() {
    const crumbs = $(".tp-crumbs");
    crumbs.querySelector(".tc-ellipsis")?.previousElementSibling?.remove();
    crumbs.querySelector(".tc-ellipsis")?.remove();
    const items = [...crumbs.querySelectorAll(".tc")];
    for (const el of crumbs.children) el.hidden = false;
    if (crumbs.scrollWidth <= crumbs.clientWidth + 1 || items.length < 3) return;
    const sep = document.createElement("span");
    sep.className = "tc-sep";
    sep.innerHTML = icons.chevron;
    const more = document.createElement("button");
    more.className = "tc tc-ellipsis";
    more.textContent = "…";
    items[0].after(sep, more);
    const hidden = [];
    for (let i = 1; i < items.length - 1 && crumbs.scrollWidth > crumbs.clientWidth + 1; i++) {
      items[i].hidden = true;
      items[i].previousElementSibling.hidden = true; // its separator
      hidden.push(items[i].dataset.path);
    }
    more.dataset.paths = JSON.stringify(hidden);
    more.title = hidden.at(-1) ?? "";
  }
  new ResizeObserver(() => fitCrumbs()).observe(topBar);

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
    pop.hidden = false;
    const r = anchor.getBoundingClientRect();
    placeFloating(pop, r, { alignRight: r.left > window.innerWidth / 2 });
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
    const ell = e.target.closest(".tc-ellipsis");
    if (ell) {
      const paths = JSON.parse(ell.dataset.paths || "[]");
      return openPop(
        ell,
        `<div class="tp-list">${paths.map((p) => `<button data-v="${esc(p)}">${folderIcon(false)}<span>${esc(baseName(p))}</span></button>`).join("")}</div>`,
        (v) => navigate(v),
      );
    }
    const c = e.target.closest(".tc");
    if (!c) return;
    if (c.classList.contains("last")) return subfolderMenu(c, c.dataset.path);
    navigate(c.dataset.path);
  });

  $(".tp-drive").addEventListener("click", (e) => {
    if (env.platform === "linux" || env.drives.length <= 1) {
      return navigate(env.drives[0] ?? "/");
    }
    const current = (active?.cwd ?? "").slice(0, 3).toUpperCase();
    openPop(
      e.currentTarget,
      `<div class="tp-list tp-drives">${env.drives.map((d) => `<button data-v="${esc(d)}" class="${d.toUpperCase() === current ? "on" : ""}">${icons.drive}<span>${esc(d)}</span>${d.toUpperCase() === current ? icons.check : ""}</button>`).join("")}</div>`,
      (v) => navigate(v),
    );
  });


  window.addEventListener("resize", () => { pop.hidden = true; hideSuggest(); });

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
    setSetting("shell", list[(list.indexOf(settings.shell) + 1) % list.length]);
    focus();
  });

  function openSettings(anchor) {
    const seg = (key, opts) =>
      `<div class="seg" data-key="${key}">${opts.map(([v, label]) => `<button data-set="${key}:${v}" class="${String(settings[key]) === String(v) ? "on" : ""}">${label}</button>`).join("")}</div>`;
    const row = (label, hint, control) => `<div class="tset-row"><div class="tset-text"><span>${label}</span>${hint ? `<small>${hint}</small>` : ""}</div>${control}</div>`;
    pop.innerHTML = `<div class="tset">
        <div class="tset-head">${icons.terminal}<div><b>Terminal</b><small>Flux Terminal settings</small></div><button class="tset-all" data-set="all:1">All settings ${icons.chevron}</button></div>
        ${row("Shell", "for commands you type", seg("shell", env.shells.map((s) => [s, s === "powershell" ? "PowerShell" : s])))}
        ${row("Font size", "", seg("fontSize", [[12, "12"], [13, "13"], [14, "14"], [15, "15"], [16, "16"]]))}
        ${row("Cursor", "", seg("cursor", [["bar", "Bar"], ["block", "Block"], ["underline", "Line"]]))}
        ${row("Smooth caret", "glides between positions", seg("smoothCaret", [[true, "On"], [false, "Off"]]))}
        ${row("Hidden files", "in <code>ls</code> listings", seg("showHidden", [[true, "Show"], [false, "Hide"]]))}
        <div class="tset-foot"><button class="tset-danger" data-set="reset:1">${icons.shield}<span>Revoke folder permissions</span></button></div>
      </div>`;
    pop.hidden = false;
    const r = anchor.getBoundingClientRect();
    placeFloating(pop, r, { alignRight: r.left > window.innerWidth / 2 });
    pop.onclick = async (e) => {
      const b = e.target.closest("[data-set]");
      if (!b) return;
      const [key, raw] = b.dataset.set.split(":");
      if (key === "all") { pop.hidden = true; return ctx.openSettings?.(); }
      if (key === "reset") {
        const choice = await ctx.modal("Revoke all folder permissions?", "Flux Terminal will ask again before running commands in any folder.", [{ id: "yes", label: "Revoke", primary: true }, { id: "cancel", label: "Cancel" }]);
        if (choice !== "yes") return;
        await invoke("access_reset");
        if (ctx.workspace()) await invoke("access_grant", { path: ctx.workspace(), scope: "session" });
        ctx.toast("All folder permissions were revoked.");
        return;
      }
      setSetting(key, raw === "true" ? true : raw === "false" ? false : /^\d+$/.test(raw) ? +raw : raw);
      // Update the segmented control in place (no re-render, no flicker).
      for (const x of b.parentElement.children) x.classList.toggle("on", x === b);
    };
  }

  document.addEventListener("mousedown", (e) => {
    if (!pop.hidden && !pop.contains(e.target) && !e.target.closest(".tp-drive, .tc, .tc-more, .tc-ellipsis, [data-t=settings]")) pop.hidden = true;
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
    const re = /File "([^"]+)", line (\d+)|((?:\/[^\s:"'<>|]+\.[A-Za-z0-9]+|[A-Za-z]:\\[^\s:"'<>|]+\.[A-Za-z0-9]+)):(\d+)/g;
    const walker = document.createTreeWalker(out, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) if (/File "|[A-Za-z]:\\|\//.test(walker.currentNode.nodeValue)) nodes.push(walker.currentNode);
    for (const node of nodes) {
      const text = node.nodeValue;
      const frag = document.createDocumentFragment();
      let last = 0;
      for (const m of text.matchAll(re)) {
        if (m.index > 0 && text.slice(Math.max(0, m.index - 5), m.index).endsWith("://")) continue;
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
        const isAbs = path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path);
        return ctx.openFile(isAbs ? path : joinPath(s.cwd, path));
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
    // A bare `python` / `py` / `node` has no TTY here, so force the interactive REPL.
    let exec = text;
    if (/^(python3?|py)$/i.test(text)) exec = `${text} -i`;
    else if (/^node$/i.test(text)) exec = "node -i";
    const b = makeBlock(s, text, s.cwd);
    b.start = performance.now();
    try {
      b.pid = await invoke("term_run", { line: exec, cwd: s.cwd, shell: settings.shell });
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

  /** Clear with a short fade-and-rise of the old blocks (opacity/transform only, so it stays cheap). */
  function clear() {
    if (!active) return;
    const s = active;
    const keep = s.running?.b ?? null;
    const old = [...s.el.children].filter((el) => el !== keep);
    if (!old.length) return;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce || old.length > 40) { for (const el of old) el.remove(); return; }
    s.el.classList.add("clearing");
    old.forEach((el, i) => { el.classList.add("leaving"); el.style.animationDelay = `${Math.min(i, 12) * 12}ms`; });
    setTimeout(() => {
      for (const el of old) el.remove();
      s.el.classList.remove("clearing");
    }, 220 + Math.min(old.length, 12) * 12);
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

  // Common programs offered as the first word when nothing better matches.
  const COMMON = ["python", "pip", "node", "npm", "npx", "git", "cargo", "code", "cls", "dir", "where", "type", "mkdir", "copy", "move", "del", "ipconfig", "ping", "tree"];

  /**
   * The inline suggestion shown after the caret (accept with Tab or →):
   * history first, then a matching file/folder for the word being typed,
   * then built-ins and common programs for the first word.
   */
  async function computeGhost() {
    const v = input.value;
    ghost = "";
    if (!v || input.selectionStart !== v.length || active?.running) return renderInput();
    const h = [...history].reverse().find((x) => x.startsWith(v) && x !== v);
    if (h) { ghost = h.slice(v.length); return renderInput(); }
    const word = v.match(/[^\s"]*$/)[0];
    const firstWord = !/\s/.test(v);
    if (word && active?.cwd) {
      const cut = sepIdx(word);
      const dirPart = cut >= 0 ? word.slice(0, cut + 1) : "";
      const prefix = word.slice(cut + 1).toLowerCase();
      try {
        const dir = dirPart ? await invoke("term_resolve", { cwd: active.cwd, input: dirPart }) : active.cwd;
        const entries = await listDir(dir);
        const pick = entries.find((e) => (!e.hidden || prefix.startsWith(".")) && e.name.toLowerCase().startsWith(prefix) && e.name.length > prefix.length && (!firstWord || e.is_dir || dirPart));
        if (pick && input.value === v) ghost = pick.name.slice(prefix.length) + (pick.is_dir ? "\\" : "");
      } catch {}
    }
    if (!ghost && firstWord) {
      const w = v.toLowerCase();
      const c = [...BUILTINS, ...COMMON].find((x) => x.startsWith(w) && x !== w);
      if (c) ghost = c.slice(v.length);
    }
    if (input.value === v) renderInput();
  }

  function acceptGhost() {
    input.value += ghost;
    ghost = "";
    input.setSelectionRange(input.value.length, input.value.length);
    renderInput();
    computeGhost(); // keep going into the next folder level
  }

  function renderInput() {
    const running = !!active?.running;
    host.classList.toggle("stdin", running);
    $(".ti-prompt").title = running ? "Input goes to the running program" : active?.cwd ?? "";
    input.placeholder = running ? "Send input to the running program…" : active?.cwd ? `Type a command or a folder name…` : "";
    mirror.innerHTML = highlightLine(input.value, lastTyped) + (ghost ? `<span class="tk-ghost">${esc(ghost)}</span><kbd class="tk-tab">Tab</kbd>` : "");
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
    suggest.hidden = false;
    const f = $(".ti-field").getBoundingClientRect();
    const x = f.left + Math.max(0, sugg.start * charW - input.scrollLeft);
    placeFloating(suggest, { left: x, right: x, top: f.top, bottom: f.bottom }, { prefer: "above" });
    suggest.querySelector("li.on")?.scrollIntoView({ block: "nearest" });
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
    if (k === "Tab") {
      e.preventDefault();
      if (ghost) return acceptGhost();
      if (active?.cwd && !active.running) complete();
      return;
    }
    if ((k === "ArrowRight" || k === "End") && ghost && input.selectionStart === input.value.length) {
      e.preventDefault();
      return acceptGhost();
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
  const ready = invoke("term_env").then((e) => {
    env = e;
    if (!env.shells.includes(settings.shell)) {
      settings.shell = env.shells[0] ?? (env.platform === "linux" ? "bash" : "cmd");
    }
    if (env.platform === "linux") {
      $(".tp-drive").title = "Root directory (/)";
      $(".tp-drive").innerHTML = `<span>/</span>`;
    }
    applySettings();
  });
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
    shells: () => env.shells,
    focus: () => booted.then(focus),
    newSession: () => booted.then(() => { newSession(); focus(); }),
    clear: () => booted.then(clear),
    stop: () => active?.running && invoke("kill_process", { id: active.running.pid }),
    remeasure: () => { measure(); placeCaret(); },
    /** The workspace changed: new terminals start there. */
    setWorkspace: (path) => booted.then(() => { if (active && !active.cwd && path) { active.el.innerHTML = ""; navigate(path, { quiet: true }).then(() => welcome(active)); } }),
  };
}

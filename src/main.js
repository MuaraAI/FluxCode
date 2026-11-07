// FluxCode workbench: title bar + menus, explorer, search, tabs, editor,
// output panel, quick open, command palette and status bar.
// Plain DOM — no UI framework, nothing re-renders that doesn't have to.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getVersion } from "@tauri-apps/api/app";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { openSearchPanel, selectNextOccurrence } from "@codemirror/search";
import {
  undo, redo, selectAll, toggleComment, moveLineUp, moveLineDown, copyLineUp, copyLineDown, selectLine,
} from "@codemirror/commands";
import {
  createState, createView, onEditorUpdate, setWrap, syncWrap, isWrapOn, findRanges, setSearchHighlight,
} from "./editor.js";
import { languageFor, loadLanguage } from "./languages.js";
import { icons, abIcon, logo, fileIcon, folderIcon } from "./icons.js";

const $ = (sel) => document.querySelector(sel);
const appWindow = getCurrentWindow();
let VERSION = "dev"; // filled from tauri.conf.json at boot (e.g. 0.2.0-dev.2026-11-07-1000)

// ------------------------------------------------------------------ paths

const sepOf = (p) => (p.includes("\\") ? "\\" : "/");
const basename = (p) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const dirname = (p) => {
  const i = Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/"));
  return i < 0 ? "" : p.slice(0, i);
};
const join = (dir, name) => (/[\\/]$/.test(dir) ? dir + name : dir + sepOf(dir) + name);
const samePath = (a, b) => !!a && !!b && a.toLowerCase() === b.toLowerCase(); // Windows: case-insensitive
const isInside = (root, p) => !!root && !!p && p.toLowerCase().startsWith(root.toLowerCase());
const relative = (root, p) => (isInside(root, p) ? p.slice(root.length).replace(/^[\\/]/, "") : p);
const escapeHtml = (s) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

// ------------------------------------------------------------------ state

const state = {
  root: null,
  expanded: new Set(),
  dirCache: new Map(), // path -> entries
  selected: null, // tree selection (path)
  fileIndex: null, // cached file list for quick open
  tabs: [],
  active: null,
  untitledCounter: 0,
  python: undefined, // undefined = not probed yet, null = not found
  run: null, // { id, start, name }
  view: "explorer",
  zoom: 1,
};

let tabSeq = 0;
const view = createView($("#editor"));

// ------------------------------------------------------------- feedback

function toast(message, kind = "info", ms = 4500) {
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.innerHTML = `${icons[kind] ?? icons.info}<span></span>`;
  el.querySelector("span").textContent = message;
  $("#toasts").append(el);
  setTimeout(() => {
    el.classList.add("leaving");
    el.addEventListener("animationend", () => el.remove(), { once: true });
  }, ms);
}

/** Modal dialog. Resolves to the id of the clicked button, or "cancel". */
function modal(message, detail, buttons = [{ id: "ok", label: "OK", primary: true }]) {
  return new Promise((resolve) => {
    const backdrop = $("#modal-backdrop");
    backdrop.querySelector(".modal-msg").textContent = message;
    backdrop.querySelector(".modal-detail").textContent = detail;
    const box = backdrop.querySelector(".modal-buttons");
    box.innerHTML = "";
    const done = (id) => {
      backdrop.hidden = true;
      document.removeEventListener("keydown", onKey, true);
      if (state.active) view.focus();
      resolve(id);
    };
    const onKey = (e) => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); done("cancel"); }
    };
    for (const b of buttons) {
      const btn = document.createElement("button");
      btn.className = b.primary ? "btn-primary" : "btn-secondary";
      btn.textContent = b.label;
      btn.onclick = () => done(b.id);
      box.append(btn);
    }
    backdrop.onmousedown = (e) => { if (e.target === backdrop) done("cancel"); };
    document.addEventListener("keydown", onKey, true);
    backdrop.hidden = false;
    box.querySelector(".btn-primary")?.focus();
  });
}

// ------------------------------------------------------------- persistence

let booted = false; // don't overwrite the saved session before it has been restored

function persist() {
  if (!booted) return;
  try {
    const css = getComputedStyle(document.documentElement);
    localStorage.setItem(
      "fluxcode.session",
      JSON.stringify({
        root: state.root,
        files: state.tabs.filter((t) => t.path && !t.preview).map((t) => t.path),
        active: state.active?.path ?? null,
        sidebarW: css.getPropertyValue("--sidebar-w").trim(),
        panelH: css.getPropertyValue("--panel-h").trim(),
        wrap: isWrapOn(),
        zoom: state.zoom,
        panelTab: state.panelTab,
      }),
    );
  } catch {}
}

function loadSession() {
  try { return JSON.parse(localStorage.getItem("fluxcode.session") ?? "null"); } catch { return null; }
}

async function restoreSession(s) {
  if (!s) return;
  if (s.root) await setRoot(s.root, { quiet: true });
  for (const f of s.files ?? []) await openFile(f, { quiet: true, activate: false });
  const active = state.tabs.find((t) => samePath(t.path, s.active)) ?? state.tabs.at(-1);
  if (active) activateTab(active);
}

// ---------------------------------------------------------------- explorer

async function loadDir(path, force = false) {
  if (!force && state.dirCache.has(path)) return state.dirCache.get(path);
  const entries = await invoke("list_dir", { path }).catch(() => []);
  state.dirCache.set(path, entries);
  return entries;
}

async function setRoot(path, { quiet = false } = {}) {
  try {
    await invoke("list_dir", { path });
  } catch (e) {
    if (!quiet) toast(`Cannot open folder: ${e}`, "error");
    return;
  }
  state.root = path;
  // Opening a folder grants the terminal access to it for this session.
  await invoke("access_grant", { path, scope: "session" }).catch(() => {});
  terminal?.setWorkspace(path);
  state.expanded = new Set([path]);
  state.dirCache.clear();
  state.fileIndex = null;
  state.selected = null;
  state.python = undefined;
  $(".ws-header").hidden = false;
  $("#tree").hidden = false;
  $(".no-folder").hidden = true;
  $(".ws-header .ws-name").textContent = basename(path);
  $(".ws-header .twistie").classList.add("open");
  await renderTree();
  updateStatus();
  updateTitle();
  resolvePython();
  persist();
}

function closeFolderUi() {
  state.root = null;
  $(".ws-header").hidden = true;
  $("#tree").hidden = true;
  $("#tree").innerHTML = "";
  $(".no-folder").hidden = false;
  updateStatus();
  updateTitle();
}

function makeRow(entry, depth) {
  const row = document.createElement("div");
  row.className = "row";
  row.dataset.path = entry.path;
  row.dataset.depth = depth;
  if (entry.is_dir) row.dataset.dir = "1";
  row.setAttribute("role", "treeitem");
  const open = entry.is_dir && state.expanded.has(entry.path);
  const guides = depth ? `<span class="guides">${"<i></i>".repeat(depth)}</span>` : "";
  row.innerHTML = entry.is_dir
    ? `${guides}<span class="twistie${open ? " open" : ""}">${icons.chevron}</span>${folderIcon(open)}<span class="label"></span>`
    : `${guides}<span class="twistie none">${icons.chevron}</span>${fileIcon(entry.name)}<span class="label"></span>`;
  row.querySelector(".label").textContent = entry.name;
  if (samePath(entry.path, state.active?.path)) row.classList.add("active");
  if (samePath(entry.path, state.selected)) row.classList.add("selected");
  if (!entry.is_dir && state.tabs.some((t) => t.dirty && samePath(t.path, entry.path))) row.classList.add("dirty");
  return row;
}

/** Rows for a folder's contents (recursing into expanded subfolders). */
async function buildRows(dir, depth, frag, animate = false) {
  let i = 0;
  for (const entry of await loadDir(dir)) {
    const row = makeRow(entry, depth);
    if (animate && i < 24) {
      row.classList.add("enter");
      row.style.animationDelay = `${i * 10}ms`;
    }
    i++;
    frag.append(row);
    if (entry.is_dir && state.expanded.has(entry.path)) await buildRows(entry.path, depth + 1, frag, animate);
  }
}

async function renderTree() {
  const tree = $("#tree");
  if (!state.root) { tree.innerHTML = ""; return; }
  const frag = document.createDocumentFragment();
  if (state.expanded.has(state.root)) await buildRows(state.root, 0, frag);
  tree.replaceChildren(frag);
}

/** Expand/collapse in place, so the chevron animates and nothing else re-renders. */
async function toggleDir(row) {
  const path = row.dataset.path;
  const depth = +row.dataset.depth;
  const opening = !state.expanded.has(path);
  row.querySelector(".twistie").classList.toggle("open", opening);
  row.querySelector(".folder-icon").outerHTML = folderIcon(opening);
  if (opening) {
    state.expanded.add(path);
    const frag = document.createDocumentFragment();
    await buildRows(path, depth + 1, frag, true);
    row.after(frag);
  } else {
    state.expanded.delete(path);
    let next = row.nextElementSibling;
    while (next && (next.dataset.depth === undefined || +next.dataset.depth > depth)) {
      const n = next.nextElementSibling;
      next.remove();
      next = n;
    }
  }
}

function refreshTreeMarkers() {
  const activePath = state.active?.path;
  for (const row of $("#tree").children) {
    const p = row.dataset.path;
    if (!p) continue;
    row.classList.toggle("active", samePath(p, activePath));
    row.classList.toggle("selected", samePath(p, state.selected));
    if (!row.dataset.dir) row.classList.toggle("dirty", state.tabs.some((t) => t.dirty && samePath(t.path, p)));
  }
}

const rowFor = (path) => $(`#tree .row[data-path="${CSS.escape(path)}"]`);

$("#tree").addEventListener("click", async (e) => {
  const row = e.target.closest(".row");
  if (!row || !row.dataset.path) return;
  state.selected = row.dataset.path;
  if (row.dataset.dir) await toggleDir(row);
  else await openFile(row.dataset.path, { preview: true });
  refreshTreeMarkers();
});

$("#tree").addEventListener("dblclick", (e) => {
  const row = e.target.closest(".row");
  if (row && !row.dataset.dir) {
    const tab = state.tabs.find((t) => samePath(t.path, row.dataset.path));
    if (tab) { tab.preview = false; renderTabs(); view.focus(); }
  }
});

$("#tree").addEventListener("keydown", (e) => {
  if (e.target.tagName === "INPUT") return;
  const rows = [...$("#tree").querySelectorAll(".row[data-path]")];
  const i = rows.findIndex((r) => samePath(r.dataset.path, state.selected));
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const next = rows[Math.min(Math.max(i + (e.key === "ArrowDown" ? 1 : -1), 0), rows.length - 1)];
    if (next) { state.selected = next.dataset.path; refreshTreeMarkers(); next.scrollIntoView({ block: "nearest" }); }
  } else if ((e.key === "ArrowRight" || e.key === "ArrowLeft") && rows[i]?.dataset.dir) {
    const open = state.expanded.has(state.selected);
    if ((e.key === "ArrowRight") !== open) toggleDir(rows[i]);
  } else if (e.key === "Enter" && rows[i]) {
    e.preventDefault();
    rows[i].click();
  } else if (e.key === "F2" && state.selected) {
    e.preventDefault();
    startRename(state.selected);
  }
});

$(".ws-header").addEventListener("click", async (e) => {
  if (e.target.closest(".ws-actions")) return;
  const open = !state.expanded.has(state.root);
  open ? state.expanded.add(state.root) : state.expanded.delete(state.root);
  $(".ws-header .twistie").classList.toggle("open", open);
  await renderTree();
});

function targetFolder() {
  if (!state.selected) return state.root;
  return rowFor(state.selected)?.dataset.dir ? state.selected : dirname(state.selected) || state.root;
}

/** Inline name input in the tree (new file/folder, rename). */
async function inlineInput({ parent, initial = "", isDir = false, replacePath = null }) {
  if (!samePath(parent, state.root) && !state.expanded.has(parent)) {
    const parentRow = rowFor(parent);
    if (parentRow) await toggleDir(parentRow);
  }
  const tree = $("#tree");
  let anchor = null;
  let depth = 0;
  if (replacePath) {
    anchor = rowFor(replacePath);
    depth = anchor ? +anchor.dataset.depth : 0;
  } else if (!samePath(parent, state.root)) {
    anchor = rowFor(parent);
    depth = anchor ? +anchor.dataset.depth + 1 : 0;
  }
  const row = document.createElement("div");
  row.className = "row enter";
  const guides = depth ? `<span class="guides">${"<i></i>".repeat(depth)}</span>` : "";
  row.innerHTML = `${guides}<span class="twistie none">${icons.chevron}</span>${isDir ? folderIcon(false) : fileIcon(initial || "file")}<input spellcheck="false" />`;
  const input = row.querySelector("input");
  input.value = initial;
  const hidden = replacePath && anchor;
  if (hidden) { anchor.hidden = true; anchor.after(row); }
  else if (anchor) anchor.after(row);
  else tree.prepend(row);
  input.focus();
  const dot = initial.lastIndexOf(".");
  input.setSelectionRange(0, dot > 0 && !isDir ? dot : initial.length);

  return new Promise((resolve) => {
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      row.remove();
      if (hidden) anchor.hidden = false;
      resolve(value?.trim() || null);
    };
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Enter") finish(input.value);
      if (e.key === "Escape") finish(null);
    });
    input.addEventListener("blur", () => finish(input.value));
  });
}

async function newEntry(isDir) {
  if (!state.root) return isDir ? openFolderDialog() : newUntitled();
  showView("explorer");
  const parent = targetFolder();
  const name = await inlineInput({ parent, isDir });
  if (!name) return;
  const path = join(parent, name);
  try {
    await invoke(isDir ? "create_dir" : "create_file", { path });
    state.fileIndex = null;
    await loadDir(parent, true);
    state.selected = path;
    await renderTree();
    if (!isDir) await openFile(path);
  } catch (e) {
    toast(`${e}`, "error");
  }
}

async function startRename(path) {
  const parent = dirname(path);
  const isDir = !!rowFor(path)?.dataset.dir;
  const name = await inlineInput({ parent, initial: basename(path), isDir, replacePath: path });
  if (!name || name === basename(path)) return;
  const to = join(parent, name);
  try {
    await invoke("rename_path", { from: path, to });
    state.fileIndex = null;
    await loadDir(parent, true);
    for (const tab of state.tabs) {
      if (tab.path && isInside(path, tab.path)) {
        tab.path = to + tab.path.slice(path.length);
        tab.name = basename(tab.path);
      }
    }
    if (state.expanded.delete(path)) state.expanded.add(to);
    state.selected = to;
    await renderTree();
    renderTabs();
    renderBreadcrumbs();
    updateTitle();
  } catch (e) {
    toast(`${e}`, "error");
  }
}

async function refreshExplorer() {
  state.dirCache.clear();
  state.fileIndex = null;
  await renderTree();
}

// ------------------------------------------------------------- side views

function showView(name, { focus = true } = {}) {
  state.view = name;
  toggleSidebar(true);
  $("#explorer").hidden = name !== "explorer";
  $("#search-view").hidden = name !== "search";
  $("#sb-title-text").textContent = name === "search" ? "SEARCH" : "EXPLORER";
  for (const b of document.querySelectorAll(".ab-item[data-view]")) b.classList.toggle("active", b.dataset.view === name);
  if (name === "search" && focus) {
    const input = $("#search-input");
    const sel = state.active ? view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to) : "";
    if (sel && !sel.includes("\n") && sel.length < 200) { input.value = sel; runSearch(); }
    input.focus();
    input.select();
  }
}

// --------------------------------------------------------- find in files

const search = { matchCase: false, wholeWord: false, seq: 0, timer: null };
const MAX_RENDERED_HITS = 1500;

function highlightRanges(text, ranges) {
  let html = "";
  let last = 0;
  for (const [a, b] of ranges) {
    html += `${escapeHtml(text.slice(last, a))}<mark class="hit">${escapeHtml(text.slice(a, b))}</mark>`;
    last = b;
  }
  return html + escapeHtml(text.slice(last));
}

async function runSearch() {
  clearTimeout(search.timer);
  const q = $("#search-input").value;
  const seq = ++search.seq;
  const results = $("#search-results");
  const summary = $("#search-summary");
  setSearchHighlight(view, q ? { text: q, matchCase: search.matchCase, wholeWord: search.wholeWord } : null);
  if (!q) { results.replaceChildren(); summary.textContent = ""; return; }
  if (!state.root) { results.replaceChildren(); summary.textContent = "Open a folder to search in its files."; return; }
  summary.innerHTML = '<span class="panel-status"><span class="spinner"></span>Searching…</span>';
  let res;
  try {
    res = await invoke("search_files", { root: state.root, query: q, matchCase: search.matchCase, wholeWord: search.wholeWord });
  } catch (e) {
    summary.textContent = `${e}`;
    return;
  }
  if (seq !== search.seq) return; // a newer search is running
  const frag = document.createDocumentFragment();
  let rendered = 0;
  for (const file of res.files) {
    if (rendered >= MAX_RENDERED_HITS) break;
    const group = document.createElement("div");
    group.className = "sr-group";
    const name = basename(file.path);
    const dir = dirname(relative(state.root, file.path));
    group.innerHTML = `<div class="sr-file"><span class="twistie open">${icons.chevron}</span>${fileIcon(name)}<span class="name">${escapeHtml(name)}</span><span class="dir">${escapeHtml(dir)}</span><span class="count">${file.hits.length}</span></div>`;
    for (const hit of file.hits) {
      if (rendered++ >= MAX_RENDERED_HITS) break;
      const row = document.createElement("div");
      row.className = "sr-hit";
      row.dataset.path = file.path;
      row.dataset.line = hit.line;
      row.title = `${relative(state.root, file.path)}:${hit.line}`;
      row.innerHTML = `<span class="text">${highlightRanges(hit.text, findRanges(hit.text, q, search.matchCase, search.wholeWord))}</span><span class="ln">${hit.line}</span>`;
      group.append(row);
    }
    frag.append(group);
  }
  results.replaceChildren(frag);
  const files = res.files.length;
  summary.textContent = res.total
    ? `${res.total}${res.truncated ? "+" : ""} result${res.total === 1 ? "" : "s"} in ${files} file${files === 1 ? "" : "s"}${rendered < res.total ? ` (showing ${MAX_RENDERED_HITS})` : ""}`
    : "No results found.";
}

$("#search-input").addEventListener("input", () => {
  clearTimeout(search.timer);
  search.timer = setTimeout(runSearch, 200);
});
$("#search-input").addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); runSearch(); }
  if (e.key === "Escape") { e.target.value = ""; runSearch(); }
});

function setSearchOption(opt, value) {
  search[opt] = value;
  $(opt === "matchCase" ? "#opt-case" : "#opt-word").classList.toggle("on", value);
  runSearch();
}
$("#opt-case").addEventListener("click", () => setSearchOption("matchCase", !search.matchCase));
$("#opt-word").addEventListener("click", () => setSearchOption("wholeWord", !search.wholeWord));

$("#search-results").addEventListener("click", async (e) => {
  const fileRow = e.target.closest(".sr-file");
  if (fileRow) {
    const group = fileRow.parentElement;
    group.classList.toggle("collapsed");
    fileRow.querySelector(".twistie").classList.toggle("open", !group.classList.contains("collapsed"));
    return;
  }
  const hit = e.target.closest(".sr-hit");
  if (!hit) return;
  for (const h of document.querySelectorAll(".sr-hit.active")) h.classList.remove("active");
  hit.classList.add("active");
  const tab = await openFile(hit.dataset.path, { preview: true, focus: false });
  if (!tab) return;
  // Select the match on that line, so it is highlighted in the editor too.
  const doc = view.state.doc;
  const line = doc.line(Math.min(+hit.dataset.line, doc.lines));
  const q = $("#search-input").value;
  const [range] = findRanges(line.text, q, search.matchCase, search.wholeWord);
  const from = line.from + (range?.[0] ?? 0);
  const to = line.from + (range?.[1] ?? 0);
  view.dispatch({ selection: EditorSelection.range(from, to), effects: EditorView.scrollIntoView(from, { y: "center" }) });
});
$("#search-results").addEventListener("dblclick", (e) => {
  if (e.target.closest(".sr-hit") && state.active) { state.active.preview = false; renderTabs(); view.focus(); }
});

// -------------------------------------------------------------------- tabs

async function openFile(path, { preview = false, line = null, quiet = false, activate = true, focus = true } = {}) {
  let tab = state.tabs.find((t) => samePath(t.path, path));
  if (!tab) {
    let content;
    try {
      content = await invoke("read_file", { path });
    } catch (e) {
      if (!quiet) toast(`${e}`, "error");
      return null;
    }
    const lang = languageFor(path);
    const langExt = await loadLanguage(lang);
    const eol = content.text.includes("\r\n") ? "\r\n" : content.text.includes("\n") ? "\n" : "\r\n";
    const editorState = createState(content.text, langExt, lang.indent);
    tab = { id: ++tabSeq, path, name: basename(path), state: editorState, saved: editorState.doc, eol, lang, dirty: false, preview, scroll: 0 };
    // Reuse the current preview tab instead of piling up tabs (VS Code preview mode).
    const previewIdx = state.tabs.findIndex((t) => t.preview && !t.dirty);
    if (preview && previewIdx >= 0) state.tabs.splice(previewIdx, 1, tab);
    else state.tabs.splice(state.active ? state.tabs.indexOf(state.active) + 1 : state.tabs.length, 0, tab);
    if (content.lossy) toast(`${tab.name} is not valid UTF-8; some characters could not be decoded.`, "warning");
  } else if (!preview) {
    tab.preview = false;
  }
  if (activate) activateTab(tab, { focus });
  if (line != null && activate) revealLine(line);
  persist();
  return tab;
}

function newUntitled() {
  const lang = languageFor("");
  const editorState = createState("", [], lang.indent);
  const tab = {
    id: ++tabSeq, path: null, name: `Untitled-${++state.untitledCounter}`, state: editorState, saved: editorState.doc,
    eol: "\r\n", lang, dirty: false, preview: false, scroll: 0,
  };
  state.tabs.splice(state.active ? state.tabs.indexOf(state.active) + 1 : state.tabs.length, 0, tab);
  activateTab(tab);
}

function activateTab(tab, { focus = true } = {}) {
  if (state.active && state.active !== tab && state.tabs.includes(state.active)) {
    state.active.state = view.state;
    state.active.scroll = view.scrollDOM.scrollTop;
  }
  const changed = state.active !== tab;
  state.active = tab;
  $("#editor-host").classList.toggle("empty", !tab);
  if (tab && changed) {
    view.setState(tab.state);
    syncWrap(view);
    const top = tab.scroll;
    requestAnimationFrame(() => { view.scrollDOM.scrollTop = top; });
  }
  if (tab && focus) view.focus();
  renderTabs();
  renderBreadcrumbs();
  refreshTreeMarkers();
  updateStatus();
  updateTitle();
  persist();
}

function revealLine(line, col = 1) {
  const doc = view.state.doc;
  const ln = doc.line(Math.min(Math.max(1, line), doc.lines));
  const pos = Math.min(ln.from + Math.max(0, col - 1), ln.to);
  view.dispatch({ selection: EditorSelection.cursor(pos), effects: EditorView.scrollIntoView(pos, { y: "center" }) });
  view.focus();
}

async function closeTab(tab, { force = false } = {}) {
  if (tab.dirty && !force) {
    if (tab === state.active) tab.state = view.state;
    const choice = await modal(
      `Do you want to save the changes you made to ${tab.name}?`,
      "Your changes will be lost if you don't save them.",
      [
        { id: "save", label: "Save", primary: true },
        { id: "dont", label: "Don't Save" },
        { id: "cancel", label: "Cancel" },
      ],
    );
    if (choice === "cancel") return false;
    if (choice === "save" && !(await saveTab(tab))) return false;
  }
  const idx = state.tabs.indexOf(tab);
  state.tabs.splice(idx, 1);
  if (state.active === tab) {
    state.active = null;
    activateTab(state.tabs[Math.min(idx, state.tabs.length - 1)] ?? null);
  } else {
    renderTabs();
  }
  persist();
  return true;
}

/** Tabs are keyed by id: existing tab elements are reused, so only new tabs animate in. */
const tabEls = new Map();
function renderTabs() {
  const bar = $("#tabs");
  const live = new Set();
  const els = state.tabs.map((tab) => {
    live.add(tab.id);
    let el = tabEls.get(tab.id);
    if (!el) {
      el = document.createElement("div");
      el.innerHTML = `<span class="ti"></span><span class="label"></span><span class="tab-close" title="Close (Ctrl+W)">${icons.close}</span>`;
      el.addEventListener("mousedown", (e) => {
        if (e.button === 1) { e.preventDefault(); closeTab(tab); }
      });
      el.addEventListener("click", (e) => {
        if (e.target.closest(".tab-close")) closeTab(tab);
        else if (tab !== state.active) activateTab(tab);
      });
      el.addEventListener("dblclick", () => { tab.preview = false; renderTabs(); });
      tabEls.set(tab.id, el);
    }
    el.className = `tab${tab === state.active ? " active" : ""}${tab.dirty ? " dirty" : ""}${tab.preview ? " preview" : ""}`;
    el.title = tab.path ?? tab.name;
    if (el.dataset.name !== tab.name) {
      el.dataset.name = tab.name;
      el.querySelector(".ti").innerHTML = fileIcon(tab.name);
      el.querySelector(".label").textContent = tab.name;
    }
    return el;
  });
  for (const id of tabEls.keys()) if (!live.has(id)) tabEls.delete(id);
  // Only touch the DOM when order/membership changed.
  if (els.length !== bar.children.length || els.some((el, i) => bar.children[i] !== el)) bar.replaceChildren(...els);
  bar.querySelector(".tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  $("#run-btn").hidden = state.active?.lang.id !== "python";
}

function renderBreadcrumbs() {
  const el = $("#breadcrumbs");
  const tab = state.active;
  if (!tab) { el.innerHTML = ""; return; }
  const parts = tab.path ? relative(state.root, tab.path).split(/[\\/]/) : [tab.name];
  el.innerHTML = parts
    .map((p, i) =>
      i === parts.length - 1
        ? `<span class="crumb">${fileIcon(p)}${escapeHtml(p)}</span>`
        : `<span class="crumb">${escapeHtml(p)}</span><span class="sep">${icons.chevron}</span>`,
    )
    .join("");
}

function updateTitle() {
  const tab = state.active;
  const parts = [];
  if (tab) parts.push(`${tab.dirty ? "● " : ""}${tab.name}`);
  if (state.root) parts.push(basename(state.root));
  parts.push("FluxCode");
  appWindow.setTitle(parts.join(" - ")).catch(() => {});
  $(".cc-label").textContent = state.root ? basename(state.root) : "FluxCode";
}

// --------------------------------------------------------------- editing

onEditorUpdate((u) => {
  const tab = state.active;
  if (!tab) return;
  if (u.docChanged) {
    const dirty = tab.eolChanged || !u.state.doc.eq(tab.saved);
    const pinned = tab.preview;
    tab.preview = false;
    if (dirty !== tab.dirty || pinned) {
      tab.dirty = dirty;
      renderTabs();
      refreshTreeMarkers();
      updateTitle();
    }
  }
  if (u.docChanged || u.selectionSet) scheduleCursorStatus();
});

async function saveTab(tab, { saveAs = false } = {}) {
  const editorState = tab === state.active ? view.state : tab.state;
  let path = tab.path;
  if (!path || saveAs) {
    path = await saveDialog({
      defaultPath: tab.path ?? join(state.root ?? "", tab.name.startsWith("Untitled") ? `${tab.name}.py` : tab.name),
      filters: [
        { name: "Python", extensions: ["py"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (!path) return false;
  }
  const text = editorState.doc.sliceString(0, editorState.doc.length, tab.eol);
  try {
    await invoke("write_file", { path, contents: text });
  } catch (e) {
    toast(`Failed to save '${tab.name}': ${e}`, "error");
    return false;
  }
  const renamed = path !== tab.path;
  tab.saved = editorState.doc;
  tab.dirty = false;
  tab.eolChanged = false;
  tab.preview = false;
  if (renamed) {
    tab.path = path;
    tab.name = basename(path);
    const lang = languageFor(path);
    if (lang.id !== tab.lang.id) {
      // The file got a real extension: rebuild the state with the right language.
      const langExt = await loadLanguage(lang);
      const fresh = createState(editorState.doc.toString(), langExt, lang.indent);
      tab.lang = lang;
      tab.state = fresh;
      tab.saved = fresh.doc;
      if (tab === state.active) view.setState(fresh);
    }
    state.fileIndex = null;
    if (isInside(state.root, path)) {
      await loadDir(dirname(path), true);
      await renderTree();
    }
  }
  renderTabs();
  renderBreadcrumbs();
  refreshTreeMarkers();
  updateTitle();
  updateStatus();
  persist();
  return true;
}

// ------------------------------------------------------------- status bar

let cursorFrame = 0;
function scheduleCursorStatus() {
  if (!cursorFrame) cursorFrame = requestAnimationFrame(() => { cursorFrame = 0; updateCursorStatus(); });
}

function updateCursorStatus() {
  if (!state.active) { $("#sb-cursor").textContent = ""; return; }
  const sel = view.state.selection.main;
  const line = view.state.doc.lineAt(sel.head);
  let text = `Ln ${line.number}, Col ${sel.head - line.from + 1}`;
  const selected = view.state.selection.ranges.reduce((n, r) => n + (r.to - r.from), 0);
  if (selected) text += ` (${selected} selected)`;
  $("#sb-cursor").textContent = text;
}

function updateStatus() {
  const tab = state.active;
  $("#sb-folder").textContent = state.root ? basename(state.root) : "";
  $("#sb-indent").textContent = tab ? `Spaces: ${tab.lang.indent}` : "";
  $("#sb-encoding").textContent = tab ? "UTF-8" : "";
  $("#sb-eol").textContent = tab ? (tab.eol === "\r\n" ? "CRLF" : "LF") : "";
  $("#sb-lang").textContent = tab ? tab.lang.name : "";
  const py = state.python;
  const showPy = tab?.lang.id === "python";
  $("#sb-python").textContent = !showPy
    ? ""
    : py === undefined
      ? "Python…"
      : py
        ? py.version.replace("Python ", "") + (/[\\/](\.?venv|env)[\\/]/i.test(py.path) ? " (venv)" : "")
        : "Select Interpreter";
  $("#sb-python").title = py ? `${py.path}\nClick to re-detect` : "Python was not found on PATH. Click to re-detect.";
  updateCursorStatus();
}

$("#sb-eol").addEventListener("click", () => {
  const tab = state.active;
  if (!tab) return;
  tab.eol = tab.eol === "\r\n" ? "\n" : "\r\n";
  tab.eolChanged = !tab.eolChanged; // same text, different line endings on disk
  tab.dirty = tab.eolChanged || !view.state.doc.eq(tab.saved);
  renderTabs();
  refreshTreeMarkers();
  updateStatus();
  updateTitle();
});
$("#sb-python").addEventListener("click", () => resolvePython(true));

async function resolvePython(announce = false) {
  state.python = undefined;
  updateStatus();
  state.python = (await invoke("resolve_python", { workspace: state.root })) ?? null;
  updateStatus();
  if (announce) {
    state.python
      ? toast(`Using ${state.python.version} — ${state.python.path}`)
      : toast("Python was not found. Install it from python.org and make sure it is on PATH.", "warning");
  }
}

// ------------------------------------------------------------- run/output

const output = $("#output");
const OUTPUT_LIMIT = 400_000; // characters kept in the panel (older output is trimmed)
let pending = [];
let flushScheduled = false;
let outputLength = 0;

function appendOutput(text, cls = "") {
  pending.push([text, cls]);
  if (!flushScheduled) {
    flushScheduled = true;
    requestAnimationFrame(flushOutput);
  }
}

function flushOutput() {
  flushScheduled = false;
  if (!pending.length) return;
  const atBottom = output.scrollHeight - output.scrollTop - output.clientHeight < 30;
  const frag = document.createDocumentFragment();
  for (const [text, cls] of pending) {
    // Merge into the previous span when the style matches: fewer DOM nodes.
    const last = frag.lastChild ?? output.lastChild;
    if (last && last.nodeType === 1 && (last.className || "") === cls && !last.dataset.linked) last.append(text);
    else {
      const span = document.createElement("span");
      if (cls) span.className = cls;
      span.textContent = text;
      frag.append(span);
    }
    outputLength += text.length;
  }
  pending = [];
  output.append(frag);
  while (outputLength > OUTPUT_LIMIT && output.firstChild) {
    outputLength -= output.firstChild.textContent.length;
    output.firstChild.remove();
  }
  if (atBottom) output.scrollTop = output.scrollHeight;
}

function clearOutput() {
  pending = [];
  outputLength = 0;
  output.replaceChildren();
}

/** Turn Python traceback locations into clickable links. */
function linkifyTracebacks() {
  const re = /File "([^"]+)", line (\d+)/g;
  for (const span of [...output.querySelectorAll("span.stderr")]) {
    const text = span.textContent;
    if (!text.includes('File "')) continue;
    const frag = document.createDocumentFragment();
    let last = 0;
    for (const m of text.matchAll(re)) {
      frag.append(text.slice(last, m.index));
      const a = document.createElement("a");
      a.textContent = m[0];
      a.dataset.file = m[1];
      a.dataset.line = m[2];
      frag.append(a);
      last = m.index + m[0].length;
    }
    frag.append(text.slice(last));
    span.replaceChildren(frag);
    span.dataset.linked = "1";
  }
}

output.addEventListener("click", (e) => {
  const a = e.target.closest("a[data-file]");
  if (a) openFile(a.dataset.file, { line: +a.dataset.line });
});

function showPanel(show = true) {
  $("#panel").hidden = !show;
  $("#toggle-panel").innerHTML = show ? icons.panelOn : icons.panel;
  $("#toggle-panel").classList.toggle("on", show);
  if (show) showPanelTab(state.panelTab, { focus: false });
  else if (state.active) view.focus();
}

// ------------------------------------------------------------- terminal

let terminal = null; // Flux Terminal, loaded on first use
let terminalLoading = null;
state.panelTab = "output";

function ensureTerminal() {
  // One shared promise: concurrent callers must not create two terminals.
  terminalLoading ??= import("./terminal.js").then(({ createTerminal }) => {
    terminal = createTerminal($("#terminal"), {
      icons, fileIcon, folderIcon, modal, toast,
      workspace: () => state.root,
      openFile: (path, line) => openFile(path, { line: Number.isFinite(line) ? line : null }),
    });
    return terminal;
  });
  return terminalLoading;
}

async function showPanelTab(name, { focus = true } = {}) {
  state.panelTab = name;
  if ($("#panel").hidden) {
    $("#panel").hidden = false;
    $("#toggle-panel").innerHTML = icons.panelOn;
    $("#toggle-panel").classList.add("on");
  }
  $("#panel").dataset.tab = name;
  for (const t of document.querySelectorAll(".panel-tab")) t.classList.toggle("active", t.dataset.ptab === name);
  $("#output-pane").hidden = name !== "output";
  $("#terminal").hidden = name !== "terminal";
  if (name === "terminal") {
    const t = await ensureTerminal();
    t.remeasure();
    if (focus) t.focus();
  }
  persist();
}

$(".panel-tabs").addEventListener("click", (e) => {
  const tab = e.target.closest("[data-ptab]");
  if (tab) showPanelTab(tab.dataset.ptab);
});

function toggleTerminal() {
  const visible = !$("#panel").hidden && state.panelTab === "terminal";
  if (visible && $("#terminal").contains(document.activeElement)) return showPanel(false);
  showPanelTab("terminal");
}

function setRunning(running) {
  $("#run-btn").innerHTML = running ? icons.stop : icons.play;
  $("#run-btn").title = running ? "Stop (Shift+F5)" : "Run Python File (F5)";
  $("#run-btn").classList.toggle("running", running);
  $("#statusbar").classList.toggle("running", running);
  $("#stdin-row").hidden = !running;
  $(".panel-actions [data-action=stop]").hidden = !running;
  $(".panel-status").innerHTML = running ? `<span class="spinner"></span>Running ${escapeHtml(state.run?.name ?? "")}` : "";
}

async function runActive() {
  const tab = state.active;
  if (!tab) return toast("Open a Python file to run it.", "warning");
  if (state.run) return stopRun();
  if (tab.lang.id !== "python") return toast(`Running ${tab.lang.name} files is not supported yet. Python works now; Rust and web are next.`, "warning");
  if ((tab.dirty || !tab.path) && !(await saveTab(tab))) return;
  if (state.python === undefined) await resolvePython();
  showPanelTab("output", { focus: false });
  clearOutput();
  if (!state.python) {
    appendOutput("Python was not found.\nInstall it from https://www.python.org/downloads/ (tick \"Add python.exe to PATH\"), or create a .venv in this folder, then click the Python item in the status bar.\n", "stderr");
    return;
  }
  const cwd = isInside(state.root, tab.path) ? state.root : dirname(tab.path);
  const shown = relative(cwd, tab.path);
  appendOutput(`> ${basename(state.python.path)} ${shown.includes(" ") ? `"${shown}"` : shown}\n`, "cmd");
  try {
    const id = await invoke("spawn_process", { program: state.python.path, args: ["-u", tab.path], cwd });
    state.run = { id, start: performance.now(), name: tab.name };
    setRunning(true);
    $("#stdin").focus(); // like VS Code focusing the terminal, so input() just works
  } catch (e) {
    appendOutput(`${e}\n`, "stderr");
  }
}

async function stopRun() {
  if (state.run) await invoke("kill_process", { id: state.run.id }).catch(() => {});
}

listen("proc-output", ({ payload }) => {
  if (payload.id === state.run?.id) appendOutput(payload.data, payload.stream === "stderr" ? "stderr" : "");
});

listen("proc-exit", ({ payload }) => {
  if (payload.id !== state.run?.id) return;
  const secs = ((performance.now() - state.run.start) / 1000).toFixed(3);
  state.run = null;
  flushOutput();
  linkifyTracebacks();
  appendOutput(`\n[Done] exited with code=${payload.code ?? "terminated"} in ${secs} seconds\n`, "info");
  setRunning(false);
});

$("#stdin").addEventListener("keydown", async (e) => {
  if (e.key !== "Enter" || !state.run) return;
  const value = e.target.value;
  e.target.value = "";
  appendOutput(`${value}\n`, "echo");
  await invoke("write_stdin", { id: state.run.id, data: `${value}\n` }).catch((err) => toast(`${err}`, "error"));
});

// --------------------------------------------------------------- commands

// id -> [palette label, keybinding shown]
const COMMANDS = {
  quickOpen: ["Go to File...", "Ctrl+P"],
  commandPalette: ["Show All Commands", "Ctrl+Shift+P"],
  openFolder: ["File: Open Folder...", "Ctrl+K Ctrl+O"],
  openFile: ["File: Open File...", "Ctrl+O"],
  newFile: ["File: New File", "Ctrl+N"],
  newFolder: ["File: New Folder..."],
  save: ["File: Save", "Ctrl+S"],
  saveAs: ["File: Save As...", "Ctrl+Shift+S"],
  saveAll: ["File: Save All", "Ctrl+K S"],
  closeEditor: ["View: Close Editor", "Ctrl+W"],
  closeAll: ["View: Close All Editors", "Ctrl+K Ctrl+W"],
  closeFolder: ["Workspaces: Close Folder"],
  refresh: ["File: Refresh Explorer"],
  showExplorer: ["View: Show Explorer", "Ctrl+Shift+E"],
  findInFiles: ["Search: Find in Files", "Ctrl+Shift+F"],
  toggleSidebar: ["View: Toggle Primary Side Bar", "Ctrl+B"],
  togglePanel: ["View: Toggle Panel", "Ctrl+J"],
  toggleWordWrap: ["View: Toggle Word Wrap", "Alt+Z"],
  zoomIn: ["View: Zoom In", "Ctrl+="],
  zoomOut: ["View: Zoom Out", "Ctrl+-"],
  zoomReset: ["View: Reset Zoom", "Ctrl+0"],
  run: ["Python: Run Python File", "F5"],
  stop: ["Python: Stop Running Program", "Shift+F5"],
  selectInterpreter: ["Python: Re-detect Interpreter"],
  toggleTerminal: ["Terminal: Toggle Terminal", "Ctrl+`"],
  newTerminal: ["Terminal: Create New Terminal", "Ctrl+Shift+`"],
  clearTerminal: ["Terminal: Clear", "Ctrl+L"],
  stopTerminal: ["Terminal: Stop Running Command", "Ctrl+C"],
  showOutput: ["View: Show Output"],
  gotoLine: ["Go to Line/Column...", "Ctrl+G"],
  find: ["Find", "Ctrl+F"],
  replace: ["Replace", "Ctrl+H"],
  undo: ["Undo", "Ctrl+Z"],
  redo: ["Redo", "Ctrl+Y"],
  cut: ["Cut", "Ctrl+X"],
  copy: ["Copy", "Ctrl+C"],
  paste: ["Paste", "Ctrl+V"],
  selectAll: ["Select All", "Ctrl+A"],
  selectLine: ["Select Line", "Ctrl+L"],
  toggleComment: ["Toggle Line Comment", "Ctrl+/"],
  moveLineUp: ["Move Line Up", "Alt+Up"],
  moveLineDown: ["Move Line Down", "Alt+Down"],
  copyLineUp: ["Copy Line Up", "Shift+Alt+Up"],
  copyLineDown: ["Copy Line Down", "Shift+Alt+Down"],
  addNextOccurrence: ["Add Selection to Next Find Match", "Ctrl+D"],
  shortcuts: ["Help: Keyboard Shortcuts"],
  about: ["Help: About FluxCode"],
  reload: ["Developer: Reload Window"],
  exit: ["File: Exit"],
};

const MENUS = {
  file: ["newFile", "newFolder", "openFile", "openFolder", "-", "save", "saveAs", "saveAll", "-", "closeEditor", "closeAll", "closeFolder", "-", "exit"],
  edit: ["undo", "redo", "-", "cut", "copy", "paste", "-", "find", "replace", "findInFiles", "-", "toggleComment"],
  selection: ["selectAll", "selectLine", "-", "copyLineUp", "copyLineDown", "moveLineUp", "moveLineDown", "-", "addNextOccurrence"],
  view: ["commandPalette", "quickOpen", "-", "showExplorer", "findInFiles", "-", "toggleSidebar", "togglePanel", "toggleWordWrap", "-", "zoomIn", "zoomOut", "zoomReset"],
  run: ["run", "stop", "-", "selectInterpreter"],
  terminal: ["newTerminal", "toggleTerminal", "-", "clearTerminal", "stopTerminal", "-", "showOutput"],
  help: ["commandPalette", "shortcuts", "-", "about"],
};
const menuLabel = (id) => COMMANDS[id][0].replace(/^(File|View|Python|Search|Help|Workspaces|Developer|Terminal): /, "");

/** Run an editor command on the active editor (from a menu or the palette). */
const editorCmd = (fn) => () => {
  if (!state.active) return;
  view.focus();
  fn(view);
};

async function clipboard(kind) {
  if (!state.active) return;
  view.focus();
  const sel = view.state.selection.main;
  try {
    if (kind === "paste") {
      const text = await navigator.clipboard.readText();
      view.dispatch(view.state.replaceSelection(text));
      return;
    }
    if (sel.empty) return;
    await navigator.clipboard.writeText(view.state.sliceDoc(sel.from, sel.to));
    if (kind === "cut") view.dispatch(view.state.replaceSelection(""));
  } catch {
    toast(`Use Ctrl+${kind === "paste" ? "V" : kind === "cut" ? "X" : "C"} to ${kind}.`, "warning");
  }
}

function setZoom(z) {
  state.zoom = Math.min(Math.max(Math.round(z * 10) / 10, 0.5), 2.5);
  getCurrentWebview().setZoom(state.zoom).catch(() => {});
  persist();
}

async function openFolderDialog() {
  const dir = await openDialog({ directory: true, multiple: false });
  if (dir) await setRoot(dir);
}

async function openFileDialog() {
  const file = await openDialog({ multiple: false, defaultPath: state.root ?? undefined });
  if (file) await openFile(file);
}

function toggleSidebar(force) {
  const wb = $("#workbench");
  const show = force === undefined ? wb.classList.contains("no-sidebar") : force;
  wb.classList.toggle("no-sidebar", !show);
  for (const b of document.querySelectorAll(".ab-item[data-view]")) b.classList.toggle("active", show && b.dataset.view === state.view);
  $("#toggle-sidebar").innerHTML = show ? icons.sidebarOn : icons.sidebar;
  $("#toggle-sidebar").classList.toggle("on", show);
}

function showShortcuts() {
  const lines = Object.values(COMMANDS)
    .filter(([, kb]) => kb)
    .map(([label, kb]) => `${kb.padEnd(16)}  ${label}`)
    .join("\n");
  modal("Keyboard Shortcuts", lines);
}

async function runCommand(id) {
  const tab = state.active;
  switch (id) {
    case "openFolder": return openFolderDialog();
    case "openFile": return openFileDialog();
    case "newFile": return state.root && document.activeElement?.closest("#sidebar") ? newEntry(false) : newUntitled();
    case "newFolder": return newEntry(true);
    case "save": return tab && saveTab(tab);
    case "saveAs": return tab && saveTab(tab, { saveAs: true });
    case "saveAll":
      for (const t of state.tabs) if (t.dirty) await saveTab(t);
      return;
    case "closeEditor": return tab && closeTab(tab);
    case "closeAll":
      for (const t of [...state.tabs]) if (!(await closeTab(t))) return;
      return;
    case "closeFolder":
      for (const t of [...state.tabs]) if (!(await closeTab(t))) return;
      closeFolderUi();
      return persist();
    case "toggleSidebar": return toggleSidebar();
    case "togglePanel": return showPanel($("#panel").hidden);
    case "clearOutput": return clearOutput();
    case "toggleTerminal": return toggleTerminal();
    case "newTerminal":
      await showPanelTab("terminal", { focus: false });
      return terminal.newSession();
    case "clearTerminal": return terminal?.clear();
    case "stopTerminal": return terminal?.stop();
    case "showOutput": return showPanelTab("output");
    case "run": return runActive();
    case "stop": return stopRun();
    case "selectInterpreter": return resolvePython(true);
    case "refresh": return refreshExplorer();
    case "collapseAll":
      state.expanded = new Set([state.root]);
      return renderTree();
    case "showExplorer": return showView("explorer");
    case "findInFiles": return showView("search");
    case "quickOpen": return openQuickInput("");
    case "commandPalette": return openQuickInput(">");
    case "gotoLine": return openQuickInput(":");
    case "find":
    case "replace":
      if (!tab) return;
      view.focus();
      return openSearchPanel(view);
    case "toggleWordWrap":
      setWrap(view, !isWrapOn());
      return persist();
    case "zoomIn": return setZoom(state.zoom + 0.1);
    case "zoomOut": return setZoom(state.zoom - 0.1);
    case "zoomReset": return setZoom(1);
    case "undo": return editorCmd(undo)();
    case "redo": return editorCmd(redo)();
    case "cut":
    case "copy":
    case "paste": return clipboard(id);
    case "selectAll": return editorCmd(selectAll)();
    case "selectLine": return editorCmd(selectLine)();
    case "toggleComment": return editorCmd(toggleComment)();
    case "moveLineUp": return editorCmd(moveLineUp)();
    case "moveLineDown": return editorCmd(moveLineDown)();
    case "copyLineUp": return editorCmd(copyLineUp)();
    case "copyLineDown": return editorCmd(copyLineDown)();
    case "addNextOccurrence": return editorCmd(selectNextOccurrence)();
    case "nextTab":
    case "prevTab": {
      if (state.tabs.length < 2) return;
      const i = state.tabs.indexOf(tab);
      return activateTab(state.tabs[(i + (id === "nextTab" ? 1 : -1) + state.tabs.length) % state.tabs.length]);
    }
    case "shortcuts": return showShortcuts();
    case "about":
      return modal(
        `FluxCode ${VERSION}`,
        `${VERSION.includes("-dev") ? "Development build — not a final release.\n\n" : ""}A lightweight code editor with the VS Code layout and its own Flux Terminal.\nTauri 2 (Rust) + CodeMirror 6, running on the system WebView2.`,
      );
    case "reload": return location.reload();
    case "exit": return appWindow.close();
  }
}

document.addEventListener("click", (e) => {
  const el = e.target.closest("[data-action]");
  if (el && !el.closest("#menu-popup")) runCommand(el.dataset.action);
  const viewBtn = e.target.closest(".ab-item[data-view]");
  if (viewBtn) {
    const name = viewBtn.dataset.view;
    const visible = !$("#workbench").classList.contains("no-sidebar");
    if (visible && state.view === name) toggleSidebar(false);
    else showView(name);
  }
});

// ------------------------------------------------------------- menu bar

const menu = { open: null, index: -1 };
const kbHtml = (kb) =>
  kb ? kb.split(" ").map((chord) => chord.split("+").map((k) => `<kbd>${escapeHtml(k)}</kbd>`).join("")).join(" ") : "";

function openMenu(name) {
  $("#quick-input").hidden = true;
  const popup = $("#menu-popup");
  const btn = $(`#menubar [data-menu=${name}]`);
  for (const b of document.querySelectorAll("#menubar button")) b.classList.toggle("open", b === btn);
  popup.innerHTML = MENUS[name]
    .map((id) =>
      id === "-"
        ? '<div class="menu-sep"></div>'
        : `<button class="menu-item" data-cmd="${id}"><span>${escapeHtml(menuLabel(id))}${id === "toggleWordWrap" && isWrapOn() ? "  ✓" : ""}</span><span class="kb">${escapeHtml(COMMANDS[id][1] ?? "")}</span></button>`,
    )
    .join("");
  const r = btn.getBoundingClientRect();
  popup.style.left = `${Math.min(r.left, window.innerWidth - 280)}px`;
  popup.style.top = `${r.bottom + 4}px`;
  popup.hidden = false;
  // Restart the open animation when switching menus.
  popup.style.animation = "none";
  void popup.offsetWidth;
  popup.style.animation = "";
  menu.open = name;
  menu.index = -1;
}

function closeMenu() {
  if (!menu.open) return;
  menu.open = null;
  $("#menu-popup").hidden = true;
  for (const b of document.querySelectorAll("#menubar button")) b.classList.remove("open");
}

$("#menubar").addEventListener("mousedown", (e) => {
  const btn = e.target.closest("[data-menu]");
  if (!btn) return;
  e.preventDefault(); // keep the editor focused
  menu.open === btn.dataset.menu ? closeMenu() : openMenu(btn.dataset.menu);
});
$("#menubar").addEventListener("mouseover", (e) => {
  const btn = e.target.closest("[data-menu]");
  if (btn && menu.open && menu.open !== btn.dataset.menu) openMenu(btn.dataset.menu);
});
$("#menu-popup").addEventListener("mousedown", (e) => e.preventDefault());
$("#menu-popup").addEventListener("click", (e) => {
  const item = e.target.closest("[data-cmd]");
  if (!item) return;
  closeMenu();
  runCommand(item.dataset.cmd);
});
document.addEventListener("mousedown", (e) => {
  if (menu.open && !e.target.closest("#menu-popup, #menubar")) closeMenu();
});
window.addEventListener("blur", closeMenu);

function menuKey(e) {
  const items = [...$("#menu-popup").querySelectorAll(".menu-item")];
  if (e.key === "Escape") closeMenu();
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    menu.index = (menu.index + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
    items.forEach((it, i) => it.classList.toggle("focus", i === menu.index));
  } else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
    const names = Object.keys(MENUS);
    openMenu(names[(names.indexOf(menu.open) + (e.key === "ArrowRight" ? 1 : -1) + names.length) % names.length]);
  } else if (e.key === "Enter" && items[menu.index]) {
    const id = items[menu.index].dataset.cmd;
    closeMenu();
    runCommand(id);
  } else return false;
  e.preventDefault();
  e.stopPropagation();
  return true;
}

// ------------------------------------------------------- window controls

$("#wc-min").addEventListener("click", () => appWindow.minimize());
$("#wc-max").addEventListener("click", () => appWindow.toggleMaximize());
$("#wc-close").addEventListener("click", () => appWindow.close());
$("#command-center").addEventListener("click", () => openQuickInput(""));

async function syncMaximized() {
  const max = await appWindow.isMaximized().catch(() => false);
  $("#wc-max").innerHTML = max ? icons.restore : icons.maximize;
  $("#wc-max").title = max ? "Restore" : "Maximize";
}
let resizeFrame = 0;
appWindow.onResized(() => {
  if (!resizeFrame) resizeFrame = requestAnimationFrame(() => { resizeFrame = 0; syncMaximized(); });
});

// ------------------------------------------------- quick open / palette

const quick = { filtered: [], index: 0 };

function fuzzyScore(query, text) {
  if (!query) return { score: 0, hits: [] };
  const q = query.toLowerCase();
  const s = text.toLowerCase();
  const hits = [];
  let qi = 0;
  let score = 0;
  let prev = -2;
  for (let i = 0; i < s.length && qi < q.length; i++) {
    if (s[i] === q[qi]) {
      hits.push(i);
      score += prev === i - 1 ? 5 : 1; // consecutive matches
      if (i === 0 || "\\/._-: ".includes(s[i - 1])) score += 3; // word starts
      prev = i;
      qi++;
    }
  }
  return qi === q.length ? { score: score - s.length * 0.01, hits } : null;
}

function highlightHits(text, hits) {
  if (!hits.length) return escapeHtml(text);
  const set = new Set(hits);
  let out = "";
  let inMark = false;
  for (let i = 0; i < text.length; i++) {
    const hit = set.has(i);
    if (hit && !inMark) { out += '<mark class="hit">'; inMark = true; }
    if (!hit && inMark) { out += "</mark>"; inMark = false; }
    out += escapeHtml(text[i]);
  }
  return inMark ? `${out}</mark>` : out;
}

async function openQuickInput(prefix = "") {
  closeMenu();
  const box = $("#quick-input");
  const input = $("#qi-input");
  if (!box.hidden && input.value === prefix) return closeQuickInput();
  box.hidden = false;
  box.style.animation = "none";
  void box.offsetWidth;
  box.style.animation = "";
  input.value = prefix;
  input.placeholder = prefix === ">" ? "Type a command" : "Search files by name (type > for commands, : for a line)";
  input.focus();
  filterQuick();
  if (!prefix.startsWith(">") && state.root && !state.fileIndex) {
    state.fileIndex = await invoke("list_files", { root: state.root }).catch(() => []);
    if (!box.hidden) filterQuick();
  }
}

function closeQuickInput() {
  $("#quick-input").hidden = true;
  if (state.active) view.focus();
}

function filterQuick() {
  const raw = $("#qi-input").value;
  let results = [];
  if (raw.startsWith(">")) {
    const q = raw.slice(1).trim();
    results = Object.entries(COMMANDS)
      .filter(([id]) => id !== "commandPalette")
      .map(([id, [label, kb]]) => ({ id, label, kb, m: fuzzyScore(q, label) }))
      .filter((r) => r.m)
      .sort((a, b) => (q ? b.m.score - a.m.score : 0))
      .map((r) => ({
        html: `<span>${highlightHits(r.label, r.m.hits)}</span><span class="kb">${kbHtml(r.kb)}</span>`,
        run: () => runCommand(r.id),
      }));
  } else if (raw.startsWith(":")) {
    const [l, c] = raw.slice(1).split(/[,:]/).map((n) => parseInt(n, 10));
    const total = state.active ? view.state.doc.lines : 0;
    results = state.active
      ? [{
          html: Number.isFinite(l)
            ? `Go to line <b>${l}</b>${Number.isFinite(c) ? `, column <b>${c}</b>` : ""}`
            : `Type a line number between 1 and ${total}`,
          run: () => Number.isFinite(l) && revealLine(l, Number.isFinite(c) ? c : 1),
        }]
      : [{ html: "Open a file first to go to a line.", run: () => {} }];
  } else {
    const q = raw.trim().replace(/\s+/g, "");
    const files = state.fileIndex ?? [];
    const recent = state.tabs.filter((t) => t.path).map((t) => t.path).reverse();
    const pool = q ? files : [...new Set([...recent, ...files])];
    const scored = [];
    for (const path of pool) {
      const rel = relative(state.root, path);
      const name = basename(path);
      const m = fuzzyScore(q, name); // prefer file-name matches, fall back to the path
      const mp = m ? null : fuzzyScore(q, rel);
      if (q && !m && !mp) continue;
      scored.push({ path, rel, name, score: m ? m.score + 10 : mp ? mp.score : 0, hits: m?.hits ?? [] });
      if (!q && scored.length >= 40) break;
    }
    if (q) scored.sort((a, b) => b.score - a.score);
    results = scored.slice(0, 40).map((r) => ({
      html: `${fileIcon(r.name)}<span>${highlightHits(r.name, r.hits)}</span><span class="detail">${escapeHtml(dirname(r.rel))}</span>`,
      run: () => openFile(r.path),
    }));
    if (!state.root && !q && !recent.length) results = [{ html: "Open a folder to search its files — type <b>&gt;</b> for commands", run: () => runCommand("openFolder") }];
  }
  quick.filtered = results;
  quick.index = 0;
  $("#qi-list").innerHTML = results.length
    ? results.map((r, i) => `<li data-i="${i}" class="${i === 0 ? "selected" : ""}">${r.html}</li>`).join("")
    : '<li class="empty">No matching results</li>';
}

function moveQuick(delta) {
  const items = $("#qi-list").querySelectorAll("li[data-i]");
  if (!items.length) return;
  items[quick.index]?.classList.remove("selected");
  quick.index = (quick.index + delta + items.length) % items.length;
  items[quick.index].classList.add("selected");
  items[quick.index].scrollIntoView({ block: "nearest" });
}

function acceptQuick(i = quick.index) {
  const item = quick.filtered[i];
  closeQuickInput();
  item?.run();
}

$("#qi-input").addEventListener("input", filterQuick);
$("#qi-input").addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown") { e.preventDefault(); moveQuick(1); }
  else if (e.key === "ArrowUp") { e.preventDefault(); moveQuick(-1); }
  else if (e.key === "Enter") { e.preventDefault(); acceptQuick(); }
  else if (e.key === "Escape") { e.preventDefault(); closeQuickInput(); }
  else if (e.ctrlKey || e.key.startsWith("F")) return; // let global shortcuts through
  e.stopPropagation();
});
$("#qi-input").addEventListener("blur", () =>
  setTimeout(() => { if (!$("#quick-input").contains(document.activeElement)) $("#quick-input").hidden = true; }, 80),
);
$("#qi-list").addEventListener("mousedown", (e) => {
  const li = e.target.closest("li[data-i]");
  if (li) { e.preventDefault(); acceptQuick(+li.dataset.i); }
});

// ------------------------------------------------------------ keybindings

let chord = null; // pending Ctrl+K chord
const keyOf = (e) =>
  `${e.ctrlKey ? "ctrl+" : ""}${e.shiftKey ? "shift+" : ""}${e.altKey ? "alt+" : ""}${e.key.length === 1 ? e.key.toLowerCase() : e.key}`;

const bindings = {
  "ctrl+s": "save", "ctrl+shift+s": "saveAs", "ctrl+o": "openFile", "ctrl+n": "newFile",
  "ctrl+w": "closeEditor", "ctrl+F4": "closeEditor", "ctrl+b": "toggleSidebar",
  "ctrl+`": "toggleTerminal", "ctrl+j": "togglePanel", "ctrl+shift+`": "newTerminal", "ctrl+shift+~": "newTerminal", "ctrl+p": "quickOpen", "ctrl+e": "quickOpen",
  "ctrl+shift+p": "commandPalette", F1: "commandPalette",
  F5: "run", "ctrl+F5": "run", "ctrl+alt+n": "run", "shift+F5": "stop",
  "ctrl+Tab": "nextTab", "ctrl+PageDown": "nextTab", "ctrl+shift+Tab": "prevTab", "ctrl+PageUp": "prevTab",
  "ctrl+shift+e": "showExplorer", "ctrl+shift+f": "findInFiles", "ctrl+h": "replace", "ctrl+f": "find",
  "alt+z": "toggleWordWrap", "ctrl+=": "zoomIn", "ctrl+shift++": "zoomIn", "ctrl++": "zoomIn",
  "ctrl+-": "zoomOut", "ctrl+shift+_": "zoomOut", "ctrl+0": "zoomReset",
};
const chordBindings = { "ctrl+o": "openFolder", "ctrl+w": "closeAll", s: "saveAll", "ctrl+s": "saveAll" };
// Keys the WebView would otherwise treat as browser shortcuts (reload, print, devtools, …).
const blockedBrowserKeys = new Set(["F5", "ctrl+r", "ctrl+shift+r", "ctrl+p", "F3", "F7", "ctrl+u", "ctrl+shift+i", "ctrl+shift+j", "ctrl+shift+c"]);

window.addEventListener(
  "keydown",
  (e) => {
    if (!$("#modal-backdrop").hidden) return;
    if (menu.open && menuKey(e) !== false) return;
    const k = keyOf(e);
    if (chord) {
      if (["Control", "Shift", "Alt"].includes(e.key)) return;
      chord = null;
      const cmd = chordBindings[k];
      if (cmd) { e.preventDefault(); e.stopPropagation(); runCommand(cmd); }
      return;
    }
    if (k === "ctrl+k") { e.preventDefault(); e.stopPropagation(); chord = true; setTimeout(() => (chord = null), 2000); return; }
    if (state.view === "search" && e.target.closest?.("#search-view") && (k === "alt+c" || k === "alt+w")) {
      e.preventDefault();
      return setSearchOption(k === "alt+c" ? "matchCase" : "wholeWord", !search[k === "alt+c" ? "matchCase" : "wholeWord"]);
    }
    const cmd = bindings[k];
    if (cmd) {
      if (e.target.closest?.("#tree input") && !e.ctrlKey) return; // inline inputs keep their keys
      e.preventDefault();
      e.stopPropagation();
      runCommand(cmd);
      return;
    }
    if (blockedBrowserKeys.has(k)) e.preventDefault();
  },
  true,
);

// The WebView's own context menu only where it helps (editor, inputs, output).
document.addEventListener("contextmenu", (e) => {
  if (!e.target.closest(".cm-editor, input, #output, .term-body")) e.preventDefault();
});

// ------------------------------------------------------------------ sashes

function dragSash(sash, onMove) {
  sash.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    sash.setPointerCapture(e.pointerId);
    sash.classList.add("dragging");
    let frame = 0;
    let last = e;
    const move = (ev) => {
      last = ev;
      if (!frame) frame = requestAnimationFrame(() => { frame = 0; onMove(last); });
    };
    const up = () => {
      sash.classList.remove("dragging");
      sash.removeEventListener("pointermove", move);
      sash.removeEventListener("pointerup", up);
      persist();
    };
    sash.addEventListener("pointermove", move);
    sash.addEventListener("pointerup", up);
  });
}
dragSash($("#sash-sidebar"), (e) => {
  const w = Math.min(Math.max(e.clientX / state.zoom - 48, 170), window.innerWidth * 0.6);
  document.documentElement.style.setProperty("--sidebar-w", `${w}px`);
});
dragSash($("#sash-panel"), (e) => {
  const h = Math.min(Math.max(window.innerHeight - 24 - e.clientY, 90), window.innerHeight - 220);
  document.documentElement.style.setProperty("--panel-h", `${h}px`);
});

// ------------------------------------------------------------ window close

appWindow.onCloseRequested(async (event) => {
  persist();
  if (state.active) state.active.state = view.state;
  const dirty = state.tabs.filter((t) => t.dirty);
  if (state.run) await stopRun();
  if (!dirty.length) return;
  event.preventDefault();
  const choice = await modal(
    `Do you want to save the changes to ${dirty.length === 1 ? dirty[0].name : `${dirty.length} files`}?`,
    "Your changes will be lost if you don't save them.",
    [
      { id: "save", label: dirty.length === 1 ? "Save" : "Save All", primary: true },
      { id: "dont", label: "Don't Save" },
      { id: "cancel", label: "Cancel" },
    ],
  );
  if (choice === "cancel") return;
  if (choice === "save") for (const t of dirty) if (!(await saveTab(t))) return;
  await appWindow.destroy();
});

// -------------------------------------------------------------------- boot

function paintIcons() {
  $(".tb-logo").innerHTML = logo;
  $(".cc-icon").innerHTML = icons.search;
  $("#wc-min").innerHTML = icons.minimize;
  $("#wc-close").innerHTML = icons.winClose;
  const [explorer, searchBtn, run] = document.querySelectorAll(".ab-top .ab-item");
  explorer.innerHTML = abIcon("explorer");
  searchBtn.innerHTML = abIcon("search");
  run.innerHTML = abIcon("run");
  $(".ab-bottom .ab-item").innerHTML = abIcon("settings");
  $(".ws-header .twistie").innerHTML = icons.chevron;
  for (const id of ["newFile", "newFolder", "refresh"]) $(`.ws-actions [data-action=${id}]`).innerHTML = icons[id];
  $(".ws-actions [data-action=collapseAll]").innerHTML = icons.collapse;
  $("#opt-case").innerHTML = icons.caseSensitive;
  $("#opt-word").innerHTML = icons.wholeWord;
  $(".panel-actions [data-action=stop]").innerHTML = icons.stop;
  $(".panel-actions [data-action=clearOutput]").innerHTML = icons.clear;
  $(".panel-actions [data-action=clearTerminal]").innerHTML = icons.clear;
  $(".panel-actions [data-action=togglePanel]").innerHTML = icons.close;
  $(".stdin-prompt").innerHTML = icons.chevron;
  $(".sb-brand").innerHTML = icons.bolt;
  $(".wm-logo").innerHTML = logo;
}

/** `fluxcode <folder|file>...` opens those (like `code .`); otherwise restore the last session. */
async function boot() {
  paintIcons();
  const session = loadSession();
  if (session?.sidebarW) document.documentElement.style.setProperty("--sidebar-w", session.sidebarW);
  if (session?.panelH) document.documentElement.style.setProperty("--panel-h", session.panelH);
  if (session?.wrap) setWrap(view, true);
  if (session?.zoom && session.zoom !== 1) setZoom(session.zoom);
  if (session?.panelTab) state.panelTab = session.panelTab;
  getVersion().then((v) => {
    VERSION = v;
    $(".tb-badge").hidden = !v.includes("-dev");
    $(".tb-badge").title = `Development build ${v} — not a final release`;
  }).catch(() => {});
  $("#panel").dataset.tab = state.panelTab;
  toggleSidebar(true);
  showPanel(false);
  setRunning(false);
  syncMaximized();
  activateTab(null);
  const launch = (await invoke("launch_paths").catch(() => null)) ?? [];
  try {
    if (!launch.length) await restoreSession(session);
    else {
      const folder = launch.find((p) => p.is_dir);
      if (folder) await setRoot(folder.path);
      for (const f of launch.filter((p) => !p.is_dir)) await openFile(f.path);
    }
  } finally {
    booted = true;
    persist();
  }
}

boot();

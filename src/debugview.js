// Run and Debug UI, VS Code style: breakpoints (kept per file across
// restarts), the floating debug toolbar, and the VARIABLES / WATCH /
// CALL STACK / BREAKPOINTS sections. The session itself (./debug.js) is only
// loaded when debugging starts, so an editor that never debugs pays for a few
// event listeners and nothing else.
//
// Rendering stays cheap: variables are fetched only when a scope is expanded,
// re-rendering after a step reuses the remembered expansion, and every async
// render is tagged with the stop it belongs to so a fast run of F10 presses
// never paints stale values.

import { breakpointLines, breakpointsChanged, setBreakpointsEffect, setExecLineEffect, setDebugHover, toggleBreakpoint } from "./breakpoints.js";

const BP_KEY = "fluxcode.breakpoints";
const WATCH_KEY = "fluxcode.watch";
const REASONS = { breakpoint: "Paused on breakpoint", step: "Paused on step", pause: "Paused", entry: "Paused on entry", exception: "Paused on exception" };

const load = (key, fallback) => { try { return JSON.parse(localStorage.getItem(key) ?? "null") ?? fallback; } catch { return fallback; } };
const save = (key, value) => { try { localStorage.setItem(key, JSON.stringify(value)); } catch {} };

/**
 * @param ctx { $, state, view, icons, fileIcon, basename, relative, samePath, toast,
 *              openFile(path, opts), onSessionEnd() }
 */
export function createDebugUI(ctx) {
  const { $, state, view, icons } = ctx;
  const keyOf = (p) => (/^[A-Za-z]:/.test(p) ? p.toLowerCase().replace(/\//g, "\\") : p);

  // ---------------------------------------------------------------- store

  /** path key -> { path, lines } */
  const bps = new Map(Object.entries(load(BP_KEY, {})).map(([path, lines]) => [keyOf(path), { path, lines }]));
  const watches = load(WATCH_KEY, []);
  const persistBps = () => save(BP_KEY, Object.fromEntries([...bps.values()].map((b) => [b.path, b.lines])));

  let session = null; // the running debug session (./debug.js)
  let paused = null; // { reason, text, frames } while stopped
  let frameId = 0;
  let stopGen = 0; // bumps on every stop/continue: async renders check it
  let execTab = null; // the tab whose state carries the current-line marker
  const expanded = new Set(["Locals", "Local"]); // variable paths kept open across steps
  const collapsedScopes = new Set();

  /** Record a file's breakpoints; returns false when nothing changed. */
  function record(path, lines) {
    const key = keyOf(path);
    const sorted = [...new Set(lines)].sort((a, b) => a - b);
    const prev = bps.get(key)?.lines ?? [];
    if (prev.join() === sorted.join()) return false;
    if (sorted.length) bps.set(key, { path, lines: sorted });
    else bps.delete(key);
    persistBps();
    session?.setBreakpoints(path, sorted);
    renderBreakpoints();
    return true;
  }

  /** Set a file's breakpoints from outside the editor (list, debugger moved them). */
  function setFileBreakpoints(path, lines) {
    if (!record(path, lines)) return;
    const tab = state.tabs.find((t) => ctx.samePath(t.path, path));
    if (!tab) return;
    const effects = setBreakpointsEffect.of(lines);
    if (tab === state.active) view.dispatch({ effects });
    else tab.state = tab.state.update({ effects }).state;
  }

  // ------------------------------------------------------------- toolbar

  const bar = document.createElement("div");
  bar.id = "debug-toolbar";
  bar.hidden = true;
  const tbButtons = [
    ["continue", "Continue (F5)", "dbgContinue"],
    ["stepOver", "Step Over (F10)", "dbgStepOver"],
    ["stepInto", "Step Into (F11)", "dbgStepInto"],
    ["stepOut", "Step Out (Shift+F11)", "dbgStepOut"],
    ["restart", "Restart (Ctrl+Shift+F5)", "dbgRestart"],
    ["stop", "Stop (Shift+F5)", "dbgStop"],
  ];
  bar.innerHTML = `<span class="dt-grip"></span>${tbButtons.map(([id, title]) => `<button data-dt="${id}" title="${title}">${icons[id === "continue" ? "dbgContinue" : `dbg${id[0].toUpperCase()}${id.slice(1)}`] ?? icons.play}</button>`).join("")}`;
  $("#editor-group").append(bar);
  bar.addEventListener("click", (e) => {
    const b = e.target.closest("[data-dt]");
    if (b) command(b.dataset.dt === "continue" && !paused ? "pause" : b.dataset.dt);
  });

  function paintToolbar() {
    bar.hidden = !session;
    const cont = bar.querySelector("[data-dt=continue]");
    cont.innerHTML = paused ? icons.dbgContinue : icons.dbgPause;
    cont.title = paused ? "Continue (F5)" : "Pause (F6)";
    for (const id of ["stepOver", "stepInto", "stepOut"]) bar.querySelector(`[data-dt=${id}]`).disabled = !paused;
    $("#statusbar").classList.toggle("debugging", !!session);
  }

  // -------------------------------------------------------------- view DOM

  const host = $("#debug-view");
  const sections = document.createElement("div");
  sections.className = "dbg-sections";
  const section = (id, title, actions = "") =>
    `<section class="dbg-sec" data-sec="${id}"><header class="dbg-sec-head"><span class="twistie open">${icons.chevron}</span><span class="dbg-sec-title">${title}</span><span class="dbg-sec-extra"></span><span class="dbg-sec-actions">${actions}</span></header><div class="dbg-sec-body" id="dbg-${id}" tabindex="0"></div></section>`;
  sections.innerHTML =
    '<div class="dbg-exception" hidden></div>' +
    section("variables", "Variables", `<button data-da="collapseVars" title="Collapse All">${icons.collapse}</button>`) +
    section("watch", "Watch", `<button data-da="addWatch" title="Add Expression">${icons.plus}</button><button data-da="clearWatch" title="Remove All Expressions">${icons.clear}</button>`) +
    section("stack", "Call Stack") +
    section("breakpoints", "Breakpoints", `<button data-da="clearBps" title="Remove All Breakpoints">${icons.clear}</button>`);
  host.append(sections);

  sections.addEventListener("click", (e) => {
    const act = e.target.closest("[data-da]");
    if (act) {
      e.stopPropagation();
      if (act.dataset.da === "collapseVars") { expanded.clear(); renderVariables(); }
      if (act.dataset.da === "addWatch") addWatch();
      if (act.dataset.da === "clearWatch") { watches.splice(0); save(WATCH_KEY, watches); renderWatch(); }
      if (act.dataset.da === "clearBps") for (const b of [...bps.values()]) setFileBreakpoints(b.path, []);
      return;
    }
    const head = e.target.closest(".dbg-sec-head");
    if (head) {
      const sec = head.parentElement;
      sec.classList.toggle("collapsed");
      head.firstElementChild.classList.toggle("open", !sec.classList.contains("collapsed"));
    }
  });

  function renderView() {
    const active = !!session;
    host.classList.toggle("debugging", active);
    $(".dbg-body").hidden = active;
    for (const id of ["variables", "stack"]) sections.querySelector(`[data-sec=${id}]`).hidden = !active;
    const exc = sections.querySelector(".dbg-exception");
    exc.hidden = !(paused?.reason === "exception" && paused.text);
    if (!exc.hidden) exc.innerHTML = `${icons.error}<span></span>`, (exc.lastElementChild.textContent = paused.text);
    renderStack();
    renderVariables();
    renderWatch();
    renderBreakpoints();
  }

  // ------------------------------------------------------------ variables

  const valueClass = (type) =>
    /^(str|string)$/.test(type) ? "v-str" : /^(int|float|complex|number|bigint|Decimal)$/.test(type) ? "v-num" : /^(bool|boolean|NoneType|null|undefined)$/.test(type) ? "v-kw" : /^(function|method|builtin_function_or_method|type)$/.test(type) ? "v-fn" : "";

  function varRow(v, depth, path) {
    const row = document.createElement("div");
    row.className = "dv-row";
    row.style.paddingLeft = `${8 + depth * 12}px`;
    row.dataset.depth = depth;
    row.dataset.path = path;
    if (v.ref) row.dataset.ref = v.ref;
    row.title = `${v.name}${v.type ? ` (${v.type})` : ""}: ${v.value}`.slice(0, 500);
    row.innerHTML = `<span class="twistie${v.ref ? "" : " none"}">${icons.chevron}</span><span class="dv-name"></span>${v.scope ? "" : '<span class="dv-sep">: </span>'}<span class="dv-value ${valueClass(v.type)}"></span>`;
    row.querySelector(".dv-name").textContent = v.name;
    row.querySelector(".dv-value").textContent = v.scope ? "" : v.value;
    if (v.scope) row.classList.add("dv-scope");
    return row;
  }

  /** Insert the children of `row` after it (and re-open remembered children). */
  async function expandRow(row, gen) {
    row.classList.add("open");
    row.firstElementChild.classList.add("open");
    let kids;
    try { kids = await session.variables(row.dataset.ref); } catch { return; }
    if (gen !== stopGen || !row.isConnected) return;
    const depth = +row.dataset.depth + 1;
    const frag = document.createDocumentFragment();
    const rows = kids.map((v) => varRow(v, depth, `${row.dataset.path}/${v.name}`));
    frag.append(...rows);
    if (!kids.length) {
      const empty = document.createElement("div");
      empty.className = "dv-row dv-empty";
      empty.dataset.depth = depth;
      empty.style.paddingLeft = `${20 + depth * 12}px`;
      empty.textContent = "(empty)";
      frag.append(empty);
    }
    row.after(frag);
    await Promise.all(rows.filter((r) => r.dataset.ref && expanded.has(r.dataset.path)).map((r) => expandRow(r, gen)));
  }

  function collapseRow(row) {
    row.classList.remove("open");
    row.firstElementChild.classList.remove("open");
    const depth = +row.dataset.depth;
    let next = row.nextElementSibling;
    while (next && +next.dataset.depth > depth) {
      const n = next.nextElementSibling;
      next.remove();
      next = n;
    }
  }

  async function renderVariables() {
    const box = $("#dbg-variables");
    const gen = stopGen;
    if (!session) return box.replaceChildren();
    if (!paused) { box.innerHTML = '<div class="dbg-empty">Running…</div>'; return; }
    let scopes;
    try { scopes = await session.scopes(frameId); } catch { scopes = []; }
    if (gen !== stopGen) return;
    const rows = scopes.map((s) => varRow({ name: s.name, value: "", type: "", ref: s.ref, scope: true }, 0, s.name));
    box.replaceChildren(...rows);
    const open = rows.filter((r, i) => !collapsedScopes.has(r.dataset.path) && (expanded.has(r.dataset.path) || (i === 0 && !scopes[0].expensive)));
    await Promise.all(open.map((r) => expandRow(r, gen)));
  }

  $("#dbg-variables").addEventListener("click", (e) => {
    const row = e.target.closest(".dv-row[data-ref]");
    if (!row || !session) return;
    if (row.classList.contains("open")) {
      collapseRow(row);
      expanded.delete(row.dataset.path);
      if (row.classList.contains("dv-scope")) collapsedScopes.add(row.dataset.path);
    } else {
      expanded.add(row.dataset.path);
      collapsedScopes.delete(row.dataset.path);
      expandRow(row, stopGen);
    }
  });

  // ---------------------------------------------------------------- watch

  async function renderWatch() {
    const box = $("#dbg-watch");
    const gen = stopGen;
    if (!watches.length) { box.innerHTML = '<div class="dbg-empty">No watch expressions. Click + to add one.</div>'; return; }
    const values = await Promise.all(watches.map(async (expr) => {
      if (!paused) return { name: expr, value: session ? "not available while running" : "not available", type: "", ref: 0, dim: true };
      try { return { ...(await session.evaluate(expr, frameId)), name: expr }; } catch (e) { return { name: expr, value: `${e.message ?? e}`, type: "", ref: 0, error: true }; }
    }));
    if (gen !== stopGen) return;
    const rows = values.map((v, i) => {
      const row = varRow(v, 0, `watch:${i}`);
      if (v.error) row.querySelector(".dv-value").classList.add("v-err");
      if (v.dim) row.querySelector(".dv-value").classList.add("v-dim");
      const x = document.createElement("button");
      x.className = "dv-x";
      x.title = "Remove Expression";
      x.innerHTML = icons.close;
      x.dataset.watch = i;
      row.append(x);
      return row;
    });
    box.replaceChildren(...rows);
  }

  $("#dbg-watch").addEventListener("click", (e) => {
    const x = e.target.closest(".dv-x");
    if (x) { watches.splice(+x.dataset.watch, 1); save(WATCH_KEY, watches); return renderWatch(); }
    const row = e.target.closest(".dv-row[data-ref]");
    if (!row || !session) return;
    if (row.classList.contains("open")) collapseRow(row);
    else expandRow(row, stopGen);
  });

  function addWatch() {
    const sec = sections.querySelector("[data-sec=watch]");
    sec.classList.remove("collapsed");
    const box = $("#dbg-watch");
    box.querySelector(".dbg-empty")?.remove();
    const input = document.createElement("input");
    input.className = "dv-input";
    input.placeholder = "Expression to watch";
    input.spellcheck = false;
    box.append(input);
    input.focus();
    let done = false;
    const finish = (ok) => {
      if (done) return;
      done = true;
      const v = input.value.trim();
      input.remove();
      if (ok && v) { watches.push(v); save(WATCH_KEY, watches); }
      renderWatch();
    };
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Enter") finish(true);
      if (e.key === "Escape") finish(false);
    });
    input.addEventListener("blur", () => finish(true));
  }

  // ------------------------------------------------------------ call stack

  function renderStack() {
    const box = $("#dbg-stack");
    const extra = sections.querySelector("[data-sec=stack] .dbg-sec-extra");
    extra.textContent = paused ? REASONS[paused.reason] ?? "Paused" : session ? "Running" : "";
    extra.className = `dbg-sec-extra${paused ? " paused" : ""}`;
    if (!paused) return box.replaceChildren();
    box.replaceChildren(...paused.frames.map((f, i) => {
      const row = document.createElement("div");
      row.className = `ds-row${i === frameId ? " active" : ""}${f.internal ? " internal" : ""}`;
      row.dataset.frame = i;
      row.title = `${f.path}:${f.line}`;
      row.innerHTML = '<span class="ds-name"></span><span class="ds-loc"></span>';
      row.firstElementChild.textContent = f.name;
      row.lastElementChild.textContent = `${ctx.basename(f.path)}:${f.line}`;
      return row;
    }));
  }

  $("#dbg-stack").addEventListener("click", (e) => {
    const row = e.target.closest(".ds-row");
    if (!row || !paused) return;
    const f = paused.frames[+row.dataset.frame];
    if (!f || (f.internal && !/[\\/]/.test(f.path))) return;
    selectFrame(+row.dataset.frame);
  });

  async function selectFrame(i) {
    frameId = i;
    stopGen++;
    await showFrame();
    renderStack();
    renderVariables();
    renderWatch();
  }

  /** Open the selected frame's file and mark its line. */
  async function showFrame() {
    const f = paused?.frames[frameId];
    if (!f) return;
    const tab = await ctx.openFile(f.path, { preview: true, line: f.line, focus: false, quiet: true });
    paintExec(tab, f.line, frameId === 0);
  }

  function paintExec(tab, line, top) {
    const clear = setExecLineEffect.of(null);
    if (execTab && execTab !== tab && state.tabs.includes(execTab)) {
      if (execTab === state.active) view.dispatch({ effects: clear });
      else execTab.state = execTab.state.update({ effects: clear }).state;
    }
    execTab = tab ?? null;
    if (!tab) return;
    const effects = setExecLineEffect.of(line ? { line, top } : null);
    if (tab === state.active) view.dispatch({ effects });
    else tab.state = tab.state.update({ effects }).state;
  }

  // ----------------------------------------------------------- breakpoints

  function renderBreakpoints() {
    const box = $("#dbg-breakpoints");
    const all = [...bps.values()].flatMap((b) => b.lines.map((line) => ({ path: b.path, line })));
    const extra = sections.querySelector("[data-sec=breakpoints] .dbg-sec-extra");
    extra.textContent = all.length ? `${all.length}` : "";
    extra.className = "dbg-sec-extra count";
    if (!all.length) { box.innerHTML = '<div class="dbg-empty">Click left of a line number, or press <kbd>F9</kbd>, to add a breakpoint.</div>'; return; }
    all.sort((a, b) => a.path.localeCompare(b.path) || a.line - b.line);
    const hit = paused?.frames[0];
    box.replaceChildren(...all.map((b) => {
      const row = document.createElement("div");
      row.className = `db-row${hit && ctx.samePath(hit.path, b.path) && hit.line === b.line ? " hit" : ""}`;
      row.dataset.path = b.path;
      row.dataset.line = b.line;
      row.title = `${b.path}:${b.line}`;
      row.innerHTML = `<span class="db-dot"></span>${ctx.fileIcon(ctx.basename(b.path))}<span class="db-name"></span><span class="db-dir"></span><span class="db-line"></span><button class="dv-x" title="Remove Breakpoint">${icons.close}</button>`;
      row.querySelector(".db-name").textContent = ctx.basename(b.path);
      row.querySelector(".db-dir").textContent = ctx.relative(state.root, b.path).replace(/[\\/]?[^\\/]+$/, "");
      row.querySelector(".db-line").textContent = b.line;
      return row;
    }));
  }

  $("#dbg-breakpoints").addEventListener("click", (e) => {
    const row = e.target.closest(".db-row");
    if (!row) return;
    const path = row.dataset.path;
    const line = +row.dataset.line;
    if (e.target.closest(".dv-x")) return setFileBreakpoints(path, (bps.get(keyOf(path))?.lines ?? []).filter((l) => l !== line));
    ctx.openFile(path, { preview: true, line });
  });

  // -------------------------------------------------------------- session

  const hooks = {
    async onStopped(info) {
      paused = info;
      // Start on the innermost frame that is the user's code (like "just my code").
      const mine = info.frames.findIndex((f) => !f.internal);
      frameId = mine >= 0 ? mine : 0;
      stopGen++;
      paintToolbar();
      setDebugHover((expr) => (paused ? session.evaluate(expr, frameId).then((r) => r.value, () => null) : Promise.resolve(null)));
      await showFrame();
      renderView();
      ctx.onPaused?.(info);
    },
    onContinued() {
      paused = null;
      stopGen++;
      setDebugHover(null);
      paintExec(null);
      paintToolbar();
      renderView();
      ctx.onContinued?.();
    },
    onBreakpoints(path, lines) {
      setFileBreakpoints(path, lines);
    },
  };

  /** Start a session: `cfg` is { kind, program, file, args, cwd, root, stopOnEntry, runtimeArgs }. */
  async function start(cfg) {
    const { startDebug } = await import("./debug.js");
    const map = new Map([...bps.values()].map((b) => [b.path, b.lines]));
    session = await startDebug({ ...cfg, breakpoints: map }, hooks);
    paused = null;
    stopGen++;
    paintToolbar();
    renderView();
    return session.id;
  }

  /** The program ended (or was stopped). */
  function ended() {
    if (!session) return;
    session.dispose();
    session = null;
    paused = null;
    stopGen++;
    setDebugHover(null);
    paintExec(null);
    paintToolbar();
    renderView();
  }

  function command(name) {
    if (name === "toggleBreakpoint") return state.active && toggleBreakpoint(view);
    if (!session) return false;
    switch (name) {
      case "continue": if (paused) session.continue(); break;
      case "pause": if (!paused) session.pause(); break;
      case "stepOver": if (paused) session.stepOver(); break;
      case "stepInto": if (paused) session.stepInto(); break;
      case "stepOut": if (paused) session.stepOut(); break;
      case "restart": ctx.restart(); break;
      case "stop": ctx.stop(); break;
      default: return false;
    }
    return true;
  }

  renderView();
  paintToolbar();

  return {
    start,
    ended,
    command,
    render: renderView,
    isActive: () => !!session,
    isPaused: () => !!paused,
    /** Evaluate in the selected frame (Debug Console in the Output panel). */
    evaluate: (expr) => session.evaluate(expr, frameId),
    /** A new tab's state with this file's saved breakpoints applied. */
    prepareState(path, edState) {
      const lines = path && bps.get(keyOf(path))?.lines;
      return lines?.length ? edState.update({ effects: setBreakpointsEffect.of(lines) }).state : edState;
    },
    /** Called for every update of the active editor. */
    editorUpdate(u, tab) {
      if (tab?.path && breakpointsChanged(u)) record(tab.path, breakpointLines(u.state));
    },
    /** A file or folder was renamed: carry its breakpoints along. */
    renamed(from, to) {
      const k = keyOf(from);
      let moved = false;
      for (const [key, b] of [...bps]) {
        if (key !== k && !key.startsWith(`${k}\\`) && !key.startsWith(`${k}/`)) continue;
        bps.delete(key);
        const path = to + b.path.slice(from.length);
        bps.set(keyOf(path), { path, lines: b.lines });
        moved = true;
      }
      if (moved) { persistBps(); renderBreakpoints(); }
    },
    removeAll() { for (const b of [...bps.values()]) setFileBreakpoints(b.path, []); },
    addWatch,
  };
}

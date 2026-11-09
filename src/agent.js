// OpenCode inside FluxCode, over the Agent Client Protocol (ACP).
//
// `opencode acp` is a JSON-RPC 2.0 agent on stdio. FluxCode is the client:
// it starts the process only when this view is used, streams the agent's
// messages, tool calls and plans into a light DOM transcript, answers its
// file-system and permission requests, and stops the process when idle.
// No framework, no markdown library: a 60-line renderer covers what agents write.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { prefs } from "./prefs.js";
import { diffLines } from "./agentdiff.js";

const esc = (s) => String(s).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const baseName = (p) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const IDLE_MS = 10 * 60 * 1000;
const MAX_TURNS = 60;

// ------------------------------------------------------------ markdown-lite

function inline(text) {
  return esc(text)
    .replace(/`([^`\n]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>")
    .replace(/(^|[\s(])\*([^*\n]+)\*/g, "$1<i>$2</i>")
    .replace(/\[([^\]\n]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" data-ext>$1</a>');
}

/** Markdown → HTML: fenced code, headings, lists, paragraphs, inline code/bold/links. */
export function renderMarkdown(src) {
  const out = [];
  const lines = src.split("\n");
  let para = [];
  let list = null;
  const flush = () => {
    if (para.length) { out.push(`<p>${inline(para.join("\n"))}</p>`); para = []; }
    if (list) { out.push(`</${list}>`); list = null; }
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const fence = /^\s*```(\w*)/.exec(line);
    if (fence) {
      flush();
      const code = [];
      for (i++; i < lines.length && !/^\s*```/.test(lines[i]); i++) code.push(lines[i]);
      out.push(`<div class="ag-code"><div class="ag-code-head"><span>${esc(fence[1] || "code")}</span><button data-copy title="Copy">copy</button></div><pre><code>${esc(code.join("\n"))}</code></pre></div>`);
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) { flush(); out.push(`<h${h[1].length + 2}>${inline(h[2])}</h${h[1].length + 2}>`); continue; }
    const li = /^\s*(?:[-*+]|(\d+)[.)])\s+(.*)$/.exec(line);
    if (li) {
      if (para.length) { out.push(`<p>${inline(para.join("\n"))}</p>`); para = []; }
      const kind = li[1] ? "ol" : "ul";
      if (list !== kind) { if (list) out.push(`</${list}>`); out.push(`<${kind}>`); list = kind; }
      out.push(`<li>${inline(li[2])}</li>`);
      continue;
    }
    if (!line.trim()) { flush(); continue; }
    if (list) { out.push(`</${list}>`); list = null; }
    para.push(line);
  }
  flush();
  return out.join("");
}

// ------------------------------------------------------------------ agent

export function createAgent(host, ctx) {
  const { icons } = ctx;
  let status = "off"; // off | starting | ready | busy | error
  let session = null; // { id, modes }
  let commands = [];
  let seq = 0;
  const pending = new Map();
  let turn = null; // current assistant turn DOM state
  let idleTimer = 0;
  let lastError = "";
  let usage = 0; // context tokens used, from usage_update
  const logTail = [];
  const changes = new Map(); // path -> { before, after }: every file the agent changed this session
  let unlisten = [];

  host.innerHTML = `
    <div class="ag-head">
      <span class="ag-status"><i class="ag-dot"></i><span class="ag-status-text">OpenCode</span></span>
      <span class="ag-actions">
        <button data-ag="new" title="New session">${icons.plus}</button>
        <button data-ag="power" title="Stop the agent process (frees memory)">${icons.stop}</button>
      </span>
    </div>
    <div class="ag-config" hidden></div>
    <div class="ag-changes" hidden></div>
    <div class="ag-log"></div>
    <div class="ag-composer">
      <div class="ag-cmds" hidden></div>
      <textarea class="ag-input" rows="1" placeholder="Ask OpenCode… (/ for commands)" spellcheck="false"></textarea>
      <div class="ag-row">
        <button class="ag-chip" data-ag="file" title="Attach the active file to the prompt"></button>
        <span class="ag-spacer"></span>
        <button class="ag-send" data-ag="send" title="Send (Enter)">${icons.send}</button>
      </div>
    </div>`;
  const $ = (s) => host.querySelector(s);
  const log = $(".ag-log");
  const input = $(".ag-input");

  // ------------------------------------------------------------ status

  function setStatus(s, text) {
    status = s;
    host.dataset.status = s;
    const label = text ?? { off: "OpenCode · off", starting: "Starting…", ready: "Ready", busy: "Working…", error: "Error" }[s];
    $(".ag-status-text").textContent = usage && (s === "ready" || s === "busy") ? `${label} · ${usage >= 1000 ? `${(usage / 1000).toFixed(usage >= 10000 ? 0 : 1)}k` : usage} tokens` : label;
    $(".ag-send").innerHTML = s === "busy" ? icons.stop : icons.send;
    $(".ag-send").title = s === "busy" ? "Stop (Esc)" : "Send (Enter)";
    $("[data-ag=power]").hidden = s === "off";
    ctx.onStatus?.(s);
  }

  function note(html, kind = "") {
    const el = document.createElement("div");
    el.className = `ag-note ${kind}`;
    el.innerHTML = html;
    log.append(el);
    scroll();
    return el;
  }

  let stick = true;
  log.addEventListener("scroll", () => { stick = log.scrollHeight - log.scrollTop - log.clientHeight < 60; });
  function scroll(force) {
    if (force) stick = true;
    if (stick) log.scrollTop = log.scrollHeight;
  }

  // --------------------------------------------------------------- rpc

  const send = (obj) => invoke("acp_send", { line: JSON.stringify(obj) });
  function request(method, params) {
    const id = ++seq;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      send({ jsonrpc: "2.0", id, method, params }).catch((e) => { pending.delete(id); reject(e); });
    });
  }
  const notify = (method, params) => send({ jsonrpc: "2.0", method, params }).catch(() => {});
  const respond = (id, result) => send({ jsonrpc: "2.0", id, result }).catch(() => {});
  const respondError = (id, code, message) => send({ jsonrpc: "2.0", id, error: { code, message } }).catch(() => {});

  function onLine(line) {
    let msg;
    try { msg = JSON.parse(line); } catch { return; }
    if (msg.method) {
      if (msg.id !== undefined) handleRequest(msg);
      else handleNotification(msg);
      return;
    }
    const p = pending.get(msg.id);
    if (!p) return;
    pending.delete(msg.id);
    msg.error ? p.reject(new Error(msg.error.message ?? "Agent error")) : p.resolve(msg.result);
  }

  async function handleRequest({ id, method, params }) {
    try {
      switch (method) {
        case "session/request_permission": return respond(id, { outcome: await askPermission(params) });
        case "fs/read_text_file": {
          const { text } = await invoke("read_file", { path: params.path });
          let content = text;
          if (params.line != null || params.limit != null) {
            const lines = text.split("\n");
            const start = Math.max(0, (params.line ?? 1) - 1);
            content = lines.slice(start, params.limit != null ? start + params.limit : undefined).join("\n");
          }
          return respond(id, { content });
        }
        case "fs/write_text_file":
          await invoke("write_file", { path: params.path, contents: params.content });
          recordChange(params.path, null, params.content);
          return respond(id, {});
        default: return respondError(id, -32601, `Method not supported: ${method}`);
      }
    } catch (e) {
      respondError(id, -32000, `${e}`);
    }
  }

  function handleNotification({ method, params }) {
    if (method !== "session/update" || !params?.update) return;
    const u = params.update;
    switch (u.sessionUpdate) {
      case "agent_message_chunk": return appendText("text", u.content);
      case "agent_thought_chunk": return appendText("thought", u.content);
      case "tool_call": return toolCall(u);
      case "tool_call_update": return toolUpdate(u);
      case "plan": return renderPlan(u.entries ?? []);
      case "available_commands_update": commands = u.availableCommands ?? []; return;
      case "usage_update": usage = u.used ?? usage; if (status === "ready" || status === "busy") setStatus(status); return;
      case "current_mode_update": if (session?.modes) { session.modes.currentModeId = u.currentModeId; renderModes(); } return;
      case "config_option_update": if (session) { session.config = u.configOptions ?? session.config; renderModes(); } return;
    }
  }

  // ------------------------------------------------------------ process

  async function start() {
    if (status !== "off" && status !== "error") return true;
    setStatus("starting");
    lastError = "";
    logTail.length = 0;
    try {
      await invoke("acp_start", { cwd: ctx.workspace() ?? null });
      if (!unlisten.length) {
        unlisten = await Promise.all([
          listen("acp-line", ({ payload }) => onLine(payload.line)),
          listen("acp-log", ({ payload }) => { logTail.push(payload.line); if (logTail.length > 40) logTail.shift(); }),
          listen("acp-exit", ({ payload }) => onExit(payload.code)),
        ]);
      }
      const init = await withTimeout(request("initialize", {
        protocolVersion: 1,
        clientCapabilities: { fs: { readTextFile: true, writeTextFile: true }, terminal: false },
        clientInfo: { name: "FluxCode", version: ctx.version?.() ?? "dev" },
      }), 30000, "initialize");
      if (init?.agentInfo?.version || init?.agentCapabilities) $(".ag-status-text").title = `ACP v${init.protocolVersion ?? 1}`;
      await newSession();
      return true;
    } catch (e) {
      failed(`${e?.message ?? e}`);
      return false;
    }
  }

  async function newSession() {
    log.replaceChildren();
    turn = null;
    const res = await withTimeout(request("session/new", { cwd: ctx.workspace() ?? ctx.home?.() ?? "C:\\", mcpServers: [] }), 60000, "session/new");
    // OpenCode v2 describes mode / model / effort as `configOptions`; older agents only have `modes`.
    session = { id: res.sessionId, modes: res.modes ?? null, config: res.configOptions ?? [] };
    const saved = prefs.get("agent").config ?? {};
    for (const opt of session.config) {
      const want = saved[opt.id];
      if (want && want !== opt.currentValue && opt.options?.some((o) => o.value === want)) {
        await request("session/set_config_option", { sessionId: session.id, configId: opt.id, value: want })
          .then((r) => { if (r?.configOptions) session.config = r.configOptions; else opt.currentValue = want; })
          .catch(() => {});
      }
    }
    const pref = prefs.get("agent").mode;
    if (pref && session.modes?.availableModes?.some((m) => m.id === pref) && pref !== session.modes.currentModeId) {
      await request("session/set_mode", { sessionId: session.id, modeId: pref }).then(() => { session.modes.currentModeId = pref; }).catch(() => {});
    }
    renderModes();
    setStatus("ready");
    note(`${icons.sparkle}<span>New session in <b>${esc(baseName(ctx.workspace() ?? "") || "no folder")}</b>. ${ctx.workspace() ? "" : "Open a folder so the agent can see your project."}</span>`);
    armIdle();
  }

  function failed(message) {
    const hint = /auth|login|api key|credential|provider/i.test(message + logTail.join(" "))
      ? "<br>Configure credentials: run <code>mura setup</code> or <code>opencode auth login</code> in Flux Terminal, then try again."
      : "";
    lastError = message;
    note(`${icons.warning}<span>${esc(message)}${hint}${logTail.length ? `<details><summary>log</summary><pre>${esc(logTail.slice(-12).join("\n"))}</pre></details>` : ""}</span>`, "err");
    invoke("acp_stop").catch(() => {});
    status = "off";
    host.dataset.status = "off";
    $("[data-ag=power]").hidden = true;
  }

  function onExit(code) {
    for (const p of pending.values()) p.reject(new Error("OpenCode exited"));
    pending.clear();
    if (status === "busy" || status === "starting") failed(`OpenCode exited (code ${code ?? "?"}).`);
    else if (status !== "off") { setStatus("off"); note(`${icons.info}<span>OpenCode stopped${code ? ` (code ${code})` : ""}.</span>`); }
    session = null;
    renderModes();
  }

  async function stop() {
    clearTimeout(idleTimer);
    if (status === "busy" && session) notify("session/cancel", { sessionId: session.id });
    await invoke("acp_stop").catch(() => {});
    session = null;
    renderModes();
    if (status !== "off") { setStatus("off"); note(`${icons.info}<span>OpenCode stopped. It starts again with your next message.</span>`); }
  }

  function armIdle() {
    clearTimeout(idleTimer);
    if (prefs.get("agent").autoStop) idleTimer = setTimeout(() => { if (status === "ready") stop(); }, IDLE_MS);
  }

  const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(`${what} timed out`)), ms))]);

  // ------------------------------------------------------------- modes

  // ------------------------------------------------------ option pills
  // Mode / model / effort come from OpenCode. Each is a small pill; clicking
  // opens one shared, fixed-position menu (no <select>: it is styled by the
  // theme, keyboard-navigable and costs one DOM node while closed).

  function optionList() {
    const list = [];
    const modes = session?.modes?.availableModes;
    if (modes?.length) list.push({ id: "__mode", name: "Mode", mode: true, current: session.modes.currentModeId, items: modes.map((m) => ({ value: m.id, name: m.name, description: m.description })) });
    for (const opt of session?.config ?? []) {
      if (opt.type !== "select" || !opt.options?.length) continue;
      list.push({ id: opt.id, name: opt.name, mode: opt.category === "mode", current: opt.currentValue, items: opt.options.map((o) => ({ value: o.value, name: o.name ?? o.value, description: o.description })) });
    }
    return list;
  }
  const shortName = (name) => String(name ?? "").split("/").pop();

  function renderModes() {
    const row = $(".ag-config");
    const list = optionList();
    row.hidden = !list.length;
    row.innerHTML = list.map((o) => {
      const cur = o.items.find((i) => i.value === o.current);
      return `<button class="ag-pill${o.mode ? " mode" : ""}" data-opt="${esc(o.id)}" title="${esc(o.name)}: ${esc(cur?.name ?? o.current ?? "")}"><small>${esc(o.name)}</small><span>${esc(shortName(cur?.name ?? o.current ?? "—"))}</span>${icons.chevron}</button>`;
    }).join("");
  }

  let menu = null; // the open option menu: { el, opt, index }
  function closeMenu() {
    if (!menu) return;
    menu.el.remove();
    host.querySelector(".ag-pill.open")?.classList.remove("open");
    menu = null;
  }
  function openMenu(optId) {
    closeMenu();
    const opt = optionList().find((o) => o.id === optId);
    const pill = host.querySelector(`.ag-pill[data-opt="${CSS.escape(optId)}"]`);
    if (!opt || !pill) return false;
    const el = document.createElement("div");
    el.className = "ag-menu";
    el.innerHTML = `<div class="ag-menu-title">${esc(opt.name)}</div>${opt.items.map((it, i) => `<button data-i="${i}" class="${it.value === opt.current ? "on" : ""}"><span class="ag-menu-check">${it.value === opt.current ? icons.check : ""}</span><span class="ag-menu-text"><b>${esc(it.name)}</b>${it.description ? `<small>${esc(it.description)}</small>` : it.name !== shortName(it.name) || String(it.value).includes("/") ? `<small>${esc(it.value)}</small>` : ""}</span></button>`).join("")}`;
    document.body.append(el);
    // Place below the pill, flip above if there is no room, keep inside the window.
    const r = pill.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8));
    const below = window.innerHeight - r.bottom - 12;
    const top = h <= below || below >= r.top ? r.bottom + 4 : Math.max(8, r.top - h - 4);
    if (h > Math.max(below, r.top - 12)) el.style.maxHeight = `${Math.max(below, r.top - 12)}px`;
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    pill.classList.add("open");
    menu = { el, opt, index: Math.max(0, opt.items.findIndex((it) => it.value === opt.current)) };
    el.querySelector("button.on")?.scrollIntoView({ block: "nearest" });
    el.addEventListener("mousedown", (e) => {
      const b = e.target.closest("button[data-i]");
      if (!b) return;
      e.preventDefault();
      pickMenu(+b.dataset.i);
    });
    return true;
  }
  function moveMenu(d) {
    if (!menu) return;
    const items = menu.el.querySelectorAll("button[data-i]");
    items[menu.index]?.classList.remove("focus");
    menu.index = (menu.index + d + items.length) % items.length;
    items[menu.index].classList.add("focus");
    items[menu.index].scrollIntoView({ block: "nearest" });
  }
  async function pickMenu(i) {
    if (!menu) return;
    const { opt } = menu;
    const value = opt.items[i]?.value;
    closeMenu();
    if (value == null || value === opt.current || !session) return;
    try {
      if (opt.mode && opt.id === "__mode") {
        await request("session/set_mode", { sessionId: session.id, modeId: value });
        session.modes.currentModeId = value;
        prefs.set("agent", { mode: value });
      } else {
        const r = await request("session/set_config_option", { sessionId: session.id, configId: opt.id, value });
        if (r?.configOptions) session.config = r.configOptions;
        else for (const o of session.config) if (o.id === opt.id) o.currentValue = value;
        prefs.set("agent", { config: { ...(prefs.get("agent").config ?? {}), [opt.id]: value } });
      }
    } catch (err) { ctx.toast(`${err.message}`, "warning"); }
    renderModes();
    input.focus();
  }
  $(".ag-config").addEventListener("click", (e) => {
    const pill = e.target.closest(".ag-pill");
    if (!pill) return;
    if (menu?.opt.id === pill.dataset.opt) closeMenu();
    else openMenu(pill.dataset.opt);
  });
  document.addEventListener("mousedown", (e) => { if (menu && !menu.el.contains(e.target) && !e.target.closest(".ag-pill")) closeMenu(); });
  document.addEventListener("keydown", (e) => {
    if (!menu) return;
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeMenu(); input.focus(); }
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); moveMenu(e.key === "ArrowDown" ? 1 : -1); }
    else if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); pickMenu(menu.index); }
  }, true);
  window.addEventListener("resize", closeMenu);
  log.addEventListener("scroll", closeMenu);

  // --------------------------------------------------------- transcript

  function userTurn(text, file) {
    const el = document.createElement("div");
    el.className = "ag-turn user";
    el.innerHTML = `<div class="ag-bubble">${inline(text).replace(/\n/g, "<br>")}</div>${file ? `<div class="ag-attach">${ctx.fileIcon(file.name)}<span>${esc(file.name)}</span></div>` : ""}`;
    log.append(el);
    trim();
    scroll(true);
  }

  function assistantTurn() {
    const el = document.createElement("div");
    el.className = "ag-turn agent";
    log.append(el);
    turn = { el, text: "", textEl: null, thought: "", thoughtEl: null, tools: new Map(), plan: null, frame: 0 };
    trim();
    return turn;
  }

  function appendText(kind, content) {
    if (!turn) assistantTurn();
    if (content?.type !== "text") return;
    if (kind === "thought") {
      turn.thought += content.text;
      if (!turn.thoughtEl) {
        turn.thoughtEl = document.createElement("details");
        turn.thoughtEl.className = "ag-thought";
        turn.thoughtEl.innerHTML = `<summary>${icons.sparkle}Thinking</summary><div></div>`;
        turn.el.append(turn.thoughtEl);
      }
    } else {
      turn.text += content.text;
      if (!turn.textEl) {
        turn.textEl = document.createElement("div");
        turn.textEl.className = "ag-md";
        turn.el.append(turn.textEl);
      }
    }
    if (!turn.frame) turn.frame = requestAnimationFrame(paint);
  }

  function paint() {
    if (!turn) return;
    turn.frame = 0;
    if (turn.textEl) turn.textEl.innerHTML = renderMarkdown(turn.text);
    if (turn.thoughtEl) turn.thoughtEl.lastElementChild.innerHTML = renderMarkdown(turn.thought);
    scroll();
  }

  const KIND_ICON = { read: icons.file, edit: icons.edit, delete: icons.trash, move: icons.file, search: icons.search, execute: icons.terminal, think: icons.sparkle, fetch: icons.reveal, other: icons.tool };

  function toolCall(u) {
    if (!turn) assistantTurn();
    const card = document.createElement("div");
    card.className = `ag-tool ${u.status ?? "pending"}`;
    card.dataset.id = u.toolCallId;
    card.innerHTML = `<button class="ag-tool-head"><span class="ag-tool-ic">${KIND_ICON[u.kind] ?? icons.tool}</span><span class="ag-tool-title"></span><span class="ag-tool-state"></span></button><div class="ag-tool-body" hidden></div>`;
    turn.el.append(card);
    turn.tools.set(u.toolCallId, card);
    turn.textEl = null; // text after a tool call starts a new block
    turn.thoughtEl = null;
    toolUpdate(u);
  }

  function toolUpdate(u) {
    const card = turn?.tools.get(u.toolCallId) ?? log.querySelector(`.ag-tool[data-id="${CSS.escape(u.toolCallId)}"]`);
    if (!card) return;
    if (u.title) card.querySelector(".ag-tool-title").textContent = u.title;
    if (u.status) { card.className = `ag-tool ${u.status}`; card.querySelector(".ag-tool-state").innerHTML = u.status === "completed" ? icons.check : u.status === "failed" ? icons.close : u.status === "in_progress" ? '<span class="spinner"></span>' : ""; }
    if (u.locations?.length) {
      const loc = u.locations[0];
      card.querySelector(".ag-tool-title").title = `${loc.path}${loc.line ? `:${loc.line}` : ""}`;
      card.dataset.path = loc.path;
      if (loc.line) card.dataset.line = loc.line;
    }
    if (u.content?.length) {
      const body = card.querySelector(".ag-tool-body");
      for (const c of u.content) {
        // OpenCode writes files itself; its diff is how the editor learns about the change.
        if (c.type === "diff" && c.path) recordChange(c.path, c.oldText ?? "", c.newText ?? "");
        if (c.type === "diff") {
          const old = (c.oldText ?? "").split("\n");
          const neu = (c.newText ?? "").split("\n");
          body.insertAdjacentHTML("beforeend", `<div class="ag-diff"><div class="ag-diff-file">${ctx.fileIcon(baseName(c.path))}${esc(baseName(c.path))}</div><pre>${old.slice(0, 60).map((l) => `<del>${esc(l)}</del>`).join("\n")}${old.length > 60 ? "\n…" : ""}\n${neu.slice(0, 80).map((l) => `<ins>${esc(l)}</ins>`).join("\n")}${neu.length > 80 ? "\n…" : ""}</pre></div>`);
        } else if (c.type === "content" && c.content?.type === "text") {
          const text = c.content.text.length > 4000 ? `${c.content.text.slice(0, 4000)}\n…` : c.content.text;
          body.insertAdjacentHTML("beforeend", `<pre class="ag-out">${esc(text)}</pre>`);
        }
      }
    }
    scroll();
  }

  function renderPlan(entries) {
    if (!turn) assistantTurn();
    if (!turn.plan) {
      turn.plan = document.createElement("div");
      turn.plan.className = "ag-plan";
      turn.el.prepend(turn.plan);
    }
    turn.plan.innerHTML = `<div class="ag-plan-head">${icons.plan}Plan</div>${entries.map((e) => `<div class="ag-plan-item ${esc(e.status ?? "pending")}"><i></i><span>${esc(e.content)}</span></div>`).join("")}`;
    scroll();
  }

  function askPermission(params) {
    const options = params.options ?? [];
    const allow = options.find((o) => o.kind === "allow_once") ?? options.find((o) => o.kind === "allow_always");
    if (prefs.get("agent").autoApprove && allow) return { outcome: "selected", optionId: allow.optionId };
    return new Promise((resolve) => {
      if (!turn) assistantTurn();
      const tc = params.toolCall ?? {};
      const card = (tc.toolCallId && turn.tools.get(tc.toolCallId)) ?? null;
      const ask = document.createElement("div");
      ask.className = "ag-perm";
      ask.innerHTML = `<div class="ag-perm-text">${icons.shield}<span>${esc(tc.title ?? "The agent wants to run a tool")}</span></div><div class="ag-perm-btns">${options.map((o) => `<button class="${/allow/.test(o.kind) ? "btn-primary" : "btn-secondary"}" data-opt="${esc(o.optionId)}">${esc(o.name)}</button>`).join("")}</div>`;
      ask.onclick = (e) => {
        const b = e.target.closest("[data-opt]");
        if (!b) return;
        ask.remove();
        resolve({ outcome: "selected", optionId: b.dataset.opt });
      };
      (card ?? turn.el).append(ask);
      turn.cancelPermission = () => { ask.remove(); resolve({ outcome: "cancelled" }); };
      scroll(true);
    });
  }

  function trim() {
    while (log.children.length > MAX_TURNS) log.firstElementChild.remove();
  }

  // ----------------------------------------------------------- changes
  // Files the agent changed, with before/after. The editor shows the hunks
  // inline (green added lines, red removed lines, Keep / Undo per hunk);
  // this list keeps the overview and the per-file Keep / Undo.

  function recordChange(path, oldText, newText) {
    const c = changes.get(path);
    if (c) c.after = newText;
    else changes.set(path, { before: oldText, after: newText });
    ctx.fileChanged?.(path, newText, c ? undefined : oldText ?? undefined);
    renderChanges();
  }

  function renderChanges() {
    const box = $(".ag-changes");
    box.hidden = !changes.size;
    if (!changes.size) { box.innerHTML = ""; return; }
    const rows = [...changes].map(([path, c]) => {
      let add = 0, del = 0;
      if (c.before != null) for (const h of diffLines(c.before, c.after)) { add += h.added.length; del += h.removed.length; }
      else add = c.after.split("\n").length;
      return `<div class="ag-ch-row" data-path="${esc(path)}"><button class="ag-ch-open" title="${esc(path)}">${ctx.fileIcon(baseName(path))}<span>${esc(baseName(path))}</span><em><i class="add">+${add}</i> <i class="del">−${del}</i></em></button><button data-ch="keep" title="Keep this file's changes">${icons.check}</button><button data-ch="undo" title="Undo: restore the file as it was">${icons.rerun}</button></div>`;
    }).join("");
    box.innerHTML = `<div class="ag-ch-head">${icons.edit}<b>Changes</b><span>${changes.size} file${changes.size > 1 ? "s" : ""}</span><span class="ag-spacer"></span><button data-ch="keepall">Keep all</button><button data-ch="undoall">Undo all</button></div>${rows}`;
  }

  async function settle(path, action) {
    const c = changes.get(path);
    if (!c) return;
    changes.delete(path);
    if (action === "undo") await ctx.revertFile?.(path, c.before);
    else ctx.keepFile?.(path);
    renderChanges();
  }

  $(".ag-changes").addEventListener("click", async (e) => {
    const open = e.target.closest(".ag-ch-open");
    if (open) return ctx.showDiff?.(open.parentElement.dataset.path);
    const act = e.target.closest("[data-ch]")?.dataset.ch;
    if (!act) return;
    if (act === "keepall" || act === "undoall") { for (const path of [...changes.keys()]) await settle(path, act === "undoall" ? "undo" : "keep"); return; }
    settle(e.target.closest(".ag-ch-row").dataset.path, act);
  });

  // ------------------------------------------------------------- prompt

  async function sendPrompt() {
    const text = input.value.trim();
    if (!text) return;
    if (status === "busy") return;
    if (!(await start())) return;
    input.value = "";
    autoSize();
    hideCommands();
    const file = prefs.get("agent").includeFile ? ctx.activeFile?.() : null;
    userTurn(text, file);
    assistantTurn();
    setStatus("busy");
    clearTimeout(idleTimer);
    const prompt = [{ type: "text", text }];
    if (file?.text != null && file.text.length < 200_000) {
      prompt.push({ type: "resource", resource: { uri: `file:///${file.path.replace(/\\/g, "/")}`, text: file.text, mimeType: "text/plain" } });
    }
    try {
      const res = await request("session/prompt", { sessionId: session.id, prompt });
      paint();
      if (res?.stopReason && res.stopReason !== "end_turn") note(`${icons.info}<span>${esc(res.stopReason.replace(/_/g, " "))}</span>`);
    } catch (e) {
      if (status !== "off") note(`${icons.warning}<span>${esc(e.message)}</span>`, "err");
    }
    turn?.cancelPermission?.();
    turn = null;
    if (status === "busy") setStatus("ready");
    armIdle();
    ctx.afterTurn?.();
  }

  function cancel() {
    if (status !== "busy" || !session) return;
    notify("session/cancel", { sessionId: session.id });
    turn?.cancelPermission?.();
  }

  // ----------------------------------------------------------- composer

  function autoSize() {
    input.style.height = "auto";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  }
  input.addEventListener("input", () => { autoSize(); showCommands(); });
  input.addEventListener("keydown", (e) => {
    const cmds = $(".ag-cmds");
    if (!cmds.hidden) {
      const items = [...cmds.querySelectorAll("button")];
      const i = items.findIndex((b) => b.classList.contains("on"));
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        items[i]?.classList.remove("on");
        items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.classList.add("on");
        return;
      }
      if ((e.key === "Tab" || e.key === "Enter") && items[i]) { e.preventDefault(); pickCommand(items[i].dataset.cmd); return; }
      if (e.key === "Escape") { e.preventDefault(); hideCommands(); return; }
    }
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendPrompt(); }
    else if (e.key === "Escape") { e.preventDefault(); cancel(); }
    e.stopPropagation();
  });
  function showCommands() {
    const m = /^\/(\w*)$/.exec(input.value);
    const box = $(".ag-cmds");
    if (!m || !commands.length) return hideCommands();
    const list = commands.filter((c) => c.name.startsWith(m[1])).slice(0, 8);
    if (!list.length) return hideCommands();
    box.innerHTML = list.map((c, i) => `<button data-cmd="${esc(c.name)}" class="${i ? "" : "on"}"><b>/${esc(c.name)}</b><span>${esc(c.description ?? "")}</span></button>`).join("");
    box.hidden = false;
  }
  function hideCommands() { $(".ag-cmds").hidden = true; }
  function pickCommand(name) { input.value = `/${name} `; hideCommands(); autoSize(); input.focus(); }
  $(".ag-cmds").addEventListener("mousedown", (e) => { const b = e.target.closest("[data-cmd]"); if (b) { e.preventDefault(); pickCommand(b.dataset.cmd); } });

  host.addEventListener("click", (e) => {
    const a = e.target.closest("[data-ag]")?.dataset.ag;
    if (a === "send") return status === "busy" ? cancel() : sendPrompt();
    if (a === "new") return status === "off" || status === "error" ? start() : (status === "busy" ? ctx.toast("Wait for the current answer or stop it first.", "warning") : newSession().catch((err) => failed(`${err.message}`)));
    if (a === "power") return stop();
    if (a === "file") { prefs.set("agent", { includeFile: !prefs.get("agent").includeFile }); return renderChip(); }
    const copy = e.target.closest("[data-copy]");
    if (copy) { navigator.clipboard.writeText(copy.closest(".ag-code").querySelector("code").textContent).then(() => ctx.toast("Copied."), () => {}); return; }
    const link = e.target.closest("a[data-ext]");
    if (link) { e.preventDefault(); ctx.openExternal?.(link.href); return; }
    const head = e.target.closest(".ag-tool-head");
    if (head) {
      const card = head.parentElement;
      const body = card.querySelector(".ag-tool-body");
      if (body.childElementCount) body.hidden = !body.hidden;
      else if (card.dataset.path) ctx.openFile(card.dataset.path, card.dataset.line ? +card.dataset.line : null);
    }
  });

  function renderChip() {
    const on = prefs.get("agent").includeFile;
    const f = ctx.activeFile?.();
    const chip = $(".ag-chip");
    chip.classList.toggle("on", on);
    chip.innerHTML = `${icons.file}<span>${f ? esc(f.name) : "active file"}</span>`;
    chip.title = on ? "The active file is sent with each prompt (click to turn off)" : "Click to attach the active file to each prompt";
  }
  prefs.on("agent", (v, patch) => { if ("includeFile" in patch) renderChip(); if ("autoStop" in patch) armIdle(); });

  renderChip();
  setStatus("off");
  note(`${icons.agent}<span><b>OpenCode</b> runs inside FluxCode through the Agent Client Protocol. The process starts with your first message and stops when idle.</span>`);

  return {
    focus: () => input.focus(),
    start,
    stop,
    newSession: () => (status === "off" || status === "error" ? start() : newSession()),
    isRunning: () => status !== "off" && status !== "error",
    refreshChip: renderChip,
    /** The editor kept/undid the last hunk of a file: drop it from the list. */
    fileSettled: (path) => { if (changes.delete(path)) renderChanges(); },
    changedFiles: () => [...changes.keys()],
    /** Open the picker for an option ("mode", "model", "effort" …); false when the session has none. */
    openOption: (id) => { const o = optionList().find((x) => x.id === id || (id === "mode" && x.mode)); return !!o && openMenu(o.id); },
    status: () => status,
    lastError: () => lastError,
    dispose: () => { for (const u of unlisten) u(); unlisten = []; stop(); },
  };
}

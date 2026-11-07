// Debug sessions, loaded on the first F5. One small API over two backends:
//
//  - Python: `fluxdbg.py` (bdb, shipped inside FluxCode) relayed by Rust over
//    a loopback socket — nothing to pip install.
//  - Node.js / TypeScript: `node --inspect-brk=127.0.0.1:0`; the webview talks
//    to Node's own inspector (Chrome DevTools Protocol) over WebSocket.
//
// Both report the same shapes, so the UI does not care which one runs:
//   frames:    [{ id, name, path, line, internal }]
//   scopes:    [{ name, ref, expensive }]
//   variables: [{ name, value, type, ref }]   (ref is falsy when not expandable)

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * @param opts  { kind: "python"|"node", program, file, args, cwd, root, stopOnEntry, breakpoints: Map<path, lines[]> }
 * @param hooks { onStopped({ reason, text, frames }), onContinued(), onBreakpoints(path, lines) }
 */
export function startDebug(opts, hooks) {
  return opts.kind === "python" ? startPython(opts, hooks) : startNode(opts, hooks);
}

// ------------------------------------------------------------------ Python

async function startPython(opts, hooks) {
  let id = null;
  let seq = 0;
  const pending = new Map();
  const early = [];
  const send = (obj) => (id == null ? Promise.resolve() : invoke("debug_send", { id, msg: JSON.stringify(obj) }).catch(() => {}));
  const request = (op, args = {}) =>
    new Promise((resolve, reject) => {
      const s = ++seq;
      pending.set(s, { resolve, reject });
      send({ op, seq: s, ...args });
    });

  const onMsg = (raw) => {
    let m;
    try { m = JSON.parse(raw); } catch { return; }
    switch (m.event) {
      case "ready":
        send({ op: "init", root: opts.root ?? opts.cwd, stopOnEntry: !!opts.stopOnEntry, breakpoints: Object.fromEntries(opts.breakpoints) });
        break;
      case "response": {
        const p = pending.get(m.seq);
        if (!p) break;
        pending.delete(m.seq);
        m.error ? p.reject(new Error(m.error)) : p.resolve(m.body);
        break;
      }
      case "stopped": hooks.onStopped({ reason: m.reason, text: m.text, frames: m.frames }); break;
      case "continued": hooks.onContinued(); break;
      case "breakpoints": hooks.onBreakpoints(m.path, m.lines); break;
    }
  };
  const unlisten = await listen("dbg-msg", ({ payload }) => {
    if (id == null) early.push(payload);
    else if (payload.id === id) onMsg(payload.msg);
  });
  try {
    id = await invoke("debug_python", { python: opts.program, script: opts.file, args: opts.args ?? [], cwd: opts.cwd });
  } catch (e) {
    unlisten();
    throw e;
  }
  for (const p of early.splice(0)) if (p.id === id) onMsg(p.msg);

  return {
    id,
    continue: () => send({ op: "continue" }),
    pause: () => send({ op: "pause" }),
    stepOver: () => send({ op: "next" }),
    stepInto: () => send({ op: "stepIn" }),
    stepOut: () => send({ op: "stepOut" }),
    setBreakpoints: (path, lines) => send({ op: "setBreakpoints", path, lines }),
    scopes: (frame) => request("scopes", { frame }),
    variables: (ref) => request("variables", { ref }),
    evaluate: (expression, frame) => request("evaluate", { expression, frame }),
    dispose() {
      unlisten();
      for (const p of pending.values()) p.reject(new Error("The debug session has ended."));
      pending.clear();
    },
  };
}

// -------------------------------------------------------------------- Node

const fileUrlToPath = (url) => {
  if (!url?.startsWith("file://")) return url ?? "";
  let p = decodeURIComponent(url.replace(/^file:\/\/\/?/, ""));
  if (/^[A-Za-z]:[\\/]/.test(p)) return p.replace(/\//g, "\\");
  return p.startsWith("/") ? p : `/${p}`;
};

/** A URL regex that matches `path` as a file:// URL or a plain path, any case, any slash. */
const urlRegexFor = (path) =>
  `${path
    .split("")
    .map((ch) => (/[a-z]/i.test(ch) ? `[${ch.toLowerCase()}${ch.toUpperCase()}]` : ch === "\\" || ch === "/" ? "[\\\\/]" : ch === " " ? "(?: |%20)" : ch.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))
    .join("")}$`;

const str = (v) => (v.type === "string" ? JSON.stringify(v.value) : v.value ?? v.type);

function fmtPreview(p, desc) {
  const parts = p.subtype === "array"
    ? (p.properties ?? []).map(str)
    : p.entries
      ? p.entries.map((e) => `${e.key ? `${fmtPreview(e.key, e.key.description)} => ` : ""}${fmtPreview(e.value, e.value.description)}`)
      : (p.properties ?? []).map((q) => `${q.name}: ${str(q)}`);
  if (p.type !== "object" || (!p.properties && !p.entries)) return p.description ?? desc ?? p.type;
  const body = parts.join(", ") + (p.overflow ? ", …" : "");
  if (p.subtype === "array") return `${desc ?? "Array"} [${body}]`;
  if (p.entries) return `${desc} {${body}}`;
  return `${desc && desc !== "Object" ? `${desc} ` : ""}{${body}}`;
}

/** One-line text for a CDP RemoteObject. */
function fmt(v) {
  if (!v) return "undefined";
  switch (v.type) {
    case "string": return JSON.stringify(v.value);
    case "undefined": return "undefined";
    case "number":
    case "boolean": return v.unserializableValue ?? String(v.value);
    case "bigint": return v.unserializableValue ?? v.description;
    case "symbol": return v.description;
    case "function": return `ƒ ${(v.description ?? "").split("\n")[0].replace(/^(async\s+)?function\s*/, "").replace(/\s*\{\s*$/, "").slice(0, 80)}`;
    case "object":
      if (v.subtype === "null") return "null";
      if (v.preview) return fmtPreview(v.preview, v.description);
      return v.description ?? v.className ?? "Object";
  }
  return v.description ?? String(v.value);
}
const typeOf = (v) => (v?.type === "object" ? v.subtype ?? v.className ?? "object" : v?.type ?? "undefined");
const expandable = (v) => v?.type === "object" && v.subtype !== "null" && !!v.objectId;

async function startNode(opts, hooks) {
  let id = null;
  let ws = null;
  let msgId = 0;
  let firstPause = true;
  let frames = [];
  let stderr = "";
  const pending = new Map();
  const scripts = new Map(); // scriptId -> url
  const bpIds = new Map(); // path -> breakpoint ids
  const early = [];

  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      if (!ws || ws.readyState !== WebSocket.OPEN) return reject(new Error("The debug session has ended."));
      const i = ++msgId;
      pending.set(i, { resolve, reject });
      ws.send(JSON.stringify({ id: i, method, params }));
    });
  const quiet = (p) => p.catch(() => {});

  async function setBreakpoints(path, lines) {
    for (const bid of bpIds.get(path) ?? []) quiet(send("Debugger.removeBreakpoint", { breakpointId: bid }));
    const ids = [];
    bpIds.set(path, ids);
    const urlRegex = urlRegexFor(path);
    await Promise.all(lines.map(async (line) => {
      const r = await send("Debugger.setBreakpointByUrl", { lineNumber: line - 1, urlRegex, columnNumber: 0 }).catch(() => null);
      if (r?.breakpointId) ids.push(r.breakpointId);
    }));
  }

  function onEvent(method, p) {
    switch (method) {
      case "Debugger.scriptParsed":
        scripts.set(p.scriptId, p.url);
        break;
      case "Debugger.paused": {
        // --inspect-brk stops before the first line; go on unless asked to stop there.
        if (firstPause) {
          firstPause = false;
          if (!opts.stopOnEntry && (p.reason === "Break on start" || p.reason === "other") && !p.hitBreakpoints?.length) {
            quiet(send("Debugger.resume"));
            return;
          }
        }
        frames = p.callFrames;
        const reason = p.hitBreakpoints?.length ? "breakpoint" : p.reason === "exception" || p.reason === "promiseRejection" ? "exception" : p.reason === "Break on start" ? "entry" : "step";
        hooks.onStopped({
          reason,
          text: reason === "exception" ? (p.data?.description ?? fmt(p.data)).split("\n")[0] : null,
          frames: frames.map((f, i) => {
            const url = scripts.get(f.location.scriptId) ?? f.url;
            return { id: i, name: f.functionName || "(anonymous)", path: fileUrlToPath(url), line: f.location.lineNumber + 1, internal: !url?.startsWith("file:") && !/^[A-Za-z]:\\|^\//.test(url ?? "") || /node_modules/.test(url ?? "") };
          }),
        });
        break;
      }
      case "Debugger.resumed":
        frames = [];
        hooks.onContinued();
        break;
      case "Runtime.executionContextDestroyed":
        ws?.close(); // lets Node exit instead of "Waiting for the debugger to disconnect..."
        break;
    }
  }

  function connect(url) {
    ws = new WebSocket(url);
    ws.onopen = async () => {
      quiet(send("Runtime.enable"));
      await quiet(send("Debugger.enable"));
      quiet(send("Debugger.setPauseOnExceptions", { state: "uncaught" }));
      quiet(send("Debugger.setBlackboxPatterns", { patterns: ["^node:", "[\\\\/]node_modules[\\\\/]"] }));
      await Promise.all([...opts.breakpoints].map(([path, lines]) => setBreakpoints(path, lines)));
      quiet(send("Runtime.runIfWaitingForDebugger"));
    };
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id != null) {
        const p = pending.get(m.id);
        if (!p) return;
        pending.delete(m.id);
        m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result);
      } else onEvent(m.method, m.params);
    };
    ws.onclose = () => {
      ws = null;
      for (const p of pending.values()) p.reject(new Error("The debug session has ended."));
      pending.clear();
    };
  }

  const onOutput = (payload) => {
    if (ws || payload.stream !== "stderr") return;
    stderr += payload.data;
    const m = stderr.match(/ws:\/\/[^\s]+/);
    if (m) connect(m[0]);
  };
  const unlisten = await listen("proc-output", ({ payload }) => {
    if (id == null) early.push(payload);
    else if (payload.id === id) onOutput(payload);
  });
  try {
    id = await invoke("spawn_process", { program: opts.program, args: [...(opts.runtimeArgs ?? []), "--inspect-brk=127.0.0.1:0", opts.file, ...(opts.args ?? [])], cwd: opts.cwd });
  } catch (e) {
    unlisten();
    throw e;
  }
  for (const p of early.splice(0)) if (p.id === id) onOutput(p);

  const callFrameId = (frame) => frames[frame]?.callFrameId;
  return {
    id,
    continue: () => quiet(send("Debugger.resume")),
    pause: () => quiet(send("Debugger.pause")),
    stepOver: () => quiet(send("Debugger.stepOver")),
    stepInto: () => quiet(send("Debugger.stepInto")),
    stepOut: () => quiet(send("Debugger.stepOut")),
    setBreakpoints: (path, lines) => (ws ? setBreakpoints(path, lines) : Promise.resolve()),
    async scopes(frame) {
      const f = frames[frame];
      if (!f) return [];
      const names = { local: "Local", block: "Block", closure: "Closure", catch: "Catch", with: "With", script: "Script", module: "Module", global: "Global", eval: "Eval" };
      return f.scopeChain.map((s) => ({ name: s.name && s.type === "closure" ? `Closure (${s.name})` : names[s.type] ?? s.type, ref: s.object.objectId, expensive: s.type === "global" }));
    },
    async variables(ref) {
      const r = await send("Runtime.getProperties", { objectId: ref, ownProperties: true, generatePreview: true });
      return (r.result ?? [])
        .filter((p) => p.value && p.name !== "__proto__")
        .slice(0, 300)
        .map((p) => ({ name: p.name, value: fmt(p.value), type: typeOf(p.value), ref: expandable(p.value) ? p.value.objectId : 0 }));
    },
    async evaluate(expression, frame) {
      const cf = callFrameId(frame);
      const r = cf
        ? await send("Debugger.evaluateOnCallFrame", { callFrameId: cf, expression, generatePreview: true, objectGroup: "flux" })
        : await send("Runtime.evaluate", { expression, generatePreview: true, objectGroup: "flux" });
      if (r.exceptionDetails) throw new Error((r.exceptionDetails.exception?.description ?? r.exceptionDetails.text ?? "Error").split("\n")[0]);
      return { name: expression, value: fmt(r.result), type: typeOf(r.result), ref: expandable(r.result) ? r.result.objectId : 0 };
    },
    dispose() {
      unlisten();
      ws?.close();
    },
  };
}


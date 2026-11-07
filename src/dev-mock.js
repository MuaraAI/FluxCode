// Mocked Tauri backend for previewing the UI in a normal browser (dev.html).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const ROOT = "C:\\demo";
const files = {
  "C:\\demo\\main.py": `"""Demo file."""\n\nimport sys\n\n\nclass Greeter:\n    def __init__(self, name: str) -> None:\n        self.name = name\n\n    def greet(self) -> str:\n        # f-strings, numbers and keywords\n        return f"Hello, {self.name}! {42 + 3.14}"\n\n\nif __name__ == "__main__":\n    for i in range(3):\n        if i % 2 == 0:\n            print(Greeter("FluxCode").greet())\n        else:\n            pass\n`,
  "C:\\demo\\utils\\helpers.py": "def add(a, b):\n    return a + b\n",
  "C:\\demo\\README.md": "# Demo\n",
  "C:\\demo\\web\\index.html": "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Demo</title>\n</head>\n<body>\n  <p>test halo</p>\n</body>\n</html>\n",
  "C:\\demo\\web\\style.css": "body {\n  margin: 0;\n}\n",
  "C:\\demo\\web\\app.js": "const el = document.querySelector('#app');\n",
  "C:\\demo\\package.json": "{\n  \"name\": \"demo\",\n  \"dependencies\": {\n    \"express\": \"^4.19.2\"\n  }\n}\n",
  "C:\\demo\\app\\Http\\Controllers\\UserController.php": "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Http\\Request;\n\nclass UserController extends Controller\n{\n    public function index(Request $request)\n    {\n        \n    }\n}\n",
};
const dirs = {
  "C:\\demo": ["app", "utils", "web", "main.py", "package.json", "README.md"], "C:\\demo\\utils": ["helpers.py"], "C:\\demo\\web": ["app.js", "index.html", "style.css"],
  "C:\\demo\\app": ["Http"], "C:\\demo\\app\\Http": ["Controllers"], "C:\\demo\\app\\Http\\Controllers": ["UserController.php"],
};

mockWindows("main");
localStorage.setItem("fluxcode.session", JSON.stringify({ root: ROOT, files: ["C:\\demo\\main.py"], active: "C:\\demo\\main.py" }));

let pid = 0;
mockIPC(
  async (cmd, args) => {
    switch (cmd) {
      case "list_dir":
        return (dirs[args.path] ?? []).map((name) => {
          const path = `${args.path}\\${name}`;
          return { name, path, is_dir: !!dirs[path] };
        });
      case "list_files": return Object.keys(files);
      case "read_file":
        if (!(args.path in files)) throw new Error("not found");
        return { text: files[args.path], lossy: false };
      case "github_status": return { login: null, name: null, source: null, gh: true };
      case "acp_info": return { path: "C:\\Users\\demo\\AppData\\Roaming\\npm\\node_modules\\@opencode\\cli\\bin\\opencode.exe", version: "2.2.5", running: false };
      case "term_env": return { home: "C:\\Users\\demo", drives: ["C:\\", "D:\\"], shells: ["cmd", "powershell", "pwsh"], user: "demo", computer: "PC" };
      case "access_check": return { granted: true };
      case "term_list": return (dirs[args.path] ?? []).map((name) => ({ name, path: `${args.path}\\${name}`, is_dir: !!dirs[`${args.path}\\${name}`], size: 120, hidden: false }));
      case "term_resolve": return args.input === ".." ? args.cwd.slice(0, args.cwd.lastIndexOf("\\")) || args.cwd : `${args.cwd}\\${args.input}`;
      case "term_is_command": return false;
      case "write_file": files[args.path] = args.contents; return null;
      case "resolve_python": return { path: "python", version: "Python 3.12.10" };
      case "launch_paths": return [];
      case "search_files": {
        // Same rules as the Rust side: case fold unless matchCase, whole-word on \w boundaries.
        const out = [];
        let total = 0;
        const q = args.matchCase ? args.query : args.query.toLowerCase();
        const word = (t, i) => !args.wholeWord || (!/[\p{L}\p{N}_]/u.test(t[i - 1] ?? "") && !/[\p{L}\p{N}_]/u.test(t[i + q.length] ?? ""));
        for (const [path, text] of Object.entries(files)) {
          const hits = [];
          text.split("\n").forEach((line, i) => {
            const hay = args.matchCase ? line : line.toLowerCase();
            let at = hay.indexOf(q);
            while (at >= 0 && !word(hay, at)) at = hay.indexOf(q, at + 1);
            if (at >= 0) hits.push({ line: i + 1, text: line.trim() });
          });
          total += hits.length;
          if (hits.length) out.push({ path, hits });
        }
        return { files: out, total, truncated: false };
      }
      // A fake program: prints, asks for a name (input()), then exits with a
      // traceback. Answer "spam" to get 15,000 lines first (render stress test).
      case "spawn_process": {
        const id = ++pid;
        setTimeout(() => emit("proc-output", { id, stream: "stdout", data: "Hello, FluxCode! 45.14 — unicode ✓ 日本語\n" }), 50);
        setTimeout(() => emit("proc-output", { id, stream: "stdout", data: "Siapa namamu? " }), 100);
        return id;
      }
      case "write_stdin": {
        const { id } = args;
        const name = args.data.trim();
        let delay = 30;
        if (name === "spam") {
          for (let c = 0; c < 300; c++) {
            let chunk = "";
            for (let i = 0; i < 50; i++) chunk += `line ${c * 50 + i} ${"x".repeat(40)}\n`;
            setTimeout(() => emit("proc-output", { id, stream: "stdout", data: chunk }), delay);
            delay += 4;
          }
        }
        setTimeout(() => emit("proc-output", { id, stream: "stdout", data: `Halo, ${name}!\n` }), delay + 30);
        setTimeout(() => emit("proc-output", { id, stream: "stderr", data: 'Traceback (most recent call last):\n  File "C:\\demo\\main.py", line 12, in greet\nZeroDivisionError: division by zero\n' }), delay + 60);
        setTimeout(() => emit("proc-exit", { id, code: 1 }), delay + 100);
        return null;
      }
      case "create_file":
      case "create_dir": {
        const parts = args.path.split("\\");
        for (let i = 2; i <= parts.length; i++) {
          const dir = parts.slice(0, i - 1).join("\\");
          const name = parts[i - 1];
          if (i < parts.length || cmd === "create_dir") dirs[parts.slice(0, i).join("\\")] ??= [];
          if (!(dirs[dir] ??= []).includes(name)) dirs[dir].push(name);
        }
        if (cmd === "create_file") files[args.path] = "";
        return null;
      }
      // A fake Python debug session (fluxdbg.py): pauses at the breakpoints, answers variables/evaluate.
      case "debug_python": {
        const id = ++pid;
        const msg = (o, delay = 20) => setTimeout(() => emit("dbg-msg", { id, msg: JSON.stringify(o) }), delay);
        globalThis.__dbg = { id, msg, step: 0, script: args.script };
        msg({ event: "ready", python: "3.12.10" });
        return id;
      }
      case "debug_send": {
        const d = globalThis.__dbg;
        const m = JSON.parse(args.msg);
        const frames = (line) => [
          { id: 0, name: "greet", path: d.script, line, internal: false },
          { id: 1, name: "<module>", path: d.script, line: 18, internal: false },
        ];
        const reply = (body) => d.msg({ event: "response", seq: m.seq, body });
        if (m.op === "init") {
          for (const [path, lines] of Object.entries(m.breakpoints)) d.msg({ event: "breakpoints", path, lines });
          emit("proc-output", { id: d.id, stream: "stdout", data: "Starting…\n" });
          d.msg({ event: "stopped", reason: "breakpoint", text: null, frames: frames(12) }, 120);
        } else if (m.op === "next" || m.op === "stepIn") {
          d.msg({ event: "continued" });
          d.msg({ event: "stopped", reason: "step", text: null, frames: frames(13 + d.step++) }, 80);
        } else if (m.op === "continue" || m.op === "stepOut") {
          d.msg({ event: "continued" });
          setTimeout(() => emit("proc-output", { id: d.id, stream: "stdout", data: "Hello, FluxCode! 45.14\n" }), 60);
          setTimeout(() => emit("proc-exit", { id: d.id, code: 0 }), 120);
        } else if (m.op === "scopes") reply([{ name: "Locals", ref: 1, expensive: false }, { name: "Globals", ref: 2, expensive: true }]);
        else if (m.op === "variables") {
          reply(+m.ref === 1
            ? [{ name: "self", value: "<__main__.Greeter object at 0x0000021F>", type: "Greeter", ref: 3 }, { name: "count", value: String(3 + d.step), type: "int", ref: 0 }, { name: "items", value: "['a', 'b', 'c']", type: "list", ref: 4 }]
            : +m.ref === 3 ? [{ name: "name", value: "'FluxCode'", type: "str", ref: 0 }]
            : +m.ref === 4 ? ["a", "b", "c"].map((v, i) => ({ name: String(i), value: `'${v}'`, type: "str", ref: 0 }))
            : [{ name: "Greeter", value: "<class '__main__.Greeter'>", type: "type", ref: 0 }, { name: "sys", value: "<module 'sys'>", type: "module", ref: 0 }]);
        } else if (m.op === "evaluate") reply(m.expression === "nope" ? undefined : { name: m.expression, value: m.expression === "self.name" ? "'FluxCode'" : "42", type: "int", ref: 0 });
        return null;
      }
      case "kill_process": setTimeout(() => emit("proc-exit", { id: args.id, code: null }), 20); return null;
      // A fake OpenCode over ACP: answers the handshake and streams a canned reply with a tool call.
      case "acp_start": return "opencode (mock)";
      case "acp_stop": return null;
      case "acp_send": {
        const msg = JSON.parse(args.line);
        const line = (o) => setTimeout(() => emit("acp-line", { line: JSON.stringify({ jsonrpc: "2.0", ...o }) }), 30);
        const update = (sessionId, update, delay) => setTimeout(() => emit("acp-line", { line: JSON.stringify({ jsonrpc: "2.0", method: "session/update", params: { sessionId, update } }) }), delay);
        if (msg.method === "initialize") line({ id: msg.id, result: { protocolVersion: 1, agentInfo: { name: "OpenCode", version: "mock" }, agentCapabilities: {} } });
        else if (msg.method === "session/new") line({ id: msg.id, result: { sessionId: "ses_mock", configOptions: [
          { id: "model", name: "Model", category: "model", type: "select", currentValue: "opencode/big-pickle", options: [{ value: "opencode/big-pickle", name: "opencode/Big Pickle" }, { value: "opencode/deepseek-v4.1-flash-free", name: "opencode/DeepSeek V4.1 Flash Free" }, { value: "my-ai-gweh/SigmaBoy", name: "my-ai-gweh/SigmaBoy" }] },
          { id: "effort", name: "Effort", category: "thought_level", type: "select", currentValue: "default", options: [{ value: "default", name: "Default" }, { value: "deep", name: "Deep" }, { value: "fast", name: "Fast" }] },
          { id: "mode", name: "Session Mode", category: "mode", type: "select", currentValue: "kerjain", options: [{ value: "kerjain", name: "kerjain", description: "Build: edit files and run commands" }, { value: "planing", name: "planing", description: "Plan only, no edits" }] },
        ] } });
        else if (msg.method === "session/set_config_option") line({ id: msg.id, result: {} });
        else if (msg.method === "session/prompt") {
          const sid = msg.params.sessionId;
          update(sid, { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: "Looking at the open file first." } }, 80);
          update(sid, { sessionUpdate: "plan", entries: [{ content: "Read index.html", status: "completed" }, { content: "Suggest a fix", status: "in_progress" }] }, 150);
          update(sid, { sessionUpdate: "tool_call", toolCallId: "t1", title: "Read web/index.html", kind: "read", status: "in_progress", locations: [{ path: "C:\\demo\\web\\index.html" }] }, 220);
          update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t1", status: "completed", content: [{ type: "content", content: { type: "text", text: "<!DOCTYPE html>…" } }] }, 500);
          // An edit, written straight to "disk" like the real OpenCode, reported as a diff.
          const target = "C:\\demo\\web\\index.html";
          const before = files[target];
          const after = before.includes("<style>") ? before.replace("<style>\n", "<style>\n      body { background: red; }\n") : before.replace("<head>", "<head>\n  <style>\n    body { background: red; }\n  </style>") .replace("<p>test halo</p>", "<p>halo dunia</p>");
          update(sid, { sessionUpdate: "tool_call", toolCallId: "t2", title: "edit", kind: "edit", status: "pending" }, 560);
          setTimeout(() => { files[target] = after; }, 600);
          update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t2", status: "completed", title: "edit", locations: [{ path: target }], content: [{ type: "content", content: { type: "text", text: "Edited web/index.html (1 replacement)" } }, { type: "diff", path: target, oldText: before, newText: after }] }, 620);
          const text = "Halo! Berikut ringkasan file **index.html**:\n\n- Ada `<style>` kosong di `<head>`\n- Tag `<body>` ganda\n\n```html\n<body>\n  <p>test halo</p>\n</body>\n```\n\nMau saya rapikan?";
          let at = 700;
          for (const chunk of text.match(/[^]{1,12}/g)) { update(sid, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: chunk } }, at); at += 25; }
          setTimeout(() => emit("acp-line", { line: JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { stopReason: "end_turn" } }) }), at + 50);
        }
        return null;
      }
      default: return null;
    }
  },
  { shouldMockEvents: true },
);

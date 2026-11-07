// Mocked Tauri backend for previewing the UI in a normal browser (dev.html).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const ROOT = "C:\\demo";
const files = {
  "C:\\demo\\main.py": `"""Demo file."""\n\nimport sys\n\n\nclass Greeter:\n    def __init__(self, name: str) -> None:\n        self.name = name\n\n    def greet(self) -> str:\n        # f-strings, numbers and keywords\n        return f"Hello, {self.name}! {42 + 3.14}"\n\n\nif __name__ == "__main__":\n    for i in range(3):\n        if i % 2 == 0:\n            print(Greeter("FluxCode").greet())\n        else:\n            pass\n`,
  "C:\\demo\\utils\\helpers.py": "def add(a, b):\n    return a + b\n",
  "C:\\demo\\README.md": "# Demo\n",
  "C:\\demo\\web\\index.html": "",
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
        const out = [];
        let total = 0;
        for (const [path, text] of Object.entries(files)) {
          const hits = text.split("\n").map((t, i) => ({ line: i + 1, text: t.trim() }))
            .filter((h) => h.text.toLowerCase().includes(args.query.toLowerCase()));
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
      case "kill_process": setTimeout(() => emit("proc-exit", { id: args.id, code: null }), 20); return null;
      default: return null;
    }
  },
  { shouldMockEvents: true },
);

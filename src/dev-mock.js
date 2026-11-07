// Mocked Tauri backend for previewing the UI in a normal browser (dev.html).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const ROOT = "C:\\demo";
const files = {
  "C:\\demo\\main.py": `"""Demo file."""\n\nimport sys\n\n\nclass Greeter:\n    def __init__(self, name: str) -> None:\n        self.name = name\n\n    def greet(self) -> str:\n        # f-strings, numbers and keywords\n        return f"Hello, {self.name}! {42 + 3.14}"\n\n\nif __name__ == "__main__":\n    for i in range(3):\n        if i % 2 == 0:\n            print(Greeter("FluxCode").greet())\n        else:\n            pass\n`,
  "C:\\demo\\utils\\helpers.py": "def add(a, b):\n    return a + b\n",
  "C:\\demo\\README.md": "# Demo\n",
};
const dirs = { "C:\\demo": ["utils", "main.py", "README.md"], "C:\\demo\\utils": ["helpers.py"] };

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
      case "read_file": return { text: files[args.path] ?? "", lossy: false };
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
      case "spawn_process": {
        const id = ++pid;
        setTimeout(() => emit("proc-output", { id, stream: "stdout", data: "Hello, FluxCode! 45.14\n" }), 50);
        setTimeout(() => emit("proc-output", { id, stream: "stderr", data: 'Traceback (most recent call last):\n  File "C:\\demo\\main.py", line 12, in greet\nZeroDivisionError: division by zero\n' }), 100);
        setTimeout(() => emit("proc-exit", { id, code: 1 }), 150);
        return id;
      }
      default: return null;
    }
  },
  { shouldMockEvents: true },
);

// End-to-end test of a FluxCode build, driven through the WebView's DevTools
// protocol (CDP). Key presses are injected into the page, so the user's real
// keyboard and mouse are never touched.
//
//   node scripts/test-app.mjs [path\to\fluxcode.exe]
//
// Opens examples/, runs Python, Node (JS + TS), the Live Preview with auto save
// + reload, the Flux Terminal and the Python GUI example, checks the results,
// takes screenshots and measures memory after each step.

import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const EXE = process.argv[2] ?? "C:/Users/User/.fluxcode-target/release/fluxcode.exe";
const EXAMPLES = join(ROOT, "examples");
const OUT = join(process.env.TEMP ?? ROOT, "fluxcode-test");
const PORT = 9233;
mkdirSync(OUT, { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (name, ok, detail = "") => {
  results.push({ name, ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `  (${detail})` : ""}`);
};

// ------------------------------------------------------------------ launch

const args = [
  "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,AutofillServerCommunication,MediaRouter,CalculateNativeWinOcclusion",
  "--disable-background-networking --disable-component-update --disable-sync --no-pings",
  "--disable-gpu --in-process-gpu --disable-backgrounding-occluded-windows",
  `--remote-debugging-port=${PORT}`,
].join(" ");
// A throwaway WebView2 profile: the test starts like a first run and never touches the user's session.
const profile = join(OUT, `profile-${Date.now()}`);
const app = spawn(EXE, [EXAMPLES], {
  env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: args, WEBVIEW2_USER_DATA_FOLDER: profile },
  stdio: "ignore",
});

let target;
for (let i = 0; i < 60 && !target; i++) {
  await sleep(250);
  try {
    const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
    target = list.find((t) => t.type === "page" && /tauri\.localhost/.test(t.url));
  } catch {}
}
if (!target) throw new Error("FluxCode did not expose a DevTools target");

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let seq = 0;
const pending = new Map();
ws.addEventListener("message", (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
});
const send = (method, params = {}) =>
  new Promise((res, rej) => {
    const id = ++seq;
    pending.set(id, (msg) => (msg.error ? rej(new Error(`${method}: ${msg.error.message}`)) : res(msg.result)));
    ws.send(JSON.stringify({ id, method, params }));
  });

const evaluate = async (expr) => (await send("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true })).result.value;
const waitFor = async (expr, ms = 10000) => {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const v = await evaluate(expr).catch(() => null);
    if (v) return v;
    await sleep(120);
  }
  return null;
};

const KEYS = {
  Enter: { code: "Enter", vk: 13, text: "\r" }, Tab: { code: "Tab", vk: 9 }, Escape: { code: "Escape", vk: 27 },
  F5: { code: "F5", vk: 116 }, "`": { code: "Backquote", vk: 192 }, p: { code: "KeyP", vk: 80 }, z: { code: "KeyZ", vk: 90 },
};
async function press(key, mods = {}) {
  const k = KEYS[key];
  const modifiers = (mods.alt ? 1 : 0) | (mods.ctrl ? 2 : 0) | (mods.shift ? 8 : 0);
  const base = { key, code: k.code, windowsVirtualKeyCode: k.vk, nativeVirtualKeyCode: k.vk, modifiers };
  await send("Input.dispatchKeyEvent", { type: "keyDown", ...base, ...(k.text && !modifiers ? { text: k.text } : {}) });
  await send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
  await sleep(60);
}
const type = (text) => send("Input.insertText", { text });

async function shot(name) {
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(data, "base64"));
}

/** Private memory of FluxCode and all its child processes (MB). */
function memory() {
  const ps = `$all = Get-CimInstance Win32_Process; $ids=@(${app.pid}); $q=@(${app.pid}); while($q){ $n=@(); foreach($i in $q){ $c=$all | ? ParentProcessId -eq $i | % ProcessId; $n+=$c; $ids+=$c }; $q=$n }; $p = $ids | % { Get-Process -Id $_ -ErrorAction SilentlyContinue }; '{0}|{1:N1}|{2:N1}' -f $p.Count, (($p|Measure-Object PrivateMemorySize64 -Sum).Sum/1MB), (($p|Measure-Object WorkingSet64 -Sum).Sum/1MB)`;
  const [count, priv, ws] = execFileSync("powershell", ["-NoProfile", "-Command", ps]).toString().trim().split("|");
  return { count: +count, priv: +priv.replace(/,/g, ""), ws: +ws.replace(/,/g, "") };
}
const mem = [];
const measure = (label) => { const m = memory(); mem.push({ label, ...m }); console.log(`      mem ${label}: ${m.count} procs, ${m.priv} MB private, ${m.ws} MB working set`); };

async function openFile(rel) {
  await press("p", { ctrl: true });
  await waitFor(`!document.querySelector('#quick-input').hidden`);
  await type(rel.replace(/\//g, "\\")); // quick open matches Windows paths
  await sleep(250);
  await press("Enter");
  return waitFor(`document.querySelector('.tab.active')?.title.toLowerCase().endsWith(${JSON.stringify(rel.replace(/\//g, "\\").toLowerCase())})`, 5000);
}
const outputText = () => evaluate(`document.querySelector('#output').innerText`);

// ------------------------------------------------------------------- tests

await waitFor(`document.querySelector('.ws-name')?.textContent === 'examples'`);
await sleep(800);
measure("idle, examples open");
await shot("00-start");

// Python
check("open python/main.py", !!(await openFile("python/main.py")));
await press("F5");
check("python prompt shown", !!(await waitFor(`document.querySelector('#output').innerText.includes('Siapa namamu?')`, 15000)));
await type("Budi");
await press("Enter");
check("python reads stdin", !!(await waitFor(`document.querySelector('#output').innerText.includes('Halo, Budi!')`)));
check("python exits 0", !!(await waitFor(`document.querySelector('#output').innerText.includes('exited with code=0')`)));
measure("after python run");
await shot("01-python");

check("open python/error_demo.py", !!(await openFile("python/error_demo.py")));
await press("F5");
await waitFor(`document.querySelector('#output').innerText.includes('exited with code=1')`, 15000);
check("traceback links", (await evaluate(`document.querySelectorAll('#output a[data-file]').length`)) >= 3);

// JavaScript (Node)
check("open js/hello.js", !!(await openFile("js/hello.js")));
check("js highlighted", (await evaluate(`document.querySelectorAll('.cm-content span').length`)) > 20);
await press("F5");
check("node prompt shown", !!(await waitFor(`document.querySelector('#output').innerText.includes('Siapa namamu?')`, 15000)));
await type("Ana");
await press("Enter");
check("node reads stdin", !!(await waitFor(`document.querySelector('#output').innerText.includes('Halo, Ana!')`)));
measure("after node run");
await shot("02-node");

check("open js/greet.ts", !!(await openFile("js/greet.ts")));
await press("F5");
check("typescript runs on node", !!(await waitFor(`document.querySelector('#output').innerText.includes('total bahasa: 5')`, 15000)), (await outputText()).split("\n").slice(1, 4).join(" | "));

// Web: Live Preview + auto save + reload
check("open web/index.html", !!(await openFile("web/index.html")));
check("HTML file icon shows its label", (await evaluate(`document.querySelector('.tab.active .ficon')?.textContent`)) === "</>");
await press("F5");
const url = await waitFor(`!document.querySelector('#preview-pane').hidden && document.querySelector('#preview-frame').src`, 5000);
check("preview pane opens", !!url, url ?? "");
if (url) {
  const html = await (await fetch(url)).text();
  check("preview serves page + reload script", html.includes("Flux Tasks") && html.includes("__flux/wait"));
  const css = await fetch(url.replace("index.html", "style.css"));
  check("preview serves CSS", css.ok && css.headers.get("content-type").startsWith("text/css"));
  const evil = await fetch(url.replace("index.html", "..%5C..%5CREADME.md"));
  check("preview refuses paths outside the folder", evil.status === 403 || evil.status === 404, `status ${evil.status}`);
}
await sleep(1500);
measure("preview open");
await shot("03-preview");

check("open web/style.css", !!(await openFile("web/style.css")));
const cssPath = join(EXAMPLES, "web", "style.css");
const original = readFileSync(cssPath, "utf8");
await type("/* flux autosave test */");
check("auto save writes the file", !!(await (async () => { for (let i = 0; i < 30; i++) { await sleep(100); if (readFileSync(cssPath, "utf8").includes("flux autosave test")) return true; } return false; })()));
check("tab not dirty after auto save", !!(await waitFor(`!document.querySelector('.tab.active').classList.contains('dirty')`, 3000)));
await press("z", { ctrl: true });
await sleep(1200);
check("undo + auto save restores the file", readFileSync(cssPath, "utf8") === original);

// Terminal
await press("`", { ctrl: true });
check("terminal opens and focuses", !!(await waitFor(`document.activeElement?.classList.contains('ti-input')`, 5000)));
await type("ls");
await press("Enter");
check("native ls lists files", !!(await waitFor(`document.querySelectorAll('.term-session:not([hidden]) .tl-item').length >= 3`)));
await type("python -c \"print(6*7)\"");
await press("Enter");
check("terminal runs python", !!(await waitFor(`[...document.querySelectorAll('.tblock.ok .tb-out')].some(e => e.innerText.trim() === '42')`, 10000)));
await type("node -v");
await press("Enter");
check("terminal runs node", !!(await waitFor(`[...document.querySelectorAll('.tblock.ok .tb-out')].some(e => /^v\\d+/.test(e.innerText.trim()))`, 10000)));
await type("we");
const ghost = await waitFor(`document.querySelector('.tk-ghost')?.textContent`, 3000);
check("inline suggestion for a folder", ghost === "b\\", `ghost=${JSON.stringify(ghost)}`);
await press("Tab");
check("Tab accepts the suggestion", (await evaluate(`document.querySelector('.ti-input').value`)) === "web\\");
await press("Enter");
check("typing a folder name enters it", !!(await waitFor(`document.querySelector('.tc.last')?.textContent === 'web'`, 5000)));
await type("..");
await press("Enter");
check("'..' goes up", !!(await waitFor(`document.querySelector('.tc.last')?.textContent === 'examples'`, 5000)));
const crumbs = await evaluate(`(() => { const c = document.querySelector('.tp-crumbs'); const first = c.querySelector('.tc:not([hidden])'); return { fits: c.scrollWidth <= c.clientWidth + 1, first: first?.textContent, last: c.querySelector('.tc.last')?.textContent }; })()`);
check("path bar fits (drive and current folder visible)", crumbs.fits && crumbs.first === "C:" && crumbs.last === "examples", JSON.stringify(crumbs));
measure("terminal used");
await shot("04-terminal");
const popOk = await evaluate(`(() => { document.querySelector('[data-t=settings]').click(); const p = document.querySelector('.term-pop'); const r = p.getBoundingClientRect(); const ok = !p.hidden && r.left >= 0 && r.top >= 0 && r.right <= innerWidth && r.bottom <= innerHeight; return ok; })()`);
await sleep(200);
await shot("05-terminal-settings");
check("settings popover stays inside the window", popOk);
await press("Escape");
await evaluate(`document.querySelector('.term-pop').hidden = true`);

await evaluate(`document.querySelector('[data-pv=close]').click()`);
await sleep(2500);
check("closing the preview hides it", await evaluate(`document.querySelector('#preview-pane').hidden`));
measure("preview closed");

// Python GUI app (opens a real window; stopped right after)
check("open python/gui_app.py", !!(await openFile("python/gui_app.py")));
await press("F5");
check("python GUI app starts", !!(await waitFor(`document.querySelector('#output').innerText.includes('Flux Counter window opened')`, 15000)));
measure("python GUI app running");
await press("F5", { shift: true });
check("stop ends the GUI app", !!(await waitFor(`document.querySelector('#output').innerText.includes('[Done]')`, 8000)));

// Layout sanity: nothing important outside the window.
const layout = await evaluate(`(() => {
  const r = (s) => document.querySelector(s)?.getBoundingClientRect();
  const sb = r('#statusbar');
  return { statusbarVisible: sb.bottom <= innerHeight + 0.5 && sb.height > 0, w: innerWidth, h: innerHeight };
})()`);
check("status bar fully inside the window", layout.statusbarVisible, `${layout.w}x${layout.h}`);

await sleep(500);
measure("final");
await shot("06-final");

writeFileSync(join(OUT, "results.json"), JSON.stringify({ results, mem }, null, 2));
const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} checks passed. Screenshots: ${OUT}`);
ws.close();
execFileSync("taskkill", ["/T", "/F", "/PID", String(app.pid)], { stdio: "ignore" });
process.exit(failed.length ? 1 : 0);

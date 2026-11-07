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

// Any key: letters, digits, F-keys and the special keys below. Sent through CDP, so they
// go through the page's real keyboard pipeline (global shortcuts, CodeMirror keymaps, inputs).
const SPECIAL = {
  Enter: [13, "Enter", "\r"], Tab: [9, "Tab"], Escape: [27, "Escape"], Backspace: [8, "Backspace"],
  "`": [192, "Backquote"], "~": [192, "Backquote"], "/": [191, "Slash"], "=": [187, "Equal"], "-": [189, "Minus"], ",": [188, "Comma"],
  PageDown: [34, "PageDown"], PageUp: [33, "PageUp"], ArrowUp: [38, "ArrowUp"], ArrowDown: [40, "ArrowDown"],
};
function keyInfo(key) {
  if (SPECIAL[key]) return SPECIAL[key];
  if (/^F\d{1,2}$/.test(key)) return [111 + +key.slice(1), key];
  if (/^[a-z]$/i.test(key)) return [key.toUpperCase().charCodeAt(0), `Key${key.toUpperCase()}`];
  if (/^\d$/.test(key)) return [key.charCodeAt(0), `Digit${key}`];
  throw new Error(`unknown key ${key}`);
}
async function press(key, mods = {}) {
  const [vk, code, text] = keyInfo(key);
  const k = mods.shift && /^[a-z]$/.test(key) ? key.toUpperCase() : key;
  const modifiers = (mods.alt ? 1 : 0) | (mods.ctrl ? 2 : 0) | (mods.shift ? 8 : 0);
  const base = { key: k, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, modifiers };
  await send("Input.dispatchKeyEvent", { type: "keyDown", ...base, ...(text && !modifiers ? { text } : {}) });
  await send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
  await sleep(70);
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
const outputText = () => evaluate(`document.querySelector('.out-text').innerText`);
const focusEditorNow = () => evaluate(`document.querySelector('.cm-content').focus()`);

// ------------------------------------------------------------------- tests

await waitFor(`document.querySelector('.ws-name')?.textContent === 'examples'`);
await sleep(800);
measure("idle, examples open");
await shot("00-start");

// Python
check("open python/main.py", !!(await openFile("python/main.py")));
await press("F5");
check("python prompt shown", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('Siapa namamu?')`, 15000)));
await type("Budi");
await press("Enter");
check("python reads stdin", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('Halo, Budi!')`)));
check("python exits 0", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('exited with code=0')`)));
measure("after python run");
await shot("01-python");

check("open python/error_demo.py", !!(await openFile("python/error_demo.py")));
await press("F5");
await waitFor(`document.querySelector('.out-text').innerText.includes('exited with code=1')`, 15000);
check("traceback links", (await evaluate(`document.querySelectorAll('.out-text a[data-file]').length`)) >= 3);

// JavaScript (Node)
check("open js/hello.js", !!(await openFile("js/hello.js")));
check("js highlighted", (await evaluate(`document.querySelectorAll('.cm-content span').length`)) > 20);
await press("F5");
check("node prompt shown", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('Siapa namamu?')`, 15000)));
await type("Ana");
await press("Enter");
check("node reads stdin", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('Halo, Ana!')`)));
measure("after node run");
await shot("02-node");

check("open js/greet.ts", !!(await openFile("js/greet.ts")));
await press("F5");
check("typescript runs on node", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('total bahasa: 5')`, 15000)), (await outputText()).split("\n").slice(1, 4).join(" | "));

// Web: Live Preview + auto save + reload
check("open web/index.html", !!(await openFile("web/index.html")));
check("HTML file icon is the markup tile", (await evaluate(`document.querySelector('.tab.active .ficon')?.dataset.kind`)) === "html");
const icons = await evaluate(`(() => { const r = [...document.querySelectorAll('#tabs .ficon')].map(i => i.getBoundingClientRect()); return r.length && r.every(b => Math.round(b.width) === 16 && Math.round(b.height) === 16); })()`);
check("every file icon is exactly 16x16", icons);
const label = await evaluate(`(() => { const l = document.querySelector('.tab.active .label'); return { clipped: l.scrollWidth > l.clientWidth, text: l.textContent }; })()`);
check("active tab label is not clipped", !label.clipped, JSON.stringify(label));
// Zero-padded line numbers: every gutter number has the same width.
const gutter = await evaluate(`(() => { const n = [...document.querySelectorAll('.cm-lineNumbers .cm-gutterElement')].slice(1).map(e => e.textContent.trim()).filter(Boolean); return { first: n[0], same: n.every(x => x.length === n[0].length) }; })()`);
check("line numbers are zero-padded (01, 02 …) and equally wide", gutter.first === "01" && gutter.same, JSON.stringify(gutter));
// Emmet: `!` suggests the boilerplate, Tab applies it; `ul>li*3` expands on Tab.
await focusEditorNow();
await press("a", { ctrl: true });
await type("!");
check("typing ! shows the Emmet suggestion", !!(await waitFor(`[...document.querySelectorAll('.cm-tooltip-autocomplete li')].some(li => li.textContent.includes('Emmet'))`, 3000)));
await press("Tab");
check("Tab applies the HTML boilerplate", !!(await waitFor(`document.querySelector('.cm-content').textContent.includes('<!DOCTYPE html>') && document.querySelector('.cm-content').textContent.includes('</html>')`, 3000)));
await press("Escape");
await press("a", { ctrl: true });
await type("<body>\n  ul>li*3>a{Item $}");
await sleep(150);
await press("Escape");
await press("Tab");
check("Tab expands ul>li*3>a{Item $} without a popup", !!(await waitFor(`(() => { const t = document.querySelector('.cm-content').textContent; return t.includes('<li><a href="">Item 1</a></li>') && t.includes('Item 3</a>'); })()`, 3000)));
await press("Escape");
await press("z", { ctrl: true });
await press("z", { ctrl: true });
await press("z", { ctrl: true });
await press("z", { ctrl: true });
await press("z", { ctrl: true });
await press("z", { ctrl: true });
await evaluate(`(() => { const v = document.querySelector('.cm-content'); return v.textContent.length; })()`);
await press("s", { ctrl: true });
await sleep(400);
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
check("active tab stays visible after the preview opens", await evaluate(`(() => { const t = document.querySelector('.tab.active').getBoundingClientRect(); const b = document.querySelector('#tabs').getBoundingClientRect(); return t.left >= b.left - 1 && t.right <= b.right + 1; })()`));
check("preview header is compact (name + folder)", (await evaluate(`document.querySelector('.pv-name').textContent + '|' + document.querySelector('.pv-dir').textContent`)) === "index.html|web");
check("preview finished loading", !!(await waitFor(`document.querySelector('#preview-pane').classList.contains('loaded')`, 5000)));
await evaluate(`document.querySelector('button[data-dev=phone]').click()`);
await sleep(450);
check("phone mode renders at 390px", Math.round(await evaluate(`document.querySelector('#preview-frame').getBoundingClientRect().width`)) === 390);
await shot("03b-preview-phone");
await evaluate(`document.querySelector('button[data-dev=full]').click()`);
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
const termSizes = await evaluate(`(() => { const px = (s) => parseFloat(getComputedStyle(document.querySelector(s)).fontSize); return { crumb: px('.tc.last'), note: px('.tnav'), input: px('.ti-input') }; })()`);
check("terminal UI text is comfortably sized", termSizes.crumb >= 13 && termSizes.note >= 13 && termSizes.input >= 13, JSON.stringify(termSizes));
check("consecutive folder moves share one compact pill", (await evaluate(`document.querySelectorAll('.term-session:not([hidden]) .tnav').length`)) === 1 && !!(await evaluate(`document.querySelector('.tnav-pill b')?.textContent === 'examples'`)));
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

// Settings: fonts, presets, auto contrast, in-place updates.
await evaluate(`document.fonts.ready.then(() => true)`);
check("interface uses the VS Code default font (Segoe UI), no Jersey", /Segoe UI/.test(await evaluate(`getComputedStyle(document.body).fontFamily`)) && !/Jersey/.test(await evaluate(`getComputedStyle(document.body).fontFamily`)));
const contrastJs = `(() => {
  const rgb = (c) => c.match(/[\\d.]+/g).slice(0, 3).map(Number);
  const lum = (c) => { const [r, g, b] = rgb(c).map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
  const cs = getComputedStyle(document.documentElement);
  const v = (n) => { const d = document.createElement('div'); d.style.color = cs.getPropertyValue(n); document.body.append(d); const c = getComputedStyle(d).color; d.remove(); return c; };
  return { text: ratio(v('--fg'), v('--bg-editor')), muted: ratio(v('--fg-muted'), v('--bg-side')), comment: ratio(v('--tk-comment'), v('--bg-editor')), theme: document.documentElement.dataset.theme };
})()`;
await evaluate(`document.querySelector('[data-action=settings]').click()`);
check("Settings panel opens", !!(await waitFor(`!!document.querySelector('#settings .ap-presets')`, 5000)));
await evaluate(`document.querySelector('[data-preset=flux-light]').click()`);
await sleep(300);
const light = await evaluate(contrastJs);
check("light theme applies and stays readable", light.theme === "light" && light.text >= 7 && light.muted >= 4.5 && light.comment >= 3.8, JSON.stringify(light));
await shot("08-appearance-light");
// A deliberately bad pick: mid-gray background and near-gray text.
await evaluate(`(() => { for (const [k, val] of [['bg', '#777777'], ['text', '#8a8a8a']]) { const i = document.querySelector('[data-color=' + k + ']'); i.value = val; i.dispatchEvent(new Event('input', { bubbles: true })); } })()`);
await sleep(300);
const fixed = await evaluate(contrastJs);
check("auto contrast rescues unreadable picks", fixed.text >= 7 && fixed.muted >= 4.5, JSON.stringify(fixed));
await evaluate(`document.querySelector('[data-preset=flux-dark]').click()`);
await sleep(300);
const darkT = await evaluate(contrastJs);
check("dark theme readable", darkT.theme === "dark" && darkT.text >= 7 && darkT.comment >= 3.8, JSON.stringify(darkT));
await shot("09-appearance-dark");
// Changing a font must not scroll the page back to the top (no re-render).
const fontChange = await evaluate(`(async () => {
  const body = document.querySelector('.st-body');
  const btn = document.querySelector('[data-k=editorFont][data-v=consolas]');
  btn.scrollIntoView({ block: 'center' });
  await new Promise(r => setTimeout(r, 50));
  const before = body.scrollTop;
  btn.click();
  await new Promise(r => setTimeout(r, 150));
  return { before, after: body.scrollTop, font: getComputedStyle(document.querySelector('.cm-scroller')).fontFamily, on: btn.classList.contains('on') };
})()`);
check("font option: Consolas applies without the panel jumping", fontChange.on && /^Consolas/.test(fontChange.font) && Math.abs(fontChange.after - fontChange.before) < 2, JSON.stringify(fontChange));
await evaluate(`document.querySelector('[data-k=editorFont][data-v=cascadia]').click()`);
await shot("10-settings-editor");
// Reset asks for confirmation first.
await evaluate(`document.querySelector('[data-ap=reset]').click()`);
check("reset to default asks to confirm", !!(await waitFor(`!document.querySelector('.st-confirm').hidden`, 1500)));
await evaluate(`document.querySelector('[data-ap=reset-no]').click()`);
// Settings has search, Terminal, Accounts and OpenCode sections.
const sections = await evaluate(`[...document.querySelectorAll('#settings .st-section')].map(s => s.dataset.section).join(',')`);
check("settings sections: appearance, editor, terminal, accounts, opencode", sections === "appearance,editor,terminal,accounts,opencode", sections);
await evaluate(`(() => { const i = document.querySelector('.st-search input'); i.value = 'shell'; i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await sleep(100);
check("settings search filters rows", (await evaluate(`[...document.querySelectorAll('#settings .st-row')].filter(r => !r.hidden).length`)) <= 3);
await press("Escape");
await sleep(250);
check("Settings panel closes", await evaluate(`!document.querySelector('#settings')`));

// Python GUI app (opens a real window; stopped right after)
check("open python/gui_app.py", !!(await openFile("python/gui_app.py")));
await press("F5");
check("python GUI app starts", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('Flux Counter window opened')`, 15000)));
measure("python GUI app running");
await press("F5", { shift: true });
check("stop ends the GUI app", !!(await waitFor(`document.querySelector('.out-text').innerText.includes('[Done]')`, 8000)));

// Command palette: clean, contiguous highlights and no scattered matches.
await press("p", { ctrl: true, shift: true });
await waitFor(`!document.querySelector('#quick-input').hidden`);
await type("ter");
await sleep(200);
const pal = await evaluate(`(() => {
  const rows = [...document.querySelectorAll('#qi-list li[data-i]')];
  return {
    labels: rows.map(r => r.textContent.replace(/\\s+(Ctrl|Shift|Alt|F\\d).*$/, '')),
    marks: rows.map(r => [...r.querySelectorAll('.qh')].map(m => m.textContent)),
    bg: rows[0] && getComputedStyle(rows[0].querySelector('.qh')).backgroundColor,
  };
})()`);
check("palette: 'ter' highlights one contiguous run per row", pal.marks.length > 0 && pal.marks.every((m) => m.length === 1 && m[0].toLowerCase() === "ter"), JSON.stringify(pal.marks.slice(0, 4)));
check("palette: no scattered matches like 'Toggle Word Wrap'", !pal.labels.some((l) => /Word Wrap/.test(l)));
check("palette: highlight has no background box", pal.bg === "rgba(0, 0, 0, 0)", pal.bg);
await shot("07-palette");
await press("Escape");
await press("p", { ctrl: true, shift: true });
await waitFor(`!document.querySelector('#quick-input').hidden`);
await type("settings");
await sleep(200);
const settingsCmds = await evaluate(`[...document.querySelectorAll('#qi-list li[data-i]')].map(r => r.textContent)`);
check("palette: 'settings' lists Open Settings, Terminal, Editor, OpenCode and GitHub settings", /Open Settings/.test(settingsCmds[0] ?? "") && settingsCmds.some((l) => /Terminal Settings/.test(l)) && settingsCmds.some((l) => /OpenCode Settings/.test(l)), JSON.stringify(settingsCmds.slice(0, 5)));
await press("Escape");

// ------------------------------------------------------------- shortcuts
// Every keyboard shortcut, pressed through the real key pipeline, with its effect checked.

const focusEditor = () => evaluate(`document.querySelector('.cm-content').focus()`);
const layoutOk = () => evaluate(`(() => {
  const m = document.querySelector('#main').getBoundingClientRect();
  const e = document.querySelector('#editor-group').getBoundingClientRect();
  const sb = document.querySelector('#statusbar').getBoundingClientRect();
  return m.width > 400 && e.width > 200 && m.left >= 47 && m.right <= innerWidth + 1 && sb.bottom <= innerHeight + 1;
})()`);
const sidebarShown = () => evaluate(`!document.querySelector('#workbench').classList.contains('no-sidebar')`);
const panelShown = () => evaluate(`!document.querySelector('#panel').hidden`);
const activeTitle = () => evaluate(`document.querySelector('.tab.active')?.title`);
const cursorStatus = () => evaluate(`document.querySelector('#sb-cursor').textContent`);
const activeLine = () => evaluate(`document.querySelector('.cm-activeLine')?.textContent`);
let gotoOpened = true;
async function gotoLine(n) {
  await press("g", { ctrl: true });
  gotoOpened = gotoOpened && !!(await waitFor(`!document.querySelector('#quick-input').hidden && document.querySelector('#qi-input').value === ':'`, 1500));
  await type(String(n));
  await press("Enter");
  await waitFor(`document.querySelector('#sb-cursor').textContent.startsWith('Ln ${n},')`, 2000);
}

// The reported bug: Ctrl+B with the Live Preview open collapsed the editor.
check("open web/index.html for shortcut tests", !!(await openFile("web/index.html")));
await press("F5");
await waitFor(`!document.querySelector('#preview-pane').hidden`, 5000);
await focusEditor();
await press("b", { ctrl: true });
await sleep(200);
check("Ctrl+B hides the side bar and the editor keeps its space (preview open)",
  !(await sidebarShown()) && (await layoutOk()) && (await evaluate(`!document.querySelector('#preview-pane').hidden && document.querySelector('#editor-host').getBoundingClientRect().width > 300`)));
await shot("11-ctrl-b-sidebar-hidden");
await press("b", { ctrl: true });
await sleep(200);
check("Ctrl+B again brings the side bar back", (await sidebarShown()) && (await layoutOk()));
await evaluate(`document.querySelector('.ab-item[data-view=explorer]').click()`);
await sleep(150);
const hiddenByClick = !(await sidebarShown()) && (await layoutOk());
await evaluate(`document.querySelector('.ab-item[data-view=explorer]').click()`);
await sleep(150);
check("clicking the active activity-bar icon toggles the side bar too", hiddenByClick && (await sidebarShown()));
await evaluate(`document.querySelector('[data-pv=close]').click()`);

const p0 = await panelShown();
await focusEditor();
await press("j", { ctrl: true });
const p1 = await panelShown();
await press("j", { ctrl: true });
const p2 = await panelShown();
check("Ctrl+J toggles the panel", p1 !== p0 && p2 === p0 && (await layoutOk()));

await focusEditor();
await press("`", { ctrl: true });
check("Ctrl+` opens the terminal and focuses its input", !!(await waitFor(`document.activeElement?.classList.contains('ti-input')`, 3000)));
await press("`", { ctrl: true });
check("Ctrl+` again (from the terminal) hides the panel", !!(await waitFor(`document.querySelector('#panel').hidden`, 2000)));
const pills0 = await evaluate(`document.querySelectorAll('.ts-pill').length`);
await press("~", { ctrl: true, shift: true });
check("Ctrl+Shift+` opens a new terminal", !!(await waitFor(`document.querySelectorAll('.ts-pill').length === ${pills0 + 1}`, 3000)));
await type("ping -n 20 127.0.0.1");
await press("Enter");
await waitFor(`!!document.querySelector('.term-session:not([hidden]) .tblock.running')`, 5000);
await press("c", { ctrl: true });
check("Ctrl+C stops the running terminal command", !!(await waitFor(`!document.querySelector('.term-session:not([hidden]) .tblock.running')`, 6000)));
await press("l", { ctrl: true });
check("Ctrl+L clears the terminal (animated, then empty)", !!(await waitFor(`document.querySelector('.term-session:not([hidden])').children.length === 0`, 2500)));
check("clear button sits right before the close button in the panel header", await evaluate(`(() => { const a = [...document.querySelectorAll('.panel-actions button')].filter(b => getComputedStyle(b).display !== 'none'); return a.length === 2 && a[0].dataset.action === 'clearTerminal' && a[1].dataset.action === 'togglePanel'; })()`));
await evaluate(`document.querySelector('.ts-pill.on [data-close]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }))`);
await press("j", { ctrl: true });

await focusEditor();
await press("f", { ctrl: true, shift: true });
check("Ctrl+Shift+F shows Search with its input focused", !!(await waitFor(`!document.querySelector('#search-view').hidden && document.activeElement?.id === 'search-input'`, 2000)));
await press("e", { ctrl: true, shift: true });
check("Ctrl+Shift+E shows the Explorer", !!(await waitFor(`!document.querySelector('#explorer').hidden`, 2000)));

await focusEditor();
await press("p", { ctrl: true });
check("Ctrl+P opens Go to File", !!(await waitFor(`!document.querySelector('#quick-input').hidden && document.querySelector('#qi-input').value === ''`, 2000)));
await press("Escape");
check("Escape closes Go to File", !!(await waitFor(`document.querySelector('#quick-input').hidden`, 1500)));
await press("p", { ctrl: true, shift: true });
check("Ctrl+Shift+P opens the Command Palette", !!(await waitFor(`!document.querySelector('#quick-input').hidden && document.querySelector('#qi-input').value === '>'`, 2000)));
await press("Escape");
await press("F1");
check("F1 opens the Command Palette", !!(await waitFor(`!document.querySelector('#quick-input').hidden && document.querySelector('#qi-input').value === '>'`, 2000)));
await press("Escape");
await waitFor(`document.querySelector('#quick-input').hidden`, 1500);

check("open python/main.py for editing shortcuts", !!(await openFile("python/main.py")));
const mainPath = join(EXAMPLES, "python", "main.py");
const mainOriginal = readFileSync(mainPath, "utf8");
await gotoLine(17);
check("Ctrl+G opens Go to Line in the quick input", gotoOpened);
check("Ctrl+G jumps to a line", (await cursorStatus()).startsWith("Ln 17,"), await cursorStatus());

await press("f", { ctrl: true });
check("Ctrl+F opens Find", !!(await waitFor(`!!document.querySelector('.cm-search')`, 1500)));
await press("Escape");
check("Escape closes Find", !!(await waitFor(`!document.querySelector('.cm-search')`, 1500)));
await focusEditor();
await press("h", { ctrl: true });
check("Ctrl+H opens Find & Replace", !!(await waitFor(`!!document.querySelector('.cm-search [name=replace]')`, 1500)));
await press("Escape");
await waitFor(`!document.querySelector('.cm-search')`, 1500);
await focusEditor();

await press("z", { alt: true });
const wrapOn = await evaluate(`document.querySelector('.cm-content').classList.contains('cm-lineWrapping')`);
await press("z", { alt: true });
const wrapOff = await evaluate(`!document.querySelector('.cm-content').classList.contains('cm-lineWrapping')`);
check("Alt+Z toggles word wrap", wrapOn && wrapOff);

const dpr0 = await evaluate(`devicePixelRatio`);
await press("=", { ctrl: true });
await sleep(350);
const dpr1 = await evaluate(`devicePixelRatio`);
await press("-", { ctrl: true });
await sleep(350);
const dpr2 = await evaluate(`devicePixelRatio`);
await press("=", { ctrl: true });
await press("0", { ctrl: true });
await sleep(350);
const dpr3 = await evaluate(`devicePixelRatio`);
check("Ctrl+= / Ctrl+- / Ctrl+0 zoom in, out and reset", dpr1 > dpr0 && Math.abs(dpr2 - dpr0) < 0.01 && Math.abs(dpr3 - dpr0) < 0.01, JSON.stringify([dpr0, dpr1, dpr2, dpr3]));
check("layout intact after zooming", await layoutOk());

await gotoLine(3);
await press("/", { ctrl: true });
const commented = await activeLine();
await press("/", { ctrl: true });
const uncommented = await activeLine();
check("Ctrl+/ toggles a line comment", commented?.startsWith("# import sys") && uncommented === "import sys", JSON.stringify([commented, uncommented]));
await press("ArrowDown", { alt: true });
const movedTo = await cursorStatus();
await press("ArrowUp", { alt: true });
check("Alt+↓ / Alt+↑ move the line", movedTo.startsWith("Ln 4,") && (await cursorStatus()).startsWith("Ln 3,") && (await activeLine()) === "import sys");
await press("d", { ctrl: true });
await press("d", { ctrl: true });
check("Ctrl+D adds the next occurrence", /\(12 selected\)/.test(await cursorStatus()), await cursorStatus());
await press("Escape");
await gotoLine(3);
await press("k", { ctrl: true, shift: true });
const afterDelete = await activeLine();
await press("z", { ctrl: true });
check("Ctrl+Shift+K deletes the line and Ctrl+Z brings it back", afterDelete !== "import sys" && (await activeLine()) === "import sys", JSON.stringify(afterDelete));
await press("a", { ctrl: true });
check("Ctrl+A selects everything", /selected\)/.test(await cursorStatus()));
await gotoLine(3);
await type("#");
await press("s", { ctrl: true });
check("Ctrl+S saves right away", !!(await (async () => { for (let i = 0; i < 20; i++) { await sleep(100); if (readFileSync(mainPath, "utf8").includes("#import sys")) return true; } return false; })()));
await press("z", { ctrl: true });
await press("k", { ctrl: true });
await press("s");
check("Ctrl+K S (save all) writes the undo back", !!(await (async () => { for (let i = 0; i < 20; i++) { await sleep(100); if (readFileSync(mainPath, "utf8") === mainOriginal) return true; } return false; })()));

const tabs0 = await evaluate(`document.querySelectorAll('.tab').length`);
await focusEditor();
await press("n", { ctrl: true });
check("Ctrl+N opens a new untitled file", !!(await waitFor(`document.querySelectorAll('.tab').length === ${tabs0 + 1} && /Untitled/.test(document.querySelector('.tab.active').textContent)`, 2000)));
await press("w", { ctrl: true });
check("Ctrl+W closes the editor", !!(await waitFor(`document.querySelectorAll('.tab').length === ${tabs0}`, 2000)));

const t0 = await activeTitle();
await press("Tab", { ctrl: true });
const t1 = await activeTitle();
await press("Tab", { ctrl: true, shift: true });
const t2 = await activeTitle();
check("Ctrl+Tab / Ctrl+Shift+Tab switch editors", t1 !== t0 && t2 === t0);
await press("PageDown", { ctrl: true });
const t3 = await activeTitle();
await press("PageUp", { ctrl: true });
check("Ctrl+PageDown / Ctrl+PageUp switch editors", t3 !== t0 && (await activeTitle()) === t0);

await press("k", { ctrl: true });
await press("t", { ctrl: true });
check("Ctrl+K Ctrl+T opens the Color Theme picker", !!(await waitFor(`!document.querySelector('#quick-input').hidden && document.querySelector('#qi-input').placeholder === 'Select Color Theme'`, 3000)));
await press("Escape");
check("Escape closes the theme picker", !!(await waitFor(`document.querySelector('#quick-input').hidden`, 1500)));
await press(",", { ctrl: true });
check("Ctrl+, opens Settings", !!(await waitFor(`!!document.querySelector('#settings')`, 3000)));
await press("Escape");
check("Escape closes Settings", !!(await waitFor(`!document.querySelector('#settings')`, 1500)));
await press("d", { ctrl: true, shift: true });
check("Ctrl+Shift+D shows Run and Debug with the runners found", !!(await waitFor(`!document.querySelector('#debug-view').hidden && document.querySelectorAll('.dbg-tool').length === 3`, 3000)));
await press("a", { ctrl: true, shift: true });
check("Ctrl+Shift+A shows the OpenCode view (composer focused, no process yet)", !!(await waitFor(`!document.querySelector('#agent-view').hidden && document.activeElement?.classList.contains('ag-input') && document.querySelector('#agent-view').dataset.status === 'off'`, 3000)));
await press("e", { ctrl: true, shift: true });

await evaluate(`document.querySelector('[data-menu=file]').dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))`);
const menuOpen = !!(await waitFor(`!document.querySelector('#menu-popup').hidden`, 1000));
await press("Escape");
check("Escape closes an open menu", menuOpen && !!(await waitFor(`document.querySelector('#menu-popup').hidden`, 1000)));

await press("k", { ctrl: true });
await press("w", { ctrl: true });
check("Ctrl+K Ctrl+W closes all editors", !!(await waitFor(`document.querySelectorAll('.tab').length === 0`, 3000)));
check("python/main.py unchanged after the shortcut tests", readFileSync(mainPath, "utf8") === mainOriginal);
check("layout intact after all shortcuts", await layoutOk());
await shot("12-after-shortcuts");

// Title bar: the Help menu never overlaps the command center; window buttons are 46px with 20px glyphs.
const tb = await evaluate(`(() => {
  const help = document.querySelector('[data-menu=help]').getBoundingClientRect();
  const cc = document.querySelector('#command-center').getBoundingClientRect();
  const close = document.querySelector('#wc-close').getBoundingClientRect();
  const glyph = document.querySelector('#wc-close svg').getBoundingClientRect();
  return { helpRight: help.right, ccLeft: cc.left, closeW: close.width, glyph: glyph.width };
})()`);
check("menu bar ends before the command center", tb.helpRight <= tb.ccLeft + 0.5, JSON.stringify(tb));
check("window buttons are 46px wide with 20px glyphs", Math.round(tb.closeW) === 46 && Math.round(tb.glyph) === 20, JSON.stringify(tb));
check("status bar has no lightning brand button", await evaluate(`!document.querySelector('.sb-brand')`));
check("activity bar: explorer, search, run and debug, opencode, settings", await evaluate(`document.querySelectorAll('.ab-item').length === 5 && !!document.querySelector('.ab-item[data-view=debug]') && !!document.querySelector('.ab-item[data-view=agent]')`));

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

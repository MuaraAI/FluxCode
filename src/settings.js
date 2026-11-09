// Settings panel (VS Code-style, light): Appearance, Editor, Terminal,
// Accounts (GitHub) and OpenCode in one searchable page. Loaded on demand.
// Changes apply instantly and update the controls in place — the page never
// re-renders, so it never jumps back to the top.

import { PRESETS, DEFAULTS, derive, contrast, applyAppearance, saveAppearance } from "./theme.js";
import { prefs } from "./prefs.js";

const esc = (s) => String(s).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Contrast badge: AAA ≥ 7, AA ≥ 4.5, "large" ≥ 3, otherwise low. */
function grade(ratio) {
  const r = (Math.floor(ratio * 10) / 10).toFixed(1); // round down: never overstate readability
  if (ratio >= 7) return `<span class="ap-grade good">${r}:1 · AAA</span>`;
  if (ratio >= 4.5) return `<span class="ap-grade good">${r}:1 · AA</span>`;
  if (ratio >= 3) return `<span class="ap-grade ok">${r}:1 · large text</span>`;
  return `<span class="ap-grade low">${r}:1 · low</span>`;
}

const SECTIONS = [["appearance", "Appearance"], ["editor", "Editor"], ["terminal", "Terminal"], ["accounts", "Accounts"], ["opencode", "OpenCode"]];

/**
 * ctx: { icons, get/set (appearance), editor: {get, set}, terminal: {shells, revoke},
 *        github: {status, login, loginCli, logout, openTokenPage}, agent: {status, open}, section }
 */
export function openSettings(ctx) {
  const { icons } = ctx;
  let s = { ...ctx.get() };
  document.getElementById("settings-backdrop")?.remove();
  const backdrop = document.createElement("div");
  backdrop.id = "settings-backdrop";
  backdrop.innerHTML = `<div id="settings" role="dialog" aria-label="Settings"></div>`;
  document.body.append(backdrop);
  const panel = backdrop.firstElementChild;

  // ------------------------------------------------------------ values

  const value = (ns, key) => (ns === "ap" ? s[key] : ns === "editor" ? ctx.editor.get()[key] : prefs.get(ns)[key]);
  let frame = 0;
  function commit(ns, patch) {
    if (ns === "ap") {
      s = { ...s, ...patch };
      ctx.set(s);
      saveAppearance(s);
      if (!frame) frame = requestAnimationFrame(() => { frame = 0; applyAppearance(s); renderScores(); });
    } else if (ns === "editor") ctx.editor.set(patch);
    else prefs.set(ns, patch);
    sync();
  }

  const seg = (ns, key, options, extra = "") =>
    `<div class="seg" ${extra}>${options.map(([v, label, title]) => `<button data-ns="${ns}" data-k="${key}" data-v="${esc(v)}" ${title ? `title="${esc(title)}"` : ""}>${label}</button>`).join("")}</div>`;
  const row = (title, desc, control, keywords = "") =>
    `<div class="st-row" data-q="${esc(`${title} ${desc} ${keywords}`.toLowerCase())}"><div class="st-text"><div class="st-label">${title}</div>${desc ? `<div class="st-desc">${desc}</div>` : ""}</div><div class="st-ctl">${control}</div></div>`;

  // ------------------------------------------------------------ render

  function render() {
    const { vars } = derive(s);
    const shells = ctx.terminal.shells();
    panel.innerHTML = `
      <div class="st-head">
        <span class="st-title">Settings</span>
        <label class="st-search">${icons.search}<input placeholder="Search settings" spellcheck="false" autocomplete="off"></label>
        <button class="ap-x" data-ap="close" title="Close (Esc)">${icons.close}</button>
      </div>
      <div class="st-main">
        <nav class="st-nav">${SECTIONS.map(([id, label]) => `<button data-go="${id}">${label}</button>`).join("")}</nav>
        <div class="st-body">
          <section class="st-section" data-section="appearance">
            <h3>Appearance</h3>
            <div class="st-row st-row-block" data-q="theme color preset dark light">
              <div class="st-text"><div class="st-label">Color theme</div><div class="st-desc">Every other color is derived from the theme and kept readable.</div></div>
              <div class="ap-presets">
                ${PRESETS.map((p) => {
                  const d = derive({ ...s, bg: p.bg, accent: p.accent, text: null }).vars;
                  return `<button class="ap-preset" data-preset="${p.id}" style="--pb:${p.bg};--ps:${d["--bg-side"]};--pa:${d["--accent"]};--pf:${d["--fg"]}">
                    <span class="ap-swatch"><i></i><b>Aa</b><em></em></span><span class="ap-pname">${esc(p.name)}</span></button>`;
                }).join("")}
              </div>
            </div>
            <div class="st-row st-row-block" data-q="accent background text color custom picker">
              <div class="st-text"><div class="st-label">Custom colors</div><div class="st-desc">Pick an accent, a background and (optionally) a text color.</div></div>
              <div class="ap-colors">
                <label class="ap-color"><input type="color" data-color="accent" value="${s.accent}"><span><b>Accent</b><code data-hex="accent"></code></span></label>
                <label class="ap-color"><input type="color" data-color="bg" value="${s.bg}"><span><b>Background</b><code data-hex="bg"></code></span></label>
                <label class="ap-color"><input type="color" data-color="text" value="${s.text ?? vars["--fg"]}"><span><b>Text</b><code data-hex="text"></code></span></label>
                <button class="ap-auto" data-ap="text-auto" title="Pick the text color automatically">Auto text</button>
              </div>
            </div>
            ${row("Auto contrast", "Nudges every color until text meets WCAG contrast (7:1 body text, 4.5:1 muted).", seg("ap", "autoContrast", [[true, "On"], [false, "Off"]]), "wcag readable accessibility")}
            <div class="ap-scores"></div>
          </section>

          <section class="st-section" data-section="editor">
            <h3>Editor</h3>
            ${row("Editor &amp; terminal font", "Monospaced fonts installed on Windows.", seg("ap", "editorFont", [["cascadia", "Cascadia Code"], ["consolas", "Consolas"], ["jetbrains", "JetBrains Mono", "Falls back to Cascadia Code when not installed"]]), "monospace code")}
            ${row("Editor font size", "", seg("ap", "editorSize", [12, 13, 14, 15, 16, 18].map((n) => [n, n])), "zoom text")}
            ${row("Interface size", "Menus, explorer, panels and labels.", seg("ap", "uiSize", [[12, "Compact"], [13, "Default"], [14, "Large"]]), "ui font workbench")}
            ${row("Auto Save", "Save 0.7 s after you stop typing and when you switch apps.", seg("editor", "autoSave", [[true, "On"], [false, "Off"]]), "autosave")}
            ${row("Word wrap", "Wrap long lines instead of scrolling horizontally (Alt+Z).", seg("editor", "wordWrap", [[true, "On"], [false, "Off"]]), "wrap lines")}
            <div class="st-row st-row-block" data-q="preview sample">
              <div class="ap-scale">
                <div class="ap-hero">FluxCode</div>
                <div class="ap-s-body">Menus, tabs, explorer and panels</div>
                <div class="ap-s-small">Status bar and small labels</div>
                <pre class="ap-s-code">def hello(name: str) -&gt; str:\n    return f"Halo, {name}!"</pre>
              </div>
            </div>
          </section>

          <section class="st-section" data-section="terminal">
            <h3>Terminal</h3>
            ${row("Shell", "Commands typed in Flux Terminal run in this shell.", seg("terminal", "shell", shells.map((sh) => [sh, sh === "powershell" ? "PowerShell" : sh])), "cmd powershell pwsh")}
            ${row("Font size", "", seg("terminal", "fontSize", [12, 13, 14, 15, 16].map((n) => [n, n])))}
            ${row("Cursor style", "", seg("terminal", "cursor", [["bar", "Bar"], ["block", "Block"], ["underline", "Line"]]), "caret")}
            ${row("Smooth caret", "Glide between positions instead of jumping.", seg("terminal", "smoothCaret", [[true, "On"], [false, "Off"]]), "animation")}
            ${row("Hidden files in ls", "Show dot-files and hidden items in listings.", seg("terminal", "showHidden", [[true, "Show"], [false, "Hide"]]), "dotfiles")}
            ${row("Folder permissions", "Flux Terminal only runs commands in folders you allowed.", `<button class="btn-danger" data-ap="revoke">${icons.shield}<span>Revoke all</span></button>`, "access revoke security")}
          </section>

          <section class="st-section" data-section="accounts">
            <h3>Accounts</h3>
            <div class="st-row st-row-block" data-q="github account sign in token login git">
              <div class="st-text"><div class="st-label">GitHub</div><div class="st-desc">Used by <code>git</code> and <code>gh</code> inside Flux Terminal and by OpenCode. The token is stored in your FluxCode config folder, never in the project.</div></div>
              <div class="gh-card"><span class="spinner"></span></div>
            </div>
          </section>

          <section class="st-section" data-section="opencode">
            <h3>OpenCode</h3>
            <div class="st-row st-row-block" data-q="opencode agent acp ai assistant status">
              <div class="st-text"><div class="st-label">Agent</div><div class="st-desc">FluxCode talks to the OpenCode CLI over the Agent Client Protocol. It starts only when the OpenCode view is used.</div></div>
              <div class="oc-card"><span class="spinner"></span></div>
            </div>
            ${row("Stop when idle", "Quit the agent process after 10 minutes without a prompt to free memory. It restarts instantly on the next message.", seg("agent", "autoStop", [[true, "On"], [false, "Off"]]), "memory lightweight")}
            ${row("Auto-approve tools", "Let the agent run tools (read, edit, shell) without asking each time.", seg("agent", "autoApprove", [[true, "On"], [false, "Off"]]), "permission yolo")}
            ${row("Attach the active file", "Send the open file as context with each prompt.", seg("agent", "includeFile", [[true, "On"], [false, "Off"]]), "context")}
          </section>
        </div>
      </div>
      <div class="ap-foot">
        <div class="st-reset">
          <button class="btn-secondary" data-ap="reset">Reset to default</button>
          <div class="st-confirm" hidden><span>Reset every setting?</span><button class="btn-secondary" data-ap="reset-no">Cancel</button><button class="btn-danger solid" data-ap="reset-yes">Reset</button></div>
        </div>
        <button class="btn-primary" data-ap="close">Done</button>
      </div>`;
    sync();
    renderScores();
    renderGithub();
    renderAgent();
  }

  /** Reflect the current values in every control (no re-render). */
  function sync() {
    for (const b of panel.querySelectorAll("button[data-ns][data-k]")) {
      b.classList.toggle("on", String(value(b.dataset.ns, b.dataset.k)) === b.dataset.v);
    }
    for (const b of panel.querySelectorAll("[data-preset]")) b.classList.toggle("on", b.dataset.preset === s.preset);
    panel.querySelector("[data-ap=text-auto]")?.classList.toggle("on", !s.text);
    const { vars } = derive(s);
    for (const inp of panel.querySelectorAll("input[data-color]")) {
      const k = inp.dataset.color;
      const want = k === "text" ? (s.text ?? vars["--fg"]) : s[k];
      if (document.activeElement !== inp && inp.value.toLowerCase() !== want.toLowerCase()) inp.value = want;
    }
  }

  function renderScores() {
    const { vars } = derive(s);
    const el = panel.querySelector(".ap-scores");
    if (!el) return;
    el.innerHTML = `
      <div><span>Text on editor</span>${grade(contrast(vars["--fg"], vars["--bg-editor"]))}</div>
      <div><span>Muted text on side bar</span>${grade(contrast(vars["--fg-muted"], vars["--bg-side"]))}</div>
      <div><span>Accent on side bar</span>${grade(contrast(vars["--accent"], vars["--bg-side"]))}</div>
      <div><span>Button text</span>${grade(contrast("#ffffff", vars["--accent-strong"]))}</div>
      <div><span>Code comments</span>${grade(contrast(vars["--tk-comment"], vars["--bg-editor"]))}</div>`;
    const shown = { bg: vars["--bg-editor"], accent: vars["--accent"], text: vars["--fg"] };
    for (const k of ["bg", "accent", "text"]) {
      const code = panel.querySelector(`[data-hex="${k}"]`);
      if (!code) continue;
      if (k === "text" && !s.text) code.textContent = `auto · ${shown.text}`;
      else code.textContent = shown[k].toLowerCase() === s[k].toLowerCase() ? s[k] : `${s[k]} → ${shown[k]}`;
      code.title = code.textContent.includes("→") ? "Adjusted by auto contrast to stay readable" : "";
    }
  }

  async function renderGithub() {
    const card = panel.querySelector(".gh-card");
    if (!card) return;
    let st;
    try { st = await ctx.github.status(); } catch (e) { st = { error: `${e}` }; }
    if (!card.isConnected) return;
    if (st?.login) {
      card.innerHTML = `
        <div class="gh-user">${icons.github}<div><b>${esc(st.name || st.login)}</b><span>@${esc(st.login)}${st.source === "gh" ? " · via GitHub CLI" : ""}</span></div></div>
        <div class="gh-actions"><button class="btn-secondary" data-ap="gh-logout">Sign out</button></div>`;
      return;
    }
    card.innerHTML = `
      <div class="gh-signin">
        <div class="gh-opt">
          <div class="gh-opt-text"><b>Personal access token</b><span>Create a token with <code>repo</code> and <code>read:user</code>, then paste it here.</span></div>
          <div class="gh-token"><input type="password" placeholder="ghp_… or github_pat_…" spellcheck="false" autocomplete="off"><button class="btn-primary" data-ap="gh-login">Sign in</button></div>
          <button class="st-link" data-ap="gh-token-page">Create a token on github.com ${icons.reveal}</button>
        </div>
        <div class="gh-opt">
          <div class="gh-opt-text"><b>GitHub CLI</b><span>Already signed in with <code>gh auth login</code>? Use that session.</span></div>
          <button class="btn-secondary" data-ap="gh-cli">Use GitHub CLI${st?.gh ? "" : " (not found)"}</button>
        </div>
        ${st?.error ? `<div class="gh-err">${esc(st.error)}</div>` : ""}
      </div>`;
  }

  async function renderAgent() {
    const card = panel.querySelector(".oc-card");
    if (!card) return;
    let st;
    try { st = await ctx.agent.status(); } catch (e) { st = { error: `${e}` }; }
    if (!card.isConnected) return;
    const name = st?.path && /mura/i.test(st.path) ? "Mura Harness" : "OpenCode";
    card.innerHTML = st?.path
      ? `<div class="oc-status"><span class="oc-dot ${st.running ? "on" : ""}"></span><div><b>${name} ${esc(st.version ?? "")}</b><span>${st.running ? "running" : "not running"} · <code>${esc(st.path)}</code></span></div></div>
         <div class="gh-actions"><button class="btn-secondary" data-ap="oc-open">Open Agent view</button>${st.running ? '<button class="btn-secondary" data-ap="oc-stop">Stop agent</button>' : ""}</div>`
      : `<div class="oc-status"><span class="oc-dot"></span><div><b>AI Agent not found</b><span>Install <code>mura</code> from MuaraAI or OpenCode from opencode.ai and reopen Settings.</span></div></div>
         ${st?.error ? `<div class="gh-err">${esc(st.error)}</div>` : ""}`;
  }

  // ------------------------------------------------------------ events

  panel.addEventListener("click", async (e) => {
    const preset = e.target.closest("[data-preset]");
    if (preset) {
      const p = PRESETS.find((x) => x.id === preset.dataset.preset);
      return commit("ap", { preset: p.id, bg: p.bg, accent: p.accent, text: null });
    }
    const opt = e.target.closest("button[data-ns][data-k]");
    if (opt) {
      const raw = opt.dataset.v;
      const v = raw === "true" ? true : raw === "false" ? false : /^\d+$/.test(raw) ? +raw : raw;
      return commit(opt.dataset.ns, { [opt.dataset.k]: v });
    }
    const go = e.target.closest("[data-go]");
    if (go) {
      panel.querySelector(`[data-section="${go.dataset.go}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
      return;
    }
    const act = e.target.closest("[data-ap]")?.dataset.ap;
    if (!act) return;
    const reset = panel.querySelector(".st-reset");
    switch (act) {
      case "close": return close();
      case "text-auto": return commit("ap", { text: null });
      case "reset": reset.classList.add("confirming"); reset.querySelector(".st-confirm").hidden = false; reset.querySelector("[data-ap=reset-yes]").focus(); return;
      case "reset-no": reset.classList.remove("confirming"); setTimeout(() => { reset.querySelector(".st-confirm").hidden = true; }, 160); return;
      case "reset-yes":
        commit("ap", { ...DEFAULTS });
        prefs.reset("terminal");
        prefs.reset("agent");
        ctx.editor.set({ ...prefs.defaults.editor });
        sync();
        reset.classList.remove("confirming");
        reset.querySelector(".st-confirm").hidden = true;
        ctx.toast("All settings were reset to their defaults.");
        return;
      case "revoke":
        await ctx.terminal.revoke();
        return ctx.toast("All folder permissions were revoked.");
      case "gh-login": {
        const input = panel.querySelector(".gh-token input");
        const token = input.value.trim();
        if (!token) return input.focus();
        const btn = e.target.closest("button");
        btn.disabled = true;
        btn.textContent = "Checking…";
        try { await ctx.github.login(token); ctx.toast("Signed in to GitHub."); }
        catch (err) { ctx.toast(`GitHub sign-in failed: ${err}`, "error"); }
        input.value = "";
        return renderGithub();
      }
      case "gh-cli":
        try { await ctx.github.loginCli(); ctx.toast("Signed in with the GitHub CLI session."); }
        catch (err) { ctx.toast(`${err}`, "error"); }
        return renderGithub();
      case "gh-logout": await ctx.github.logout(); return renderGithub();
      case "gh-token-page": return ctx.github.openTokenPage();
      case "oc-open": close(); return ctx.agent.open();
      case "oc-stop": await ctx.agent.stop(); return renderAgent();
    }
  });

  // Live while dragging in the picker; only the scores update (keeps the picker open).
  panel.addEventListener("input", (e) => {
    const key = e.target.dataset.color;
    if (key) return commit("ap", { [key]: e.target.value, ...(key === "text" ? {} : { preset: "custom" }) });
    if (e.target.closest(".st-search")) filter(e.target.value);
  });
  panel.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.target.closest(".gh-token")) panel.querySelector("[data-ap=gh-login]")?.click();
  });

  function filter(q) {
    q = q.trim().toLowerCase();
    for (const r of panel.querySelectorAll(".st-row")) r.hidden = !!q && !r.dataset.q.includes(q);
    for (const sec of panel.querySelectorAll(".st-section")) sec.hidden = !!q && ![...sec.querySelectorAll(".st-row")].some((r) => !r.hidden);
    panel.querySelector(".ap-scores").hidden = !!q;
  }

  // Highlight the section in view in the side nav.
  const body = () => panel.querySelector(".st-body");
  let navFrame = 0;
  panel.addEventListener("scroll", () => {
    if (navFrame) return;
    navFrame = requestAnimationFrame(() => {
      navFrame = 0;
      const b = body();
      const top = b.scrollTop + 60;
      let current = SECTIONS[0][0];
      for (const sec of b.querySelectorAll(".st-section")) if (sec.offsetTop <= top) current = sec.dataset.section;
      for (const n of panel.querySelectorAll("[data-go]")) n.classList.toggle("on", n.dataset.go === current);
    });
  }, true);

  const onKey = (e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
  };
  backdrop.addEventListener("mousedown", (e) => { if (e.target === backdrop) close(); });
  document.addEventListener("keydown", onKey, true);
  const unsub = [prefs.on("terminal", sync), prefs.on("agent", sync)];

  function close() {
    document.removeEventListener("keydown", onKey, true);
    for (const u of unsub) u();
    backdrop.classList.add("leaving");
    setTimeout(() => backdrop.remove(), 140);
  }

  render();
  panel.querySelector("[data-go]").classList.add("on");
  if (ctx.section) panel.querySelector(`[data-section="${ctx.section}"]`)?.scrollIntoView({ block: "start" });
  else panel.querySelector(".st-search input")?.focus();
  return { close };
}

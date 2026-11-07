// Appearance panel: theme presets, color pickers, live contrast scores and
// fonts. Loaded on demand the first time it is opened.

import { PRESETS, DEFAULTS, derive, contrast, applyAppearance, saveAppearance } from "./theme.js";

const esc = (s) => String(s).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** Contrast badge: AAA ≥ 7, AA ≥ 4.5, "large" ≥ 3, otherwise low. */
function grade(ratio) {
  const r = (Math.floor(ratio * 10) / 10).toFixed(1); // round down: never overstate readability
  if (ratio >= 7) return `<span class="ap-grade good">${r}:1 · AAA</span>`;
  if (ratio >= 4.5) return `<span class="ap-grade good">${r}:1 · AA</span>`;
  if (ratio >= 3) return `<span class="ap-grade ok">${r}:1 · large text</span>`;
  return `<span class="ap-grade low">${r}:1 · low</span>`;
}

export function openAppearance({ icons, get, set }) {
  let s = { ...get() };
  const backdrop = document.createElement("div");
  backdrop.id = "appearance-backdrop";
  backdrop.innerHTML = `<div id="appearance" role="dialog" aria-label="Appearance"></div>`;
  document.body.append(backdrop);
  const panel = backdrop.firstElementChild;

  let frame = 0;
  const commit = (patch, { rerender = true } = {}) => {
    s = { ...s, ...patch };
    set(s);
    saveAppearance(s);
    if (!frame) frame = requestAnimationFrame(() => { frame = 0; applyAppearance(s); if (rerender) render(); else renderScores(); });
  };

  const seg = (key, options) =>
    `<div class="seg">${options.map(([v, label]) => `<button data-k="${key}" data-v="${esc(v)}" class="${String(s[key]) === String(v) ? "on" : ""}">${label}</button>`).join("")}</div>`;

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
    // Show where auto contrast had to adjust a picked color.
    const shown = { bg: vars["--bg-editor"], accent: vars["--accent"], text: vars["--fg"] };
    for (const k of ["bg", "accent", "text"]) {
      const code = panel.querySelector(`[data-hex="${k}"]`);
      if (!code) continue;
      if (k === "text" && !s.text) code.textContent = `auto · ${shown.text}`;
      else code.textContent = shown[k].toLowerCase() === s[k].toLowerCase() ? s[k] : `${s[k]} → ${shown[k]}`;
      code.title = code.textContent.includes("→") ? "Adjusted by auto contrast to stay readable" : "";
    }
  }

  function render() {
    const { vars } = derive(s);
    panel.innerHTML = `
      <div class="ap-head">
        <span class="ap-title">Appearance</span>
        <button class="ap-x" data-ap="close" title="Close (Esc)">${icons.close}</button>
      </div>
      <div class="ap-body">
        <section class="ap-section">
          <h3>Theme</h3>
          <div class="ap-presets">
            ${PRESETS.map((p) => {
              const d = derive({ ...s, bg: p.bg, accent: p.accent, text: null }).vars;
              return `<button class="ap-preset${s.preset === p.id ? " on" : ""}" data-preset="${p.id}" style="--pb:${p.bg};--ps:${d["--bg-side"]};--pa:${d["--accent"]};--pf:${d["--fg"]}">
                <span class="ap-swatch"><i></i><b>Aa</b><em></em></span><span class="ap-pname">${esc(p.name)}</span></button>`;
            }).join("")}
          </div>
        </section>

        <section class="ap-section">
          <h3>Colors</h3>
          <div class="ap-colors">
            <label class="ap-color"><input type="color" data-color="accent" value="${s.accent}"><span><b>Accent</b><code data-hex="accent"></code></span></label>
            <label class="ap-color"><input type="color" data-color="bg" value="${s.bg}"><span><b>Background</b><code data-hex="bg"></code></span></label>
            <label class="ap-color"><input type="color" data-color="text" value="${s.text ?? vars["--fg"]}"><span><b>Text</b><code data-hex="text"></code></span></label>
            <button class="ap-auto${s.text ? "" : " on"}" data-ap="text-auto" title="Pick the text color automatically">Auto text</button>
          </div>
          <div class="ap-row"><span>Auto contrast <em>keeps every color readable</em></span>${seg("autoContrast", [[true, "On"], [false, "Off"]])}</div>
          <div class="ap-scores"></div>
        </section>

        <section class="ap-section">
          <h3>Fonts</h3>
          <div class="ap-row"><span>Interface</span>${seg("uiFont", [["jersey", "Jersey"], ["system", "System"]])}</div>
          <div class="ap-row"><span>Editor &amp; terminal</span>${seg("editorFont", [["cascadia", "Cascadia Code"], ["consolas", "Consolas"], ["jersey", "Jersey 15"]])}</div>
          <div class="ap-row"><span>Editor size</span>${seg("editorSize", [12, 13, 14, 15, 16, 18].map((n) => [n, n]))}</div>
          ${s.editorFont === "jersey" ? '<div class="ap-note">Jersey is not monospaced: columns and indentation will not line up exactly.</div>' : ""}
          <div class="ap-scale">
            <div class="ap-hero">FluxCode</div>
            <div class="ap-s-title">Jersey 20 · section titles</div>
            <div class="ap-s-body">Jersey 15 · menus, tabs, explorer and panels</div>
            <div class="ap-s-small">Jersey 10 · status bar and small labels</div>
            <pre class="ap-s-code">def hello(name: str) -&gt; str:\n    return f"Halo, {name}!"</pre>
          </div>
        </section>
      </div>
      <div class="ap-foot">
        <button class="btn-secondary" data-ap="reset">Reset to default</button>
        <button class="btn-primary" data-ap="close">Done</button>
      </div>`;
    renderScores();
  }

  panel.addEventListener("click", (e) => {
    const preset = e.target.closest("[data-preset]");
    if (preset) {
      const p = PRESETS.find((x) => x.id === preset.dataset.preset);
      return commit({ preset: p.id, bg: p.bg, accent: p.accent, text: null });
    }
    const opt = e.target.closest("[data-k]");
    if (opt) {
      const raw = opt.dataset.v;
      const value = raw === "true" ? true : raw === "false" ? false : /^\d+$/.test(raw) ? +raw : raw;
      return commit({ [opt.dataset.k]: value });
    }
    const act = e.target.closest("[data-ap]")?.dataset.ap;
    if (act === "close") close();
    else if (act === "reset") commit({ ...DEFAULTS });
    else if (act === "text-auto") commit({ text: null });
  });

  // Live while dragging in the picker; only the scores re-render (keeps the picker open).
  panel.addEventListener("input", (e) => {
    const key = e.target.dataset.color;
    if (key) commit({ [key]: e.target.value, ...(key === "text" ? {} : { preset: "custom" }) }, { rerender: false });
  });
  panel.addEventListener("change", (e) => {
    if (e.target.dataset.color) render();
  });

  const onKey = (e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
  };
  backdrop.addEventListener("mousedown", (e) => { if (e.target === backdrop) close(); });
  document.addEventListener("keydown", onKey, true);

  function close() {
    document.removeEventListener("keydown", onKey, true);
    backdrop.classList.add("leaving");
    setTimeout(() => backdrop.remove(), 140);
  }

  render();
  panel.querySelector(".ap-preset.on, .ap-preset")?.focus();
}

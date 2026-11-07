// "What's New": FluxCode's release notes, written in its own .flx format
// (see ./notes.js) and shown in a page over the workbench. It opens by itself
// once after every update — and on the very first start — whether a folder is
// open or not, and any time from Help → Release Notes. The same viewer
// previews any .flx (or .md) file from the editor.
//
// Loaded on demand: nothing here is parsed until a page is actually shown.

import { renderFlx } from "./notes.js";

const SEEN_KEY = "fluxcode.notesSeen";
const AUTO_KEY = "fluxcode.notesAuto";

const baseVersion = (v) => (v ?? "").split("-")[0];
const cmpVersion = (a, b) => {
  const pa = baseVersion(a).split(".").map(Number);
  const pb = baseVersion(b).split(".").map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
  return 0;
};
const get = (k) => { try { return localStorage.getItem(k); } catch { return null; } };
const set = (k, v) => { try { localStorage.setItem(k, v); } catch {} };

/** Show the release notes if this version (dev builds included) has not been seen yet. */
export async function maybeShowReleaseNotes(version, ctx) {
  if (!version || get(AUTO_KEY) === "off") return false;
  const seen = get(SEEN_KEY);
  if (seen === version) return false;
  await showReleaseNotes(version, ctx, { since: seen });
  return true;
}

/** Help → Release Notes. `since`: the version used before this update (marks newer sections). */
export async function showReleaseNotes(version, ctx, { since = null } = {}) {
  const { default: text } = await import("../CHANGELOG.flx?raw");
  set(SEEN_KEY, version);
  openNotes(text, { ...ctx, version, since, releaseNotes: true });
}

/** Preview any .flx / .md text (the editor's "Open Preview"). */
export function previewNotes(text, name, ctx) {
  openNotes(text, { ...ctx, fileName: name });
}

function openNotes(text, ctx) {
  const { icons } = ctx;
  const { meta, html } = renderFlx(text);
  document.getElementById("notes-backdrop")?.remove();
  const backdrop = document.createElement("div");
  backdrop.id = "notes-backdrop";
  const title = meta.title ?? ctx.fileName ?? "Notes";
  const sub = ctx.releaseNotes
    ? `Version ${ctx.version}${meta.date ? ` · ${meta.date}` : ""}`
    : [meta.version && `Version ${meta.version}`, meta.date, ctx.fileName].filter(Boolean).join(" · ");
  backdrop.innerHTML = `
    <div id="notes" role="dialog" aria-label="${title.replace(/"/g, "&quot;")}">
      <div class="nt-head">
        <span class="nt-logo">${ctx.logo ?? ""}</span>
        <div class="nt-titles"><div class="nt-title"></div><div class="nt-sub"></div></div>
        ${ctx.releaseNotes ? `<label class="nt-auto"><input type="checkbox"${get(AUTO_KEY) === "off" ? "" : " checked"}> Show after updates</label>` : ""}
        <button class="ap-x" title="Close (Esc)">${icons.close}</button>
      </div>
      <article class="nt-body flx" tabindex="0">${html || '<p class="nt-empty">Nothing here yet.</p>'}</article>
    </div>`;
  backdrop.querySelector(".nt-title").textContent = title;
  backdrop.querySelector(".nt-sub").textContent = sub;
  const body = backdrop.querySelector(".nt-body");

  // Sections for versions newer than the one used before this update get a "New" badge.
  if (ctx.releaseNotes) {
    for (const h of body.querySelectorAll("h2")) {
      const v = h.textContent.match(/^\s*v?(\d+\.\d+(?:\.\d+)?)/)?.[1];
      if (v && (!ctx.since || cmpVersion(v, ctx.since) > 0 || (cmpVersion(v, ctx.version) === 0 && ctx.since !== ctx.version))) {
        h.insertAdjacentHTML("beforeend", ' <span class="nt-new">New</span>');
      }
      if (!ctx.since) break; // first start: only the current release is "new"
    }
  }

  const close = () => {
    document.removeEventListener("keydown", onKey, true);
    backdrop.classList.add("leaving");
    backdrop.addEventListener("animationend", () => backdrop.remove(), { once: true });
    setTimeout(() => backdrop.remove(), 250);
    ctx.onClose?.();
  };
  const onKey = (e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
  };
  document.addEventListener("keydown", onKey, true);
  backdrop.addEventListener("mousedown", (e) => { if (e.target === backdrop) close(); });
  backdrop.querySelector(".ap-x").addEventListener("click", close);
  backdrop.querySelector(".nt-auto input")?.addEventListener("change", (e) => set(AUTO_KEY, e.target.checked ? "on" : "off"));
  body.addEventListener("click", (e) => {
    const a = e.target.closest("a[href]");
    if (!a) return;
    e.preventDefault();
    ctx.openUrl?.(a.getAttribute("href"));
  });
  document.body.append(backdrop);
  body.focus();
}

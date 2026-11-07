// Tiny preference store: one JSON object per namespace in localStorage, with
// subscribers, so the Settings panel, the terminal and the agent all see the
// same values without importing each other.

const DEFAULTS = {
  terminal: { shell: "cmd", fontSize: 14, cursor: "bar", smoothCaret: true, showHidden: false },
  agent: { autoStop: true, autoApprove: false, includeFile: true, mode: "" },
  editor: { autoSave: true, wordWrap: false },
};

const cache = new Map();
const listeners = new Map();

function load(ns) {
  if (!cache.has(ns)) {
    let saved = null;
    try { saved = JSON.parse(localStorage.getItem(`fluxcode.${ns}`) ?? "null"); } catch {}
    cache.set(ns, { ...DEFAULTS[ns], ...(saved && typeof saved === "object" ? saved : {}) });
  }
  return cache.get(ns);
}

export const prefs = {
  defaults: DEFAULTS,
  /** The current values of a namespace (a live object: don't mutate it, use `set`). */
  get: (ns) => load(ns),
  /** Merge `patch` into the namespace, persist it and notify subscribers. */
  set(ns, patch) {
    const next = { ...load(ns), ...patch };
    cache.set(ns, next);
    try { localStorage.setItem(`fluxcode.${ns}`, JSON.stringify(next)); } catch {}
    for (const fn of listeners.get(ns) ?? []) fn(next, patch);
    return next;
  },
  reset(ns) { return this.set(ns, { ...DEFAULTS[ns] }); },
  /** Subscribe to changes of a namespace; returns an unsubscribe function. */
  on(ns, fn) {
    if (!listeners.has(ns)) listeners.set(ns, new Set());
    listeners.get(ns).add(fn);
    return () => listeners.get(ns).delete(fn);
  },
};

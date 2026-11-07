// Packages declared by the open workspace (package.json, requirements.txt,
// pyproject.toml, composer.json), so completions offer them first.
// Filled by main.js when a folder opens; empty when nothing is known.

export const workspace = { npm: [], pypi: [], composer: [] };

const depsOf = (json, keys) => keys.flatMap((k) => Object.keys(json?.[k] ?? {}));

/** Parse the manifests' text (any may be null) into the shared `workspace` lists. */
export function setWorkspaceManifests({ packageJson, requirements, pyproject, composerJson }) {
  workspace.npm = [];
  workspace.pypi = [];
  workspace.composer = [];
  try {
    if (packageJson) workspace.npm = depsOf(JSON.parse(packageJson), ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]);
  } catch {}
  if (requirements) {
    workspace.pypi = requirements.split(/\r?\n/).map((l) => l.replace(/[#;].*$/, "").trim()).filter((l) => l && !l.startsWith("-"))
      .map((l) => /^[A-Za-z0-9_.-]+/.exec(l)?.[0]).filter(Boolean);
  }
  if (pyproject) {
    const m = /dependencies\s*=\s*\[([^\]]*)\]/.exec(pyproject);
    if (m) workspace.pypi.push(...(m[1].match(/"([A-Za-z0-9_.-]+)/g) ?? []).map((x) => x.slice(1)));
  }
  try {
    if (composerJson) workspace.composer = depsOf(JSON.parse(composerJson), ["require", "require-dev"]);
  } catch {}
}

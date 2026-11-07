// JSON: package.json / composer.json keys and package names.

import { workspace } from "./workspace.js";

const PKG_KEYS = ["name", "version", "description", "main", "module", "types", "type", "private", "license", "author", "scripts", "dependencies", "devDependencies", "peerDependencies", "optionalDependencies", "engines", "bin", "files", "repository", "keywords", "homepage", "bugs", "browserslist", "workspaces", "exports", "imports", "sideEffects", "packageManager", "publishConfig"];
const COMPOSER_KEYS = ["name", "description", "type", "license", "require", "require-dev", "autoload", "autoload-dev", "scripts", "config", "extra", "minimum-stability", "prefer-stable", "keywords", "authors", "repositories"];
const SCRIPT_KEYS = ["dev", "build", "start", "test", "lint", "format", "preview", "serve", "watch", "clean", "prepare", "postinstall"];

let lists = null;

/** `file` is the lower-cased file name; sources are only added for the manifests. */
export function jsonCompletion(file) {
  const isPkg = file === "package.json";
  const isComposer = file === "composer.json";
  if (!isPkg && !isComposer) return [];
  return [
    async (context) => {
      const key = context.matchBefore(/"([^"]*)$/);
      if (!key) return null;
      // Which object are we in? Walk back to the nearest unclosed `"name": {`.
      const before = context.state.sliceDoc(0, key.from);
      const stack = [];
      for (const m of before.matchAll(/"([^"\\]*)"\s*:\s*\{|\{|\}/g)) {
        if (m[0] === "}") stack.pop();
        else stack.push(m[1] ?? null);
      }
      const parent = stack.at(-1);
      const from = key.from + 1;
      let options;
      if (parent === "dependencies" || parent === "devDependencies" || parent === "peerDependencies" || parent === "optionalDependencies") {
        lists ??= await import("./javascript.js");
        options = [...new Set([...workspace.npm, ...lists.NPM_POPULAR])].map((n) => ({ label: n, type: "module", detail: "npm", apply: `${n}": "^` }));
      } else if (parent === "require" || parent === "require-dev") {
        const php = await import("./php.js");
        options = [...new Set([...workspace.composer, ...php.COMPOSER_POPULAR])].map((n) => ({ label: n, type: "module", detail: "packagist", apply: `${n}": "^` }));
      } else if (parent === "scripts") options = SCRIPT_KEYS.map((k) => ({ label: k, type: "property", apply: `${k}": "` }));
      else if (parent === null && stack.length === 1) options = (isPkg ? PKG_KEYS : COMPOSER_KEYS).map((k) => ({ label: k, type: "property", apply: `${k}": ` }));
      else return null;
      return { from, options, validFor: /^[\w@./-]*$/ };
    },
  ];
}

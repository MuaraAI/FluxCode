// .flx — FluxCode's own notes format, used for the release notes
// (CHANGELOG.flx). It reads like Markdown's everyday subset plus a few
// FluxCode extras, and this small renderer turns it into HTML with no library:
//
//   @title / @version / @date    directives at the top of the file
//   # Heading  ## Sub  ### Minor
//   - item  * item  1. item      lists
//   - [x] done  - [ ] todo       task items
//   - !new / !fix / !better / !breaking text    tagged items (colored badges)
//   > note    >! warning         call-outs
//   ``` … ```                    code block
//   ---                          rule
//   **bold**  *italic*  `code`  [text](https://…)  [[Ctrl+Shift+P]] (key caps)
//
// Everything is escaped first, so a .flx file can never inject HTML.

const esc = (s) => s.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);

const TAGS = { new: "New", fix: "Fix", better: "Improved", improved: "Improved", breaking: "Breaking", perf: "Faster" };

function inline(s) {
  return esc(s)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\[\[([^\]]+)\]\]/g, (_, keys) => keys.split("+").map((k) => `<kbd>${k.trim()}</kbd>`).join(""))
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/(^|[\s(])\*([^*\s][^*]*)\*(?=[\s).,;:!?]|$)/g, "$1<i>$2</i>")
    .replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<a href="$2">$1</a>');
}

function listItem(text) {
  const task = text.match(/^\[([ xX])\]\s+(.*)$/);
  if (task) return `<li class="task${task[1] === " " ? "" : " done"}"><span class="cb"></span>${inline(task[2])}</li>`;
  const tag = text.match(/^!(\w+)\s+(.*)$/);
  if (tag && TAGS[tag[1].toLowerCase()]) {
    const k = tag[1].toLowerCase();
    return `<li><span class="tag tag-${k}">${TAGS[k]}</span>${inline(tag[2])}</li>`;
  }
  return `<li>${inline(text)}</li>`;
}

/** `{ meta, html }` for a .flx document. */
export function renderFlx(text) {
  const meta = {};
  const out = [];
  let list = null; // { tag: "ul"|"ol", items: [] }
  let code = null;
  let para = [];
  const flushPara = () => { if (para.length) { out.push(`<p>${inline(para.join(" "))}</p>`); para = []; } };
  const flushList = () => { if (list) { out.push(`<${list.tag}>${list.items.join("")}</${list.tag}>`); list = null; } };
  const flushAll = () => { flushPara(); flushList(); };
  for (const raw of text.split(/\r?\n/)) {
    if (code) {
      if (raw.startsWith("```")) { out.push(`<pre>${esc(code.join("\n"))}</pre>`); code = null; } else code.push(raw);
      continue;
    }
    const line = raw.trimEnd();
    const directive = line.match(/^@(\w+)\s+(.*)$/);
    if (directive && !out.length && !para.length) { meta[directive[1]] = directive[2].trim(); continue; }
    if (line.startsWith("```")) { flushAll(); code = []; continue; }
    if (!line.trim()) { flushAll(); continue; }
    const heading = line.match(/^(#{1,3})\s+(.*)$/);
    if (heading) { flushAll(); const level = heading[1].length + 1; out.push(`<h${level}>${inline(heading[2])}</h${level}>`); continue; }
    const item = line.match(/^\s*(?:([-*])|(\d+)[.)])\s+(.*)$/);
    if (item) {
      flushPara();
      const tag = item[2] ? "ol" : "ul";
      if (list && list.tag !== tag) flushList();
      (list ??= { tag, items: [] }).items.push(listItem(item[3]));
      continue;
    }
    if (line.startsWith(">! ")) { flushAll(); out.push(`<blockquote class="warn">${inline(line.slice(3))}</blockquote>`); continue; }
    if (line.startsWith("> ")) { flushAll(); out.push(`<blockquote>${inline(line.slice(2))}</blockquote>`); continue; }
    if (line === "---") { flushAll(); out.push("<hr>"); continue; }
    flushList();
    para.push(line.trim());
  }
  flushAll();
  if (code) out.push(`<pre>${esc(code.join("\n"))}</pre>`);
  return { meta, html: out.join("") };
}

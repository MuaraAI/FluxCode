// A small Emmet: HTML abbreviations (`!`, `ul>li*3>a`, `div.card#main`,
// `input:email`, `lorem`) and CSS abbreviations (`m10`, `df`, `posa`, `bgc#fff`).
// Expansions are CodeMirror snippet templates: `${}` marks a tab stop and
// `\t` an indent level, so Tab walks through the result like in VS Code.
// Written for FluxCode; nothing is loaded for languages that don't use it.

// ------------------------------------------------------------------ HTML

const VOID = new Set(["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"]);
const INLINE = new Set(["a", "abbr", "b", "bdi", "bdo", "br", "button", "cite", "code", "data", "dfn", "em", "i", "img", "input", "kbd", "label", "mark", "q", "s", "samp", "select", "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr", "textarea", "option"]);
// Children get their tag from the parent when the abbreviation gives none (`.item` inside `ul` → li).
const CHILD_OF = { ul: "li", ol: "li", table: "tr", tbody: "tr", thead: "tr", tfoot: "tr", tr: "td", select: "option", optgroup: "option", dl: "dt", datalist: "option", picture: "source", audio: "source", video: "source", map: "area", nav: "a" };
const INLINE_PARENT = new Set(["p", "span", "a", "b", "i", "strong", "em", "label", "button", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th", "dt", "dd", "legend", "figcaption", "summary", "caption", "q", "small", "cite", "code"]);
// Default attributes (Emmet's). Empty string = a tab stop.
const ATTRS = {
  a: [["href", ""]], abbr: [["title", ""]], img: [["src", ""], ["alt", ""]], link: [["rel", "stylesheet"], ["href", ""]],
  script: [], style: [], form: [["action", ""]], label: [["for", ""]], iframe: [["src", ""], ["frameborder", "0"]],
  input: [["type", "text"]], select: [["name", ""], ["id", ""]], option: [["value", ""]], textarea: [["name", ""], ["id", ""], ["cols", "30"], ["rows", "10"]],
  button: [], video: [["src", ""]], audio: [["src", ""]], source: [["src", ""], ["type", ""]], embed: [["src", ""], ["type", ""]],
  object: [["data", ""], ["type", ""]], area: [["shape", ""], ["coords", ""], ["href", ""], ["alt", ""]], meta: [], base: [["href", ""]],
  canvas: [["id", ""]], svg: [["xmlns", "http://www.w3.org/2000/svg"], ["viewBox", "0 0 24 24"]], dialog: [], details: [], template: [],
};
// Aliases: name → [tag, extra attrs]
const ALIAS = {
  "a:link": ["a", [["href", "http://"]]], "a:mail": ["a", [["href", "mailto:"]]], "a:tel": ["a", [["href", "tel:+"]]],
  "link:css": ["link", [["rel", "stylesheet"], ["href", "style.css"]]], "link:favicon": ["link", [["rel", "shortcut icon"], ["type", "image/x-icon"], ["href", "favicon.ico"]]],
  "link:font": ["link", [["rel", "stylesheet"], ["href", "https://fonts.googleapis.com/css2?family="]]],
  "script:src": ["script", [["src", ""]]], "script:module": ["script", [["type", "module"], ["src", ""]]],
  "meta:vp": ["meta", [["name", "viewport"], ["content", "width=device-width, initial-scale=1.0"]]], "meta:utf": ["meta", [["charset", "UTF-8"]]],
  "meta:desc": ["meta", [["name", "description"], ["content", ""]]], "meta:kw": ["meta", [["name", "keywords"], ["content", ""]]],
  "form:get": ["form", [["action", ""], ["method", "get"]]], "form:post": ["form", [["action", ""], ["method", "post"]]],
  "input:t": ["input", [["type", "text"], ["name", ""], ["id", ""]]], "input:text": ["input", [["type", "text"], ["name", ""], ["id", ""]]],
  "input:email": ["input", [["type", "email"], ["name", ""], ["id", ""]]], "input:password": ["input", [["type", "password"], ["name", ""], ["id", ""]]],
  "input:p": ["input", [["type", "password"], ["name", ""], ["id", ""]]], "input:number": ["input", [["type", "number"], ["name", ""], ["id", ""]]],
  "input:date": ["input", [["type", "date"], ["name", ""], ["id", ""]]], "input:time": ["input", [["type", "time"], ["name", ""], ["id", ""]]],
  "input:file": ["input", [["type", "file"], ["name", ""], ["id", ""]]], "input:hidden": ["input", [["type", "hidden"], ["name", ""]]],
  "input:h": ["input", [["type", "hidden"], ["name", ""]]], "input:checkbox": ["input", [["type", "checkbox"], ["name", ""], ["id", ""]]],
  "input:c": ["input", [["type", "checkbox"], ["name", ""], ["id", ""]]], "input:radio": ["input", [["type", "radio"], ["name", ""], ["id", ""]]],
  "input:r": ["input", [["type", "radio"], ["name", ""], ["id", ""]]], "input:range": ["input", [["type", "range"], ["name", ""], ["id", ""]]],
  "input:search": ["input", [["type", "search"], ["name", ""], ["id", ""]]], "input:url": ["input", [["type", "url"], ["name", ""], ["id", ""]]],
  "input:tel": ["input", [["type", "tel"], ["name", ""], ["id", ""]]], "input:color": ["input", [["type", "color"], ["name", ""], ["id", ""]]],
  "input:submit": ["input", [["type", "submit"], ["value", ""]]], "input:s": ["input", [["type", "submit"], ["value", ""]]],
  "input:button": ["input", [["type", "button"], ["value", ""]]], "input:b": ["input", [["type", "button"], ["value", ""]]],
  "input:reset": ["input", [["type", "reset"], ["value", ""]]],
  btn: ["button", []], "btn:s": ["button", [["type", "submit"]]], "btn:r": ["button", [["type", "reset"]]], "btn:b": ["button", [["type", "button"]]],
  inp: ["input", [["type", "text"], ["name", ""], ["id", ""]]], sel: ["select", [["name", ""], ["id", ""]]], opt: ["option", [["value", ""]]],
  tarea: ["textarea", [["name", ""], ["id", ""], ["cols", "30"], ["rows", "10"]]], "img:s": ["img", [["srcset", ""], ["src", ""], ["alt", ""]]],
  "img:z": ["img", [["sizes", ""], ["srcset", ""], ["src", ""], ["alt", ""]]], "video:src": ["video", [["src", ""]]], "audio:src": ["audio", [["src", ""]]],
  bq: ["blockquote", []], fig: ["figure", []], figc: ["figcaption", []], pic: ["picture", []], ifr: ["iframe", [["src", ""], ["frameborder", "0"]]],
  emb: ["embed", [["src", ""], ["type", ""]]], obj: ["object", [["data", ""], ["type", ""]]], cap: ["caption", []], colg: ["colgroup", []],
  fst: ["fieldset", []], fset: ["fieldset", []], leg: ["legend", []], optg: ["optgroup", []], tbl: ["table", []], hdr: ["header", []],
  ftr: ["footer", []], adr: ["address", []], dlg: ["dialog", []], str: ["strong", []], prog: ["progress", []], mn: ["main", []],
  sect: ["section", []], art: ["article", []], tem: ["template", []], datal: ["datalist", [["id", ""]]], det: ["details", []], summ: ["summary", []],
};

const BOILERPLATE = `<!DOCTYPE html>
<html lang="en">
<head>
\t<meta charset="UTF-8">
\t<meta name="viewport" content="width=device-width, initial-scale=1.0">
\t<title>\${Document}</title>
</head>
<body>
\t\${}
</body>
</html>`;

const LOREM = "Lorem ipsum dolor sit amet consectetur adipisicing elit. Quisquam voluptates, nam ipsum eos voluptas dolorem ut sapiente molestiae architecto necessitatibus incidunt ea dolore facere cumque obcaecati laudantium totam ipsa. Maxime, delectus voluptatum repellat tempora fugit minima dolorum vel earum explicabo iste quae aliquid quo nesciunt itaque mollitia soluta omnis nihil natus culpa alias".split(" ");
const lorem = (n) => {
  const words = LOREM.slice(0, Math.max(1, Math.min(n, LOREM.length)));
  return words.join(" ").replace(/,$/, "") + ".";
};

/** Escape snippet syntax in literal text. */
const lit = (s) => s.replace(/[\\${}]/g, (c) => `\\${c}`);

const RE_NAME = /^[a-zA-Z][\w:-]*/;

/** Parse an abbreviation into a tree (siblings at the top). Returns null if it isn't one. */
function parse(src) {
  let i = 0;
  const err = () => { throw new Error("bad abbreviation"); };
  function element() {
    const node = { name: "", id: null, classes: [], attrs: [], text: null, mult: 1, children: [] };
    const m = RE_NAME.exec(src.slice(i));
    if (m) { node.name = m[0]; i += m[0].length; }
    let any = !!m;
    for (;;) {
      const c = src[i];
      if (c === "#") {
        const mm = /^#([\w$@-]+)/.exec(src.slice(i)) ?? err();
        node.id = mm[1]; i += mm[0].length; any = true;
      } else if (c === ".") {
        const mm = /^\.([\w$@-]+)/.exec(src.slice(i)) ?? err();
        node.classes.push(mm[1]); i += mm[0].length; any = true;
      } else if (c === "[") {
        const end = src.indexOf("]", i);
        if (end < 0) err();
        for (const a of src.slice(i + 1, end).match(/[\w:@.-]+(?:="[^"]*"|='[^']*'|=[^\s\]]*)?/g) ?? []) {
          const eq = a.indexOf("=");
          node.attrs.push(eq < 0 ? [a, ""] : [a.slice(0, eq), a.slice(eq + 1).replace(/^(["'])(.*)\1$/, "$2")]);
        }
        i = end + 1; any = true;
      } else if (c === "{") {
        let depth = 0, j = i;
        for (; j < src.length; j++) { if (src[j] === "{") depth++; else if (src[j] === "}" && --depth === 0) break; }
        if (j >= src.length) err();
        node.text = src.slice(i + 1, j); i = j + 1; any = true;
      } else if (c === "*") {
        const mm = /^\*(\d+)/.exec(src.slice(i)) ?? err();
        node.mult = Math.min(+mm[1], 200); i += mm[0].length;
      } else break;
    }
    return any ? node : null;
  }
  function group() {
    const out = [];
    for (;;) {
      let node;
      if (src[i] === "(") {
        i++;
        node = { group: true, mult: 1, children: group() };
        if (src[i] !== ")") err();
        i++;
        const mm = /^\*(\d+)/.exec(src.slice(i));
        if (mm) { node.mult = Math.min(+mm[1], 200); i += mm[0].length; }
      } else {
        node = element();
        if (!node) return out;
      }
      out.push(node);
      if (src[i] === ">") { i++; node.children = group(); }
      if (src[i] === "+") { i++; continue; }
      if (src[i] === "^") { i++; return out; }
      if (src[i] === ")" || i >= src.length) return out;
    }
  }
  try {
    const nodes = group();
    return i === src.length && nodes.length ? nodes : null;
  } catch { return null; }
}

const numbered = (s, n, total) => s.replace(/\$@-?\d*|\$+/g, (m) => {
  if (m.startsWith("$@")) { const base = parseInt(m.slice(2) || "1", 10); return String(m[2] === "-" ? total - n + 1 : base + n - 1); }
  return String(n).padStart(m.length, "0");
});

/** Render parsed nodes to snippet lines. */
function render(nodes, parent, total = 1, n = 1) {
  const lines = [];
  for (const node of nodes) {
    for (let k = 1; k <= node.mult; k++) {
      if (node.group) { lines.push(...render(node.children, parent, node.mult, k)); continue; }
      const idx = node.mult > 1 ? k : n;
      const cnt = node.mult > 1 ? node.mult : total;
      let name = node.name;
      let attrs = [];
      let text = node.text;
      if (/^lorem(\d*)$/.test(name)) { text = lorem(+RegExp.$1 || 30); name = ""; }
      const alias = ALIAS[name];
      if (alias) { name = alias[0]; attrs = [...alias[1]]; }
      if (!name) name = parent && CHILD_OF[parent] ? CHILD_OF[parent] : parent && INLINE_PARENT.has(parent) ? "span" : "div";
      if (!alias && ATTRS[name]) attrs = [...ATTRS[name]];
      if (node.id) attrs.push(["id", numbered(node.id, idx, cnt)]);
      if (node.classes.length) attrs.push(["class", node.classes.map((c) => numbered(c, idx, cnt)).join(" ")]);
      for (const [k, v] of node.attrs) {
        const at = attrs.findIndex((a) => a[0] === k);
        if (at >= 0) attrs[at] = [k, v]; else attrs.push([k, v]);
      }
      const attrStr = attrs.map(([k, v]) => ` ${k}="${v === "" ? "${}" : lit(numbered(v, idx, cnt))}"`).join("");
      const open = `<${name}${attrStr}>`;
      if (VOID.has(name)) { lines.push(open); continue; }
      const kids = node.children.length ? render(node.children, name, cnt, idx) : [];
      const body = text != null ? lit(numbered(text, idx, cnt)) : "";
      const inlineKids = kids.length && kids.every((l) => !l.includes("\n") && !l.startsWith("\t")) && node.children.every((c) => INLINE.has(c.name || CHILD_OF[name] || "") || c.group);
      if (!kids.length) lines.push(`${open}${body || "${}"}</${name}>`);
      else if (inlineKids && kids.length === node.children.length) lines.push(`${open}${body}${kids.join("")}</${name}>`);
      else lines.push(open, ...(body ? [`\t${body}`] : []), ...kids.map((l) => `\t${l}`), `</${name}>`);
    }
  }
  return lines;
}

/** Expand an HTML abbreviation to a snippet template, or null. */
export function expandHtml(abbr) {
  if (!abbr) return null;
  if (abbr === "!" || abbr === "!!!" || abbr === "html:5" || abbr === "doc") return BOILERPLATE;
  if (abbr === "!!!4") return '<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01//EN" "http://www.w3.org/TR/html4/strict.dtd">';
  if (/^lorem\d*$/.test(abbr)) return lit(lorem(+abbr.slice(5) || 30));
  if (abbr.includes("\n") || /^[\d.]+$/.test(abbr)) return null;
  const nodes = parse(abbr);
  if (!nodes) return null;
  const text = render(nodes, null).join("\n");
  return text.includes("${") ? text : `${text}\${}`;
}

/** Does the abbreviation start with something that could be an element (so "test" is not expanded silently)? */
export function looksLikeHtmlAbbr(abbr, known) {
  if (!abbr) return false;
  if (abbr === "!" || abbr === "!!!" || abbr.startsWith("html:") || /^lorem\d*$/.test(abbr)) return true;
  const first = /^[a-zA-Z][\w:-]*/.exec(abbr)?.[0];
  if (!first) return /^[.#(]/.test(abbr) && !!parse(abbr); // ".card" / "#app" → div, "(a+b)" groups
  const base = first.split(":")[0];
  return known.has(base) || !!ALIAS[first] || !!ALIAS[base] || /[>+*.#\[{^(]/.test(abbr.slice(first.length));
}

// ------------------------------------------------------------------- CSS

// Emmet's CSS snippets (the common ones). "prop" alone takes a value; "prop:val" is complete.
const CSS = {
  pos: "position", posa: "position:absolute", posr: "position:relative", posf: "position:fixed", poss: "position:sticky",
  t: "top", r: "right", b: "bottom", l: "left", z: "z-index", zi: "z-index", fl: "float", fll: "float:left", flr: "float:right", cl: "clear",
  d: "display", db: "display:block", di: "display:inline", dib: "display:inline-block", df: "display:flex", dif: "display:inline-flex", dg: "display:grid", dn: "display:none", dc: "display:contents",
  v: "visibility", vh: "visibility:hidden", vv: "visibility:visible", ov: "overflow", ovh: "overflow:hidden", ova: "overflow:auto", ovs: "overflow:scroll", ovv: "overflow:visible", ovx: "overflow-x", ovy: "overflow-y",
  bxz: "box-sizing", bxzb: "box-sizing:border-box", bxsh: "box-shadow", bxshn: "box-shadow:none",
  m: "margin", mt: "margin-top", mr: "margin-right", mb: "margin-bottom", ml: "margin-left", ma: "margin:auto", "m:a": "margin:auto", mx: "margin-inline", my: "margin-block",
  p: "padding", pt: "padding-top", pr: "padding-right", pb: "padding-bottom", pl: "padding-left", px: "padding-inline", py: "padding-block",
  w: "width", h: "height", maw: "max-width", mah: "max-height", miw: "min-width", mih: "min-height", "w:a": "width:auto", "h:a": "height:auto", mawn: "max-width:none",
  o: "outline", oln: "outline:none", ol: "outline", olc: "outline-color", olw: "outline-width", ols: "outline-style", olo: "outline-offset",
  bd: "border", bdn: "border:none", bdt: "border-top", bdr: "border-right", bdb: "border-bottom", bdl: "border-left", bdc: "border-color", bdw: "border-width", bds: "border-style", bdrs: "border-radius", bdcl: "border-collapse", bdclc: "border-collapse:collapse",
  bg: "background", bgc: "background-color", bgi: "background-image", bgr: "background-repeat", bgrn: "background-repeat:no-repeat", bgp: "background-position", bgs: "background-size", bgsc: "background-size:cover", bgsct: "background-size:contain", bgn: "background:none", bga: "background-attachment",
  c: "color", op: "opacity", ff: "font-family", fz: "font-size", fw: "font-weight", fwb: "font-weight:bold", fwn: "font-weight:normal", fs: "font-style", fsi: "font-style:italic", fsn: "font-style:normal", f: "font", lh: "line-height", ls: "letter-spacing",
  ta: "text-align", tal: "text-align:left", tac: "text-align:center", tar: "text-align:right", taj: "text-align:justify", td: "text-decoration", tdn: "text-decoration:none", tdu: "text-decoration:underline", tdl: "text-decoration:line-through",
  tt: "text-transform", ttu: "text-transform:uppercase", ttl: "text-transform:lowercase", ttc: "text-transform:capitalize", ttn: "text-transform:none", ti: "text-indent", tsh: "text-shadow", tov: "text-overflow", tove: "text-overflow:ellipsis",
  ws: "white-space", wsn: "white-space:nowrap", wsp: "white-space:pre", wspw: "white-space:pre-wrap", wob: "word-break", wobba: "word-break:break-all", wow: "overflow-wrap", va: "vertical-align", vam: "vertical-align:middle", vat: "vertical-align:top", vab: "vertical-align:bottom",
  cur: "cursor", curp: "cursor:pointer", curd: "cursor:default", cura: "cursor:auto", curt: "cursor:text", curm: "cursor:move", curna: "cursor:not-allowed",
  lis: "list-style", lisn: "list-style:none", list: "list-style-type", lisp: "list-style-position",
  fx: "flex", fxd: "flex-direction", fxdc: "flex-direction:column", fxdr: "flex-direction:row", fxw: "flex-wrap", fxww: "flex-wrap:wrap", fxwn: "flex-wrap:nowrap", fxg: "flex-grow", fxs: "flex-shrink", fxb: "flex-basis", fxf: "flex-flow",
  jc: "justify-content", jcc: "justify-content:center", jcsb: "justify-content:space-between", jcsa: "justify-content:space-around", jcse: "justify-content:space-evenly", jcfs: "justify-content:flex-start", jcfe: "justify-content:flex-end",
  ai: "align-items", aic: "align-items:center", aifs: "align-items:flex-start", aife: "align-items:flex-end", ais: "align-items:stretch", aib: "align-items:baseline", as: "align-self", asc: "align-self:center", ac: "align-content", acc: "align-content:center",
  ji: "justify-items", jic: "justify-items:center", js: "justify-self", jsc: "justify-self:center", pi: "place-items", pic: "place-items:center", pc: "place-content", pcc: "place-content:center",
  g: "gap", gp: "gap", rg: "row-gap", cg: "column-gap", gtc: "grid-template-columns", gtr: "grid-template-rows", gta: "grid-template-areas", ga: "grid-area", gc: "grid-column", gr: "grid-row", gaf: "grid-auto-flow", gac: "grid-auto-columns", gar: "grid-auto-rows", ord: "order",
  trs: "transition", trf: "transform", trfo: "transform-origin", anim: "animation", animn: "animation-name", animdur: "animation-duration", animdel: "animation-delay", animtf: "animation-timing-function", animic: "animation-iteration-count",
  us: "user-select", usn: "user-select:none", pe: "pointer-events", pen: "pointer-events:none", pea: "pointer-events:auto", rsz: "resize", rszn: "resize:none", rszv: "resize:vertical",
  cnt: "content", ct: "content", ctn: "content:none", cte: "content:''", q: "quotes", cm: "/* ${} */", "@m": "@media (${}) {\n\t${}\n}", "@media": "@media (${}) {\n\t${}\n}", "@kf": "@keyframes ${name} {\n\t${}\n}",
  "@i": "@import url(${});", "@f": "@font-face {\n\tfont-family: ${};\n\tsrc: url(${});\n}", colm: "columns", colmc: "column-count", colmg: "column-gap", obf: "object-fit", obfc: "object-fit:cover", obfct: "object-fit:contain", obp: "object-position",
  ar: "aspect-ratio", scb: "scroll-behavior", scbs: "scroll-behavior:smooth", ssp: "scroll-snap-type", fil: "filter", bdf: "backdrop-filter", mx2: "mix-blend-mode", isn: "isolation:isolate", wfc: "will-change", wc: "will-change", cnts: "contain",
  tabs: "tab-size", hy: "hyphens", acc2: "accent-color", csc: "caret-color", ins: "inset", ins0: "inset:0", tp: "text-wrap", tpb: "text-wrap:balance", tpp: "text-wrap:pretty",
};
const CSS_KEYS = Object.keys(CSS).sort((a, b) => b.length - a.length);
const UNITLESS = new Set(["line-height", "opacity", "z-index", "font-weight", "flex", "flex-grow", "flex-shrink", "order", "zoom", "column-count", "columns", "aspect-ratio", "tab-size", "scale"]);
const KEYWORDS = {
  a: "auto", n: "none", i: "inherit", ini: "initial", u: "unset", c: "center", b: "block", f: "flex", g: "grid", r: "relative", ab: "absolute", fx: "fixed", s: "solid", d: "dashed", dt: "dotted", h: "hidden", v: "visible", t: "transparent", cc: "currentColor", e: "ease", ei: "ease-in", eo: "ease-out", eio: "ease-in-out", l: "linear", nw: "nowrap", w: "wrap", sb: "space-between", sa: "space-around", fs: "flex-start", fe: "flex-end", st: "stretch", cv: "cover", ct: "contain", p: "pointer",
};

function cssValue(prop, raw) {
  if (!raw) return null;
  const parts = [];
  // "-" separates values ("m10-20"); it is a sign only at the start or after another "-" ("m-10", "m10--20").
  for (const part of raw.split(/(?<=[\da-z%#.])-(?=[\d.#a-z-])/i)) {
    if (!part) return null;
    if (/^-?\d*\.?\d+/.test(part)) {
      const m = /^(-?\d*\.?\d+)([a-z%]*)$/.exec(part);
      if (!m) return null;
      const [, num, u] = m;
      const unit = u === "" ? (UNITLESS.has(prop) || num === "0" ? "" : "px") : u === "p" ? "%" : u === "e" ? "em" : u === "r" ? "rem" : u === "x" ? "ex" : u;
      parts.push(num + unit);
    } else if (/^#[0-9a-fA-F]{1,8}$/.test(part)) {
      const hex = part.slice(1);
      parts.push(`#${hex.length === 1 ? hex.repeat(6) : hex.length === 2 ? hex.repeat(3) : hex}`);
    } else if (/^[a-zA-Z-]+$/.test(part)) parts.push(KEYWORDS[part] ?? part);
    else return null;
  }
  return parts.join(" ") || null;
}

/** Expand a CSS abbreviation (`m10-20`, `df`, `bgc#fff`, `p:1.5r!`) to a declaration, or null. */
export function expandCss(abbr) {
  if (!abbr || /\s/.test(abbr)) return null;
  const important = abbr.endsWith("!");
  if (important) abbr = abbr.slice(0, -1);
  if (!abbr) return null;
  const bang = important ? " !important" : "";
  // Exact snippet (possibly with a value: "prop:val").
  const exact = CSS[abbr];
  if (exact) {
    if (exact.includes("${") || exact.startsWith("@") || exact.startsWith("/*")) return exact;
    const [prop, val] = exact.split(":");
    return val ? `${prop}: ${val}${bang};\${}` : `${prop}: \${}${bang};`;
  }
  // A full property name typed out: "margin-top10", "line-height1.5".
  let m = /^([a-z]+(?:-[a-z]+)+)(-?[\d.#].*)$/.exec(abbr);
  if (m) {
    const val = cssValue(m[1], m[2]);
    if (val) return `${m[1]}: ${val}${bang};\${}`;
  }
  // Abbreviation + value: "m10", "fz1.2r", "bgc#fff", "w100p", "p10-20", "trs:all".
  if (/^[a-z]/i.test(abbr)) {
    for (const key of CSS_KEYS) {
      if (!abbr.startsWith(key)) continue;
      const explicit = abbr[key.length] === ":";
      const rest = abbr.slice(key.length).replace(/^:/, "");
      if (!rest || (!explicit && !/^[-\d.#]/.test(rest))) continue;
      const base = CSS[key].split(":")[0];
      if (base.startsWith("@") || base.includes("${")) continue;
      const val = cssValue(base, rest);
      if (val) return `${base}: ${val}${bang};\${}`;
    }
  }
  return null;
}

/** Human-readable preview of a snippet (fields removed). */
export const previewOf = (tpl) => tpl.replace(/\$\{([^}]*)\}/g, "$1").replace(/\\([\\${}])/g, "$1").replace(/\t/g, "  ");

export const cssAbbreviations = CSS;

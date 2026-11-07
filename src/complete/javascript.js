// JavaScript / TypeScript: browser globals (from the WebView itself, so every
// DOM API is covered), Node.js globals and modules, npm packages (workspace
// dependencies first) and snippets. Local variables and the standard
// snippets come from @codemirror/lang-javascript.

import { snippetCompletion as snip } from "@codemirror/autocomplete";
import { scopeCompletionSource } from "@codemirror/lang-javascript";
import { workspace } from "./workspace.js";

const words = (s) => s.trim().split(/\s+/);

export const NODE_MODULES = words(`
  fs fs/promises path os http https http2 url crypto events stream stream/promises child_process util readline readline/promises zlib net dns dns/promises
  buffer assert assert/strict worker_threads cluster dgram timers timers/promises perf_hooks async_hooks querystring string_decoder tty v8 vm module
  process test sqlite inspector diagnostics_channel punycode repl tls trace_events wasi`);

export const NPM_POPULAR = words(`
  express react react-dom vue next nuxt svelte @sveltejs/kit @angular/core rxjs axios lodash lodash-es underscore dayjs moment luxon date-fns chalk commander yargs
  dotenv cors body-parser cookie-parser express-session helmet morgan compression multer passport passport-local jsonwebtoken bcrypt bcryptjs uuid nanoid zod joi yup validator
  mongoose mongodb sequelize prisma @prisma/client drizzle-orm knex typeorm pg mysql2 sqlite3 better-sqlite3 redis ioredis socket.io socket.io-client ws
  typescript vite webpack esbuild rollup @vitejs/plugin-react @vitejs/plugin-vue jest vitest mocha chai supertest eslint prettier nodemon ts-node tsx concurrently cross-env
  tailwindcss postcss autoprefixer sass bootstrap @mui/material @emotion/react @emotion/styled antd styled-components framer-motion lucide-react react-icons clsx classnames
  @headlessui/react @radix-ui/react-dialog @radix-ui/react-dropdown-menu @tanstack/react-query @tanstack/react-table swr zustand redux @reduxjs/toolkit react-redux react-router-dom react-hook-form
  vue-router pinia vuex @vueuse/core graphql @apollo/client apollo-server fastify koa hono nest @nestjs/common @nestjs/core
  fs-extra glob fast-glob minimist inquirer ora puppeteer playwright cheerio node-fetch got undici sharp jimp nodemailer stripe firebase firebase-admin @supabase/supabase-js
  openai @anthropic-ai/sdk @google/generative-ai langchain @langchain/core electron @tauri-apps/api three d3 chart.js react-chartjs-2 recharts leaflet mapbox-gl
  marked highlight.js prismjs dompurify immer rimraf mkdirp semver xml2js csv-parse papaparse xlsx exceljs pdfkit jspdf html2canvas qrcode ejs pug handlebars nunjucks
  i18next react-i18next vue-i18n alpinejs htmx.org jquery @popperjs/core @floating-ui/dom swiper gsap animejs lottie-web pixi.js phaser matter-js
  @types/node @types/react @types/react-dom @types/express next-auth @auth/core bullmq p-limit p-queue p-map execa shelljs yaml js-yaml toml ini
  winston pino debug lru-cache node-cache dotenv-expand envalid open update-notifier boxen figlet gradient-string cli-table3 progress listr2
  socket.io-client peerjs simple-peer mqtt amqplib kafkajs @aws-sdk/client-s3 aws-sdk @azure/storage-blob @google-cloud/storage cloudinary
  react-native expo @react-navigation/native react-native-reanimated nativewind formik final-form @hookform/resolvers react-select react-datepicker react-toastify sonner
  vue-toastification vuetify quasar primevue element-plus naive-ui @nuxt/content @nuxtjs/tailwindcss astro @astrojs/react solid-js qwik lit preact htm`);

const NODE_GLOBALS = [
  { label: "process", type: "variable", detail: "Node.js" }, { label: "require", type: "function", detail: "Node.js" }, { label: "module", type: "variable", detail: "Node.js" },
  { label: "exports", type: "variable", detail: "Node.js" }, { label: "__dirname", type: "variable", detail: "Node.js" }, { label: "__filename", type: "variable", detail: "Node.js" },
  { label: "Buffer", type: "class", detail: "Node.js" }, { label: "global", type: "variable", detail: "Node.js" }, { label: "setImmediate", type: "function", detail: "Node.js" },
  { label: "clearImmediate", type: "function", detail: "Node.js" }, { label: "import.meta", type: "variable", detail: "ES modules" },
];

const SNIPPETS = [
  snip("console.log(${});", { label: "log", detail: "console.log", type: "snippet" }),
  snip("console.error(${});", { label: "error", detail: "console.error", type: "snippet" }),
  snip("const ${name} = ${value};", { label: "const", detail: "declaration", type: "snippet" }),
  snip("let ${name} = ${value};", { label: "let", detail: "declaration", type: "snippet" }),
  snip("const ${fn} = (${params}) => {\n\t${}\n};", { label: "arrow", detail: "arrow function", type: "snippet" }),
  snip("async function ${name}(${params}) {\n\t${}\n}", { label: "asyncfn", detail: "async function", type: "snippet" }),
  snip("for (const ${item} of ${items}) {\n\t${}\n}", { label: "forof", detail: "for…of loop", type: "snippet" }),
  snip("for (const ${key} in ${object}) {\n\t${}\n}", { label: "forin", detail: "for…in loop", type: "snippet" }),
  snip("${items}.forEach((${item}) => {\n\t${}\n});", { label: "foreach", detail: "Array.forEach", type: "snippet" }),
  snip("${items}.map((${item}) => ${item});", { label: "map", detail: "Array.map", type: "snippet" }),
  snip("${items}.filter((${item}) => ${});", { label: "filter", detail: "Array.filter", type: "snippet" }),
  snip("${items}.reduce((${acc}, ${item}) => ${acc}, ${0});", { label: "reduce", detail: "Array.reduce", type: "snippet" }),
  snip("if (${cond}) {\n\t${}\n} else {\n\t${}\n}", { label: "ifelse", detail: "if / else", type: "snippet" }),
  snip("switch (${value}) {\n\tcase ${1}:\n\t\t${}\n\t\tbreak;\n\tdefault:\n\t\tbreak;\n}", { label: "switch", detail: "switch", type: "snippet" }),
  snip("try {\n\t${}\n} catch (${err}) {\n\tconsole.error(${err});\n}", { label: "trycatch", detail: "try / catch", type: "snippet" }),
  snip("const ${res} = await fetch(${url});\nconst ${data} = await ${res}.json();\n${}", { label: "fetch", detail: "fetch + json", type: "snippet" }),
  snip("new Promise((resolve, reject) => {\n\t${}\n})", { label: "promise", detail: "new Promise", type: "snippet" }),
  snip("setTimeout(() => {\n\t${}\n}, ${1000});", { label: "timeout", detail: "setTimeout", type: "snippet" }),
  snip("setInterval(() => {\n\t${}\n}, ${1000});", { label: "interval", detail: "setInterval", type: "snippet" }),
  snip("import ${name} from \"${module}\";", { label: "import", detail: "import default", type: "snippet" }),
  snip("import { ${name} } from \"${module}\";", { label: "imp", detail: "import named", type: "snippet" }),
  snip("export default ${};", { label: "exportdefault", detail: "export default", type: "snippet" }),
  snip("export const ${name} = ${};", { label: "exportconst", detail: "export const", type: "snippet" }),
  snip("const ${name} = require(\"${module}\");", { label: "require", detail: "CommonJS require", type: "snippet" }),
  snip("document.querySelector(\"${selector}\")", { label: "qs", detail: "querySelector", type: "snippet" }),
  snip("document.querySelectorAll(\"${selector}\")", { label: "qsa", detail: "querySelectorAll", type: "snippet" }),
  snip("document.getElementById(\"${id}\")", { label: "gid", detail: "getElementById", type: "snippet" }),
  snip("${el}.addEventListener(\"${click}\", (${e}) => {\n\t${}\n});", { label: "listener", detail: "addEventListener", type: "snippet" }),
  snip("document.addEventListener(\"DOMContentLoaded\", () => {\n\t${}\n});", { label: "domready", detail: "DOMContentLoaded", type: "snippet" }),
  snip("JSON.stringify(${value}, null, 2)", { label: "stringify", detail: "JSON.stringify", type: "snippet" }),
  snip("JSON.parse(${text})", { label: "parse", detail: "JSON.parse", type: "snippet" }),
  snip("class ${Name} {\n\tconstructor(${params}) {\n\t\t${}\n\t}\n}", { label: "class", detail: "class with constructor", type: "snippet" }),
  snip("function ${Name}(${props}) {\n\treturn (\n\t\t<div>\n\t\t\t${}\n\t\t</div>\n\t);\n}\n\nexport default ${Name};", { label: "rfc", detail: "React function component", type: "snippet" }),
  snip("const [${state}, set${State}] = useState(${initial});", { label: "useState", detail: "React hook", type: "snippet" }),
  snip("useEffect(() => {\n\t${}\n}, [${deps}]);", { label: "useEffect", detail: "React hook", type: "snippet" }),
  snip("const express = require(\"express\");\nconst app = express();\n\napp.use(express.json());\n\napp.get(\"/\", (req, res) => {\n\tres.send(\"${Hello}\");\n});\n\napp.listen(${3000}, () => console.log(\"Server running\"));", { label: "express", detail: "Express server", type: "snippet" }),
  snip("app.${get}(\"${/path}\", async (req, res) => {\n\t${}\n});", { label: "route", detail: "Express route", type: "snippet" }),
  snip("(async () => {\n\t${}\n})();", { label: "iife", detail: "async IIFE", type: "snippet" }),
  snip("/**\n * ${description}\n */", { label: "doc", detail: "JSDoc comment", type: "snippet" }),
];
const TS_SNIPPETS = [
  snip("interface ${Name} {\n\t${}\n}", { label: "interface", detail: "TypeScript", type: "snippet" }),
  snip("type ${Name} = ${};", { label: "type", detail: "TypeScript", type: "snippet" }),
  snip("enum ${Name} {\n\t${}\n}", { label: "enum", detail: "TypeScript", type: "snippet" }),
];

let browserScope = null;
function browserGlobals(context) {
  // Enumerate the WebView's own globals once: document, window, fetch, Map, every DOM constructor …
  browserScope ??= scopeCompletionSource(globalThis);
  return browserScope(context);
}

const moduleNames = () => [...new Set([...workspace.npm, ...NODE_MODULES, ...NPM_POPULAR])];

/** Module specifiers: `import x from "…"`, `require("…")`, `import("…")`. */
function moduleCompletion(context) {
  const m = context.matchBefore(/(?:from\s*|import\s*\(?\s*|require\s*\(\s*|import\s+)["']([^"']*)$/);
  if (!m) return null;
  const typed = /["']([^"']*)$/.exec(m.text)[1];
  const from = context.pos - typed.length;
  if (typed.startsWith(".") || typed.startsWith("/")) return null; // relative paths: let the user type
  const deps = new Set(workspace.npm);
  return {
    from,
    options: moduleNames().map((name) => ({
      label: name, type: "module",
      detail: deps.has(name) ? "package.json" : NODE_MODULES.includes(name) ? "node" : "npm",
      boost: deps.has(name) ? 20 : NODE_MODULES.includes(name) ? 10 : 0,
    })),
    validFor: /^[\w@./-]*$/,
  };
}

export function javascriptCompletion(typescript) {
  const statics = [...NODE_GLOBALS, ...SNIPPETS, ...(typescript ? TS_SNIPPETS : [])];
  return [
    moduleCompletion,
    (context) => {
      const word = context.matchBefore(/[\w$]*$/);
      if (!word || (!word.text && !context.explicit)) return null;
      if (context.matchBefore(/\.[\w$]*$/)) return null; // after a dot: the scope source handles it
      return { from: word.from, options: statics, validFor: /^[\w$]*$/ };
    },
    browserGlobals,
  ];
}

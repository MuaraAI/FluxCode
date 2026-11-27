'use strict';

const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const Module = require('module');
const readline = require('readline');
const os = require('os');
const crypto = require('crypto');
const { spawn } = require('child_process');

// Keep raw stdout exclusively for NDJSON JSON-RPC frames.
const rawStdoutWrite = process.stdout.write.bind(process.stdout);
function sendRpc(obj) {
  try {
    rawStdoutWrite(JSON.stringify(obj) + '\n');
  } catch (err) {
    process.stderr.write('rpc-encode-error: ' + String(err) + '\n');
  }
}
function notify(method, params) {
  sendRpc({ method, params });
}
function reply(id, result, error) {
  sendRpc(error !== undefined ? { id, error: String(error) } : { id, result });
}

let activeExtLogId = 'host';
function formatLogArgs(args) {
  return args.map((a) => (typeof a === 'string' ? a : JSON.stringify(a) || String(a))).join(' ');
}
console.log = (...args) => notify('outputAppend', { extId: activeExtLogId, channel: 'console', text: formatLogArgs(args) });
console.info = console.log;
console.warn = (...args) => notify('outputAppend', { extId: activeExtLogId, channel: 'console', text: '[warn] ' + formatLogArgs(args) });
console.error = (...args) => notify('outputAppend', { extId: activeExtLogId, channel: 'console', text: '[error] ' + formatLogArgs(args) });
console.debug = () => {};

process.on('uncaughtException', (err) => {
  notify('outputAppend', { extId: activeExtLogId, channel: 'host', text: 'Uncaught error: ' + (err && err.stack ? err.stack : String(err)) });
});
process.on('unhandledRejection', (reason) => {
  notify('outputAppend', { extId: activeExtLogId, channel: 'host', text: 'Unhandled rejection: ' + (reason && reason.stack ? reason.stack : String(reason)) });
});

const hostState = {
  bridgePort: 0,
  bridgeToken: '',
  stateDir: '',
  workspaceFolders: [],
  activeFile: null,
  themeKind: 2, // 1 = Light, 2 = Dark
  extConfigs: new Map(),
  userConfig: {},
  allExtMetas: [],
  contexts: new Map(),
  commands: new Map(),
  webviewViewProviders: new Map(),
  webviewViews: new Map(), // handle -> { viewId, extId, webview, emitter }
  webviewPanels: new Map(),
  customEditors: new Map(),
  treeProviders: new Map(),
  treeNodesById: new Map(), // viewId -> Map(nodeId -> raw element)
  uriHandlers: [],
  activated: new Map(), // extId -> { exports, context }
  pendingResolveViews: [],
  statusBarSeq: 0,
  handleSeq: 0,
};

// ---------------- Core VS Code Types ----------------

class Disposable {
  constructor(callOnDispose) { this._fn = typeof callOnDispose === 'function' ? callOnDispose : null; }
  dispose() { if (this._fn) { const fn = this._fn; this._fn = null; fn(); } }
  static from(...disposables) {
    return new Disposable(() => {
      for (const d of disposables) {
        if (d && typeof d.dispose === 'function') d.dispose();
      }
    });
  }
}

class EventEmitter {
  constructor() {
    this._listeners = new Set();
    this.event = (listener, thisArgs, disposables) => {
      const entry = { listener, thisArgs };
      this._listeners.add(entry);
      const disp = new Disposable(() => this._listeners.delete(entry));
      if (Array.isArray(disposables)) disposables.push(disp);
      return disp;
    };
  }
  fire(data) {
    for (const { listener, thisArgs } of Array.from(this._listeners)) {
      try { listener.call(thisArgs, data); } catch (e) { console.error(e && e.stack ? e.stack : e); }
    }
  }
  dispose() { this._listeners.clear(); }
}

class CancellationError extends Error {
  constructor(msg = 'Canceled') { super(msg); this.name = 'Canceled'; }
}

class CancellationTokenSource {
  constructor() {
    this._cancelled = false;
    this._emitter = new EventEmitter();
    this.token = {
      get isCancellationRequested() { return this._src._cancelled; },
      _src: this,
      onCancellationRequested: this._emitter.event,
    };
  }
  cancel() {
    if (!this._cancelled) {
      this._cancelled = true;
      this._emitter.fire();
    }
  }
  dispose() { this._emitter.dispose(); }
}

class Uri {
  constructor(scheme, authority, p, query, fragment) {
    this.scheme = scheme || 'file';
    this.authority = authority || '';
    this.path = p || '';
    this.query = query || '';
    this.fragment = fragment || '';
  }
  get fsPath() {
    if (this.scheme !== 'file') return this.path;
    let p = this.path;
    if (process.platform === 'win32' && /^\/[A-Za-z]:/.test(p)) {
      p = p.slice(1).replace(/\//g, '\\');
    }
    return p;
  }
  with(change) {
    return new Uri(
      change.scheme !== undefined ? change.scheme : this.scheme,
      change.authority !== undefined ? change.authority : this.authority,
      change.path !== undefined ? change.path : this.path,
      change.query !== undefined ? change.query : this.query,
      change.fragment !== undefined ? change.fragment : this.fragment,
    );
  }
  toString(skipEncoding) {
    let res = this.scheme ? this.scheme + ':' : '';
    if (this.authority || this.scheme === 'file') res += '//' + this.authority;
    res += this.path || '';
    if (this.query) res += '?' + (skipEncoding ? this.query : encodeURI(this.query));
    if (this.fragment) res += '#' + this.fragment;
    return res;
  }
  toJSON() { return { $mid: 1, scheme: this.scheme, authority: this.authority, path: this.path, query: this.query, fragment: this.fragment, external: this.toString(true) }; }
  static file(fsPath) {
    let p = String(fsPath || '').replace(/\\/g, '/');
    if (p && p[0] !== '/') p = '/' + p;
    return new Uri('file', '', p, '', '');
  }
  static parse(value) {
    const str = String(value || '');
    const m = /^([^:/?#]+):(?:\/\/([^/?#]*))?([^?#]*)(?:\?([^#]*))?(?:#(.*))?$/.exec(str);
    if (!m) return Uri.file(str);
    return new Uri(m[1], m[2] || '', m[3] || '', m[4] || '', m[5] || '');
  }
  static joinPath(base, ...segments) {
    let p = base.path || '/';
    for (const seg of segments) {
      const s = String(seg).replace(/\\/g, '/').replace(/^\/+/, '');
      if (!s) continue;
      p = p.endsWith('/') ? p + s : p + '/' + s;
    }
    p = path.posix.normalize(p);
    return base.with({ path: p });
  }
  static from(c) { return new Uri(c.scheme, c.authority, c.path, c.query, c.fragment); }
  static isUri(v) { return v instanceof Uri || (v && typeof v.scheme === 'string' && typeof v.path === 'string'); }
}

class Position {
  constructor(line, character) { this.line = Math.max(0, line | 0); this.character = Math.max(0, character | 0); }
  isBefore(o) { return this.line < o.line || (this.line === o.line && this.character < o.character); }
  isAfter(o) { return this.line > o.line || (this.line === o.line && this.character > o.character); }
  isEqual(o) { return this.line === o.line && this.character === o.character; }
  translate(dl = 0, dc = 0) { return new Position(this.line + dl, this.character + dc); }
  with(line = this.line, character = this.character) { return new Position(line, character); }
}

class Range {
  constructor(a, b, c, d) {
    if (typeof a === 'number') {
      this.start = new Position(a, b);
      this.end = new Position(c, d);
    } else {
      this.start = a;
      this.end = b;
    }
  }
  get isEmpty() { return this.start.isEqual(this.end); }
  get isSingleLine() { return this.start.line === this.end.line; }
  contains(posOrRange) {
    if (posOrRange instanceof Range) return this.contains(posOrRange.start) && this.contains(posOrRange.end);
    return !posOrRange.isBefore(this.start) && !posOrRange.isAfter(this.end);
  }
  with(start = this.start, end = this.end) { return new Range(start, end); }
}

class Selection extends Range {
  constructor(a, b, c, d) {
    super(a, b, c, d);
    this.anchor = this.start;
    this.active = this.end;
  }
  get isReversed() { return this.active.isBefore(this.anchor); }
}

class Location {
  constructor(uri, rangeOrPos) {
    this.uri = uri;
    this.range = rangeOrPos instanceof Position ? new Range(rangeOrPos, rangeOrPos) : rangeOrPos;
  }
}

class TextEdit {
  constructor(range, newText) { this.range = range; this.newText = newText; }
  static replace(range, text) { return new TextEdit(range, text); }
  static insert(pos, text) { return new TextEdit(new Range(pos, pos), text); }
  static delete(range) { return new TextEdit(range, ''); }
}

class WorkspaceEdit {
  constructor() { this._edits = new Map(); }
  _list(uri) {
    const k = uri.toString();
    if (!this._edits.has(k)) this._edits.set(k, { uri, edits: [] });
    return this._edits.get(k).edits;
  }
  replace(uri, range, newText) { this._list(uri).push(TextEdit.replace(range, newText)); }
  insert(uri, pos, newText) { this._list(uri).push(TextEdit.insert(pos, newText)); }
  delete(uri, range) { this._list(uri).push(TextEdit.delete(range)); }
  set(uri, edits) { this._edits.set(uri.toString(), { uri, edits: edits.slice() }); }
  entries() { return Array.from(this._edits.values()).map((x) => [x.uri, x.edits]); }
  get size() { return this._edits.size; }
}

class MarkdownString {
  constructor(value = '', supportThemeIcons = false) {
    this.value = value;
    this.isTrusted = false;
    this.supportThemeIcons = supportThemeIcons;
  }
  appendText(v) { this.value += String(v).replace(/[\\`*_{}[\]()#+\-.!]/g, '\\$&'); return this; }
  appendMarkdown(v) { this.value += String(v); return this; }
  appendCodeblock(v, lang = '') { this.value += `\n\`\`\`${lang}\n${v}\n\`\`\`\n`; return this; }
}

class ThemeIcon {
  constructor(id, color) { this.id = id; this.color = color; }
}
ThemeIcon.File = new ThemeIcon('file');
ThemeIcon.Folder = new ThemeIcon('folder');

class ThemeColor { constructor(id) { this.id = id; } }
class SnippetString { constructor(value = '') { this.value = value; } }

class TreeItem {
  constructor(label, collapsibleState = 0) {
    this.label = label;
    this.collapsibleState = collapsibleState;
  }
}

class CodeLens { constructor(range, command) { this.range = range; this.command = command; this.isResolved = !!command; } }
class CodeAction { constructor(title, kind) { this.title = title; this.kind = kind; } }
class CodeActionKind {
  constructor(value) { this.value = value; }
  append(part) { return new CodeActionKind(this.value ? this.value + '.' + part : part); }
  contains(other) { return other && (other.value === this.value || other.value.startsWith(this.value + '.')); }
}
CodeActionKind.Empty = new CodeActionKind('');
CodeActionKind.QuickFix = new CodeActionKind('quickfix');
CodeActionKind.Refactor = new CodeActionKind('refactor');
CodeActionKind.Source = new CodeActionKind('source');
CodeActionKind.SourceOrganizeImports = new CodeActionKind('source.organizeImports');

class CompletionItem { constructor(label, kind) { this.label = label; this.kind = kind; } }
class CompletionList { constructor(items = [], isIncomplete = false) { this.items = items; this.isIncomplete = isIncomplete; } }
class Hover { constructor(contents, range) { this.contents = Array.isArray(contents) ? contents : [contents]; this.range = range; } }
class Diagnostic {
  constructor(range, message, severity = 0) { this.range = range; this.message = message; this.severity = severity; }
}
class DiagnosticRelatedInformation { constructor(location, message) { this.location = location; this.message = message; } }

class RelativePattern {
  constructor(base, pattern) {
    this.baseUri = Uri.isUri(base) ? base : Uri.file(typeof base === 'string' ? base : base.uri.fsPath);
    this.base = this.baseUri.fsPath;
    this.pattern = pattern;
  }
}

class FileSystemError extends Error {
  constructor(message, code = 'Unknown') { super(message); this.code = code; this.name = 'EntryNotFound'; }
  static FileNotFound(msg) { return new FileSystemError(String(msg || 'FileNotFound'), 'FileNotFound'); }
  static FileExists(msg) { return new FileSystemError(String(msg || 'FileExists'), 'FileExists'); }
  static FileNotADirectory(msg) { return new FileSystemError(String(msg || 'FileNotADirectory'), 'FileNotADirectory'); }
  static FileIsADirectory(msg) { return new FileSystemError(String(msg || 'FileIsADirectory'), 'FileIsADirectory'); }
  static NoPermissions(msg) { return new FileSystemError(String(msg || 'NoPermissions'), 'NoPermissions'); }
  static Unavailable(msg) { return new FileSystemError(String(msg || 'Unavailable'), 'Unavailable'); }
}

// ---------------- Enums ----------------

const Enum = {
  TreeItemCollapsibleState: { None: 0, Collapsed: 1, Expanded: 2 },
  StatusBarAlignment: { Left: 1, Right: 2 },
  ViewColumn: { Active: -1, Beside: -2, One: 1, Two: 2, Three: 3 },
  DiagnosticSeverity: { Error: 0, Warning: 1, Information: 2, Hint: 3 },
  ConfigurationTarget: { Global: 1, Workspace: 2, WorkspaceFolder: 3 },
  ExtensionMode: { Production: 1, Development: 2, Test: 3 },
  UIKind: { Desktop: 1, Web: 2 },
  ColorThemeKind: { Light: 1, Dark: 2, HighContrast: 3, HighContrastLight: 4 },
  ProgressLocation: { SourceControl: 1, Window: 10, Notification: 15 },
  QuickPickItemKind: { Separator: -1, Default: 0 },
  InputBoxValidationSeverity: { Info: 1, Warning: 2, Error: 3 },
  FileType: { Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 },
  FileChangeType: { Changed: 1, Created: 2, Deleted: 3 },
  EndOfLine: { LF: 1, CRLF: 2 },
  DecorationRangeBehavior: { OpenOpen: 0, ClosedClosed: 1, OpenClosed: 2, ClosedOpen: 3 },
  OverviewRulerLane: { Left: 1, Center: 2, Right: 4, Full: 7 },
  CommentMode: { Editing: 0, Preview: 1 },
  CommentThreadCollapsibleState: { Collapsed: 0, Expanded: 1 },
  TextEditorRevealType: { Default: 0, InCenter: 1, InCenterIfOutsideViewport: 2, AtTop: 3 },
  TextDocumentSaveReason: { Manual: 1, AfterDelay: 2, FocusOut: 3 },
  EnvironmentVariableMutatorType: { Replace: 1, Append: 2, Prepend: 3 },
  LogLevel: { Off: 0, Trace: 1, Debug: 2, Info: 3, Warning: 4, Error: 5 },
  CompletionItemKind: {
    Text: 0, Method: 1, Function: 2, Constructor: 3, Field: 4, Variable: 5, Class: 6,
    Interface: 7, Module: 8, Property: 9, Unit: 10, Value: 11, Enum: 12, Keyword: 13,
    Snippet: 14, Color: 15, File: 16, Reference: 17, Folder: 18,
  },
  SymbolKind: {
    File: 0, Module: 1, Namespace: 2, Package: 3, Class: 4, Method: 5, Property: 6,
    Field: 7, Constructor: 8, Enum: 9, Interface: 10, Function: 11, Variable: 12, Constant: 13,
  },
};

// ---------------- State Persistence ----------------

function safeStateFile(extId, scope) {
  const safe = String(extId || 'default').replace(/[^a-zA-Z0-9._-]/g, '_');
  return path.join(hostState.stateDir || os.tmpdir(), `${safe}.${scope}.json`);
}

function createMemento(extId, scope) {
  const file = safeStateFile(extId, scope);
  let store = {};
  try {
    if (fs.existsSync(file)) store = JSON.parse(fs.readFileSync(file, 'utf8')) || {};
  } catch {}
  return {
    keys() { return Object.keys(store); },
    get(key, defaultValue) { return Object.prototype.hasOwnProperty.call(store, key) ? store[key] : defaultValue; },
    async update(key, value) {
      if (value === undefined) delete store[key];
      else store[key] = value;
      try {
        await fsp.mkdir(path.dirname(file), { recursive: true });
        await fsp.writeFile(file, JSON.stringify(store), 'utf8');
      } catch {}
    },
    setKeysForSync() {},
  };
}

function createSecrets(extId) {
  const memento = createMemento(extId, 'secrets');
  const emitter = new EventEmitter();
  return {
    get: async (key) => memento.get(key, undefined),
    store: async (key, val) => { await memento.update(key, String(val)); emitter.fire({ key }); },
    delete: async (key) => { await memento.update(key, undefined); emitter.fire({ key }); },
    onDidChange: emitter.event,
  };
}

// ---------------- Webview Helpers ----------------

function makeWebview(handle, extFolder, initialOptions) {
  let options = initialOptions || {};
  let html = '';
  const receiveEmitter = new EventEmitter();
  const webview = {
    get options() { return options; },
    set options(v) {
      options = v || {};
      const roots = Array.isArray(options.localResourceRoots)
        ? options.localResourceRoots.map((u) => (Uri.isUri(u) ? u.fsPath : String(u)))
        : [];
      notify('webviewOptions', { handle, localRoots: roots });
    },
    get html() { return html; },
    set html(v) {
      html = String(v || '');
      notify('webviewHtml', { handle, html });
    },
    get cspSource() {
      return `http://127.0.0.1:${hostState.bridgePort}`;
    },
    onDidReceiveMessage: receiveEmitter.event,
    async postMessage(message) {
      notify('webviewPostMessage', { handle, messageJson: JSON.stringify(message === undefined ? null : message) });
      return true;
    },
    asWebviewUri(uri) {
      const target = Uri.isUri(uri) ? uri.fsPath : String(uri);
      const roots = [extFolder];
      if (Array.isArray(options.localResourceRoots)) {
        for (const r of options.localResourceRoots) {
          const rp = Uri.isUri(r) ? r.fsPath : String(r);
          if (rp && !roots.includes(rp)) roots.push(rp);
        }
      }
      let chosenIdx = 0;
      let rel = path.relative(extFolder, target);
      if (rel.startsWith('..') || path.isAbsolute(rel)) {
        for (let i = 1; i < roots.length; i++) {
          const candidate = path.relative(roots[i], target);
          if (!candidate.startsWith('..') && !path.isAbsolute(candidate)) {
            chosenIdx = i;
            rel = `r${i}/` + candidate;
            break;
          }
        }
      }
      const enc = rel.replace(/\\/g, '/').split('/').map(encodeURIComponent).join('/');
      return Uri.parse(`http://127.0.0.1:${hostState.bridgePort}/res/${encodeURIComponent(handle)}/${enc}?t=${encodeURIComponent(hostState.bridgeToken)}`);
    },
  };
  return { webview, receiveEmitter };
}

// ---------------- Namespaces ----------------

const onDidChangeConfigEmitter = new EventEmitter();
const onDidChangeFoldersEmitter = new EventEmitter();
const onDidOpenDocEmitter = new EventEmitter();
const onDidChangeDocEmitter = new EventEmitter();
const onDidSaveDocEmitter = new EventEmitter();
const onDidCloseDocEmitter = new EventEmitter();
const onDidChangeEditorEmitter = new EventEmitter();
const onDidChangeThemeEmitter = new EventEmitter();
const onDidChangeWinStateEmitter = new EventEmitter();

function resolveConfigValue(section, key) {
  const full = section ? `${section}.${key}` : key;
  if (Object.prototype.hasOwnProperty.call(hostState.userConfig, full)) return hostState.userConfig[full];
  for (const cfg of hostState.extConfigs.values()) {
    if (Object.prototype.hasOwnProperty.call(cfg, full)) return cfg[full];
  }
  return undefined;
}

const pkgJsonCache = new Map();
function readPackageJson(folder) {
  if (!folder) return {};
  if (pkgJsonCache.has(folder)) return pkgJsonCache.get(folder);
  let parsed = {};
  try {
    parsed = JSON.parse(fs.readFileSync(path.join(folder, 'package.json'), 'utf8')) || {};
  } catch {}
  pkgJsonCache.set(folder, parsed);
  return parsed;
}

const vscodeApi = {
  version: '1.99.0',
  ...Enum,
  Disposable,
  EventEmitter,
  CancellationError,
  CancellationTokenSource,
  Uri,
  Position,
  Range,
  Selection,
  Location,
  TextEdit,
  WorkspaceEdit,
  MarkdownString,
  ThemeIcon,
  ThemeColor,
  SnippetString,
  TreeItem,
  CodeLens,
  CodeAction,
  CodeActionKind,
  CompletionItem,
  CompletionList,
  Hover,
  Diagnostic,
  DiagnosticRelatedInformation,
  RelativePattern,
  FileSystemError,
  TabInputText: class { constructor(uri) { this.uri = uri; } },
  TabInputCustom: class { constructor(uri, viewType) { this.uri = uri; this.viewType = viewType; } },
  TabInputWebview: class { constructor(viewType) { this.viewType = viewType; } },
  TabInputTextDiff: class { constructor(original, modified) { this.original = original; this.modified = modified; } },

  commands: {
    registerCommand(command, callback, thisArg) {
      hostState.commands.set(command, { callback, thisArg });
      return new Disposable(() => hostState.commands.delete(command));
    },
    registerTextEditorCommand(command, callback, thisArg) {
      hostState.commands.set(command, {
        callback: (...args) => callback.call(thisArg, undefined, undefined, ...args),
        thisArg,
      });
      return new Disposable(() => hostState.commands.delete(command));
    },
    async executeCommand(command, ...rest) {
      if (command === 'setContext' && rest.length >= 1) {
        hostState.contexts.set(String(rest[0]), rest[1]);
        notify('contextKey', { key: String(rest[0]), value: rest[1] === undefined ? null : rest[1] });
        return;
      }
      if (command === 'vscode.open' && rest[0]) {
        const u = Uri.isUri(rest[0]) ? rest[0] : Uri.parse(String(rest[0]));
        if (u.scheme === 'file') notify('openFile', { path: u.fsPath });
        else notify('openUrl', { url: u.toString(true) });
        return;
      }
      if (typeof command === 'string' && command.startsWith('workbench.view.extension.')) {
        notify('focusContainer', { id: command.slice('workbench.view.extension.'.length) });
        return;
      }
      const entry = hostState.commands.get(command);
      if (entry) {
        return await Promise.resolve(entry.callback.apply(entry.thisArg, rest));
      }
      return undefined;
    },
    async getCommands(filterInternal) {
      const list = Array.from(hostState.commands.keys());
      return filterInternal ? list.filter((c) => !c.startsWith('_')) : list;
    },
  },

  window: {
    activeTextEditor: undefined,
    visibleTextEditors: [],
    terminals: [],
    activeTerminal: undefined,
    state: { focused: true, active: true },
    get activeColorTheme() { return { kind: hostState.themeKind }; },
    tabGroups: { all: [], activeTabGroup: { tabs: [], activeTab: undefined }, onDidChangeTabs: new EventEmitter().event, onDidChangeTabGroups: new EventEmitter().event },
    onDidChangeActiveTextEditor: onDidChangeEditorEmitter.event,
    onDidChangeVisibleTextEditors: new EventEmitter().event,
    onDidChangeTextEditorSelection: new EventEmitter().event,
    onDidChangeActiveColorTheme: onDidChangeThemeEmitter.event,
    onDidChangeWindowState: onDidChangeWinStateEmitter.event,
    onDidOpenTerminal: new EventEmitter().event,
    onDidCloseTerminal: new EventEmitter().event,
    onDidChangeActiveTerminal: new EventEmitter().event,
    onDidChangeActiveNotebookEditor: new EventEmitter().event,
    onDidChangeTextEditorVisibleRanges: new EventEmitter().event,
    async showNotebookDocument() { return undefined; },

    registerWebviewViewProvider(viewId, provider, opts) {
      const extId = activeExtLogId;
      hostState.webviewViewProviders.set(viewId, { viewId, extId, provider, options: opts || {} });
      const pending = hostState.pendingResolveViews.filter((p) => p.viewId === viewId);
      if (pending.length) {
        hostState.pendingResolveViews = hostState.pendingResolveViews.filter((p) => p.viewId !== viewId);
        for (const p of pending) {
          resolveWebviewViewInternal(p.extId, p.viewId, p.handle).catch((e) => console.error(e));
        }
      }
      return new Disposable(() => hostState.webviewViewProviders.delete(viewId));
    },

    createWebviewPanel(viewType, title, showOptions, options) {
      const handle = 'panel_' + (++hostState.handleSeq);
      const extMeta = hostState.activated.get(activeExtLogId);
      const extFolder = extMeta ? extMeta.folder : process.cwd();
      const { webview, receiveEmitter } = makeWebview(handle, extFolder, options);
      const dispEmitter = new EventEmitter();
      const stateEmitter = new EventEmitter();
      notify('webviewPanelCreated', {
        handle,
        extId: activeExtLogId,
        viewType,
        title: title || viewType,
        extFolder,
        localRoots: (options && Array.isArray(options.localResourceRoots)) ? options.localResourceRoots.map((u) => (Uri.isUri(u) ? u.fsPath : String(u))) : [],
      });
      const panel = {
        viewType,
        get title() { return title; },
        set title(v) { title = String(v); notify('webviewTitle', { handle, title }); },
        webview,
        options: options || {},
        viewColumn: typeof showOptions === 'number' ? showOptions : 1,
        active: true,
        visible: true,
        onDidDispose: dispEmitter.event,
        onDidChangeViewState: stateEmitter.event,
        reveal() {},
        dispose() {
          receiveEmitter.dispose();
          dispEmitter.fire();
          dispEmitter.dispose();
          stateEmitter.dispose();
          hostState.webviewPanels.delete(handle);
        },
      };
      hostState.webviewPanels.set(handle, { panel, receiveEmitter });
      return panel;
    },

    registerWebviewPanelSerializer() { return new Disposable(() => {}); },
    registerCustomEditorProvider(viewType, provider) {
      hostState.customEditors.set(viewType, provider);
      return new Disposable(() => hostState.customEditors.delete(viewType));
    },
    registerTreeDataProvider(viewId, provider) {
      hostState.treeProviders.set(viewId, { extId: activeExtLogId, provider });
      if (typeof provider.onDidChangeTreeData === 'function') {
        provider.onDidChangeTreeData(() => {
          fetchTreeChildren(viewId, null).catch((e) => console.error(e));
        });
      }
      fetchTreeChildren(viewId, null).catch((e) => console.error(e));
      return new Disposable(() => hostState.treeProviders.delete(viewId));
    },
    createTreeView(viewId, options) {
      const disp = this.registerTreeDataProvider(viewId, options.treeDataProvider);
      return {
        selection: [],
        visible: true,
        onDidExpandElement: new EventEmitter().event,
        onDidCollapseElement: new EventEmitter().event,
        onDidChangeSelection: new EventEmitter().event,
        onDidChangeVisibility: new EventEmitter().event,
        reveal: async () => {},
        dispose: () => disp.dispose(),
      };
    },
    registerUriHandler(handler) {
      hostState.uriHandlers.push(handler);
      return new Disposable(() => {
        const idx = hostState.uriHandlers.indexOf(handler);
        if (idx >= 0) hostState.uriHandlers.splice(idx, 1);
      });
    },
    registerFileDecorationProvider() { return new Disposable(() => {}); },

    createOutputChannel(name, options) {
      const extId = activeExtLogId;
      const emit = (txt) => notify('outputAppend', { extId, channel: name, text: txt });
      const ch = {
        name,
        append: (v) => emit(String(v)),
        appendLine: (v) => emit(String(v) + '\n'),
        replace: (v) => emit(String(v)),
        clear: () => {},
        show: () => {},
        hide: () => {},
        dispose: () => {},
      };
      if (options && options.log) {
        ch.logLevel = Enum.LogLevel.Info;
        ch.onDidChangeLogLevel = new EventEmitter().event;
        ch.trace = (m) => emit('[trace] ' + m);
        ch.debug = (m) => emit('[debug] ' + m);
        ch.info = (m) => emit('[info] ' + m);
        ch.warn = (m) => emit('[warn] ' + m);
        ch.error = (m) => emit('[error] ' + (m instanceof Error ? (m.stack || m.message) : m));
      }
      return ch;
    },

    createStatusBarItem(alignmentOrId, priority) {
      const id = typeof alignmentOrId === 'string' ? alignmentOrId : ('sb_' + (++hostState.statusBarSeq));
      const extId = activeExtLogId;
      const item = {
        id,
        alignment: typeof alignmentOrId === 'number' ? alignmentOrId : Enum.StatusBarAlignment.Left,
        priority: priority || 0,
        text: '',
        tooltip: '',
        command: undefined,
        name: extId,
        _visible: false,
        show() {
          this._visible = true;
          const cmdId = typeof this.command === 'string' ? this.command : (this.command && this.command.command) || null;
          notify('statusBarUpdate', {
            id: this.id,
            extId,
            text: stripCodicons(this.text || ''),
            tooltip: typeof this.tooltip === 'string' ? this.tooltip : (this.tooltip && this.tooltip.value) || '',
            command: cmdId,
            priority: this.priority || 0,
          });
        },
        hide() {
          this._visible = false;
          notify('statusBarRemove', { id: this.id });
        },
        dispose() { this.hide(); },
      };
      return item;
    },

    createTerminal(nameOrOpts, shellPath, shellArgs) {
      const opts = typeof nameOrOpts === 'object' ? nameOrOpts : { name: nameOrOpts, shellPath, shellArgs };
      return {
        name: opts.name || 'Terminal',
        processId: Promise.resolve(undefined),
        creationOptions: opts,
        sendText(text) { notify('runTerminal', { name: opts.name || 'Terminal', cmd: String(text || '') }); },
        show() {},
        hide() {},
        dispose() {},
      };
    },

    createTextEditorDecorationType() { return { key: 'deco', dispose() {} }; },
    showInformationMessage(msg, ...items) {
      const actions = items.filter((i) => typeof i === 'string' || (i && typeof i.title === 'string')).map((i) => (typeof i === 'string' ? i : i.title));
      notify('showMessage', { level: 'info', text: String(msg), actions });
      return Promise.resolve(undefined);
    },
    showWarningMessage(msg, ...items) {
      const actions = items.filter((i) => typeof i === 'string' || (i && typeof i.title === 'string')).map((i) => (typeof i === 'string' ? i : i.title));
      notify('showMessage', { level: 'warn', text: String(msg), actions });
      return Promise.resolve(undefined);
    },
    showErrorMessage(msg, ...items) {
      const actions = items.filter((i) => typeof i === 'string' || (i && typeof i.title === 'string')).map((i) => (typeof i === 'string' ? i : i.title));
      notify('showMessage', { level: 'error', text: String(msg), actions });
      return Promise.resolve(undefined);
    },
    setStatusBarMessage(text) {
      const id = 'sb_tmp_' + (++hostState.statusBarSeq);
      notify('statusBarUpdate', { id, extId: activeExtLogId, text: stripCodicons(String(text || '')), tooltip: '', command: null, priority: 0 });
      return new Disposable(() => notify('statusBarRemove', { id }));
    },
    async showQuickPick() { return undefined; },
    async showInputBox() { return undefined; },
    async showOpenDialog() { return undefined; },
    async showSaveDialog() { return undefined; },
    async showTextDocument(docOrUri) {
      const u = Uri.isUri(docOrUri) ? docOrUri : (docOrUri && docOrUri.uri);
      if (u && u.scheme === 'file') notify('openFile', { path: u.fsPath });
      return undefined;
    },
    async withProgress(options, task) {
      const src = new CancellationTokenSource();
      return await Promise.resolve(task({ report() {} }, src.token));
    },
    createQuickPick() {
      const items = [];
      return {
        items, selectedItems: [], activeItems: [], value: '', placeholder: '', busy: false,
        onDidChangeValue: new EventEmitter().event,
        onDidAccept: new EventEmitter().event,
        onDidHide: new EventEmitter().event,
        show() {}, hide() {}, dispose() {},
      };
    },
    createInputBox() {
      return {
        value: '', placeholder: '', password: false, busy: false,
        onDidChangeValue: new EventEmitter().event,
        onDidAccept: new EventEmitter().event,
        onDidHide: new EventEmitter().event,
        show() {}, hide() {}, dispose() {},
      };
    },
  },

  workspace: {
    get workspaceFolders() {
      return hostState.workspaceFolders.map((f, i) => ({ uri: Uri.file(f), name: path.basename(f) || f, index: i }));
    },
    get name() {
      return hostState.workspaceFolders.length ? path.basename(hostState.workspaceFolders[0]) : undefined;
    },
    isTrusted: true,
    onDidGrantWorkspaceTrust: new EventEmitter().event,
    onDidChangeConfiguration: onDidChangeConfigEmitter.event,
    onDidChangeWorkspaceFolders: onDidChangeFoldersEmitter.event,
    onDidOpenTextDocument: onDidOpenDocEmitter.event,
    onDidChangeTextDocument: onDidChangeDocEmitter.event,
    onDidSaveTextDocument: onDidSaveDocEmitter.event,
    onDidCloseTextDocument: onDidCloseDocEmitter.event,
    onWillSaveTextDocument: new EventEmitter().event,
    onWillRenameFiles: new EventEmitter().event,
    onDidSaveNotebookDocument: new EventEmitter().event,
    textDocuments: [],
    workspaceFile: undefined,
    registerFileSystemProvider() { return new Disposable(() => {}); },
    async openNotebookDocument() { return { uri: Uri.parse('untitled:Notebook-1'), cellCount: 0, getCells: () => [] }; },

    getWorkspaceFolder(uri) {
      const target = (Uri.isUri(uri) ? uri.fsPath : String(uri)).toLowerCase();
      return this.workspaceFolders.find((wf) => target.startsWith(wf.uri.fsPath.toLowerCase()));
    },
    asRelativePath(pathOrUri) {
      const p = Uri.isUri(pathOrUri) ? pathOrUri.fsPath : String(pathOrUri);
      for (const wf of hostState.workspaceFolders) {
        const rel = path.relative(wf, p);
        if (!rel.startsWith('..') && !path.isAbsolute(rel)) return rel.replace(/\\/g, '/');
      }
      return p;
    },
    getConfiguration(section) {
      return {
        get: (key, defaultValue) => {
          const val = resolveConfigValue(section, key);
          return val !== undefined ? val : defaultValue;
        },
        has: (key) => resolveConfigValue(section, key) !== undefined,
        inspect: (key) => {
          const val = resolveConfigValue(section, key);
          return { key: section ? `${section}.${key}` : key, defaultValue: val, globalValue: hostState.userConfig[section ? `${section}.${key}` : key] };
        },
        update: async (key, value) => {
          const full = section ? `${section}.${key}` : key;
          if (value === undefined) delete hostState.userConfig[full];
          else hostState.userConfig[full] = value;
          onDidChangeConfigEmitter.fire({ affectsConfiguration: (s) => full === s || full.startsWith(s + '.') });
        },
      };
    },
    async openTextDocument(uriOrPath) {
      const p = Uri.isUri(uriOrPath) ? uriOrPath.fsPath : (typeof uriOrPath === 'string' ? uriOrPath : null);
      if (!p) return makeTextDoc(Uri.parse('untitled:Untitled-1'), '');
      const text = await fsp.readFile(p, 'utf8').catch(() => '');
      return makeTextDoc(Uri.file(p), text);
    },
    async findFiles(include, exclude, maxResults = 200) {
      const root = hostState.workspaceFolders[0];
      if (!root) return [];
      const results = [];
      const needle = typeof include === 'string' ? include : (include && include.pattern) || '*';
      const suffix = needle.startsWith('**/*') ? needle.slice(4) : (needle.startsWith('*') ? needle.slice(1) : null);
      async function walk(dir) {
        if (results.length >= maxResults) return;
        let entries;
        try { entries = await fsp.readdir(dir, { withFileTypes: true }); } catch { return; }
        for (const e of entries) {
          if (results.length >= maxResults) break;
          if (e.name === 'node_modules' || e.name === '.git' || e.name === 'target') continue;
          const full = path.join(dir, e.name);
          if (e.isDirectory()) await walk(full);
          else if (!suffix || e.name.endsWith(suffix)) results.push(Uri.file(full));
        }
      }
      await walk(root);
      return results;
    },
    createFileSystemWatcher() {
      return {
        onDidCreate: new EventEmitter().event,
        onDidChange: new EventEmitter().event,
        onDidDelete: new EventEmitter().event,
        dispose() {},
      };
    },
    async applyEdit() { return false; },
    registerTextDocumentContentProvider() { return new Disposable(() => {}); },
    async saveAll() { return true; },
    fs: {
      async stat(uri) {
        try {
          const st = await fsp.stat(uri.fsPath);
          return {
            type: st.isDirectory() ? Enum.FileType.Directory : (st.isFile() ? Enum.FileType.File : Enum.FileType.Unknown),
            ctime: Math.floor(st.ctimeMs),
            mtime: Math.floor(st.mtimeMs),
            size: st.size,
          };
        } catch { throw FileSystemError.FileNotFound(uri.toString()); }
      },
      async readDirectory(uri) {
        try {
          const ents = await fsp.readdir(uri.fsPath, { withFileTypes: true });
          return ents.map((e) => [e.name, e.isDirectory() ? Enum.FileType.Directory : (e.isFile() ? Enum.FileType.File : Enum.FileType.Unknown)]);
        } catch { throw FileSystemError.FileNotFound(uri.toString()); }
      },
      async createDirectory(uri) { await fsp.mkdir(uri.fsPath, { recursive: true }); },
      async readFile(uri) {
        try { return new Uint8Array(await fsp.readFile(uri.fsPath)); }
        catch { throw FileSystemError.FileNotFound(uri.toString()); }
      },
      async writeFile(uri, content) {
        await fsp.mkdir(path.dirname(uri.fsPath), { recursive: true });
        await fsp.writeFile(uri.fsPath, content);
      },
      async delete(uri, opts) {
        await fsp.rm(uri.fsPath, { recursive: !!(opts && opts.recursive), force: true });
      },
      async rename(oldUri, newUri, opts) {
        if (!(opts && opts.overwrite) && fs.existsSync(newUri.fsPath)) throw FileSystemError.FileExists();
        await fsp.rename(oldUri.fsPath, newUri.fsPath);
      },
      async copy(srcUri, dstUri, opts) {
        await fsp.cp(srcUri.fsPath, dstUri.fsPath, { recursive: true, force: !!(opts && opts.overwrite) });
      },
      isWritableFileSystem(scheme) { return scheme === 'file'; },
    },
  },

  env: {
    appName: 'FluxCode',
    appRoot: process.cwd(),
    appHost: 'desktop',
    uriScheme: 'fluxcode',
    language: 'en',
    machineId: crypto.createHash('sha256').update(os.hostname() + ':' + os.userInfo().username).digest('hex'),
    sessionId: crypto.randomUUID ? crypto.randomUUID() : String(Date.now()),
    isNewAppInstall: false,
    isTelemetryEnabled: false,
    uiKind: 1,
    shell: process.platform === 'win32' ? (process.env.ComSpec || 'cmd.exe') : (process.env.SHELL || '/bin/sh'),
    onDidChangeTelemetryEnabled: new EventEmitter().event,
    clipboard: {
      async readText() { return ''; },
      async writeText(value) { notify('clipboardWrite', { text: String(value || '') }); },
    },
    async openExternal(target) {
      const url = Uri.isUri(target) ? target.toString(true) : String(target);
      notify('openUrl', { url });
      return true;
    },
    async asExternalUri(target) { return target; },
    createTelemetryLogger() {
      return {
        isUsageEnabled: false,
        isErrorsEnabled: false,
        onDidChangeEnableStates: new EventEmitter().event,
        logUsage() {},
        logError() {},
        dispose() {},
      };
    },
  },

  extensions: {
    get all() {
      return hostState.allExtMetas.map((m) => ({
        id: m.id,
        extensionUri: Uri.file(m.folder),
        extensionPath: m.folder,
        isActive: hostState.activated.has(m.id.toLowerCase()),
        packageJSON: readPackageJson(m.folder),
        exports: hostState.activated.get(m.id.toLowerCase())?.exports,
        activate: async () => hostState.activated.get(m.id.toLowerCase())?.exports,
      }));
    },
    getExtension(id) {
      const low = String(id || '').toLowerCase();
      return this.all.find((e) => e.id.toLowerCase() === low);
    },
    onDidChange: new EventEmitter().event,
  },

  languages: {
    registerCodeLensProvider() { return new Disposable(() => {}); },
    registerHoverProvider() { return new Disposable(() => {}); },
    registerCompletionItemProvider() { return new Disposable(() => {}); },
    registerCodeActionsProvider() { return new Disposable(() => {}); },
    registerDefinitionProvider() { return new Disposable(() => {}); },
    registerDocumentFormattingEditProvider() { return new Disposable(() => {}); },
    createDiagnosticCollection() {
      return { set() {}, delete() {}, clear() {}, dispose() {} };
    },
    getLanguages: async () => ['plaintext', 'javascript', 'typescript', 'rust', 'json', 'markdown'],
    match: () => 0,
  },

  comments: {
    createCommentController(id, label) {
      return {
        id, label,
        options: {},
        createCommentThread: (uri, range, comments) => ({
          uri, range, comments: comments || [], canReply: true, dispose() {},
        }),
        dispose() {},
      };
    },
  },

  authentication: {
    onDidChangeSessions: new EventEmitter().event,
    async getSession() { return undefined; },
    registerAuthenticationProvider() { return new Disposable(() => {}); },
  },

  debug: { onDidChangeActiveDebugSession: new EventEmitter().event, registerDebugConfigurationProvider: () => new Disposable(() => {}) },
  tasks: { registerTaskProvider: () => new Disposable(() => {}), fetchTasks: async () => [] },
  scm: { createSourceControl: (id, label) => ({ id, label, createResourceGroup: () => ({ resourceStates: [], dispose() {} }), dispose() {} }) },
  tests: { createTestController: () => ({ items: { replace() {}, add() {} }, createRunProfile: () => ({ dispose() {} }), dispose() {} }) },
  l10n: { t: (msg, ...args) => (typeof msg === 'string' ? msg.replace(/\{(\d+)\}/g, (_, i) => (args[i] !== undefined ? String(args[i]) : `{${i}}`)) : String(msg)) },
};

function stripCodicons(s) {
  return String(s).replace(/\$\([^)]+\)\s*/g, '').trim();
}

function makeTextDoc(uri, text) {
  const lines = String(text).split(/\r?\n/);
  return {
    uri,
    fileName: uri.fsPath,
    isUntitled: uri.scheme === 'untitled',
    languageId: path.extname(uri.fsPath).slice(1) || 'plaintext',
    version: 1,
    isDirty: false,
    isClosed: false,
    lineCount: lines.length,
    getText: () => text,
    lineAt: (n) => ({ text: lines[n] || '', lineNumber: n, range: new Range(n, 0, n, (lines[n] || '').length) }),
    save: async () => true,
  };
}

// Intercept require('vscode') safely across extension CommonJS bundles.
const origResolve = Module._resolveFilename;
Module._resolveFilename = function (request, parent, isMain, options) {
  if (request === 'vscode') return 'vscode';
  return origResolve.call(this, request, parent, isMain, options);
};
const origLoad = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === 'vscode') return vscodeApi;
  return origLoad.call(this, request, parent, isMain);
};

// ---------------- RPC Handlers ----------------

async function activateExtension(params) {
  const key = String(params.extId || '').toLowerCase();
  if (hostState.activated.has(key)) {
    return { ok: true, alreadyActive: true };
  }
  if (params.defaultConfig && typeof params.defaultConfig === 'object') {
    hostState.extConfigs.set(key, params.defaultConfig);
  }
  const folder = params.folder || process.cwd();
  const mainPath = params.mainPath;
  if (!mainPath || !fs.existsSync(mainPath)) {
    throw new Error('Extension entrypoint missing: ' + String(mainPath));
  }
  const folderUri = Uri.file(folder);
  const storageBase = path.join(hostState.stateDir || os.tmpdir(), key.replace(/[^a-zA-Z0-9._-]/g, '_'));
  const context = {
    subscriptions: [],
    workspaceState: createMemento(key, 'workspace'),
    globalState: createMemento(key, 'global'),
    secrets: createSecrets(key),
    extensionUri: folderUri,
    extensionPath: folder,
    extensionMode: Enum.ExtensionMode.Production,
    storageUri: Uri.file(path.join(storageBase, 'ws')),
    storagePath: path.join(storageBase, 'ws'),
    globalStorageUri: Uri.file(path.join(storageBase, 'global')),
    globalStoragePath: path.join(storageBase, 'global'),
    logUri: Uri.file(path.join(storageBase, 'logs')),
    logPath: path.join(storageBase, 'logs'),
    asAbsolutePath: (rel) => path.join(folder, String(rel || '')),
    environmentVariableCollection: { persistent: false, replace() {}, append() {}, prepend() {}, clear() {} },
    extension: { id: params.extId, extensionUri: folderUri, extensionPath: folder, isActive: true, packageJSON: readPackageJson(folder) },
  };
  const prevId = activeExtLogId;
  activeExtLogId = key;
  try {
    const mod = require(mainPath);
    let exportsVal = undefined;
    if (mod && typeof mod.activate === 'function') {
      exportsVal = await Promise.resolve(mod.activate(context));
    }
    hostState.activated.set(key, { exports: exportsVal, context, mod, folder });
    notify('extStatus', { extId: key, status: 'active' });
    return { ok: true };
  } catch (err) {
    const msg = err && err.stack ? err.stack : String(err);
    notify('extStatus', { extId: key, status: 'error', error: err && err.message ? err.message : msg });
    notify('outputAppend', { extId: key, channel: 'host', text: 'Activation failed:\n' + msg });
    throw err;
  } finally {
    activeExtLogId = prevId;
  }
}

async function resolveWebviewViewInternal(extId, viewId, handle) {
  const reg = hostState.webviewViewProviders.get(viewId);
  if (!reg) {
    hostState.pendingResolveViews.push({ extId, viewId, handle });
    return { pending: true };
  }
  const act = hostState.activated.get(String(reg.extId || extId).toLowerCase());
  const extFolder = (act && act.folder) || process.cwd();
  const { webview, receiveEmitter } = makeWebview(handle, extFolder, {});
  const disposeEmitter = new EventEmitter();
  const visEmitter = new EventEmitter();
  const webviewView = {
    viewType: viewId,
    webview,
    title: undefined,
    description: undefined,
    badge: undefined,
    visible: true,
    onDidDispose: disposeEmitter.event,
    onDidChangeVisibility: visEmitter.event,
    show() {},
  };
  hostState.webviewViews.set(handle, { viewId, extId: reg.extId, webview, receiveEmitter });
  const prevId = activeExtLogId;
  activeExtLogId = reg.extId;
  try {
    const cts = new CancellationTokenSource();
    await Promise.resolve(reg.provider.resolveWebviewView(webviewView, { state: undefined }, cts.token));
    notify('webviewViewResolved', {
      viewId,
      handle,
      title: webviewView.title || '',
      extFolder,
      localRoots: (webview.options && Array.isArray(webview.options.localResourceRoots))
        ? webview.options.localResourceRoots.map((u) => (Uri.isUri(u) ? u.fsPath : String(u)))
        : [],
    });
    return { ok: true };
  } finally {
    activeExtLogId = prevId;
  }
}

async function fetchTreeChildren(viewId, elementId) {
  const reg = hostState.treeProviders.get(viewId);
  if (!reg) return [];
  const provider = reg.provider;
  if (!hostState.treeNodesById.has(viewId)) hostState.treeNodesById.set(viewId, new Map());
  const nodeMap = hostState.treeNodesById.get(viewId);
  const parentObj = elementId ? nodeMap.get(elementId) : undefined;
  const rawList = await Promise.resolve(provider.getChildren(parentObj)) || [];
  const items = [];
  for (let i = 0; i < rawList.length; i++) {
    const raw = rawList[i];
    const ti = provider.getTreeItem ? await Promise.resolve(provider.getTreeItem(raw)) : raw;
    const label = typeof ti.label === 'string' ? ti.label : (ti.label && ti.label.label) || String(raw);
    const id = ti.id || `${elementId || 'root'}:${i}:${label}`;
    nodeMap.set(id, raw);
    let commandId = null;
    let commandArgs = [];
    if (ti.command && typeof ti.command === 'object') {
      commandId = ti.command.command || null;
      commandArgs = Array.isArray(ti.command.arguments) ? ti.command.arguments : [];
    }
    items.push({
      id,
      label,
      description: typeof ti.description === 'string' ? ti.description : '',
      tooltip: typeof ti.tooltip === 'string' ? ti.tooltip : (ti.tooltip && ti.tooltip.value) || '',
      collapsible: (ti.collapsibleState || 0) > 0,
      expanded: (ti.collapsibleState || 0) === 2,
      icon: ti.iconPath && ti.iconPath.id ? ti.iconPath.id : null,
      command: commandId,
      commandArgs,
    });
  }
  notify('treeUpdated', { viewId, parentId: elementId || null, items });
  return items;
}

const handlers = {
  async init(params) {
    hostState.bridgePort = params.bridgePort || 0;
    hostState.bridgeToken = params.bridgeToken || '';
    hostState.stateDir = params.stateDir || '';
    hostState.workspaceFolders = Array.isArray(params.workspaceFolders) ? params.workspaceFolders : [];
    hostState.activeFile = params.activeFile || null;
    hostState.themeKind = params.themeKind || 2;
    hostState.allExtMetas = Array.isArray(params.extensions) ? params.extensions : [];
    return { ok: true, pid: process.pid };
  },
  async activate(params) { return await activateExtension(params); },
  async resolveWebviewView(params) { return await resolveWebviewViewInternal(params.extId, params.viewId, params.handle); },
  async webviewMessage(params) {
    const slot = hostState.webviewViews.get(params.handle) || hostState.webviewPanels.get(params.handle);
    if (slot) {
      let parsed = params.message;
      if (typeof parsed === 'string') {
        try { parsed = JSON.parse(parsed); } catch {}
      }
      slot.receiveEmitter.fire(parsed);
    }
    return { ok: true };
  },
  async treeGetChildren(params) { return await fetchTreeChildren(params.viewId, params.elementId || null); },
  async executeCommand(params) {
    const args = Array.isArray(params.args) ? params.args : [];
    return await vscodeApi.commands.executeCommand(params.command, ...args);
  },
  async updateWorkspace(params) {
    if (Array.isArray(params.workspaceFolders)) {
      const added = params.workspaceFolders.map((f, i) => ({ uri: Uri.file(f), name: path.basename(f), index: i }));
      hostState.workspaceFolders = params.workspaceFolders;
      onDidChangeFoldersEmitter.fire({ added, removed: [] });
    }
    if (params.themeKind && params.themeKind !== hostState.themeKind) {
      hostState.themeKind = params.themeKind;
      onDidChangeThemeEmitter.fire({ kind: hostState.themeKind });
    }
    if (typeof params.focused === 'boolean') {
      vscodeApi.window.state.focused = params.focused;
      vscodeApi.window.state.active = params.focused;
      onDidChangeWinStateEmitter.fire(vscodeApi.window.state);
    }
    return { ok: true };
  },
  async shutdown() {
    for (const [key, entry] of hostState.activated.entries()) {
      try {
        if (entry.mod && typeof entry.mod.deactivate === 'function') await Promise.resolve(entry.mod.deactivate());
        for (const sub of entry.context.subscriptions || []) {
          if (sub && typeof sub.dispose === 'function') sub.dispose();
        }
      } catch (e) { console.error('deactivate ' + key + ': ' + e); }
    }
    setTimeout(() => process.exit(0), 20);
    return { ok: true };
  },
};

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', async (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  let req;
  try { req = JSON.parse(trimmed); } catch { return; }
  const fn = handlers[req.method];
  if (!fn) {
    if (req.id !== undefined) reply(req.id, undefined, 'Unknown method: ' + req.method);
    return;
  }
  try {
    const res = await fn(req.params || {});
    if (req.id !== undefined) reply(req.id, res === undefined ? null : res);
  } catch (err) {
    if (req.id !== undefined) reply(req.id, undefined, err && err.message ? err.message : String(err));
  }
});
rl.on('close', () => process.exit(0));

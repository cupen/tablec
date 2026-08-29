// tablec webui — Lit-powered Web Components SPA.
//
// All components extend `LitElement`. State lives in a single `store`
// object; mutations go through helpers that call `notify()`, which
// dispatches a 'change' event on a shared `EventTarget`. Every
// component subscribes via `StoreSub` and re-renders on change.
//
// Lit handles: shadow root creation, scoped styles, reactive property
// updates, declarative templates. We get cleaner code than vanilla
// `attachShadow` + manual `render()`.
//
// Visual system: dark "Inkwell" / light "Blueprint" via [data-theme].
// Components reference tokens by role (--bg / --surface / --accent),
// so theme flips just by setting an attribute on <html> — no per-
// component JS re-render needed. Only the theme-toggle button re-
// renders to swap sun/moon icon.

import { LitElement, html, css } from '/static/vendor/lit.js';

// =============================================================================
// store + bus
// =============================================================================

const store = {
  dir: '.',
  files: [],            // [{ name, path, size, modified_secs }]
  selectedPath: null,
  sheets: [],           // [{ name, row_count, col_count }]
  activeSheet: null,
  preview: null,        // legacy raw grid (from /api/preview) — used by raw view toggle
  parsed: null,         // { schema, rows, summary, diagnostics } from /api/parsed_preview
  selectedCell: null,   // { row, col } in 0-indexed grid coords (parsed view: data-row offset)
  parserNames: [],
  activeParser: 'standard',
  configPath: null,
  previewMode: 'parsed', // 'parsed' (default) or 'raw'
  busy: false,
  lastResult: null,     // { kind, status, payload }
};

const bus = new EventTarget();
const notify = () => bus.dispatchEvent(new Event('change'));

// Subscribe a LitElement to store changes. Drop this on every component
// that reads from `store`; re-renders are auto-batched.
class StoreSub {
  constructor(host) {
    host.addController(this);
    this.host = host;
  }
  hostConnected() {
    this._onChange = () => this.host.requestUpdate();
    bus.addEventListener('change', this._onChange);
  }
  hostDisconnected() {
    bus.removeEventListener('change', this._onChange);
  }
}

// =============================================================================
// theme controller — flips [data-theme] on <html>, persists to localStorage.
// Components don't need to re-render on theme change (CSS vars flip), but the
// toggle button does to swap sun/moon icons.
// =============================================================================

const THEME_KEY = 'tablec-theme';

class ThemeCtrl {
  constructor(host) {
    host.addController(this);
    this.host = host;
    this.theme = 'dark';
  }
  hostConnected() {
    let stored = null;
    try { stored = localStorage.getItem(THEME_KEY); } catch { /* private mode */ }
    if (stored !== 'dark' && stored !== 'light') stored = 'dark';
    this.theme = stored;
    document.documentElement.setAttribute('data-theme', this.theme);
  }
  toggle() {
    this.theme = this.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', this.theme);
    try { localStorage.setItem(THEME_KEY, this.theme); } catch { /* ignore */ }
    this.host.requestUpdate();
  }
}

// =============================================================================
// helpers
// =============================================================================

// 0 → A, 25 → Z, 26 → AA, 51 → AZ, 52 → BA (0-indexed spreadsheet cols).
function colLetter(idx) {
  let s = '';
  let n = idx;
  while (true) {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
    if (n < 0) break;
  }
  return s;
}

function extOf(name) {
  const i = name.lastIndexOf('.');
  return i >= 0 ? name.slice(i + 1).toLowerCase() : '';
}

function baseName(p) {
  if (!p) return '';
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
}

function truncErr(e) {
  const s = String(e);
  return s.length > 60 ? s.slice(0, 57) + '…' : s;
}

function humanSize(n) {
  const u = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
}

// Render a cell to a Lit template fragment. Handles both untagged (bare
// `1.0`, `"alice"`) and tagged (`{Float: 1.0}`, `{Bool: true}`) shapes.
function renderCell(cell) {
  if (cell == null) return html`<span class="null">·</span>`;
  if (typeof cell === 'number') return html`${cell}`;
  if (typeof cell === 'string') return html`${cell}`;
  if (typeof cell === 'boolean') return html`<span class="bool">${cell ? '✓' : '✗'}</span>`;
  if (typeof cell === 'object') {
    if ('Float' in cell) return html`${cell.Float}`;
    if ('Bool' in cell) return html`<span class="bool">${cell.Bool ? '✓' : '✗'}</span>`;
    if ('Str' in cell) return html`${cell.Str}`;
    if ('DateTime' in cell) return html`${String(cell.DateTime)}`;
    if ('Duration' in cell) return html`${String(cell.Duration)}`;
    return html`${JSON.stringify(cell)}`;
  }
  return html`${String(cell)}`;
}

// CSS class for a cell based on its runtime type.
function cellClass(cell) {
  if (cell == null) return 'null';
  if (typeof cell === 'number') return 'num';
  if (typeof cell === 'boolean') return 'bool';
  if (typeof cell === 'object' && 'Float' in cell) return 'num';
  if (typeof cell === 'object' && 'Bool' in cell) return 'bool';
  return '';
}

// =============================================================================
// icons — inline SVG, themed via currentColor
// =============================================================================

const SUN_ICON = html`
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <circle cx="12" cy="12" r="4"/>
    <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l1.41-1.41M17.66 6.34l1.41-1.41"/>
  </svg>`;

const MOON_ICON = html`
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
  </svg>`;

const RELOAD_ICON = html`
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M3 12a9 9 0 0 1 15.5-6.3L21 8"/>
    <path d="M21 3v5h-5"/>
    <path d="M21 12a9 9 0 0 1-15.5 6.3L3 16"/>
    <path d="M3 21v-5h5"/>
  </svg>`;

// =============================================================================
// <app-shell> — top-level layout
// =============================================================================

class AppShell extends LitElement {
  static styles = css`
    :host {
      /* Layout-critical styles (display, grid-template-rows, height, background)
       * are set on the outer 'app-shell' selector in style.css to win the
       * cascade. Here we only style what's safe inside shadow DOM. */
      color: var(--text);
      font: var(--t-13)/1.45 var(--sans);
    }
    header {
      display: flex; align-items: center; gap: 16px;
      padding: 10px 16px;
      background: linear-gradient(180deg, var(--surface-2) 0%, var(--surface) 100%);
      border-bottom: 1px solid var(--rule);
      box-shadow: var(--shadow-1);
    }
    .brand {
      display: flex; align-items: center; gap: 9px;
      font: 600 14px/1 var(--serif);
      letter-spacing: 0.01em;
      color: var(--text);
    }
    .brand .mark {
      display: inline-grid;
      grid-template-columns: repeat(3, 6px);
      grid-template-rows: repeat(2, 6px);
      gap: 1px;
      transition: filter 200ms ease;
    }
    .brand:hover .mark { filter: drop-shadow(0 0 4px var(--accent-soft)); }
    .brand .mark i {
      background: var(--rule-2);
      display: block;
      border-radius: 1px;
    }
    .brand .mark i:nth-child(1) { background: var(--accent); }
    .brand .mark i:nth-child(3) { background: var(--accent-2); }
    .brand .mark i:nth-child(5) { background: var(--text); }
    .brand .ver {
      font: 400 9px/1 var(--mono);
      color: var(--text-2);
      padding: 2px 5px;
      border: 1px solid var(--rule-2);
      border-radius: 2px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .spacer { flex: 1; }
    .dir-display {
      display: inline-flex; align-items: center;
      flex: 1; min-width: 0;
      max-width: 640px;
      font: 400 var(--t-12)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.02em;
      overflow: hidden;
    }
    .dir-display .prefix {
      color: var(--text-3);
      padding-right: 6px;
      flex-shrink: 0;
    }
    .dir-display .path {
      color: var(--text);
      overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
      min-width: 0;
    }
    .icon-btn {
      display: inline-flex; align-items: center; justify-content: center;
      background: transparent;
      color: var(--text-2);
      border: 1px solid var(--rule);
      border-radius: 4px;
      padding: 5px 8px;
      cursor: pointer;
      transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    }
    .icon-btn:hover {
      background: var(--surface-2);
      color: var(--accent);
      border-color: var(--rule-2);
    }
    .icon-btn:focus-visible {
      outline: 2px solid var(--accent);
      outline-offset: 1px;
    }
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      display: flex; gap: 14px;
      letter-spacing: 0.04em;
    }
    .meta b { color: var(--text); font-weight: 500; }
    .theme-toggle {
      display: inline-flex; align-items: center; justify-content: center;
      width: 28px; height: 28px;
      background: transparent;
      color: var(--text-2);
      border: 1px solid var(--rule);
      border-radius: 4px;
      cursor: pointer;
      transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    }
    .theme-toggle:hover {
      background: var(--surface-2);
      color: var(--accent);
      border-color: var(--rule-2);
    }
    .theme-toggle:focus-visible {
      outline: 2px solid var(--accent);
      outline-offset: 1px;
    }
    main {
      display: grid;
      grid-template-columns: 280px minmax(0, 1fr) 340px;
      background: var(--rule);   /* gap color */
      gap: 1px;
      overflow: hidden;
      min-height: 0;
    }
    main > * { background: var(--surface); overflow: auto; min-width: 0; min-height: 0; }
    footer {
      background: var(--surface);
      border-top: 1px solid var(--rule);
      display: flex; align-items: center;
    }
  `;
  _store = new StoreSub(this);
  _theme = new ThemeCtrl(this);

  render() {
    const s = store;
    const dark = this._theme.theme === 'dark';
    return html`
      <header>
        <span class="brand">
          <span class="mark"><i></i><i></i><i></i><i></i><i></i><i></i></span>
          tablec
          <span class="ver">webui</span>
        </span>
        <span class="dir-display" title=${s.dir}>
          <span class="prefix">~/</span><span class="path">${s.dir}</span>
        </span>
        <span class="meta">
          <span>parser <b>${s.activeParser}</b></span>
          <span>cfg <b>${s.configPath ?? '(default)'}</b></span>
        </span>
        <button
          class="icon-btn"
          @click=${() => refreshState()}
          title="重新扫描 (⌘R)"
          aria-label="Reload"
        >
          ${RELOAD_ICON}
        </button>
        <button
          class="theme-toggle"
          @click=${() => this._theme.toggle()}
          title=${dark ? 'Switch to light theme' : 'Switch to dark theme'}
          aria-label="Toggle theme"
        >
          ${dark ? SUN_ICON : MOON_ICON}
        </button>
      </header>
      <main>
        <file-list></file-list>
        <file-preview></file-preview>
        <build-panel></build-panel>
      </main>
      <footer><status-bar></status-bar></footer>
    `;
  }

  firstUpdated() {
    refreshState();
    // Global keyboard shortcuts: ⌘B build · ⌘C check · ⌘R reload · ⌘T theme.
    window.addEventListener('keydown', this._onKey);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.removeEventListener('keydown', this._onKey);
  }

  _onKey = (e) => {
    if (!(e.metaKey || e.ctrlKey)) return;
    const tag = (e.target.tagName || '').toLowerCase();
    if (tag === 'input' || tag === 'select' || tag === 'textarea') return;
    if (e.key === 'b') { e.preventDefault(); document.querySelector('build-panel')?.runAction('build'); }
    else if (e.key === 'c') { e.preventDefault(); document.querySelector('build-panel')?.runAction('check'); }
    else if (e.key === 'r') { e.preventDefault(); refreshState(); }
    else if (e.key === 't') { e.preventDefault(); this._theme.toggle(); }
  };
}
customElements.define('app-shell', AppShell);

// =============================================================================
// <file-list>
// =============================================================================

class FileList extends LitElement {
  static styles = css`
    :host { display: block; }
    .head {
      position: sticky; top: 0; z-index: 2;
      padding: 12px 16px 10px;
      background: linear-gradient(180deg, var(--surface) 0%, var(--surface) 70%, transparent 100%);
      border-bottom: 1px solid var(--rule);
      font: 500 10px/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      display: flex; align-items: center; gap: 9px;
    }
    .head .dot {
      display: inline-block;
      width: 6px; height: 6px;
      background: var(--accent-2);
      border-radius: 1px;
    }
    .head .count {
      margin-left: auto;
      color: var(--text);
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }
    ul { list-style: none; padding: 0; margin: 0; }
    li {
      padding: 10px 16px 10px 14px;
      cursor: pointer;
      border-bottom: 1px solid var(--rule);
      border-left: 2px solid transparent;
      display: flex; flex-direction: column; gap: 4px;
      transition: background 100ms ease, border-color 100ms ease;
    }
    li:hover { background: var(--surface-2); }
    li.selected {
      background: var(--surface-2);
      border-left-color: var(--accent);
    }
    .name {
      font: 500 var(--t-13)/1.2 var(--sans);
      color: var(--text);
      display: flex; align-items: center; gap: 7px;
    }
    .name .ext {
      font: 500 9px/1 var(--mono);
      color: var(--accent-2);
      padding: 2px 5px;
      background: var(--accent-2-soft);
      border: 1px solid var(--rule-2);
      border-radius: 2px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.02em;
    }
    .empty {
      padding: 22px 16px;
      color: var(--text-2);
      font: 400 var(--t-12)/1.55 var(--sans);
      border-left: 2px solid var(--rule);
      margin: 4px 0;
    }
    .empty h3 {
      margin: 0 0 8px;
      font: 500 var(--t-13)/1.3 var(--serif);
      color: var(--text);
      letter-spacing: 0;
      text-transform: none;
    }
    .empty p {
      margin: 0 0 12px;
    }
    .empty .step {
      display: flex; align-items: baseline; gap: 8px;
      padding: 3px 0;
      color: var(--text-2);
    }
    .empty .step b {
      font: 500 var(--t-11)/1 var(--mono);
      color: var(--accent-2);
      width: 14px;
      flex-shrink: 0;
      letter-spacing: 0.04em;
    }
    .empty code {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text);
      background: var(--surface-2);
      padding: 1px 5px;
      border-radius: 2px;
    }
    .empty .hint {
      margin-top: 14px;
      padding-top: 12px;
      border-top: 1px solid var(--rule);
      font: 400 var(--t-11)/1.5 var(--mono);
      color: var(--text-3);
      letter-spacing: 0.02em;
    }
  `;
  _store = new StoreSub(this);

  render() {
    const { files, selectedPath, dir } = store;
    return html`
      <div class="head">
        <span class="dot" aria-hidden="true"></span>
        <span>FILES</span>
        <span class="count">${String(files.length).padStart(2, '0')}</span>
      </div>
      ${files.length === 0
        ? html`<div class="empty">
            <h3>No tables in this directory.</h3>
            <p>Point the bar above at a folder that holds your <code>.xlsx</code> / <code>.xls</code> / <code>.xlsb</code> / <code>.ods</code> files.</p>
            <div class="step"><b>1.</b><span>Type a path above and press 打开</span></div>
            <div class="step"><b>2.</b><span>Files appear here as they're scanned</span></div>
            <div class="step"><b>3.</b><span>Click one to preview and build</span></div>
            <div class="hint">Currently scanning: <code>${dir || '.'}</code></div>
          </div>`
        : html`<ul>${files.map((f) => html`
            <li
              class=${selectedPath === f.path ? 'selected' : ''}
              @click=${() => this._select(f.path)}
            >
              <span class="name">
                ${f.name}
                <span class="ext">${extOf(f.name)}</span>
              </span>
              <span class="meta">${humanSize(f.size)} · ${new Date(f.modified_secs * 1000).toLocaleString()}</span>
            </li>
          `)}</ul>`}
    `;
  }

  _select(path) {
    if (store.selectedPath === path) return;
    store.selectedPath = path;
    store.sheets = [];
    store.activeSheet = null;
    store.preview = null;
    store.selectedCell = null;
    notify();
    // Defer to preview component via custom event for separation.
    document.querySelector('file-preview')?._loadFor(path);
  }
}
customElements.define('file-list', FileList);

// =============================================================================
// <file-preview> — formula bar · sheet tabs · parsed/raw grid · summary
// =============================================================================

// `FieldType` is a serde-tagged enum; default representation is either a bare
// string (for unit variants like "Int32") or `{VariantName: {…}}`. Map either
// form to a short chip label for the schema row.
function typeNameOf(t) {
  if (t == null) return '?';
  if (typeof t === 'string') return t.toLowerCase();
  if (typeof t === 'object') {
    const k = Object.keys(t)[0];
    return k ? k.toLowerCase() : '?';
  }
  return '?';
}

class FilePreview extends LitElement {
  static styles = css`
    :host {
      display: flex; flex-direction: column;
      height: 100%; min-height: 0;
      background: var(--surface);
    }

    /* ---- formula bar (signature) ---- */
    .formula {
      display: grid;
      grid-template-columns: 72px 1fr;
      align-items: stretch;
      border-bottom: 1px solid var(--rule);
      background: var(--surface-2);
      font-family: var(--mono);
    }
    .formula .coord {
      font: 600 var(--t-12)/1 var(--mono);
      color: var(--bg);
      background: var(--accent);
      padding: 10px 12px;
      letter-spacing: 0.05em;
      display: flex; align-items: center; justify-content: center;
      transition: background 100ms ease;
    }
    .formula .coord.muted {
      background: transparent;
      color: var(--text-3);
      border-right: 1px solid var(--rule);
    }
    .formula .fn {
      display: flex; align-items: center; gap: 10px;
      padding: 0 14px;
      font: 400 var(--t-12)/1.4 var(--mono);
      color: var(--text);
      overflow: hidden;
      min-width: 0;
    }
    .formula .fn .src {
      color: var(--text-2);
      font-size: var(--t-11);
      flex-shrink: 0;
      max-width: 50%;
      overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    }
    .formula .fn .sep { color: var(--text-2); }
    .formula .fn .val {
      color: var(--text);
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
      font-variant-numeric: tabular-nums;
    }
    .formula .fn .val.num { color: var(--accent-2); }
    .formula .fn .val.bool { color: var(--accent); }
    .formula .fn .val.empty { color: var(--text-3); font-style: italic; }
    .formula .fn .status {
      margin-left: auto;
      font: 600 9px/1 var(--mono);
      padding: 3px 7px;
      border-radius: 2px;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      flex-shrink: 0;
    }
    .formula .fn .status.ok {
      color: var(--ok);
      background: rgba(127, 176, 105, 0.14);
    }
    .formula .fn .status.err {
      color: var(--err);
      background: rgba(224, 108, 117, 0.16);
    }

    /* ---- tabs (pill row + view-mode toggle) ---- */
    .tabs {
      display: flex; gap: 2px; align-items: center;
      padding: 8px 14px;
      background: var(--surface-2);
      border-bottom: 1px solid var(--rule);
      overflow-x: auto;
      scrollbar-width: none;
    }
    .tabs::-webkit-scrollbar { display: none; }
    .tab {
      padding: 6px 12px;
      cursor: pointer;
      font: 500 var(--t-12)/1 var(--mono);
      color: var(--text-2);
      border-radius: 4px;
      transition: background 100ms, color 100ms;
      white-space: nowrap;
      display: flex; align-items: baseline; gap: 6px;
    }
    .tab:hover { color: var(--text); background: var(--accent-soft); }
    .tab.active {
      color: var(--bg);
      background: var(--text);
      font-weight: 600;
    }
    .tab .size {
      font-size: 9px;
      color: var(--text-3);
      letter-spacing: 0.04em;
      font-weight: 400;
    }
    .tab.active .size { color: var(--text-2); }
    .view-toggle {
      margin-left: auto;
      display: inline-flex;
      border: 1px solid var(--rule);
      border-radius: 4px;
      overflow: hidden;
      flex-shrink: 0;
    }
    .view-toggle button {
      font: 500 10px/1 var(--mono);
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--text-2);
      background: transparent;
      border: none;
      padding: 5px 9px;
      cursor: pointer;
      transition: background 80ms, color 80ms;
    }
    .view-toggle button:hover {
      color: var(--text);
      background: var(--accent-soft);
    }
    .view-toggle button.active {
      color: var(--bg);
      background: var(--text);
      font-weight: 600;
    }
    .view-toggle button + button { border-left: 1px solid var(--rule); }

    /* ---- summary strip (parsed view) ---- */
    .summary {
      display: flex; align-items: center; gap: 14px;
      padding: 6px 14px;
      background: var(--surface-2);
      border-bottom: 1px solid var(--rule);
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.04em;
    }
    .summary .errs { color: var(--err); font-weight: 600; }
    .summary .warns { color: var(--accent); font-weight: 600; }
    .summary .ok { color: var(--ok); font-weight: 600; }
    .summary .meta { margin-left: auto; color: var(--text-3); }

    /* ---- spreadsheet grid ---- */
    .body {
      flex: 1; min-height: 0;
      overflow: auto;
      background: var(--surface-2);
      position: relative;
    }
    table.grid {
      border-collapse: collapse;
      font: 400 var(--t-12)/1.35 var(--mono);
      color: var(--text);
      font-variant-numeric: tabular-nums;
    }
    table.grid th, table.grid td {
      border-right: 1px solid var(--rule);
      border-bottom: 1px solid var(--rule);
      padding: 5px 10px;
      text-align: left;
      white-space: nowrap;
      max-width: 360px;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    table.grid th {
      background: var(--surface);
      color: var(--text-2);
      font-weight: 500;
      position: sticky;
      z-index: 1;
      font-size: var(--t-11);
      letter-spacing: 0.04em;
      user-select: none;
    }
    table.grid tr.letters th { top: 0; z-index: 3; }
    table.grid tr.schema-info th {
      top: 22px;
      z-index: 2;
      background: var(--surface-2);
      text-align: left;
    }
    table.grid tr.schema-info .schema-label {
      font: 500 9px/1 var(--mono);
      color: var(--text-3);
      letter-spacing: 0.14em;
      text-transform: uppercase;
    }
    table.grid tr.schema-info .field-name {
      color: var(--text);
      font-weight: 500;
      font-family: var(--sans);
      letter-spacing: 0;
    }
    table.grid tr.schema-info .col-type {
      display: inline-block;
      font: 500 9px/1 var(--mono);
      color: var(--accent-2);
      padding: 2px 6px;
      margin-left: 6px;
      background: var(--accent-2-soft);
      border: 1px solid var(--rule-2);
      border-radius: 2px;
      letter-spacing: 0.04em;
      vertical-align: middle;
    }
    table.grid th.rowh {
      left: 0;
      z-index: 2;
      text-align: right;
      color: var(--text-2);
      min-width: 44px;
      background: var(--surface);
    }
    table.grid th.corner { z-index: 4; background: var(--surface); color: var(--text-3); }
    table.grid td.cell { cursor: cell; transition: background 60ms; }
    table.grid td.cell:hover { background: var(--accent-soft); }
    table.grid td.selected {
      background: var(--accent-soft);
      outline: 1.5px solid var(--accent);
      outline-offset: -1.5px;
      color: var(--text);
    }
    table.grid td.cell.error {
      background: rgba(224, 108, 117, 0.10);
      outline: 1.5px solid var(--err);
      outline-offset: -1.5px;
      color: var(--err);
      cursor: help;
    }
    table.grid td.cell.error:hover {
      background: rgba(224, 108, 117, 0.18);
    }
    table.grid td.cell.error .err-mark {
      font: 500 9px/1 var(--mono);
      color: var(--err);
      margin-right: 4px;
      letter-spacing: 0.04em;
    }
    table.grid td.num { text-align: right; color: var(--accent-2); }
    table.grid td.bool { color: var(--accent); text-align: center; }
    table.grid td.null { color: var(--text-3); text-align: center; }

    /* ---- empty / hero state (signature amber rule) ---- */
    .empty {
      position: relative;
      padding: 28px 28px 28px 32px;
      margin: 24px;
      max-width: 460px;
      background: var(--surface);
      border: 1px solid var(--rule);
      border-radius: 6px;
      color: var(--text-2);
      font: 400 var(--t-13)/1.6 var(--sans);
      box-shadow: var(--shadow-2);
    }
    .empty::before {
      content: '';
      position: absolute;
      left: 0; top: 0; bottom: 0;
      width: 4px;
      background: var(--accent);
      border-radius: 6px 0 0 6px;
    }
    .empty h3 {
      margin: 0 0 6px;
      font: 500 var(--t-15)/1.3 var(--serif);
      color: var(--text);
      letter-spacing: 0;
      text-transform: none;
    }
    .empty p {
      margin: 0 0 14px;
    }
    .empty .steps {
      list-style: none;
      padding: 0;
      margin: 0 0 14px;
    }
    .empty .steps li {
      display: flex; align-items: baseline; gap: 10px;
      padding: 4px 0;
      color: var(--text-2);
    }
    .empty .steps b {
      font: 600 var(--t-11)/1 var(--mono);
      color: var(--accent);
      width: 16px;
      flex-shrink: 0;
      letter-spacing: 0.04em;
    }
    .empty .hint {
      padding-top: 12px;
      border-top: 1px solid var(--rule);
      font: 400 var(--t-11)/1.5 var(--mono);
      color: var(--text-3);
      letter-spacing: 0.02em;
    }
    .empty code {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text);
      background: var(--surface-2);
      padding: 1px 5px;
      border-radius: 2px;
    }
    .empty.muted {
      padding: 22px 24px;
      margin: 16px;
      max-width: 380px;
    }
    .empty.muted::before { width: 3px; }
    .empty.muted h3 {
      font: 500 var(--t-13)/1.3 var(--sans);
      margin-bottom: 4px;
    }
  `;
  _store = new StoreSub(this);

  connectedCallback() {
    super.connectedCallback();
    this.addEventListener('keydown', this._onKey);
  }

  render() {
    const { selectedPath, sheets, activeSheet, parsed, preview,
            selectedCell, previewMode } = store;

    // Resolve coordinate + value for the formula bar based on current mode.
    let coord = null;
    let cellValue = null;
    let cellError = null;
    if (selectedCell) {
      if (previewMode === 'parsed' && parsed?.rows?.length) {
        const row = parsed.rows[selectedCell.row];
        const cell = row?.cells?.[selectedCell.col];
        if (row) coord = colLetter(selectedCell.col) + row.line;
        cellValue = cell?.value;
        cellError = cell?.error;
      } else if (previewMode === 'raw' && preview?.rows?.length) {
        coord = colLetter(selectedCell.col) + (selectedCell.row + 1);
        cellValue = preview.rows[selectedCell.row]?.[selectedCell.col];
      }
    }

    return html`
      <div class="formula">
        <div class="coord ${coord ? '' : 'muted'}">${coord ?? '—'}</div>
        <div class="fn">
          <span class="src">${baseName(selectedPath || '')} · ${activeSheet ?? ''}</span>
          ${coord ? html`<span class="sep">▸</span>` : null}
          <span class="val ${cellClass(cellValue) || 'empty'}">
            ${coord ? this._renderFormulaValue(cellValue, previewMode) : '点击任意单元格查看坐标'}
          </span>
          ${previewMode === 'parsed' && coord ? html`
            <span class="status ${cellError ? 'err' : 'ok'}">
              ${cellError ? '⚠ err' : '✓ ok'}
            </span>
          ` : null}
        </div>
      </div>

      <div class="tabs">
        ${sheets.length === 0 ? null : sheets.map((s) => html`
          <div
            class="tab ${activeSheet === s.name ? 'active' : ''}"
            @click=${() => this._selectSheet(s.name)}
          >
            <span>${s.name}</span>
            <span class="size">${s.row_count ?? '?'}×${s.col_count ?? '?'}</span>
          </div>
        `)}
        ${sheets.length > 0 ? html`
          <div class="view-toggle" role="tablist" aria-label="preview mode">
            <button
              class=${previewMode === 'parsed' ? 'active' : ''}
              @click=${() => this._setMode('parsed')}
              title="Schema + per-cell validation"
            >Parsed</button>
            <button
              class=${previewMode === 'raw' ? 'active' : ''}
              @click=${() => this._setMode('raw')}
              title="Raw cells from the file"
            >Raw</button>
          </div>
        ` : null}
      </div>

      ${previewMode === 'parsed' && parsed ? this._renderSummary(parsed) : null}

      <div class="body">
        ${this._renderBody()}
      </div>
    `;
  }

  _renderSummary(parsed) {
    const s = parsed.summary;
    const errCls = s.error_count > 0 ? 'errs' : 'ok';
    const warnCls = s.warning_count > 0 ? 'warns' : '';
    return html`
      <div class="summary">
        <span>${s.shown_rows} / ${s.data_rows} rows</span>
        <span class=${errCls}>${s.error_count} err</span>
        <span class=${warnCls}>${s.warning_count} warn</span>
        <span class="meta">schema · line ${parsed.data_start_row}</span>
      </div>
    `;
  }

  _renderFormulaValue(cell, mode) {
    if (cell == null) return html`<span style="font-style:italic">∅ empty</span>`;
    // Parsed mode: `cell` is a plain JSON value (number / string / bool / object).
    if (mode === 'parsed') {
      if (typeof cell === 'number') return html`${cell}`;
      if (typeof cell === 'boolean') return html`${cell ? 'TRUE' : 'FALSE'}`;
      if (typeof cell === 'string') return html`"${cell}"`;
      return html`${JSON.stringify(cell)}`;
    }
    // Raw mode: legacy tagged-enum shape from /api/preview.
    if (typeof cell === 'number') return html`${cell}`;
    if (typeof cell === 'string') return html`"${cell}"`;
    if (typeof cell === 'boolean') return html`${cell ? 'TRUE' : 'FALSE'}`;
    if (typeof cell === 'object') {
      if ('Float' in cell) return html`${cell.Float}`;
      if ('Bool' in cell) return html`${cell.Bool ? 'TRUE' : 'FALSE'}`;
      if ('Str' in cell) return html`"${cell.Str}"`;
      if ('DateTime' in cell) return html`${String(cell.DateTime)}`;
      if ('Duration' in cell) return html`${String(cell.Duration)}`;
      return html`${JSON.stringify(cell)}`;
    }
    return html`${String(cell)}`;
  }

  _renderBody() {
    const { selectedPath, sheets, parsed, preview, previewMode } = store;
    if (!selectedPath) {
      return html`<div class="empty">
        <h3>Pick a file to preview.</h3>
        <p>Three quick steps to see your data laid out — typed and validated:</p>
        <ol class="steps">
          <li><b>1.</b><span>Open a directory above</span></li>
          <li><b>2.</b><span>Pick a file from the list on the left</span></li>
          <li><b>3.</b><span>Cells appear here as a parsed grid — click any to inspect</span></li>
        </ol>
        <div class="hint">Supports <code>.xlsx</code> · <code>.xls</code> · <code>.xlsb</code> · <code>.ods</code></div>
      </div>`;
    }
    if (sheets.length === 0) {
      return html`<div class="empty muted">
        <h3>No sheets in this file.</h3>
        <p>The file exists, but we couldn't find any tables inside. It may be empty or in an unsupported format.</p>
        <div class="hint">Path: <code>${baseName(selectedPath)}</code></div>
      </div>`;
    }
    if (previewMode === 'parsed') {
      if (!parsed) return html`<div class="empty muted"><h3>Loading…</h3></div>`;
      return this._renderParsedBody(parsed);
    }
    if (!preview) return html`<div class="empty muted"><h3>Loading…</h3></div>`;
    return this._renderRawBody(preview);
  }

  _renderParsedBody(parsed) {
    const schema = parsed.schema;
    if (!schema || !schema.fields?.length) {
      return html`<div class="empty muted">
        <h3>No schema parsed.</h3>
        <p>The schema parser didn't recognize this sheet. It may start with <code>#</code>, or the first row isn't field names.</p>
        <div class="hint">Switch to <b>Raw</b> above to see the cells as-is.</div>
      </div>`;
    }
    const fields = schema.fields;
    const rows = parsed.rows;
    if (!rows.length) {
      return html`<div class="empty muted">
        <h3>Schema OK, no data rows.</h3>
        <p>Columns are declared, but rows after line ${parsed.data_start_row} are empty.</p>
      </div>`;
    }
    const ncols = fields.length;
    const sel = store.selectedCell;
    const trs = [];

    // Header row 1: column letters
    const headCells = [html`<th class="corner"></th>`];
    for (let c = 0; c < ncols; c++) headCells.push(html`<th>${colLetter(c)}</th>`);
    trs.push(html`<tr class="letters">${headCells}</tr>`);

    // Header row 2: schema info — field name + type chip per column
    const schemaCells = [html`<th class="rowh"><span class="schema-label">schema</span></th>`];
    for (let c = 0; c < ncols; c++) {
      const f = fields[c];
      schemaCells.push(html`<th>
        <span class="field-name">${f.name}</span><span class="col-type">${typeNameOf(f.t)}</span>
      </th>`);
    }
    trs.push(html`<tr class="schema-info">${schemaCells}</tr>`);

    // Data rows
    rows.forEach((row, ri) => {
      const cells = [html`<th class="rowh">${row.line}</th>`];
      for (let c = 0; c < ncols; c++) {
        const cell = row.cells[c];
        if (!cell) continue;
        const isSelected = sel && sel.row === ri && sel.col === c;
        const cls = [
          'cell',
          cell.error ? 'error' : '',
          cellClassTyped(cell.value),
          isSelected ? 'selected' : '',
        ].filter(Boolean).join(' ');
        const title = cell.error
          ? `${cell.error} (raw: "${cell.raw || '∅'}")`
          : '';
        cells.push(html`
          <td
            tabindex="-1"
            class=${cls}
            data-row=${ri}
            data-col=${c}
            title=${title}
            @click=${(e) => this._selectCell(ri, c, e)}
          >${this._renderTypedCell(cell)}</td>
        `);
      }
      trs.push(html`<tr>${cells}</tr>`);
    });

    return html`<table class="grid"><tbody>${trs}</tbody></table>`;
  }

  _renderTypedCell(cell) {
    if (cell.error) {
      return html`<span class="err-mark">⚠</span><span>${cell.raw || '∅'}</span>`;
    }
    const v = cell.value;
    if (v === undefined || v === null) {
      return html`<span class="null">·</span>`;
    }
    if (typeof v === 'number') return html`${v}`;
    if (typeof v === 'boolean') return html`<span class="bool">${v ? '✓' : '✗'}</span>`;
    if (typeof v === 'string') return html`${v}`;
    if (Array.isArray(v)) return html`${JSON.stringify(v)}`;
    if (typeof v === 'object') return html`${JSON.stringify(v)}`;
    return html`${String(v)}`;
  }

  _renderRawBody(preview) {
    const rows = preview.rows || [];
    if (rows.length === 0) {
      return html`<div class="empty muted"><h3>Empty sheet.</h3><p>This sheet has no rows.</p></div>`;
    }
    const ncols = Math.max(...rows.map((r) => r.length), 1);
    const sel = store.selectedCell;

    const trs = [];
    const headCells = [html`<th class="corner"></th>`];
    for (let c = 0; c < ncols; c++) headCells.push(html`<th>${colLetter(c)}</th>`);
    trs.push(html`<tr class="letters">${headCells}</tr>`);

    rows.forEach((row, ri) => {
      const cells = [html`<th class="rowh">${ri + 1}</th>`];
      for (let c = 0; c < ncols; c++) {
        const cell = row[c];
        const isSelected = sel && sel.row === ri && sel.col === c;
        const cls = [
          'cell',
          cellClass(cell),
          isSelected ? 'selected' : '',
        ].filter(Boolean).join(' ');
        cells.push(html`
          <td
            tabindex="-1"
            class=${cls}
            data-row=${ri}
            data-col=${c}
            @click=${(e) => this._selectCell(ri, c, e)}
          >${renderCell(cell)}</td>
        `);
      }
      trs.push(html`<tr>${cells}</tr>`);
    });

    return html`<table class="grid"><tbody>${trs}</tbody></table>`;
  }

  _selectCell(row, col, e) {
    store.selectedCell = { row, col };
    notify();
    e.currentTarget.focus({ preventScroll: true });
  }

  _selectSheet(name) {
    if (store.activeSheet === name) return;
    store.activeSheet = name;
    store.selectedCell = null;
    store.parsed = null;
    store.preview = null;
    notify();
    this._loadActive();
  }

  _setMode(mode) {
    if (store.previewMode === mode) return;
    store.previewMode = mode;
    store.selectedCell = null;
    notify();
    if (mode === 'raw' && !store.preview) this._loadRaw();
    else if (mode === 'parsed' && !store.parsed) this._loadParsed();
  }

  async _loadFor(path) {
    store.selectedPath = path;
    store.sheets = [];
    store.activeSheet = null;
    store.preview = null;
    store.parsed = null;
    store.selectedCell = null;
    notify();
    try {
      const sheets = await getJson(`/api/sheets?path=${encodeURIComponent(path)}`);
      store.sheets = sheets;
      if (sheets.length) {
        store.activeSheet = sheets[0].name;
        notify();
        await this._loadActive();
      } else {
        notify();
      }
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  async _loadActive() {
    // Always keep both views fresh so toggling is instant.
    await Promise.all([this._loadParsed(), this._loadRaw()]);
  }

  async _loadParsed() {
    if (!store.selectedPath || !store.activeSheet) return;
    try {
      const pp = await getJson(
        `/api/parsed_preview?path=${encodeURIComponent(store.selectedPath)}` +
        `&sheet=${encodeURIComponent(store.activeSheet)}` +
        `&parser=${encodeURIComponent(store.activeParser)}` +
        `&max_rows=120`
      );
      store.parsed = pp;
      if (pp.rows?.length && !store.selectedCell) {
        store.selectedCell = { row: 0, col: 0 };
      }
      notify();
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  async _loadRaw() {
    if (!store.selectedPath || !store.activeSheet) return;
    try {
      const grid = await getJson(
        `/api/preview?path=${encodeURIComponent(store.selectedPath)}` +
        `&sheet=${encodeURIComponent(store.activeSheet)}&max_rows=120`
      );
      store.preview = grid;
      notify();
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  _onKey = (e) => {
    const isParsed = store.previewMode === 'parsed';
    const rows = isParsed ? store.parsed?.rows : store.preview?.rows;
    if (!store.selectedCell || !rows?.length) return;
    const sel = store.selectedCell;
    const nrows = rows.length;
    const ncols = isParsed
      ? (store.parsed.schema?.fields?.length || 1)
      : Math.max(...rows.map((r) => r.length), 1);
    let { row, col } = sel;
    let handled = true;
    if (e.key === 'ArrowDown' && row < nrows - 1) row++;
    else if (e.key === 'ArrowUp' && row > 0) row--;
    else if (e.key === 'ArrowRight' && col < ncols - 1) col++;
    else if (e.key === 'ArrowLeft' && col > 0) col--;
    else if (e.key === 'Home') col = 0;
    else if (e.key === 'End') col = ncols - 1;
    else if (e.key === 'PageDown' && row < nrows - 1) row = Math.min(row + 10, nrows - 1);
    else if (e.key === 'PageUp' && row > 0) row = Math.max(row - 10, 0);
    else handled = false;
    if (handled) {
      e.preventDefault();
      store.selectedCell = { row, col };
      notify();
    }
  };
}

// CSS class for a typed cell value (parsed view).
function cellClassTyped(v) {
  if (v == null) return '';
  if (typeof v === 'number') return 'num';
  if (typeof v === 'boolean') return 'bool';
  return '';
}

customElements.define('file-preview', FilePreview);

// =============================================================================
// <build-panel> — Configuration · Actions · Output zones
// =============================================================================

class BuildPanel extends LitElement {
  static styles = css`
    :host { display: block; padding: 14px; color: var(--text); }
    .head {
      font: 500 10px/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      margin: 0 0 14px;
      display: flex; align-items: center; gap: 9px;
    }
    .head .dot {
      display: inline-block;
      width: 6px; height: 6px;
      background: var(--ok);
      border-radius: 1px;
    }
    .zone-label {
      font: 500 9px/1 var(--mono);
      color: var(--text-3);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      margin: 16px 0 8px;
    }
    .zone-label:first-of-type { margin-top: 0; }
    .zone-sep {
      height: 1px;
      background: var(--rule);
      margin: 14px -14px 0;
    }
    .group { margin-bottom: 10px; }
    label.row {
      display: flex; gap: 8px; align-items: center;
      font: 500 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      margin-bottom: 6px;
      letter-spacing: 0.04em;
    }
    label.row b { color: var(--text); font-weight: 500; }
    select {
      width: 100%;
      font: 400 var(--t-12)/1 var(--mono);
      color: var(--text);
      background: var(--surface-2);
      border: 1px solid var(--rule);
      padding: 6px 8px;
      border-radius: 4px;
      outline: none;
      appearance: none;
      cursor: pointer;
      transition: border-color 100ms ease, box-shadow 100ms ease;
    }
    select:focus {
      border-color: var(--accent);
      box-shadow: 0 0 0 1px var(--accent-glow);
    }
    .opts {
      display: grid; grid-template-columns: 1fr;
      gap: 8px;
      font: 400 var(--t-12)/1 var(--mono);
    }
    .opts label {
      display: flex; gap: 9px; align-items: center;
      color: var(--text-2);
      cursor: pointer;
      user-select: none;
    }
    .opts input[type=checkbox] {
      appearance: none;
      width: 13px; height: 13px;
      background: var(--surface-2);
      border: 1px solid var(--rule-2);
      border-radius: 3px;
      display: inline-grid; place-items: center;
      cursor: pointer;
      transition: background 100ms, border-color 100ms;
    }
    .opts input[type=checkbox]:hover { border-color: var(--accent); }
    .opts input[type=checkbox]:checked {
      background: var(--accent);
      border-color: var(--accent);
    }
    .opts input[type=checkbox]:checked::after {
      content: '';
      width: 5px; height: 2.5px;
      border-left: 1.5px solid var(--bg);
      border-bottom: 1.5px solid var(--bg);
      transform: translateY(-1px) rotate(-45deg);
    }
    .opts label:has(input:checked) { color: var(--text); }

    .actions {
      display: grid; grid-template-columns: 1fr 1fr 1fr;
      gap: 6px;
    }
    button.act {
      font: 600 var(--t-11)/1 var(--mono);
      letter-spacing: 0.06em;
      color: var(--bg);
      border: 1px solid;
      padding: 9px 6px;
      cursor: pointer;
      border-radius: 4px;
      text-transform: uppercase;
      transition: filter 100ms, transform 80ms;
      display: flex; flex-direction: column; align-items: center; gap: 3px;
    }
    button.act:hover:not(:disabled) { filter: brightness(1.08); }
    button.act:active:not(:disabled) { transform: translateY(1px); }
    button.act:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
    button.act:disabled { opacity: .4; cursor: progress; }
    button.act[data-kind=build] {
      background: var(--accent); border-color: var(--accent);
    }
    button.act[data-kind=check] {
      background: var(--accent-2); border-color: var(--accent-2);
    }
    button.act[data-kind=validate] {
      background: transparent;
      border-color: var(--rule-2);
      color: var(--text-2);
      border-style: dashed;
    }
    button.act .hint {
      font: 400 9px/1 var(--mono);
      opacity: .55;
      letter-spacing: 0.08em;
    }

    .live {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--accent-2);
      display: flex; align-items: center; gap: 7px;
      margin-top: 10px;
      height: 14px;
      letter-spacing: 0.04em;
    }
    .live .dot {
      width: 7px; height: 7px;
      background: var(--accent-2);
      border-radius: 50%;
      box-shadow: 0 0 6px var(--accent-2);
      animation: pulse 1.2s ease-in-out infinite;
    }
    @keyframes pulse { 50% { opacity: .3; } }
    .live.off { color: var(--text-3); }
    .live.off .dot { background: var(--text-3); box-shadow: none; animation: none; }

    /* ---- result card ---- */
    .out {
      font: 400 var(--t-11)/1.5 var(--mono);
      background: var(--surface-2);
      border: 1px solid var(--rule);
      color: var(--text);
      padding: 10px 12px;
      border-radius: 4px;
    }
    .out .row { display: flex; gap: 10px; padding: 2px 0; }
    .out .k { color: var(--text-2); flex-shrink: 0; width: 64px; }
    .out .v { color: var(--text); word-break: break-all; }
    .out .n { color: var(--accent-2); font-variant-numeric: tabular-nums; }
    .out .s { color: var(--accent); }
    .out .e { color: var(--err); }
    .out .ok { color: var(--ok); }
    .out .sep {
      height: 1px; background: var(--rule); margin: 6px 0;
    }
    .out .placeholder { color: var(--text-3); font-style: italic; }

    .diag-list {
      list-style: none; padding: 0; margin: 10px 0 0;
      max-height: 240px; overflow: auto;
    }
    .diag {
      padding: 7px 10px 7px 12px;
      border-left: 3px solid var(--rule-2);
      margin-bottom: 3px;
      font: 400 var(--t-11)/1.4 var(--mono);
      background: var(--surface-2);
      color: var(--text);
      border-radius: 0 3px 3px 0;
      display: flex; flex-direction: column; gap: 2px;
    }
    .diag.error { border-left-color: var(--err); }
    .diag.warning { border-left-color: var(--accent); }
    .diag.note { border-left-color: var(--accent-2); }
    .diag .head { display: flex; gap: 8px; align-items: baseline; }
    .diag .sev {
      font-size: 9px;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--text-2);
      font-weight: 600;
    }
    .diag.error .sev { color: var(--err); }
    .diag.warning .sev { color: var(--accent); }
    .diag .code { color: var(--accent-2); font-weight: 500; }
    .diag .msg { color: var(--text); }
    .diag .where { color: var(--text-2); font-size: 10px; margin-top: 1px; }
  `;
  _store = new StoreSub(this);

  render() {
    const { parserNames, activeParser, busy, lastResult } = store;
    return html`
      <div class="head">
        <span class="dot" aria-hidden="true"></span>
        <span>BUILD &amp; CHECK</span>
      </div>

      <div class="zone-label">Configuration</div>
      <div class="group">
        <label class="row"><b>format</b></label>
        <select id="fmt">
          <option value="json">json</option>
          <option value="json-pretty" selected>json-pretty</option>
          <option value="msgpack">msgpack</option>
        </select>
      </div>
      <div class="group">
        <label class="row"><b>parser</b></label>
        <select id="parser">
          ${parserNames.map((n) => html`
            <option value=${n} ?selected=${n === activeParser}>${n}</option>
          `)}
        </select>
      </div>
      <div class="opts">
        <label><input type="checkbox" id="pretty"> pretty</label>
        <label><input type="checkbox" id="includeFields"> include_fields</label>
        <label><input type="checkbox" id="write"> write to disk</label>
      </div>

      <div class="zone-sep"></div>
      <div class="zone-label">Actions</div>
      <div class="actions">
        <button class="act" data-kind="build" ?disabled=${busy} @click=${() => this.runAction('build')}>
          Build
          <span class="hint">⌘B</span>
        </button>
        <button class="act" data-kind="check" ?disabled=${busy} @click=${() => this.runAction('check')}>
          Check
          <span class="hint">⌘C</span>
        </button>
        <button class="act" data-kind="validate" ?disabled=${busy} @click=${() => this.runAction('validate')}>
          Validate
          <span class="hint">501</span>
        </button>
      </div>

      <div class="live ${busy ? '' : 'off'}">
        <span class="dot"></span>
        <span>${busy ? 'running…' : 'idle'}</span>
      </div>

      <div class="zone-sep"></div>
      <div class="zone-label">Output</div>
      <div class="out">${this._renderResult(lastResult)}</div>
      ${this._renderDiagnostics(lastResult)}
    `;
  }

  _renderResult(last) {
    if (!last) {
      return html`<div class="placeholder">Idle. ⌘B to build, ⌘C to check.</div>`;
    }
    if (last.kind === 'validate') {
      return html`
        <div class="row"><span class="k">status</span><span class="e">501 todo</span></div>
        <div class="sep"></div>
        <div class="row"><span class="k">note</span><span class="v">数据校验功能仍在研究中</span></div>
      `;
    }
    if (last.error) {
      return html`<div class="row"><span class="k">error</span><span class="e">${last.error}</span></div>`;
    }
    const p = last.payload || {};
    const diags = p.diagnostics || [];
    const nErr = diags.filter((d) => (d.severity || 'Error') === 'Error').length;
    const nWarn = diags.filter((d) => d.severity === 'Warning').length;
    const ok = last.status >= 200 && last.status < 300 && nErr === 0;
    return html`
      <div class="row"><span class="k">kind</span><span class="v">${last.kind}</span></div>
      <div class="row"><span class="k">status</span>
        <span class="v ${ok ? 'ok' : 'e'}">${last.status}${ok ? ' ok' : ''}</span>
      </div>
      <div class="row"><span class="k">duration</span><span class="n">${p.duration_ms ?? '—'} ms</span></div>
      ${p.bytes != null ? html`
        <div class="row"><span class="k">bytes</span><span class="n">${p.bytes}</span></div>
      ` : null}
      ${p.output_path ? html`
        <div class="row"><span class="k">output</span><span class="s">${p.output_path}</span></div>
      ` : null}
      <div class="sep"></div>
      <div class="row">
        <span class="k">diagnostics</span>
        <span class="v">
          <span class="n">${diags.length}</span>
          (<span class="e">${nErr} err</span> ·
           <span class="s">${nWarn} warn</span>)
        </span>
      </div>
      ${p.preview_first_500 ? html`
        <div class="sep"></div>
        <div class="row"><span class="k">preview</span><span class="v">${p.preview_first_500}</span></div>
      ` : null}
    `;
  }

  _renderDiagnostics(last) {
    const diags = last?.payload?.diagnostics || [];
    if (diags.length === 0) return null;
    return html`
      <ul class="diag-list">
        ${diags.map((d) => {
          const loc = d.location || {};
          const where = [loc.file, loc.sheet, loc.line, loc.column].filter(Boolean).join(':');
          return html`
            <li class="diag ${(d.severity || 'Error').toLowerCase()}">
              <div class="head">
                <span class="sev">${d.severity || 'error'}</span>
                <span class="code">${d.code || ''}</span>
              </div>
              <div class="msg">${d.message || ''}</div>
              ${where ? html`<div class="where">${where}</div>` : null}
            </li>
          `;
        })}
      </ul>
    `;
  }

  async runAction(kind) {
    if (store.busy) return;
    store.busy = true;
    store.lastResult = { kind, status: 0, payload: null };
    notify();

    let url, body;
    if (kind === 'build') {
      url = '/api/build';
      const fmt = this.renderRoot.getElementById('fmt').value;
      const pretty = this.renderRoot.getElementById('pretty').checked || fmt === 'json-pretty';
      const includeFields = this.renderRoot.getElementById('includeFields').checked;
      const write = this.renderRoot.getElementById('write').checked;
      const parser = this.renderRoot.getElementById('parser').value;
      body = JSON.stringify({
        dir: store.dir,
        format: fmt,
        pretty,
        include_fields: includeFields,
        write,
        parser,
        plugin_paths: [],
      });
    } else if (kind === 'check') {
      url = '/api/check';
      const parser = this.renderRoot.getElementById('parser').value;
      body = JSON.stringify({ dir: store.dir, parser, plugin_paths: [] });
    } else {
      url = '/api/validate';
      body = '{}';
    }

    try {
      const r = await fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body,
      });
      const text = await r.text();
      let payload;
      try { payload = JSON.parse(text); } catch { payload = { raw: text }; }
      store.lastResult = { kind, status: r.status, payload };
    } catch (e) {
      store.lastResult = { kind, error: String(e) };
    } finally {
      store.busy = false;
      notify();
    }
  }
}
customElements.define('build-panel', BuildPanel);

// =============================================================================
// <status-bar>
// =============================================================================

class StatusBar extends LitElement {
  static styles = css`
    :host {
      flex: 1;
      display: flex; gap: 0; align-items: center;
      padding: 0 16px;
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.04em;
    }
    .seg {
      display: flex; gap: 7px; align-items: center;
      padding: 0 14px;
      border-right: 1px solid var(--rule);
    }
    .seg:first-child { padding-left: 0; }
    .seg:last-child { border-right: none; }
    .seg b { color: var(--text); font-weight: 500; }
    .seg .live-dot {
      width: 6px; height: 6px;
      background: var(--text-3);
      border-radius: 50%;
    }
    .seg.busy .live-dot { background: var(--accent-2); box-shadow: 0 0 4px var(--accent-2); animation: blink 1.2s ease-in-out infinite; }
    @keyframes blink { 50% { opacity: .3; } }
    .seg.ok .live-dot { background: var(--ok); }
    .seg.err .live-dot { background: var(--err); }
    .spacer { flex: 1; }
    .right { color: var(--text-3); padding-right: 0; border-right: none; }
  `;
  _store = new StoreSub(this);

  render() {
    const { dir, sheets, lastResult, busy } = store;
    const last = lastResult;
    let lastCls = '';
    let lastText = '—';
    if (busy) {
      lastCls = 'busy';
      lastText = 'running…';
    } else if (last) {
      if (last.error) { lastCls = 'err'; lastText = `error · ${truncErr(last.error)}`; }
      else if (last.status === 501) { lastCls = 'err'; lastText = 'validate · 501 todo'; }
      else if (last.status >= 200 && last.status < 300) {
        lastCls = last.kind === 'validate' ? 'err' : 'ok';
        const dur = last.payload?.duration_ms != null ? `${last.payload.duration_ms}ms` : `${last.status}`;
        lastText = `${last.kind} · ${dur}`;
      } else {
        lastCls = 'err';
        lastText = `${last.kind} · ${last.status}`;
      }
    }
    return html`
      <span class="seg"><span class="live-dot"></span><span>dir</span><b>${dir}</b></span>
      <span class="seg"><span>sheets</span><b>${String(sheets.length).padStart(2, '0')}</b></span>
      <span class="seg ${lastCls}"><span class="live-dot"></span><span>last</span><b>${lastText}</b></span>
      <span class="spacer"></span>
      <span class="seg right">tablec · webui</span>
    `;
  }
}
customElements.define('status-bar', StatusBar);

// =============================================================================
// fetch helpers
// =============================================================================

async function getJson(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}

async function refreshState() {
  try {
    const s = await getJson('/api/state');
    store.dir = s.dir;
    store.parserNames = s.parser_names || [];
    store.activeParser = s.active_parser || 'standard';
    store.configPath = s.config_path;
    notify();
  } catch (e) { console.error('refreshState', e); }
  try {
    const files = await getJson(`/api/files?dir=${encodeURIComponent(store.dir)}`);
    store.files = files;
    notify();
  } catch (e) { console.error('files', e); }
}
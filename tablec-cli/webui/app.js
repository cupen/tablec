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
// Aesthetic: "Lab Notebook" — dark + quiet + amber/cyan. Signature
// element is the cell-coordinate formula bar that reads `[B2] ▸ alice`.

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
  preview: null,        // { sheet, rows, max_rows }
  selectedCell: null,   // { row, col } in 0-indexed grid coords
  parserNames: [],
  activeParser: 'standard',
  configPath: null,
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
// <app-shell> — top-level layout
// =============================================================================

class AppShell extends LitElement {
  static styles = css`
    :host {
      display: grid;
      grid-template-rows: auto 1fr 24px;
      height: 100%;
      background: var(--ink);
      color: var(--paper);
      font: var(--t-13)/1.45 var(--sans);
    }
    header {
      display: flex; align-items: center; gap: 18px;
      padding: 10px 18px;
      background: linear-gradient(180deg, #181C25 0%, var(--panel) 100%);
      border-bottom: 1px solid var(--rule);
    }
    .brand {
      display: flex; align-items: center; gap: 9px;
      font: 600 14px/1 var(--serif);
      letter-spacing: 0.01em;
      color: var(--paper);
    }
    .brand .mark {
      display: inline-grid;
      grid-template-columns: repeat(3, 6px);
      grid-template-rows: repeat(2, 6px);
      gap: 1px;
      transition: filter 200ms ease;
    }
    .brand:hover .mark { filter: drop-shadow(0 0 4px var(--amber-soft)); }
    .brand .mark i {
      background: var(--rule-2);
      display: block;
      border-radius: 1px;
    }
    .brand .mark i:nth-child(1) { background: var(--amber); }
    .brand .mark i:nth-child(3) { background: var(--cyan); }
    .brand .mark i:nth-child(5) { background: var(--paper); }
    .brand .ver {
      font: 400 9px/1 var(--mono);
      color: var(--graphite);
      padding: 2px 5px;
      border: 1px solid var(--rule-2);
      border-radius: 2px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .spacer { flex: 1; }
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--graphite);
      display: flex; gap: 14px;
      letter-spacing: 0.04em;
    }
    .meta b { color: var(--paper); font-weight: 500; }
    main {
      display: grid;
      grid-template-columns: 264px 1fr 340px;
      background: var(--rule);
      gap: 1px;
      overflow: hidden;
    }
    main > * { background: var(--panel); overflow: auto; min-width: 0; }
    footer {
      background: var(--panel);
      border-top: 1px solid var(--rule);
      display: flex; align-items: center;
    }
  `;
  _store = new StoreSub(this);

  render() {
    const s = store;
    return html`
      <header>
        <span class="brand">
          <span class="mark"><i></i><i></i><i></i><i></i><i></i><i></i></span>
          tablec
          <span class="ver">webui</span>
        </span>
        <dir-picker></dir-picker>
        <span class="spacer"></span>
        <span class="meta">
          <span>parser <b>${s.activeParser}</b></span>
          <span>cfg <b>${s.configPath ?? '(default)'}</b></span>
        </span>
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
    // Global keyboard shortcuts: ⌘B build · ⌘C check · ⌘R reload.
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
  };
}
customElements.define('app-shell', AppShell);

// =============================================================================
// <dir-picker>
// =============================================================================

class DirPicker extends LitElement {
  static styles = css`
    :host { display: flex; gap: 0; align-items: center; flex: 1; max-width: 620px; }
    .prefix {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--graphite);
      padding: 6px 8px;
      background: var(--panel-2);
      border: 1px solid var(--rule);
      border-right: none;
      border-radius: 4px 0 0 4px;
    }
    input {
      flex: 1; min-width: 0;
      font: 400 var(--t-12)/1 var(--mono);
      color: var(--paper);
      background: var(--panel-2);
      border: 1px solid var(--rule);
      border-left: none;
      padding: 6px 10px;
      outline: none;
      border-radius: 0;
    }
    input:focus { border-color: var(--amber); box-shadow: 0 0 0 1px var(--amber-soft); }
    button {
      font: 500 var(--t-11)/1 var(--mono);
      color: var(--paper);
      background: var(--panel-2);
      border: 1px solid var(--rule);
      padding: 6px 12px;
      cursor: pointer;
      border-radius: 0;
      letter-spacing: 0.02em;
      transition: background 100ms ease, border-color 100ms ease;
    }
    button:hover { background: var(--rule); border-color: var(--rule-2); }
    button.go {
      background: var(--amber);
      color: var(--ink);
      border-color: var(--amber);
      border-left: 1px solid var(--amber);
      font-weight: 600;
      border-radius: 0 4px 4px 0;
    }
    button.go:hover { filter: brightness(1.05); }
    button.reload {
      margin-left: 6px;
      border-radius: 4px;
      font-family: var(--mono);
    }
  `;
  _store = new StoreSub(this);

  render() {
    return html`
      <span class="prefix">~/</span>
      <input
        spellcheck="false"
        autocomplete="off"
        .value=${store.dir}
        @keydown=${this._onKey}
      />
      <button class="go" @click=${this._go}>打开</button>
      <button class="reload" title="重新扫描 (⌘R)" @click=${() => refreshState()}>⟳</button>
    `;
  }

  _onKey = (e) => { if (e.key === 'Enter') this._go(); };
  _go = async () => {
    const input = this.renderRoot.querySelector('input');
    store.dir = (input?.value || '.').trim() || '.';
    notify();
    await refreshState();
  };
}
customElements.define('dir-picker', DirPicker);

// =============================================================================
// <file-list>
// =============================================================================

class FileList extends LitElement {
  static styles = css`
    :host { display: block; }
    .head {
      position: sticky; top: 0; z-index: 2;
      padding: 12px 16px 10px;
      background: linear-gradient(180deg, var(--panel) 0%, var(--panel) 70%, transparent 100%);
      border-bottom: 1px solid var(--rule);
      font: 500 10px/1 var(--mono);
      color: var(--graphite);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      display: flex; justify-content: space-between; align-items: baseline;
    }
    .head .count { color: var(--paper); font-weight: 600; }
    ul { list-style: none; padding: 0; margin: 0; }
    li {
      padding: 10px 16px 10px 17px;
      cursor: pointer;
      border-bottom: 1px solid var(--rule);
      border-left: 2px solid transparent;
      display: flex; flex-direction: column; gap: 4px;
      transition: background 100ms ease, border-color 100ms ease;
    }
    li:hover { background: var(--panel-2); }
    li.selected {
      background: var(--panel-2);
      border-left-color: var(--amber);
    }
    .name {
      font: 500 var(--t-13)/1.2 var(--sans);
      color: var(--paper);
      display: flex; align-items: center; gap: 7px;
    }
    .name .ext {
      font: 500 9px/1 var(--mono);
      color: var(--cyan);
      padding: 2px 5px;
      background: var(--cyan-soft);
      border: 1px solid rgba(93, 211, 196, 0.2);
      border-radius: 2px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--graphite);
      letter-spacing: 0.02em;
    }
    .empty {
      padding: 24px 16px;
      color: var(--graphite);
      font: 400 var(--t-12)/1.55 var(--sans);
    }
    .empty code {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--paper);
      background: var(--panel-2);
      padding: 1px 5px;
      border-radius: 2px;
    }
  `;
  _store = new StoreSub(this);

  render() {
    const { files, selectedPath } = store;
    return html`
      <div class="head">
        <span>FILES</span>
        <span class="count">${String(files.length).padStart(2, '0')}</span>
      </div>
      ${files.length === 0
        ? html`<div class="empty">
            该目录下没有可识别的数据文件。<br />
            支持 <code>.xlsx</code> <code>.xls</code> <code>.xlsb</code> <code>.ods</code>
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
// <file-preview> — formula bar · sheet tabs · spreadsheet grid
// =============================================================================

class FilePreview extends LitElement {
  static styles = css`
    :host {
      display: flex; flex-direction: column;
      height: 100%; min-height: 0;
      background: var(--panel);
    }

    /* ---- formula bar (signature) ---- */
    .formula {
      display: grid;
      grid-template-columns: 72px 1fr;
      align-items: stretch;
      border-bottom: 1px solid var(--rule);
      background: var(--panel-2);
      font-family: var(--mono);
    }
    .formula .coord {
      font: 600 var(--t-12)/1 var(--mono);
      color: var(--ink);
      background: var(--amber);
      padding: 10px 12px;
      letter-spacing: 0.05em;
      display: flex; align-items: center; justify-content: center;
      transition: background 100ms ease;
    }
    .formula .coord.muted {
      background: transparent;
      color: var(--dim);
      border-right: 1px solid var(--rule);
    }
    .formula .fn {
      display: flex; align-items: center; gap: 10px;
      padding: 0 14px;
      font: 400 var(--t-12)/1.4 var(--mono);
      color: var(--paper);
      overflow: hidden;
      min-width: 0;
    }
    .formula .fn .src {
      color: var(--graphite);
      font-size: var(--t-11);
      flex-shrink: 0;
      max-width: 50%;
      overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    }
    .formula .fn .sep { color: var(--graphite); }
    .formula .fn .val {
      color: var(--paper);
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
      font-variant-numeric: tabular-nums;
    }
    .formula .fn .val.num { color: var(--cyan); }
    .formula .fn .val.bool { color: var(--amber); }
    .formula .fn .val.empty { color: var(--dim); font-style: italic; }

    /* ---- tabs (pill row) ---- */
    .tabs {
      display: flex; gap: 2px;
      padding: 8px 14px;
      background: var(--panel-2);
      border-bottom: 1px solid var(--rule);
      overflow-x: auto;
      scrollbar-width: none;
    }
    .tabs::-webkit-scrollbar { display: none; }
    .tab {
      padding: 6px 12px;
      cursor: pointer;
      font: 500 var(--t-12)/1 var(--mono);
      color: var(--graphite);
      border-radius: 4px;
      transition: background 100ms, color 100ms;
      white-space: nowrap;
      display: flex; align-items: baseline; gap: 6px;
    }
    .tab:hover { color: var(--paper); background: rgba(232, 236, 242, 0.04); }
    .tab.active {
      color: var(--ink);
      background: var(--paper);
      font-weight: 600;
    }
    .tab .size {
      font-size: 9px;
      color: var(--dim);
      letter-spacing: 0.04em;
      font-weight: 400;
    }
    .tab.active .size { color: var(--graphite); }

    /* ---- spreadsheet grid ---- */
    .body {
      flex: 1; min-height: 0;
      overflow: auto;
      background: var(--ink);
      position: relative;
    }
    table.grid {
      border-collapse: collapse;
      font: 400 var(--t-12)/1.35 var(--mono);
      color: var(--paper);
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
      background: var(--panel-2);
      color: var(--graphite);
      font-weight: 500;
      position: sticky;
      z-index: 1;
      font-size: var(--t-11);
      letter-spacing: 0.04em;
      user-select: none;
    }
    table.grid tr.letters th { top: 0; z-index: 3; }
    table.grid th.rowh {
      left: 0;
      z-index: 2;
      text-align: right;
      color: var(--graphite);
      min-width: 44px;
      background: var(--panel-2);
    }
    table.grid th.corner { z-index: 4; background: var(--panel-2); color: var(--dim); }
    table.grid td.cell { cursor: cell; transition: background 60ms; }
    table.grid td.cell:hover { background: var(--amber-soft); }
    table.grid td.selected {
      background: var(--amber-soft);
      outline: 1.5px solid var(--amber);
      outline-offset: -1.5px;
      color: var(--paper);
    }
    table.grid td.schema { color: var(--graphite); font-style: italic; }
    table.grid td.schema.col-name { color: var(--cyan); font-style: normal; font-weight: 500; }
    table.grid td.num { text-align: right; color: var(--cyan); }
    table.grid td.bool { color: var(--amber); text-align: center; }
    table.grid td.null { color: var(--dim); text-align: center; }

    .empty {
      padding: 32px;
      color: var(--graphite);
      font: 400 var(--t-13)/1.6 var(--sans);
      max-width: 400px;
    }
    .empty .hint {
      font: 400 var(--t-11)/1.4 var(--mono);
      color: var(--dim);
      margin-top: 10px;
    }
  `;
  _store = new StoreSub(this);

  connectedCallback() {
    super.connectedCallback();
    this.addEventListener('keydown', this._onKey);
  }

  render() {
    const { selectedPath, sheets, activeSheet, preview, selectedCell } = store;
    const coord = selectedCell ? colLetter(selectedCell.col) + (selectedCell.row + 1) : null;
    const cellValue = selectedCell && preview?.rows
      ? preview.rows[selectedCell.row]?.[selectedCell.col]
      : undefined;

    return html`
      <div class="formula">
        <div class="coord ${coord ? '' : 'muted'}">${coord ?? '—'}</div>
        <div class="fn">
          <span class="src">${baseName(selectedPath || '')} · ${activeSheet ?? ''}</span>
          ${coord ? html`<span class="sep">▸</span>` : null}
          <span class="val ${cellClass(cellValue) || 'empty'}">
            ${cellValue == null && !coord ? '点击任意单元格查看坐标' : this._renderFormulaValue(cellValue)}
          </span>
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
      </div>

      <div class="body">
        ${this._renderBody()}
      </div>
    `;
  }

  _renderFormulaValue(cell) {
    if (cell == null) return html`<span style="font-style:italic">∅ empty</span>`;
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
    const { selectedPath, sheets, preview } = store;
    if (!selectedPath) {
      return html`<div class="empty">
        请选择左侧文件以预览。
        <div class="hint">支持 .xlsx / .xls / .xlsb / .ods</div>
      </div>`;
    }
    if (sheets.length === 0) {
      return html`<div class="empty">${selectedPath} 没有可预览的 sheet。</div>`;
    }
    if (!preview) {
      return html`<div class="empty">加载中…</div>`;
    }
    const rows = preview.rows || [];
    if (rows.length === 0) {
      return html`<div class="empty">空 sheet。</div>`;
    }
    const ncols = Math.max(...rows.map((r) => r.length), 1);
    const sel = store.selectedCell;

    const trs = [];
    // Header row: column letters
    const headCells = [html`<th class="corner"></th>`];
    for (let c = 0; c < ncols; c++) headCells.push(html`<th>${colLetter(c)}</th>`);
    trs.push(html`<tr class="letters">${headCells}</tr>`);

    rows.forEach((row, ri) => {
      const isSchema = ri < 5;
      const cells = [html`<th class="rowh">${ri + 1}</th>`];
      for (let c = 0; c < ncols; c++) {
        const cell = row[c];
        const isSelected = sel && sel.row === ri && sel.col === c;
        const cls = [
          'cell',
          isSchema ? 'schema' : '',
          c === 0 ? 'col-name' : '',
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
    store.preview = null;
    notify();
    this._loadPreview();
  }

  async _loadFor(path) {
    store.selectedPath = path;
    store.sheets = [];
    store.activeSheet = null;
    store.preview = null;
    store.selectedCell = null;
    notify();
    try {
      const sheets = await getJson(`/api/sheets?path=${encodeURIComponent(path)}`);
      store.sheets = sheets;
      if (sheets.length) {
        store.activeSheet = sheets[0].name;
        notify();
        await this._loadPreview();
      } else {
        notify();
      }
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  async _loadPreview() {
    if (!store.selectedPath || !store.activeSheet) return;
    try {
      const grid = await getJson(
        `/api/preview?path=${encodeURIComponent(store.selectedPath)}` +
        `&sheet=${encodeURIComponent(store.activeSheet)}&max_rows=120`
      );
      store.preview = grid;
      // Pick the first non-schema cell as initial selection if available.
      if (grid.rows && grid.rows.length > 5) {
        store.selectedCell = { row: 5, col: 0 };
      } else if (grid.rows && grid.rows.length) {
        store.selectedCell = { row: 0, col: 0 };
      }
      notify();
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  _onKey = (e) => {
    if (!store.selectedCell || !store.preview?.rows?.length) return;
    const sel = store.selectedCell;
    const nrows = store.preview.rows.length;
    const ncols = Math.max(...store.preview.rows.map((r) => r.length), 1);
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
customElements.define('file-preview', FilePreview);

// =============================================================================
// <build-panel>
// =============================================================================

class BuildPanel extends LitElement {
  static styles = css`
    :host { display: block; padding: 16px; color: var(--paper); }
    .head {
      font: 500 10px/1 var(--mono);
      color: var(--graphite);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      margin-bottom: 12px;
      display: flex; justify-content: space-between; align-items: baseline;
    }
    .group { margin-bottom: 12px; }
    label.row {
      display: flex; gap: 8px; align-items: center;
      font: 500 var(--t-11)/1 var(--mono);
      color: var(--graphite);
      margin-bottom: 6px;
      letter-spacing: 0.04em;
    }
    label.row b { color: var(--paper); font-weight: 500; }
    select {
      width: 100%;
      font: 400 var(--t-12)/1 var(--mono);
      color: var(--paper);
      background: var(--panel-2);
      border: 1px solid var(--rule);
      padding: 6px 8px;
      border-radius: 4px;
      outline: none;
      appearance: none;
      cursor: pointer;
    }
    select:focus { border-color: var(--amber); box-shadow: 0 0 0 1px var(--amber-soft); }
    .opts {
      display: grid; grid-template-columns: 1fr;
      gap: 8px;
      font: 400 var(--t-12)/1 var(--mono);
      margin-bottom: 14px;
    }
    .opts label {
      display: flex; gap: 9px; align-items: center;
      color: var(--graphite);
      cursor: pointer;
      user-select: none;
    }
    .opts input[type=checkbox] {
      appearance: none;
      width: 13px; height: 13px;
      background: var(--panel-2);
      border: 1px solid var(--rule-2);
      border-radius: 3px;
      display: inline-grid; place-items: center;
      cursor: pointer;
      transition: background 100ms, border-color 100ms;
    }
    .opts input[type=checkbox]:hover { border-color: var(--amber); }
    .opts input[type=checkbox]:checked {
      background: var(--amber);
      border-color: var(--amber);
    }
    .opts input[type=checkbox]:checked::after {
      content: '';
      width: 5px; height: 2.5px;
      border-left: 1.5px solid var(--ink);
      border-bottom: 1.5px solid var(--ink);
      transform: translateY(-1px) rotate(-45deg);
    }
    .opts label:has(input:checked) { color: var(--paper); }

    .actions {
      display: grid; grid-template-columns: 1fr 1fr 1fr;
      gap: 6px;
      margin-bottom: 14px;
    }
    button.act {
      font: 600 var(--t-11)/1 var(--mono);
      letter-spacing: 0.06em;
      color: var(--ink);
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
    button.act:disabled { opacity: .4; cursor: progress; }
    button.act[data-kind=build] {
      background: var(--amber); border-color: var(--amber);
    }
    button.act[data-kind=check] {
      background: var(--cyan); border-color: var(--cyan);
    }
    button.act[data-kind=validate] {
      background: transparent;
      border-color: var(--rule-2);
      color: var(--graphite);
      border-style: dashed;
    }
    button.act .hint {
      font: 400 9px/1 var(--mono);
      opacity: .55;
      letter-spacing: 0.08em;
    }

    .live {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--cyan);
      display: flex; align-items: center; gap: 7px;
      margin-bottom: 8px;
      height: 14px;
      letter-spacing: 0.04em;
    }
    .live .dot {
      width: 7px; height: 7px;
      background: var(--cyan);
      border-radius: 50%;
      box-shadow: 0 0 6px var(--cyan);
      animation: pulse 1.2s ease-in-out infinite;
    }
    @keyframes pulse { 50% { opacity: .3; } }
    .live.off { color: var(--dim); }
    .live.off .dot { background: var(--dim); box-shadow: none; animation: none; }

    /* ---- result card ---- */
    .out {
      font: 400 var(--t-11)/1.5 var(--mono);
      background: var(--ink);
      border: 1px solid var(--rule);
      color: var(--paper);
      padding: 10px 12px;
      border-radius: 4px;
    }
    .out .row { display: flex; gap: 10px; padding: 2px 0; }
    .out .k { color: var(--graphite); flex-shrink: 0; width: 64px; }
    .out .v { color: var(--paper); word-break: break-all; }
    .out .n { color: var(--cyan); font-variant-numeric: tabular-nums; }
    .out .s { color: var(--amber); }
    .out .e { color: var(--rose); }
    .out .ok { color: var(--jade); }
    .out .sep {
      height: 1px; background: var(--rule); margin: 6px 0;
    }
    .out .placeholder { color: var(--dim); font-style: italic; }

    .diag-list {
      list-style: none; padding: 0; margin: 10px 0 0;
      max-height: 240px; overflow: auto;
    }
    .diag {
      padding: 7px 10px 7px 12px;
      border-left: 3px solid var(--rule-2);
      margin-bottom: 3px;
      font: 400 var(--t-11)/1.4 var(--mono);
      background: var(--panel-2);
      color: var(--paper);
      border-radius: 0 3px 3px 0;
      display: flex; flex-direction: column; gap: 2px;
    }
    .diag.error { border-left-color: var(--rose); }
    .diag.warning { border-left-color: var(--amber); }
    .diag.note { border-left-color: var(--cyan); }
    .diag .head { display: flex; gap: 8px; align-items: baseline; }
    .diag .sev {
      font-size: 9px;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--graphite);
      font-weight: 600;
    }
    .diag.error .sev { color: var(--rose); }
    .diag.warning .sev { color: var(--amber); }
    .diag .code { color: var(--cyan); font-weight: 500; }
    .diag .msg { color: var(--paper); }
    .diag .where { color: var(--graphite); font-size: 10px; margin-top: 1px; }
  `;
  _store = new StoreSub(this);

  render() {
    const { parserNames, activeParser, busy, lastResult } = store;
    return html`
      <div class="head">
        <span>BUILD &amp; CHECK</span>
      </div>

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

      <div class="out">${this._renderResult(lastResult)}</div>
      ${this._renderDiagnostics(lastResult)}
    `;
  }

  _renderResult(last) {
    if (!last) {
      return html`<div class="placeholder">尚未运行。</div>`;
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
      color: var(--graphite);
      letter-spacing: 0.04em;
    }
    .seg {
      display: flex; gap: 7px; align-items: center;
      padding: 0 14px;
      border-right: 1px solid var(--rule);
    }
    .seg:first-child { padding-left: 0; }
    .seg:last-child { border-right: none; }
    .seg b { color: var(--paper); font-weight: 500; }
    .seg .live-dot {
      width: 6px; height: 6px;
      background: var(--dim);
      border-radius: 50%;
    }
    .seg.busy .live-dot { background: var(--cyan); box-shadow: 0 0 4px var(--cyan); animation: blink 1.2s ease-in-out infinite; }
    @keyframes blink { 50% { opacity: .3; } }
    .seg.ok .live-dot { background: var(--jade); }
    .seg.err .live-dot { background: var(--rose); }
    .spacer { flex: 1; }
    .right { color: var(--dim); padding-right: 0; border-right: none; }
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
// tablec webui — Web Components SPA, zero external deps.
//
// Each component extends HTMLElement, attaches a Shadow DOM in
// connectedCallback, and listens via the local `bus` pub/sub. State is
// kept in `appState` (a plain object) and mutated via setters that emit
// bus events; components re-render on event receipt.
//
// Aesthetic: "The Cell" — dark IDE meets spreadsheet. Column letters
// (A, B, C…) above the grid, row numbers on the left, a cell-coordinate
// formula bar at the top. The signature element is the formula bar
// reading `[B2] ▸ Items.name`.

const bus = (() => {
  const map = new Map();
  return {
    on(name, fn) {
      let s = map.get(name);
      if (!s) { s = new Set(); map.set(name, s); }
      s.add(fn);
      return () => s.delete(fn);
    },
    emit(name, detail) {
      const s = map.get(name);
      if (!s) return;
      for (const fn of s) {
        try { fn(detail); } catch (e) { console.error('bus handler', name, e); }
      }
    },
  };
})();

const appState = {
  dir: '.',
  files: [],          // [{ name, path, size, modified_secs }]
  selectedPath: null,
  sheets: [],         // [{ name, row_count, col_count }]
  activeSheet: null,
  preview: null,      // { sheet, rows: [[cell,...]], max_rows }
  selectedCell: null, // { row, col } — currently-focused cell in the grid
  parserNames: [],
  activeParser: 'standard',
  configPresent: false,
  configPath: null,
  busy: false,
  lastResult: null,   // { kind, status, payload }
};

// ---------- color tokens, duplicated from style.css because shadow roots
//            can't see :root vars defined in light DOM. Keep in sync. ----
const TOKENS = `
  :host {
    --ink:     #0E1116;
    --panel:   #161A21;
    --panel-2: #1B2029;
    --rule:    #22272F;
    --rule-2:  #2A3140;
    --paper:   #E6EAF2;
    --graphite:#7A8497;
    --dim:     #4B5566;
    --amber:   #F5C242;
    --cyan:    #5DD3C4;
    --rose:    #E06C75;
    --mono:    ui-monospace, 'JetBrains Mono', 'Cascadia Code', 'SF Mono', Menlo, Consolas, monospace;
    --sans:    -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
  }
`;

// =============================================================================
// <app-shell> — top-level layout: title bar · 3 columns · status footer
// =============================================================================

class AppShell extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host {
          display: grid;
          grid-template-rows: 44px 1fr 24px;
          height: 100%;
          background: var(--ink);
          color: var(--paper);
          font: 13px/1.45 var(--sans);
        }
        header {
          display: flex; align-items: center; gap: 16px;
          padding: 0 14px;
          background: linear-gradient(to bottom, #181C25, var(--panel));
          border-bottom: 1px solid var(--rule);
        }
        .brand {
          font: 600 13px/1 var(--mono);
          letter-spacing: 0.04em;
          color: var(--paper);
          display: flex; align-items: center; gap: 8px;
        }
        .brand .mark {
          display: inline-grid;
          grid-template-columns: repeat(3, 8px);
          grid-template-rows: repeat(2, 8px);
          gap: 1px;
        }
        .brand .mark i { background: var(--amber); display: block; }
        .brand .mark i:nth-child(1) { background: var(--amber); }
        .brand .mark i:nth-child(2) { background: var(--rule-2); }
        .brand .mark i:nth-child(3) { background: var(--cyan); opacity: .7; }
        .brand .mark i:nth-child(4) { background: var(--rule-2); }
        .brand .mark i:nth-child(5) { background: var(--paper); }
        .brand .mark i:nth-child(6) { background: var(--rule-2); }
        .brand .ver {
          font: 400 10px/1 var(--mono);
          color: var(--graphite);
          padding: 2px 5px;
          border: 1px solid var(--rule-2);
          border-radius: 2px;
        }
        .spacer { flex: 1; }
        .cfg {
          font: 400 11px/1 var(--mono);
          color: var(--graphite);
          display: flex; gap: 12px;
        }
        .cfg span b { color: var(--paper); font-weight: 600; }
        main {
          display: grid;
          grid-template-columns: 260px 1fr 320px;
          background: var(--rule);
          gap: 1px;
          overflow: hidden;
        }
        main > * {
          background: var(--panel);
          overflow: auto;
          min-width: 0;
        }
        footer {
          background: var(--panel);
          border-top: 1px solid var(--rule);
          display: flex; align-items: center;
        }
      </style>
      <header>
        <span class="brand">
          <span class="mark"><i></i><i></i><i></i><i></i><i></i><i></i></span>
          tablec
          <span class="ver">webui</span>
        </span>
        <dir-picker></dir-picker>
        <span class="spacer"></span>
        <span class="cfg" id="cfg"></span>
      </header>
      <main>
        <file-list></file-list>
        <file-preview></file-preview>
        <build-panel></build-panel>
      </main>
      <footer><status-bar></status-bar></footer>
    `;
    this.shadowRoot.getElementById('cfg').innerHTML =
      `<span>parser <b id="parser">${escapeHtml(appState.activeParser)}</b></span>` +
      `<span>cfg <b id="cfg-path">${appState.configPath ? escapeHtml(appState.configPath) : '(default)'}</b></span>`;
    bus.on('app:state', (s) => {
      this.shadowRoot.getElementById('parser').textContent = s.activeParser;
      this.shadowRoot.getElementById('cfg-path').textContent =
        s.configPath ?? '(default)';
    });
    refreshState();
  }
}
customElements.define('app-shell', AppShell);

// =============================================================================
// <dir-picker>
// =============================================================================

class DirPicker extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host { display: flex; gap: 6px; align-items: center; flex: 1; max-width: 640px; }
        .prefix {
          font: 400 11px/1 var(--mono);
          color: var(--graphite);
          padding: 4px 6px;
          background: var(--panel-2);
          border: 1px solid var(--rule);
          border-right: none;
          border-radius: 3px 0 0 3px;
        }
        input {
          flex: 1; min-width: 0;
          font: 400 12px/1 var(--mono);
          color: var(--paper);
          background: var(--panel-2);
          border: 1px solid var(--rule);
          padding: 5px 8px;
          outline: none;
          border-radius: 0;
        }
        input:focus { border-color: var(--amber); }
        button {
          font: 400 11px/1 var(--mono);
          color: var(--paper);
          background: var(--panel-2);
          border: 1px solid var(--rule);
          padding: 6px 10px;
          cursor: pointer;
          border-radius: 3px;
          letter-spacing: 0.02em;
        }
        button:hover { background: var(--rule); border-color: var(--rule-2); }
        button.go {
          background: var(--amber);
          color: var(--ink);
          border-color: var(--amber);
          font-weight: 600;
        }
        button.go:hover { filter: brightness(1.08); }
        button.reload { font-family: var(--mono); }
      </style>
      <span class="prefix">~/</span>
      <input id="dir" value="${escapeAttr(appState.dir)}" spellcheck="false" autocomplete="off">
      <button id="go" class="go">打开</button>
      <button id="reload" class="reload" title="重新扫描目录">⟳</button>
    `;
    const input = shadow.getElementById('dir');
    const go = shadow.getElementById('go');
    const reload = shadow.getElementById('reload');
    go.onclick = async () => {
      appState.dir = input.value.trim() || '.';
      bus.emit('app:state', { ...appState });
      await refreshState();
      bus.emit('app:dir-changed', appState.dir);
    };
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') go.click(); });
    reload.onclick = async () => { await refreshState(); };
  }
}
customElements.define('dir-picker', DirPicker);

// =============================================================================
// <file-list>
// =============================================================================

class FileList extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host { display: block; }
        .head {
          position: sticky; top: 0; z-index: 2;
          padding: 10px 14px 8px;
          background: var(--panel);
          border-bottom: 1px solid var(--rule);
          font: 400 10px/1 var(--mono);
          color: var(--graphite);
          letter-spacing: 0.16em;
          text-transform: uppercase;
          display: flex; justify-content: space-between; align-items: baseline;
        }
        .head .count { color: var(--paper); font-weight: 600; }
        ul { list-style: none; padding: 0; margin: 0; }
        li {
          padding: 8px 14px 8px 16px;
          cursor: pointer;
          border-bottom: 1px solid var(--rule);
          border-left: 3px solid transparent;
          display: flex; flex-direction: column; gap: 3px;
          transition: background 80ms ease, border-color 80ms ease;
        }
        li:hover { background: var(--panel-2); }
        li.selected {
          background: var(--panel-2);
          border-left-color: var(--amber);
        }
        .name {
          font: 500 13px/1.2 var(--sans);
          color: var(--paper);
          display: flex; align-items: center; gap: 6px;
        }
        .name .ext {
          font: 400 10px/1 var(--mono);
          color: var(--cyan);
          padding: 1px 4px;
          background: rgba(93,211,196,0.08);
          border: 1px solid rgba(93,211,196,0.18);
          border-radius: 2px;
        }
        .meta {
          font: 400 11px/1 var(--mono);
          color: var(--graphite);
          letter-spacing: 0.02em;
        }
        .empty {
          padding: 16px 14px;
          color: var(--graphite);
          font-size: 12px;
          line-height: 1.55;
        }
        .empty code {
          font: 400 11px/1 var(--mono);
          color: var(--paper);
          background: var(--panel-2);
          padding: 1px 4px;
          border-radius: 2px;
        }
      </style>
      <div class="head">
        <span>FILES</span>
        <span class="count" id="count">0</span>
      </div>
      <div id="root"></div>
    `;
    this.render();
    bus.on('app:files', () => this.render());
    bus.on('app:select-file', () => this.render());
  }

  render() {
    const root = this.shadowRoot.getElementById('root');
    const count = this.shadowRoot.getElementById('count');
    count.textContent = String(appState.files.length).padStart(2, '0');
    if (!appState.files.length) {
      root.innerHTML = `<div class="empty">该目录下没有可识别的数据文件。<br>支持 <code>.xlsx</code> <code>.xls</code> <code>.xlsb</code> <code>.ods</code></div>`;
      return;
    }
    root.innerHTML = `<ul>${appState.files.map((f) => `
      <li data-path="${escapeAttr(f.path)}" class="${appState.selectedPath === f.path ? 'selected' : ''}">
        <span class="name">
          ${escapeHtml(f.name)}
          <span class="ext">${escapeHtml(extOf(f.name))}</span>
        </span>
        <span class="meta">${humanSize(f.size)}  ·  ${new Date(f.modified_secs * 1000).toLocaleString()}</span>
      </li>`).join('')}</ul>`;
    root.querySelectorAll('li').forEach((li) => {
      li.onclick = () => bus.emit('app:select-file', { path: li.dataset.path });
    });
  }
}
customElements.define('file-list', FileList);

// =============================================================================
// <file-preview> — sheet tabs + spreadsheet grid with cell coord formula bar
// =============================================================================

class FilePreview extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--panel); }

        /* ---- formula bar (signature element) ---- */
        .formula {
          display: grid;
          grid-template-columns: 64px 1fr;
          align-items: stretch;
          border-bottom: 1px solid var(--rule);
          background: var(--panel-2);
          font-family: var(--mono);
        }
        .formula .coord {
          font: 600 12px/1 var(--mono);
          color: var(--ink);
          background: var(--amber);
          padding: 9px 10px;
          letter-spacing: 0.04em;
          display: flex; align-items: center; justify-content: center;
          border-right: 1px solid var(--rule);
        }
        .formula .coord.muted { background: transparent; color: var(--dim); border-color: var(--rule); }
        .formula .fn {
          display: flex; align-items: center; gap: 8px;
          padding: 0 12px;
          font: 400 12px/1.4 var(--mono);
          color: var(--paper);
          overflow: hidden;
        }
        .formula .fn .sep { color: var(--graphite); }
        .formula .fn .src { color: var(--graphite); font-size: 11px; }
        .formula .fn .val {
          white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
          color: var(--paper);
        }
        .formula .fn .val.num { color: var(--cyan); font-variant-numeric: tabular-nums; }
        .formula .fn .val.bool { color: var(--amber); }
        .formula .fn .val.empty { color: var(--dim); font-style: italic; }

        /* ---- tabs ---- */
        .tabs {
          display: flex; gap: 0;
          padding: 0 12px;
          background: var(--panel-2);
          border-bottom: 1px solid var(--rule);
          overflow-x: auto;
          scrollbar-width: none;
        }
        .tabs::-webkit-scrollbar { display: none; }
        .tab {
          padding: 8px 14px 8px 12px;
          cursor: pointer;
          font: 400 12px/1 var(--mono);
          color: var(--graphite);
          border-bottom: 2px solid transparent;
          transition: color 80ms, border-color 80ms;
          white-space: nowrap;
          display: flex; align-items: baseline; gap: 6px;
        }
        .tab:hover { color: var(--paper); }
        .tab.active {
          color: var(--paper);
          border-bottom-color: var(--amber);
        }
        .tab .size {
          font-size: 10px;
          color: var(--dim);
          letter-spacing: 0.04em;
        }

        /* ---- spreadsheet grid ---- */
        .body {
          flex: 1; min-height: 0;
          overflow: auto;
          background: var(--ink);
        }
        table.grid {
          border-collapse: collapse;
          font: 400 12px/1.3 var(--mono);
          color: var(--paper);
          font-variant-numeric: tabular-nums;
        }
        table.grid th, table.grid td {
          border-right: 1px solid var(--rule);
          border-bottom: 1px solid var(--rule);
          padding: 4px 8px;
          text-align: left;
          white-space: nowrap;
          max-width: 320px;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        table.grid th {
          background: var(--panel-2);
          color: var(--graphite);
          font-weight: 500;
          position: sticky;
          z-index: 1;
          font-size: 11px;
          letter-spacing: 0.04em;
          user-select: none;
        }
        /* column-letter row sticks to top */
        table.grid tr.letters th {
          top: 0;
          z-index: 3;
        }
        /* row-number column sticks to left */
        table.grid th.rowh {
          left: 0;
          z-index: 2;
          text-align: right;
          color: var(--graphite);
          min-width: 40px;
          background: var(--panel-2);
        }
        /* the corner where row # col meets letter row */
        table.grid th.corner {
          z-index: 4;
          background: var(--panel-2);
          color: var(--dim);
        }
        table.grid td.cell {
          cursor: cell;
        }
        table.grid td.cell:hover { background: rgba(245,194,66,0.06); }
        table.grid td.selected {
          background: rgba(245,194,66,0.12);
          box-shadow: inset 0 0 0 1px var(--amber);
          color: var(--paper);
        }
        table.grid td.schema { color: var(--graphite); font-style: italic; }
        table.grid td.schema.col-name { color: var(--cyan); font-style: normal; font-weight: 500; }
        table.grid td.num { text-align: right; color: var(--cyan); }
        table.grid td.bool { color: var(--amber); }
        table.grid td.null { color: var(--dim); }

        .empty {
          padding: 24px;
          color: var(--graphite);
          font-size: 13px;
          line-height: 1.6;
        }
        .empty .hint {
          font: 400 11px/1.4 var(--mono);
          color: var(--dim);
          margin-top: 6px;
        }
      </style>

      <div class="formula">
        <div class="coord muted" id="coord">—</div>
        <div class="fn" id="fn">
          <span class="src" id="fn-src"></span>
          <span class="sep" id="fn-sep" style="display:none">▸</span>
          <span class="val empty" id="fn-val">点击任意单元格查看坐标</span>
        </div>
      </div>

      <div class="tabs" id="tabs"></div>
      <div class="body" id="body">
        <div class="empty">请选择左侧文件以预览。</div>
      </div>
    `;
    this.tabs = shadow.getElementById('tabs');
    this.body = shadow.getElementById('body');
    this.coordEl = shadow.getElementById('coord');
    this.fnSrc = shadow.getElementById('fn-src');
    this.fnSep = shadow.getElementById('fn-sep');
    this.fnVal = shadow.getElementById('fn-val');

    this.render();
    bus.on('app:select-file', ({ path }) => this.onSelect(path));
    bus.on('app:sheet', () => { this.renderTabs(); this.renderBody(); });
    bus.on('app:preview', () => this.renderBody());
    bus.on('app:cell', () => this.renderFormula());
  }

  async onSelect(path) {
    appState.selectedPath = path;
    appState.preview = null;
    appState.sheets = [];
    appState.activeSheet = null;
    appState.selectedCell = null;
    this.renderTabs();
    this.renderBody();
    this.renderFormula();
    try {
      const sheets = await getJson(`/api/sheets?path=${encodeURIComponent(path)}`);
      appState.sheets = sheets;
      if (sheets.length) {
        appState.activeSheet = sheets[0].name;
        bus.emit('app:sheet', { sheet: appState.activeSheet });
        await this.loadPreview();
      } else {
        this.renderBody();
      }
    } catch (e) {
      appState.lastResult = { error: String(e) };
      this.renderBody();
    }
  }

  async loadPreview() {
    if (!appState.selectedPath || !appState.activeSheet) return;
    try {
      const grid = await getJson(
        `/api/preview?path=${encodeURIComponent(appState.selectedPath)}` +
        `&sheet=${encodeURIComponent(appState.activeSheet)}&max_rows=120`);
      appState.preview = grid;
      // pick the first non-schema cell as initial selection if available
      if (grid.rows && grid.rows.length > 5) {
        appState.selectedCell = { row: 5, col: 0 };
      } else if (grid.rows && grid.rows.length) {
        appState.selectedCell = { row: 0, col: 0 };
      }
      bus.emit('app:preview', grid);
      bus.emit('app:cell', appState.selectedCell);
    } catch (e) {
      appState.lastResult = { error: String(e) };
      this.renderBody();
    }
  }

  render() {
    this.renderTabs();
    this.renderBody();
    this.renderFormula();
  }

  renderTabs() {
    if (!appState.sheets.length) {
      this.tabs.innerHTML = '';
      return;
    }
    this.tabs.innerHTML = appState.sheets.map((s) => `
      <div class="tab ${appState.activeSheet === s.name ? 'active' : ''}" data-sheet="${escapeAttr(s.name)}">
        <span>${escapeHtml(s.name)}</span>
        <span class="size">${s.row_count ?? '?'}×${s.col_count ?? '?'}</span>
      </div>`).join('');
    this.tabs.querySelectorAll('.tab').forEach((t) => {
      t.onclick = () => {
        appState.activeSheet = t.dataset.sheet;
        bus.emit('app:sheet', { sheet: appState.activeSheet });
        this.loadPreview();
      };
    });
  }

  renderFormula() {
    const c = appState.selectedCell;
    if (!c) {
      this.coordEl.textContent = '—';
      this.coordEl.classList.add('muted');
      this.fnSrc.textContent = '';
      this.fnSep.style.display = 'none';
      this.fnVal.textContent = '点击任意单元格查看坐标';
      this.fnVal.className = 'val empty';
      return;
    }
    const ref = colLetter(c.col) + (c.row + 1);
    this.coordEl.textContent = ref;
    this.coordEl.classList.remove('muted');
    // "src" = file + sheet name (like Excel's address bar for context)
    this.fnSrc.textContent = `${baseName(appState.selectedPath || '')} · ${appState.activeSheet || ''}`;
    this.fnSep.style.display = '';
    // value
    const v = (appState.preview?.rows?.[c.row] || [])[c.col];
    this.setFnValue(v);
  }

  setFnValue(cell) {
    const val = this.fnVal;
    val.classList.remove('num', 'bool', 'empty');
    if (cell == null) {
      val.textContent = '∅  empty';
      val.classList.add('empty');
    } else if (typeof cell === 'number') {
      val.textContent = String(cell);
      val.classList.add('num');
    } else if (typeof cell === 'string') {
      val.textContent = `"${cell}"`;
    } else if (typeof cell === 'boolean') {
      val.textContent = cell ? 'TRUE' : 'FALSE';
      val.classList.add('bool');
    } else if (typeof cell === 'object') {
      if ('Float' in cell) { val.textContent = String(cell.Float); val.classList.add('num'); }
      else if ('Bool' in cell) { val.textContent = cell.Bool ? 'TRUE' : 'FALSE'; val.classList.add('bool'); }
      else if ('Str' in cell) { val.textContent = `"${cell.Str}"`; }
      else if ('DateTime' in cell) { val.textContent = String(cell.DateTime); }
      else if ('Duration' in cell) { val.textContent = String(cell.Duration); }
      else { val.textContent = JSON.stringify(cell); }
    } else {
      val.textContent = String(cell);
    }
  }

  renderBody() {
    if (!appState.selectedPath) {
      this.body.innerHTML = `<div class="empty">请选择左侧文件以预览。<div class="hint">支持 .xlsx / .xls / .xlsb / .ods</div></div>`;
      return;
    }
    if (!appState.sheets.length) {
      this.body.innerHTML = `<div class="empty">${escapeHtml(appState.selectedPath)} 没有可预览的 sheet。</div>`;
      return;
    }
    if (!appState.preview) {
      this.body.innerHTML = `<div class="empty">加载中…</div>`;
      return;
    }
    const rows = appState.preview.rows || [];
    if (!rows.length) {
      this.body.innerHTML = `<div class="empty">空 sheet。</div>`;
      return;
    }
    const ncols = Math.max(...rows.map(r => r.length), 1);
    const isSchemaRow = (i) => i < 5;
    const isSchemaColName = (i) => i === 0;

    let html = `<table class="grid"><thead><tr class="letters"><th class="corner"></th>`;
    for (let c = 0; c < ncols; c++) html += `<th>${colLetter(c)}</th>`;
    html += `</tr></thead><tbody>`;
    rows.forEach((row, ri) => {
      html += `<tr>`;
      html += `<th class="rowh">${ri + 1}</th>`;
      const rowCls = isSchemaRow(ri) ? 'schema' : '';
      for (let c = 0; c < ncols; c++) {
        const cell = row[c];
        const sel = appState.selectedCell
          && appState.selectedCell.row === ri
          && appState.selectedCell.col === c;
        const cls = [
          'cell',
          rowCls,
          isSchemaColName(c) ? 'col-name' : '',
          cellClass(cell),
          sel ? 'selected' : '',
        ].filter(Boolean).join(' ');
        const content = cellText(cell);
        html += `<td class="${cls}" data-row="${ri}" data-col="${c}">${content}</td>`;
      }
      html += `</tr>`;
    });
    html += `</tbody></table>`;
    this.body.innerHTML = html;
    // delegate cell clicks
    this.body.querySelectorAll('td.cell').forEach((td) => {
      td.onclick = () => {
        const r = parseInt(td.dataset.row, 10);
        const co = parseInt(td.dataset.col, 10);
        appState.selectedCell = { row: r, col: co };
        bus.emit('app:cell', appState.selectedCell);
      };
    });
  }
}
customElements.define('file-preview', FilePreview);

function cellText(cell) {
  if (cell == null) return '<span style="color:var(--dim)">·</span>';
  if (typeof cell === 'number') return escapeHtml(String(cell));
  if (typeof cell === 'string') return escapeHtml(cell);
  if (typeof cell === 'boolean') return cell ? '✓' : '✗';
  if (typeof cell === 'object') {
    if ('Float' in cell) return escapeHtml(String(cell.Float));
    if ('Bool' in cell) return cell.Bool ? '✓' : '✗';
    if ('Str' in cell) return escapeHtml(cell.Str);
    if ('DateTime' in cell) return escapeHtml(String(cell.DateTime));
    if ('Duration' in cell) return escapeHtml(String(cell.Duration));
    return escapeHtml(JSON.stringify(cell));
  }
  return escapeHtml(String(cell));
}
function cellClass(cell) {
  if (cell == null) return 'null';
  if (typeof cell === 'number') return 'num';
  if (typeof cell === 'boolean') return 'bool';
  if (typeof cell === 'object' && 'Float' in cell) return 'num';
  if (typeof cell === 'object' && 'Bool' in cell) return 'bool';
  return '';
}

// =============================================================================
// <build-panel>
// =============================================================================

class BuildPanel extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host { display: block; padding: 14px; color: var(--paper); }
        .head {
          font: 400 10px/1 var(--mono);
          color: var(--graphite);
          letter-spacing: 0.16em;
          text-transform: uppercase;
          margin-bottom: 10px;
        }
        .group {
          margin-bottom: 12px;
        }
        label.row {
          display: flex; gap: 8px; align-items: center;
          font: 400 11px/1 var(--mono);
          color: var(--graphite);
          margin-bottom: 6px;
          letter-spacing: 0.04em;
        }
        label.row b { color: var(--paper); font-weight: 500; }
        select {
          flex: 1;
          font: 400 12px/1 var(--mono);
          color: var(--paper);
          background: var(--panel-2);
          border: 1px solid var(--rule);
          padding: 5px 6px;
          border-radius: 3px;
          outline: none;
        }
        select:focus { border-color: var(--amber); }
        .opts {
          display: grid; grid-template-columns: 1fr;
          gap: 6px;
          font: 400 12px/1 var(--mono);
          margin-bottom: 12px;
        }
        .opts label {
          display: flex; gap: 8px; align-items: center;
          color: var(--graphite);
          cursor: pointer;
          user-select: none;
        }
        .opts input[type=checkbox] {
          appearance: none;
          width: 12px; height: 12px;
          background: var(--panel-2);
          border: 1px solid var(--rule-2);
          border-radius: 2px;
          display: inline-grid; place-items: center;
          cursor: pointer;
        }
        .opts input[type=checkbox]:checked {
          background: var(--amber);
          border-color: var(--amber);
        }
        .opts input[type=checkbox]:checked::after {
          content: '';
          width: 6px; height: 3px;
          border-left: 2px solid var(--ink);
          border-bottom: 2px solid var(--ink);
          transform: translateY(-1px) rotate(-45deg);
        }
        .opts label:has(input:checked) { color: var(--paper); }

        .actions {
          display: grid; grid-template-columns: 1fr 1fr 1fr;
          gap: 6px;
          margin-bottom: 14px;
        }
        button.act {
          font: 600 11px/1 var(--mono);
          letter-spacing: 0.04em;
          color: var(--ink);
          border: 1px solid;
          padding: 9px 4px;
          cursor: pointer;
          border-radius: 3px;
          text-transform: uppercase;
          transition: filter 80ms;
        }
        button.act:hover { filter: brightness(1.1); }
        button.act:disabled { opacity: .4; cursor: progress; filter: none; }
        button.act[data-kind=build]    { background: var(--amber); border-color: var(--amber); color: var(--ink); }
        button.act[data-kind=check]    { background: var(--cyan); border-color: var(--cyan); color: var(--ink); }
        button.act[data-kind=validate] { background: transparent; border-color: var(--rule-2); color: var(--graphite); }

        .live {
          font: 400 11px/1 var(--mono);
          color: var(--cyan);
          display: flex; align-items: center; gap: 6px;
          margin-bottom: 8px;
          height: 14px;
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

        .out {
          font: 400 11px/1.5 var(--mono);
          background: var(--ink);
          border: 1px solid var(--rule);
          color: var(--paper);
          padding: 8px 10px;
          white-space: pre-wrap;
          word-break: break-all;
          max-height: 180px;
          overflow: auto;
          border-radius: 3px;
        }
        .out .k { color: var(--graphite); }
        .out .n { color: var(--cyan); font-variant-numeric: tabular-nums; }
        .out .s { color: var(--amber); }
        .out .e { color: var(--rose); }
        .out .t { color: var(--dim); }

        .diag-list { list-style: none; padding: 0; margin: 8px 0 0; max-height: 220px; overflow: auto; }
        .diag {
          padding: 6px 8px 6px 10px;
          border-left: 3px solid var(--rule-2);
          margin-bottom: 2px;
          font: 400 11px/1.4 var(--mono);
          background: var(--panel-2);
          color: var(--paper);
          display: flex; flex-direction: column; gap: 2px;
        }
        .diag.error { border-left-color: var(--rose); }
        .diag.warning { border-left-color: var(--amber); }
        .diag.note { border-left-color: var(--cyan); }
        .diag .head { display: flex; gap: 8px; align-items: baseline; }
        .diag .sev {
          font-size: 9px;
          letter-spacing: 0.12em;
          text-transform: uppercase;
          color: var(--graphite);
        }
        .diag.error .sev { color: var(--rose); }
        .diag.warning .sev { color: var(--amber); }
        .diag .code {
          color: var(--cyan);
        }
        .diag .msg { color: var(--paper); }
        .diag .where { color: var(--graphite); font-size: 10px; }
      </style>

      <div class="head">BUILD &amp; CHECK</div>

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
        <select id="parser"></select>
      </div>
      <div class="opts">
        <label><input type="checkbox" id="pretty"> pretty</label>
        <label><input type="checkbox" id="includeFields"> include_fields</label>
        <label><input type="checkbox" id="write"> write to disk</label>
      </div>
      <div class="actions">
        <button class="act" data-kind="build" id="build">Build</button>
        <button class="act" data-kind="check" id="check">Check</button>
        <button class="act" data-kind="validate" id="validate">Validate</button>
      </div>
      <div class="live off" id="live"><span class="dot"></span><span id="live-text">idle</span></div>
      <div id="out" class="out"><span class="t">尚未运行。</span></div>
      <ul id="diag" class="diag-list"></ul>
    `;
    const parserSel = shadow.getElementById('parser');
    parserSel.innerHTML = appState.parserNames
      .map((n) => `<option value="${escapeAttr(n)}" ${n === appState.activeParser ? 'selected' : ''}>${escapeHtml(n)}</option>`)
      .join('');
    shadow.getElementById('build').onclick = () => this.run('build');
    shadow.getElementById('check').onclick = () => this.run('check');
    shadow.getElementById('validate').onclick = () => this.run('validate');
  }

  async run(kind) {
    if (appState.busy) return;
    appState.busy = true;
    this.refresh();
    const out = this.shadowRoot.getElementById('out');
    const diag = this.shadowRoot.getElementById('diag');
    const live = this.shadowRoot.getElementById('live');
    const liveText = this.shadowRoot.getElementById('live-text');
    live.classList.remove('off');
    liveText.textContent = `running · ${kind}`;
    out.innerHTML = `<span class="t">⟶ ${kind}…</span>`;
    diag.innerHTML = '';
    try {
      let url, body;
      if (kind === 'build') {
        url = '/api/build';
        const fmt = this.shadowRoot.getElementById('fmt').value;
        const pretty = this.shadowRoot.getElementById('pretty').checked || fmt === 'json-pretty';
        const includeFields = this.shadowRoot.getElementById('includeFields').checked;
        const write = this.shadowRoot.getElementById('write').checked;
        const parser = this.shadowRoot.getElementById('parser').value;
        body = JSON.stringify({
          dir: appState.dir,
          format: fmt,
          pretty,
          include_fields: includeFields,
          write,
          parser,
          plugin_paths: [],
        });
      } else if (kind === 'check') {
        url = '/api/check';
        const parser = this.shadowRoot.getElementById('parser').value;
        body = JSON.stringify({ dir: appState.dir, parser, plugin_paths: [] });
      } else {
        url = '/api/validate';
        body = '{}';
      }
      const r = await fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body,
      });
      const text = await r.text();
      let payload;
      try { payload = JSON.parse(text); } catch { payload = { raw: text }; }
      appState.lastResult = { kind, status: r.status, payload };
      out.innerHTML = renderResultLine(kind, r.status, payload);
      diag.innerHTML = '';
      const diags = payload.diagnostics || [];
      for (const d of diags) {
        const li = document.createElement('li');
        li.className = `diag ${(d.severity || 'Error').toLowerCase()}`;
        const loc = d.location || {};
        const where = [loc.file, loc.sheet, loc.line, loc.column].filter(Boolean).join(':');
        li.innerHTML = `
          <div class="head">
            <span class="sev">${escapeHtml(d.severity || 'error')}</span>
            <span class="code">${escapeHtml(d.code || '')}</span>
          </div>
          <div class="msg">${escapeHtml(d.message || '')}</div>
          ${where ? `<div class="where">${escapeHtml(where)}</div>` : ''}
        `;
        diag.appendChild(li);
      }
    } catch (e) {
      appState.lastResult = { kind, error: String(e) };
      out.innerHTML = `<span class="e">✗ ${escapeHtml(String(e))}</span>`;
    } finally {
      appState.busy = false;
      this.refresh();
      live.classList.add('off');
      liveText.textContent = 'idle';
      bus.emit('app:result', appState.lastResult);
    }
  }

  refresh() {
    for (const id of ['build', 'check', 'validate']) {
      const b = this.shadowRoot.getElementById(id);
      b.disabled = appState.busy;
    }
  }
}
customElements.define('build-panel', BuildPanel);

function renderResultLine(kind, status, payload) {
  if (kind === 'validate') {
    return `<span class="k">${kind}</span> <span class="s">${status}</span> <span class="t">Not Implemented — 数据校验功能仍在研究中</span>`;
  }
  if (payload?.error) {
    return `<span class="k">${kind}</span> <span class="e">✗ HTTP ${status}</span>\n<span class="t">${escapeHtml(payload.error)}</span>`;
  }
  const dur = payload?.duration_ms != null ? ` <span class="n">${payload.duration_ms}ms</span>` : '';
  const nDiag = payload?.diagnostics?.length ?? 0;
  const nErr = (payload?.diagnostics || []).filter(d => (d.severity || 'Error') === 'Error').length;
  const nWarn = (payload?.diagnostics || []).filter(d => d.severity === 'Warning').length;
  const bytes = payload?.bytes_written != null ? ` <span class="k">·</span> <span class="n">${payload.bytes_written}</span><span class="k">B</span>` : '';
  const out = payload?.output_path ? ` <span class="k">→</span> <span class="s">${escapeHtml(payload.output_path)}</span>` : '';
  return `<span class="k">${kind}</span> <span class="s">${status}</span>${dur}${bytes}${out}\n<span class="k">diagnostics</span> <span class="n">${nDiag}</span> <span class="k">(</span><span class="e">${nErr}</span> <span class="k">err ·</span> <span class="s">${nWarn}</span> <span class="k">warn)</span>`;
}

// =============================================================================
// <status-bar>
// =============================================================================

class StatusBar extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>${TOKENS}
        :host {
          flex: 1;
          display: flex; gap: 18px; align-items: center;
          padding: 0 14px;
          font: 400 11px/1 var(--mono);
          color: var(--graphite);
          letter-spacing: 0.04em;
        }
        .seg { display: flex; gap: 6px; align-items: center; }
        .seg b { color: var(--paper); font-weight: 500; }
        .seg .live-dot {
          width: 6px; height: 6px;
          background: var(--dim);
          border-radius: 50%;
        }
        .seg.busy .live-dot { background: var(--cyan); box-shadow: 0 0 4px var(--cyan); animation: blink 1.2s ease-in-out infinite; }
        @keyframes blink { 50% { opacity: .3; } }
        .seg.ok .live-dot { background: var(--cyan); }
        .seg.err .live-dot { background: var(--rose); }
        .spacer { flex: 1; }
        .right { color: var(--dim); }
      </style>
      <span class="seg"><span class="live-dot"></span><span>dir</span><b id="dir">—</b></span>
      <span class="seg"><span>sheets</span><b id="sheets">0</b></span>
      <span class="seg" id="last-seg"><span>last</span><b id="last">—</b></span>
      <span class="spacer"></span>
      <span class="right">tablec · webui</span>
    `;
    this.dirEl = shadow.getElementById('dir');
    this.sheetsEl = shadow.getElementById('sheets');
    this.lastEl = shadow.getElementById('last');
    this.lastSeg = shadow.getElementById('last-seg');
    this.render();
    bus.on('app:state', () => this.render());
    bus.on('app:result', () => this.render());
    bus.on('app:sheet', () => this.render());
  }
  render() {
    this.dirEl.textContent = appState.dir;
    this.sheetsEl.textContent = String(appState.sheets.length).padStart(2, '0');
    const last = appState.lastResult;
    this.lastSeg.classList.remove('busy', 'ok', 'err');
    if (appState.busy) {
      this.lastSeg.classList.add('busy');
      this.lastEl.textContent = 'running…';
    } else if (last) {
      if (last.error) { this.lastSeg.classList.add('err'); this.lastEl.textContent = `error · ${truncErr(last.error)}`; }
      else if (last.status === 501) { this.lastSeg.classList.add('err'); this.lastEl.textContent = 'validate · 501 todo'; }
      else if (last.status >= 200 && last.status < 300) {
        this.lastSeg.classList.add('ok');
        const dur = last.payload?.duration_ms != null ? `${last.payload.duration_ms}ms` : `${last.status}`;
        this.lastEl.textContent = `${last.kind} · ${dur}`;
      } else {
        this.lastSeg.classList.add('err');
        this.lastEl.textContent = `${last.kind} · ${last.status}`;
      }
    } else {
      this.lastEl.textContent = '—';
    }
  }
}
customElements.define('status-bar', StatusBar);

// =============================================================================
// helpers
// =============================================================================

async function getJson(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}

async function refreshState() {
  try {
    const s = await getJson('/api/state');
    appState.dir = s.dir;
    appState.parserNames = s.parser_names || [];
    appState.activeParser = s.active_parser || 'standard';
    appState.configPresent = s.config_present;
    appState.configPath = s.config_path;
    bus.emit('app:state', { ...appState });
  } catch (e) { console.error('refreshState', e); }
  try {
    const files = await getJson(`/api/files?dir=${encodeURIComponent(appState.dir)}`);
    appState.files = files;
    bus.emit('app:files', files);
  } catch (e) { console.error('files', e); }
}

function colLetter(idx) {
  // 0 → A, 25 → Z, 26 → AA, 51 → AZ, 52 → BA (0-indexed spreadsheet cols)
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

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
function escapeAttr(s) { return escapeHtml(s); }
function humanSize(n) {
  const u = ['B', 'KB', 'MB', 'GB']; let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
}

// tablec webui — Web Components SPA, zero external deps.
//
// Each component extends HTMLElement, attaches a Shadow DOM in
// connectedCallback, and listens via the local `bus` pub/sub. State is
// kept in `appState` (a plain object) and mutated via setters that emit
// bus events; components re-render on event receipt.

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
  selectedPath: null, // currently-selected file
  sheets: [],         // [{ name, row_count, col_count }]
  activeSheet: null,
  preview: null,      // { sheet, rows: [[cell,...]] }
  parserNames: [],
  activeParser: 'standard',
  configPresent: false,
  configPath: null,
  busy: false,
  lastResult: null,   // { diagnostics, output_path, format, ... }
};

// -----------------------------------------------------------------------------
// <app-shell> — top-level layout
// -----------------------------------------------------------------------------

class AppShell extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: grid; grid-template-rows: 48px 1fr 24px; height: 100%; }
        header {
          display: flex; align-items: center; gap: 12px;
          padding: 0 16px; background: #24292f; color: #fff;
          font-weight: 600;
        }
        header .brand { font-size: 16px; }
        header .spacer { flex: 1; }
        header .cfg { font-size: 12px; opacity: .8; font-weight: 400; }
        main {
          display: grid;
          grid-template-columns: 240px 1fr 320px;
          gap: 1px;
          background: #d0d7de;
          overflow: hidden;
        }
        main > * { background: #fff; overflow: auto; }
        footer {
          background: #f6f8fa; color: #57606a;
          font-size: 12px;
          padding: 0 16px;
          display: flex; align-items: center; gap: 12px;
        }
      </style>
      <header>
        <span class="brand">tablec webui</span>
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
    this.shadowRoot.getElementById('cfg').textContent =
      `parser=${appState.activeParser}` +
      (appState.configPath ? ` · cfg=${appState.configPath}` : ' · cfg=(default)');
    bus.on('app:state', (s) => {
      const el = this.shadowRoot.getElementById('cfg');
      el.textContent =
        `parser=${s.activeParser}` +
        (s.configPath ? ` · cfg=${s.configPath}` : ' · cfg=(default)');
    });
    refreshState();
  }
}
customElements.define('app-shell', AppShell);

// -----------------------------------------------------------------------------
// <dir-picker>
// -----------------------------------------------------------------------------

class DirPicker extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: flex; gap: 6px; align-items: center; }
        input {
          background: #32383f; color: #fff; border: 1px solid #444c56;
          border-radius: 4px; padding: 4px 8px; font: inherit; width: 280px;
        }
        button {
          background: #32383f; color: #fff; border: 1px solid #444c56;
          border-radius: 4px; padding: 4px 10px; cursor: pointer; font: inherit;
        }
        button:hover { background: #444c56; }
      </style>
      <input id="dir" value="${appState.dir}">
      <button id="go">打开</button>
      <button id="reload" title="重新扫描目录">⟳</button>
    `;
    const input = shadow.getElementById('dir');
    const go = shadow.getElementById('go');
    const reload = shadow.getElementById('reload');
    go.onclick = async () => {
      appState.dir = input.value || '.';
      bus.emit('app:state', { ...appState });
      await refreshState();
      bus.emit('app:dir-changed', appState.dir);
    };
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') go.click(); });
    reload.onclick = async () => { await refreshState(); };
  }
}
customElements.define('dir-picker', DirPicker);

// -----------------------------------------------------------------------------
// <file-list>
// -----------------------------------------------------------------------------

class FileList extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: block; }
        .empty { padding: 16px; color: #57606a; font-size: 13px; }
        ul { list-style: none; padding: 0; margin: 0; }
        li {
          padding: 6px 12px; cursor: pointer; border-bottom: 1px solid #eaeef2;
          display: flex; flex-direction: column; gap: 2px;
        }
        li:hover { background: #f6f8fa; }
        li.selected { background: #ddf4ff; }
        .name { font-weight: 500; }
        .meta { font-size: 11px; color: #57606a; }
      </style>
      <div id="root"></div>
    `;
    this.render();
    bus.on('app:files', () => this.render());
    bus.on('app:select-file', () => this.render());
  }

  render() {
    const root = this.shadowRoot.getElementById('root');
    if (!appState.files.length) {
      root.innerHTML = `<div class="empty">该目录下没有可识别的数据文件（.xlsx/.xls/.xlsb/.ods）</div>`;
      return;
    }
    root.innerHTML = `<ul>${appState.files.map((f) => `
      <li data-path="${escapeAttr(f.path)}" class="${appState.selectedPath === f.path ? 'selected' : ''}">
        <span class="name">${escapeHtml(f.name)}</span>
        <span class="meta">${humanSize(f.size)} · ${new Date(f.modified_secs * 1000).toLocaleString()}</span>
      </li>`).join('')}</ul>`;
    root.querySelectorAll('li').forEach((li) => {
      li.onclick = () => bus.emit('app:select-file', { path: li.dataset.path });
    });
  }
}
customElements.define('file-list', FileList);

// -----------------------------------------------------------------------------
// <file-preview>
// -----------------------------------------------------------------------------

class FilePreview extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: flex; flex-direction: column; height: 100%; }
        .empty { padding: 16px; color: #57606a; }
        .tabs {
          display: flex; gap: 0; padding: 8px 8px 0; border-bottom: 1px solid #d0d7de;
          background: #f6f8fa; flex-wrap: wrap;
        }
        .tab {
          padding: 6px 12px; cursor: pointer; border-radius: 4px 4px 0 0;
          background: #eaeef2; margin-right: 2px; font-size: 13px;
        }
        .tab.active { background: #fff; border: 1px solid #d0d7de; border-bottom-color: #fff; }
        .body { flex: 1; overflow: auto; padding: 12px; }
        table { border-collapse: collapse; font-size: 12px; min-width: 100%; }
        th, td { border: 1px solid #d0d7de; padding: 4px 8px; text-align: left; vertical-align: top; }
        th { background: #f6f8fa; position: sticky; top: 0; }
        .schema th { background: #fff8c5; }
        .schema td:nth-child(1) { color: #57606a; font-style: italic; }
        .num { text-align: right; font-variant-numeric: tabular-nums; }
        .null { color: #8c959f; }
      </style>
      <div id="root" class="empty">请选择左侧文件以预览。</div>
    `;
    this.render();
    bus.on('app:select-file', ({ path }) => this.onSelect(path));
    bus.on('app:sheet', () => this.render());
    bus.on('app:preview', () => this.render());
  }

  async onSelect(path) {
    appState.selectedPath = path;
    appState.preview = null;
    appState.sheets = [];
    appState.activeSheet = null;
    this.render();
    bus.emit('app:select-file', { path });
    try {
      const sheets = await getJson(`/api/sheets?path=${encodeURIComponent(path)}`);
      appState.sheets = sheets;
      if (sheets.length) {
        appState.activeSheet = sheets[0].name;
        bus.emit('app:sheet', { sheet: appState.activeSheet });
        await this.loadPreview();
      } else {
        this.render();
      }
    } catch (e) {
      appState.lastResult = { error: String(e) };
      this.render();
    }
  }

  async loadPreview() {
    if (!appState.selectedPath || !appState.activeSheet) return;
    try {
      const grid = await getJson(
        `/api/preview?path=${encodeURIComponent(appState.selectedPath)}` +
        `&sheet=${encodeURIComponent(appState.activeSheet)}&max_rows=120`);
      appState.preview = grid;
      bus.emit('app:preview', grid);
    } catch (e) {
      appState.lastResult = { error: String(e) };
      this.render();
    }
  }

  render() {
    const root = this.shadowRoot.getElementById('root');
    if (!appState.selectedPath) {
      root.outerHTML = `<div id="root" class="empty">请选择左侧文件以预览。</div>`;
      return;
    }
    if (!appState.sheets.length) {
      root.outerHTML = `<div id="root" class="empty">${escapeHtml(appState.selectedPath)} 没有可预览的 sheet。</div>`;
      return;
    }
    root.outerHTML = `
      <div id="root" style="display:flex; flex-direction:column; height:100%;">
        <div class="tabs">
          ${appState.sheets.map((s) => `
            <div class="tab ${appState.activeSheet === s.name ? 'active' : ''}"
                 data-sheet="${escapeAttr(s.name)}">
              ${escapeHtml(s.name)}
              <small>(${s.row_count ?? '?'}×${s.col_count ?? '?'})</small>
            </div>`).join('')}
        </div>
        <div class="body" id="body"></div>
      </div>`;
    // After re-render the element is new; re-query.
    const newRoot = this.shadowRoot.getElementById('root');
    newRoot.querySelectorAll('.tab').forEach((t) => {
      t.onclick = () => {
        appState.activeSheet = t.dataset.sheet;
        bus.emit('app:sheet', { sheet: appState.activeSheet });
        this.loadPreview();
      };
    });
    this.renderBody();
  }

  renderBody() {
    const body = this.shadowRoot.getElementById('body');
    if (!body) return;
    if (!appState.preview) {
      body.innerHTML = `<div class="empty">加载中…</div>`;
      return;
    }
    const rows = appState.preview.rows || [];
    const isSchema = (i) => i < 5;
    body.innerHTML = `
      <table>
        <thead>
          <tr>
            <th>#</th>
            ${rows[0]?.map((_, c) => `<th>col ${c + 1}</th>`).join('') ?? ''}
          </tr>
        </thead>
        <tbody>
          ${rows.map((row, ri) => `
            <tr class="${isSchema(ri) ? 'schema' : ''}">
              <th>${ri + 1}${ri < 5 ? ` <small>${schemaLabel(ri)}</small>` : ''}</th>
              ${row.map((cell) => renderCell(cell, isSchema(ri))).join('')}
            </tr>`).join('')}
        </tbody>
      </table>`;
  }
}
customElements.define('file-preview', FilePreview);

function renderCell(cell, isSchema) {
  if (cell === null || cell === undefined) return `<td class="null">·</td>`;
  switch (cell.Bool ?? null) {
    // Booleans serialize to bare true/false — render with style.
  }
  if (typeof cell === 'number') {
    return `<td class="num">${cell}</td>`;
  }
  if (typeof cell === 'string') {
    return `<td>${escapeHtml(cell)}</td>`;
  }
  if (typeof cell === 'boolean') {
    return `<td>${cell ? '✓' : '✗'}</td>`;
  }
  // nested objects (Duration/DateTime come through as plain strings already
  // because of Cell enum).
  return `<td class="null">${escapeHtml(String(cell))}</td>`;
}

function schemaLabel(i) {
  return ['name', 'type', 'comment', 'constraint', 'reserved'][i] || '';
}

// -----------------------------------------------------------------------------
// <build-panel>
// -----------------------------------------------------------------------------

class BuildPanel extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: block; padding: 12px; }
        h3 { margin: 0 0 8px; font-size: 14px; }
        .row { display: flex; gap: 8px; margin-bottom: 12px; align-items: center; }
        select, button {
          padding: 4px 8px; font: inherit; border: 1px solid #d0d7de;
          border-radius: 4px; background: #fff;
        }
        button {
          background: #2da44e; color: #fff; border-color: #1a7f37; cursor: pointer;
          padding: 6px 12px;
        }
        button:hover { background: #1a7f37; }
        button.check { background: #0969da; border-color: #0969da; }
        button.check:hover { background: #0550ae; }
        button.validate {
          background: #bf8700; border-color: #9a6700;
        }
        button.validate:hover { background: #9a6700; }
        button:disabled { opacity: .6; cursor: progress; }
        label { font-size: 12px; color: #57606a; display: flex; gap: 4px; align-items: center; }
        .opts { display: flex; gap: 12px; margin-bottom: 12px; font-size: 12px; }
        .out {
          margin-top: 12px;
          font-size: 12px;
          background: #f6f8fa;
          border: 1px solid #d0d7de;
          border-radius: 4px;
          padding: 8px;
          white-space: pre-wrap;
          word-break: break-all;
          max-height: 200px;
          overflow: auto;
        }
        .diag-list { list-style: none; padding: 0; margin: 8px 0 0; max-height: 240px; overflow: auto; }
        .diag {
          padding: 4px 6px; border-left: 3px solid #d0d7de;
          margin-bottom: 2px; font-size: 12px; background: #f6f8fa;
        }
        .diag.error { border-color: #cf222e; }
        .diag.warning { border-color: #9a6700; }
        .diag .code { color: #57606a; font-family: ui-monospace, monospace; }
      </style>
      <h3>构建</h3>
      <div class="row">
        <label>format
          <select id="fmt">
            <option value="json">json</option>
            <option value="json-pretty" selected>json-pretty</option>
            <option value="msgpack">msgpack</option>
          </select>
        </label>
        <label>parser
          <select id="parser"></select>
        </label>
      </div>
      <div class="opts">
        <label><input type="checkbox" id="pretty"> pretty</label>
        <label><input type="checkbox" id="includeFields"> include_fields</label>
        <label><input type="checkbox" id="write"> write to disk</label>
      </div>
      <div class="row">
        <button id="build">Build</button>
        <button id="check" class="check">Check</button>
        <button id="validate" class="validate">Validate</button>
      </div>
      <div id="out" class="out">尚未运行。</div>
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
      out.textContent = (kind === 'validate' && r.status === 501)
        ? `${kind}: 501 Not Implemented\n${text}`
        : `${kind}: HTTP ${r.status}\n${text}`;
      diag.innerHTML = '';
      const diags = payload.diagnostics || [];
      for (const d of diags) {
        const li = document.createElement('li');
        li.className = `diag ${(d.severity || 'Error').toLowerCase()}`;
        const loc = d.location || {};
        const where = [loc.file, loc.sheet, loc.line, loc.column].filter(Boolean).join(':');
        li.innerHTML = `<div><span class="code">${escapeHtml(d.code || '')}</span> ${escapeHtml(d.message || '')}</div>
                        <small>${escapeHtml(where)}</small>`;
        diag.appendChild(li);
      }
    } catch (e) {
      appState.lastResult = { kind, error: String(e) };
      out.textContent = `error: ${e}`;
    } finally {
      appState.busy = false;
      this.refresh();
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

// -----------------------------------------------------------------------------
// <status-bar>
// -----------------------------------------------------------------------------

class StatusBar extends HTMLElement {
  connectedCallback() {
    const shadow = this.attachShadow({ mode: 'open' });
    shadow.innerHTML = `
      <style>
        :host { display: flex; gap: 16px; align-items: center; font-size: 12px; }
      </style>
      <span id="dir"></span>
      <span id="busy"></span>
      <span id="last"></span>
    `;
    this.render();
    bus.on('app:state', () => this.render());
    bus.on('app:result', () => this.render());
  }
  render() {
    this.shadowRoot.getElementById('dir').textContent =
      `dir: ${appState.dir}`;
    this.shadowRoot.getElementById('busy').textContent =
      appState.busy ? 'busy…' : '';
    const last = appState.lastResult;
    if (last) {
      if (last.error) this.shadowRoot.getElementById('last').textContent = `error: ${last.error}`;
      else if (last.status === 501) this.shadowRoot.getElementById('last').textContent = `validate: 501 (todo)`;
      else if (last.payload?.duration_ms != null) this.shadowRoot.getElementById('last').textContent =
        `${last.kind}: ${last.status} · ${last.payload.duration_ms}ms · ${last.payload.diagnostics?.length ?? 0} diags`;
      else this.shadowRoot.getElementById('last').textContent = `${last.kind}: ${last.status}`;
    } else {
      this.shadowRoot.getElementById('last').textContent = '';
    }
  }
}
customElements.define('status-bar', StatusBar);

// -----------------------------------------------------------------------------
// helpers
// -----------------------------------------------------------------------------

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
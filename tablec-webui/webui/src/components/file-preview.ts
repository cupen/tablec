import { LitElement, html, css, nothing } from 'lit';
import type { TemplateResult } from 'lit';

import { baseName, cellClass, cellClassTyped, colLetter, typeNameOf } from '../format.js';
import { StoreSub, notify, store } from '../store.js';
import type { ParsedCell, ParsedPreview, RawGrid, SheetInfo } from '../store.js';
import { getJson } from '../api.js';

// <file-preview> — formula bar · sheet tabs · parsed/raw grid · summary.
export class FilePreview extends LitElement {
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
      flex-shrink: 0;
      font: 500 10px/1 var(--mono);
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }
    .view-toggle wa-button {
      --wa-form-control-height: 24px;
      --wa-form-control-padding-inline: 9px;
    }
    .view-toggle wa-button::part(base) {
      font: 500 10px/1 var(--mono);
      letter-spacing: 0.08em;
      text-transform: uppercase;
      border-radius: 0;
    }

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
    /* Git diff colors — added green, deleted red, modified amber. */
    table.grid td.cell.diff-added { background: rgba(46, 160, 67, 0.22); }
    table.grid td.cell.diff-deleted { background: rgba(209, 36, 47, 0.22); }
    table.grid td.cell.diff-modified { background: rgba(230, 184, 0, 0.22); }
    table.grid td.cell.diff-added:hover,
    table.grid td.cell.diff-deleted:hover,
    table.grid td.cell.diff-modified:hover {
      filter: brightness(1.05);
    }
    .legend {
      display: flex; align-items: center; gap: 12px;
      font: 400 11px/1 var(--sans);
      color: var(--text-2);
      padding: 6px 12px;
      border-top: 1px solid var(--rule);
      user-select: none;
    }
    .legend .swatch {
      display: inline-block; width: 10px; height: 10px;
      border-radius: 2px; margin-right: 4px;
      vertical-align: -1px;
    }
    .legend .swatch.added { background: rgba(46, 160, 67, 0.6); }
    .legend .swatch.deleted { background: rgba(209, 36, 47, 0.6); }
    .legend .swatch.modified { background: rgba(230, 184, 0, 0.6); }
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
    .empty p { margin: 0 0 14px; }
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

  render(): TemplateResult {
    const { selectedPath, sheets, activeSheet, parsed, preview, selectedCell, previewMode } = store;

    // Resolve coordinate + value for the formula bar based on current mode.
    let coord: string | null = null;
    let cellValue: unknown = null;
    let cellError: string | null = null;
    if (selectedCell) {
      if (previewMode === 'parsed' && parsed?.rows?.length) {
        const row = parsed.rows[selectedCell.row];
        const cell = row?.cells?.[selectedCell.col];
        if (row) coord = colLetter(selectedCell.col) + row.line;
        cellValue = cell?.value;
        cellError = cell?.error ?? null;
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
          ${coord ? html`<span class="sep">▸</span>` : nothing}
          <span class="val ${cellValueClass(cellValue)}">
            ${coord ? renderFormulaValue(cellValue, previewMode) : '点击任意单元格查看坐标'}
          </span>
          ${previewMode === 'parsed' && coord ? html`
            <span class="status ${cellError ? 'err' : 'ok'}">
              ${cellError ? '⚠ err' : '✓ ok'}
            </span>
          ` : nothing}
        </div>
      </div>

      <div class="tabs">
        ${sheets.length === 0 ? nothing : sheets.map((s) => html`
          <div
            class="tab ${activeSheet === s.name ? 'active' : ''}"
            @click=${() => this._selectSheet(s.name)}
          >
            <span>${s.name}</span>
            <span class="size">${s.row_count ?? '?'}×${s.col_count ?? '?'}</span>
          </div>
        `)}
        ${sheets.length > 0 ? html`
          <wa-button-group class="view-toggle" role="tablist" aria-label="preview mode">
            <wa-button
              appearance=${previewMode === 'parsed' ? 'filled' : 'plain'}
              variant="neutral"
              @click=${() => this._setMode('parsed')}
              title="Schema + per-cell validation"
            >Parsed</wa-button>
            <wa-button
              appearance=${previewMode === 'raw' ? 'filled' : 'plain'}
              variant="neutral"
              @click=${() => this._setMode('raw')}
              title="Raw cells from the file"
            >Raw</wa-button>
          </wa-button-group>
        ` : nothing}
      </div>

      ${previewMode === 'parsed' && parsed ? this._renderSummary(parsed) : nothing}

      <div class="body">
        ${this._renderBody()}
      </div>
    `;
  }

  _renderSummary(parsed: ParsedPreview): TemplateResult {
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

  _renderBody(): TemplateResult {
    const { selectedPath, sheets, parsed, preview, previewMode } = store;
    if (!selectedPath) {
      return html`<div class="empty">
        <h3>Pick a file to preview.</h3>
        <p>Three quick steps to see your data laid out — typed and validated:</p>
        <ol class="steps">
          <li><b>1.</b><span>Pick a file from the list on the left</span></li>
          <li><b>2.</b><span>Cells appear here as a parsed grid — typed and validated</span></li>
          <li><b>3.</b><span>Click any cell to inspect its coordinates and value</span></li>
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

  _renderParsedBody(parsed: ParsedPreview): TemplateResult {
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
    const trs: TemplateResult[] = [];

    // Header row 1: column letters
    const headCells: TemplateResult[] = [html`<th class="corner"></th>`];
    for (let c = 0; c < ncols; c++) headCells.push(html`<th>${colLetter(c)}</th>`);
    trs.push(html`<tr class="letters">${headCells}</tr>`);

    // Header row 2: schema info — field name + type chip per column
    const schemaCells: TemplateResult[] = [html`<th class="rowh"><span class="schema-label">schema</span></th>`];
    for (let c = 0; c < ncols; c++) {
      const f = fields[c];
      schemaCells.push(html`<th>
        <span class="field-name">${f.name}</span><span class="col-type">${typeNameOf(f.t)}</span>
      </th>`);
    }
    trs.push(html`<tr class="schema-info">${schemaCells}</tr>`);

    // Data rows
    rows.forEach((row, ri) => {
      const cells: TemplateResult[] = [html`<th class="rowh">${row.line}</th>`];
      for (let c = 0; c < ncols; c++) {
        const cell = row.cells[c];
        if (!cell) continue;
        const isSelected = sel && sel.row === ri && sel.col === c;
        const cls = [
          'cell',
          cell.error ? 'error' : '',
          cellClassTyped(cell.value),
          diffClass(cell.diff),
          isSelected ? 'selected' : '',
        ].filter(Boolean).join(' ');
        const title = cell.error ? `${cell.error} (raw: "${cell.raw || '∅'}")` : '';
        cells.push(html`
          <td
            tabindex="-1"
            class=${cls}
            data-row=${ri}
            data-col=${c}
            title=${title}
            @click=${(e: Event) => this._selectCell(ri, c, e)}
          >${renderTypedCell(cell)}</td>
        `);
      }
      trs.push(html`<tr>${cells}</tr>`);
    });

    return html`<table class="grid"><tbody>${trs}</tbody></table>
      ${hasAnyDiff(parsed) ? html`<div class="legend">
        <span><span class="swatch added"></span>added</span>
        <span><span class="swatch deleted"></span>deleted</span>
        <span><span class="swatch modified"></span>modified</span>
        <span class="meta">vs HEAD</span>
      </div>` : ''}`;
  }

  _renderRawBody(preview: RawGrid): TemplateResult {
    const rows = preview.rows || [];
    if (rows.length === 0) {
      return html`<div class="empty muted"><h3>Empty sheet.</h3><p>This sheet has no rows.</p></div>`;
    }
    const ncols = Math.max(...rows.map((r) => r.length), 1);
    const sel = store.selectedCell;

    const trs: TemplateResult[] = [];
    const headCells: TemplateResult[] = [html`<th class="corner"></th>`];
    for (let c = 0; c < ncols; c++) headCells.push(html`<th>${colLetter(c)}</th>`);
    trs.push(html`<tr class="letters">${headCells}</tr>`);

    rows.forEach((row, ri) => {
      const cells: TemplateResult[] = [html`<th class="rowh">${ri + 1}</th>`];
      for (let c = 0; c < ncols; c++) {
        const cell = row[c];
        const isSelected = sel && sel.row === ri && sel.col === c;
        const cls = ['cell', cellClass(cell), isSelected ? 'selected' : ''].filter(Boolean).join(' ');
        cells.push(html`
          <td
            tabindex="-1"
            class=${cls}
            data-row=${ri}
            data-col=${c}
            @click=${(e: Event) => this._selectCell(ri, c, e)}
          >${renderRawCell(cell)}</td>
        `);
      }
      trs.push(html`<tr>${cells}</tr>`);
    });

    return html`<table class="grid"><tbody>${trs}</tbody></table>`;
  }

  _selectCell(row: number, col: number, e: Event) {
    store.selectedCell = { row, col };
    notify();
    (e.currentTarget as HTMLElement).focus({ preventScroll: true });
  }

  _selectSheet(name: string) {
    if (store.activeSheet === name) return;
    store.activeSheet = name;
    store.selectedCell = null;
    store.parsed = null;
    store.preview = null;
    notify();
    this._loadActive();
  }

  _setMode(mode: 'parsed' | 'raw') {
    if (store.previewMode === mode) return;
    store.previewMode = mode;
    store.selectedCell = null;
    notify();
    if (mode === 'raw' && !store.preview) this._loadRaw();
    else if (mode === 'parsed' && !store.parsed) this._loadParsed();
  }

  /** Entry point for file-list: load sheets for a path, then both views. */
  async _loadFor(path: string) {
    store.selectedPath = path;
    store.sheets = [];
    store.activeSheet = null;
    store.preview = null;
    store.parsed = null;
    store.selectedCell = null;
    notify();
    try {
      const sheets = await getJson<SheetInfo[]>(`/api/sheets?path=${encodeURIComponent(path)}`);
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
      const pp = await getJson<ParsedPreview>(
        `/api/parsed_preview?path=${encodeURIComponent(store.selectedPath)}` +
        `&sheet=${encodeURIComponent(store.activeSheet)}` +
        `&parser=${encodeURIComponent(store.activeParser)}` +
        `&max_rows=120`,
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
      const grid = await getJson<RawGrid>(
        `/api/preview?path=${encodeURIComponent(store.selectedPath)}` +
        `&sheet=${encodeURIComponent(store.activeSheet)}&max_rows=120`,
      );
      store.preview = grid;
      notify();
    } catch (e) {
      store.lastResult = { error: String(e) };
      notify();
    }
  }

  _onKey = (e: KeyboardEvent) => {
    const isParsed = store.previewMode === 'parsed';
    const rows = isParsed ? store.parsed?.rows : store.preview?.rows;
    if (!store.selectedCell || !rows?.length) return;
    const sel = store.selectedCell;
    const nrows = rows.length;
    const ncols = isParsed
      ? store.parsed?.schema?.fields?.length || 1
      : Math.max(
          ...store.preview?.rows.map((r) => (Array.isArray(r) ? r.length : 1)) ?? [1],
          1,
        );
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

// ---- cell render helpers (module scope; pure functions of the data) ----

/** Map a cell diff status to a grid class (empty → none). */
function diffClass(d?: 'added' | 'deleted' | 'modified' | 'unchanged'): string {
  switch (d) {
    case 'added': return 'diff-added';
    case 'deleted': return 'diff-deleted';
    case 'modified': return 'diff-modified';
    default: return '';
  }
}

/** Whether a parsed preview carries any diff status (show the legend). */
function hasAnyDiff(parsed: ParsedPreview): boolean {
  return parsed.rows.some((r) => r.cells.some((c) => c.diff && c.diff !== 'unchanged'));
}

function cellValueClass(v: unknown): string {
  if (v == null) return 'empty';
  if (typeof v === 'number') return 'num';
  if (typeof v === 'boolean') return 'bool';
  return '';
}

function renderFormulaValue(cell: unknown, mode: 'parsed' | 'raw'): TemplateResult {
  if (cell == null) return html`<span style="font-style:italic">∅ empty</span>`;
  if (typeof cell === 'number') return html`${cell}`;
  if (typeof cell === 'boolean') return html`${cell ? 'TRUE' : 'FALSE'}`;
  if (typeof cell === 'string') return html`"${cell}"`;
  if (typeof cell === 'object') {
    if ('Float' in cell && mode === 'raw') return html`${(cell as { Float: number }).Float}`;
    if ('Bool' in cell) return html`${(cell as { Bool: boolean }).Bool ? 'TRUE' : 'FALSE'}`;
    if ('Str' in cell) return html`"${(cell as { Str: string }).Str}"`;
    if ('DateTime' in cell) return html`${String((cell as { DateTime: unknown }).DateTime)}`;
    if ('Duration' in cell) return html`${String((cell as { Duration: unknown }).Duration)}`;
  }
  return html`${JSON.stringify(cell)}`;
}

function renderTypedCell(cell: ParsedCell): TemplateResult {
  if (cell.error) {
    return html`<span class="err-mark">⚠</span><span>${cell.raw || '∅'}</span>`;
  }
  const v = cell.value;
  if (v === undefined || v === null) return html`<span class="null">·</span>`;
  if (typeof v === 'number') return html`${v}`;
  if (typeof v === 'boolean') return html`<span class="bool">${v ? '✓' : '✗'}</span>`;
  if (typeof v === 'string') return html`${v}`;
  if (Array.isArray(v)) return html`${JSON.stringify(v)}`;
  if (typeof v === 'object') return html`${JSON.stringify(v)}`;
  return html`${String(v)}`;
}

function renderRawCell(cell: unknown): TemplateResult {
  if (cell == null) return html`<span class="null">·</span>`;
  if (typeof cell === 'number') return html`${cell}`;
  if (typeof cell === 'string') return html`${cell}`;
  if (typeof cell === 'boolean') return html`<span class="bool">${cell ? '✓' : '✗'}</span>`;
  if (typeof cell === 'object') {
    if ('Float' in cell) return html`${(cell as { Float: number }).Float}`;
    if ('Bool' in cell) return html`<span class="bool">${(cell as { Bool: boolean }).Bool ? '✓' : '✗'}</span>`;
    if ('Str' in cell) return html`${(cell as { Str: string }).Str}`;
    if ('DateTime' in cell) return html`${String((cell as { DateTime: unknown }).DateTime)}`;
    if ('Duration' in cell) return html`${String((cell as { Duration: unknown }).Duration)}`;
    return html`${JSON.stringify(cell)}`;
  }
  return html`${String(cell)}`;
}

customElements.define('file-preview', FilePreview);

declare global {
  interface HTMLElementTagNameMap {
    'file-preview': FilePreview;
  }
}

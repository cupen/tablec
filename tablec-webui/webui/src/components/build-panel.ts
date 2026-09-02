import { LitElement, html, css, nothing } from 'lit';
import type { TemplateResult } from 'lit';

import { StoreSub, notify, store } from '../store.js';
import type { ActionResult } from '../store.js';
import { postJson } from '../api.js';

// Minimal structural types for the Web Awesome form controls we drive.
interface WaSelect extends HTMLElement { value: string; }
interface WaCheckbox extends HTMLElement { checked: boolean; }

type ActionKind = 'build' | 'check' | 'validate';

// <build-panel> — Configuration · Actions · Output zones.
export class BuildPanel extends LitElement {
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
    wa-select { width: 100%; }
    wa-select::part(listbox) {
      background: var(--surface-2);
      color: var(--text);
    }
    .opts {
      display: grid; grid-template-columns: 1fr;
      gap: 8px;
      font: 400 var(--t-12)/1 var(--mono);
    }
    .opts wa-checkbox {
      font: 400 var(--t-12)/1 var(--mono);
      color: var(--text-2);
    }

    .actions {
      display: grid; grid-template-columns: 1fr 1fr 1fr;
      gap: 6px;
    }
    .actions wa-button {
      font-family: var(--mono);
    }
    .actions wa-button::part(base) {
      width: 100%;
      font: 600 var(--t-11)/1.3 var(--mono);
      letter-spacing: 0.06em;
      text-transform: uppercase;
      padding: 6px 4px;
      display: flex; flex-direction: column; align-items: center; gap: 3px;
    }
    .actions wa-button .hint {
      font: 400 9px/1 var(--mono);
      opacity: .55;
      letter-spacing: 0.08em;
      text-transform: none;
    }
    .actions wa-button[data-kind='validate']::part(base) {
      background: transparent;
      border: 1px dashed var(--rule-2);
      color: var(--text-2);
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

  render(): TemplateResult {
    const { parserNames, activeParser, busy, lastResult } = store;
    return html`
      <div class="head">
        <span class="dot" aria-hidden="true"></span>
        <span>BUILD &amp; CHECK</span>
      </div>

      <div class="zone-label">Configuration</div>
      <div class="group">
        <label class="row"><b>format</b></label>
        <wa-select id="fmt" .value=${'json-pretty'} size="small">
          <wa-option value="json">json</wa-option>
          <wa-option value="json-pretty">json-pretty</wa-option>
          <wa-option value="msgpack">msgpack</wa-option>
        </wa-select>
      </div>
      <div class="group">
        <label class="row"><b>parser</b></label>
        <wa-select id="parser" .value=${activeParser} size="small">
          ${parserNames.map((n) => html`<wa-option value=${n}>${n}</wa-option>`)}
        </wa-select>
      </div>
      <div class="opts">
        <wa-checkbox id="pretty">pretty</wa-checkbox>
        <wa-checkbox id="includeFields">include_fields</wa-checkbox>
        <wa-checkbox id="write">write to disk</wa-checkbox>
      </div>

      <div class="zone-sep"></div>
      <div class="zone-label">Actions</div>
      <div class="actions">
        <wa-button
          data-kind="build"
          variant="brand"
          appearance="accent"
          ?disabled=${busy}
          @click=${() => this.runAction('build')}
        >Build <span class="hint">⌘B</span></wa-button>
        <wa-button
          data-kind="check"
          variant="success"
          appearance="accent"
          ?disabled=${busy}
          @click=${() => this.runAction('check')}
        >Check <span class="hint">⌘C</span></wa-button>
        <wa-button
          data-kind="validate"
          variant="neutral"
          appearance="outlined"
          ?disabled=${busy}
          @click=${() => this.runAction('validate')}
        >Validate <span class="hint">501</span></wa-button>
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

  private _fmt(): string {
    return (this.renderRoot.querySelector<WaSelect>('#fmt') as WaSelect | null)?.value ?? 'json-pretty';
  }
  private _parser(): string {
    return (this.renderRoot.querySelector<WaSelect>('#parser') as WaSelect | null)?.value ?? store.activeParser;
  }
  private _checked(id: string): boolean {
    return (this.renderRoot.querySelector<WaCheckbox>(`#${id}`) as WaCheckbox | null)?.checked ?? false;
  }

  async runAction(kind: ActionKind) {
    if (store.busy) return;
    store.busy = true;
    store.lastResult = { kind, status: 0, payload: undefined };
    notify();

    let url = '/api/validate';
    let body: unknown = {};
    if (kind === 'build') {
      url = '/api/build';
      const fmt = this._fmt();
      body = {
        dir: store.dir,
        format: fmt,
        pretty: this._checked('pretty') || fmt === 'json-pretty',
        include_fields: this._checked('includeFields'),
        write: this._checked('write'),
        parser: this._parser(),
        plugin_paths: [],
      };
    } else if (kind === 'check') {
      url = '/api/check';
      body = { dir: store.dir, parser: this._parser(), plugin_paths: [] };
    }

    try {
      const r = await postJson(url, body);
      store.lastResult = { kind, status: r.status, payload: r.payload as ActionResult['payload'] };
    } catch (e) {
      store.lastResult = { kind, error: String(e) };
    } finally {
      store.busy = false;
      notify();
    }
  }

  _renderResult(last: ActionResult | null): TemplateResult {
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
    const ok = last.status !== undefined && last.status >= 200 && last.status < 300 && nErr === 0;
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

  _renderDiagnostics(last: ActionResult | null): TemplateResult | typeof nothing {
    const diags = last?.payload?.diagnostics || [];
    if (diags.length === 0) return nothing;
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
}

customElements.define('build-panel', BuildPanel);

declare global {
  interface HTMLElementTagNameMap {
    'build-panel': BuildPanel;
  }
}

import { LitElement, html, css } from 'lit';
import type { TemplateResult } from 'lit';

import { truncErr } from '../format.js';
import { StoreSub, store } from '../store.js';

// <status-bar> — footer strip: dir, sheet count, last action outcome.
export class StatusBar extends LitElement {
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

  render(): TemplateResult {
    const { dir, sheets, lastResult, busy } = store;
    const last = lastResult;
    let lastCls = '';
    let lastText = '—';
    if (busy) {
      lastCls = 'busy';
      lastText = 'running…';
    } else if (last) {
      if (last.error) {
        lastCls = 'err';
        lastText = `error · ${truncErr(last.error)}`;
      } else if (last.status === 501) {
        lastCls = 'err';
        lastText = 'validate · 501 todo';
      } else if (last.status !== undefined && last.status >= 200 && last.status < 300) {
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

declare global {
  interface HTMLElementTagNameMap {
    'status-bar': StatusBar;
  }
}

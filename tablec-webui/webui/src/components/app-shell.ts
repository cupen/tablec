import { LitElement, html, css } from 'lit';
import type { TemplateResult } from 'lit';

import { StoreSub, store } from '../store.js';
import { ThemeCtrl } from '../theme.js';
import { refreshState } from '../api.js';
import { MOON_ICON, SUN_ICON, RELOAD_ICON } from '../icons.js';
import './file-list.js';
import './file-preview.js';
import './build-panel.js';
import './status-bar.js';

// <app-shell> — top-level layout: header · main grid · footer.
export class AppShell extends LitElement {
  static styles = css`
    :host {
      /* Layout-critical styles (display, grid-template-rows, height, background)
       * are set on the outer 'app-shell' selector in styles.css to win the
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
    .brand wa-badge {
      font: 400 9px/1 var(--mono);
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
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      display: flex; gap: 14px;
      letter-spacing: 0.04em;
    }
    .meta b { color: var(--text); font-weight: 500; }
    .header-btns {
      display: flex; gap: 6px; align-items: center;
    }
    .header-btns wa-button {
      /* Compact header icon buttons — WA's default control height is built
       * for forms; the header wants the original tight toolbar look. */
      --wa-form-control-height: 28px;
      --wa-form-control-padding-inline: 8px;
    }
    .header-btns wa-button::part(base) {
      font: 400 var(--t-11)/1 var(--mono);
      border-radius: 4px;
    }
    /* Three-pane layout: left rail · preview · right build panel.
     * The 1px gap shows --rule as hairline separators. */
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
  private _theme = new ThemeCtrl(this);

  render(): TemplateResult {
    const s = store;
    const dark = this._theme.theme === 'dark';
    return html`
      <header>
        <span class="brand">
          <span class="mark"><i></i><i></i><i></i><i></i><i></i><i></i></span>
          tablec
          <wa-badge appearance="outlined" variant="neutral">webui</wa-badge>
        </span>
        <span class="dir-display" title=${s.dir}>
          <span class="prefix">~/</span><span class="path">${s.dir}</span>
        </span>
        <span class="meta">
          <span>parser <b>${s.activeParser}</b></span>
          <span>cfg <b>${s.configPath ?? '(default)'}</b></span>
        </span>
        <span class="header-btns">
          <wa-button
            appearance="outlined"
            variant="neutral"
            size="small"
            @click=${() => refreshState()}
            title="重新扫描 (⌘R)"
            aria-label="Reload"
          ><span slot="start">${RELOAD_ICON}</span> Reload</wa-button>
          <wa-button
            appearance="outlined"
            variant="neutral"
            size="small"
            @click=${() => this._theme.toggle()}
            title=${dark ? 'Switch to light theme' : 'Switch to dark theme'}
            aria-label="Toggle theme"
          ><span slot="start">${dark ? SUN_ICON : MOON_ICON}</span></wa-button>
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
    // Global keyboard shortcuts: ⌘B build · ⌘C check · ⌘R reload · ⌘T theme.
    window.addEventListener('keydown', this._onKey);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.removeEventListener('keydown', this._onKey);
  }

  _onKey = (e: KeyboardEvent) => {
    if (!(e.metaKey || e.ctrlKey)) return;
    const tag = (e.target as HTMLElement | null)?.tagName?.toLowerCase() ?? '';
    if (tag === 'input' || tag === 'select' || tag === 'textarea') return;
    if (e.key === 'b') { e.preventDefault(); this.shadowRoot?.querySelector('build-panel')?.runAction('build'); }
    else if (e.key === 'c') { e.preventDefault(); this.shadowRoot?.querySelector('build-panel')?.runAction('check'); }
    else if (e.key === 'r') { e.preventDefault(); refreshState(); }
    else if (e.key === 't') { e.preventDefault(); this._theme.toggle(); }
  };
}

customElements.define('app-shell', AppShell);

declare global {
  interface HTMLElementTagNameMap {
    'app-shell': AppShell;
  }
}

// <compare-bar> — the git comparison selector in the app header.
//
// Two dropdowns pick the diff sides: left = 'worktree' or a branch, right =
// a branch / HEAD / any free-form ref (tag, sha). Changing either side
// re-fetches the file list and previews against the new baseline, and the
// choice persists to localStorage (defaults are never stored). Backend
// errors that name a ref (HTTP 400) surface inline until the next change.

import { LitElement, html, css, nothing } from 'lit';
import type { TemplateResult } from 'lit';

import { StoreSub, notify, persistDiffSides, store, SIDES_CHANGED_EVENT } from '../store.js';
import { refetchFiles } from '../api.js';

const CUSTOM = '__custom_ref__';

export class CompareBar extends LitElement {
  static styles = css`
    :host {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.02em;
      white-space: nowrap;
    }
    .label {
      color: var(--text-3);
      text-transform: uppercase;
      letter-spacing: 0.08em;
      font-size: 9px;
    }
    wa-select {
      --wa-form-control-height: 24px;
      --wa-form-control-font-size: 11px;
      min-width: 96px;
      max-width: 160px;
      font-family: var(--mono);
    }
    wa-select::part(control) {
      border-radius: 4px;
    }
    .vs {
      color: var(--text-3);
    }
    input.ref-input {
      height: 24px;
      width: 108px;
      border: 1px solid var(--rule-2);
      border-radius: 4px;
      background: var(--surface-2);
      color: var(--text);
      font: 400 11px/1 var(--mono);
      padding: 0 7px;
      outline: none;
    }
    input.ref-input:focus {
      border-color: var(--accent);
    }
    .error {
      max-width: 240px;
      overflow: hidden;
      text-overflow: ellipsis;
      color: var(--danger, #d24444);
      font-size: 10px;
    }
  `;
  _store = new StoreSub(this);

  /** Right select shows "custom…" when the active ref isn't a listed branch. */
  private get rightIsCustom(): boolean {
    const r = store.diffSides.right;
    return r !== 'HEAD' && !store.branches.includes(r);
  }

  render(): TemplateResult {
    const { left, right } = store.diffSides;
    const leftOptions: Array<[string, string]> = [
      ['worktree', '工作区 (worktree)'],
      ...store.branches.map((b) => [b, b] as [string, string]),
    ];
    const rightOptions: Array<[string, string]> = [
      ['HEAD', 'HEAD'],
      ...store.branches.map((b) => [b, b] as [string, string]),
      [CUSTOM, '其它 ref…'],
    ];
    const rightValue = this.rightIsCustom ? CUSTOM : right;
    return html`
      <span class="label">diff</span>
      <wa-select
        .value=${left}
        size="small"
        title="对比左侧：工作区或分支"
        @wa-change=${(e: Event) => this._apply((e.target as HTMLInputElement).value, right)}
      >
        ${leftOptions.map(
          ([v, label]) => html`<wa-option .value=${v}>${label}</wa-option>`,
        )}
      </wa-select>
      <span class="vs">vs</span>
      <wa-select
        .value=${rightValue}
        size="small"
        title="对比右侧（基线）：分支、HEAD 或任意 ref"
        @wa-change=${(e: Event) => this._rightSelect((e.target as HTMLInputElement).value)}
      >
        ${rightOptions.map(
          ([v, label]) => html`<wa-option .value=${v}>${label}</wa-option>`,
        )}
      </wa-select>
      ${this.rightIsCustom || this._customDraft !== null
        ? html`<input
            class="ref-input"
            type="text"
            placeholder="tag / commit…"
            spellcheck="false"
            .value=${this._customDraft ?? right}
            title="自由输入右侧 ref（tag、commit sha），回车应用"
            @keydown=${(e: KeyboardEvent) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                this._apply(left, (e.target as HTMLInputElement).value.trim());
                this._customDraft = null;
              }
              if (e.key === 'Escape') this._customDraft = null;
            }}
            @input=${(e: InputEvent) => {
              this._customDraft = (e.target as HTMLInputElement).value;
            }}
          />`
        : nothing}
      ${store.diffError
        ? html`<span class="error" title=${store.diffError}>${store.diffError}</span>`
        : nothing}
    `;
  }

  /** Draft text for the custom-ref input (null = not editing). */
  private _customDraft: string | null = null;

  private _rightSelect(v: string) {
    if (v === CUSTOM) {
      // Reveal the input; keep current right until Enter applies a new one.
      this._customDraft = '';
      this.requestUpdate();
      return;
    }
    this._customDraft = null;
    this._apply(store.diffSides.left, v);
  }

  private _apply(left: string, right: string) {
    if (!left || !right || right.toLowerCase() === 'worktree') return;
    const changed =
      left !== store.diffSides.left || right !== store.diffSides.right;
    store.diffSides = { left, right };
    store.diffError = null;
    persistDiffSides(store.diffSides);
    notify();
    if (changed) {
      void refetchFiles();
      document.dispatchEvent(new CustomEvent(SIDES_CHANGED_EVENT));
    }
    this.requestUpdate();
  }
}

customElements.define('compare-bar', CompareBar);

declare global {
  interface HTMLElementTagNameMap {
    'compare-bar': CompareBar;
  }
}

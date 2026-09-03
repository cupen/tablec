import { LitElement, html, css } from 'lit';
import type { TemplateResult } from 'lit';

import { refreshState } from '../api.js';
import { extOf, humanSize } from '../format.js';
import {
  StoreSub,
  notify,
  store,
  visibleFileCount,
  visibleFiles,
} from '../store.js';
import type { FileStatus } from '../store.js';
import type { FilePreview } from './file-preview.js';

// <file-list> — left rail: scanned spreadsheets; click selects + previews.
// Supports an "All files / Modified only" filter that re-fetches /api/files
// with `filter=modified` and shows a colored dot + add/del counts per file.
export class FileList extends LitElement {
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
    .filter {
      display: flex;
      gap: 4px;
      padding: 6px 16px 4px;
      border-bottom: 1px solid var(--rule);
    }
    .filter button {
      flex: 1;
      font: 500 11px/1 var(--sans);
      color: var(--text-2);
      background: transparent;
      border: 1px solid var(--rule);
      border-radius: 4px;
      padding: 5px 8px;
      cursor: pointer;
      transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    }
    .filter button:hover { background: var(--surface-2); }
    .filter button.active {
      color: var(--accent);
      border-color: var(--accent);
      background: var(--accent-2-soft);
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
    .name .status-dot {
      width: 7px; height: 7px;
      border-radius: 50%;
      flex-shrink: 0;
    }
    .status-dot.modified { background: #e6b800; }   /* changed → amber */
    .status-dot.added,
    .status-dot.untracked { background: #2ea043; } /* new → green */
    .status-dot.deleted { background: #d1242f; }    /* removed → red */
    .status-dot.clean { display: none; }
    .meta {
      font: 400 var(--t-11)/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.02em;
      display: flex; align-items: center; gap: 6px;
    }
    .meta .numstat {
      font: 500 10px/1 var(--mono);
      font-variant-numeric: tabular-nums;
    }
    .meta .numstat .add { color: #2ea043; }
    .meta .numstat .del { color: #d1242f; }
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
    .empty p { margin: 0 0 12px; }
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

  render(): TemplateResult {
    const { files, selectedPath, dir, inputDir } = store;
    const files2 = visibleFiles();
    const count = visibleFileCount();
    const filter = store.filesFilter;
    return html`
      <div class="head">
        <span class="dot" aria-hidden="true"></span>
        <span>FILES</span>
        <span class="count">${String(count).padStart(2, '0')}</span>
      </div>
      <div class="filter" role="group" aria-label="File filter">
        <button
          class=${filter === 'all' ? 'active' : ''}
          @click=${() => this._setFilter('all')}
        >All files</button>
        <button
          class=${filter === 'modified' ? 'active' : ''}
          @click=${() => this._setFilter('modified')}
        >Modified only</button>
      </div>
      ${files.length === 0
        ? html`<div class="empty">
            <h3>No spreadsheets here yet.</h3>
            <p>Put <code>.xlsx</code> / <code>.xls</code> / <code>.xlsb</code> / <code>.ods</code> files in the folder being scanned, then press Reload (⌘R) above.</p>
            <div class="step"><b>1.</b><span>Drop spreadsheet files into the scanned folder</span></div>
            <div class="step"><b>2.</b><span>Press Reload to rescan</span></div>
            <div class="step"><b>3.</b><span>Click one to preview and build</span></div>
            <div class="hint">Currently scanning: <code>${inputDir || dir || '.'}</code></div>
          </div>`
        : html`<ul>${files2.map((f) => html`
            <li
              class=${selectedPath === f.path ? 'selected' : ''}
              @click=${() => this._select(f.path)}
            >
              <span class="name">
                <span class="status-dot ${f.status || 'clean'}" title=${statusTitle(f.status)}></span>
                ${f.name}
                <span class="ext">${extOf(f.name)}</span>
              </span>
              <span class="meta">
                <span>${humanSize(f.size)} · ${new Date(f.modified_secs * 1000).toLocaleString()}</span>
                ${numstat(f)}
              </span>
            </li>
          `)}</ul>
          ${files2.length === 0 && files.length > 0
            ? html`<div class="empty"><h3>No modified files.</h3><p>Everything is up to date with HEAD. Switch to “All files” to see the full list.</p></div>`
            : ''}
        `}
    `;
  }

  _setFilter(f: 'all' | 'modified') {
    if (store.filesFilter === f) return;
    store.filesFilter = f;
    notify();
    // Re-fetch so the list matches the active filter (the server filters too).
    void refreshState();
  }

  _select(path: string) {
    if (store.selectedPath === path) return;
    store.selectedPath = path;
    store.sheets = [];
    store.activeSheet = null;
    store.preview = null;
    store.selectedCell = null;
    notify();
    // file-list and file-preview are siblings inside <app-shell>'s shadow
    // root; document.querySelector can't see past the shadow boundary.
    (this.getRootNode() as ShadowRoot)
      .querySelector<FilePreview>('file-preview')
      ?._loadFor(path);
  }
}

function statusTitle(s?: FileStatus): string {
  switch (s) {
    case 'modified': return 'Modified';
    case 'added': return 'Added (staged)';
    case 'untracked': return 'Untracked (new)';
    case 'deleted': return 'Deleted';
    default: return 'No changes';
  }
}

function numstat(f: { status?: FileStatus; numstat_added?: number; numstat_deleted?: number }) {
  if (f.status !== 'modified' || (!f.numstat_added && !f.numstat_deleted)) return '';
  return html`<span class="numstat"><span class="add">+${f.numstat_added ?? 0}</span> <span class="del">−${f.numstat_deleted ?? 0}</span></span>`;
}

customElements.define('file-list', FileList);

declare global {
  interface HTMLElementTagNameMap {
    'file-list': FileList;
  }
}

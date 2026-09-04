import { LitElement, html, css } from 'lit';
import type { TemplateResult } from 'lit';

import { refreshState } from '../api.js';
import { extOf, humanSize } from '../format.js';
import {
  StoreSub,
  buildTree,
  notify,
  relPathOf,
  sortTree,
  store,
  visibleFileCount,
  visibleFiles,
} from '../store.js';
import type { FileEntry, FileStatus, SortFactor, TreeDir } from '../store.js';
import type { FilePreview } from './file-preview.js';

// <file-list> — left rail: the /api/files listing rendered as a compact
// directory tree (one 24px line per row). Clicking a directory toggles it;
// clicking a file selects + previews it. "All files / Modified only" re-fetches
// /api/files with `filter=modified`; error/warning badges come from the
// post-listing /api/check run (diagnosticsByFile in the store).
export class FileList extends LitElement {
  static styles = css`
    :host { display: block; }
    .head {
      position: sticky; top: 0; z-index: 2;
      padding: 12px 12px 10px 16px;
      background: linear-gradient(180deg, var(--surface) 0%, var(--surface) 70%, transparent 100%);
      border-bottom: 1px solid var(--rule);
      font: 500 10px/1 var(--mono);
      color: var(--text-2);
      letter-spacing: 0.18em;
      text-transform: uppercase;
      display: flex; align-items: center; gap: 8px;
    }
    .head .dot {
      display: inline-block;
      width: 6px; height: 6px;
      background: var(--accent-2);
      border-radius: 1px;
    }
    .head .count {
      color: var(--text);
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }
    .head .etotal {
      font: 600 10px/1 var(--mono);
      color: #fff;
      background: #d1242f;
      padding: 2px 6px;
      border-radius: 8px;
      font-variant-numeric: tabular-nums;
      letter-spacing: 0;
    }
    .head .etotal.warn-only {
      color: #1a1a1a;
      background: #e6b800;
    }
    .sort {
      margin-left: auto;
      display: inline-flex; align-items: center; gap: 3px;
      letter-spacing: 0;
      text-transform: none;
    }
    .sort-factor {
      font: 500 10px/1 var(--mono);
      color: var(--text);
      background: transparent;
      border: 1px solid var(--rule);
      border-radius: 4px;
      height: 20px;
      padding: 0 1px;
      cursor: pointer;
    }
    .sort-factor:hover { background: var(--surface-2); }
    .sort-factor:focus-visible { outline: 1px solid var(--accent); }
    .sort-dir {
      font: 600 11px/1 var(--mono);
      color: var(--text-2);
      background: transparent;
      border: 1px solid var(--rule);
      border-radius: 4px;
      height: 20px; width: 20px;
      padding: 0;
      cursor: pointer;
    }
    .sort-dir:hover { background: var(--surface-2); color: var(--text); }
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
    .tree { padding-bottom: 4px; }
    .row {
      display: flex; align-items: center; gap: 6px;
      min-height: 24px;
      padding: 2px 12px 2px calc(12px + var(--depth, 0) * 13px);
      border-bottom: 1px solid var(--rule);
      border-left: 2px solid transparent;
      cursor: pointer;
      white-space: nowrap;
      font: 400 var(--t-12)/1.2 var(--sans);
      color: var(--text);
      transition: background 100ms ease, border-color 100ms ease;
    }
    .row:hover { background: var(--surface-2); }
    .row.selected {
      background: var(--surface-2);
      border-left-color: var(--accent);
    }
    .row.dir { font-weight: 500; }
    .children {
      border-left: 1px solid var(--rule);
      margin-left: calc(19px + var(--depth, 0) * 13px);
    }
    .chevron {
      width: 9px;
      flex-shrink: 0;
      font: 500 9px/1 var(--mono);
      color: var(--text-2);
    }
    .dname, .fname {
      overflow: hidden;
      text-overflow: ellipsis;
      min-width: 0;
      flex: 0 1 auto;
    }
    .spacer { flex: 1 0 6px; }
    .dcount {
      font: 500 10px/1 var(--mono);
      color: var(--text-2);
      font-variant-numeric: tabular-nums;
      flex-shrink: 0;
    }
    .ext {
      font: 500 9px/1 var(--mono);
      color: var(--accent-2);
      padding: 2px 5px;
      background: var(--accent-2-soft);
      border: 1px solid var(--rule-2);
      border-radius: 2px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      flex-shrink: 0;
    }
    .badge {
      font: 600 9px/1 var(--mono);
      color: #fff;
      background: #d1242f;
      padding: 2px 5px;
      border-radius: 8px;
      font-variant-numeric: tabular-nums;
      flex-shrink: 0;
    }
    .badge.warn {
      color: #1a1a1a;
      background: #e6b800;
    }
    .status-dot {
      width: 7px; height: 7px;
      border-radius: 50%;
      flex-shrink: 0;
    }
    .status-dot.modified { background: #e6b800; }   /* changed → amber */
    .status-dot.added,
    .status-dot.untracked { background: #2ea043; } /* new → green */
    .status-dot.deleted { background: #d1242f; }    /* removed → red */
    .status-dot.clean { display: none; }
    .numstat {
      font: 500 10px/1 var(--mono);
      font-variant-numeric: tabular-nums;
      flex-shrink: 0;
    }
    .numstat .add { color: #2ea043; }
    .numstat .del { color: #d1242f; }
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
  /** Ancestor dirs force-expanded under the modified-only filter (render-time). */
  private _autoExpand = new Set<string>();

  render(): TemplateResult {
    const { files, selectedPath, dir, inputDir, filesFilter } = store;
    const visible = visibleFiles();
    const count = visibleFileCount();
    const tree = buildTree(visible, store.diagnosticsByFile);
    sortTree(tree, store.sortFactor, store.sortAsc);

    // Under the modified-only filter the ancestors of every visible file are
    // effectively expanded, so filtered results are reachable without manual
    // expansion. Computed at render, never written back — switching back to
    // "All files" restores the user's manual expansion state untouched.
    this._autoExpand = new Set<string>();
    if (filesFilter === 'modified') {
      for (const f of visible) {
        let p = relPathOf(f);
        for (let sep = p.lastIndexOf('/'); sep !== -1; sep = p.lastIndexOf('/')) {
          p = p.slice(0, sep);
          this._autoExpand.add(p);
        }
      }
    }

    const totalErrors = tree.errorCount;
    const totalWarnings = tree.warningCount;

    return html`
      <div class="head">
        <span class="dot" aria-hidden="true"></span>
        <span>FILES</span>
        <span class="count">${String(count).padStart(2, '0')}</span>
        ${headerHealth(totalErrors, totalWarnings)}
        <span class="sort" role="group" aria-label="Sort files">
          <select
            class="sort-factor"
            aria-label="Sort factor"
            title="Sort files"
            .value=${store.sortFactor}
            @change=${this._setFactor}
          >
            <option value="name" ?selected=${store.sortFactor === 'name'}>name</option>
            <option value="modified" ?selected=${store.sortFactor === 'modified'}>modified</option>
            <option value="errors" ?selected=${store.sortFactor === 'errors'}>errors</option>
          </select>
          <button
            class="sort-dir"
            @click=${this._toggleSortDir}
            title=${store.sortAsc ? 'Ascending — click to reverse' : 'Descending — click to reverse'}
            aria-label=${store.sortAsc ? 'Sort ascending' : 'Sort descending'}
          >${store.sortAsc ? '↑' : '↓'}</button>
        </span>
      </div>
      <div class="filter" role="group" aria-label="File filter">
        <button
          class=${filesFilter === 'all' ? 'active' : ''}
          @click=${() => this._setFilter('all')}
        >All files</button>
        <button
          class=${filesFilter === 'modified' ? 'active' : ''}
          @click=${() => this._setFilter('modified')}
        >Modified only</button>
      </div>
      ${files.length === 0
        ? html`<div class="empty">
            <h3>No spreadsheets here yet.</h3>
            <p>Put <code>.xlsx</code> / <code>.xls</code> files in the folder being scanned, then press Reload (⌘R) above.</p>
            <div class="step"><b>1.</b><span>Drop spreadsheet files into the scanned folder</span></div>
            <div class="step"><b>2.</b><span>Press Reload to rescan</span></div>
            <div class="step"><b>3.</b><span>Click one to preview and build</span></div>
            <div class="hint">No files match the build include patterns (default <code>*.xlsx</code>, scan-root only). Add <code>include</code> patterns to <code>tablec.toml</code> — e.g. <code>include = ["/**/*.xlsx"]</code> to also scan subfolders. Currently scanning: <code>${inputDir || dir || '.'}</code></div>
          </div>`
        : html`<div class="tree" role="tree" aria-label="Table files">
            ${tree.dirs.map((d) => this._dirRow(d, 0))}
            ${tree.files.map((f) => this._fileRow(f, selectedPath === f.path, 0))}
          </div>
          ${visible.length === 0 && files.length > 0
            ? html`<div class="empty"><h3>No modified files.</h3><p>Everything is up to date with HEAD. Switch to “All files” to see the full list.</p></div>`
            : ''}
        `}
    `;
  }

  /** A directory row: chevron + name + aggregate health + contained count. */
  private _dirRow(d: TreeDir, depth: number): TemplateResult {
    const open = this._isOpen(d.path);
    return html`
      <div
        class="row dir"
        role="treeitem"
        aria-expanded=${open ? 'true' : 'false'}
        style=${`--depth:${depth}`}
        title=${`${d.path} — ${d.totalFiles} file${d.totalFiles === 1 ? '' : 's'} inside; click to ${open ? 'collapse' : 'expand'}`}
        @click=${() => this._toggleDir(d.path)}
      >
        <span class="chevron" aria-hidden="true">${open ? '▾' : '▸'}</span>
        <span class="dname">${d.name}</span>
        <span class="spacer"></span>
        ${healthBadge(d.errorCount, d.warningCount)}
        <span class="dcount">${d.totalFiles}</span>
      </div>
      ${open
        ? html`<div class="children" role="group" style=${`--depth:${depth}`}>
            ${d.dirs.map((c) => this._dirRow(c, depth + 1))}
            ${d.files.map((f) => this._fileRow(f, store.selectedPath === f.path, depth + 1))}
          </div>`
        : ''}
    `;
  }

  /** A leaf row: status dot + name + ext badge; health + numstat right-aligned. */
  private _fileRow(f: FileEntry, selected: boolean, depth: number): TemplateResult {
    const health = store.diagnosticsByFile.get(relPathOf(f));
    // Single-line row: size and modification time live on hover, not a line.
    const title = `${relPathOf(f)} · ${humanSize(f.size)} · ${new Date(f.modified_secs * 1000).toLocaleString()}`;
    return html`
      <div
        class="row file ${selected ? 'selected' : ''}"
        role="treeitem"
        aria-selected=${selected ? 'true' : 'false'}
        style=${`--depth:${depth}`}
        title=${title}
        @click=${() => this._select(f.path)}
      >
        <span class="status-dot ${f.status || 'clean'}" title=${statusTitle(f.status)}></span>
        <span class="fname">${f.name}</span>
        <span class="ext">${extOf(f.name)}</span>
        <span class="spacer"></span>
        ${numstat(f)}
        ${healthBadge(health?.errors ?? 0, health?.warnings ?? 0)}
      </div>
    `;
  }

  /**
   * Effective expansion: a directory is open unless the user collapsed it —
   * or the modified-only filter is force-expanding its subtree.
   */
  private _isOpen(path: string): boolean {
    return (
      (store.filesFilter === 'modified' && this._autoExpand.has(path)) ||
      !store.expandedDirs.has(path)
    );
  }

  _setFilter(f: 'all' | 'modified') {
    if (store.filesFilter === f) return;
    store.filesFilter = f;
    notify();
    // Re-fetch so the list matches the active filter (the server filters too).
    void refreshState();
  }

  _setFactor(e: Event) {
    const v = (e.target as HTMLSelectElement).value as SortFactor;
    if (store.sortFactor === v) return;
    store.sortFactor = v;
    notify();
  }

  _toggleSortDir() {
    store.sortAsc = !store.sortAsc;
    notify();
  }

  _toggleDir(path: string) {
    // expandedDirs holds explicitly collapsed dirs; absence = expanded.
    if (store.expandedDirs.has(path)) store.expandedDirs.delete(path);
    else store.expandedDirs.add(path);
    notify();
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

/** Header health: total error count for the visible listing (tooltip adds warnings). */
function headerHealth(errors: number, warnings: number): TemplateResult | string {
  if (errors > 0) {
    return html`<span class="etotal" title=${`${errors} error${errors === 1 ? '' : 's'}${warnings ? `, ${warnings} warning${warnings === 1 ? '' : 's'}` : ''} in visible files`}>${errors}</span>`;
  }
  if (warnings > 0) {
    return html`<span class="etotal warn-only" title=${`${warnings} warning${warnings === 1 ? '' : 's'} in visible files`}>${warnings}</span>`;
  }
  return '';
}

/** Row health badge: red error count, or amber warning count when error-free. */
function healthBadge(errors: number, warnings: number): TemplateResult | string {
  if (errors > 0) {
    const t =
      `${errors} error${errors === 1 ? '' : 's'}` +
      (warnings > 0 ? `, ${warnings} warning${warnings === 1 ? '' : 's'}` : '');
    return html`<span class="badge" title=${t}>${errors}</span>`;
  }
  if (warnings > 0) {
    return html`<span class="badge warn" title=${`${warnings} warning${warnings === 1 ? '' : 's'}`}>${warnings}</span>`;
  }
  return '';
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

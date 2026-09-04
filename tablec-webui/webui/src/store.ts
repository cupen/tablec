// Central UI store: a single mutable object + a change bus.
//
// Components never hold copies of shared state; they read `store` inside
// `render()` and subscribe with `StoreSub`, which re-renders them whenever
// `notify()` fires.

import type { ReactiveController, ReactiveControllerHost } from 'lit';

// ---- API shapes (mirror tablec-webui/src/handlers.rs) ----

export interface FileEntry {
  name: string;
  path: string;
  /** Path relative to the resolved input dir, separators normalized to `/`. */
  rel_path: string;
  size: number;
  modified_secs: number;
  /** Git change status vs current branch HEAD: modified|added|untracked|deleted|clean */
  status?: FileStatus;
  numstat_added?: number;
  numstat_deleted?: number;
}

export type FileStatus = 'modified' | 'added' | 'untracked' | 'deleted' | 'clean';

export type FilesFilter = 'all' | 'modified';

/** True when a file status counts as "changed" for the Modified-only filter. */
export function isChangedStatus(s?: FileStatus): boolean {
  return !!s && s !== 'clean';
}

/** Check-pipeline problem counts for one file (from /api/check diagnostics). */
export interface FileDiagnostics {
  errors: number;
  warnings: number;
}

/** Rail sort factor: file name, modification time, or error count. */
export type SortFactor = 'name' | 'modified' | 'errors';

/** One directory node of the file-rail tree (the virtual root has path ''). */
export interface TreeDir {
  /** Display name: last path segment ('' for the virtual root). */
  name: string;
  /** Full relative directory path ('' for the virtual root). */
  path: string;
  /** Subdirectories, in insertion order (sorted at render time). */
  dirs: TreeDir[];
  /** Files directly in this directory, in insertion order. */
  files: FileEntry[];
  /** Direct files in this directory. */
  fileCount: number;
  /** Files contained in this whole subtree (incl. nested directories). */
  totalFiles: number;
  /** Latest modification time in this subtree (0 when unknown). */
  latestModified: number;
  /** Aggregated check problems over this subtree. */
  errorCount: number;
  warningCount: number;
}

/** `rel_path` normalized to `/` separators (defensive: backend already does). */
export function relPathOf(f: FileEntry): string {
  return (f.rel_path || f.name).replace(/\\/g, '/');
}

/**
 * Derive a directory trie from the flat listing: split each entry's
 * `rel_path` on `/`, fold into directories, and aggregate per-directory
 * counts (file totals, latest mtime, check problems) bottom-up.
 */
export function buildTree(files: FileEntry[], diags: Map<string, FileDiagnostics>): TreeDir {
  const mkDir = (name: string, path: string): TreeDir => ({
    name,
    path,
    dirs: [],
    files: [],
    fileCount: 0,
    totalFiles: 0,
    latestModified: 0,
    errorCount: 0,
    warningCount: 0,
  });
  const root = mkDir('', '');
  const index = new Map<string, TreeDir>([['', root]]);
  const ensureDir = (dirPath: string): TreeDir => {
    const existing = index.get(dirPath);
    if (existing) return existing;
    const sep = dirPath.lastIndexOf('/');
    const parent = ensureDir(sep === -1 ? '' : dirPath.slice(0, sep));
    const dir = mkDir(dirPath.slice(sep + 1), dirPath);
    parent.dirs.push(dir);
    index.set(dirPath, dir);
    return dir;
  };
  for (const f of files) {
    const rel = relPathOf(f);
    const sep = rel.lastIndexOf('/');
    ensureDir(sep === -1 ? '' : rel.slice(0, sep)).files.push(f);
  }
  const aggregate = (d: TreeDir): void => {
    let total = d.files.length;
    let latest = 0;
    let errors = 0;
    let warnings = 0;
    for (const f of d.files) {
      latest = Math.max(latest, f.modified_secs || 0);
      const c = diags.get(relPathOf(f));
      if (c) {
        errors += c.errors;
        warnings += c.warnings;
      }
    }
    for (const child of d.dirs) {
      aggregate(child);
      total += child.totalFiles;
      latest = Math.max(latest, child.latestModified);
      errors += child.errorCount;
      warnings += child.warningCount;
    }
    d.fileCount = d.files.length;
    d.totalFiles = total;
    d.latestModified = latest;
    d.errorCount = errors;
    d.warningCount = warnings;
  };
  aggregate(root);
  return root;
}

/**
 * Sort a tree in place by the rail's sort factor. Files order within their
 * directory; directories order by the matching aggregate (name →
 * alphabetical, modified → latest contained mtime, errors → total contained
 * errors with warnings as tiebreak). Missing check counts compare as zero,
 * so error-ordering degrades to name order when no check results exist
 * (name is the final tiebreak everywhere, keeping the order deterministic).
 */
export function sortTree(root: TreeDir, factor: SortFactor, asc: boolean): void {
  const dir = (n: number): number => (asc ? n : -n);
  const byName = (a: { name: string }, b: { name: string }): number =>
    a.name.localeCompare(b.name);
  const fileHealth = (f: FileEntry): FileDiagnostics =>
    store.diagnosticsByFile.get(relPathOf(f)) ?? { errors: 0, warnings: 0 };

  const cmpFiles = (a: FileEntry, b: FileEntry): number => {
    switch (factor) {
      case 'modified':
        return dir((a.modified_secs || 0) - (b.modified_secs || 0)) || byName(a, b);
      case 'errors': {
        const ka = fileHealth(a);
        const kb = fileHealth(b);
        return dir(ka.errors - kb.errors) || dir(ka.warnings - kb.warnings) || byName(a, b);
      }
      default:
        return dir(byName(a, b));
    }
  };
  const cmpDirs = (a: TreeDir, b: TreeDir): number => {
    switch (factor) {
      case 'modified':
        return dir(a.latestModified - b.latestModified) || byName(a, b);
      case 'errors':
        return (
          dir(a.errorCount - b.errorCount) ||
          dir(a.warningCount - b.warningCount) ||
          byName(a, b)
        );
      default:
        return dir(byName(a, b));
    }
  };

  const walk = (d: TreeDir): void => {
    d.files.sort(cmpFiles);
    d.dirs.sort(cmpDirs);
    for (const c of d.dirs) walk(c);
  };
  walk(root);
}

export interface SheetInfo {
  name: string;
  row_count: number;
  col_count: number;
}

export interface Field {
  name: string;
  /** serde-tagged FieldType — either "Int32" or {Variant: {...}} */
  t: unknown;
}

export interface ParsedCell {
  raw: string;
  value: unknown;
  error: string | null;
  type_name: string;
  /** Git diff status vs HEAD: 'added'|'deleted'|'modified'|'unchanged'. Absent when no baseline. */
  diff?: 'added' | 'deleted' | 'modified' | 'unchanged';
}

export interface DiffSummary {
  compared_rows: number;
  added_rows: number;
  deleted_rows: number;
  modified_cells: number;
  changed_cells: number;
}

export interface ParsedRow {
  row_index: number;
  line: number;
  cells: ParsedCell[];
  error_count: number;
}

export interface PreviewSummary {
  data_rows: number;
  shown_rows: number;
  total_rows: number;
  error_count: number;
  warning_count: number;
}

export interface ParsedSchemaInfo {
  fields: Field[];
  constraints: unknown[];
}

export interface Diagnostic {
  severity?: string;
  code?: string;
  message?: string;
  location?: {
    file?: string;
    sheet?: string;
    line?: number;
    column?: number;
  };
}

export interface ParsedPreview {
  sheet: string;
  schema: ParsedSchemaInfo | null;
  data_start_row: number;
  total_rows: number;
  shown_rows: number;
  rows: ParsedRow[];
  diagnostics: Diagnostic[];
  summary: PreviewSummary;
  diff_summary?: DiffSummary;
}

/** Legacy raw grid from /api/preview (Cell is a tagged enum). */
export interface RawGrid {
  sheet: string;
  rows: unknown[][];
}

export interface StateBody {
  dir: string;
  parser_names: string[];
  active_parser: string;
  config_path: string | null;
  config_present: boolean;
  input_dir: string;
}

export type ActionResult = {
  kind?: 'build' | 'check' | 'validate';
  status?: number;
  payload?: {
    diagnostics?: Diagnostic[];
    duration_ms?: number;
    bytes?: number | null;
    output_path?: string | null;
    preview_first_500?: string | null;
    sheets_checked?: number;
  };
  error?: string;
};

// ---- the store ----

export interface AppStore {
  dir: string;
  inputDir: string | null;
  files: FileEntry[];
  selectedPath: string | null;
  sheets: SheetInfo[];
  activeSheet: string | null;
  /** legacy raw grid (from /api/preview) — used by the raw view toggle */
  preview: RawGrid | null;
  /** { schema, rows, summary, diagnostics } from /api/parsed_preview */
  parsed: ParsedPreview | null;
  /** { row, col } in 0-indexed grid coords (parsed view: data-row offset) */
  selectedCell: { row: number; col: number } | null;
  parserNames: string[];
  activeParser: string;
  configPath: string | null;
  previewMode: 'parsed' | 'raw';
  /** Left-menu filter: show all files, or only files with git changes. */
  filesFilter: FilesFilter;
  /**
   * Directory paths (rel_path form, `/` separators) the user explicitly
   * collapsed. A directory NOT in the set renders expanded — so freshly
   * appearing directories default to open, matching the old flat "everything
   * visible" behavior. Re-fetches replace `files` but never this set, which
   * is what makes expansion survive reloads and live refreshes.
   */
  expandedDirs: Set<string>;
  /** Per-file check results keyed by rel_path ('/' separators). */
  diagnosticsByFile: Map<string, FileDiagnostics>;
  /** True while the post-listing /api/check is in flight. */
  checkRunning: boolean;
  /** Rail sort: factor + direction (default name ascending). */
  sortFactor: SortFactor;
  sortAsc: boolean;
  busy: boolean;
  lastResult: ActionResult | null;
}

export const store: AppStore = {
  dir: '.',
  inputDir: null,
  files: [],
  selectedPath: null,
  sheets: [],
  activeSheet: null,
  preview: null,
  parsed: null,
  selectedCell: null,
  parserNames: [],
  activeParser: 'standard',
  configPath: null,
  previewMode: 'parsed',
  filesFilter: 'all',
  expandedDirs: new Set<string>(),
  diagnosticsByFile: new Map<string, FileDiagnostics>(),
  checkRunning: false,
  sortFactor: 'name',
  sortAsc: true,
  busy: false,
  lastResult: null,
};

/** Count of files the active filter shows: total, or changed-only. */
export function visibleFileCount(): number {
  return store.filesFilter === 'modified'
    ? store.files.filter((f) => isChangedStatus(f.status)).length
    : store.files.length;
}

/** Files the left menu renders under the active filter. */
export function visibleFiles(): FileEntry[] {
  return store.filesFilter === 'modified'
    ? store.files.filter((f) => isChangedStatus(f.status))
    : store.files;
}

const bus = new EventTarget();
export const notify = () => bus.dispatchEvent(new Event('change'));

// ---- subscription controller ----

/** Subscribe a LitElement to store changes; re-renders are auto-batched. */
export class StoreSub implements ReactiveController {
  private host: ReactiveControllerHost;
  private onChange = () => this.host.requestUpdate();

  constructor(host: ReactiveControllerHost) {
    host.addController(this);
    this.host = host;
  }
  hostConnected() {
    bus.addEventListener('change', this.onChange);
  }
  hostDisconnected() {
    bus.removeEventListener('change', this.onChange);
  }
}

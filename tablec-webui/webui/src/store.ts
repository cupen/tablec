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
  size: number;
  modified_secs: number;
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
  busy: false,
  lastResult: null,
};

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

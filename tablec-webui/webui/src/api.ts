// Typed fetch helpers + the /api/state bootstrap.

import { notify, store } from './store.js';
import type {
  Diagnostic,
  FileDiagnostics,
  FileEntry,
  StateBody,
} from './store.js';

export async function getJson<T = unknown>(url: string): Promise<T> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json() as Promise<T>;
}

export async function postJson<T = unknown>(
  url: string,
  body: unknown,
): Promise<{ status: number; payload: T }> {
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const text = await r.text();
  let payload: unknown;
  try {
    payload = JSON.parse(text);
  } catch {
    payload = { raw: text };
  }
  return { status: r.status, payload: payload as T };
}

interface CheckResponse {
  diagnostics?: Diagnostic[];
  duration_ms?: number;
  sheets_checked?: number;
}

/** Bootstrap / reload: server state first, then the file list. */
export async function refreshState(): Promise<void> {
  try {
    const s = await getJson<StateBody>('/api/state');
    store.dir = s.dir;
    store.parserNames = s.parser_names || [];
    store.activeParser = s.active_parser || 'standard';
    store.configPath = s.config_path;
    store.inputDir = s.input_dir;
    notify();
  } catch (e) {
    console.error('refreshState', e);
  }
  try {
    const filter = store.filesFilter === 'modified' ? '&filter=modified' : '';
    const files = await getJson<FileEntry[]>(
      `/api/files?dir=${encodeURIComponent(store.dir)}${filter}`,
    );
    store.files = files;
    notify();
    // The listing changed (initial load, reload button, live refresh, filter
    // switch) — its health badges come from a fresh check.
    scheduleCheck();
  } catch (e) {
    console.error('files', e);
  }
}

// ---- post-listing check → per-file diagnostics ----
//
// After every listing fetch the frontend runs ONE /api/check and groups the
// diagnostics by source file, so the rail can badge rows with error/warning
// counts. Debounced (trailing) so live-reload save storms collapse into a
// single check; while in flight (or after a failure) the map is empty and
// rows simply render without badges — never with stale numbers.

const CHECK_DEBOUNCE_MS = 500;

let checkTimer: ReturnType<typeof setTimeout> | null = null;
let checkRerunPending = false;

/** Schedule the post-listing check (debounced, trailing edge). */
export function scheduleCheck(): void {
  if (checkTimer !== null) clearTimeout(checkTimer);
  checkTimer = setTimeout(() => {
    checkTimer = null;
    void runCheck();
  }, CHECK_DEBOUNCE_MS);
}

/**
 * Run one /api/check and regroup the store's per-file diagnostics. While the
 * request is in flight the map is cleared (no stale badges); a failed or
 * non-200 check leaves it empty. A request arriving while one is in flight
 * re-runs once afterward, so the latest listing still gets fresh counts.
 */
export async function runCheck(): Promise<void> {
  if (store.checkRunning) {
    checkRerunPending = true;
    return;
  }
  store.checkRunning = true;
  store.diagnosticsByFile = new Map<string, FileDiagnostics>();
  notify();
  try {
    const { status, payload } = await postJson<CheckResponse>('/api/check', {
      dir: store.dir,
      parser: store.activeParser,
      plugin_paths: [],
    });
    store.diagnosticsByFile =
      status === 200 && Array.isArray(payload?.diagnostics)
        ? groupDiagnostics(payload.diagnostics)
        : new Map<string, FileDiagnostics>();
  } catch {
    store.diagnosticsByFile = new Map<string, FileDiagnostics>();
  } finally {
    store.checkRunning = false;
    notify();
    if (checkRerunPending) {
      checkRerunPending = false;
      void runCheck();
    }
  }
}

/**
 * Group check diagnostics by source file: `{ [file]: {errors, warnings} }`.
 * Keys are normalized to rel_path form (`/` separators, input-dir prefix
 * stripped) so they match FileEntry.rel_path exactly. Diagnostics without a
 * file location (e.g. the "no spreadsheets found" warning) are excluded —
 * they surface nowhere per-file.
 */
export function groupDiagnostics(diags: Diagnostic[]): Map<string, FileDiagnostics> {
  const byFile = new Map<string, FileDiagnostics>();
  for (const d of diags) {
    const file = d.location?.file;
    if (!file) continue;
    const key = diagKey(file);
    if (!key) continue;
    const cur = byFile.get(key) ?? { errors: 0, warnings: 0 };
    if (d.severity === 'Warning') cur.warnings += 1;
    else cur.errors += 1;
    byFile.set(key, cur);
  }
  return byFile;
}

/** Normalize a diagnostic file path to the rel_path form used by the rail. */
function diagKey(file: string): string {
  let p = file.replace(/\\/g, '/');
  // The server echoes the same input-dir-prefixed string the listing was
  // built from, so stripping store.inputDir recovers the rel_path.
  const input = (store.inputDir || '').replace(/\\/g, '/').replace(/\/+$/, '');
  if (input && p.startsWith(input)) {
    p = p.slice(input.length);
  }
  return p.replace(/^\/+/, '');
}

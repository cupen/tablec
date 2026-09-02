// Typed fetch helpers + the /api/state bootstrap.

import { notify, store } from './store.js';
import type { FileEntry, StateBody } from './store.js';

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
    const files = await getJson<FileEntry[]>(`/api/files?dir=${encodeURIComponent(store.dir)}`);
    store.files = files;
    notify();
  } catch (e) {
    console.error('files', e);
  }
}

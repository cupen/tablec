// Live file-change notifications: keep a WebSocket open to /ws and refresh
// the file list whenever the server says something changed.
//
// Semantics (spec: "WebSocket endpoint and client lifecycle"):
// - On a `files_changed` message → `refreshState()` (re-fetch server state
//   and the file list, honoring the active filter).
// - On unexpected close → reconnect with linear backoff (1s → 10s cap).
// - After a successful (re)connect → unconditional `refreshState()`, so any
//   change that happened while disconnected is picked up. This is a reconnect
//   catch-up, not a polling loop — there is no timer-based refresh.
// - If the socket never connects, the SPA still works via the Reload button.

import { refreshState } from './api.js';

/** Delay before the next reconnect attempt (capped). */
const BASE_RETRY_MS = 1000;
const MAX_RETRY_MS = 10_000;

let ws: WebSocket | null = null;
let closedByUs = false;
let retryMs = BASE_RETRY_MS;
let timer: ReturnType<typeof setTimeout> | null = null;

function scheduleReconnect() {
  if (timer !== null || closedByUs) return;
  timer = setTimeout(() => {
    timer = null;
    // Linear backoff with a cap: 1s → 2s → ... → 10s max.
    retryMs = Math.min(retryMs * 2, MAX_RETRY_MS);
    connect();
  }, retryMs);
}

function onMessage(ev: MessageEvent) {
  if (ev.data === 'files_changed') {
    void refreshState();
  }
}

function connect() {
  // The webui is served from the same origin; /ws is a same-host upgrade.
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${location.host}/ws`;
  try {
    ws = new WebSocket(url);
  } catch {
    scheduleReconnect();
    return;
  }

  ws.addEventListener('open', () => {
    retryMs = BASE_RETRY_MS;
    // Catch up on anything we missed while disconnected — and give the very
    // first load a fresh list even if no event fires (cheap, idempotent).
    void refreshState();
  });

  ws.addEventListener('message', onMessage);

  ws.addEventListener('close', () => {
    if (!closedByUs) scheduleReconnect();
  });

  ws.addEventListener('error', () => {
    // close will follow; just make sure we reconnect even if it doesn't.
    scheduleReconnect();
  });
}

/** Start the live-refresh socket. Call once at app boot. */
export function startLiveReload() {
  if (ws !== null) return; // already running
  closedByUs = false;
  connect();
}

/** Stop the live-refresh socket (e.g. on teardown in tests). */
export function stopLiveReload() {
  closedByUs = true;
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  if (ws !== null) {
    ws.close();
    ws = null;
  }
}
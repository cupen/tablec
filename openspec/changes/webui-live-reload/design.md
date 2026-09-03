## Context

The webui (axum 0.7, tokio) currently serves a static SPA and REST endpoints (`/api/files`, `/api/sheets`, `/api/preview`, `/api/parsed_preview`, `/api/build`, `/api/check`) with no push channel — the file list refreshes only on explicit reload (button / ⌘R) or filter toggle. We now add live notifications: watch the resolved input directory for file changes and push a "rescan" signal to connected browsers. The git-diff baseline feature (already archived) is orthogonal — watching must not depend on git, and must not trigger builds.

## Goals / Non-Goals

**Goals:**

- A WebSocket endpoint that delivers change notifications after a file change under the input directory.
- Frontend auto-refresh of the file list on notification, with reconnect + backoff and a reconnect-time full re-fetch — event-driven, no polling timer anywhere.
- Cross-platform watching (Linux inotify / Windows ReadDirectoryChangesW / macOS FSEvents or kqueue) via a single well-maintained crate.

**Non-Goals:**

- No polling fallback loop (server or client).
- No auto-build/auto-check/auto-parse on change.
- No cell-level or per-file delta push — the frontend always re-fetches the full list.
- No watching outside the resolved input directory.
- No multi-user sync or cross-device consistency.

## Decisions

### D1: Use the `notify` crate (stable 8.x) for cross-platform watching, not raw inotify or a polling loop
`notify` (notify-rs/notify) is the de-facto standard (deno, rust-analyzer, zed use it), with inotify on Linux, ReadDirectoryChangesW on Windows, and FSEvents/kqueue on macOS — one API for all targets the webui supports. Raw `inotify` would drop Windows support; a hand-rolled polling loop is cross-platform but adds a timer that contradicts the "no polling" requirement and wastes IO while idle. Version 9.0.0 is still rc and bumps MSRV; lock to 8.x stable.

### D2: Events are dirty markers; the source of truth is a rescan, and the push carries a "changed" signal
The watcher delivers raw `notify` events (create/modify/remove/rename, including editor rename-style saves). We do not forward raw events or attempt per-path dedup up front; instead any event marks the watch dirty, prompts a rescan of the supported spreadsheets (the same scan `/api/files` already does), and a rescan that observes a difference broadcasts "files changed". This absorbs notify's known lossiness (large dirs, transient events, editor atomic-save creating Delete/Create pairs) because final state always comes from a fresh dir scan, not from event accounting. Debounce consecutive events with a short quiet window (e.g. 300–500 ms) so a burst of events (a save that creates+replaces a temp file) collapses into one rescan.

### D3: WebSocket (`/ws`) + tokio broadcast, not SSE, not per-client polling
The user explicitly prefers WebSocket and no polling. A `tokio::sync::broadcast` channel is shared state: the watcher task publishes "files changed"; each `/ws` connection subscribes and forwards either the message or (on lag/reconnect) a signal that forces the client to re-fetch. axum's `ws` feature (tungstenite) is first-class and needs no extra crate. WebSocket also gives us a natural ping/pong keepalive and a clean reconnect handshake.

### D4: Frontend: connect once on boot, auto-reconnect with backoff, re-fetch on message and on reconnect
On load the SPA opens the WebSocket. On "files changed" it calls the existing `refreshState()` (which re-fetches server state + file list and already re-runs the active `filter=modified`). On unexpected close it reconnects with linear/exponential backoff; after a successful reconnect it calls `refreshState()` unconditionally to cover any changes missed while disconnected. The Reload button (⌘R) stays as manual fallback. No `setInterval` anywhere in the refresh path.

### D5: Watcher lifecycle ties to server startup; watches the resolved input directory
The watcher starts when the axum server starts, watching the same resolved input directory the file list uses. If the input directory does not exist at startup (e.g. `tablec.toml` points elsewhere or is missing), the watcher starts in a no-op state and the webui keeps working (manual reload still works); we do not crash the server. Configuration changes (e.g. `tablec.toml` re-pointing the input dir) are out of scope for v1 — watching follows the directory resolved at startup.

## Risks / Trade-offs

- [notify event loss or transient events (atomic saves, large dirs)] → Events are only dirty markers; a rescan is the source of truth; debounce collapses bursts; a missed event is caught by the next event, and a disconnect is caught by reconnect re-fetch. Degraded coverage means the UI may lag one change behind — never a wrong list.
- [Watch limit (inotify `max_user_watches`) or permission errors] → Watcher errors are logged and treated as dirty markers (rescan still runs); the server never fails because of a watcher issue. Manual reload remains.
- [WebSocket connection churn on flaky networks] → Backoff reconnect + unconditional re-fetch after reconnect; if the socket never connects, the SPA still works via reload.
- [Editor rename-style saves produce Delete+Create/Rename, not Modify] → Watching the directory (not individual files) + treating any event as dirty catches renames; debounce merges the pair into one refresh.
- [Broadcast lag for slow clients] → Use `try_recv`-style handling: a lagged subscriber skips to the latest "changed" signal (the message is idempotent — it only says "re-fetch").

## Migration Plan

Additive: `/ws` endpoint and frontend socket are new; existing REST endpoints and payloads are untouched; the file list's initial load path is unchanged. No data migration. Rollback: remove the watcher task and `/ws` route, revert the SPA socket code — old behavior (manual reload) is fully preserved by the additive design.

## Open Questions

- Debounce window and reconnect backoff constants (300–500 ms / e.g. 1s→10s cap) are implementation details; the spec only requires a refresh to happen, not the exact latency.
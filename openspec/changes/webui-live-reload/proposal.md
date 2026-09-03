## Why

The webui today only refreshes the file list on explicit user action: an initial load, a Reload button press (or ⌘R), or a filter toggle. When a user edits an Excel/CSV spreadsheet in an external editor while the webui is open, the preview stays stale until they manually reload. For a table compiler whose whole workflow is *edit → preview*, this manual step is friction. We want the webui to notice file changes and push a refresh to the browser automatically.

## What Changes

- **Backend file watching**: the webui watches the resolved input directory using the `notify` crate (cross-platform: inotify on Linux, ReadDirectoryChangesW on Windows, FSEvents/kqueue on macOS). Any create/modify/remove/rename event for a file under the directory is treated as a *dirty marker* that triggers a rescan of the supported spreadsheets.
- **WebSocket push**: a new `/ws` endpoint streams change notifications to connected browsers. The server sends a `files_changed` message after a scan detects a difference; no timer-based polling loop on server or client.
- **Frontend auto-refresh**: the webui opens a WebSocket to `/ws` on load, and on a `files_changed` message re-fetches the file list (`refreshState`). The existing Reload button stays as a manual fallback; the socket auto-reconnects with backoff after drops.
- **Documented scope**: watching applies to the resolved input directory only. No build/check re-run is triggered — this is a preview-refresh feature, not an auto-build feature. Outside a git repo the watcher still works (watching is unrelated to the git diff baseline).
- **BREAKING**: none. `/ws` is additive; all existing endpoints and payloads are unchanged.

## Capabilities

### New Capabilities

- (none — the behavior is folded into the existing `webui` capability)

### Modified Capabilities

- `webui`: The server SHALL watch the resolved input directory for changes and notify connected browsers over a WebSocket endpoint, so the file list refreshes automatically without manual reload.

## Impact

- **Code**: `tablec-webui/` — a new watcher module (notify → rescan → broadcast), an axum WebSocket route (`/ws`) with a tokio broadcast channel, and frontend socket handling in the webui SPA (connect, auto-reconnect, refresh on message). `state.rs` gains the broadcast state.
- **New dependency**: `notify` (stable 8.x) and activation of axum's `ws` feature (tungstenite). No other new crates.
- **Docs**: `README.md` webui section and `docs/design.md` note the auto-refresh and the new `/ws` endpoint; the `webui` main spec is updated.
- **CI / test surface**: unit tests for the watcher→broadcast wiring and handler tests for the scan-diff logic; a WebSocket e2e test against a temp dir (or a handler-level test that asserts a broadcast fires after a watch-triggered rescan).

## Non-Goals

- No auto-build/auto-check on change; no push of cell-level diff deltas; no cross-device/multi-user sync; no handling of files outside the resolved input directory.
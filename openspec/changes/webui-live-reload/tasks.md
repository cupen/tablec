## 1. Backend — watcher plumbing

- [x] 1.1 Add `notify` (stable 8.x) to `tablec-webui/Cargo.toml` and enable axum's `ws` feature; verify `cargo build -p tablec-webui` compiles
- [x] 1.2 Add a `watcher` module: a task that watches the resolved input directory via `notify::recommended_watcher`, debounces events (300–500 ms quiet window), and treats any event as a dirty marker; verify by unit test that a write to a file under the dir produces a dirty signal
- [x] 1.3 Wire the watcher to a `tokio::sync::broadcast` channel so a dirty→rescan→difference cycle publishes a `files_changed` message; verify by unit test that a touch of a watched file leads to a publish (or that the rescan-diff logic signals a change)
- [x] 1.4 Add graceful degradation: watcher start failures (missing dir, permissions, watch limits) are logged and leave the server fully functional with manual reload; verify with a test that a nonexistent input dir does not crash startup

## 2. Backend — WebSocket endpoint

- [x] 2.1 Add `GET /ws` (WebSocket) route that subscribes to the broadcast channel and forwards `files_changed` messages to the client; run `cargo test -p tablec-webui` and confirm existing tests still pass
- [x] 2.2 Handle broadcast lag gracefully: a lagged subscriber gets the latest "changed" signal rather than a disconnect; verify with a handler/unit test
- [x] 2.3 Add handler-level or integration test proving a connected WebSocket client receives a `files_changed` message after a change in a temp watched dir (skip when the environment cannot create sockets, if needed); run `cargo test -p tablec-webui`

## 3. Frontend — auto-refresh via WebSocket

- [x] 3.1 In the SPA, open a WebSocket to `/ws` on boot; on a `files_changed` message call the existing `refreshState()`; verify by typing (`pnpm check`) and a DOM/unit-style check that a message triggers a refresh call
- [x] 3.2 Add auto-reconnect with backoff (e.g. 1s → 10s cap) on unexpected close, and an unconditional `refreshState()` (full file-list re-fetch) after a successful reconnect; verify by typing and a logic test of the reconnect/refresh sequence
- [x] 3.3 Keep the Reload button (⌘R) working as manual fallback independent of the socket; verify no `setInterval`-based polling was introduced (`grep` check + typing)
- [x] 3.4 Run `pnpm check && pnpm build` in `tablec-webui/webui/` and confirm the compiled bundle contains the WebSocket code

## 4. Integration + docs + quality gates

- [x] 4.1 Integration: run the webui against a temp dir, modify a spreadsheet from another shell, and confirm the file list refreshes without a manual reload (browser-based or a handler-level equivalent); verify the Reconnect path by stopping/starting the server
- [x] 4.2 Update `openspec/specs/webui/spec.md` (already delta'd; synced in a follow-up), `docs/design.md`, and the webui README snippet to mention the `/ws` endpoint, auto-refresh, and manual-reload fallback
- [x] 4.3 Run full gates: `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy -p tablec-webui`, and `cd tablec-webui/webui && pnpm check && pnpm build`; confirm all green
- [x] 4.4 File issues for any deferred items via `bd` (e.g. config-change re-watch, cross-branch diff preview is separate); close or claim the implementation issue when done
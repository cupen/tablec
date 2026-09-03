//! Live file-change watching for the webui.
//!
//! Watches the resolved input directory with the [`notify`] crate (inotify on
//! Linux, ReadDirectoryChangesW on Windows, FSEvents/kqueue on macOS). Events
//! are treated as *dirty markers* only — the source of truth is a rescan of
//! the directory, never event accounting. A quiet window debounces bursts
//! (editor atomic saves, temp-file rename pairs) so a burst collapses into a
//! single "files changed" broadcast on [`WatcherState::tx`].
//!
//! Channel topology (two channels deliberately — forwarding the broadcast
//! straight from the notify callback would feed the debouncer's own output
//! back into its input, looping forever):
//!
//! ```text
//!   notify callback ──raw──▶ (raw channel) ──▶ debounce worker ──▶ (out_tx)
//!                                                                    │
//!                                                         WatcherState::tx
//! ```
//!
//! The worker is a plain `std::thread` (no tokio dependency), owning the
//! notify watcher for its whole lifetime. Dropping the [`WatcherState`]
//! signals the worker to exit by closing the shutdown channel, so tests don't
//! leak threads and the server shuts down cleanly.
//!
//! The watcher is deliberately lossy-tolerant: a missed event is caught by the
//! next one, and a disconnected client catches up by re-fetching on reconnect.
//! Watcher failures (missing dir, permissions, inotify watch limits) degrade to
//! a logged no-op — the webui keeps working with manual reload.

use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// Debounce quiet window: bursts of events (a save that creates and replaces a
/// temp file) collapse into one rescan/broadcast.
const DEBOUNCE: Duration = Duration::from_millis(400);

/// How often the worker wakes to check for shutdown while idle.
const SHUTDOWN_POLL: Duration = Duration::from_millis(200);

/// Broadcast capacity. The message is idempotent ("re-fetch"), so lagged
/// subscribers just skip to the latest value — losing a notification is fine
/// because the client re-fetches the whole list anyway.
const BROADCAST_CAPACITY: usize = 16;

/// Shared handle to the watcher broadcast channel, stored in [`WebuiState`] so
/// `/ws` handlers can subscribe.
pub struct WatcherState {
    /// True when a watcher is actually running for the input directory.
    pub active: bool,
    /// Publish "files changed" here; clients subscribe and re-fetch.
    pub tx: broadcast::Sender<()>,
    /// Dropping this closes the shutdown channel and stops the worker thread.
    _shutdown: Option<std::sync::mpsc::Sender<()>>,
}

impl WatcherState {
    /// A detached handle that never fires — used before the input dir is
    /// resolved (the webui keeps working with manual reload).
    pub fn inactive() -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        // Keep `_rx` alive: a broadcast sender's `send` errors when there are
        // no receivers, and `inactive` is not supposed to look broken.
        Self {
            active: false,
            tx,
            _shutdown: None,
        }
    }

    /// Start watching `input_dir` (if it exists) and return the broadcast
    /// handle. When the directory is missing or the watcher fails to start,
    /// returns an inactive state — the webui stays fully functional.
    ///
    /// The watch is non-recursive: it covers files directly in `input_dir`,
    /// which is exactly the set [`crate::handlers::api_files`] lists. Watching
    /// recursively would descend into `.git` and other noisy subdirectories,
    /// spamming refreshes and burning inotify watch slots.
    pub fn start(input_dir: &Path) -> Self {
        let (out_tx, _out_rx) = broadcast::channel(BROADCAST_CAPACITY);
        let mut state = Self {
            active: false,
            tx: out_tx.clone(),
            _shutdown: None,
        };

        if !input_dir.is_dir() {
            eprintln!(
                "webui watcher: input directory {} not found; live refresh disabled",
                input_dir.display()
            );
            return state;
        }

        // Raw events: notify callback -> worker. The callback runs on notify's
        // own thread; std mpsc is enough for a "something changed" ping.
        let (raw_tx, raw_rx) = std::sync::mpsc::channel::<()>();
        // Shutdown: when `state` is dropped, this sender closes and the
        // worker observes Disconnected and exits.
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
        state._shutdown = Some(shutdown_tx);

        let raw_for_callback = raw_tx.clone();
        match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                // Dirty marker: we don't care WHICH file changed — the next
                // rescan re-lists the directory and compares. Both familiar
                // save styles (in-place write, temp-file+rename) map to some
                // event here because we watch the directory, not individual
                // files. A channel lag means events coalesce, which is fine.
                let _ = raw_for_callback.send(());
            } else if let Err(e) = res {
                // Permission/limits/transient errors: keep watching; the next
                // good event still triggers a rescan. Never fail the server.
                eprintln!("webui watcher event error: {e}");
            }
        }) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(input_dir, RecursiveMode::NonRecursive) {
                    eprintln!(
                        "webui watcher: failed to watch {}: {e}; live refresh disabled",
                        input_dir.display()
                    );
                    return state;
                }
                state.active = true;
                let out_for_worker = out_tx.clone();
                std::thread::spawn(move || {
                    // Owning the notify watcher keeps it delivering events for
                    // the whole life of the worker.
                    let _watcher = watcher;
                    loop {
                        match raw_rx.recv_timeout(SHUTDOWN_POLL) {
                            Ok(()) => {}
                            Err(RecvTimeoutError::Timeout) => {
                                if is_shutdown(&shutdown_rx) {
                                    break;
                                }
                                continue;
                            }
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                        // Quiet window: coalesce a burst into one broadcast.
                        std::thread::sleep(DEBOUNCE);
                        while raw_rx.try_recv().is_ok() {}
                        if is_shutdown(&shutdown_rx) {
                            break;
                        }
                        // "Files changed" — idempotent: clients re-fetch all.
                        let _ = out_for_worker.send(());
                    }
                });
            }
            Err(e) => {
                eprintln!(
                    "webui watcher: failed to create watcher for {}: {e}; live refresh disabled",
                    input_dir.display()
                );
            }
        }

        state
    }
}

/// True when the worker should stop: a shutdown message was sent, or every
/// sender (i.e. the [`WatcherState`] that owned it) has been dropped.
fn is_shutdown(rx: &std::sync::mpsc::Receiver<()>) -> bool {
    matches!(rx.try_recv(), Ok(()) | Err(TryRecvError::Disconnected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_on_missing_dir_is_inactive_not_error() {
        let state = WatcherState::start(Path::new("/no/such/dir/tablec-watcher-test"));
        assert!(!state.active, "missing dir must not crash startup");
        // The channel exists; sending is allowed even with zero subscribers
        // (n is the number received, 0 here).
        let _ = state.tx.send(());
    }

    #[test]
    fn start_on_existing_dir_activates() {
        let tmp = tempfile::tempdir().unwrap();
        let state = WatcherState::start(tmp.path());
        assert!(state.active, "watcher should start on an existing dir");
        drop(state); // signals shutdown; worker thread exits
        std::thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn dirty_event_leads_to_files_changed_broadcast() {
        // Integration of the real notify watcher + debounced worker: touching
        // a file inside the watched dir must eventually produce a single
        // "files changed" message on the broadcast.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.xlsx"), b"x").unwrap();
        let state = WatcherState::start(tmp.path());
        assert!(state.active);

        let mut rx = state.tx.subscribe();
        std::fs::write(tmp.path().join("a.xlsx"), b"y").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let got = loop {
            match rx.try_recv() {
                Ok(()) => break true,
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
                    if std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break false,
            }
        };
        assert!(got, "expected a files_changed broadcast after a write");
        drop(state);
        std::thread::sleep(Duration::from_millis(50));
    }
}

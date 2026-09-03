use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;

use crate::WebuiState;
use crate::router;

/// Launch the local webui: an HTTP server that serves a single-page
/// Web Components UI for previewing, building and checking table files.
#[derive(Parser, Debug)]
#[command(
    name = "webui",
    about = "Launch local webui for previewing, building and checking tables"
)]
pub struct WebuiCommand {
    /// Directory to preview/build. Defaults to the current working directory.
    #[arg(long, short = 'd')]
    pub dir: Option<PathBuf>,

    /// Host/interface to bind on. Loopback by default — do NOT expose publicly.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// TCP port. Default `9527`.
    #[arg(long, short = 'p', default_value_t = 9527)]
    pub port: u16,

    /// Skip the auto-open browser step (useful for CI / remote boxes).
    #[arg(long)]
    pub no_browser: bool,

    /// Path to a `tablec.toml` file (overrides auto-discovery).
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Schema parser name (e.g. `standard`).
    #[arg(long)]
    pub parser: Option<String>,

    /// Plugin `.so`/`.dylib` path. May be repeated.
    #[arg(long = "plugin-path")]
    pub plugin_path: Vec<PathBuf>,
}

impl WebuiCommand {
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Resolve the working directory (used as default when handlers don't override).
        let dir = self
            .dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let dir = dir.canonicalize().unwrap_or(dir);

        // Build parser registry. Plugin paths come only from CLI flags — never from HTTP input.
        let registry = router::build_registry(&self.plugin_path)?;
        let parser_name = self.parser.clone();
        let config_override = self.config.clone();

        let state = Arc::new(WebuiState::new(
            dir.clone(),
            registry,
            parser_name,
            config_override,
        ));

        // Start the live file watcher on the resolved input directory. This
        // resolves the config (read from disk, synchronously) and begins
        // watching; failures degrade to manual reload, never crash the server.
        state.start_watcher(&dir);

        let app = router::router(state.clone());

        let host = self.host.clone();
        let port = self.port;
        let no_browser = self.no_browser;

        // Tokio runtime — main is sync; we drive axum from a single-threaded executor.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        rt.block_on(async move {
            let addr: std::net::SocketAddr = format!("{host}:{port}")
                .parse()
                .map_err(|e| format!("invalid bind address '{host}:{port}': {e}"))?;

            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|e| format!("failed to bind {host}:{port}: {e}"))?;
            let bound = listener.local_addr().unwrap_or(addr);
            let _bound_port = bound.port();

            eprintln!("tablec webui listening on http://{}/", bound);
            eprintln!("serving directory: {}", state.dir.display());

            let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());

            // Browser launch happens after the server is actually bound, so the
            // URL we hand the OS is real (not the wildcard :0 we asked for).
            if !no_browser {
                let url = format!("http://{}/", bound);
                if let Err(e) = webbrowser::open(&url) {
                    eprintln!(
                        "warning: failed to launch browser for {url}: {e}. \
                         Open it manually."
                    );
                }
            }

            if let Err(e) = server.await {
                eprintln!("webui server error: {e}");
            }

            Ok::<(), Box<dyn std::error::Error>>(())
        })
    }
}

/// Wait for SIGINT (Ctrl-C) — used as the graceful-shutdown trigger.
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webui_command_parses_minimal() {
        // clap derive smoke test: the long flags must parse even when only a
        // bare `webui` is given (everything is optional / defaulted).
        let w = WebuiCommand::parse_from(["webui"]);
        assert!(w.dir.is_none());
        assert_eq!(w.host, "127.0.0.1");
        assert_eq!(w.port, 9527);
        assert!(!w.no_browser);
        assert!(w.config.is_none());
        assert!(w.parser.is_none());
        assert!(w.plugin_path.is_empty());
    }

    #[test]
    fn webui_command_parses_short_port_flag() {
        let w = WebuiCommand::parse_from(["webui", "-p", "8080"]);
        assert_eq!(w.port, 8080);
    }

    #[test]
    fn webui_command_parses_full() {
        let w = WebuiCommand::parse_from([
            "webui",
            "--dir",
            "./data",
            "--host",
            "0.0.0.0",
            "--port",
            "8080",
            "--no-browser",
            "--config",
            "./tablec.toml",
            "--parser",
            "standard",
            "--plugin-path",
            "/tmp/libfoo.so",
        ]);
        assert_eq!(w.dir, Some(PathBuf::from("./data")));
        assert_eq!(w.host, "0.0.0.0");
        assert_eq!(w.port, 8080);
        assert!(w.no_browser);
        assert_eq!(w.config, Some(PathBuf::from("./tablec.toml")));
        assert_eq!(w.parser, Some("standard".to_string()));
        assert_eq!(w.plugin_path, vec![PathBuf::from("/tmp/libfoo.so")]);
    }
}

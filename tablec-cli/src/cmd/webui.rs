use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;

use crate::web::{self, WebuiState};

/// Launch the local webui: an HTTP server that serves a single-page
/// Web Components UI for previewing, building and checking table files.
#[derive(Args, Debug)]
pub struct WebuiCommand {
    /// TCP port (positional). `0` lets the OS pick a free port.
    /// When both a positional port and `--port` are given, `--port` wins.
    #[arg(value_name = "PORT")]
    pub port_pos: Option<u16>,

    /// Directory to preview/build. Defaults to the current working directory.
    #[arg(long, short = 'd')]
    pub dir: Option<PathBuf>,

    /// Host/interface to bind on. Loopback by default — do NOT expose publicly.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// TCP port. `0` lets the OS pick a free port. Wins over the positional
    /// port if both are given.
    #[arg(long, default_value_t = 0)]
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
    /// Resolve the final port. `--port` wins; if both are `0`/absent,
    /// returns `0` (OS-assigned).
    pub fn resolved_port(&self) -> u16 {
        if self.port != 0 {
            return self.port;
        }
        self.port_pos.unwrap_or(0)
    }
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
        let registry = web::build_registry(&self.plugin_path)?;
        let parser_name = self.parser.clone();
        let config_override = self.config.clone();

        let state = Arc::new(WebuiState::new(
            dir.clone(),
            registry,
            parser_name,
            config_override,
        ));

        let app = web::router(state.clone());

        let host = self.host.clone();
        let port = self.resolved_port();
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
        use clap::Parser;
        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: crate::cli::Command,
        }
        let parsed = Wrap::parse_from(["tablec", "webui"]).cmd;
        match parsed {
            crate::cli::Command::Webui(w) => {
                assert!(w.dir.is_none());
                assert_eq!(w.host, "127.0.0.1");
                assert_eq!(w.port, 0);
                assert!(w.port_pos.is_none());
                assert_eq!(w.resolved_port(), 0);
                assert!(!w.no_browser);
                assert!(w.config.is_none());
                assert!(w.parser.is_none());
                assert!(w.plugin_path.is_empty());
            }
            _ => panic!("expected Webui variant"),
        }
    }

    #[test]
    fn webui_command_parses_positional_port() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: crate::cli::Command,
        }
        let parsed = Wrap::parse_from(["tablec", "webui", "8765"]).cmd;
        match parsed {
            crate::cli::Command::Webui(w) => {
                assert_eq!(w.port_pos, Some(8765));
                assert_eq!(w.port, 0);
                assert_eq!(w.resolved_port(), 8765);
            }
            _ => panic!("expected Webui variant"),
        }
    }

    #[test]
    fn webui_command_flag_overrides_positional_port() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: crate::cli::Command,
        }
        // Both positional and --port: --port wins.
        let parsed = Wrap::parse_from(["tablec", "webui", "1111", "--port", "2222"]).cmd;
        match parsed {
            crate::cli::Command::Webui(w) => {
                assert_eq!(w.port_pos, Some(1111));
                assert_eq!(w.port, 2222);
                assert_eq!(w.resolved_port(), 2222);
            }
            _ => panic!("expected Webui variant"),
        }
    }

    #[test]
    fn webui_command_parses_full() {
        use clap::Parser;
        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: crate::cli::Command,
        }
        let parsed = Wrap::parse_from([
            "tablec",
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
        ])
        .cmd;
        match parsed {
            crate::cli::Command::Webui(w) => {
                assert_eq!(w.dir, Some(PathBuf::from("./data")));
                assert_eq!(w.host, "0.0.0.0");
                assert_eq!(w.port, 8080);
                assert!(w.no_browser);
                assert_eq!(w.config, Some(PathBuf::from("./tablec.toml")));
                assert_eq!(w.parser, Some("standard".to_string()));
                assert_eq!(w.plugin_path, vec![PathBuf::from("/tmp/libfoo.so")]);
            }
            _ => panic!("expected Webui variant"),
        }
    }
}

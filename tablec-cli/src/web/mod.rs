//! webui backend: shared state, HTTP handlers, and calamine-based excel reader.
//!
//! The actual `WebuiCommand` lives in [`crate::cmd::webui`]. This module is the
//! library half of the split — it only depends on `axum` + `tablec-core`, so
//! unit tests can drive the router via `tower::ServiceExt::oneshot` without
//! spawning a server.

pub mod excel;
pub mod handlers;
pub mod state;

pub use state::WebuiState;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

/// Build a [`SchemaParserRegistry`] from `plugin_paths`. Returns a
/// `DynamicPluginError`-flavored [`String`] error so callers don't have to
/// import the inner error type.
pub fn build_registry(
    plugin_paths: &[PathBuf],
) -> Result<Arc<tablec_core::core::schema::SchemaParserRegistry>, String> {
    let reg =
        tablec_core::core::schema::SchemaParserRegistry::with_standard_and_plugins(plugin_paths)
            .map_err(|e| format!("plugin load failed: {e}"))?;
    Ok(Arc::new(reg))
}

/// Construct the axum [`Router`] for the webui.
///
/// All endpoints live under `/api/*`. Static assets (HTML, CSS, JS) are
/// served at the root paths `/`, `/static/app.js`, `/static/style.css`.
pub fn router(state: Arc<WebuiState>) -> Router {
    Router::new()
        .route("/", get(handlers::index_html))
        .route("/static/app.js", get(handlers::app_js))
        .route("/static/style.css", get(handlers::style_css))
        .route("/static/vendor/lit.js", get(handlers::vendor_lit))
        .route("/api/health", get(handlers::api_health))
        .route("/api/state", get(handlers::api_state))
        .route("/api/files", get(handlers::api_files))
        .route("/api/sheets", get(handlers::api_sheets))
        .route("/api/preview", get(handlers::api_preview))
        .route("/api/build", post(handlers::api_build))
        .route("/api/check", post(handlers::api_check))
        .route("/api/validate", post(handlers::api_validate))
        .with_state(state)
}

//! `tablec-webui` — HTTP backend + clap subcommand for `tablec webui`,
//! extracted from `tablec-cli` so the core CLI doesn't pay the axum/tokio
//! build cost when the user only runs `build`/`check`/`example`.
//!
//! Public API:
//!   - [`WebuiState`] — shared axum state
//!   - [`router`]     — `axum::Router` builder
//!   - [`command::WebuiCommand`] — clap Parser for `tablec webui ...`
//!   - [`excel::{list_sheets, preview_sheet}`] — calamine helpers
//!   - [`handlers`] — axum handler fns + request/response types

pub mod command;
pub mod excel;
pub mod git;
pub mod handlers;
pub mod router;
pub mod state;

pub use state::WebuiState;

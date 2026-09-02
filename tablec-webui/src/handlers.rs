//! axum HTTP handlers for the webui.
//!
//! All endpoints accept `Arc<WebuiState>` via axum's `State` extractor and
//! return either `Json<T>` or [`ApiError`]. Static assets are served from
//! `tablec-webui/webui/` via `include_str!` so the binary stays self-contained.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use serde_json::json;

use tablec_core::core::config::Config;
use tablec_core::core::diagnostic::Diagnostic;
use tablec_core::core::project::project::Project;
use tablec_core::core::table::constraint::ConstraintValidator;
use tablec_core::export::{Format, Json as JsonFmt, Msgpack};

use crate::excel::{self, Grid, SheetInfo};
use crate::state::WebuiState;

// -----------------------------------------------------------------------------
// Static assets — the Vite build output, embedded at compile time.
//
// The frontend lives in `webui/` as a pnpm + Vite + TypeScript project.
// `pnpm build` (run in `webui/`) emits `webui/dist/`, which is embedded here
// so the binary stays self-contained — `cargo build` works without node.
// Regenerate dist after frontend changes, then rebuild the crate.
// -----------------------------------------------------------------------------

pub static WEBUI_DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/webui/dist");

pub async fn index_html() -> Response {
    static_asset(axum::extract::Path("index.html".to_string())).await
}

/// Serve an embedded dist file by path. Extensionless paths fall back to
/// index.html (SPA semantics); unknown paths with an extension are a 404.
/// Unknown `/api/*` paths always 404 — the catch-all route must never answer
/// for the API namespace.
pub async fn static_asset(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    let path = path.trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        let mut resp = (
            StatusCode::NOT_FOUND,
            format!("no such API route: /{path}"),
        )
            .into_response();
        no_cache(&mut resp);
        return resp;
    }

    let (file, served_path) = match WEBUI_DIST.get_file(path) {
        Some(f) => (f, path),
        None if !path.contains('.') => match WEBUI_DIST.get_file("index.html") {
            Some(f) => (f, "index.html"),
            None => {
                let mut resp = (
                    StatusCode::NOT_FOUND,
                    format!("asset not found: {path}"),
                )
                    .into_response();
                no_cache(&mut resp);
                return resp;
            }
        },
        _ => {
            let mut resp = (
                StatusCode::NOT_FOUND,
                format!("asset not found: {path}"),
            )
                .into_response();
            no_cache(&mut resp);
            return resp;
        }
    };

    let mime = match served_path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    };
    let mut resp = (StatusCode::OK, file.contents().to_vec()).into_response();
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    no_cache(&mut resp);
    resp
}

/// Stamp a `Cache-Control: no-cache` header so dev iteration doesn't get
/// masked by the browser's heuristic caching.
fn no_cache(resp: &mut Response) {
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
}

// -----------------------------------------------------------------------------
// ApiError — uniform error rendering.
// -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            body: json!({
                "error": code,
                "message": message.into(),
            }),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }

    pub fn not_implemented(todo: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            body: json!({
                "error": "not_implemented",
                "message": "数据校验功能仍在研究中",
                "todo": todo.into(),
            }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let mut resp = (status, Json(self.body)).into_response();
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        resp
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(format!("io: {e}"))
    }
}

impl From<excel::ExcelError> for ApiError {
    fn from(e: excel::ExcelError) -> Self {
        match e {
            excel::ExcelError::SheetNotFound { .. } => Self::not_found(e.to_string()),
            _ => Self::internal(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::bad_request(format!("json: {e}"))
    }
}

// -----------------------------------------------------------------------------
// GET /api/health
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct Health {
    ok: bool,
    version: &'static str,
    uptime_secs: u64,
}

pub async fn api_health(State(state): State<Arc<WebuiState>>) -> Json<Health> {
    let uptime = state.started_at.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: uptime,
    })
}

// -----------------------------------------------------------------------------
// GET /api/state
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StateBody {
    pub dir: String,
    pub host: String,
    pub port_obs: u16,
    pub parser_names: Vec<String>,
    pub active_parser: Option<String>,
    pub config_path: Option<String>,
    pub config_present: bool,
    /// Absolute path spreadsheets are scanned in: `<dir>/<input_dir>` from
    /// the resolved config. Surfaced so the UI can explain empty file lists.
    pub input_dir: String,
}

pub async fn api_state(State(state): State<Arc<WebuiState>>) -> Json<StateBody> {
    let active_parser = state
        .parser_override
        .clone()
        .or_else(|| Some("standard".to_string()));

    let cfg = resolve_config(&state, &state.dir).await;
    let config_present = cfg.1.is_some();

    Json(StateBody {
        dir: state.dir.display().to_string(),
        host: "127.0.0.1".to_string(),
        port_obs: 0,
        parser_names: state.registry.parser_names(),
        active_parser,
        config_path: cfg.1.map(|p| p.display().to_string()),
        config_present,
        input_dir: resolve_input_dir(&state.dir, &cfg.0.data.input_dir)
            .display()
            .to_string(),
    })
}

/// Join `dir` with the configured `input_dir`, treating `"."` as "the
/// directory itself" so paths don't grow a trailing `/.`.
fn resolve_input_dir(dir: &Path, input_dir: &str) -> PathBuf {
    if input_dir == "." {
        dir.to_path_buf()
    } else {
        dir.join(input_dir)
    }
}

async fn resolve_config(state: &WebuiState, dir: &Path) -> (Config, Option<PathBuf>) {
    let mut guard = state.config_cache.lock().await;
    if guard.is_none() {
        let (cfg, from) = state.load_config_for(dir).await;
        *guard = Some(cfg.clone());
        return (cfg, from);
    }
    (guard.as_ref().unwrap().clone(), None)
}

// -----------------------------------------------------------------------------
// GET /api/files?dir=...
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct FilesQuery {
    #[serde(default)]
    pub dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_secs: i64,
}

pub async fn api_files(
    State(state): State<Arc<WebuiState>>,
    Query(q): Query<FilesQuery>,
) -> Result<Json<Vec<FileEntry>>, ApiError> {
    let dir = q
        .dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| state.dir.clone());
    if !dir.is_dir() {
        return Err(ApiError::not_found(format!(
            "directory not found: {}",
            dir.display()
        )));
    }

    // List files under the same directory build/check read from, so the
    // preview list always matches what the actions operate on.
    let (config, _) = resolve_config(&state, &dir).await;
    let input_dir = resolve_input_dir(&dir, &config.data.input_dir);

    let mut entries = Vec::new();
    if !input_dir.is_dir() {
        return Ok(Json(entries));
    }
    let read = std::fs::read_dir(&input_dir)?;
    for entry in read.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        let recognized = matches!(
            ext.as_deref(),
            Some("xlsx") | Some("xls") | Some("xlsb") | Some("ods")
        );
        if !recognized {
            continue;
        }
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_secs = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        entries.push(FileEntry {
            name: p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            path: p.display().to_string(),
            size,
            modified_secs,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(entries))
}

// -----------------------------------------------------------------------------
// GET /api/sheets?path=...
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SheetsQuery {
    pub path: String,
}

pub async fn api_sheets(Query(q): Query<SheetsQuery>) -> Result<Json<Vec<SheetInfo>>, ApiError> {
    let p = PathBuf::from(&q.path);
    if !p.exists() {
        return Err(ApiError::not_found(format!(
            "file not found: {}",
            p.display()
        )));
    }
    let sheets = excel::list_sheets(&p)?;
    Ok(Json(sheets))
}

// -----------------------------------------------------------------------------
// GET /api/preview?path=...&sheet=...&max_rows=100
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PreviewQuery {
    pub path: String,
    pub sheet: String,
    #[serde(default)]
    pub max_rows: Option<usize>,
}

pub async fn api_preview(Query(q): Query<PreviewQuery>) -> Result<Json<Grid>, ApiError> {
    let p = PathBuf::from(&q.path);
    if !p.exists() {
        return Err(ApiError::not_found(format!(
            "file not found: {}",
            p.display()
        )));
    }
    let max = q.max_rows.unwrap_or(100).clamp(5, 1000);
    let grid = excel::preview_sheet(&p, &q.sheet, max)?;
    Ok(Json(grid))
}

// -----------------------------------------------------------------------------
// GET /api/parsed_preview?path=...&sheet=...&parser=standard&max_rows=120
//
// Default view of the preview pane: runs the schema parser + per-cell type
// check, so the UI shows the file *as tablec will see it during build*, not
// as raw bytes. /api/preview remains for the "view raw" toggle.
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ParsedPreviewQuery {
    pub path: String,
    pub sheet: String,
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub max_rows: Option<usize>,
}

pub async fn api_parsed_preview(
    State(state): State<Arc<WebuiState>>,
    Query(q): Query<ParsedPreviewQuery>,
) -> Result<Json<excel::ParsedPreview>, ApiError> {
    let p = PathBuf::from(&q.path);
    if !p.exists() {
        return Err(ApiError::not_found(format!(
            "file not found: {}",
            p.display()
        )));
    }
    let parser_name = q.parser.clone().unwrap_or_else(|| "standard".to_string());
    let max = q.max_rows.unwrap_or(120).clamp(1, 1000);
    match excel::parsed_preview(&p, &q.sheet, &parser_name, &state.registry, max) {
        Ok(pp) => Ok(Json(pp)),
        Err(excel::ParsedPreviewError::UnknownParser { name, available }) => Err(
            ApiError::bad_request(format!("unknown parser '{name}'; available: {available:?}")),
        ),
        Err(excel::ParsedPreviewError::Excel(e)) => Err(ApiError::from(e)),
    }
}

// -----------------------------------------------------------------------------
// POST /api/build
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BuildRequest {
    pub dir: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub pretty: bool,
    #[serde(default)]
    pub include_fields: bool,
    #[serde(default)]
    pub write: bool,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub plugin_paths: Vec<String>,
}

fn default_format() -> String {
    "json".to_string()
}

#[derive(Serialize)]
pub struct BuildResponse {
    pub format: String,
    pub bytes: Option<usize>,
    pub preview_first_500: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub output_path: Option<String>,
    pub duration_ms: u128,
    pub written: bool,
}

pub async fn api_build(
    State(state): State<Arc<WebuiState>>,
    Json(req): Json<BuildRequest>,
) -> Result<Json<BuildResponse>, ApiError> {
    if req.format != "json" && req.format != "json-pretty" && req.format != "msgpack" {
        return Err(ApiError::bad_request(format!(
            "unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
            req.format
        )));
    }
    let dir = PathBuf::from(&req.dir);
    if !dir.is_dir() {
        return Err(ApiError::not_found(format!(
            "directory not found: {}",
            dir.display()
        )));
    }

    // Resolve parser: CLI override > request override > first available.
    let parser_name = req
        .parser
        .clone()
        .or_else(|| state.parser_override.clone())
        .unwrap_or_else(|| "standard".to_string());
    let parser = state
        .registry
        .get(&parser_name)
        .ok_or_else(|| ApiError::bad_request(format!("unknown parser '{parser_name}'")))?;

    // Plugin paths from HTTP input are NEVER honored — security boundary.
    if !req.plugin_paths.is_empty() {
        return Err(ApiError::bad_request(
            "plugin_paths from HTTP requests are not accepted (CLI flag only)",
        ));
    }

    let (config, _config_from) = resolve_config(&state, &dir).await;
    let input_path = resolve_input_dir(&dir, &config.data.input_dir);
    if !input_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "input directory not found: {} (input_dir = {:?} from {}); put spreadsheets there or set [data] input_dir in tablec.toml",
            input_path.display(),
            config.data.input_dir,
            _config_from
                .as_deref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "built-in defaults".to_string()),
        )));
    }

    let started = Instant::now();
    let (project, diagnostics) = match build_project(&dir, &config, &input_path, parser.as_ref()) {
        Ok((p, diags)) => (p, diags),
        Err((diags, _)) => {
            // `diags` is what `read_excel_with` reported.
            return Ok(Json(BuildResponse {
                format: req.format.clone(),
                bytes: None,
                preview_first_500: None,
                diagnostics: diags,
                output_path: None,
                duration_ms: started.elapsed().as_millis(),
                written: false,
            }));
        }
    };

    let bytes: Vec<u8> = match req.format.as_str() {
        "json" => JsonFmt {
            pretty: false,
            include_fields: req.include_fields,
        }
        .to_vec(&project)
        .map_err(|e| ApiError::internal(format!("json encode: {e}")))?,
        "json-pretty" => JsonFmt {
            pretty: true,
            include_fields: req.include_fields,
        }
        .to_vec(&project)
        .map_err(|e| ApiError::internal(format!("json encode: {e}")))?,
        "msgpack" => Msgpack
            .to_vec(&project)
            .map_err(|e| ApiError::internal(format!("msgpack encode: {e}")))?,
        _ => unreachable!("format guarded above"),
    };

    let output_path = req.output_path.clone().or_else(|| {
        Some(
            dir.join(&config.export.output_dir)
                .join(format!("{}.{}", config.project.name, ext_for(&req.format)))
                .display()
                .to_string(),
        )
    });

    let mut written = false;
    if req.write {
        if let Some(p) = &output_path {
            let pb = PathBuf::from(p);
            if let Some(parent) = pb.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&pb, &bytes)?;
            written = true;
        }
    }

    let preview_first_500 = if req.format.starts_with("json") {
        let s = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]).to_string();
        Some(if bytes.len() > 500 {
            format!("{s}…")
        } else {
            s
        })
    } else {
        None
    };

    Ok(Json(BuildResponse {
        format: req.format.clone(),
        bytes: Some(bytes.len()),
        preview_first_500,
        diagnostics,
        output_path,
        duration_ms: started.elapsed().as_millis(),
        written,
    }))
}

fn ext_for(format: &str) -> &'static str {
    match format {
        "json" | "json-pretty" => "json",
        "msgpack" => "msgpack",
        _ => "bin",
    }
}

/// Build a `Project` by walking the configured input directory and parsing
/// every Excel file we find. Returns either the project + collected
/// diagnostics, or the diagnostics + the parse error (for the caller to
/// surface verbatim).
fn build_project(
    dir: &Path,
    config: &Config,
    input_path: &Path,
    parser: &dyn tablec_core::core::schema::SchemaParser,
) -> Result<(Project, Vec<Diagnostic>), (Vec<Diagnostic>, String)> {
    use tablec_core::core::config::find_excel_files;
    let files = match find_excel_files(
        &input_path.to_string_lossy(),
        config.data.include.as_deref().unwrap_or(&[]),
        config.data.exclude.as_deref().unwrap_or(&[]),
    ) {
        Ok(f) => f,
        Err(e) => {
            return Err((
                vec![Diagnostic::new(
                    tablec_core::core::diagnostic::DiagnosticCode::Other,
                    format!("failed to enumerate input files: {e}"),
                    tablec_core::core::diagnostic::SourceLocation::default(),
                )],
                e.to_string(),
            ));
        }
    };

    let mut tables = Vec::new();
    let mut diagnostics = Vec::new();
    if files.is_empty() {
        diagnostics.push(Diagnostic {
            severity: tablec_core::core::diagnostic::Severity::Warning,
            code: tablec_core::core::diagnostic::DiagnosticCode::Other,
            message: format!(
                "no spreadsheet files found under {} (include: {:?}, exclude: {:?})",
                input_path.display(),
                config.data.include.as_deref().unwrap_or(&[]),
                config.data.exclude.as_deref().unwrap_or(&[]),
            ),
            location: tablec_core::core::diagnostic::SourceLocation::default(),
        });
    }
    for file in &files {
        match tablec_core::core::table::table::read_excel_with(&file.to_string_lossy(), parser) {
            Ok(mut t) => tables.append(&mut t),
            Err(errs) => diagnostics.extend(errs),
        }
    }
    if !diagnostics.is_empty() && tables.is_empty() {
        return Err((diagnostics, "all sheets failed to parse".to_string()));
    }
    let source = if files.is_empty() {
        vec![dir.to_path_buf()]
    } else {
        files
    };
    let project = Project::from_tables_with_source(config.project.name.clone(), tables, source);
    Ok((project, diagnostics))
}

// -----------------------------------------------------------------------------
// POST /api/check
// -----------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct CheckRequest {
    pub dir: String,
    #[serde(default)]
    pub parser: Option<String>,
    #[serde(default)]
    pub plugin_paths: Vec<String>,
}

#[derive(Serialize)]
pub struct CheckResponse {
    pub diagnostics: Vec<Diagnostic>,
    pub duration_ms: u128,
    pub sheets_checked: usize,
}

pub async fn api_check(
    State(state): State<Arc<WebuiState>>,
    Json(req): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, ApiError> {
    let dir = PathBuf::from(&req.dir);
    if !dir.is_dir() {
        return Err(ApiError::not_found(format!(
            "directory not found: {}",
            dir.display()
        )));
    }
    let parser_name = req
        .parser
        .clone()
        .or_else(|| state.parser_override.clone())
        .unwrap_or_else(|| "standard".to_string());
    let parser = state
        .registry
        .get(&parser_name)
        .ok_or_else(|| ApiError::bad_request(format!("unknown parser '{parser_name}'")))?;
    if !req.plugin_paths.is_empty() {
        return Err(ApiError::bad_request(
            "plugin_paths from HTTP requests are not accepted (CLI flag only)",
        ));
    }
    let (config, config_from) = resolve_config(&state, &dir).await;
    let input_path = resolve_input_dir(&dir, &config.data.input_dir);
    if !input_path.is_dir() {
        return Err(ApiError::bad_request(format!(
            "input directory not found: {} (input_dir = {:?} from {}); put spreadsheets there or set [data] input_dir in tablec.toml",
            input_path.display(),
            config.data.input_dir,
            config_from
                .as_deref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "built-in defaults".to_string()),
        )));
    }

    let started = Instant::now();
    use tablec_core::core::config::find_excel_files;
    let files = find_excel_files(
        &input_path.to_string_lossy(),
        config.data.include.as_deref().unwrap_or(&[]),
        config.data.exclude.as_deref().unwrap_or(&[]),
    )
    .map_err(|e| ApiError::internal(format!("enumerate: {e}")))?;

    let mut tables = Vec::new();
    let mut diagnostics = Vec::new();
    if files.is_empty() {
        diagnostics.push(Diagnostic {
            severity: tablec_core::core::diagnostic::Severity::Warning,
            code: tablec_core::core::diagnostic::DiagnosticCode::Other,
            message: format!(
                "no spreadsheet files found under {} (include: {:?}, exclude: {:?})",
                input_path.display(),
                config.data.include.as_deref().unwrap_or(&[]),
                config.data.exclude.as_deref().unwrap_or(&[]),
            ),
            location: tablec_core::core::diagnostic::SourceLocation::default(),
        });
    }
    for f in &files {
        match tablec_core::core::table::table::read_excel_with(
            &f.to_string_lossy(),
            parser.as_ref(),
        ) {
            Ok(mut t) => {
                tables.append(&mut t);
                // Belt-and-suspenders: also run cross-table `@ref` validation
                // (which the CLI commands currently skip via `validate_all`).
                if let Err(errs) = ConstraintValidator::validate_project(&tables) {
                    diagnostics.extend(errs);
                }
            }
            Err(errs) => diagnostics.extend(errs),
        }
    }

    Ok(Json(CheckResponse {
        diagnostics,
        duration_ms: started.elapsed().as_millis(),
        sheets_checked: tables.len(),
    }))
}

// -----------------------------------------------------------------------------
// POST /api/validate  —  501 Not Implemented
// -----------------------------------------------------------------------------

pub async fn api_validate() -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "数据校验功能仍在研究中；CLI flag --no-browser / issue tablec-2fy",
    ))
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use tablec_core::core::schema::SchemaParserRegistry;

    fn fixture_xlsx() -> std::path::PathBuf {
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate_dir.join("../tablec-core/tests/fixtures/testdata/basic_table.xlsx")
    }

    fn fixture_dir() -> std::path::PathBuf {
        fixture_xlsx()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures/testdata")
    }

    /// `Dir::files()` only yields direct children; recurse to collect all.
    fn all_dist_files() -> Vec<&'static include_dir::File<'static>> {
        fn walk(dir: &'static include_dir::Dir<'static>, out: &mut Vec<&'static include_dir::File<'static>>) {
            out.extend(dir.files());
            for sub in dir.dirs() {
                walk(sub, out);
            }
        }
        let mut out = Vec::new();
        walk(&WEBUI_DIST, &mut out);
        out
    }

    fn make_state(dir: std::path::PathBuf) -> Arc<WebuiState> {
        Arc::new(WebuiState::new(
            dir,
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            None,
        ))
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert!(v["version"].as_str().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn index_html_returns_html() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ct.starts_with("text/html"), "got content-type {ct}");
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(
            s.contains("<!DOCTYPE html>") || s.contains("<html"),
            "got {s}"
        );
    }

    #[tokio::test]
    async fn validate_returns_501() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let req = Request::builder()
            .method("POST")
            .uri("/api/validate")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "not_implemented");
    }

    #[tokio::test]
    async fn files_lists_basic_fixture() {
        let dir = fixture_dir();
        if !dir.is_dir() {
            // test data not present (rare); skip rather than fail.
            eprintln!("skipping: fixture dir {} not present", dir.display());
            return;
        }
        let app = crate::router::router(make_state(dir.clone()));
        let url = format!(
            "/api/files?dir={}",
            urlencoding::encode(&dir.display().to_string())
        );
        let resp = app
            .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let entries: Vec<FileEntry> = serde_json::from_slice(&body).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.ends_with(".xlsx")),
            "expected at least one .xlsx in {:?}, got {names:?}",
            dir
        );
    }

    #[tokio::test]
    async fn preview_returns_grid_for_basic_fixture() {
        let p = fixture_xlsx();
        if !p.exists() {
            eprintln!("skipping: fixture {} not present", p.display());
            return;
        }
        let sheets = excel::list_sheets(&p).unwrap();
        let target = sheets.first().unwrap().name.clone();
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = format!(
            "/api/preview?path={}&sheet={}&max_rows=20",
            urlencoding::encode(&p.display().to_string()),
            urlencoding::encode(&target),
        );
        let resp = app
            .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        // Grid has an untagged enum (Cell) which doesn't auto-deserialize;
        // assert the JSON shape via Value instead.
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["sheet"].as_str(), Some(target.as_str()));
        let rows = v["rows"].as_array().expect("rows is array");
        assert!(!rows.is_empty(), "rows shouldn't be empty");
        assert!(
            rows[0].as_array().map(|a| !a.is_empty()).unwrap_or(false),
            "header row shouldn't be empty"
        );
        // helper guarantees ≥5 schema rows
        assert!(rows.len() >= 5, "got {} rows", rows.len());
    }

    #[tokio::test]
    async fn check_returns_200_with_diagnostics_shape() {
        let dir = fixture_dir();
        if !dir.is_dir() {
            eprintln!("skipping: fixture dir {} not present", dir.display());
            return;
        }
        let app = crate::router::router(make_state(dir.clone()));
        let body = serde_json::json!({ "dir": dir.display().to_string() }).to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["diagnostics"].is_array());
        assert!(v["duration_ms"].is_number());
        assert!(v["sheets_checked"].is_number());
    }

    // -------------------------------------------------------------------------
    // Static asset content types
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // Static assets (embedded Vite dist)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn embedded_dist_serves_hashed_js_and_css() {
        // The Vite build emits /assets/index-<hash>.js/.css; find their
        // actual embedded names and fetch them through the router. Skips
        // when dist/ hasn't been generated (placeholder-only build).
        let files = all_dist_files();
        if files.len() <= 1 {
            eprintln!("skipping: webui/dist has no build output; run `pnpm build`");
            return;
        }
        let js = files
            .iter()
            .find(|f| f.path().extension().is_some_and(|e| e == "js"))
            .expect("embedded dist has no .js asset");
        let css = files
            .iter()
            .find(|f| f.path().extension().is_some_and(|e| e == "css"))
            .expect("embedded dist has no .css asset");
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));

        for (path, expected_ct) in [
            (format!("/{}", js.path().display()), "application/javascript"),
            (format!("/{}", css.path().display()), "text/css"),
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path.clone())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "GET {path}");
            let ct = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            assert!(ct.starts_with(expected_ct), "GET {path}: got {ct}");
        }
    }

    #[tokio::test]
    async fn embedded_dist_bundle_contains_lit_and_web_awesome() {
        // Sanity-check that the built bundle really contains Lit 3 and the
        // Web Awesome runtime we rely on. Skips on placeholder-only builds.
        let files = all_dist_files();
        if files.len() <= 1 {
            eprintln!("skipping: webui/dist has no build output; run `pnpm build`");
            return;
        }
        let js = files
            .iter()
            .find(|f| f.path().extension().is_some_and(|e| e == "js"))
            .expect("embedded dist has no .js asset");
        let s = std::str::from_utf8(js.contents()).unwrap();
        assert!(
            s.contains("LitElement") || s.contains("lit-element") || s.contains("lit-html"),
            "bundle missing Lit"
        );
        assert!(
            s.contains("web-awesome") || s.contains("webawesome") || s.contains("wa-button"),
            "bundle missing Web Awesome"
        );
    }

    #[tokio::test]
    async fn unknown_extensionless_path_falls_back_to_index_html() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/some/client/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ct.starts_with("text/html"), "got content-type {ct}");
    }

    #[tokio::test]
    async fn unknown_asset_path_404s() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/no/such/file.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // -------------------------------------------------------------------------
    // /api/state
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn state_returns_expected_fields() {
        let dir = std::path::PathBuf::from("/tmp/webui_smoke_state_test");
        std::fs::create_dir_all(&dir).ok();
        let app = crate::router::router(make_state(dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/state")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["dir"].as_str(), Some(dir.display().to_string().as_str()));
        assert!(v["parser_names"].is_array());
        assert!(
            v["parser_names"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n.as_str() == Some("standard"))
        );
        assert_eq!(v["active_parser"].as_str(), Some("standard"));
        assert!(v["config_present"].is_boolean());
        // input_dir surfaces the resolved scan path (<dir>/<input_dir>).
        assert_eq!(
            v["input_dir"].as_str(),
            Some(dir.display().to_string().as_str()),
            "no-config fallback must scan the state dir itself"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // -------------------------------------------------------------------------
    // input_dir resolution & guards (webui fallback scans the dir itself)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn files_lists_root_when_no_config_but_not_data_subdir() {
        // Loose xlsx in the state dir + no tablec.toml → /api/files must
        // list them (input_dir fallback is "."), even though a stray `data/`
        // directory exists.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("data")).unwrap();
        std::fs::copy(fixture_xlsx(), tmp.path().join("loose.xlsx")).unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/files")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let entries: Vec<FileEntry> = serde_json::from_slice(&body).unwrap();
        assert!(
            entries.iter().any(|e| e.name == "loose.xlsx"),
            "expected loose.xlsx listed, got {entries:?}"
        );
    }

    #[tokio::test]
    async fn check_400_when_input_dir_missing() {
        // tablec.toml points at data/ which doesn't exist → the action can't
        // run at all, so it must fail loudly instead of "succeeding" on zero
        // files.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            "[project]\nname = \"t\"\n\n[data]\ninput_dir = \"data\"\n\n[export]\nformat = \"json\"\noutput_dir = \"output\"\n",
        )
        .unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"dir":"{}"}}"#,
                tmp.path().display()
            )))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), 8 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let msg = v["message"].as_str().unwrap_or_default();
        assert!(msg.contains("input directory not found"), "got: {msg}");
        assert!(msg.contains("tablec.toml"), "hint missing: {msg}");
    }

    #[tokio::test]
    async fn check_warns_when_input_dir_exists_but_empty() {
        // Input dir exists (the state dir itself) but holds no spreadsheets
        // → 200 with a Warning diagnostic, not a silent all-clear.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "not a spreadsheet").unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"dir":"{}"}}"#,
                tmp.path().display()
            )))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 16 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let diags = v["diagnostics"].as_array().unwrap();
        assert_eq!(
            diags.len(),
            1,
            "expected exactly the no-files warning: {diags:?}"
        );
        assert_eq!(diags[0]["severity"], "Warning");
        assert!(
            diags[0]["message"]
                .as_str()
                .unwrap_or("")
                .contains("no spreadsheet files found"),
            "got: {diags:?}"
        );
    }

    #[tokio::test]
    async fn build_400_when_input_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            "[project]\nname = \"t\"\n\n[data]\ninput_dir = \"data\"\n\n[export]\nformat = \"json\"\noutput_dir = \"output\"\n",
        )
        .unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"dir":"{}","format":"json"}}"#,
                tmp.path().display()
            )))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -------------------------------------------------------------------------
    // /api/files error paths
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn files_404_when_dir_missing() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = "/api/files?dir=%2Fno%2Fsuch%2Fdir%2Fsomewhere%2Fzzz";
        let resp = app
            .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "not_found");
    }

    #[tokio::test]
    async fn files_falls_back_to_state_dir_when_query_missing() {
        // When ?dir= is omitted, /api/files uses state.dir. Build state
        // pointing at the fixture dir; the response must include the xlsx.
        let dir = fixture_dir();
        if !dir.is_dir() {
            eprintln!("skipping: fixture dir {} not present", dir.display());
            return;
        }
        let app = crate::router::router(make_state(dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/files")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let entries: Vec<FileEntry> = serde_json::from_slice(&body).unwrap();
        assert!(
            !entries.is_empty(),
            "expected ≥1 file under {}",
            dir.display()
        );
    }

    // -------------------------------------------------------------------------
    // /api/sheets & /api/preview error paths
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn sheets_404_when_path_missing() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = "/api/sheets?path=%2Fno%2Fsuch%2Ffile.xlsx";
        let resp = app
            .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "not_found");
    }

    #[tokio::test]
    async fn preview_404_when_path_missing() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = "/api/preview?path=%2Fno%2Fsuch.xlsx&sheet=Items";
        let resp = app
            .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn preview_404_when_sheet_missing() {
        let p = fixture_xlsx();
        if !p.exists() {
            eprintln!("skipping: fixture {} not present", p.display());
            return;
        }
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = format!(
            "/api/preview?path={}&sheet=does-not-exist",
            urlencoding::encode(&p.display().to_string()),
        );
        let resp = app
            .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            v["message"]
                .as_str()
                .unwrap_or("")
                .contains("does-not-exist")
        );
    }

    // -------------------------------------------------------------------------
    // /api/parsed_preview — happy + error paths
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn parsed_preview_returns_schema_and_typed_rows() {
        let p = fixture_xlsx();
        if !p.exists() {
            eprintln!("skipping: fixture {} not present", p.display());
            return;
        }
        let sheets = excel::list_sheets(&p).unwrap();
        let target = sheets.first().unwrap().name.clone();
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = format!(
            "/api/parsed_preview?path={}&sheet={}&max_rows=20",
            urlencoding::encode(&p.display().to_string()),
            urlencoding::encode(&target),
        );
        let resp = app
            .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["sheet"].as_str(), Some(target.as_str()));
        assert!(v["schema"]["fields"].is_array(), "schema.fields missing");
        assert!(
            v["schema"]["fields"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        );
        assert_eq!(v["data_start_row"].as_u64(), Some(5));
        assert!(v["rows"].is_array());
        assert!(v["rows"].as_array().map(|a| !a.is_empty()).unwrap_or(false));
        assert!(v["summary"]["error_count"].is_number());
        assert!(v["summary"]["total_rows"].as_u64().unwrap() >= 5);
    }

    #[tokio::test]
    async fn parsed_preview_rejects_unknown_parser() {
        let p = fixture_xlsx();
        if !p.exists() {
            eprintln!("skipping: fixture {} not present", p.display());
            return;
        }
        let sheets = excel::list_sheets(&p).unwrap();
        let target = sheets.first().unwrap().name.clone();
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = format!(
            "/api/parsed_preview?path={}&sheet={}&parser=does-not-exist",
            urlencoding::encode(&p.display().to_string()),
            urlencoding::encode(&target),
        );
        let resp = app
            .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "bad_request");
        assert!(
            v["message"]
                .as_str()
                .unwrap_or("")
                .contains("does-not-exist")
        );
    }

    #[tokio::test]
    async fn parsed_preview_404_when_path_missing() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let url = "/api/parsed_preview?path=%2Fno%2Fsuch.xlsx&sheet=Items";
        let resp = app
            .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // -------------------------------------------------------------------------
    // /api/build — happy + error paths
    // -------------------------------------------------------------------------

    fn tmp_with_xlsx() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::copy(fixture_xlsx(), data.join("basic_table.xlsx")).unwrap();
        (tmp, data)
    }

    #[tokio::test]
    async fn build_returns_json_for_valid_request() {
        let (tmp, _data) = tmp_with_xlsx();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            r#"
[project]
name = "smoke"

[data]
input_dir = "data"
include = ["*.xlsx"]

[export]
format = "json"
output_dir = "out"
"#,
        )
        .unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let body = serde_json::json!({
            "dir": tmp.path().display().to_string(),
            "format": "json-pretty",
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["format"], "json-pretty");
        assert!(v["bytes"].as_u64().unwrap() > 0);
        assert_eq!(v["written"], false);
        assert!(v["preview_first_500"].as_str().unwrap().contains("smoke"));
        assert!(v["diagnostics"].is_array());
        assert!(v["diagnostics"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn build_writes_to_disk_when_write_true() {
        let (tmp, _data) = tmp_with_xlsx();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            r#"
[project]
name = "out"

[data]
input_dir = "data"
include = ["*.xlsx"]

[export]
format = "json"
output_dir = "out"
"#,
        )
        .unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let body = serde_json::json!({
            "dir": tmp.path().display().to_string(),
            "format": "json",
            "write": true,
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["written"], true);
        let on_disk = tmp.path().join("out").join("out.json");
        assert!(on_disk.exists(), "expected {} to exist", on_disk.display());
    }

    #[tokio::test]
    async fn build_rejects_unknown_format() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let body = serde_json::json!({
            "dir": ".",
            "format": "protobuf",
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "bad_request");
    }

    #[tokio::test]
    async fn build_rejects_plugin_paths_from_http() {
        // plugin_paths from HTTP must be rejected with 400 — only the CLI
        // flag is trusted. This is the security boundary for cdylib loading.
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let body = serde_json::json!({
            "dir": ".",
            "format": "json",
            "plugin_paths": ["/tmp/evil.so"],
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "bad_request");
        assert!(v["message"].as_str().unwrap_or("").contains("plugin_paths"));
    }

    #[tokio::test]
    async fn build_rejects_missing_dir() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let body = serde_json::json!({
            "dir": "/no/such/dir/zzz",
            "format": "json",
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/build")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // -------------------------------------------------------------------------
    // /api/check error paths
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn check_404_when_dir_missing() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let body = serde_json::json!({ "dir": "/no/such/dir/zzz" }).to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn check_rejects_plugin_paths_from_http() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let body = serde_json::json!({
            "dir": ".",
            "plugin_paths": ["/tmp/evil.so"],
        })
        .to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn check_runs_against_real_dir_and_returns_shape() {
        let (tmp, _data) = tmp_with_xlsx();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            r#"
[project]
name = "smoke"

[data]
input_dir = "data"
include = ["*.xlsx"]

[export]
format = "json"
output_dir = "out"
"#,
        )
        .unwrap();
        let app = crate::router::router(make_state(tmp.path().to_path_buf()));
        let body = serde_json::json!({ "dir": tmp.path().display().to_string() }).to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/api/check")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["diagnostics"].is_array());
        assert!(v["sheets_checked"].as_u64().unwrap() >= 1);
    }

    // -------------------------------------------------------------------------
    // /api/validate body shape
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn validate_501_body_has_todo_field() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let req = Request::builder()
            .method("POST")
            .uri("/api/validate")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
        let body = axum::body::to_bytes(resp.into_body(), 4 * 1024)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "not_implemented");
        assert!(v["todo"].is_string());
        assert!(v["message"].is_string());
    }

    // -------------------------------------------------------------------------
    // Unknown routes return 404
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let app = crate::router::router(make_state(std::path::PathBuf::from(".")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

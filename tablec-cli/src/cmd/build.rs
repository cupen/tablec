use clap::Args;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tablec_core::core::config::{self, Config};
use tablec_core::core::project::project::Project;
use tablec_core::core::table::constraint::ConstraintValidator;
use tablec_core::core::table::table::read_excel;
use tablec_core::export::{Format, Json, Msgpack};

/// Format a `Duration` as a human-readable ms/s string.
fn fmt_duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 10.0 {
        format!("{:.2}ms", ms)
    } else if ms < 100.0 {
        format!("{:.1}ms", ms)
    } else if ms < 10_000.0 {
        format!("{:.0}ms", ms)
    } else {
        format!("{:.1}s", ms / 1000.0)
    }
}

/// Look for `tablec.toml` (preferred) or `.tablec.toml` inside `dir`.
/// Returns the first match, or `None` if neither file exists.
fn find_tablec_toml(dir: &Path) -> Option<PathBuf> {
    let plain = dir.join("tablec.toml");
    if plain.exists() {
        return Some(plain);
    }
    let dotfile = dir.join(".tablec.toml");
    if dotfile.exists() {
        return Some(dotfile);
    }
    None
}

/// Map an export format name to its file extension.
fn ext_for(format: &str) -> &'static str {
    if format == "msgpack" {
        "msgpack"
    } else {
        "json"
    }
}

/// Validate a single table, printing any diagnostics to stderr. Returns
/// silently on success. Validation never aborts the build.
fn validate_table_silent(table: &tablec_core::core::table::table::Table) {
    if let Err(errs) = ConstraintValidator::validate_table(table) {
        let _ = crate::diag_render::render_diags(&errs, &mut io::stderr().lock());
    }
}

#[derive(Args, Debug)]
pub struct BuildCommand {
    /// Input Excel file (if not using config)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (if not using config)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Config file path (default: tablec.toml in current directory)
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Export format: json (minified) | json-pretty (indented) | msgpack
    #[arg(long)]
    pub format: Option<String>,

    /// Include field metadata
    #[arg(long)]
    pub include_fields: Option<bool>,
}

impl BuildCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let input_path = self
            .input
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new("."));

        let explicit_cfg = Config::load(self.config.as_deref())?;

        if input_path.is_dir() {
            self.run_dir(input_path, explicit_cfg)
        } else if input_path.is_file() {
            self.run_single(input_path, explicit_cfg)
        } else {
            Err(format!("input {:?} is neither a file nor a directory", input_path).into())
        }
    }

    fn run_single(&self, input: &Path, explicit_cfg: Option<Config>) -> Result<(), Box<dyn Error>> {
        let cfg = explicit_cfg.unwrap_or_default();
        let format = self.format.clone().unwrap_or(cfg.export.format);
        let include_fields = self
            .include_fields
            .unwrap_or(cfg.export.include_fields.unwrap_or(false));

        let output = self
            .output
            .as_ref()
            .ok_or("No output file specified. Use -o <file> or provide a config file.")?;

        let input_str = input.to_string_lossy().into_owned();
        let format_for_ext = format.clone();
        self.build_single_file_with_ext(
            &input_str,
            output,
            &format,
            include_fields,
            ext_for(&format_for_ext),
        )
    }

    fn run_dir(
        &self,
        input_dir: &Path,
        explicit_cfg: Option<Config>,
    ) -> Result<(), Box<dyn Error>> {
        let cfg = match explicit_cfg {
            Some(c) => c,
            None => match find_tablec_toml(input_dir) {
                Some(path) => Config::load_from_file(&path)?,
                None => Config::default(),
            },
        };

        let format = self.format.clone().unwrap_or(cfg.export.format);
        let include_fields = self
            .include_fields
            .unwrap_or(cfg.export.include_fields.unwrap_or(false));

        let include = cfg.data.include.clone().unwrap_or_default();
        let exclude = cfg.data.exclude.clone().unwrap_or_default();
        let files = config::find_excel_files(&input_dir.to_string_lossy(), &include, &exclude)?;

        if files.is_empty() {
            return Err(format!(
                "directory {:?} contains no xlsx files matching config",
                input_dir
            )
            .into());
        }

        let ext = ext_for(&format).to_string();

        // Output path: from -o if given; else from config's export.output_dir
        // and project.name; else error.
        let output_path: PathBuf = match self.output.as_deref() {
            Some(s) => PathBuf::from(s),
            None => {
                let dir = cfg.export.output_dir.as_str();
                let name = cfg.project.name.as_str();
                let path = format!("{}/{}.{}", dir, name, ext);
                PathBuf::from(path)
            }
        };

        self.build_merged_files_with_ext(&files, &output_path, &format, include_fields, &ext)
    }

    fn build_single_file_with_ext(
        &self,
        input: &str,
        output: &str,
        format: &str,
        include_fields: bool,
        ext: &str,
    ) -> Result<(), Box<dyn Error>> {
        let total_start = Instant::now();

        let parse_start = Instant::now();
        let tables = match read_excel(input) {
            Ok(t) => t,
            Err(errs) => {
                crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                return Err(format!(
                    "read_excel failed: {}",
                    crate::diag_render::diag_summary(&errs)
                )
                .into());
            }
        };
        let parse_elapsed = parse_start.elapsed();
        let table_count = tables.len();

        for table in &tables {
            let v_start = Instant::now();
            validate_table_silent(table);
            let v_elapsed = v_start.elapsed();
            eprintln!(
                "{}/{}: {} rows (parse={}, validate={})",
                input,
                table.name,
                table.data.len(),
                fmt_duration(parse_elapsed),
                fmt_duration(v_elapsed),
            );
        }

        let project = Project::from_tables("unnamed".to_string(), tables);

        match format {
            "json" => {
                Json {
                    pretty: false,
                    include_fields,
                }
                .export(&project, output)?;
            }
            "json-pretty" => {
                Json {
                    pretty: true,
                    include_fields,
                }
                .export(&project, output)?;
            }
            "msgpack" => {
                Msgpack.export(&project, output)?;
            }
            _ => {
                return Err(format!(
                    "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                    format
                )
                .into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "Total: {} tables, {}",
            table_count,
            fmt_duration(total_elapsed)
        );
        Ok(())
    }

    fn build_merged_files_with_ext(
        &self,
        files: &[PathBuf],
        output: &Path,
        format: &str,
        include_fields: bool,
        ext: &str,
    ) -> Result<(), Box<dyn Error>> {
        let _ = ext;
        let output_str = output
            .to_str()
            .ok_or_else(|| format!("output path {:?} is not valid UTF-8", output))?;
        let total_start = Instant::now();

        // Merge all tables from all files; track per-file parse time and
        // emit per-table validate line as we go (before Project::from_tables
        // collapses duplicates by name).
        let mut all_tables = Vec::new();
        let mut total_table_count = 0;
        for file_path in files {
            let parse_start = Instant::now();
            let tables = match read_excel(file_path.to_str().unwrap()) {
                Ok(t) => t,
                Err(errs) => {
                    crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                    return Err(format!(
                        "read_excel failed: {}",
                        crate::diag_render::diag_summary(&errs)
                    )
                    .into());
                }
            };
            let parse_elapsed = parse_start.elapsed();
            let file_str = file_path.to_string_lossy();

            for table in &tables {
                let v_start = Instant::now();
                validate_table_silent(table);
                let v_elapsed = v_start.elapsed();
                eprintln!(
                    "{}/{}: {} rows (parse={}, validate={})",
                    file_str,
                    table.name,
                    table.data.len(),
                    fmt_duration(parse_elapsed),
                    fmt_duration(v_elapsed),
                );
            }

            total_table_count += tables.len();
            all_tables.extend(tables);
        }

        let project = Project::from_tables("unnamed".to_string(), all_tables);

        match format {
            "json" => {
                Json {
                    pretty: false,
                    include_fields,
                }
                .export(&project, output_str)?;
                println!("Merged {} tables into {}", project.tables.len(), output_str);
            }
            "json-pretty" => {
                Json {
                    pretty: true,
                    include_fields,
                }
                .export(&project, output_str)?;
                println!("Merged {} tables into {}", project.tables.len(), output_str);
            }
            "msgpack" => {
                Msgpack.export(&project, output_str)?;
                println!("Merged tables into {}", output_str);
            }
            _ => {
                return Err(format!(
                    "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                    format
                )
                .into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "Total: {} tables, {}",
            total_table_count,
            fmt_duration(total_elapsed)
        );
        Ok(())
    }
}

// This function is for the python library, returning the JSON as a string.
pub fn build_to_string(
    input_path: &str,
    format: &str,
    include_fields: bool,
) -> Result<String, Box<dyn Error>> {
    let tables = match read_excel(input_path) {
        Ok(t) => t,
        Err(errs) => {
            crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
            return Err(format!(
                "read_excel failed: {}",
                crate::diag_render::diag_summary(&errs)
            )
            .into());
        }
    };
    let project = Project::from_tables("unnamed".to_string(), tables);
    match format {
        "json" => {
            let json = Json {
                pretty: false,
                include_fields,
            };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        "json-pretty" => {
            let json = Json {
                pretty: true,
                include_fields,
            };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        // Other formats could be added here if needed.
        _ => Err(format!(
            "Unsupported format '{}'. Use 'json' or 'json-pretty'.",
            format
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_duration;
    use std::time::Duration;

    #[test]
    fn fmt_duration_sub_millisecond() {
        let s = fmt_duration(Duration::from_micros(500));
        assert!(s.ends_with("ms"), "got: {}", s);
        // 0.5ms — should be 2 decimal places
        assert!(s.starts_with("0.5"), "got: {}", s);
    }

    #[test]
    fn fmt_duration_one_digit_ms() {
        let s = fmt_duration(Duration::from_micros(2_400));
        assert_eq!(s, "2.40ms");
    }

    #[test]
    fn fmt_duration_two_digit_ms() {
        let s = fmt_duration(Duration::from_micros(45_000));
        assert_eq!(s, "45.0ms");
    }

    #[test]
    fn fmt_duration_three_digit_ms() {
        let s = fmt_duration(Duration::from_micros(250_000));
        assert_eq!(s, "250ms");
    }

    #[test]
    fn fmt_duration_seconds() {
        let s = fmt_duration(Duration::from_micros(12_500_000));
        assert_eq!(s, "12.5s");
    }

    use super::find_tablec_toml;
    use std::fs;

    #[test]
    fn find_tablec_toml_prefers_tablec_over_dotfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("tablec.toml"), "").unwrap();
        fs::write(dir.path().join(".tablec.toml"), "").unwrap();
        let found = find_tablec_toml(dir.path()).unwrap();
        assert_eq!(found, dir.path().join("tablec.toml"));
    }

    #[test]
    fn find_tablec_toml_falls_back_to_dotfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".tablec.toml"), "").unwrap();
        let found = find_tablec_toml(dir.path()).unwrap();
        assert_eq!(found, dir.path().join(".tablec.toml"));
    }

    #[test]
    fn find_tablec_toml_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_tablec_toml(dir.path()).is_none());
    }

    use super::{BuildCommand, ext_for};
    use rust_xlsxwriter::Workbook;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_minimal_xlsx(dir: &std::path::Path, name: &str) -> PathBuf {
        write_minimal_xlsx_with_sheet(dir, name, "Sheet1")
    }

    fn write_minimal_xlsx_with_sheet(
        dir: &std::path::Path,
        name: &str,
        sheet_name: &str,
    ) -> PathBuf {
        let path = dir.join(name);
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        sheet.set_name(sheet_name).ok();
        sheet.write_string(0, 0, "field").ok();
        sheet.write_string(0, 1, "name").ok();
        sheet.write_string(0, 2, "value").ok();
        sheet.write_string(1, 0, "int").ok();
        sheet.write_string(1, 1, "string").ok();
        sheet.write_string(1, 2, "int").ok();
        // Row 2 (data)
        sheet.write_string(2, 0, "id").ok();
        sheet.write_string(2, 1, "alice").ok();
        sheet.write_number(2, 2, 1.0).ok();
        wb.save(&path).unwrap();
        path
    }

    #[test]
    fn ext_for_msgpack() {
        assert_eq!(ext_for("msgpack"), "msgpack");
    }

    #[test]
    fn ext_for_json() {
        assert_eq!(ext_for("json"), "json");
    }

    #[test]
    fn ext_for_json_pretty() {
        assert_eq!(ext_for("json-pretty"), "json");
    }

    #[test]
    fn test_dir_mode_uses_default_when_no_config() {
        let dir = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(dir.path(), "a.xlsx", "SheetA");
        write_minimal_xlsx_with_sheet(dir.path(), "b.xlsx", "SheetB");
        fs::write(dir.path().join("notes.csv"), "ignore me").unwrap();

        let cmd = BuildCommand {
            input: Some(dir.path().to_string_lossy().into_owned()),
            output: Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            config: None,
            format: None,
            include_fields: None,
        };
        cmd.run().expect("dir mode build should succeed");

        // One merged output file containing both sheets
        let out_text = fs::read_to_string(dir.path().join("out.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        let tables = parsed
            .get("tables")
            .and_then(|v| v.as_array())
            .expect("tables array");
        assert_eq!(tables.len(), 2, "expected 2 tables, got: {}", out_text);
        let names: Vec<&str> = tables
            .iter()
            .map(|t| t.get("name").and_then(|n| n.as_str()).unwrap_or(""))
            .collect();
        assert!(names.contains(&"SheetA") && names.contains(&"SheetB"));
    }

    #[test]
    fn test_dir_mode_auto_discovers_tablec_toml() {
        let dir = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(dir.path(), "only.xlsx", "Only");
        write_minimal_xlsx_with_sheet(dir.path(), "ignored.xlsx", "Ignored");
        fs::write(
            dir.path().join("tablec.toml"),
            r#"
[project]
name = "auto"

[data]
input_dir = "."

[export]
format = "json"
output_dir = "."
"#,
        )
        .unwrap();

        // Without -o, should write to ./auto.json in cwd; use -o to scope output
        let cmd = BuildCommand {
            input: Some(dir.path().to_string_lossy().into_owned()),
            output: Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            config: None,
            format: None,
            include_fields: None,
        };
        cmd.run().expect("dir mode build should succeed");

        let out_text = fs::read_to_string(dir.path().join("out.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        let tables = parsed
            .get("tables")
            .and_then(|v| v.as_array())
            .expect("tables array");
        assert!(tables.len() >= 1);
    }

    #[test]
    fn test_dir_mode_errors_when_no_xlsx() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "no xlsx here").unwrap();

        let cmd = BuildCommand {
            input: Some(dir.path().to_string_lossy().into_owned()),
            output: Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            config: None,
            format: None,
            include_fields: None,
        };
        let err = cmd.run().expect_err("should fail with no xlsx files");
        assert!(
            err.to_string().contains("no xlsx"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_single_file_still_requires_output() {
        let dir = tempdir().unwrap();
        let xlsx = write_minimal_xlsx(dir.path(), "foo.xlsx");

        let cmd = BuildCommand {
            input: Some(xlsx.to_string_lossy().into_owned()),
            output: None,
            config: None,
            format: None,
            include_fields: None,
        };
        let err = cmd.run().expect_err("single file without -o should fail");
        assert!(
            err.to_string().contains("No output file specified"),
            "unexpected error: {}",
            err
        );
    }
}

use clap::Args;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tablec_core::core::config::{self, Config};
use tablec_core::core::table::constraint::ConstraintValidator;
use tablec_core::core::table::table::read_excel;
use tablec_core::core::project::project::Project;
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
        // Load configuration
        let config = Config::load(self.config.as_deref())?;

        match config {
            Some(cfg) => self.run_with_config(cfg),
            None => {
                // No config file, use CLI arguments only
                self.run_without_config()
            }
        }
    }

    fn run_with_config(&self, cfg: Config) -> Result<(), Box<dyn Error>> {
        // CLI arguments override config values
        let format = self.format.clone().unwrap_or(cfg.export.format);
        let include_fields = self.include_fields.unwrap_or(
            cfg.export.include_fields.unwrap_or(false)
        );

        // Determine input/output
        if let (Some(input), Some(output)) = (&self.input, &self.output) {
            // CLI args take precedence
            self.build_single_file(input, output, &format, include_fields)?;
        } else {
            // Use config values - find all Excel files in input_dir
            let include = cfg.data.include.clone().unwrap_or_default();
            let exclude = cfg.data.exclude.clone().unwrap_or_default();
            let excel_files = config::find_excel_files(&cfg.data.input_dir, &include, &exclude)?;

            if excel_files.is_empty() {
                return Err(format!(
                    "No Excel files found in '{}' matching the specified patterns.",
                    cfg.data.input_dir
                ).into());
            }

            if excel_files.len() == 1 {
                // Single file - use project name as output
                let input_path = excel_files[0].to_string_lossy();
                let output = self.output.clone().unwrap_or_else(|| {
                    format!("{}/{}.{}",
                        cfg.export.output_dir,
                        cfg.project.name,
                        if format == "msgpack" { "msgpack" } else { "json" }
                    )
                });
                self.build_single_file(&input_path, &output, &format, include_fields)?;
            } else {
                // Multiple files - merge all tables into single output
                let output = self.output.clone().unwrap_or_else(|| {
                    format!("{}/{}.{}",
                        cfg.export.output_dir,
                        cfg.project.name,
                        if format == "msgpack" { "msgpack" } else { "json" }
                    )
                });
                self.build_merged_files(&excel_files, &output, &format, include_fields)?;
            }
        }

        Ok(())
    }

    fn run_without_config(&self) -> Result<(), Box<dyn Error>> {
        // Require explicit CLI arguments
        let input = self.input.as_ref()
            .ok_or("No input file specified. Use -i <file> or provide a config file.")?;
        let output = self.output.as_ref()
            .ok_or("No output file specified. Use -o <file> or provide a config file.")?;
        let format = self.format.clone().unwrap_or_else(|| "json".to_string());
        let include_fields = self.include_fields.unwrap_or(false);

        self.build_single_file(input, output, &format, include_fields)
    }

    fn build_single_file(
        &self,
        input: &str,
        output: &str,
        format: &str,
        include_fields: bool,
    ) -> Result<(), Box<dyn Error>> {
        let total_start = Instant::now();

        let parse_start = Instant::now();
        let tables = match read_excel(input) {
            Ok(t) => t,
            Err(errs) => {
                crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
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
                input, table.name, table.data.len(),
                fmt_duration(parse_elapsed), fmt_duration(v_elapsed),
            );
        }

        let project = Project::from_tables("unnamed".to_string(), tables);

        match format {
            "json" => {
                Json { pretty: false, include_fields }.export(&project, output)?;
            }
            "json-pretty" => {
                Json { pretty: true, include_fields }.export(&project, output)?;
            }
            "msgpack" => {
                Msgpack.export(&project, output)?;
            }
            _ => {
                return Err(format!("Unsupported format '{}'. Use one of: json, json-pretty, msgpack.", format).into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!("Total: {} tables, {}", table_count, fmt_duration(total_elapsed));
        Ok(())
    }

    fn build_merged_files(
        &self,
        files: &[PathBuf],
        output: &str,
        format: &str,
        include_fields: bool,
    ) -> Result<(), Box<dyn Error>> {
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
                    return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
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
                    file_str, table.name, table.data.len(),
                    fmt_duration(parse_elapsed), fmt_duration(v_elapsed),
                );
            }

            total_table_count += tables.len();
            all_tables.extend(tables);
        }

        let project = Project::from_tables("unnamed".to_string(), all_tables);

        match format {
            "json" => {
                Json { pretty: false, include_fields }.export(&project, output)?;
                println!("Merged {} tables into {}", project.tables.len(), output);
            }
            "json-pretty" => {
                Json { pretty: true, include_fields }.export(&project, output)?;
                println!("Merged {} tables into {}", project.tables.len(), output);
            }
            "msgpack" => {
                Msgpack.export(&project, output)?;
                println!("Merged tables into {}", output);
            }
            _ => {
                return Err(format!("Unsupported format '{}'. Use one of: json, json-pretty, msgpack.", format).into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!("Total: {} tables, {}", total_table_count, fmt_duration(total_elapsed));
        Ok(())
    }
}

// This function is for the python library, returning the JSON as a string.
pub fn build_to_string(input_path: &str, format: &str, include_fields: bool) -> Result<String, Box<dyn Error>> {
    let tables = match read_excel(input_path) {
        Ok(t) => t,
        Err(errs) => {
            crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
            return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
        }
    };
    let project = Project::from_tables("unnamed".to_string(), tables);
    match format {
        "json" => {
            let json = Json { pretty: false, include_fields };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        "json-pretty" => {
            let json = Json { pretty: true, include_fields };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        // Other formats could be added here if needed.
        _ => Err(format!("Unsupported format '{}'. Use 'json' or 'json-pretty'.", format).into()),
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
}

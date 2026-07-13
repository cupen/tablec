use clap::Args;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use tablec_core::core::config::{self, Config};
use tablec_core::core::table::table::read_excel;
use tablec_core::core::project::project::Project;
use tablec_core::export::{Format, Json, Msgpack};

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
        let tables = match read_excel(input) {
            Ok(t) => t,
            Err(errs) => {
                crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
            }
        };
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
        Ok(())
    }

    fn build_merged_files(
        &self,
        files: &[PathBuf],
        output: &str,
        format: &str,
        include_fields: bool,
    ) -> Result<(), Box<dyn Error>> {
        // Merge all tables from all files
        let mut all_tables = Vec::new();
        for file_path in files {
            let tables = match read_excel(file_path.to_str().unwrap()) {
                Ok(t) => t,
                Err(errs) => {
                    crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                    return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
                }
            };
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

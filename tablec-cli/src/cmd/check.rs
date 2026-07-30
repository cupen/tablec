use clap::Args;
use std::error::Error;
use std::path::PathBuf;
use tablec_core::core::config::{self, Config};
use tablec_core::core::table::{constraint::ConstraintValidator, table::read_excel};

#[derive(Args, Debug)]
pub struct CheckCommand {
    /// Config file path (default: tablec.toml in current directory)
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(short, long)]
    pub verbose: bool,

    #[arg()] // Positional argument
    pub path: Option<PathBuf>,

    /// Schema parser name to use. Overrides `tablec.toml [parser] name` when given.
    /// Falls back to "standard" if neither is set.
    #[arg(long)]
    pub parser: Option<String>,

    /// Additional schema parser plugin library path. May be repeated.
    #[arg(long = "plugin-path", value_name = "PATH")]
    pub plugin_paths: Vec<String>,
}

impl CheckCommand {
    pub fn run(self) -> Result<(), Box<dyn Error>> {
        return _run(self);
    }
}

fn _run(c: CheckCommand) -> Result<(), Box<dyn Error>> {
    println!("Checking tables...");

    let mut excel_files = Vec::new();

    // Try to load config
    let config = Config::load(c.config.as_deref())?;

    // Resolve schema parser (cli > config > "standard").
    let parser_cfg = config.clone().unwrap_or_default();
    let plugin_paths: Vec<PathBuf> = parser_cfg
        .plugins
        .iter()
        .map(|plugin| PathBuf::from(&plugin.path))
        .chain(c.plugin_paths.iter().map(PathBuf::from))
        .collect();
    let _parser =
        crate::parser_resolve::resolve_parser(&parser_cfg, c.parser.as_deref(), &plugin_paths);

    let path = if let Some(p) = c.path {
        // CLI path takes precedence
        PathBuf::from(p)
    } else if let Some(ref cfg) = config {
        // Use config's input_dir
        PathBuf::from(&cfg.data.input_dir)
    } else {
        // Default to current directory
        PathBuf::from(".")
    };

    if path.is_dir() {
        // Get include/exclude patterns from config if available
        let include = config
            .as_ref()
            .and_then(|c| c.data.include.clone())
            .unwrap_or_else(|| vec!["*.xlsx".to_string()]);
        let exclude = config
            .as_ref()
            .and_then(|c| c.data.exclude.clone())
            .unwrap_or_default();

        excel_files = config::find_excel_files(&path.to_string_lossy(), &include, &exclude)?;
    } else if path.is_file() {
        excel_files.push(path);
    }

    if excel_files.is_empty() {
        println!("No Excel files found to check.");
        return Ok(());
    }

    let mut total_errors = 0;

    for file_path in excel_files {
        println!("Checking file: {}", file_path.display());
        match read_excel(file_path.to_str().unwrap()) {
            Ok(tables) => {
                for table in tables {
                    println!("  Checking sheet: {}", table.name);
                    match ConstraintValidator::validate_table(&table) {
                        Ok(_) => {
                            if c.verbose {
                                println!("    OK");
                            }
                        }
                        Err(errors) => {
                            total_errors += errors.len();
                            for d in errors {
                                eprintln!("    Error: {}", d);
                            }
                        }
                    }
                }
            }
            Err(errs) => {
                total_errors += errs.len();
                crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
            }
        }
    }

    if total_errors > 0 {
        println!("\nFound {} errors.", total_errors);
        // Return an error to indicate failure
        return Err(format!("{} errors found during check", total_errors).into());
    } else {
        println!("\nCheck finished successfully. No errors found.");
    }

    Ok(())
}

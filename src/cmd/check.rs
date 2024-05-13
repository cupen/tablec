use clap::Args;
use std::error::Error;
use std::path::PathBuf;
use crate::core::table::{table::read_excel, validator::validate_table};

#[derive(Args, Debug)]
pub struct CheckCommand {
    #[arg(short, long)]
    pub verbose: bool,

    #[arg()] // Positional argument
    pub path: Option<PathBuf>,
}

impl CheckCommand {
    pub fn run(self) -> Result<(), Box<dyn Error>> {
        return _run(self);
    }
}

fn _run(c: CheckCommand) -> Result<(), Box<dyn Error>> {
    println!("Checking tables...");

    let mut excel_files = Vec::new();
    let path = c.path.unwrap_or_else(|| PathBuf::from("."));

    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "xlsx" || ext == "xls" {
                        excel_files.push(path);
                    }
                }
            }
        }
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
                    match validate_table(&table) {
                        Ok(_) => {
                            if c.verbose {
                                println!("    OK");
                            }
                        }
                        Err(errors) => {
                            total_errors += errors.len();
                            for error in errors {
                                eprintln!("    Error: {}", error);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                total_errors += 1;
                eprintln!("  Error reading file: {}", e);
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


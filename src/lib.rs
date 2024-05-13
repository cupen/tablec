pub mod core;
pub mod cli;
pub mod cmd;
pub mod export;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "tablec", about = "A tool for compiling Excel files.")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build data from Excel files
    Build(cmd::build::BuildCommand),
    /// Check Excel files for errors
    Check(cmd::check::CheckCommand),
    /// Start a web server
    Web(cmd::web::WebCommand),
}

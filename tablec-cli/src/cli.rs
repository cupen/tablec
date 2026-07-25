use clap::{Parser, Subcommand};

pub use crate::cmd::build::BuildCommand;
pub use crate::cmd::check::CheckCommand;
pub use crate::cmd::example::ExampleCommand;

#[derive(Debug, Parser)]
#[command(name = "tablec", about = "table compiler for build data from Excel files")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build data from Excel files
    Build(BuildCommand),
    /// Check Excel files for errors
    Check(CheckCommand),
    /// Create an example Excel file
    Example(ExampleCommand),
}

pub fn parse_args() -> Args {
    return Args::parse()
}
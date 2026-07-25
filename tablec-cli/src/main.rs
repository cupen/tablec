use cli::Command;
use tablec_cli::cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::parse_args();
    match args.command {
        Command::Build(c) => {
            c.run()?;
        }
        Command::Check(c) => {
            c.run()?;
        }
        Command::Example(c) => {
            c.run()?;
        }
    }
    Ok(())
}

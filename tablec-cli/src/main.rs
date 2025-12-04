use tablec_cli::cli;
use cli::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::parse_args();
    match args.command {
        Command::Build(c) => {
            c.run()?;
        }
        Command::Check(c) => {
            c.run()?;
        }
        Command::Web(c) => {
            c.run().await?;
        }
        Command::Example(c) => {
            c.run()?;
        }
    }
    Ok(())
}
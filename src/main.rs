use tablec::cli;
use cli::Command;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = cli::parse_args();
    match args.command {
        Command::Build(c) => {
            // cmd::build::_run(c).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            c.run().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            Ok(())
        }
        Command::Check(c) => {
            c.run().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            Ok(())
        }
        Command::Web(c) => {
            c.run().await
        }
        Command::Example(c) => {
            c.run().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            Ok(())
        }
    }
}
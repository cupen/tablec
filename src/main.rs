mod cli;
use cli::Command;

mod table;
use table as table;


fn main()  {
    let args = cli::parse_args();
    match args.command {
        Command::Build { input, output, format } => {
            if let Err(e) = table::build_json(input, output) {
                eprintln!("Error: {}", e);
            }
        }
        Command::Check { verbose } => {
            println!("Checking...");
        }
        Command::Web { listen } => {
            println!("Listening on: {}", listen);
        }
    }
}


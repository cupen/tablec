use serde::{Serialize};
use structopt::StructOpt;


#[derive(Debug, StructOpt)]
#[structopt(name = "args")]
pub struct Args {
    #[structopt(subcommand)]
    pub command: Command,

    #[structopt(short, long)]
    pub verbose: bool,
}

#[derive(StructOpt, Debug)]
pub enum Command {
    Build {
        #[structopt(short, long)]
        input: String,

        #[structopt(short, long)]
        output: String,

        #[structopt(long, default_value="json")]
        format: String,
    },

    Check {
        #[structopt(short, long)]
        verbose: bool,
    },

    Web {
        #[structopt(long)]
        listen: String,
    },
}


pub fn parse_args() -> Args {
    return Args::from_args()
}
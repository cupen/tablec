use clap::Args;
use std::error::Error;
use crate::core::table::table::read_excel;
use crate::export;

#[derive(Args, Debug)]
pub struct BuildCommand {
    #[arg(short, long)]
    pub input: String,

    #[arg(short, long)]
    pub output: String,

    #[arg(long, default_value="json")]
    pub format: String,

    #[arg(long, default_value_t = false)]
    pub include_fields: bool,
}

// This function is for the CLI, writing the output to a file.
impl BuildCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let tables = read_excel(&self.input)?;
        match self.format.as_str() {
            "json" => {
                let json_data = export::json::to_string(&tables, self.include_fields)?;
                std::fs::write(&self.output, json_data)?;
                println!("Exported data to {}", &self.output);
            }
            "msgpack" => {
                for table in tables {
                    export::msgpack::export(&table, &self.output)?;
                }
            }
            "protobuf" => {
                export::protobuf::export(&tables, &self.output)?;
            }
            _ => {
                return Err(format!("Unsupported format '{}'.", self.format).into());
            }
        }
        Ok(())
    }
}

// This function is for the python library, returning the JSON as a string.
pub fn build_to_string(input_path: &str, format: &str, include_fields: bool) -> Result<String, Box<dyn Error>> {
    let tables = read_excel(input_path)?;
    match format {
        "json" => {
            export::json::to_string(&tables, include_fields)
        }
        // Other formats could be added here if needed.
        _ => Err(format!("Unsupported format '{}'.", format).into()),
    }
}
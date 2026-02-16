use std::error::Error;
use crate::core::table::table::Table;

pub fn export(tab: &Table, output: &str) -> Result<(), Box<dyn Error>> {
    let encoded = rmp_serde::to_vec(tab)?;
    std::fs::write(output, encoded)?;
    println!("Exported data to {}", output);
    Ok(())
}

pub fn to_vec(tabs: &Vec<Table>) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = rmp_serde::to_vec(tabs)?;
    Ok(encoded)
}
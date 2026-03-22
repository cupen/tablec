use std::error::Error;
use crate::core::project::project::Project;
use crate::export::Format;

/// MessagePack export format
pub struct Msgpack;

impl Format for Msgpack {
    fn export(&self, project: &Project, output: &str) -> Result<(), Box<dyn Error>> {
        let data = self.to_vec(project)?;
        if let Some(parent) = std::path::Path::new(output).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, data)?;
        println!("Exported data to {}", output);
        Ok(())
    }

    fn to_vec(&self, project: &Project) -> Result<Vec<u8>, Box<dyn Error>> {
        let encoded = rmp_serde::to_vec(project)?;
        Ok(encoded)
    }
}

/// Legacy function for backward compatibility
pub fn to_vec(tabs: &Vec<crate::core::table::table::Table>) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = rmp_serde::to_vec(tabs)?;
    Ok(encoded)
}

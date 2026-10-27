use crate::core::project::project::Project;
use std::error::Error;

pub mod json;
pub mod msgpack;
pub mod report;
// pub mod protobuf;

/// Unified export format trait
pub trait Format {
    fn export(&self, project: &Project, output: &str) -> Result<(), Box<dyn Error>>;
    fn to_vec(&self, project: &Project) -> Result<Vec<u8>, Box<dyn Error>>;
}

pub use json::Json;
pub use msgpack::Msgpack;
pub use report::Report;

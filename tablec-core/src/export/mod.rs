use std::error::Error;
use crate::core::project::project::Project;

pub mod json;
pub mod msgpack;
// pub mod protobuf;

/// Unified export format trait
pub trait Format {
    fn export(&self, project: &Project, output: &str) -> Result<(), Box<dyn Error>>;
    fn to_vec(&self, project: &Project) -> Result<Vec<u8>, Box<dyn Error>>;
}

pub use json::Json;
pub use msgpack::Msgpack;
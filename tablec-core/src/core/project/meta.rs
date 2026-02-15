use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub version: String,
    pub build_at: i64,
    pub hash: i64,
    pub build_time: String,
    pub source_hash: String,
}
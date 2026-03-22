use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub build: Option<BuildConfig>,
    pub output: Option<OutputConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub input_dir: Option<String>,
    pub output_dir: Option<String>,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: Option<String>,
    pub pretty: Option<bool>,
    pub include_fields: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            project: ProjectConfig {
                name: "default".to_string(),
                version: None,
                description: None,
                author: None,
            },
            build: None,
            output: None,
        }
    }
}

impl Config {
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_from_current_dir() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let paths = vec!["tablec.toml", ".tablec.toml"];
        
        for path_str in paths {
            let path = Path::new(path_str);
            if path.exists() {
                return Ok(Some(Self::load_from_file(path)?));
            }
        }
        
        Ok(None)
    }
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub data: DataConfig,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    pub input_dir: String,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub format: String,
    pub output_dir: String,
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
            },
            data: DataConfig {
                input_dir: "data".to_string(),
                include: Some(vec!["*.xlsx".to_string()]),
                exclude: None,
            },
            export: ExportConfig {
                format: "json".to_string(),
                output_dir: "output".to_string(),
                pretty: Some(true),
                include_fields: Some(false),
            },
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&Path>) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        // If a specific config path is provided, use it directly
        if let Some(path) = config_path {
            return Ok(Some(Self::load_from_file(path)?));
        }

        // Otherwise, search in current directory
        Self::load_from_current_dir()
    }

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

/// Find Excel files matching the given patterns in the input directory
pub fn find_excel_files(
    input_dir: &str,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    let dir_path = Path::new(input_dir);

    if !dir_path.exists() {
        return Ok(files);
    }

    // Default include pattern if none specified
    let include_patterns = if include.is_empty() {
        vec!["*.xlsx".to_string()]
    } else {
        include.to_vec()
    };

    for pattern in &include_patterns {
        // Handle glob patterns
        let full_pattern = if pattern.starts_with("**/") {
            format!("{}{}", input_dir, &pattern[2..])
        } else if pattern.contains('*') {
            format!("{}/{}", input_dir, pattern)
        } else {
            format!("{}/{}", input_dir, pattern)
        };

        for entry in glob::glob(&full_pattern)? {
            match entry {
                Ok(path) => {
                    // Check exclude patterns
                    let should_exclude = exclude.iter().any(|excl| {
                        let excl_pattern = if excl.starts_with("**/") {
                            format!("{}{}", input_dir, &excl[2..])
                        } else {
                            format!("{}/{}", input_dir, excl)
                        };
                        if let Ok(mut matches) = glob::glob(&excl_pattern) {
                            matches.any(|m| m.map(|p| p == path).unwrap_or(false))
                        } else {
                            false
                        }
                    });

                    if !should_exclude {
                        // Verify it's an Excel file
                        if let Some(ext) = path.extension() {
                            if ext == "xlsx" || ext == "xls" {
                                files.push(path);
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Error reading glob entry: {}", e),
            }
        }
    }

    // Sort and deduplicate
    files.sort();
    files.dedup();

    Ok(files)
}

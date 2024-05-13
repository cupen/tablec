use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub supported_formats: Vec<String>,
}

#[derive(Debug)]
pub enum PluginError {
    PluginNotFound(String),
    InvalidPluginConfig(String),
    PluginExecutionFailed(String),
    UnsupportedFormat(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PluginError::PluginNotFound(name) => write!(f, "Plugin '{}' not found", name),
            PluginError::InvalidPluginConfig(msg) => write!(f, "Invalid plugin configuration: {}", msg),
            PluginError::PluginExecutionFailed(msg) => write!(f, "Plugin execution failed: {}", msg),
            PluginError::UnsupportedFormat(format) => write!(f, "Unsupported format: {}", format),
        }
    }
}

impl Error for PluginError {}

pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn process_data(&self, input: &str, config: &serde_json::Value) -> Result<String, Box<dyn Error>>;
    fn supports_format(&self, format: &str) -> bool;
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: HashMap::new(),
        }
    }

    pub fn register_plugin(&mut self, name: String, plugin: Box<dyn Plugin>) {
        self.plugins.insert(name, plugin);
    }

    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.values()
            .map(|p| p.metadata())
            .collect()
    }

    pub fn execute_plugin(&self, name: &str, input: &str, config: &serde_json::Value) -> Result<String, PluginError> {
        let plugin = self.plugins.get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        plugin.process_data(input, config)
            .map_err(|e| PluginError::PluginExecutionFailed(e.to_string()))
    }

    pub fn supports_format(&self, plugin_name: &str, format: &str) -> Result<bool, PluginError> {
        let plugin = self.plugins.get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;
        Ok(plugin.supports_format(format))
    }
}

// Built-in plugins
pub struct JsonFormatterPlugin;
impl Plugin for JsonFormatterPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "json_formatter".to_string(),
            version: "1.0.0".to_string(),
            description: "Formats data as pretty JSON".to_string(),
            author: "tablec".to_string(),
            supported_formats: vec!["json".to_string()],
        }
    }

    fn process_data(
        &self, input: &str, _config: &serde_json::Value,
    ) -> Result<String, Box<dyn Error>> {
        let parsed: serde_json::Value = serde_json::from_str(input)?;
        Ok(serde_json::to_string_pretty(&parsed)?)
    }

    fn supports_format(&self, format: &str) -> bool {
        format == "json"
    }
}

pub struct DataValidatorPlugin;
impl Plugin for DataValidatorPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "data_validator".to_string(),
            version: "1.0.0".to_string(),
            description: "Validates table data against schema rules".to_string(),
            author: "tablec".to_string(),
            supported_formats: vec!["json".to_string(), "csv".to_string(), "xlsx".to_string()],
        }
    }

    fn process_data(
        &self, input: &str, config: &serde_json::Value,
    ) -> Result<String, Box<dyn Error>> {
        let strict_mode = config.get("strict")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        
        let parsed: serde_json::Value = serde_json::from_str(input)?;
        let mut validation_result = serde_json::json!({
            "valid": true,
            "errors": [],
            "warnings": [],
            "strict_mode": strict_mode
        });
        
        if let Some(array) = parsed.as_array() {
            for (i, item) in array.iter().enumerate() {
                if item.get("id").is_none() {
                    validation_result["errors"].as_array_mut()
                        .unwrap()
                        .push(serde_json::Value::String(format!("Missing 'id' field in item {}", i)));
                    validation_result["valid"] = serde_json::Value::Bool(false);
                }
            }
        }
        
        Ok(serde_json::to_string_pretty(&validation_result)?)
    }

    fn supports_format(
        &self,
        format: &str,
    ) -> bool {
        vec!["json", "csv", "xlsx"].contains(&format)
    }
}

pub struct CsvExporterPlugin;
impl Plugin for CsvExporterPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "csv_exporter".to_string(),
            version: "1.0.0".to_string(),
            description: "Exports table data to CSV format".to_string(),
            author: "tablec".to_string(),
            supported_formats: vec!["csv".to_string()],
        }
    }

    fn process_data(
        &self, input: &str, config: &serde_json::Value,
    ) -> Result<String, Box<dyn Error>> {
        let delimiter = config.get("delimiter")
            .and_then(|v| v.as_str())
            .unwrap_or(", ");
            
        let parsed: serde_json::Value = serde_json::from_str(input)?;
        let mut csv_output = Vec::new();
        
        if let Some(array) = parsed.as_array() {
            if let Some(first_item) = array.first() {
                if let Some(obj) = first_item.as_object() {
                    let headers: Vec<String> = obj.keys().cloned().collect();
                    csv_output.push(headers.join(delimiter));
                    
                    for item in array {
                        if let Some(obj) = item.as_object() {
                            let values: Vec<String> = headers.iter()
                                .map(|key| obj.get(key)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string())
                                .collect();
                            csv_output.push(values.join(delimiter));
                        }
                    }
                }
            }
        }
        
        Ok(csv_output.join("\n"))
    }

    fn supports_format(
        &self,
        format: &str,
    ) -> bool {
        format == "csv"
    }
}

pub fn create_default_plugin_manager() -> PluginManager {
    let mut manager = PluginManager::new();
    manager.register_plugin("json_formatter".to_string(), Box::new(JsonFormatterPlugin));
    manager.register_plugin("data_validator".to_string(), Box::new(DataValidatorPlugin));
    manager.register_plugin("csv_exporter".to_string(), Box::new(CsvExporterPlugin));
    manager
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration() {
        let mut manager = create_default_plugin_manager();
        assert_eq!(manager.list_plugins().len(), 3);
    }

    #[test]
    fn test_json_formatter_plugin() {
        let manager = create_default_plugin_manager();
        let plugin = manager.get_plugin("json_formatter").unwrap();
        assert!(plugin.supports_format("json"));
        assert!(!plugin.supports_format("csv"));
    }

    #[test]
    fn test_plugin_execution() {
        let manager = create_default_plugin_manager();
        let input = r#"{"name": "test", "value": 42}"#;
        let config = serde_json::json!({});
        let result = manager.execute_plugin("json_formatter", input, &config);
        assert!(result.is_ok());
    }
}
*/

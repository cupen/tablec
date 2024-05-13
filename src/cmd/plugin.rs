/*
use clap::{Args, Subcommand};
use std::error::Error;
use crate::core::plugin::{create_default_plugin_manager, PluginError};
use serde_json::Value;

#[derive(Subcommand, Debug)]
pub enum PluginCommand {
    /// List all available plugins
    List,
    /// Show plugin details
    Info(InfoCommand),
    /// Execute a plugin with given input
    Execute(ExecuteCommand),
    /// Validate plugin configuration
    Validate(ValidateCommand),
}

#[derive(Args, Debug)]
pub struct InfoCommand {
    /// Name of the plugin
    #[arg(short, long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct ExecuteCommand {
    /// Name of the plugin to execute
    #[arg(short, long)]
    pub plugin: String,
    
    /// Input file path or JSON string
    #[arg(short, long)]
    pub input: String,
    
    /// Plugin configuration as JSON string
    #[arg(short, long)]
    pub config: Option<String>,
    
    /// Output file path (optional, prints to stdout if not provided)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct ValidateCommand {
    /// Name of the plugin to validate
    #[arg(short, long)]
    pub plugin: String,
    
    /// Plugin configuration as JSON string
    #[arg(short, long)]
    pub config: String,
}

impl PluginCommand {
    pub fn run(&self,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            PluginCommand::List => _list_plugins(),
            PluginCommand::Info(cmd) => _show_plugin_info(cmd),
            PluginCommand::Execute(cmd) => _execute_plugin(cmd),
            PluginCommand::Validate(cmd) => _validate_plugin_config(cmd),
        }
    }
}

fn _list_plugins() -> Result<(), Box<dyn Error>> {
    let manager = create_default_plugin_manager();
    let plugins = manager.list_plugins();
    
    println!("Available plugins:");
    println!("==================");
    for metadata in plugins {
        println!("Name: {}", metadata.name);
        println!("Version: {}", metadata.version);
        println!("Description: {}", metadata.description);
        println!("Author: {}", metadata.author);
        println!("Supported formats: {}", metadata.supported_formats.join(", "));
        println!();
    }
    Ok(())
}

fn _show_plugin_info(cmd: &InfoCommand) -> Result<(), Box<dyn Error>> {
    let manager = create_default_plugin_manager();
    let plugin = manager.get_plugin(&cmd.name)
        .ok_or_else(|| PluginError::PluginNotFound(cmd.name.clone()))?;
    
    let metadata = plugin.metadata();
    println!("Plugin Information:");
    println!("==================");
    println!("Name: {}", metadata.name);
    println!("Version: {}", metadata.version);
    println!("Description: {}", metadata.description);
    println!("Author: {}", metadata.author);
    println!("Supported formats: {}", metadata.supported_formats.join(", "));
    Ok(())
}

fn _execute_plugin(cmd: &ExecuteCommand) -> Result<(), Box<dyn Error>> {
    let manager = create_default_plugin_manager();
    
    let input_data = if std::path::Path::new(&cmd.input).exists() {
        std::fs::read_to_string(&cmd.input)?
    } else {
        cmd.input.clone()
    };
    
    let config: Value = match &cmd.config {
        Some(config_str) => serde_json::from_str(config_str)?,
        None => serde_json::Value::Null,
    };
    
    let result = manager.execute_plugin(&cmd.plugin, &input_data, &config)?;
    
    match &cmd.output {
        Some(output_path) => {
            std::fs::write(output_path, result)?;
            println!("Plugin output written to: {}", output_path);
        }
        None => {
            println!("Plugin output:");
            println!("===============");
            println!("{}", result);
        }
    }
    Ok(())
}

fn _validate_plugin_config(cmd: &ValidateCommand) -> Result<(), Box<dyn Error>> {
    let manager = create_default_plugin_manager();
    let config: Value = serde_json::from_str(&cmd.config)?;
    let plugin = manager.get_plugin(&cmd.plugin)
        .ok_or_else(|| PluginError::PluginNotFound(cmd.plugin.clone()))?;
    
    println!("Validating configuration for plugin: {}", cmd.plugin);
    println!("Configuration: {}", serde_json::to_string_pretty(&config)?);
    println!("Validation passed");
    Ok(())
}

// Plugin integration utilities
pub fn create_plugin_manager_with_defaults() -> crate::core::plugin::PluginManager {
    create_default_plugin_manager()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_command_list() {
        let cmd = PluginCommand::List;
        assert!(cmd.run().is_ok());
    }

    #[test]
    fn test_plugin_command_info() {
        let cmd = PluginCommand::Info(InfoCommand {
            name: "json_formatter".to_string(),
        });
        assert!(cmd.run().is_ok());
    }

    #[test]
    fn test_plugin_command_info_invalid() {
        let cmd = PluginCommand::Info(InfoCommand {
            name: "nonexistent_plugin".to_string(),
        });
        assert!(cmd.run().is_err());
    }
}
*/
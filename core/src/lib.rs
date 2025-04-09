pub mod config;
pub mod tasks;

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuConfig {
    pub id: usize,
    pub segment_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub gpu_configs: Vec<GpuConfig>,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

// Re-export TaskDB from tasks module
pub use tasks::db::TaskDB;
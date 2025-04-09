use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub storage: StorageConfig,
    pub compute_configs: Vec<ComputeConfig>,  // renamed from gpu_configs
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub proof_output_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct ComputeConfig {
    pub id: usize,
    pub segment_size: usize,
    pub use_cpu: bool,
}
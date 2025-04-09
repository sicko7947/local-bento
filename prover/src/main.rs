use anyhow::Result;
use sqlx::PgPool;
use core::{Config, TaskDB};
use core::tasks::{ProofTask, TaskStatus};
use crate::gpu::GpuCoordinator;
use uuid::Uuid;
use risc0_zkvm::{compute_segment_count, ExecutorEnv, ProverOpts};
use std::path::PathBuf;

mod gpu;
mod service;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load config
    let config = Config::load("config/config.yaml")?;
    
    // Setup database
    let pool = PgPool::connect(&config.database.url).await?;
    
    // Create TaskDB instance
    let task_db = TaskDB::new(pool);

    // Initialize coordinator with CPU configuration
    let mut coordinator = GpuCoordinator::new(config, task_db);
    
    // Start processing
    coordinator.start().await?;

    Ok(())
}
use anyhow::Result;
use sqlx::PgPool;
use core::{Config, TaskDB};
use core::tasks::{ProofTask, TaskStatus};
use uuid::Uuid;
use time::OffsetDateTime;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = Config::load("../../config/config.yaml")?;
    
    // Connect to database
    let pool = PgPool::connect(&config.database.url).await?;
    let task_db = TaskDB::new(pool);

    // Create a new task
    let test_task = ProofTask {
        id: Uuid::new_v4(),
        status: TaskStatus::Pending,
        image_id: "../../guest/target/release/guest".to_string(),
        input_data: Some(vec![1, 2, 3, 4]),
        segment_count: 2,
        segment_size: 1 << 19,
        created_at: OffsetDateTime::now_utc(),
    };

    // Submit the task
    task_db.create_task(&test_task).await?;
    println!("Submitted task with ID: {}", test_task.id);

    Ok(())
}
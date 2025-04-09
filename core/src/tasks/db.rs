use sqlx::{Pool, Postgres};
use anyhow::Result;
use uuid::Uuid;
use super::types::{ProofTask, TaskStatus};

pub struct TaskDB {
    pool: Pool<Postgres>,
}

impl TaskDB {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn create_pool(database_url: &str) -> Result<Pool<Postgres>> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        
        sqlx::migrate!("../migrations")  // Updated path
            .run(&pool)
            .await?;
        
        Ok(pool)
    }

    pub async fn create_task(&self, task: &ProofTask) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO proof_tasks (id, status, image_id, input_data, segment_count, segment_size)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            task.id,
            task.status as _,
            task.image_id,
            task.input_data,
            task.segment_count,
            task.segment_size,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<Option<ProofTask>> {
        let task = sqlx::query_as!(
            ProofTask,
            r#"
            SELECT 
                id, status as "status: TaskStatus", image_id, 
                input_data, segment_count, segment_size, created_at
            FROM proof_tasks 
            WHERE id = $1
            "#,
            task_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(task)
    }
}
use serde::{Deserialize, Serialize};
use sqlx::Type;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofTask {
    pub id: Uuid,
    pub status: TaskStatus,
    pub image_id: String,
    pub input_data: Option<Vec<u8>>,
    pub segment_count: i32,
    pub segment_size: i32,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofSegment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub segment_index: i32,
    pub gpu_id: Option<i32>,
    pub status: TaskStatus,
    pub proof: Option<Vec<u8>>,
}
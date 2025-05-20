pub mod gen {
    // Include the generated gRPC code
    pub mod bento {
        pub mod v1 {
            tonic::include_proto!("bento.v1");
        }
    }
}

mod client;

// Re-exports
pub use client::BentoClient;

// Export the key types from the generated code
pub use gen::bento::v1::{
    bento_task_service_client, // Add this
    ArtifactInfo, ArtifactType, CompressionType, DownloadArtifactRequest, DownloadArtifactResponse,
    ExecutorTaskDefinition, FinalizeTaskDefinition, GetTaskRequest, GetTaskResponse, 
    ProveReceiptType, ProveTaskDefinition, RollupType, SnarkTaskDefinition, Task, TaskStatus, 
    TaskType, UpdateTaskStatusRequest, UpdateTaskStatusResponse, UploadArtifactRequest,
    UploadArtifactResponse,
};

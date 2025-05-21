use anyhow::Result;
use tonic::transport::{Channel, Endpoint};

use crate::bento::v1::{
    bento_task_service_client::BentoTaskServiceClient,
    ArtifactInfo, DownloadArtifactRequest, GetTaskRequest, UpdateTaskStatusRequest,
    UploadArtifactRequest,
};

/// Client for the Bento Task Service
pub struct BentoTaskClient {
    client: BentoTaskServiceClient<Channel>,
}

impl BentoTaskClient {
    /// Create a new BentoTaskClient connected to the specified endpoint
    pub async fn connect(endpoint: &str) -> Result<Self> {
        let channel = Endpoint::from_shared(endpoint.to_string())?
            .connect()
            .await?;
        
        let client = BentoTaskServiceClient::new(channel);
        
        Ok(Self { client })
    }
    
    /// Get a task from the service
    pub async fn get_task(&mut self, worker_id: &str, capabilities: Vec<String>) -> Result<Option<crate::bento::v1::Task>> {
        let request = GetTaskRequest {
            worker_id: worker_id.to_string(),
            worker_capabilities: capabilities,
        };
        
        let response = self.client.get_task(request).await?;
        Ok(response.into_inner().task)
    }
    
    /// Update task status
    pub async fn update_task_status(&mut self, request: UpdateTaskStatusRequest) -> Result<bool> {
        let response = self.client.update_task_status(request).await?;
        Ok(response.into_inner().acknowledged)
    }
    
    // Additional methods for other RPC calls can be implemented here
}

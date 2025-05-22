use anyhow::Result;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Streaming}; 

use crate::bento::v1::{
    bento_service_client::BentoServiceClient, 
    RequestTaskRequest, 
    TaskAssignment, 
    UpdateTaskProgressRequest,
    UpdateTaskProgressResponse,
    UploadGroth16ResultRequest, 
    UploadGroth16ResultResponse, 
    UploadStarkResultRequest, 
    UploadStarkResultResponse
};

/// Client for the Bento Task Service
#[derive(Clone)]
pub struct BentoClient {
    channel: Channel,
}

impl BentoClient {

    pub async fn new(endpoint: impl Into<String>) -> Result<Self> {
        let endpoint_str = endpoint.into();
        // Ensure the endpoint has a scheme, defaulting to http if not present.
        let endpoint_str = if !endpoint_str.starts_with("http://") && !endpoint_str.starts_with("https://") {
            format!("http://{}", endpoint_str)
        } else {
            endpoint_str
        };
        let channel = Endpoint::from_shared(endpoint_str)?
            // Customize the channel with options if needed
            // .buffer_size(sz)
            // .concurrency_limit(limit)
            // .http2_adaptive_window(enabled) 
            .connect()
            .await?;
        Ok(Self { channel })
    }

    /// Requests a task from the server.
    /// The server returns a stream of task assignments.
    pub async fn request_task(
        &self,
        request: RequestTaskRequest,
    ) -> Result<Streaming<TaskAssignment>> {
        let mut client = BentoServiceClient::new(self.channel.clone());
        let response = client.request_task(Request::new(request)).await?;
        Ok(response.into_inner())
    }

    /// Sends a task progress update to the server.
    /// Returns the server's response with any instructions.
    pub async fn update_task_progress(&self, request: UpdateTaskProgressRequest) -> Result<UpdateTaskProgressResponse> {
        let mut client = BentoServiceClient::new(self.channel.clone());
        let response = client.update_task_progress(Request::new(request)).await?;
        Ok(response.into_inner())
    }

    /// Uploads the Groth16 result to the server.
    /// Returns the server's response.
    pub async fn upload_groth16_result(&self, request: UploadGroth16ResultRequest) -> Result<UploadGroth16ResultResponse> {
        let mut client = BentoServiceClient::new(self.channel.clone());
        let response = client.upload_groth16_result(Request::new(request)).await?;
        Ok(response.into_inner())
    }
    
    /// Uploads the Stark result to the server.
    /// Returns the server's response.
    pub async fn upload_stark_result(&self, request: UploadStarkResultRequest) -> Result<UploadStarkResultResponse> {
        let mut client = BentoServiceClient::new(self.channel.clone());
        let response = client.upload_stark_result(Request::new(request)).await?;
        Ok(response.into_inner())
    }
}

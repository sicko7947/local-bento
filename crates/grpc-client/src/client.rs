use anyhow::Result;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Streaming}; // 仍然需要Streaming用于RequestTask

use crate::bento::v1::{
    bento_service_client::BentoServiceClient, 
    RequestTaskRequest, 
    TaskAssignment, 
    UpdateTaskProgressRequest,
    UpdateTaskProgressResponse, // 添加一元响应类型
    UploadGroth16ResultRequest, 
    UploadGroth16ResultResponse, 
    UploadStarkResultRequest, 
    UploadStarkResultResponse
};

/// Client for the Bento Task Service
pub struct BentoClient {
    client: BentoServiceClient<Channel>,
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
            .connect()
            .await?;
        let client = BentoServiceClient::new(channel);
        Ok(Self { client })
    }

    /// Requests a task from the server.
    /// The server returns a stream of task assignments.
    pub async fn request_task(
        &mut self,
        request: RequestTaskRequest,
    ) -> Result<Streaming<TaskAssignment>> {
        let response = self.client.request_task(Request::new(request)).await?;
        Ok(response.into_inner())
    }

    /// Sends a task progress update to the server.
    /// Returns the server's response with any instructions.
    pub async fn update_task_progress( &mut self,request: UpdateTaskProgressRequest) -> Result<UpdateTaskProgressResponse> {
        let response = self.client.update_task_progress(Request::new(request)).await?;
        Ok(response.into_inner())
    }

    pub async fn upload_groth16_result(&mut self, request: UploadGroth16ResultRequest) -> Result<UploadGroth16ResultResponse> {
        let response = self.client.upload_groth16_result(Request::new(request)).await?;
        Ok(response.into_inner())
    }
    
    pub async fn upload_stark_result(&mut self, request: UploadStarkResultRequest) -> Result<UploadStarkResultResponse> {
        let response = self.client.upload_stark_result(Request::new(request)).await?;
        Ok(response.into_inner())
    }
}

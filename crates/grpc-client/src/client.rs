use anyhow::Result;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Streaming}; // Added Request and Streaming
use futures_core::Stream; // Added for stream arguments

use crate::bento::v1::{
    bento_service_client::BentoServiceClient, // Corrected import
    RequestTaskRequest, 
    TaskAssignment, 
    UpdateTaskProgressRequest,
    ServerInstruction, // Added for StreamTaskUpdates
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

    /// Establishes a bi-directional stream for task progress updates and server instructions.
    /// The client sends a stream of `UpdateTaskProgressRequest`.
    /// The server returns a stream of `ServerInstruction`.
    pub async fn stream_task_updates(
        &mut self,
        request_stream: impl Stream<Item = UpdateTaskProgressRequest> + Send + Sync + 'static,
    ) -> Result<Streaming<ServerInstruction>> {
        let response = self
            .client
            .stream_task_updates(Request::new(request_stream))
            .await?;
        Ok(response.into_inner())
    }

    /// Uploads a STARK proof result to the server using a client stream.
    /// The client sends a stream of `UploadStarkResultRequest` (metadata then data chunks).
    /// The server returns a single `UploadStarkResultResponse`.
    pub async fn upload_stark_result(
        &mut self,
        request_stream: impl Stream<Item = UploadStarkResultRequest> + Send + Sync + 'static,
    ) -> Result<UploadStarkResultResponse> {
        let response = self
            .client
            .upload_stark_result(Request::new(request_stream))
            .await?;
        Ok(response.into_inner())
    }

    /// Uploads a Groth16 proof result to the server using a client stream.
    /// The client sends a stream of `UploadGroth16ResultRequest` (metadata then data chunks).
    /// The server returns a single `UploadGroth16ResultResponse`.
    pub async fn upload_groth16_result(
        &mut self,
        request_stream: impl Stream<Item = UploadGroth16ResultRequest> + Send + Sync + 'static,
    ) -> Result<UploadGroth16ResultResponse> {
        let response = self
            .client
            .upload_groth16_result(Request::new(request_stream))
            .await?;
        Ok(response.into_inner())
    }
}

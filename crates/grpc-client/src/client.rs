use std::{path::Path, time::Duration};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::{Stream, StreamExt, TryStreamExt};
use thiserror::Error;
use tokio::{fs::File, io::{AsyncRead, AsyncReadExt, AsyncWriteExt}};
use tonic::{
    transport::{Channel, Endpoint, Uri},
    Request, Status, Streaming,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::gen::bento::v1::{
    bento_task_service_client::BentoTaskServiceClient, ArtifactInfo, ArtifactType,
    DownloadArtifactRequest, DownloadArtifactResponse, GetTaskRequest, GetTaskResponse,
    Task, TaskStatus, UpdateTaskStatusRequest, UpdateTaskStatusResponse,
    UploadArtifactRequest, UploadArtifactResponse, UploadArtifactRequestChunk, // Add this
    DownloadArtifactResponseChunk, // Add this
};

// Client-specific errors
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("gRPC error: {0}")]
    GrpcError(#[from] tonic::Status),
    
    #[error("Transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Task not available")]
    TaskNotAvailable,
    
    #[error("No task in response")]
    NoTaskInResponse,
    
    #[error("Artifact error: {0}")]
    ArtifactError(String),
    
    #[error("Invalid artifact info: {0}")]
    InvalidArtifactInfo(String),
    
    #[error("Timeout")]
    Timeout,
}

// Main client for interacting with the Bento gRPC service
pub struct BentoClient {
    client: BentoTaskServiceClient<Channel>,
    worker_id: String,
    worker_capabilities: Vec<String>,
}

impl BentoClient {
    /// Create a new client with the given gRPC server address
    pub async fn new(
        server_addr: &str,
        worker_id: Option<String>,
        worker_capabilities: Vec<String>,
    ) -> Result<Self, ClientError> {
        let uri = format!("http://{}", server_addr).parse::<Uri>()
            .map_err(|e| anyhow!("Invalid URI: {}", e))?;
        
        let endpoint = Endpoint::from(uri)
            .timeout(Duration::from_secs(30))
            .tcp_keepalive(Some(Duration::from_secs(60)));
        
        let channel = endpoint.connect().await?;
        let client = BentoTaskServiceClient::new(channel);
        
        // Generate a worker ID if none was provided
        let worker_id = worker_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        Ok(Self {
            client,
            worker_id,
            worker_capabilities,
        })
    }
    
    /// Get the worker ID for this client
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }
    
    /// Get a task from the server based on this client\'s worker capabilities
    pub async fn get_task(&mut self) -> Result<Task, ClientError> {
        debug!("Requesting task for worker {} with capabilities {:?}", self.worker_id, self.worker_capabilities);
        
        let request = Request::new(GetTaskRequest {
            worker_id: self.worker_id.clone(),
            worker_capabilities: self.worker_capabilities.clone(),
        });
        
        match self.client.get_task(request).await {
            Ok(response) => {
                let inner_response = response.into_inner();
                match inner_response.task {
                    Some(task) => {
                        info!("Received task: {:?}", task);
                        Ok(task)
                    }
                    None => {
                        warn!("No task available for worker {}", self.worker_id);
                        Err(ClientError::TaskNotAvailable)
                    }
                }
            }
            Err(status) => {
                error!("Error getting task: {}", status);
                Err(ClientError::GrpcError(status))
            }
        }
    }

    /// Update the status of a task
    pub async fn update_task_status(
        &mut self,
        task_id: String,
        job_id: String, // Added job_id as per proto
        status: TaskStatus,
        output: Option<prost_types::Any>, // Changed to Any as per proto
    ) -> Result<UpdateTaskStatusResponse, ClientError> {
        debug!("Updating task {} for job {} to status {:?}", task_id, job_id, status);
        
        let request = Request::new(UpdateTaskStatusRequest {
            worker_id: self.worker_id.clone(),
            task_id,
            job_id, // Added job_id
            status: status.into(),
            output, // Changed to Any
        });
        
        match self.client.update_task_status(request).await {
            Ok(response) => {
                info!("Successfully updated task status");
                Ok(response.into_inner())
            }
            Err(status) => {
                error!("Error updating task status: {}", status);
                Err(ClientError::GrpcError(status))
            }
        }
    }

    /// Upload an artifact to the server
    pub async fn upload_artifact(
        &mut self,
        artifact_info: ArtifactInfo,
        file_path: impl AsRef<Path>,
    ) -> Result<UploadArtifactResponse, ClientError> {
        info!("Uploading artifact: {:?} from file: {:?}", artifact_info, file_path.as_ref());

        let file = File::open(file_path.as_ref()).await.map_err(ClientError::IoError)?;
        let mut reader = tokio::io::BufReader::new(file);
        let mut buffer = vec![0; 1024 * 1024]; // 1MB chunks

        let stream = async_stream::stream! {
            // First message is ArtifactInfo
            yield UploadArtifactRequest {
                data: Some(crate::gen::bento::v1::upload_artifact_request::Data::Info(artifact_info.clone())),
            };

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        yield UploadArtifactRequest {
                            data: Some(crate::gen::bento::v1::upload_artifact_request::Data::Chunk(
                                UploadArtifactRequestChunk {
                                    data: Bytes::copy_from_slice(&buffer[..n]),
                                }
                            )),
                        };
                    }
                    Err(e) => {
                        error!("Error reading artifact file: {}", e);
                        // How to signal error in stream? For now, just break.
                        break;
                    }
                }
            }
        };

        match self.client.upload_artifact(Request::new(stream)).await {
            Ok(response) => {
                info!("Successfully uploaded artifact");
                Ok(response.into_inner())
            }
            Err(status) => {
                error!("Error uploading artifact: {}", status);
                Err(ClientError::GrpcError(status))
            }
        }
    }

    /// Download an artifact from the server
    pub async fn download_artifact(
        &mut self,
        artifact_id: String,
        artifact_type: ArtifactType,
        task_id: String, // Added task_id
        job_id: String, // Added job_id
        output_path: impl AsRef<Path>,
    ) -> Result<(), ClientError> {
        info!("Downloading artifact {} ({:?}) for task {} (job {}) to file: {:?}", artifact_id, artifact_type, task_id, job_id, output_path.as_ref());

        let request = Request::new(DownloadArtifactRequest {
            artifact_id,
            artifact_type: artifact_type.into(),
            task_id,
            job_id,
        });

        let mut stream: Streaming<DownloadArtifactResponse> = self.client.download_artifact(request).await?.into_inner();

        let mut file = File::create(output_path.as_ref()).await.map_err(ClientError::IoError)?;
        let mut artifact_info_received = false;

        while let Some(response_part_res) = stream.next().await {
            match response_part_res {
                Ok(response_part) => {
                    match response_part.data {
                        Some(crate::gen::bento::v1::download_artifact_response::Data::Info(info)) => {
                            debug!("Received artifact info: {:?}", info);
                            // You might want to validate this info or use it somehow
                            artifact_info_received = true;
                        }
                        Some(crate::gen::bento::v1::download_artifact_response::Data::Chunk(chunk)) => {
                            if !artifact_info_received {
                                warn!("Received chunk before artifact info, this might indicate an issue.");
                                // Optionally, you could error out here if strict ordering is required.
                                // For now, we'll proceed but this is worth noting.
                            }
                            file.write_all(&chunk.data).await.map_err(ClientError::IoError)?;
                        }
                        None => {
                            warn!("Received empty data part in download stream.");
                        }
                    }
                }
                Err(status) => {
                    error!("Error downloading artifact: {}", status);
                    return Err(ClientError::GrpcError(status));
                }
            }
        }
        file.sync_all().await.map_err(ClientError::IoError)?; // Ensure all data is written to disk
        info!("Successfully downloaded artifact");
        Ok(())
    }
}

use grpc_client::bento::v1::{
    RequestTaskRequest,
    UpdateTaskProgressRequest,
    UploadStarkResultRequest,
    UploadGroth16ResultRequest,
    TaskStatus,
    task_assignment::TaskDetails,
};
use grpc_client::BentoClient;


#[tokio::test]
async fn test_client_creation() {
    let endpoint = "http://127.0.0.1:50051";
    
    // Test creating client with explicit http scheme
    let client = BentoClient::new(endpoint).await;
    assert!(client.is_ok(), "Failed to create client with explicit scheme");

    // Test creating client without scheme (should default to http)
    let client = BentoClient::new("127.0.0.1:50051").await;
    assert!(client.is_ok(), "Failed to create client without scheme");

    // Test creating client with https scheme
    let client = BentoClient::new("https://127.0.0.1:50051").await;
    assert!(client.is_ok(), "Failed to create client with https scheme");
}

#[tokio::test]
async fn test_request_task() {
    let server_addr = "http://127.0.0.1:50051";
    let client = BentoClient::new(server_addr).await.unwrap();

    let request = RequestTaskRequest {
        gpu_memory: 8192, // 8GB
    };

    let mut stream = client.request_task(request).await.unwrap();

    // Receive first task (STARK)
    let first_task = stream.message().await.unwrap().unwrap();
    assert_eq!(first_task.task_id, "test-stark-task-1");
    assert!(matches!(first_task.task_details, Some(TaskDetails::StarkTask(_))));

    if let Some(TaskDetails::StarkTask(stark_details)) = first_task.task_details {
        assert_eq!(stark_details.image_id, "test-image-id");
        assert_eq!(stark_details.elf_data, b"mock elf data");
        assert!(stark_details.input_data.is_some());
        assert_eq!(stark_details.exec_cycle_limit, 1000000);
    }

    // Receive second task (Groth16)
    let second_task = stream.message().await.unwrap().unwrap();
    assert_eq!(second_task.task_id, "test-groth16-task-1");
    assert!(matches!(second_task.task_details, Some(TaskDetails::Groth16Task(_))));

    if let Some(TaskDetails::Groth16Task(groth16_details)) = second_task.task_details {
        assert_eq!(groth16_details.stark_original_task_id, "test-stark-task-1");
        assert_eq!(groth16_details.stark_receipt_data, b"mock stark receipt");
    }
}

#[tokio::test]
async fn test_update_task_progress() {
    let server_addr = "http://127.0.0.1:50051";
    let client = BentoClient::new(server_addr).await.unwrap();

    // Test different progress updates
    let test_cases = vec![
        (TaskStatus::Pending, "Task assigned, preparing"),
        (TaskStatus::GeneratingProof, "Generating STARK proof"),
        (TaskStatus::UploadingProof, "Uploading proof data"),
        (TaskStatus::Completed, "Task completed successfully"),
        (TaskStatus::Failed, "Task failed: out of memory"),
    ];

    for (status, message) in test_cases {
        let request = UpdateTaskProgressRequest {
            task_id: "test-task-1".to_string(),
            status: status as i32,
            message: message.to_string(),
            total_segments: Some(10),
            total_cycles: Some(1000000),
        };

        let response = client.update_task_progress(request).await;
        assert!(response.is_ok(), "Failed to update progress for status {:?}", status);
    }
}

#[tokio::test]
async fn test_upload_stark_result() {
    let server_addr = "http://127.0.0.1:50051";
    let client = BentoClient::new(server_addr).await.unwrap();

    let request = UploadStarkResultRequest {
        task_id: "test-stark-task-1".to_string(),
        receipt_data: b"mock stark receipt data".to_vec(),
        journal_data: b"mock journal data".to_vec(),
        description: "Test STARK proof upload".to_string(),
    };

    let response = client.upload_stark_result(request).await.unwrap();
    assert!(response.success);
    assert!(response.error_message.is_empty());
}

#[tokio::test]
async fn test_upload_groth16_result() {
    let server_addr = "http://127.0.0.1:50051";
    let client = BentoClient::new(server_addr).await.unwrap();

    let request = UploadGroth16ResultRequest {
        task_id: "test-groth16-task-1".to_string(),
        proof_data: b"mock groth16 proof data".to_vec(),
        description: "Test Groth16 proof upload".to_string(),
    };

    let response = client.upload_groth16_result(request).await.unwrap();
    assert!(response.success);
    assert!(response.error_message.is_empty());
}

#[tokio::test]
async fn test_concurrent_clients() {
    let server_addr = "http://127.0.0.1:50051";
    
    let mut handles = vec![];
    
    for i in 0..5 {
        let addr = server_addr.clone();
        let handle = tokio::spawn(async move {
        let client = BentoClient::new(addr).await.unwrap();
            
            let request = RequestTaskRequest {
                gpu_memory: 4096 + i * 1024,
            };
            
            let mut stream = client.request_task(request).await.unwrap();
            let task = stream.message().await.unwrap().unwrap();
            
            // Update progress
            let progress_request = UpdateTaskProgressRequest {
                task_id: task.task_id.clone(),
                status: TaskStatus::Completed as i32,
                message: format!("Client {} completed", i),
                total_segments: None,
                total_cycles: None,
            };
            
            client.update_task_progress(progress_request).await.unwrap();
            
            task.task_id
        });
        
        handles.push(handle);
    }
    
    // Wait for all clients to complete
    let results = futures::future::join_all(handles).await;
    
    for (i, result) in results.into_iter().enumerate() {
        let task_id = result.unwrap();
        // All should get the same first task
        assert_eq!(task_id, "test-stark-task-1");
    }
}

#[tokio::test]
async fn test_error_handling() {
    // Test connection to non-existent server
    let client = BentoClient::new("http://127.0.0.1:99999").await.unwrap();
    
    let request = RequestTaskRequest {
        gpu_memory: 8192,
    };
    
    let result = client.request_task(request).await;
    assert!(result.is_err(), "Should fail to connect to non-existent server");
}

#[tokio::test]
async fn test_large_data_upload() {
    let server_addr = "http://127.0.0.1:50051";
    let client = BentoClient::new(server_addr).await.unwrap();

    // Test with larger data (simulating real proof data)
    let large_receipt_data = vec![0u8; 1024 * 1024]; // 1MB
    let large_journal_data = vec![1u8; 512 * 1024]; // 512KB

    let request = UploadStarkResultRequest {
        task_id: "test-large-stark-task".to_string(),
        receipt_data: large_receipt_data,
        journal_data: large_journal_data,
        description: "Large STARK proof test".to_string(),
    };

    let response = client.upload_stark_result(request).await.unwrap();
    assert!(response.success);

    // Test large Groth16 proof
    let large_proof_data = vec![2u8; 256 * 1024]; // 256KB

    let request = UploadGroth16ResultRequest {
        task_id: "test-large-groth16-task".to_string(),
        proof_data: large_proof_data,
        description: "Large Groth16 proof test".to_string(),
    };

    let response = client.upload_groth16_result(request).await.unwrap();
    assert!(response.success);
}
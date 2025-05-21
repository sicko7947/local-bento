// Re-export generated code from protobuf
pub mod bento {
    pub mod v1 {
        include!("gen/bento.v1.rs");
        
        // Re-export the service client
        pub use self::bento_task_service_client::BentoTaskServiceClient;
    }
}

// Export client module
mod client;
pub use client::BentoTaskClient;

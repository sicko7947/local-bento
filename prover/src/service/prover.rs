use anyhow::Result;
use core::{Config, TaskDB};
use risc0_zkvm::ProverOpts;
use tokio::sync::mpsc::{self, Sender};
use uuid::Uuid;
use std::sync::Arc;

pub struct ProverService {
    config: Config,
    task_db: TaskDB,
    gpu_id: usize,
    tx: Sender<Uuid>,
}

impl ProverService {
    pub fn new(config: Config, task_db: TaskDB, gpu_id: usize) -> Self {
        let (tx, _) = mpsc::channel(32);
        Self {
            config,
            task_db,
            gpu_id,
            tx,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(32);
        
        let self_clone = Arc::clone(&self);
        let gpu_worker = tokio::spawn(async move {
            while let Some(segment_id) = rx.recv().await {
                self_clone.prove_segment(segment_id).await?;
            }
            Ok::<(), anyhow::Error>(())
        });

        gpu_worker.await??;
        Ok(())
    }

    pub fn get_sender(&self) -> Sender<Uuid> {
        self.tx.clone()
    }

    async fn prove_segment(&self, segment_id: Uuid) -> Result<()> {
        let segment = self.task_db.get_segment(segment_id).await?.ok_or_else(|| {
            anyhow::anyhow!("Segment not found")
        })?;
        
        let opts = ProverOpts::default();
        // GPU selection is handled by CUDA_VISIBLE_DEVICES environment variable

        println!("Processing proof on GPU {} for segment {}", self.gpu_id, segment_id);
        
        Ok(())
    }
}
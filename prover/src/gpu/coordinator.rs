use anyhow::Result;
use core::{Config, TaskDB, tasks::ProofSegment};
use tokio::sync::mpsc;
use crate::gpu::worker::GpuWorker;

pub struct GpuCoordinator {
    config: Config,
    task_db: TaskDB,
    workers: Vec<tokio::task::JoinHandle<Result<()>>>,
}

impl GpuCoordinator {
    pub fn new(config: Config, task_db: TaskDB) -> Self {
        Self {
            config,
            task_db,
            workers: Vec::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        for gpu_config in &self.config.gpu_configs {
            let (tx, rx) = mpsc::channel(32);
            
            let mut worker = GpuWorker::new(
                gpu_config.id,
                gpu_config.segment_size,
                rx,
            );

            let handle = tokio::spawn(async move {
                worker.run().await
            });

            self.workers.push(handle);
        }

        // Wait for all workers
        for worker in self.workers.drain(..) {
            worker.await??;
        }

        Ok(())
    }
}
//! Workflow processing Agent service
use crate::redis::RedisPool;
use anyhow::{bail, Context, Result};
use clap::Parser;
use grpc_client::bento::v1::{
    RequestTaskRequest, StarkTaskDetails, TaskAssignment, UpdateTaskProgressRequest,
    UploadGroth16ResultRequest, UploadStarkResultRequest,
};
use nvml_wrapper::Nvml;
use risc0_zkvm::{compute_image_id, get_prover_server, ProverOpts, ProverServer, VerifierContext};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use taskdb::ReadyTask;
use tokio::time;
use uuid::Uuid;
use workflow_common::s3::{
    ELF_BUCKET_DIR, GROTH16_BUCKET_DIR, INPUT_BUCKET_DIR, RECEIPT_BUCKET_DIR, STARK_BUCKET_DIR,
};
use workflow_common::{TaskType, COPROC_WORK_TYPE};

mod redis;
mod tasks;

pub use workflow_common::{
    s3::S3Client, AUX_WORK_TYPE, EXEC_WORK_TYPE, JOIN_WORK_TYPE, PROVE_WORK_TYPE, SNARK_WORK_TYPE,
};

/// Workflow agent
///
/// Monitors taskdb for new tasks on the selected stream and processes the work.
/// Requires redis / task (psql) access
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// agent stream type to monitor for tasks
    ///
    /// ex: `exec`, `prove`, `join`, `snark`, etc
    #[arg(short, long)]
    pub task_stream: String,

    /// Polling internal between tasks
    ///
    /// Time to wait between request_work calls
    #[arg(short, long, default_value_t = 1)]
    pub poll_time: u64,

    /// taskdb postgres DATABASE_URL
    #[clap(env)]
    pub database_url: String,

    /// redis connection URL
    #[clap(env)]
    pub redis_url: String,

    /// risc0 segment po2 arg
    #[clap(short, long, default_value_t = 20)]
    pub segment_po2: u32,

    /// max connections to SQL db in connection pool
    #[clap(long, default_value_t = 1)]
    pub db_max_connections: u32,

    /// Redis TTL, seconds before objects expire automatically
    ///
    /// Defaults to 8 hours
    #[clap(long, default_value_t = 8 * 60 * 60)]
    pub redis_ttl: u64,

    /// Executor limit, in millions of cycles
    #[clap(short, long, default_value_t = 100_000)]
    pub exec_cycle_limit: u64,

    /// S3 / Minio bucket
    #[clap(env)]
    pub s3_bucket: String,

    /// S3 / Minio access key
    #[clap(env)]
    pub s3_access_key: String,

    /// S3 / Minio secret key
    #[clap(env)]
    pub s3_secret_key: String,

    /// S3 / Minio url
    #[clap(env)]
    pub s3_url: String,

    /// Enables a background thread to monitor for tasks that need to be retried / timed-out
    #[clap(long, default_value_t = false)]
    monitor_requeue: bool,

    // Task flags
    /// How many times a task be running for, before it is marked as timed-out
    #[clap(long, default_value_t = 0)]
    exec_retries: i32,

    /// How long can a task be running for, before it is marked as timed-out
    #[clap(long, default_value_t = 4 * 60 * 60)]
    exec_timeout: i32,

    /// How many times a prove+lift can fail before hard failure
    #[clap(long, default_value_t = 3)]
    prove_retries: i32,

    /// How long can a prove+lift can be running for, before it is marked as timed-out
    #[clap(long, default_value_t = 30)]
    prove_timeout: i32,

    /// How many times a join can fail before hard failure
    #[clap(long, default_value_t = 3)]
    join_retries: i32,

    /// How long can a join can be running for, before it is marked as timed-out
    #[clap(long, default_value_t = 10)]
    join_timeout: i32,

    /// How many times a resolve can fail before hard failure
    #[clap(long, default_value_t = 3)]
    resolve_retries: i32,

    /// How long can a resolve can be running for, before it is marked as timed-out
    #[clap(long, default_value_t = 10)]
    resolve_timeout: i32,

    /// How many times a finalize can fail before hard failure
    #[clap(long, default_value_t = 0)]
    finalize_retries: i32,

    /// How long can a finalize can be running for, before it is marked as timed-out
    ///
    /// NOTE: This value is multiplied by the assumption count
    #[clap(long, default_value_t = 10)]
    finalize_timeout: i32,

    /// Snark timeout in seconds
    #[clap(long, default_value_t = 60 * 4)]
    snark_timeout: i32,

    /// Snark retries
    #[clap(long, default_value_t = 0)]
    snark_retries: i32,

    /// gRPC server endpoint for task polling
    #[clap(env, long, default_value = "http://localhost:50051")]
    pub grpc_endpoint: Option<String>,

    /// Enable polling for gRPC tasks
    #[clap(long, default_value_t = true)]
    pub enable_grpc: bool,

    /// Polling interval between gRPC tasks
    #[arg(short, long, default_value_t = 1)]
    pub grpc_poll_time: u64,
}

/// Core agent context to hold all optional clients / pools and state
pub struct Agent {
    /// Postgresql database connection pool
    pub db_pool: PgPool,
    /// segment po2 config
    pub segment_po2: u32,
    /// redis connection pool
    pub redis_pool: RedisPool,
    /// S3 client
    pub s3_client: S3Client,
    /// all configuration params:
    args: Args,
    /// risc0 Prover server
    prover: Option<Rc<dyn ProverServer>>,
    /// risc0 verifier context
    verifier_ctx: VerifierContext,
    /// gRPC client for task updates
    grpc_client: Option<grpc_client::BentoClient>,
    /// GPU memory limit in MB
    gpu_memory: u64,
}

impl Agent {
    /// Initialize the [Agent] from the [Args] config params
    ///
    /// Starts any connection pools and establishes the agents configs
    pub async fn new(args: Args) -> Result<Self> {
        let gpu_memory = (|| {
            let nvml = Nvml::init()?;
            let device = nvml.device_by_index(0)?;
            let info = device.memory_info()?;
            Ok::<_, nvml_wrapper::error::NvmlError>(info.total / 1024 / 1024) // Convert to MB
        })()
        .unwrap_or_else(|err| {
            tracing::warn!("Failed to get GPU memory info: {}, defaulting to 0", err);
            0
        });
        let db_pool = PgPoolOptions::new()
            .max_connections(args.db_max_connections)
            .connect(&args.database_url)
            .await
            .context("Failed to initialize postgresql pool")?;
        let redis_pool = crate::redis::create_pool(&args.redis_url)?;
        let s3_client = S3Client::from_minio(
            &args.s3_url,
            &args.s3_bucket,
            &args.s3_access_key,
            &args.s3_secret_key,
        )
        .await
        .context("Failed to initialize s3 client / bucket")?;

        let verifier_ctx = VerifierContext::default();
        let prover = if args.task_stream == PROVE_WORK_TYPE
            || args.task_stream == JOIN_WORK_TYPE
            || args.task_stream == COPROC_WORK_TYPE
        {
            let opts = ProverOpts::default();
            let prover = get_prover_server(&opts).context("Failed to initialize prover server")?;
            Some(prover)
        } else {
            None
        };

        let grpc_client = if args.enable_grpc {
            if let Some(endpoint) = &args.grpc_endpoint {
                match grpc_client::BentoClient::new(endpoint.clone()).await {
                    Ok(client) => Some(client),
                    Err(err) => {
                        tracing::warn!("Failed to create gRPC client: {}", err);
                        None
                    }
                }
            } else {
                tracing::warn!("gRPC is enabled but no endpoint provided");
                None
            }
        } else {
            None
        };

        Ok(Self {
            db_pool,
            segment_po2: args.segment_po2,
            redis_pool,
            s3_client,
            args,
            prover,
            verifier_ctx,
            grpc_client,
            gpu_memory,
        })
    }

    /// Create a signal hook to flip a boolean if its triggered
    ///
    /// Allows us to catch SIGTERM and exit any hard loop
    fn create_sig_monitor() -> Result<Arc<AtomicBool>> {
        let term = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
        Ok(term)
    }

    /// Starts the work polling, runs until sig_hook triggers
    ///
    /// This function will poll for work and dispatch to the [Self::process_work] function until
    /// the process is terminated. It also handles retries / failures depending on the
    /// [Self::process_work] result
    pub async fn poll_work(&self) -> Result<()> {
        let term_sig = Self::create_sig_monitor().context("Failed to create signal hook")?;

        // Enables task retry management background thread, good for 1-2 aux workers to run in the
        // cluster
        if self.args.monitor_requeue {
            let term_sig_copy = term_sig.clone();
            let db_pool_copy = self.db_pool.clone();
            tokio::spawn(async move {
                Self::poll_for_requeue(term_sig_copy, db_pool_copy)
                    .await
                    .expect("Requeue failed")
            });
        }

        while !term_sig.load(Ordering::Relaxed) {
            let task = taskdb::request_work(&self.db_pool, &self.args.task_stream)
                .await
                .context("Failed to request_work")?;
            let Some(task) = task else {
                time::sleep(time::Duration::from_secs(self.args.poll_time)).await;
                continue;
            };

            if let Err(err) = self.process_work(&task).await {
                tracing::error!("Failure during task processing: {err:?}");

                if task.max_retries > 0 {
                    if !taskdb::update_task_retry(&self.db_pool, &task.job_id, &task.task_id)
                        .await
                        .context("Failed to update task retries")?
                    {
                        tracing::info!("update_task_retried failed: {}", task.job_id);
                    }
                } else {
                    // Prevent massive errors from being reported to the DB
                    let mut err_str = format!("{err:?}");
                    err_str.truncate(1024);
                    taskdb::update_task_failed(
                        &self.db_pool,
                        &task.job_id,
                        &task.task_id,
                        &err_str,
                    )
                    .await
                    .context("Failed to report task failure")?;
                }
                continue;
            }
        }
        tracing::warn!("Handled SIGTERM, shutting down...");

        Ok(())
    }

    /// Process a task and dispatch based on the task type
    pub async fn process_work(&self, task: &ReadyTask) -> Result<()> {
        let task_type: TaskType = serde_json::from_value(task.task_def.clone())
            .with_context(|| format!("Invalid task_def: {}:{}", task.job_id, task.task_id))?;

        // run the task
        let res = match &task_type {
            TaskType::Executor(req) => serde_json::to_value(
                tasks::executor::executor(self, &task.job_id, &req)
                    .await
                    .context("Executor failed")?,
            )
            .context("Failed to serialize prove response")?,
            TaskType::Prove(req) => serde_json::to_value(
                tasks::prove::prover(self, &task.job_id, &task.task_id, &req)
                    .await
                    .context("Prove failed")?,
            )
            .context("Failed to serialize prove response")?,
            TaskType::Join(req) => serde_json::to_value(
                tasks::join::join(self, &task.job_id, &req)
                    .await
                    .context("Join failed")?,
            )
            .context("Failed to serialize join response")?,
            TaskType::Resolve(req) => serde_json::to_value(
                tasks::resolve::resolver(self, &task.job_id, &req)
                    .await
                    .context("Resolve failed")?,
            )
            .context("Failed to serialize join response")?,
            TaskType::Finalize(req) => serde_json::to_value(
                tasks::finalize::finalize(self, &task.job_id, &req)
                    .await
                    .context("Finalize failed")?,
            )
            .context("Failed to serialize finalize response")?,
            TaskType::Snark(req) => serde_json::to_value(
                tasks::snark::stark2snark(self, &task.job_id.to_string(), &req)
                    .await
                    .context("Snark failed")?,
            )
            .context("failed to serialize snark response")?,
            TaskType::Keccak(req) => serde_json::to_value(
                tasks::keccak::keccak(self, &task.job_id, &task.task_id, &req)
                    .await
                    .context("Keccak failed")?,
            )
            .context("failed to serialize keccak response")?,
            TaskType::Union(req) => serde_json::to_value(
                tasks::union::union(self, &task.job_id, &req)
                    .await
                    .context("Union failed")?,
            )
            .context("failed to serialize union response")?,
        };

        taskdb::update_task_done(&self.db_pool, &task.job_id, &task.task_id, res.clone())
            .await
            .context("Failed to report task done")?;

        self.update_grpc_task_status_if_exists(&task.job_id, &task_type, &res)
            .await?;

        Ok(())
    }

    /// background task to poll for jobs that need to be requeued
    ///
    /// Scan the queue looking for tasks that need to be retried and update them
    /// the agent will catch and fail max retries.
    async fn poll_for_requeue(term_sig: Arc<AtomicBool>, db_pool: PgPool) -> Result<()> {
        while !term_sig.load(Ordering::Relaxed) {
            tracing::debug!("Triggering a requeue job...");
            let retry_tasks = taskdb::requeue_tasks(&db_pool, 100).await?;
            if retry_tasks > 0 {
                tracing::info!("Found {retry_tasks} tasks that needed to be retried");
            }
            time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        Ok(())
    }

    /// Starts the work polling, runs until sig_hook triggers
    ///
    /// This function will poll for work and dispatch to the [Self::process_grpc_task] function until
    /// the process is terminated. It also handles retries / failures depending on the
    /// [Self::process_grpc_task] result
    pub async fn poll_grpc_tasks(&self) -> Result<()> {
        let term_sig =
            Self::create_sig_monitor().context("Failed to create signal hook for gRPC client")?;

        let request = RequestTaskRequest {
            gpu_memory: self.gpu_memory,
        };

        tracing::info!("Starting gRPC task polling");

        while !term_sig.load(Ordering::Relaxed) {
            if let Some(client) = &self.grpc_client {
                match client.request_task(request.clone()).await {
                    Ok(mut stream) => {
                        while let Some(task_assignment) =
                            stream.message().await.context("Failed to receive task")?
                        {
                            match self.process_grpc_task(&task_assignment).await {
                                Ok(_) => tracing::info!(
                                    "Successfully processed gRPC task: {}",
                                    task_assignment.task_id
                                ),
                                Err(err) => tracing::error!(
                                    "Failed to process gRPC task: {}: {}",
                                    task_assignment.task_id,
                                    err
                                ),
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!("Failed to request task from gRPC server: {}", err);
                        time::sleep(time::Duration::from_secs(self.args.grpc_poll_time)).await;
                    }
                }
            }

            time::sleep(time::Duration::from_secs(self.args.grpc_poll_time)).await;
        }

        tracing::warn!("gRPC task polling terminated due to signal");
        Ok(())
    }

    async fn process_grpc_task(&self, task: &TaskAssignment) -> Result<()> {
        tracing::info!("Received gRPC task: {}", task.task_id);

        if let Some(client) = &self.grpc_client {
            let update_request = UpdateTaskProgressRequest {
                task_id: task.task_id.clone(),
                status: grpc_client::bento::v1::TaskStatus::Pending as i32,
                message: "Task received".to_string(),
                total_segments: None,
                total_cycles: None,
            };
            client.update_task_progress(update_request).await?;
        }

        match &task.task_details {
            Some(grpc_client::bento::v1::task_assignment::TaskDetails::StarkTask(details)) => {
                self.process_stark_task(task, details).await
            }
            Some(grpc_client::bento::v1::task_assignment::TaskDetails::Groth16Task(details)) => {
                self.process_groth16_task(task, details).await
            }
            None => {
                bail!("Task details not provided");
            }
        }
    }

    async fn process_stark_task(&self, task: &TaskAssignment, details: &StarkTaskDetails) -> Result<()> {

        let image_id = if !details.elf_data.is_empty() {
            // Compute image_id from ELF data and check if it matches details.image_id
            let computed_image_id = compute_image_id(&details.elf_data)
                .context("Failed to compute image id from ELF data")?
                .to_string();
            if computed_image_id != details.image_id {
                return Err(anyhow::anyhow!(
                    "ELF image_id mismatch: expected {}, got {}",
                    details.image_id,
                    computed_image_id
                ));
            }
        
            let key = format!("{ELF_BUCKET_DIR}/{}", details.image_id);
            if self
                .s3_client
                .object_exists(&key)
                .await
                .context("Failed to check if ELF object exists")?
            {
                tracing::info!("ELF object already exists at {}, skipping upload.", key);
            } else {
                self.s3_client
                    .write_buf_to_s3(&key, details.elf_data.clone())
                    .await
                    .with_context(|| {
                        format!("Failed to upload ELF data to S3 for task {}", task.task_id)
                    })?;
            }
            details.image_id.clone()
        } else {
            tracing::warn!("ELF data not provided, using default value");
            return Err(anyhow::anyhow!("ELF data not provided"));
        };

        let input_id = if let Some(input_data) = &details.input_data {
            if input_data.id.is_empty() {
            if !input_data.data.is_empty() {
                // Generate a new UUID for the input id
                let new_id = Uuid::new_v4().to_string();
                let key = format!("{INPUT_BUCKET_DIR}/{}", new_id);
                self.s3_client
                .write_buf_to_s3(&key, input_data.data.clone())
                .await
                .with_context(|| {
                    format!(
                    "Failed to upload input data to S3 for task {} with generated id",
                    task.task_id
                    )
                })?;
                new_id
            } else {
                tracing::warn!("Input id and data are both empty, skipping upload.");
                return Err(anyhow::anyhow!("Input id and data are both empty"));
            }
            } else {
            let key = format!("{INPUT_BUCKET_DIR}/{}", input_data.id);
            if self
                .s3_client
                .object_exists(&key)
                .await
                .context("Failed to check if input object exists")?
            {
                tracing::info!("Input object already exists at {}, skipping upload.", key);
            } else if !input_data.data.is_empty() {
                self.s3_client
                .write_buf_to_s3(&key, input_data.data.clone())
                .await
                .with_context(|| {
                    format!(
                    "Failed to upload input data to S3 for task {} with id {}",
                    task.task_id,
                    input_data.id
                    )
                })?;
            } else {
                tracing::warn!("Input id provided but input data is empty, cannot upload.");
                return Err(anyhow::anyhow!("Input id provided but input data is empty"));
            }
            input_data.id.clone()
            }
        } else {
            tracing::warn!("Input data not provided, using default value");
            return Err(anyhow::anyhow!("Input data not provided"));
        };

        let execute_only = details.execute_only;
        // Use exec_cycle_limit from details if provided, otherwise fallback to self.args.exec_cycle_limit
        let exec_cycle_limit = details.exec_cycle_limit.max(self.args.exec_cycle_limit);

        let mut assumptions = Vec::new();
        for input in &details.assumption_inputs {
            assumptions.push(input.id.clone());
        }

        let req = workflow_common::ExecutorReq {
            image: image_id,
            input: input_id,
            user_id: "grpc".to_string(),
            assumptions,
            execute_only,
            compress: workflow_common::CompressType::None,
            exec_limit: Some(exec_cycle_limit),
        };

        let (
            _aux_stream,
            exec_stream,
            _gpu_prove_stream,
            _gpu_coproc_stream,
            _gpu_join_stream,
            _snark_stream,
        ) = self.get_or_create_streams_for_grpc().await?;

        let task_def = serde_json::to_value(workflow_common::TaskType::Executor(req))
            .context("Failed to serialize ExecutorReq")?;
            
        // Parse task_id to UUID to use as job_id
        let job_id = Uuid::parse_str(&task.task_id)
            .with_context(|| format!("Failed to parse task_id as UUID: {}", task.task_id))?;
            
        taskdb::create_job(
            &self.db_pool,
            &job_id,
            &exec_stream,
            &task_def,
            self.args.exec_retries,
            self.args.exec_timeout,
            "grpc",
        )
        .await
        .context("Failed to create prove task for gRPC request")?;
        
        Ok(())
    }

    async fn process_groth16_task(
        &self,
        task: &grpc_client::bento::v1::TaskAssignment,
        details: &grpc_client::bento::v1::Groth16TaskDetails,
    ) -> Result<()> {
        tracing::info!("Processing Groth16 task: {}", task.task_id);
        
        let job_id = Uuid::parse_str(&task.task_id)
            .with_context(|| format!("Failed to parse task_id as UUID: {}", task.task_id))?;

        let receipt_id = if !details.stark_receipt_data.is_empty() {
            let key = format!("{RECEIPT_BUCKET_DIR}/{STARK_BUCKET_DIR}/{}.bincode", task.task_id);
            if self
                .s3_client
                .object_exists(&key)
                .await
                .context("Failed to check if Stark receipt for Snark object exists")?
            {
                tracing::warn!(
                    "Stark receipt for Snark object already exists at {}, skipping upload.",
                    key
                );
            } else {
                self.s3_client
                    .write_buf_to_s3(&key, details.stark_receipt_data.clone())
                    .await
                    .with_context(|| {
                    format!(
                        "Failed to upload Stark receipt data to S3 for Groth16 task {}",
                        task.task_id
                    )
                })?;
            }
            task.task_id.clone()
        } else {
            task.task_id.clone()
        };

        let (
            _aux_stream,
            _exec_stream,
            _gpu_prove_stream,
            _gpu_coproc_stream,
            _gpu_join_stream,
            snark_stream,
        ) = self.get_or_create_streams_for_grpc().await?;

        let req = workflow_common::SnarkReq {
            receipt: receipt_id, // Use the task_id directly as the receipt_id
            compress_type: workflow_common::CompressType::Groth16,
        };

        let task_def = serde_json::to_value(workflow_common::TaskType::Snark(req))
            .context("Failed to serialize SnarkReq")?;

        taskdb::create_job(
            &self.db_pool,
            &job_id,
            &snark_stream,
            &task_def,
            self.args.snark_retries,
            self.args.snark_timeout,
            "grpc",
        )
        .await
        .context("Failed to create snark task for gRPC request")?;
        
        Ok(())
    }

    async fn get_or_create_streams_for_grpc(
        &self,
    ) -> Result<(
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )> {
        let user_id = "grpc";

        let aux_stream = match taskdb::get_stream(&self.db_pool, user_id, AUX_WORK_TYPE).await? {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, AUX_WORK_TYPE, 1, 1.0, user_id).await?,
        };

        let exec_stream = match taskdb::get_stream(&self.db_pool, user_id, EXEC_WORK_TYPE).await? {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, EXEC_WORK_TYPE, 1, 1.0, user_id).await?,
        };

        let gpu_prove_stream = match taskdb::get_stream(&self.db_pool, user_id, PROVE_WORK_TYPE)
            .await?
        {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, PROVE_WORK_TYPE, 2, 2.0, user_id).await?,
        };

        let gpu_coproc_stream = match taskdb::get_stream(&self.db_pool, user_id, COPROC_WORK_TYPE)
            .await?
        {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, COPROC_WORK_TYPE, 0, 1.0, user_id).await?,
        };

        let gpu_join_stream = match taskdb::get_stream(&self.db_pool, user_id, JOIN_WORK_TYPE)
            .await?
        {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, JOIN_WORK_TYPE, 0, 1.0, user_id).await?,
        };

        let snark_stream = match taskdb::get_stream(&self.db_pool, user_id, SNARK_WORK_TYPE).await?
        {
            Some(id) => id,
            None => taskdb::create_stream(&self.db_pool, SNARK_WORK_TYPE, 0, 1.0, user_id).await?,
        };

        Ok((
            aux_stream,
            exec_stream,
            gpu_prove_stream,
            gpu_coproc_stream,
            gpu_join_stream,
            snark_stream,
        ))
    }

    async fn upload_task_result(&self, job_id: &Uuid, task_type: &TaskType) -> Result<()> {
        tracing::info!("Uploading results for gRPC task: {}", job_id);

        let bucket_dir = match task_type {
            TaskType::Snark(_) => GROTH16_BUCKET_DIR,
            _ => STARK_BUCKET_DIR,
        };
        let receipt_key = format!("{RECEIPT_BUCKET_DIR}/{bucket_dir}/{job_id}.bincode");
        if !&self
            .s3_client
            .object_exists(&receipt_key)
            .await
            .context("Failed to check if object exists")?
        {
            tracing::error!("Receipt missing for job_id: {} in {} bucket", job_id, bucket_dir);
            return Err(anyhow::anyhow!("Receipt missing for job_id: {} in {} bucket", job_id, bucket_dir));
        }

        let receipt = &self
            .s3_client
            .read_buf_from_s3(&receipt_key)
            .await
            .context("Failed to read from object store")?;

        match task_type {
            TaskType::Snark(_) => {
                let request = UploadGroth16ResultRequest {
                    task_id: job_id.to_string(),
                    proof_data: receipt.clone(),
                    description: "Groth16 proof from SNARK task".to_string(),
                };

                if let Some(client) = &self.grpc_client {
                    match client.upload_groth16_result(request).await {
                        Ok(response) => {
                            if response.success {
                                tracing::info!(
                                    "Successfully uploaded Groth16 proof for task: {}",
                                    job_id
                                );
                            } else {
                                tracing::error!(
                                    "Server rejected Groth16 upload: {}",
                                    response.error_message
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to upload Groth16 proof: {}", e);
                        }
                    }
                }
            }
            _ => {
                let receipt_obj: risc0_zkvm::Receipt = bincode::deserialize(receipt)
                    .context("Failed to deserialize receipt with bincode")?;
                let journal: risc0_zkvm::Journal = receipt_obj.journal.clone();
                let journal_data = bincode::serialize(&journal)
                    .context("Failed to serialize journal data")?;
                let request = UploadStarkResultRequest {
                    task_id: job_id.to_string(),
                    receipt_data: receipt.clone(),
                    journal_data,
                    description: "STARK receipt from Executor task".to_string(),
                };

                if let Some(client) = &self.grpc_client {
                    match client.upload_stark_result(request).await {
                        Ok(response) => {
                            if response.success {
                                tracing::info!(
                                    "Successfully uploaded STARK receipt for task: {}",
                                    job_id
                                );
                            } else {
                                tracing::error!(
                                    "Server rejected STARK upload: {}",
                                    response.error_message
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to upload STARK receipt: {}", e);
                        }
                    }
                }
            }
        }

        // Delete the receipt from S3 after successful upload
        match &self.s3_client.delete_object(&receipt_key).await {
            Ok(_) => {
                tracing::info!("Deleted receipt from S3: {}", receipt_key);
            }
            Err(e) => {
                tracing::warn!("Failed to delete receipt from S3: {}: {}", receipt_key, e);
            }
        }
        Ok(())
    }

    async fn update_grpc_task_status_if_exists(&self, job_id: &uuid::Uuid, task_type: &TaskType, res: &serde_json::Value) -> Result<()> {
        if self.grpc_client.is_none() {
            return Ok(());
        }

        let (status, message, total_segments, total_cycles) = match task_type {
            TaskType::Executor(_) => {
                let mut total_segments = None;
                let mut total_cycles = None;
                if let Ok(exec_resp) =
                    serde_json::from_value::<workflow_common::ExecutorResp>(res.clone())
                {
                    total_cycles = Some(exec_resp.total_cycles);
                    total_segments = Some(exec_resp.segments);
                }
                (
                    grpc_client::bento::v1::TaskStatus::GeneratingProof as i32,
                    "Task is executing".to_string(),
                    total_segments,
                    total_cycles,
                )
            }
            TaskType::Finalize(_) | TaskType::Snark(_) => (
                grpc_client::bento::v1::TaskStatus::Completed as i32,
                "Task is Completed".to_string(),
                None,
                None,
            ),
            _ => (
                grpc_client::bento::v1::TaskStatus::GeneratingProof as i32,
                "Task is generating proof".to_string(),
                None,
                None,
            ),
        };

        let update_request = grpc_client::bento::v1::UpdateTaskProgressRequest {
            task_id: job_id.to_string(),
            status,
            message,
            total_segments,
            total_cycles,
        };

        if let Some(client) = &self.grpc_client {
            client.update_task_progress(update_request).await?;
        }

        if status == grpc_client::bento::v1::TaskStatus::Completed as i32 {
            self.upload_task_result(job_id, task_type).await?;
        }

        Ok(())
    }
}

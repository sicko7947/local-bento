use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::filter::EnvFilter;
use workflow::{Agent, Args, EXEC_WORK_TYPE};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let task_stream = args.task_stream.clone();
    let enable_grpc = args.enable_grpc;
    let grpc_endpoint = args.grpc_endpoint.clone();
    
    let agent = Agent::new(args)
        .await
        .context("Failed to initialize Agent")?;
    
    let agent = Arc::new(agent);

    sqlx::migrate!("../taskdb/migrations")
        .run(&agent.db_pool)
        .await
        .context("Failed to run migrations")?;

    tracing::info!("Successful agent startup! Original worker type: {task_stream}");

    if enable_grpc && task_stream == EXEC_WORK_TYPE{
        tracing::info!("Starting gRPC task polling to endpoint: {}", grpc_endpoint);
        
        tokio::select! {
            grpc_result = agent.poll_grpc_tasks() => {
                if let Err(err) = grpc_result {
                    tracing::error!("gRPC task polling failed: {}", err);
                }
            }
            work_result = agent.poll_work() => {
                return work_result.context("Exiting agent polling");
            }
        }
    } else {
        agent.poll_work().await.context("Exiting agent polling")?;
    }
    
    Ok(())
}
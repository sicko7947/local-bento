use anyhow::{Context, Result};
use bonsai_sdk::non_blocking::Client as ProvingClient;
use clap::Parser;
use risc0_zkvm::compute_image_id;
use std::path::PathBuf;
use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Risc0 ZKVM elf file on disk
    #[clap(short = 'f', long)]
    elf_file: PathBuf,

    /// ZKVM encoded input to be supplied to ExecEnv .write() method
    #[clap(short, long)]
    input_file: PathBuf,

    /// Optionally Create a SNARK proof
    #[clap(short, long, default_value_t = false)]
    snarkify: bool,

    /// Bento HTTP API Endpoint
    #[clap(short = 't', long, default_value = "http://localhost:8080")]
    endpoint: String,
    
    /// Output directory for downloaded proofs
    #[clap(short = 'o', long, default_value = "./proofs")]
    output_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Create output directory if it doesn't exist
    if !args.output_dir.exists() {
        fs::create_dir_all(&args.output_dir)
            .context("Failed to create output directory")?;
    }

    let client =
        ProvingClient::from_parts(args.endpoint, String::new(), risc0_zkvm::VERSION).unwrap();

    // Read ELF file
    let image = std::fs::read(&args.elf_file)
        .context("Failed to read elf file from disk")?;
    
    // Read input file
    let input = std::fs::read(&args.input_file)
        .context("Failed to read input file from disk")?;

    // Submit and track job
    let (session_uuid, _) = stark_workflow(&client, image, input, vec![], &args.output_dir).await?;

    // Generate SNARK if requested
    if args.snarkify {
        stark_2_snark(session_uuid, client, &args.output_dir).await?;
    }

    Ok(())
}

async fn stark_workflow(
    client: &ProvingClient,
    image: Vec<u8>,
    input: Vec<u8>,
    assumptions: Vec<String>,
    output_dir: &PathBuf,
) -> Result<(String, String)> {
    // Generate and upload image
    let image_id = compute_image_id(&image).unwrap().to_string();
    client.upload_img(&image_id, image).await
        .context("Failed to upload image")?;

    // Upload input
    let input_id = client.upload_input(input).await
        .context("Failed to upload input")?;

    tracing::info!("Image ID: {image_id} | Input ID: {input_id}");

    // Create session
    let session = client
        .create_session(image_id.clone(), input_id.clone(), assumptions, false)
        .await
        .context("Failed to create STARK proving session")?;
    tracing::info!("STARK job ID: {}", session.uuid);

    // Save session information
    let session_info_path = output_dir.join(format!("{}_session_info.json", session.uuid));
    let session_info = serde_json::json!({
        "session_uuid": session.uuid,
        "image_id": image_id,
        "input_id": input_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    fs::write(&session_info_path, serde_json::to_string_pretty(&session_info)?)
        .context("Failed to write session info to file")?;
    tracing::info!("Session info saved to: {}", session_info_path.display());

    // Poll for completion
    let mut receipt_id = String::new();
    loop {
        let res = session.status(client).await
            .context("Failed to get STARK status")?;

        match res.status.as_ref() {
            "RUNNING" => {
                tracing::info!("STARK Job running....");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
            "SUCCEEDED" => {
                tracing::info!("Job done!");
                
                let receipt = client
                    .receipt_download(&session)
                    .await
                    .context("Failed to download receipt")?;

                // Save STARK receipt to local file
                let receipt_path = output_dir.join(format!("{}_stark_receipt.bin", session.uuid));
                fs::write(&receipt_path, &receipt)
                    .context("Failed to write STARK receipt to file")?;
                tracing::info!("STARK receipt saved to: {}", receipt_path.display());

                receipt_id = client.upload_receipt(receipt).await
                    .context("Failed to upload receipt")?;
                break;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Job failed: {} - {}",
                    session.uuid,
                    res.error_msg.as_ref().unwrap_or(&String::new())
                ));
            }
        }
    }
    Ok((session.uuid, receipt_id))
}

async fn stark_2_snark(session_id: String, client: ProvingClient, output_dir: &PathBuf) -> Result<()> {
    tracing::info!("STARK 2 SNARK job_id: {}", session_id);
    let snark_session = client.create_snark(session_id).await
        .context("Failed to create SNARK session")?;
    
    loop {
        let res = snark_session.status(&client).await
            .context("Failed to get snark session status")?;
        
        match res.status.as_ref() {
            "RUNNING" => {
                tracing::info!("SNARK Job running....");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
            "SUCCEEDED" => {
                tracing::info!("SNARK Job done!");

                let snark_receipt = client
                    .download(&res.output.context("SNARK missing output URL")?)
                    .await
                    .context("Failed to download snark receipt")?;
                
                // Save SNARK receipt to local file
                let snark_path = output_dir.join(format!("{}_snark_receipt.bin", snark_session.uuid));
                fs::write(&snark_path, &snark_receipt)
                    .context("Failed to write SNARK receipt to file")?;
                tracing::info!("SNARK receipt saved to: {}", snark_path.display());
                
                break;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "SNARK Job failed: {} - {}",
                    snark_session.uuid,
                    res.error_msg.as_ref().unwrap_or(&String::new())
                ));
            }
        }
    }
    Ok(())
}
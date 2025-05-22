use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../proto/bento/v1/bento_task_service.proto");
    println!("cargo:rerun-if-changed=../../proto");
    
    let out_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/gen");
    
    // Create the output directory if it doesn't exist
    std::fs::create_dir_all(&out_dir)?;
    
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(out_dir)
        .compile_protos(
            &["../../proto/bento/v1/bento_task_service.proto"],
            &["../../proto"],
        )?;
    
    Ok(())
}

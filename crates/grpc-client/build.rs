fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../proto/bento/v1/bento_task_service.proto");
    
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir("src/gen")
        .compile(
            &["../../proto/bento/v1/bento_task_service.proto"],
            &["../../proto"],
        )?;
    
    Ok(())
}

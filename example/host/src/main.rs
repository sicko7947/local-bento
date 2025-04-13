use methods::{GUEST_CODE_FOR_ZK_PROOF_ELF};
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: u32 = 999;
    let b: u32 = 888;

    println!("Preparing to compute the sum of {} and {}", a, b);

    // Create a directory for our files if it doesn't exist
    let bento_dir = Path::new("bento_files");
    fs::create_dir_all(&bento_dir)?;

    // Save the ELF file to disk
    let elf_path = bento_dir.join("guest.elf");
    fs::write(&elf_path, GUEST_CODE_FOR_ZK_PROOF_ELF)?;
    println!("ELF file saved to: {}", elf_path.display());

    // Serialize u32 values to little-endian byte arrays
    let mut input_bytes = Vec::new();
    input_bytes.extend_from_slice(&a.to_le_bytes());
    input_bytes.extend_from_slice(&b.to_le_bytes());

    // Save the input to disk
    let input_path = bento_dir.join("input.bin");
    fs::write(&input_path, &input_bytes)?;
    println!("Input file saved to: {}", input_path.display());

    // Print instructions for running Bento
    println!("\nTo prove with Bento, run the following command:");
    println!("./bento_cli -f {} -i {} -t http://localhost:8081 -o ./proofs", 
             elf_path.display(), input_path.display());

    Ok(())
}

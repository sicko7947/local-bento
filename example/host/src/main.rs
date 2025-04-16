use methods::{GUEST_CODE_FOR_ZK_PROOF_ELF, GUEST_CODE_FOR_ZK_PROOF_ID};
use risc0_zkvm::serde::{from_slice, to_vec};
use risc0_zkvm::{Receipt, Digest};
use std::fs;
use std::path::Path;

fn verify_receipt(receipt: &Receipt) -> Result<(), Box<dyn std::error::Error>> {
    // Verify the receipt with the provided image_id
    receipt.verify(GUEST_CODE_FOR_ZK_PROOF_ID).unwrap();
    
    let output: u32 = receipt
        .journal
        .decode()
        .expect("Failed to decode journal");

    println!("Receipt verification successful!");
    println!("Output: {}", output);
    Ok(())
}

fn read_receipt_from_file(file_path: &str) -> Result<Receipt, Box<dyn std::error::Error>> {
    // Read the receipt bytes from the file
    let receipt_bytes = fs::read(file_path)?;
    // Deserialize the receipt
    let receipt: Receipt = bincode::deserialize(&receipt_bytes)?;
    println!("Receipt successfully read from file: {}", file_path);
    Ok(receipt)
}

fn generate_input() -> Result<(), Box<dyn std::error::Error>> {
    let a: u32 = 17;
    let b: u32 = 25;

    println!("Preparing to compute the sum of {} and {}", a, b);

    // Create a directory for our files if it doesn't exist
    let bento_dir = Path::new("bento_files");
    fs::create_dir_all(&bento_dir)?;

    // Save the ELF file to disk
    let elf_path = bento_dir.join("guest.elf");
    fs::write(&elf_path, GUEST_CODE_FOR_ZK_PROOF_ELF)?;
    println!("ELF file saved to: {}", elf_path.display());

    // Serialize input values using risc0_zkvm::serde::to_vec
    let input_data: Vec<u32> = to_vec(&(a, b))?;
    let input_data_bytes: Vec<u8> = input_data
        .iter()
        .flat_map(|x| x.to_le_bytes().to_vec())
        .collect(); // Convert Vec<u64> to Vec<u8>
    let input_path = bento_dir.join("input.bin");
    fs::write(&input_path, &input_data_bytes)?;
    println!("Input file saved to: {}", input_path.display());

    Ok(())
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receipt = read_receipt_from_file("/home/sicko/Downloads/323dbcd5-c8a1-4b39-ac0a-19e9508d8451.bincode")?;

    verify_receipt(&receipt)?;

    Ok(())
}
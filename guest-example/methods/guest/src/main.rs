use risc0_zkvm::guest::env;

fn main() {
    // Read the two input values from the host
    let a: u32 = env::read();
    let b: u32 = env::read();
    
    // Compute the sum
    let sum = a + b;
    
    // Print the result for debugging
    println!("The sum of {} and {} is: {}", a, b, sum);
    
    // Commit the result to the journal
    env::commit(&sum);
}
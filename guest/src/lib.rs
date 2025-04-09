#[cfg(not(target_os = "zkvm"))]
pub fn get_methods() -> Methods {
    Methods::new()
}

#[derive(Debug)]
pub struct Methods {}

impl Methods {
    pub fn new() -> Self {
        Self {}
    }
}
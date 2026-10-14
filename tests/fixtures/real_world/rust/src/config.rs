#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub max_workers: usize,
    pub batch_size: usize,
    pub timeout_seconds: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            batch_size: 100,
            timeout_seconds: 30,
        }
    }
}

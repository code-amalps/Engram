use crate::queue::WorkQueue;

pub struct Worker {
    pub id: String,
}

impl Worker {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn process_next(&self, queue: &mut WorkQueue) -> Option<String> {
        let task = queue.pop()?;
        Some(format!("Worker {} processed task: {}", self.id, task))
    }
}

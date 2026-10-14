use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct WorkQueue {
    tasks: VecDeque<String>,
}

impl WorkQueue {
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
        }
    }

    pub fn push(&mut self, task: String) {
        self.tasks.push_back(task);
    }

    pub fn pop(&mut self) -> Option<String> {
        self.tasks.pop_front()
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

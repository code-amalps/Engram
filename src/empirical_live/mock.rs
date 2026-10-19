//! Deterministic offline mock LLM provider for tests and benchmark baseline validation.
//!
//! # Safety & Determinism Invariants
//! 1. **100% Offline:** Zero live API calls, zero network sockets, zero credentials.
//! 2. **Canned Response Queue:** Configurable FIFO queue of canned responses or simulated errors.
//! 3. **Execution Recording:** Records full request history for post-execution assertions
//!    (verifying that rejected or halted requests never reach the provider).

use crate::empirical_live::provider::{LlmProvider, ProviderError};
use crate::empirical_live::schema::{LlmRequest, LlmResponse};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Deterministic mock LLM provider for offline testing.
pub struct MockLlmProvider {
    model_id: String,
    canned_responses: Mutex<VecDeque<Result<LlmResponse, ProviderError>>>,
    call_history: Mutex<Vec<LlmRequest>>,
}

impl MockLlmProvider {
    /// Construct a new mock provider for a given model identifier.
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            canned_responses: Mutex::new(VecDeque::new()),
            call_history: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a canned successful response to be returned on subsequent invocation.
    pub fn push_response(&self, response: LlmResponse) {
        let mut queue = self
            .canned_responses
            .lock()
            .expect("mock canned mutex poisoned");
        queue.push_back(Ok(response));
    }

    /// Enqueue a canned error (e.g. timeout, rate limit, authentication) to be simulated.
    pub fn push_error(&self, error: ProviderError) {
        let mut queue = self
            .canned_responses
            .lock()
            .expect("mock canned mutex poisoned");
        queue.push_back(Err(error));
    }

    /// Total count of calls made to `execute_turn`.
    pub fn call_count(&self) -> usize {
        let history = self
            .call_history
            .lock()
            .expect("mock history mutex poisoned");
        history.len()
    }

    /// Retrieve clones of all recorded requests.
    pub fn recorded_requests(&self) -> Vec<LlmRequest> {
        let history = self
            .call_history
            .lock()
            .expect("mock history mutex poisoned");
        history.clone()
    }

    /// Retrieve the most recently dispatched request, if any.
    pub fn last_request(&self) -> Option<LlmRequest> {
        let history = self
            .call_history
            .lock()
            .expect("mock history mutex poisoned");
        history.last().cloned()
    }

    /// Clear all recorded call history.
    pub fn clear_history(&self) {
        let mut history = self
            .call_history
            .lock()
            .expect("mock history mutex poisoned");
        history.clear();
    }

    /// Clear remaining queued canned responses.
    pub fn clear_canned(&self) {
        let mut queue = self
            .canned_responses
            .lock()
            .expect("mock canned mutex poisoned");
        queue.clear();
    }
}

impl LlmProvider for MockLlmProvider {
    fn provider_name(&self) -> &'static str {
        "mock"
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn execute_turn(&self, request: &LlmRequest) -> Result<LlmResponse, ProviderError> {
        // Record call in history
        {
            let mut history = self
                .call_history
                .lock()
                .map_err(|_| ProviderError::Internal("Mock history lock poisoned".to_string()))?;
            history.push(request.clone());
        }

        // Pop next canned response
        let mut queue = self
            .canned_responses
            .lock()
            .map_err(|_| ProviderError::Internal("Mock canned lock poisoned".to_string()))?;

        queue.pop_front().unwrap_or_else(|| {
            Err(ProviderError::ProviderError {
                status_code: None,
                message: format!(
                    "MockLlmProvider ('{}') has no remaining canned responses",
                    self.model_id
                ),
            })
        })
    }
}

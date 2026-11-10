//! Unique operation identities shared by the local composition or one remote endpoint.

use super::ApplicationRuntimeError;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

#[derive(Clone)]
pub(super) struct RequestIds(Arc<AtomicU32>);

impl Default for RequestIds {
    fn default() -> Self {
        Self(Arc::new(AtomicU32::new(1)))
    }
}
impl RequestIds {
    pub(super) fn allocate(&self) -> Result<u32, ApplicationRuntimeError> {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ApplicationRuntimeError::RequestExhausted)
    }
}

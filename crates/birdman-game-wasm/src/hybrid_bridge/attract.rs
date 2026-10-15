use super::*;

#[wasm_bindgen]
impl HybridGameSessionBridge {
    /// Opens the independent cached hybrid demonstration only from Title.
    pub fn enter_attract(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::EnterAttract {}, BackendReadiness::Preparing)
    }

    /// Leaves demonstration playback without changing the player's selection.
    pub fn leave_attract(&mut self) -> Result<(), JsValue> {
        self.execute_application(SessionCommand::LeaveAttract {}, BackendReadiness::Preparing)
    }
}

#[cfg(test)]
mod tests;

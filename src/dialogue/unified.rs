use serde::{Deserialize, Serialize};
use crate::handlers::broadcast::BroadcastState;
use crate::dialogue::RegistrationState;

#[derive(Clone, Serialize, Deserialize)]
pub enum UnifiedDialogueState {
    #[serde(rename = "broadcast")]
    Broadcast(BroadcastState),
    #[serde(rename = "registration")]
    Registration(RegistrationState),
}

impl Default for UnifiedDialogueState {
    fn default() -> Self {
        UnifiedDialogueState::Broadcast(BroadcastState::default())
    }
}

// Helper functions para crear estados
impl UnifiedDialogueState {
    pub fn broadcast(state: BroadcastState) -> Self {
        UnifiedDialogueState::Broadcast(state)
    }
    
    pub fn registration(state: RegistrationState) -> Self {
        UnifiedDialogueState::Registration(state)
    }
}

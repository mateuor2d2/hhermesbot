use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

// Re-exportar estructuras desde dialogue::states
pub use crate::dialogue::states::{CentroPendiente, ServicioPendiente};

/// Estado del wizard de registro por usuario
#[derive(Clone, Debug)]
pub struct WizardState {
    pub step: WizardStep,
    pub data: RegistrationData,
}

#[derive(Clone, Debug)]
pub enum WizardStep {
    AskType,
    AskName,
    AskDescription,
    AskCifChoice,
    AskCif,
    AskContactChoice,
    AskPhone,
    AskEmail,
    // Nuevos pasos para centros
    AskAddCentro,
    CentroAskNombre,
    CentroAskDireccion,
    CentroAskCiudad,
    CentroAskTelefono,
    CentroAskEmail,
    CentroConfirm,
    // Nuevos pasos para servicios
    AskAddServicio,
    ServicioAskTipo,
    ServicioAskCategoria,
    ServicioAskNombre,
    ServicioAskDescripcion,
    ServicioAskPrecio,
    ServicioConfirm,
    // Confirmación final
    Confirm,
}

// RegistrationData ahora se importa desde dialogue::states
pub use crate::dialogue::states::RegistrationData;

// Estado global del wizard
static WIZARD_STATES: Lazy<Mutex<HashMap<i64, WizardState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn start_wizard(user_id: i64) {
    let mut states = WIZARD_STATES.lock().unwrap();
    states.insert(
        user_id,
        WizardState {
            step: WizardStep::AskType,
            data: RegistrationData::default(),
        },
    );
}

pub fn get_wizard_state(user_id: i64) -> Option<WizardState> {
    let states = WIZARD_STATES.lock().unwrap();
    states.get(&user_id).cloned()
}

pub fn update_wizard_state(user_id: i64, state: WizardState) {
    let mut states = WIZARD_STATES.lock().unwrap();
    states.insert(user_id, state);
}

pub fn clear_wizard(user_id: i64) {
    let mut states = WIZARD_STATES.lock().unwrap();
    states.remove(&user_id);
}

pub fn set_wizard_step(user_id: i64, step: WizardStep) {
    let mut states = WIZARD_STATES.lock().unwrap();
    if let Some(state) = states.get_mut(&user_id) {
        state.step = step;
    }
}

pub fn update_wizard_data(user_id: i64, data: RegistrationData) {
    let mut states = WIZARD_STATES.lock().unwrap();
    if let Some(state) = states.get_mut(&user_id) {
        state.data = data;
    }
}

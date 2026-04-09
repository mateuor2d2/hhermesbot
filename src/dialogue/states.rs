use serde::{Deserialize, Serialize};

/// Estados para la búsqueda interactiva
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub enum SearchState {
    #[default]
    Idle,
    /// Esperando término de búsqueda (flujo simplificado)
    WaitingQuery,
    /// Mostrar opciones de campos de búsqueda (por nombre, dirección, servicio, ciudad, todo)
    WaitingForField,
    /// Esperando término de búsqueda tras seleccionar un campo
    WaitingForQuery { field: SearchField },
}

/// Campos disponibles para la búsqueda
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SearchField {
    Name,    // Buscar por nombre (empresa)
    Address, // Buscar por dirección
    Service, // Buscar por servicio
    City,    // Buscar por ciudad
    All,     // Buscar en todos los campos (comportamiento actual)
}

/// Estados para la conversación de difusión
#[derive(Clone, Default, Serialize, Deserialize)]
pub enum BroadcastState {
    #[default]
    Idle,
    WaitingTitle,
    WaitingContent {
        title: String,
    },
    Confirm {
        title: String,
        content: String,
    },
}

/// Combined dialogue state that can handle broadcast and search flows
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum BotDialogueState {
    #[default]
    Idle,
    // Broadcast states
    Broadcast(BroadcastState),
    // Search states
    Search(SearchState),
}

/// Datos de un centro durante el registro
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CentroPendiente {
    pub nombre: String,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
}

impl Default for CentroPendiente {
    fn default() -> Self {
        Self {
            nombre: "".to_string(),
            direccion: None,
            ciudad: None,
            telefono: None,
            email: None,
        }
    }
}

/// Datos de un servicio durante el registro
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServicioPendiente {
    pub tipo: String, // "bien" o "servicio"
    pub categoria: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub precio: Option<String>,
}

impl Default for ServicioPendiente {
    fn default() -> Self {
        Self {
            tipo: "servicio".to_string(),
            categoria: "".to_string(),
            nombre: "".to_string(),
            descripcion: None,
            precio: None,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct RegistrationData {
    pub user_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub cif: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub centros: Vec<CentroPendiente>,
    pub servicios: Vec<ServicioPendiente>,
    // Campos adicionales para empresas
    pub website: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub category: Option<String>,
    pub logo_url: Option<String>,
    pub completed: bool,
    // Campos temporales para el wizard
    pub temp_centro: Option<CentroPendiente>,
    pub temp_servicio: Option<ServicioPendiente>,
    // Método de contacto preferido: "phone", "email", "both"
    pub contact_method: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(dead_code)]
pub enum UserType {
    Empresa,
    Autonomo,
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserType::Empresa => write!(f, "Empresa"),
            UserType::Autonomo => write!(f, "Autónomo"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum RegistrationState {
    AskType,
    AskName {
        data: RegistrationData,
    },
    AskDescription {
        data: RegistrationData,
    },
    AskCifChoice {
        data: RegistrationData,
    },
    AskCif {
        data: RegistrationData,
    },
    AskContactChoice {
        data: RegistrationData,
    },
    AskPhone {
        data: RegistrationData,
    },
    AskEmail {
        data: RegistrationData,
    },
    // Nuevos estados para centros
    AskAddCentro {
        data: RegistrationData,
    },
    CentroAskNombre {
        data: RegistrationData,
    },
    CentroAskDireccion {
        data: RegistrationData,
    },
    CentroAskCiudad {
        data: RegistrationData,
    },
    CentroAskTelefono {
        data: RegistrationData,
    },
    CentroAskEmail {
        data: RegistrationData,
    },
    CentroConfirm {
        data: RegistrationData,
    },
    // Nuevos estados para servicios
    AskAddServicio {
        data: RegistrationData,
    },
    ServicioAskTipo {
        data: RegistrationData,
    },
    ServicioAskCategoria {
        data: RegistrationData,
    },
    ServicioAskNombre {
        data: RegistrationData,
    },
    ServicioAskDescripcion {
        data: RegistrationData,
    },
    ServicioAskPrecio {
        data: RegistrationData,
    },
    ServicioConfirm {
        data: RegistrationData,
    },
    // Confirmación final
    Confirm {
        data: RegistrationData,
    },
    EditField {
        data: RegistrationData,
        field: FieldToEdit,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum FieldToEdit {
    Type,
    Name,
    Description,
    Cif,
    Phone,
    Email,
}

/// Pasos adicionales para añadir centros y servicios
#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(dead_code)]
pub enum CentroStep {
    AskNombre,
    AskDireccion,
    AskCiudad,
    AskTelefono,
    AskEmail,
    Confirm,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(dead_code)]
pub enum ServicioStep {
    AskTipo,
    AskCategoria,
    AskNombre,
    AskDescripcion,
    AskPrecio,
    Confirm,
}

impl RegistrationData {
    #[allow(dead_code)]
    pub fn is_complete(&self) -> bool {
        self.user_type.is_some() && self.name.is_some() && self.description.is_some()
    }

    pub fn to_summary(&self) -> String {
        let mut text = String::from("📋 <b>Resumen de tu registro</b>\n\n");

        if let Some(ref t) = self.user_type {
            text.push_str(&format!("<b>Tipo:</b> {}\n", t));
        }
        if let Some(ref n) = self.name {
            text.push_str(&format!("<b>Nombre:</b> {}\n", n));
        }
        if let Some(ref d) = self.description {
            text.push_str(&format!("<b>Descripción:</b> {}\n", d));
        }
        if let Some(ref c) = self.cif {
            text.push_str(&format!("<b>CIF:</b> {}\n", c));
        } else {
            text.push_str("<b>CIF:</b> <i>No especificado</i>\n");
        }
        if let Some(ref p) = self.phone {
            text.push_str(&format!("<b>Teléfono:</b> {}\n", p));
        } else {
            text.push_str("<b>Teléfono:</b> <i>No especificado</i>\n");
        }
        if let Some(ref e) = self.email {
            text.push_str(&format!("<b>Email:</b> {}\n", e));
        } else {
            text.push_str("<b>Email:</b> <i>No especificado</i>\n");
        }

        // Centros
        text.push_str(&format!("\n<b>🏢 Centros ({})</b>\n", self.centros.len()));
        if self.centros.is_empty() {
            text.push_str("<i>No se han añadido centros</i>\n");
        } else {
            for (i, centro) in self.centros.iter().enumerate() {
                text.push_str(&format!("{}. {}\n", i + 1, centro.nombre));
                if let Some(ref ciudad) = centro.ciudad {
                    text.push_str(&format!("   📍 {}\n", ciudad));
                }
            }
        }

        // Servicios
        text.push_str(&format!(
            "\n<b>🛎️ Servicios/Productos ({})</b>\n",
            self.servicios.len()
        ));
        if self.servicios.is_empty() {
            text.push_str("<i>No se han añadido servicios</i>\n");
        } else {
            for (i, servicio) in self.servicios.iter().enumerate() {
                text.push_str(&format!(
                    "{}. {} ({} - {})\n",
                    i + 1,
                    servicio.nombre,
                    servicio.tipo,
                    servicio.categoria
                ));
            }
        }

        text
    }
}

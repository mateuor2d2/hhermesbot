use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Empresa {
    pub id: i64,
    pub telegram_id: i64,
    pub tipo: TipoEmpresa,
    pub nombre_fiscal: String,
    pub nombre_comercial: Option<String>,
    pub cif_nif: Option<String>,
    pub direccion: Option<String>,
    pub codigo_postal: Option<String>,
    pub ciudad: Option<String>,
    pub provincia: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub web: Option<String>,
    pub descripcion: Option<String>,
    pub activa: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum TipoEmpresa {
    Autonomo,
    Sociedad,
}

impl std::fmt::Display for TipoEmpresa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TipoEmpresa::Autonomo => write!(f, "Autónomo"),
            TipoEmpresa::Sociedad => write!(f, "Sociedad"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NuevaEmpresa {
    pub telegram_id: i64,
    pub tipo: TipoEmpresa,
    pub nombre_fiscal: String,
    pub nombre_comercial: Option<String>,
    pub cif_nif: Option<String>,
    pub direccion: Option<String>,
    pub codigo_postal: Option<String>,
    pub ciudad: Option<String>,
    pub provincia: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub web: Option<String>,
    pub descripcion: Option<String>,
}

impl Empresa {
    pub fn nombre_publico(&self) -> &str {
        self.nombre_comercial.as_deref().unwrap_or(&self.nombre_fiscal)
    }
}

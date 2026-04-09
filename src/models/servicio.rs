use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Servicio {
    pub id: i64,
    pub empresa_id: i64,
    pub tipo: String,
    pub categoria: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub precio: Option<String>,
    pub disponible: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NuevoServicio {
    pub empresa_id: i64,
    pub categoria: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub precio: Option<String>,
}

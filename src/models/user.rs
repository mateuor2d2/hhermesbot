use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub telegram_id: i64,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_internal: bool,
    pub is_admin: bool,
    pub is_member: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub telegram_id: i64,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
}

impl User {
    pub fn display_name(&self) -> String {
        match (&self.first_name, &self.last_name) {
            (first, Some(last)) => format!("{} {}", first, last),
            (first, None) => first.clone(),
        }
    }
}

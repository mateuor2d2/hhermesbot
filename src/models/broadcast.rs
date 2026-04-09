use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Tracks how many broadcasts a user has sent in a quarter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastUsage {
    pub id: i64,
    pub user_id: i64,
    pub quarter: i32,
    pub year: i32,
    pub count: i32,
    pub paid_extra: i32,
    pub last_used_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl BroadcastUsage {
    /// Total broadcasts available (free + paid)
    pub fn total_available(&self, free_limit: i32) -> i32 {
        free_limit + self.paid_extra
    }
    
    /// Remaining broadcasts
    pub fn remaining(&self, free_limit: i32) -> i32 {
        (self.total_available(free_limit) - self.count).max(0)
    }
    
    /// Check if user has broadcasts remaining
    pub fn has_remaining(&self, free_limit: i32) -> bool {
        self.count < self.total_available(free_limit)
    }
}

/// Payment record for additional broadcasts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastPayment {
    pub id: i64,
    pub user_id: i64,
    pub quarter: i32,
    pub year: i32,
    pub amount: f64,
    pub broadcasts_added: i32,
    pub payment_method: String,
    pub payment_reference: Option<String>,
    pub status: String, // pending, completed, failed
    pub paid_at: Option<NaiveDateTime>,
    pub verified_by: Option<i64>,
    pub created_at: NaiveDateTime,
}

/// Draft of a broadcast being created
#[derive(Debug, Clone, Default)]
pub struct BroadcastDraft {
    pub title: Option<String>,
    pub content: Option<String>,
    pub target_channel: Option<i64>,
}

impl BroadcastDraft {
    pub fn is_complete(&self) -> bool {
        self.title.is_some() && self.content.is_some()
    }
    
    pub fn formatted_message(&self) -> Option<String> {
        let title = self.title.as_ref()?;
        let content = self.content.as_ref()?;
        
        Some(format!(
            "📢 *{}*\n\n{}",
            title, content
        ))
    }
}

/// Result of checking if user can broadcast
#[derive(Debug, Clone)]
pub struct BroadcastCheckResult {
    pub can_broadcast: bool,
    pub usage: BroadcastUsage,
    pub free_limit: i32,
    pub remaining: i32,
    pub reason: Option<String>,
}

impl BroadcastCheckResult {
    pub fn success(usage: BroadcastUsage, free_limit: i32) -> Self {
        let remaining = usage.remaining(free_limit);
        Self {
            can_broadcast: remaining > 0,
            usage,
            free_limit,
            remaining,
            reason: None,
        }
    }
    
    pub fn denied(usage: BroadcastUsage, free_limit: i32, reason: String) -> Self {
        Self {
            can_broadcast: false,
            usage,
            free_limit,
            remaining: 0,
            reason: Some(reason),
        }
    }
    
    pub fn message(&self) -> String {
        if self.can_broadcast {
            format!(
                "✅ Tienes {} difusiones disponibles (usadas: {}, límite gratuito: {}, pagadas: {})",
                self.remaining,
                self.usage.count,
                self.free_limit,
                self.usage.paid_extra
            )
        } else {
            format!(
                "❌ {}\n\nUsadas: {} | Gratis: {} | Pagadas: {}\nContacta con un administrador si necesitas más difusiones.",
                self.reason.as_ref().unwrap_or(&"No tienes difusiones disponibles".to_string()),
                self.usage.count,
                self.free_limit,
                self.usage.paid_extra
            )
        }
    }
}

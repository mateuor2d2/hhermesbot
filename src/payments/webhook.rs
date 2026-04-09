//! Funciones de pago y webhook
//! 
//! Este módulo contiene funciones para manejar pagos de Stripe
//! sin necesidad de un servidor HTTP separado.

use sqlx::SqlitePool;
use chrono::{Utc, Datelike};
// use std::sync::Arc;

/// Configuración del webhook
#[derive(Clone)]
pub struct WebhookConfig {
    pub stripe_webhook_secret: String,
    pub db_pool: SqlitePool,
}

/// Procesar payload de webhook de Stripe (llamado desde donde sea necesario)
pub async fn handle_stripe_webhook_payload(
    payload: &str,
    signature: &str,
    config: &WebhookConfig,
) -> Result<WebhookResponse, String> {
    // Verificar firma del webhook usando hmac directamente
    let sig_parts: Vec<&str> = signature.split(',').collect();
    let mut timestamp = None;
    let mut v1_signature = None;
    
    for part in sig_parts {
        if let Some(stripped) = part.strip_prefix("t=") {
            timestamp = stripped.parse::<u64>().ok();
        } else if let Some(stripped) = part.strip_prefix("v1=") {
            v1_signature = Some(stripped);
        }
    }
    
    let timestamp = timestamp.ok_or("Missing timestamp in signature")?;
    let v1_signature = v1_signature.ok_or("Missing v1 signature")?;
    
    // Verificar que el timestamp no sea muy antiguo (5 minutos)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?
        .as_secs();
    
    if now.saturating_sub(timestamp) > 300 {
        return Err("Webhook timestamp too old".to_string());
    }
    
    // Calcular firma esperada
    let signed_payload = format!("{}.{}", timestamp, payload);
    let expected_sig = compute_hmac_sha256(&config.stripe_webhook_secret, &signed_payload);
    
    if !constant_time_compare(&expected_sig, v1_signature) {
        return Err("Invalid webhook signature".to_string());
    }
    
    // Parsear el evento JSON
    let event: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let event_type = event["type"].as_str().unwrap_or("");
    
    tracing::info!("Received Stripe event: {}", event_type);
    
    if event_type == "checkout.session.completed" {
        let session = &event["data"]["object"];
        process_checkout_session(session, &config.db_pool).await?;
    }
    
    Ok(WebhookResponse {
        success: true,
        message: "Webhook processed".to_string(),
    })
}

/// Procesar sesión de checkout completada
async fn process_checkout_session(
    session: &serde_json::Value,
    pool: &SqlitePool,
) -> Result<(), String> {
    let metadata = &session["metadata"];
    
    // Extraer telegram_id
    let telegram_id = match metadata["telegram_id"].as_str() {
        Some(id) => id.parse::<i64>().map_err(|_| "Invalid telegram_id")?,
        None => {
            // Intentar extraer del client_reference_id
            let ref_id = session["client_reference_id"].as_str()
                .ok_or("No telegram_id or client_reference_id")?;
            let parts: Vec<&str> = ref_id.split('_').collect();
            if parts.is_empty() {
                return Err("Cannot extract telegram_id".to_string());
            }
            parts[0].parse::<i64>().map_err(|_| "Invalid telegram_id in ref_id")?
        }
    };
    
    // Extraer créditos
    let credits = match metadata["credits"].as_str() {
        Some(c) => c.parse::<i32>().map_err(|_| "Invalid credits")?,
        None => {
            let ref_id = session["client_reference_id"].as_str()
                .ok_or("No credits found")?;
            let parts: Vec<&str> = ref_id.split('_').collect();
            if parts.len() < 2 {
                return Err("Cannot extract credits".to_string());
            }
            parts[1].parse::<i32>().map_err(|_| "Invalid credits in ref_id")?
        }
    };
    
    let pack_name = metadata["pack_name"].as_str().unwrap_or("Unknown");
    let amount = session["amount_total"].as_i64().unwrap_or(0) as f64 / 100.0;
    let session_id = session["id"].as_str().unwrap_or("unknown");
    
    tracing::info!(
        "Payment completed: user={}, credits={}, pack={}, amount={:.2}€",
        telegram_id, credits, pack_name, amount
    );
    
    // Añadir créditos al usuario
    let now = Utc::now();
    let year = now.year();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    
    let result = sqlx::query(
        "INSERT INTO broadcast_usage (telegram_id, quarter, year, count, paid_extra) 
         VALUES (?, ?, ?, 0, ?)
         ON CONFLICT(telegram_id, quarter, year) DO UPDATE SET 
         paid_extra = paid_extra + ?"
    )
    .bind(telegram_id)
    .bind(quarter)
    .bind(year)
    .bind(credits)
    .bind(credits)
    .execute(pool)
    .await;
    
    match result {
        Ok(_) => {
            tracing::info!("Credits added successfully for user {}", telegram_id);
            
            // Registrar el pago
            let _ = sqlx::query(
                "INSERT INTO payments (telegram_id, stripe_session_id, amount, credits, pack_name, status, created_at)
                 VALUES (?, ?, ?, ?, ?, 'completed', ?)"
            )
            .bind(telegram_id)
            .bind(session_id)
            .bind(amount)
            .bind(credits)
            .bind(pack_name)
            .bind(now.naive_utc())
            .execute(pool)
            .await;
            
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to add credits: {}", e);
            Err(format!("Database error: {}", e))
        }
    }
}

/// Calcular HMAC-SHA256
fn compute_hmac_sha256(key: &str, data: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Comparación en tiempo constante para evitar timing attacks
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let a_bytes = hex::decode(a).unwrap_or_default();
    let b_bytes = hex::decode(b).unwrap_or_default();
    
    let mut result = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Respuesta del webhook
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
}

/// Estructura para almacenar pagos
#[derive(Debug, sqlx::FromRow)]
pub struct Payment {
    pub id: i64,
    pub telegram_id: i64,
    pub stripe_session_id: String,
    pub amount: f64,
    pub credits: i32,
    pub pack_name: String,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
}

/// Obtener historial de pagos de un usuario
pub async fn get_user_payments(
    telegram_id: i64,
    pool: &SqlitePool,
) -> Result<Vec<Payment>, sqlx::Error> {
    sqlx::query_as(
        "SELECT * FROM payments WHERE telegram_id = ? ORDER BY created_at DESC LIMIT 20"
    )
    .bind(telegram_id)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_response() {
        let response = WebhookResponse {
            success: true,
            message: "OK".to_string(),
        };
        assert!(response.success);
        assert_eq!(response.message, "OK");
    }
}

use reqwest::Client;
use teloxide::types::Message;
use anyhow::Result;
use crate::config::Config;
use tracing::{info, error, debug};

pub struct RelayBot {
    client: Client,
    token: String,
    timeout: u64,
    enabled: bool,
    target_chat_id: Option<i64>,
}

impl RelayBot {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            token: config.bot.token.clone(),
            timeout: config.bot.relay_timeout,
            enabled: config.features.enable_relay,
            target_chat_id: Some(config.features.relay_target_chat_id),
        }
    }
    
    pub async fn relay_message(&self, original_msg: &Message, entity_name: &str) -> Result<bool> {
        if !self.enabled {
            debug!("Reenvío deshabilitado");
            return Ok(false);
        }
        
        let api_url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.token
        );
        
        let user = original_msg.from()
            .map(|u| format!("{} (@{})", u.full_name(), u.username.as_deref().unwrap_or("N/A")))
            .unwrap_or_else(|| "Desconocido".to_string());
        
        let text = original_msg.text()
            .unwrap_or("[Mensaje sin texto]");
        
        let relay_text = format!(
            "📨 Mensaje de {}\n👤 Usuario: {}\n💬 Contenido: {}",
            entity_name,
            user,
            text
        );
        
        // Obtener chat_id de destino (configurado en relay_target_chat_id, o fallback al original)
        let chat_id = self.target_chat_id.unwrap_or(original_msg.chat.id.0);
        
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": relay_text,
            "parse_mode": "HTML"
        });
        
        debug!("Reenviando mensaje a Bot B...");
        
        match self.client
            .post(&api_url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Mensaje reenviado exitosamente a Bot B");
                    Ok(true)
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    error!("Bot B respondió con error: {} - {}", status, body);
                    Ok(false)
                }
            }
            Err(e) => {
                error!("Error al reenviar a Bot B: {}", e);
                Ok(false)
            }
        }
    }
    
    pub async fn send_notification(&self, chat_id: i64, text: &str) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        
        let api_url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.token
        );
        
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        });
        
        let response = self.client
            .post(&api_url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()
            .await?;
        
        Ok(response.status().is_success())
    }
}

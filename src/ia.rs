use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::ApiConfig;

#[derive(Clone)]
pub struct IaClient {
    client: Client,
    config: Arc<ApiConfig>,
    api_key: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: i32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

impl IaClient {
    pub fn new(config: Arc<ApiConfig>, api_key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            config,
            api_key,
        })
    }

    pub async fn chat(&self, user_message: &str, context: Option<&str>) -> anyhow::Result<String> {
        let system_prompt = context.unwrap_or(
            "Eres un asistente útil y amable. Responde de forma concisa y clara en español."
        );

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            max_tokens: 1000,
        };

        let url = format!("{}/chat/completions", self.config.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API error: {}", error_text));
        }

        let chat_response: ChatResponse = response.json().await?;
        
        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "Lo siento, no pude generar una respuesta.".to_string());

        Ok(content)
    }
}

pub type SharedIa = Option<Arc<IaClient>>;

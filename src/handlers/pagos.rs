use teloxide::{prelude::*, types::{Message, ParseMode}};
use sqlx::Row;
use std::sync::Arc;
use crate::BotState;
use crate::text_processor::escape_html;
use anyhow::Result;

/// Handler for /mis_pagos command - shows payment history
pub async fn handle_mis_pagos(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> Result<()> {
    let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    
    let rows = sqlx::query(
        "SELECT created_at, amount, broadcasts_added, status 
         FROM broadcast_payments 
         WHERE user_id = ?1
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(user_id)
    .fetch_all(&state.db.pool)
    .await?;
    
    let mut text = "💳 Tu historial de pagos\n\n".to_string();
    
    if rows.is_empty() {
        text.push_str("No tienes pagos registrados.\n\n");
        text.push_str("💡 Usa /comprar para adquirir difusiones adicionales.");
    } else {
        for row in rows {
            let created_at: String = row.get("created_at");
            let amount: f64 = row.get("amount");
            let broadcasts_added: i32 = row.get("broadcasts_added");
            let status: String = row.get("status");
            
            let status_emoji = match status.as_str() {
                "completed" | "succeeded" => "✅",
                "pending" => "⏳",
                "failed" => "❌",
                "cancelled" => "🚫",
                _ => "❓"
            };
            
            text.push_str(&format!(
                "{} {} — {:.2}€ ({} créditos)\n",
                status_emoji,
                escape_html(&created_at),
                amount,
                broadcasts_added
            ));
        }
    }
    
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    
    Ok(())
}

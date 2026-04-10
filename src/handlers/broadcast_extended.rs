//! Handlers extendidos para difusiones
//!
//! Este módulo contiene handlers adicionales para la gestión de difusiones:
//! - Menú principal de difusiones
//! - Historial de difusiones
//! - Estadísticas detalladas
//! - Compra de créditos

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode, ChatId};
use crate::db::SharedDb;
use crate::config::Config;
use crate::text_processor::escape_html;
use std::sync::Arc;
use chrono::{Utc, Datelike};

/// Estructura Broadcast para usar en los handlers
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BroadcastInfo {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content: String,
    pub channel_message_id: Option<i32>,
    pub is_paid: bool,
    pub created_at: chrono::NaiveDateTime,
}

/// Mostrar el menú principal de difusiones
pub async fn handle_menu_difusiones(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    // Obtener uso del trimestre actual
    let now = Utc::now();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    let year = now.year();
    
    let usage = get_broadcast_usage(&db, user_id, year, quarter).await?;
    let (used, paid_extra) = usage.unwrap_or((0, 0));
    
    let free_limit = config.broadcast.quarterly_limit;
    let remaining_free = free_limit.saturating_sub(used);
    let total_remaining = remaining_free + paid_extra;
    
    // Calcular estadísticas
    let total_sent = count_user_broadcasts(&db, user_id).await?;
    
    let text = format!(
        "📢 <b>Difusiones</b>\n\n\
        📊 <b>Tu uso actual:</b>\n\
        ├ Trimestre: Q{} {}\n\
        ├ Usadas: {} de {} gratis\n\
        ├ Créditos comprados: {}\n\
        └ <b>Disponibles: {}</b>\n\n\
        📈 <b>Total enviadas: {}</b>\n\n\
        ¿Qué quieres hacer?",
        quarter, year,
        used, free_limit,
        paid_extra,
        total_remaining,
        total_sent
    );
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📝 Nueva difusión", "broadcast:new"),
            InlineKeyboardButton::callback("📜 Historial", "broadcast:history:0"),
        ],
        vec![
            InlineKeyboardButton::callback("📊 Estadísticas", "broadcast:stats"),
            InlineKeyboardButton::callback("💳 Comprar créditos", "broadcast:buy"),
        ],
        vec![
            InlineKeyboardButton::callback("🔙 Volver", "menu:start"),
        ],
    ]);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Mostrar historial de difusiones con paginación
pub async fn handle_broadcast_history(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    db: SharedDb,
    page: usize,
) -> anyhow::Result<()> {
    const PER_PAGE: usize = 5;
    let offset = page * PER_PAGE;
    
    let broadcasts = get_user_broadcasts_paginated(&db, user_id, offset as i32, PER_PAGE as i32).await?;
    let total = count_user_broadcasts(&db, user_id).await?;
    let total_pages = if total == 0 { 1 } else { (total + PER_PAGE - 1) / PER_PAGE };
    
    if broadcasts.is_empty() && page == 0 {
        bot.send_message(chat_id, "📭 No tienes difusiones enviadas todavía.")
            .await?;
        return Ok(());
    }
    
    let mut text = format!("📜 <b>Tus difusiones</b> (página {})\n\n", page + 1);
    
    for b in broadcasts.iter() {
        let status_emoji = if b.is_paid { "💳" } else { "🆓" };
        let date = b.created_at.format("%d/%m/%Y %H:%M");
        let snippet: String = b.content.chars().take(50).collect();
        
        text.push_str(&format!(
            "{} <b>{}</b> ({})\n\
            └ {}...\n\
            └ ID: <code>{}</code>\n\n",
            status_emoji,
            escape_html(&b.title),
            escape_html(&date.to_string()),
            escape_html(&snippet),
            b.id
        ));
    }
    
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = broadcasts
        .iter()
        .map(|b| {
            vec![InlineKeyboardButton::callback(
                format!("📋 Ver: {}", b.title.chars().take(25).collect::<String>()),
                format!("broadcast:view:{}", b.id)
            )]
        })
        .collect();
    
    // Paginación
    let mut nav_buttons = vec![];
    if page > 0 {
        nav_buttons.push(InlineKeyboardButton::callback("⬅️ Anterior", format!("broadcast:history:{}", page - 1)));
    }
    if page + 1 < total_pages {
        nav_buttons.push(InlineKeyboardButton::callback("➡️ Siguiente", format!("broadcast:history:{}", page + 1)));
    }
    if !nav_buttons.is_empty() {
        buttons.push(nav_buttons);
    }
    
    buttons.push(vec![InlineKeyboardButton::callback("🔙 Volver", "menu:difusiones")]);
    
    let keyboard = InlineKeyboardMarkup::new(buttons);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Ver detalles de una difusión específica
pub async fn handle_broadcast_view(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    broadcast_id: i64,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let broadcast = match get_broadcast_by_id(&db, broadcast_id).await? {
        Some(b) => b,
        None => {
            bot.send_message(chat_id, "❌ Difusión no encontrada.").await?;
            return Ok(());
        }
    };
    
    // Verificar que pertenece al usuario
    if broadcast.user_id != user_id {
        bot.send_message(chat_id, "⛔ No tienes permiso para ver esta difusión.").await?;
        return Ok(());
    }
    
    let status = if broadcast.is_paid { "💳 Pagada" } else { "🆓 Gratuita" };
    let date = broadcast.created_at.format("%d/%m/%Y a las %H:%M");
    
    let text = format!(
        "📋 <b>Detalles de difusión</b>\n\n\
        <b>Título:</b> {}\n\
        <b>Fecha:</b> {}\n\
        <b>Estado:</b> {}\n\n\
        <b>Contenido:</b>\n{}",
        escape_html(&broadcast.title),
        escape_html(&date.to_string()),
        escape_html(status),
        escape_html(&broadcast.content)
    );
    
    // Botones
    let mut buttons = vec![];
    
    // Si tiene link al canal, mostrar botón
    if broadcast.channel_message_id.is_some() {
        let channel_id = &config.broadcast.channel_id;
        let channel_link = format!("https://t.me/c/{}/{}", 
            channel_id.trim_start_matches("-100"),
            broadcast.channel_message_id.unwrap_or(0)
        );
        
        let url: reqwest::Url = channel_link.parse()
            .unwrap_or_else(|_| "https://t.me".parse().unwrap());
        buttons.push(vec![InlineKeyboardButton::url("🔗 Ver en canal", url)]);
    }
    
    buttons.push(vec![
        InlineKeyboardButton::callback("🗑️ Eliminar", format!("broadcast:delete_confirm:{}", broadcast_id)),
        InlineKeyboardButton::callback("🔙 Volver", "broadcast:history:0"),
    ]);
    
    let keyboard = InlineKeyboardMarkup::new(buttons);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Confirmar eliminación de difusión
pub async fn handle_broadcast_delete_confirm(
    bot: Bot,
    chat_id: ChatId,
    broadcast_id: i64,
) -> anyhow::Result<()> {
    let text = "⚠️ <b>¿Eliminar difusión?</b>\n\n\
        Esta acción no se puede deshacer.\n\
        La difusión se eliminará del historial, pero seguirá visible en el canal si ya fue publicada.";
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("✅ Sí, eliminar", format!("broadcast:delete:{}", broadcast_id)),
            InlineKeyboardButton::callback("❌ Cancelar", format!("broadcast:view:{}", broadcast_id)),
        ],
    ]);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Eliminar una difusión
pub async fn handle_broadcast_delete(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    broadcast_id: i64,
    db: SharedDb,
) -> anyhow::Result<()> {
    let broadcast = match get_broadcast_by_id(&db, broadcast_id).await? {
        Some(b) => b,
        None => {
            bot.send_message(chat_id, "❌ Difusión no encontrada.").await?;
            return Ok(());
        }
    };
    
    if broadcast.user_id != user_id {
        bot.send_message(chat_id, "⛔ No tienes permiso para eliminar esta difusión.").await?;
        return Ok(());
    }
    
    delete_broadcast(&db, broadcast_id).await?;
    
    bot.send_message(
        chat_id,
        "✅ Difusión eliminada del historial.\n\n\
        💡 Si ya estaba publicada en el canal, seguirá visible allí."
    )
    .parse_mode(ParseMode::Html)
    .await?;
    
    Ok(())
}

/// Mostrar estadísticas detalladas
pub async fn handle_broadcast_stats(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let total_sent = count_user_broadcasts(&db, user_id).await?;
    
    // Estadísticas por trimestre del año actual
    let year = Utc::now().year();
    let mut quarterly_stats = Vec::new();
    
    for q in 1..=4 {
        let usage = get_broadcast_usage(&db, user_id, year, q).await?;
        let (used, paid) = usage.unwrap_or((0, 0));
        quarterly_stats.push((q, used, paid));
    }
    
    // Primera difusión
    let first_broadcast: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
        "SELECT MIN(created_at) FROM broadcasts WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(&db.pool)
    .await?
    .flatten();
    
    let first_date = first_broadcast
        .map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|| "Nunca".to_string());
    
    let mut text = format!(
        "📊 <b>Estadísticas de difusiones</b>\n\n\
        📈 <b>Resumen general:</b>\n\
        ├ Total enviadas: <b>{}</b>\n\
        └ Primera difusión: <b>{}</b>\n\n\
        📅 <b>Uso por trimestre ({}):</b>\n",
        total_sent,
        escape_html(&first_date),
        year
    );
    
    for (q, used, paid) in &quarterly_stats {
        let remaining = config.broadcast.quarterly_limit.saturating_sub(*used) + paid;
        text.push_str(&format!(
            "├ Q{}: {} usadas, {} compradas, {} disponibles\n",
            q, used, paid, remaining
        ));
    }
    
    // Promedio mensual
    let months_active = first_broadcast
        .map(|fb| {
            let months = (Utc::now().naive_utc().signed_duration_since(fb).num_days() / 30) as f64;
            months.max(1.0)
        })
        .unwrap_or(1.0);
    
    let monthly_avg = total_sent as f64 / months_active;
    
    text.push_str(&format!(
        "\n📉 <b>Promedio:</b> {:.1} difusiones/mes",
        monthly_avg
    ));
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📝 Nueva difusión", "broadcast:new"),
            InlineKeyboardButton::callback("🔙 Volver", "menu:difusiones"),
        ],
    ]);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Mostrar opciones de compra de créditos
pub async fn handle_broadcast_buy(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    // Log para debugging
    tracing::info!("handle_broadcast_buy: payment_enabled = {}", config.broadcast.payment_enabled);
    
    // Verificar que los pagos están habilitados
    if !config.broadcast.payment_enabled {
        bot.send_message(
            chat_id,
            "❌ El sistema de pagos está deshabilitado en la configuración.\n\
            Contacta con el administrador."
        )
        .parse_mode(ParseMode::Html)
        .await?;
        return Ok(());
    }
    
    let now = Utc::now();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    let year = now.year();
    
    let usage = get_broadcast_usage(&db, user_id, year, quarter).await?;
    let (used, paid_extra) = usage.unwrap_or((0, 0));
    let remaining_free = config.broadcast.quarterly_limit.saturating_sub(used);
    let total_remaining = remaining_free + paid_extra;
    
    let text = format!(
        "💳 <b>Comprar créditos de difusión</b>\n\n\
        📊 <b>Estado actual:</b>\n\
        ├ Disponibles: <b>{}</b> gratis + <b>{}</b> comprados\n\
        └ Total: <b>{}</b> difusiones\n\n\
        📦 <b>Paquetes disponibles:</b>\n\n\
        • <b>Pack S:</b> 5 créditos - 3,00€\n\
        • <b>Pack M:</b> 10 créditos - 5,00€\n\
        • <b>Pack L:</b> 25 créditos - 10,00€\n\n\
        💡 Los créditos no caducan y se acumulan.",
        remaining_free, paid_extra, total_remaining
    );
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("📦 Pack S: 5 créditos - 3€", "buy_pack:S")],
        vec![InlineKeyboardButton::callback("📦 Pack M: 10 créditos - 5€", "buy_pack:M")],
        vec![InlineKeyboardButton::callback("📦 Pack L: 25 créditos - 10€", "buy_pack:L")],
        vec![InlineKeyboardButton::callback("❓ ¿Cuántas necesito?", "calc_needs")],
        vec![InlineKeyboardButton::callback("🔙 Volver", "menu:difusiones")],
    ]);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

/// Iniciar proceso de compra de pack
pub async fn handle_buy_pack(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    pack_name: &str,
    _db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    use crate::payments::{StripeClient, get_pack_by_name_owned};
    
    // Log para debugging - mostrar primeros caracteres de la key
    let key_preview = if config.stripe.secret_key.len() > 10 {
        format!("{}...", &config.stripe.secret_key[..10])
    } else {
        "VACIO O CORTO".to_string()
    };
    tracing::info!("Stripe secret_key preview: {}", key_preview);
    
    let stripe_config = Arc::new(config.stripe.clone());
    
    let pack = match get_pack_by_name_owned(pack_name) {
        Some(p) => p,
        None => {
            bot.send_message(chat_id, "❌ Pack no encontrado.").await?;
            return Ok(());
        }
    };
    
    bot.send_message(
        chat_id,
        "⏳ Creando sesión de pago..."
    )
    .parse_mode(ParseMode::Html)
    .await?;
    
    let stripe = StripeClient::new(stripe_config);
    
    match stripe.create_checkout_session(user_id, &pack).await {
        Ok(checkout_url) => {
            let text = format!(
                "💳 <b>Pack {}</b>\n\n\
                • Créditos: <b>{}</b>\n\
                • Precio: <b>{:.2}€</b>\n\n\
                Pulsa el botón para completar el pago.",
                pack.name, pack.credits, pack.price_eur
            );
            
            let url: reqwest::Url = checkout_url.parse()
                .map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
            
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![InlineKeyboardButton::url("💳 Pagar ahora", url)],
                vec![InlineKeyboardButton::callback("❌ Cancelar", "menu:difusiones")],
            ]);
            
            bot.send_message(chat_id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard)
                .await?;
        }
        Err(e) => {
            tracing::error!("Error creating checkout session: {}", e);
            let error_str = e.to_string();
            // Limit error message length for Telegram (max 4096 chars)
            let truncated = if error_str.len() > 200 {
                format!("{}...", &error_str[..200])
            } else {
                error_str
            };
            let error_msg = format!(
                "❌ <b>Error al crear la sesión de pago</b>\n\n\
                Detalle: <code>{}</code>\n\n\
                Verifica que la configuración de Stripe sea correcta.",
                truncated
            );
            bot.send_message(chat_id, error_msg)
                .parse_mode(ParseMode::Html)
                .await?;
        }
    }
    
    Ok(())
}

/// Calcular necesidades del usuario
pub async fn handle_calc_needs(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let total_sent = count_user_broadcasts(&db, user_id).await?;
    
    let text = format!(
        "❓ <b>¿Cuántas difusiones necesitas?</b>\n\n\
        📊 <b>Tu actividad:</b>\n\
        └ Total enviadas: <b>{}</b>\n\n\
        🆓 <b>Gratuitas:</b>\n\
        Cada trimestre recibes <b>{}</b> difusiones gratis.\n\n\
        💡 <b>Consejo:</b>\n\
        Si envías más de {} difusiones por trimestre, \
        considera comprar el <b>Pack M</b> o <b>Pack L</b> para ahorrar.\n\n\
        💳 Los créditos comprados no caducan.",
        total_sent,
        config.broadcast.quarterly_limit,
        config.broadcast.quarterly_limit
    );
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("💳 Comprar ahora", "broadcast:buy")],
        vec![InlineKeyboardButton::callback("🔙 Volver", "menu:difusiones")],
    ]);
    
    bot.send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

// ===== FUNCIONES AUXILIARES DE BASE DE DATOS =====

async fn get_broadcast_usage(db: &SharedDb, user_id: i64, year: i32, quarter: i32) -> anyhow::Result<Option<(i32, i32)>> {
    let result: Option<(i32, i32)> = sqlx::query_as(
        "SELECT count, paid_extra FROM broadcast_usage WHERE telegram_id = ? AND year = ? AND quarter = ?"
    )
    .bind(user_id)
    .bind(year)
    .bind(quarter)
    .fetch_optional(&db.pool)
    .await?;
    
    Ok(result)
}

async fn count_user_broadcasts(db: &SharedDb, user_id: i64) -> anyhow::Result<usize> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM broadcasts WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_one(&db.pool)
    .await?;
    
    Ok(count as usize)
}

async fn get_user_broadcasts_paginated(db: &SharedDb, user_id: i64, offset: i32, limit: i32) -> anyhow::Result<Vec<BroadcastInfo>> {
    let broadcasts = sqlx::query_as::<_, BroadcastInfo>(
        "SELECT id, user_id, title, content, channel_message_id, is_paid, created_at 
         FROM broadcasts 
         WHERE user_id = ? 
         ORDER BY created_at DESC 
         LIMIT ? OFFSET ?"
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&db.pool)
    .await?;
    
    Ok(broadcasts)
}

async fn get_broadcast_by_id(db: &SharedDb, broadcast_id: i64) -> anyhow::Result<Option<BroadcastInfo>> {
    let broadcast = sqlx::query_as::<_, BroadcastInfo>(
        "SELECT id, user_id, title, content, channel_message_id, is_paid, created_at 
         FROM broadcasts 
         WHERE id = ?"
    )
    .bind(broadcast_id)
    .fetch_optional(&db.pool)
    .await?;
    
    Ok(broadcast)
}

async fn delete_broadcast(db: &SharedDb, broadcast_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM broadcasts WHERE id = ?")
        .bind(broadcast_id)
        .execute(&db.pool)
        .await?;
    
    Ok(())
}

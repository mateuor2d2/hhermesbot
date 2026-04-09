use teloxide::prelude::*;
use teloxide::types::{ParseMode, InlineKeyboardMarkup, InlineKeyboardButton};
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage};
use crate::db::SharedDb;
use crate::config::Config;
use crate::handlers::BotState;
use crate::dialogue::states::{BotDialogueState, BroadcastState};

use crate::payments::{format_packs_keyboard, get_pack_by_name, StripeClient};
use std::sync::Arc;
use chrono::{Utc, Datelike};
// use tracing as log;

pub type BroadcastDialogue = Dialogue<BotDialogueState, InMemStorage<BotDialogueState>>;

/// Comando /difundir - inicia el proceso
pub async fn start_broadcast(
    bot: Bot,
    msg: Message,
    dialogue: BroadcastDialogue,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let user_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let _username = msg.from().as_ref().and_then(|u| u.username.clone());
    
    // Verificar que el broadcast está habilitado
    if !config.broadcast.enabled {
        bot.send_message(msg.chat.id, "❌ Las difusiones están temporalmente deshabilitadas.").await?;
        return Ok(());
    }

    // Verificar suscripción al canal si es requerida
    if config.subscriptions.required_for_broadcast {
        let channel_id_i64 = config.broadcast.channel_id.parse::<i64>()?;
        let is_subscribed = check_channel_subscription(&bot, UserId(user_id as u64), channel_id_i64).await?;
        
        if !is_subscribed {
            let channel_id_str = &config.broadcast.channel_id;
            let channel_username = channel_id_str.trim_start_matches("-100");
            let channel_link = format!("https://t.me/c/{}", channel_username);
            
            bot.send_message(
                msg.chat.id,
                format!(
                    "⚠️ Para difundir, debes ser miembro del canal de difusión.\n\n\
                    Únete aquí: {}",
                    channel_link
                )
            ).await?;
            return Ok(());
        }
    }

    // Obtener uso actual
    let now = Utc::now();
    let year = now.year();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    
    let (used_count, paid_extra) = db.get_broadcast_usage(user_id, year, quarter).await?;
    let free_limit = config.broadcast.quarterly_limit;
    
    let (has_free, has_paid) = db.can_broadcast(user_id, year, quarter, free_limit).await?;
    
    if !has_free && !has_paid {
        // Sin créditos disponibles
        let keyboard = if config.broadcast.payment_enabled {
            InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback("💳 Comprar difusión extra", "buy_broadcast")
            ]])
        } else {
            InlineKeyboardMarkup::default()
        };
        
        bot.send_message(
            msg.chat.id,
            format!(
                "📊 <b>Uso de difusiones (T{})</b>\n\
                Gratis usadas: {}/{}\n\
                Pagadas disponibles: {}\n\n\
                ❌ Has alcanzado el límite de difusiones gratuitas este trimestre.",
                quarter, used_count, free_limit, paid_extra
            )
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
        return Ok(());
    }

    // Mostrar estado y pedir título
    let credits_info = if has_free {
        format!("Gratis: {}/{} | Pagadas: {}", used_count, free_limit, paid_extra)
    } else {
        format!("Usando: 1 difusión pagada (quedan {})", paid_extra - 1)
    };

    bot.send_message(
        msg.chat.id,
        format!(
            "📢 <b>Nueva Difusión</b>\n\
            Créditos: {}\n\n\
            Paso 1/3: Envía el <b>título</b> de tu anuncio:",
            credits_info
        )
    )
    .parse_mode(ParseMode::Html)
    .await?;

    dialogue.update(BotDialogueState::Broadcast(BroadcastState::WaitingTitle)).await?;
    Ok(())
}

/// Recibe el título del broadcast
pub async fn receive_broadcast_title(
    bot: Bot,
    msg: Message,
    dialogue: BroadcastDialogue,
    title: String,
) -> anyhow::Result<()> {
    bot.send_message(
        msg.chat.id,
        format!(
            "✅ Título: <b>{}</b>\n\n\
            Paso 2/3: Ahora envía el <b>contenido</b> de tu anuncio:",
            title
        )
    )
    .parse_mode(ParseMode::Html)
    .await?;
    
    dialogue.update(BotDialogueState::Broadcast(BroadcastState::WaitingContent { title })).await?;
    Ok(())
}

/// Recibe el contenido y muestra confirmación
pub async fn receive_broadcast_content(
    bot: Bot,
    msg: Message,
    dialogue: BroadcastDialogue,
    title: String,
    content: String,
) -> anyhow::Result<()> {
    let preview = format!(
        "📋 <b>Vista previa:</b>\n\n\
        <b>{}</b>\n\n\
        {}\n\n\
        ¿Confirmar envío?",
        title,
        content
    );

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("✅ Confirmar", format!("broadcast_confirm:{}", msg.id.0)),
        ],
        vec![
            InlineKeyboardButton::callback("📝 Editar título", "broadcast_edit_title"),
            InlineKeyboardButton::callback("📝 Editar contenido", "broadcast_edit_content"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar", "broadcast_cancel"),
        ],
    ]);

    bot.send_message(msg.chat.id, preview)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    // Guardar temporalmente (usamos un enfoque simple: guardamos en el estado)
    dialogue.update(BotDialogueState::Broadcast(BroadcastState::Confirm { title, content })).await?;
    Ok(())
}

/// Procesa callbacks de broadcast
pub async fn handle_broadcast_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: BroadcastDialogue,
    state: BroadcastState,
    db: SharedDb,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    if let Some(data) = q.data.as_ref() {
        let chat_id = q.message.as_ref().map(|m| m.chat.id);
        
        match data.as_str() {
            data if data.starts_with("broadcast_confirm") => {
                if let BroadcastState::Confirm { title, content } = state {
                    if let Some(chat_id) = chat_id {
                        match send_broadcast_to_channel(&bot, &db, &config, q.from.id.0 as i64, &title, &content, q.from.username.as_deref()).await {
                            Ok(result_msg) => {
                                bot.send_message(chat_id, result_msg).await?;
                            }
                            Err(e) => {
                                bot.send_message(chat_id, format!("❌ Error al enviar: {}", e)).await?;
                            }
                        }
                    }
                    dialogue.reset().await?;
                }
            }
            "broadcast_edit_title" => {
                if let Some(chat_id) = chat_id {
                    bot.send_message(chat_id, "📝 Envía el nuevo título:").await?;
                    dialogue.update(BotDialogueState::Broadcast(BroadcastState::WaitingTitle)).await?;
                }
            }
            "broadcast_edit_content" => {
                if let BroadcastState::Confirm { title, .. } = state {
                    if let Some(chat_id) = chat_id {
                        bot.send_message(chat_id, "📝 Envía el nuevo contenido:").await?;
                        dialogue.update(BotDialogueState::Broadcast(BroadcastState::WaitingContent { title })).await?;
                    }
                }
            }
            "broadcast_cancel" => {
                if let Some(chat_id) = chat_id {
                    bot.send_message(chat_id, "❌ Difusión cancelada.").await?;
                }
                dialogue.reset().await?;
            }
            "buy_broadcast" => {
                let chat_id = q.message.as_ref().map(|m| m.chat.id);
                let user_id = q.from.id;
                buy_broadcast(bot.clone(), chat_id, user_id, config).await?;
            }
            data if data.starts_with("buy_pack:") => {
                let pack_name = data.trim_start_matches("buy_pack:").to_string();
                let chat_id = q.message.as_ref().map(|m| m.chat.id);
                let user_id = q.from.id;
                if let Some(chat_id) = chat_id {
                    process_pack_purchase(bot.clone(), chat_id, user_id, pack_name, config).await?;
                }
            }
            "calc_needs" => {
                if let Some(chat_id) = q.message.as_ref().map(|m| m.chat.id) {
                    bot.send_message(
                        chat_id,
                        "❓ <b>¿Cuántas difusiones necesito?</b>\n\n\
                        Esto depende de tu actividad:\n\n\
                        🏢 <b>Empresa establecida:</b> 2-3 difusiones/trimestre\n\
                        📢 <b>Promoción activa:</b> 5-10 difusiones/trimestre\n\
                        🚀 <b>Lanzamiento/nuevo:</b> 10+ difusiones/trimestre\n\n\
                        💡 <b>Consejo:</b> Comienza con el <b>Pack M (10 créditos)</b>.\n\
                        Si tienes 3 gratis + 10 comprados = 13 difusiones,\n\
                        suficiente para 4-5 meses de actividad normal.\n\n\
                        Los créditos <b>no caducan</b> dentro del trimestre actual."
                    )
                    .parse_mode(ParseMode::Html)
                    .await?;
                }
            }
            "back_to_menu" => {
                if let Some(chat_id) = q.message.as_ref().map(|m| m.chat.id) {
                    bot.send_message(
                        chat_id,
                        "🔙 Volviendo al menú principal. Usa /ayuda para ver opciones."
                    )
                    .await?;
                }
            }
            _ => {}
        }
        
        // Responder al callback
        bot.answer_callback_query(&q.id).await?;
    }
    
    Ok(())
}

/// Mostrar packs disponibles para comprar
pub async fn buy_broadcast(
    bot: Bot,
    chat_id: Option<teloxide::types::ChatId>,
    _user_id: teloxide::types::UserId,
    _config: Arc<Config>,
) -> anyhow::Result<()> {
    if let Some(chat_id) = chat_id {
        let keyboard = InlineKeyboardMarkup::new(format_packs_keyboard());
        
        bot.send_message(
            chat_id,
            "💳 <b>Comprar Difusiones Adicionales</b>\n\n\
            Elige un pack:\n\n\
            • Pack S: 5 créditos - 3€\n\
            • Pack M: 10 créditos - 5€ (ahorras 1€)\n\
            • Pack L: 25 créditos - 10€ (ahorras 5€)\n\n\
            Los créditos no caducan y se suman a tu trimestre actual."
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    }
    Ok(())
}

/// Procesar compra de un pack específico
pub async fn process_pack_purchase(
    bot: Bot,
    chat_id: ChatId,
    user_id: teloxide::types::UserId,
    pack_name: String,
    config: Arc<Config>,
) -> anyhow::Result<()> {
    let stripe_client = StripeClient::new(Arc::new(config.stripe.clone()));
    
    if let Some(pack) = get_pack_by_name(&pack_name) {
        match stripe_client.create_checkout_session(user_id.0 as i64, pack).await {
            Ok(checkout_url) => {
                bot.send_message(
                    chat_id,
                    format!(
                        "🔗 <b>Pago seguro con Stripe</b>\n\n\
                        Pack {}: {} créditos por {:.0}€\n\n\
                        <a href=\"{}\">👉 Hacer pago seguro</a>\n\n\
                        Una vez completado el pago, los créditos se añadirán automáticamente.",
                        pack.name, pack.credits, pack.price_eur, checkout_url
                    )
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            Err(e) => {
                bot.send_message(
                    chat_id,
                    format!("❌ Error al crear la sesión de pago: {}", e)
                ).await?;
            }
        }
    } else {
        bot.send_message(chat_id, "❌ Pack no encontrado.").await?;
    }
    
    Ok(())
}

/// Añadir créditos (admin)
pub async fn admin_add_credits(
    bot: Bot,
    msg: Message,
    db: SharedDb,
    _config: Arc<Config>,
    target_user_id: i64,
    credits: i32,
) -> anyhow::Result<()> {
    // Verificar que es admin
    let admin_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let admin = db.get_user(admin_id).await?;
    
    if let Some(admin) = admin {
        if !admin.is_admin {
            bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
            return Ok(());
        }
    }
    
    let now = Utc::now();
    let year = now.year();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    
    // Añadir créditos pagados
    sqlx::query(
        "INSERT INTO broadcast_usage (telegram_id, quarter, year, count, paid_extra) 
         VALUES (?, ?, ?, 0, ?)
         ON CONFLICT(telegram_id, quarter, year) DO UPDATE SET 
         paid_extra = paid_extra + ?"
    )
    .bind(target_user_id)
    .bind(quarter)
    .bind(year)
    .bind(credits)
    .bind(credits)
    .execute(&db.pool)
    .await?;
    
    bot.send_message(
        msg.chat.id,
        format!("✅ Añadidas {} difusiones extra al usuario {}", credits, target_user_id)
    ).await?;
    
    Ok(())
}

/// Verifica si el usuario está suscrito al canal
async fn check_channel_subscription(
    bot: &Bot,
    user_id: UserId,
    channel_id: i64,
) -> anyhow::Result<bool> {
    match bot.get_chat_member(ChatId(channel_id), user_id).await {
        Ok(member) => {
            use teloxide::types::ChatMemberStatus;
            Ok(!matches!(member.status(), ChatMemberStatus::Left | ChatMemberStatus::Banned))
        }
        Err(e) => {
            tracing::warn!("Error checking subscription: {:?}", e);
            Ok(false)
        }
    }
}

/// Envía la difusión al canal
async fn send_broadcast_to_channel(
    bot: &Bot,
    db: &SharedDb,
    config: &Config,
    user_id: i64,
    title: &str,
    content: &str,
    username: Option<&str>,
) -> anyhow::Result<String> {
    let now = Utc::now();
    let year = now.year();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    
    // Verificar si usa crédito gratis o pagado
    let (used_count, _) = db.get_broadcast_usage(user_id, year, quarter).await?;
    let free_limit = config.broadcast.quarterly_limit;
    
    let used_free_credit = used_count < free_limit;
    
    // Preparar mensaje para el canal
    let author_info = username
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| "Anónimo".to_string());
    
    let channel_message = format!(
        "📢 <b>{}</b>\n\n\
        {}\n\n\
        ───────────────\n\
        📎 Publicado por: {}\n\
        #Difusión #T{}",
        title,
        content,
        author_info,
        quarter
    );

    // Enviar al canal
    let channel_id_i64 = config.broadcast.channel_id.parse::<i64>()?;
    let sent = bot
        .send_message(ChatId(channel_id_i64), channel_message)
        .parse_mode(ParseMode::Html)
        .await?;

    // Actualizar uso
    if used_free_credit {
        db.increment_broadcast_usage(user_id, year, quarter).await?;
    } else {
        db.use_paid_broadcast(user_id, year, quarter).await?;
    }

    // Guardar registro
    sqlx::query(
        "INSERT INTO broadcasts (user_id, title, content, channel_message_id, is_paid) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(title)
    .bind(content)
    .bind(sent.id.0)
    .bind(!used_free_credit)
    .execute(&db.pool)
    .await?;

    Ok(format!(
        "✅ ¡Difusión enviada correctamente!\n\n\
        📊 Crédito usado: {}\n\
        🔗 Ver en canal",
        if used_free_credit { "Gratis" } else { "Pagado" }
    ))
}

/// Ver difusiones del usuario
pub async fn handle_mis_difusiones(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let user_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let now = Utc::now();
    let year = now.year();
    let quarter = ((now.month() - 1) / 3 + 1) as i32;
    
    let (used_count, paid_extra) = state.db.get_broadcast_usage(user_id, year, quarter).await?;
    let free_limit = state.config.broadcast.quarterly_limit;
    let remaining_free = (free_limit - used_count).max(0);
    let total_remaining = remaining_free + paid_extra;
    
    // Calculate days until quarter ends
    let next_quarter_month = quarter * 3 + 1;
    let next_quarter_year = if next_quarter_month > 12 { year + 1 } else { year };
    let next_quarter_start = if next_quarter_month > 12 {
        chrono::NaiveDate::from_ymd_opt(next_quarter_year, (next_quarter_month - 12) as u32, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(next_quarter_year, next_quarter_month as u32, 1)
    };
    let days_until_reset = next_quarter_start.map(|d| {
        d.signed_duration_since(now.date_naive()).num_days()
    }).unwrap_or(0);
    
    // Progress bar for free usage
    let free_percentage = (used_count as f32 / free_limit as f32 * 10.0).min(10.0) as usize;
    let progress_bar = format!(
        "[{}]",
        "█".repeat(free_percentage) + &"░".repeat(10 - free_percentage)
    );
    
    // Tip based on usage
    let tip = if total_remaining == 0 {
        "❌ <b>¡Sin difusiones!</b> Usa /comprar para adquirir más."
    } else if remaining_free == 0 && paid_extra > 0 {
        "⚠️ Has agotado tus difusiones gratis. Considera comprar un pack antes del reinicio."
    } else if used_count >= free_limit / 2 {
        "💡 Has usado más de la mitad de tus difusiones gratuitas."
    } else {
        "✅ Vas bien con tus difusiones."
    };
    
    // Next reset date
    let reset_date = next_quarter_start.map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|| "próximo trimestre".to_string());
    
    let text = format!(
        "📊 <b>Tus Difusiones - T{} {}</b>\n\n\
        <b>Uso de difusiones gratuitas:</b>\n\
        {}\n\
        {} de {} usadas ({} disponibles)\n\n\
        💳 <b>Pagadas disponibles:</b> {}\n\
        📊 <b>Total disponible:</b> {}\n\n\
        ⏰ <b>Reinicio:</b> {} (quedan {} días)\n\n\
        {}\n\n\
        💡 Usa /comprar para adquirir más difusiones.",
        quarter, year,
        progress_bar,
        used_count, free_limit, remaining_free,
        paid_extra,
        total_remaining,
        reset_date, days_until_reset,
        tip
    );
    
    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    
    Ok(())
}


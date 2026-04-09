use chrono::{Utc, Datelike};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage};

use crate::config::Config;
use crate::db::SharedDb;
use crate::ia::SharedIa;
use crate::text_processor::escape_html;
use crate::wizard::{self, WizardStep, RegistrationData};
use crate::dialogue::states::{BotDialogueState, SearchState, SearchField};
use std::sync::Arc;
pub mod broadcast_extended;
pub mod broadcast;
pub mod pagos;

// Imports de broadcast_extended para routing de callbacks
use crate::handlers::broadcast_extended::{
    // handle_menu_difusiones,  // unused for now
    handle_broadcast_history,
    handle_broadcast_view,
    handle_broadcast_delete_confirm,
    handle_broadcast_delete,
    handle_broadcast_stats,
    handle_broadcast_buy,
    handle_calc_needs,
    handle_buy_pack,
};

pub struct BotState {
    pub db: SharedDb,
    pub ia: SharedIa,
    pub config: Arc<Config>,
}

pub type MyDialogue = Dialogue<BotDialogueState, InMemStorage<BotDialogueState>>;

// Re-export broadcast functions and types
pub use broadcast::{
    start_broadcast,
    receive_broadcast_title,
    receive_broadcast_content,
    handle_broadcast_callback,
    handle_mis_difusiones,
    admin_add_credits,
};

// Re-export pagos functions
pub use pagos::handle_mis_pagos;

// Payment types disponibles cuando se necesiten
// pub use crate::payments::{PaymentStatus, notify_payment_status};

// BroadcastDialogue se mantiene privado al módulo broadcast

// ===== COMANDOS PRINCIPALES =====

pub async fn handle_start(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    
    // Crear/obtener usuario
    let user = state
        .db
        .get_or_create_user(
            telegram_id,
            msg.from().and_then(|u| u.username.as_deref()),
            msg.from().map(|u| u.first_name.as_str()),
            msg.from().and_then(|u| u.last_name.as_deref()),
        )
        .await?;

    // Verificar si ya tiene empresa registrada (activa o pendiente)
    let empresas = state.db.get_empresas_by_user(telegram_id).await?;
    let tiene_empresa_activa = empresas.iter().any(|e| e.activa);
    let tiene_empresa_pendiente = empresas.iter().any(|e| !e.activa);

    // Verificar si es admin del config
    let is_config_admin = state.config.bot.admins.contains(&telegram_id);
    
    // Si ya está registrado completamente (es miembro, admin, tiene empresa activa o es interno) → mostrar menú principal
    if user.is_member || tiene_empresa_activa || user.is_internal || is_config_admin {
        let welcome_text = format!(
            "¡Bienvenido de nuevo, <b>{}</b>! 🤖\n\n\
            Eres miembro de <b>{}</b>. Tienes acceso completo:\n\n\
            • ℹ️ Información sobre servicios\n\
            • 🔍 Buscar empresas y profesionales\n\
            • 💬 Chat con IA ({} msgs/día)\n\
            • 📩 Contactar con otros usuarios\n\
            • 📢 Enviar difusiones",
            user.first_name.as_deref().unwrap_or("Usuario"),
            state.config.bot.name,
            state.config.limits.max_ia_messages_per_day
        );

        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("🔍 Buscar", "menu:buscar"),
                InlineKeyboardButton::callback("💬 Chat IA", "menu:chat"),
            ],
            vec![
                InlineKeyboardButton::callback("ℹ️ Info", "menu:info"),
                InlineKeyboardButton::callback("📋 Mis datos", "menu:misdatos"),
            ],
            vec![
                InlineKeyboardButton::callback("📢 Difusiones", "menu:difusiones"),
                InlineKeyboardButton::callback("📩 Mensajes", "menu:mensajes"),
            ],
        ]);

        bot.send_message(msg.chat.id, welcome_text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard)
            .await?;

        return Ok(());
    }

    // Si tiene empresa pendiente de aprobación
    if tiene_empresa_pendiente {
        let welcome_text = format!(
            "¡Hola de nuevo, <b>{}</b>! 🤖\n\n\
            ⏳ Tu solicitud de registro está <b>pendiente de aprobación</b>.\n\n\
            Un administrador de <b>{}</b> revisará tu información en 24-48 horas.\n\
            Te notificaremos cuando sea aprobada.\n\n\
            Mientras, puedes usar estas funciones:",
            user.first_name.as_deref().unwrap_or("Usuario"),
            state.config.bot.name
        );

        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("🔍 Buscar", "menu:buscar"),
                InlineKeyboardButton::callback("💬 Chat IA", "menu:chat"),
            ],
            vec![
                InlineKeyboardButton::callback("ℹ️ Info", "menu:info"),
                InlineKeyboardButton::callback("❓ Ayuda", "help"),
            ],
        ]);

        bot.send_message(msg.chat.id, welcome_text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard)
            .await?;

        return Ok(());
    }

    // Usuario nuevo o sin empresa registrada
    let welcome_text = format!(
        "¡Bienvenido a <b>{}</b>! 🤖\n\n\
        Soy tu asistente virtual. Puedo ayudarte con:\n\n\
        • ℹ️ Información sobre servicios\n\
        • 🔍 Buscar empresas y profesionales\n\
        • 💬 Chat con IA ({} msgs/día)\n\
        • 📩 Contactar con otros usuarios\n\n\
        Selecciona tu tipo de usuario para continuar:",
        state.config.bot.name,
        state.config.limits.max_ia_messages_per_day
    );

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("👤 Soy particular", "set_type:external"),
            InlineKeyboardButton::callback("🏢 Soy empresa/autónomo", "set_type:internal"),
        ],
        vec![InlineKeyboardButton::callback("❓ Ayuda", "help")],
    ]);

    bot.send_message(msg.chat.id, welcome_text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

pub async fn handle_help(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    let telegram_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let is_admin = state.config.bot.admins.contains(&telegram_id);
    
    let mut help_text = format!(
        "<b>📋 Comandos disponibles:</b>\n\n\
        /start - Iniciar el bot\n\
        /help - Mostrar esta ayuda\n\
        /info - Información sobre {}\n\
        /chat - Iniciar chat con IA\n\
        /buscar - Buscar servicios\n\
        /misdatos - Ver tus datos registrados\n\
        /registrar - Registrar empresa (requiere aprobación)\n\
        /mensajes - Ver mensajes recibidos",
        state.config.bot.name
    );
    
    // Añadir comandos de administrador si el usuario es admin
    if is_admin {
        help_text.push_str("\n\n<b>🔧 Comandos de Administrador:</b>\n\
        /pendientes - Ver empresas pendientes de aprobación\n\
        /aprobar &lt;id&gt; - Aprobar una empresa\n\
        /rechazar &lt;id&gt; - Rechazar una empresa\n\
        /admin_org &lt;campo&gt; &lt;valor&gt; - Configurar datos de la organización\n\
        /admin_add_credits &lt;user_id&gt; &lt;cantidad&gt; - Añadir créditos a usuario\n\
        /admin_member &lt;user_id&gt; &lt;on|off&gt; - Cambiar estado de miembro\n\
        /admin_users &lt;busqueda&gt; - Buscar usuarios");
    }
    
    help_text.push_str("\n\n<b>👤 Tu tipo de usuario:</b>\n\
        • Externos: Buscar información y contactar\n\
        • Internos: Además, ofrecer servicios (requiere aprobación)");

    bot.send_message(msg.chat.id, help_text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;

    Ok(())
}

pub async fn handle_info(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    // Obtener datos de la organización desde la base de datos
    let org = match state.db.get_organization().await {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Error obteniendo organización: {}", e);
            // Fallback a config si hay error
            let info_text = format!(
                "<b>{}</b>\n\n{}",
                state.config.bot.name,
                state.config.bot.description
            );
            bot.send_message(msg.chat.id, info_text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
            return Ok(());
        }
    };

    // Construir mensaje con datos de la organización
    let mut info_text = format!(
        "🏛️ <b>{}</b>\n",
        escape_html(&org.name)
    );

    if let Some(full_name) = &org.full_name {
        info_text.push_str(&format!("<i>{}</i>\n", escape_html(full_name)));
    }

    info_text.push('\n');

    if let Some(description) = &org.description {
        info_text.push_str(&format!("{}\n\n", escape_html(description)));
    }

    if let Some(mission) = &org.mission {
        info_text.push_str(&format!(
            "📜 <b>Misión:</b>\n{}\n\n",
            escape_html(mission)
        ));
    }

    if let Some(vision) = &org.vision {
        info_text.push_str(&format!(
            "🔭 <b>Visión:</b>\n{}\n\n",
            escape_html(vision)
        ));
    }

    // Datos de contacto
    let mut contact_info = String::new();
    if let Some(address) = &org.address {
        contact_info.push_str(&format!("📍 {}\n", escape_html(address)));
    }
    if let Some(city) = &org.city {
        contact_info.push_str(&format!("🏙️ {}", escape_html(city)));
        if let Some(province) = &org.province {
            contact_info.push_str(&format!(", {}", escape_html(province)));
        }
        if let Some(cp) = &org.postal_code {
            contact_info.push_str(&format!(" ({})", escape_html(cp)));
        }
        contact_info.push('\n');
    }
    if let Some(phone) = &org.phone {
        contact_info.push_str(&format!("📞 {}\n", escape_html(phone)));
    }
    if let Some(email) = &org.email {
        contact_info.push_str(&format!("📧 {}\n", escape_html(email)));
    }
    if let Some(website) = &org.website {
        contact_info.push_str(&format!("🌐 {}\n", escape_html(website)));
    }

    if !contact_info.is_empty() {
        info_text.push_str(&format!(
            "📋 <b>Contacto:</b>\n{}",
            contact_info
        ));
    }

    if let Some(benefits) = &org.benefits {
        info_text.push_str(&format!(
            "\n✨ <b>Beneficios de ser miembro:</b>\n{}\n",
            escape_html(benefits)
        ));
    }

    if let Some(reg_info) = &org.registration_info {
        info_text.push_str(&format!(
            "\n📝 <b>Información de registro:</b>\n{}\n",
            escape_html(reg_info)
        ));
    }

    bot.send_message(msg.chat.id, info_text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;

    Ok(())
}

// ===== GESTIÓN DE USUARIOS =====

pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    if let Some(ref data) = q.data {
        let chat_id = q.message.as_ref().map(|m| m.chat.id);

        if data.starts_with("search:field:") {
            const PREFIX_LEN: usize = "search:field:".len();
            let field_str = &data[PREFIX_LEN..];
            let (field, label) = match field_str {
                "name" => (SearchField::Name, "Nombre"),
                "address" => (SearchField::Address, "Dirección"),
                "service" => (SearchField::Service, "Servicio"),
                "city" => (SearchField::City, "Ciudad"),
                "all" => (SearchField::All, "Todos los campos"),
                _ => {
                    bot.answer_callback_query(q.id).await?;
                    return Ok(());
                }
            };

            if let Some(chat_id) = chat_id {
                dialogue.update(BotDialogueState::Search(SearchState::WaitingForQuery { field })).await?;
                bot.send_message(
                    chat_id,
                    format!("Escribe el término de búsqueda para <b>{}</b>:", label)
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            bot.answer_callback_query(q.id).await?;
            return Ok(());
        }

        if data.starts_with("search:details:") {
            const PREFIX_LEN: usize = "search:details:".len();
            if let Ok(empresa_id) = data[PREFIX_LEN..].parse::<i64>() {
                if let Some(chat_id) = chat_id {
                    handle_search_details_callback(bot.clone(), chat_id, state, empresa_id).await?;
                }
            }
            bot.answer_callback_query(q.id).await?;
            return Ok(());
        }

        if data.starts_with("search:again:") {
            const PREFIX_LEN: usize = "search:again:".len();
            let field_str = &data[PREFIX_LEN..];
            let (field, label) = match field_str {
                "name" => (SearchField::Name, "Nombre"),
                "address" => (SearchField::Address, "Dirección"),
                "service" => (SearchField::Service, "Servicio"),
                "city" => (SearchField::City, "Ciudad"),
                "all" => (SearchField::All, "Todos los campos"),
                _ => {
                    bot.answer_callback_query(q.id).await?;
                    return Ok(());
                }
            };

            if let Some(chat_id) = chat_id {
                dialogue.update(BotDialogueState::Search(SearchState::WaitingForQuery { field })).await?;
                bot.send_message(
                    chat_id,
                    format!("Escribe el término de búsqueda para <b>{}</b>:", label)
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            bot.answer_callback_query(q.id).await?;
            return Ok(());
        }

        if data.starts_with("set_type:") {
            let data = data.clone();
            let user_type = &data[9..];

            if let Some(user) = state
                .db
                .get_user(q.from.id.0 as i64)
                .await?
            {
                state.db.set_user_type(user.telegram_id, user_type).await?;

                let response = if user_type == "internal" {
                    // Aviso de membresía para empresas/autónomos
                    format!(
                        "✅ Has seleccionado <b>empresa/autónomo</b>\n\n\
                        📋 <b>Para registrarte necesitas:</b>\n\
                        • Ser miembro de <b>{}</b>\n\n\
                        ⏳ <b>Proceso de aprobación:</b>\n\
                        • Revisaremos tu solicitud en 24-48 horas\n\
                        • Te notificaremos cuando sea aprobada\n\n\
                        ¿Deseas continuar con el registro?",
                        state.config.bot.name
                    )
                } else {
                    "✅ Ahora estás registrado como <b>particular</b>\n\n\
                     Puedes buscar servicios con /buscar.".to_string()
                };

                if let Some(chat_id) = chat_id {
                    if user_type == "internal" {
                        // Mostrar botones para continuar o ver opciones de membresía
                        let keyboard = InlineKeyboardMarkup::new(vec![
                            vec![
                                InlineKeyboardButton::callback("✅ Sí, soy miembro", "register:member"),
                                InlineKeyboardButton::callback("💳 No soy miembro", "register:non_member"),
                            ],
                            vec![
                                InlineKeyboardButton::callback("❌ Cancelar", "register:cancel"),
                            ],
                        ]);
                        
                        bot.send_message(chat_id, response)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .reply_markup(keyboard)
                            .await?;
                    } else {
                        bot.send_message(chat_id, response)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .await?;
                    }
                }
            }
        } else if data.starts_with("register:") {
            // Manejar respuestas de registro
            handle_register_callback(bot.clone(), q.clone(), state, data).await?;
        } else if data.starts_with("menu:") || data.starts_with("misdatos:") {
            // Manejar menú principal y mis datos
            handle_menu_callback(bot.clone(), q.clone(), state, dialogue, data).await?;
        } else if data == "help" {
            if let Some(chat_id) = chat_id {
                let help_msg = "¿Necesitas ayuda? Usa el comando /help para ver todas las opciones.";
                bot.send_message(chat_id, help_msg).await?;
            }
        }

        // Manejar callbacks del wizard de registro
        else if data.starts_with("wiz:") {
            handle_wizard_callback(bot, q.clone(), state, data).await?;
            return Ok(());
        }

        // Manejar callbacks del menú de difusiones
        else if data.starts_with("broadcast:") {
            handle_broadcast_menu_callback(bot.clone(), q.clone(), state, data).await?;
            return Ok(());
        }
        
        // Manejar callbacks de compra de créditos
        else if data.starts_with("buy_pack:") || data == "calc_needs" {
            handle_buy_pack_callback(bot.clone(), q.clone(), state, data).await?;
            return Ok(());
        }

        // Responder al callback
        bot.answer_callback_query(q.id).await?;
    }

    Ok(())
}

// ===== WIZARD DE REGISTRO =====

async fn handle_wizard_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    data: &str,
) -> anyhow::Result<()> {
    let user_id = q.from.id.0 as i64;
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    let message_id = q.message.as_ref().map(|m| m.id);
    
    if data == "wiz:cancel" {
        wizard::clear_wizard(user_id);
        if let Some(chat_id) = chat_id {
            if let Some(message_id) = message_id {
                bot.edit_message_text(chat_id, message_id, "❌ Registro cancelado.").await?;
            }
        }
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }
    
    if let Some(wiz_state) = wizard::get_wizard_state(user_id) {
        match wiz_state.step {
            WizardStep::AskType => {
                if data == "wiz:type:empresa" {
                    let mut new_data = wiz_state.data;
                    new_data.user_type = Some("Empresa".to_string());
                    wizard::update_wizard_data(user_id, new_data);
                    wizard::set_wizard_step(user_id, WizardStep::AskName);
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id, 
                                message_id,
                                "🏢 <b>Registro de empresa</b>\n\n\
                                Paso 2/6: ¿Cuál es el <b>nombre</b> de tu empresa?\n\n\
                                Escribe el nombre y presiona enviar ✈️"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                            ]]))
                            .await?;
                        }
                    }
                } else if data == "wiz:type:autonomo" {
                    let mut new_data = wiz_state.data;
                    new_data.user_type = Some("Autónomo".to_string());
                    wizard::update_wizard_data(user_id, new_data);
                    wizard::set_wizard_step(user_id, WizardStep::AskName);
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id, 
                                message_id,
                                "👤 <b>Registro de autónomo</b>\n\n\
                                Paso 2/6: ¿Cuál es tu <b>nombre profesional</b>?\n\n\
                                Escribe el nombre y presiona enviar ✈️"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                            ]]))
                            .await?;
                        }
                    }
                }
            }
            WizardStep::AskCifChoice => {
                if data == "wiz:cif:yes" {
                    wizard::set_wizard_step(user_id, WizardStep::AskCif);
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id, 
                                message_id,
                                "📝 Escribe tu <b>CIF/NIF</b>:"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                            ]]))
                            .await?;
                        }
                    }
                } else if data == "wiz:cif:no" {
                    wizard::set_wizard_step(user_id, WizardStep::AskContactChoice);
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id, 
                                message_id,
                                "⏭️ CIF omitido\n\n\
                                Paso 5/6: ¿Cómo quieres que te contacten?"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(wizard_contact_keyboard())
                            .await?;
                        }
                    }
                }
            }
            WizardStep::AskContactChoice => {
                match data {
                    "wiz:contact:phone" => {
                        // Guardar elección del contacto
                        let new_data = RegistrationData {
                            contact_method: Some("phone".to_string()),
                            ..wiz_state.data.clone()
                        };
                        wizard::update_wizard_data(user_id, new_data);
                        wizard::set_wizard_step(user_id, WizardStep::AskPhone);
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id, 
                                    message_id,
                                    "📱 Escribe tu <b>número de teléfono</b>:"
                                )
                                .parse_mode(ParseMode::Html)
                                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                                ]]))
                                .await?;
                            }
                        }
                    }
                    "wiz:contact:email" => {
                        // Guardar elección del contacto
                        let new_data = RegistrationData {
                            contact_method: Some("email".to_string()),
                            ..wiz_state.data.clone()
                        };
                        wizard::update_wizard_data(user_id, new_data);
                        wizard::set_wizard_step(user_id, WizardStep::AskEmail);
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id, 
                                    message_id,
                                    "📧 Escribe tu <b>email</b>:"
                                )
                                .parse_mode(ParseMode::Html)
                                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                                ]]))
                                .await?;
                            }
                        }
                    }
                    "wiz:contact:both" => {
                        // Guardar elección del contacto
                        let new_data = RegistrationData {
                            contact_method: Some("both".to_string()),
                            ..wiz_state.data.clone()
                        };
                        wizard::update_wizard_data(user_id, new_data);
                        wizard::set_wizard_step(user_id, WizardStep::AskPhone);
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id, 
                                    message_id,
                                    "📱 Escribe tu <b>número de teléfono</b> (luego pediré el email):"
                                )
                                .parse_mode(ParseMode::Html)
                                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                                ]]))
                                .await?;
                            }
                        }
                    }
                    "wiz:contact:none" => {
                        wizard::set_wizard_step(user_id, WizardStep::Confirm);
                        let summary = wiz_state.data.to_summary();
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id, 
                                    message_id,
                                    format!("{}\n\n¿Todo correcto?", summary)
                                )
                                .parse_mode(ParseMode::Html)
                                .reply_markup(wizard_confirm_keyboard())
                                .await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            WizardStep::Confirm => {
                if data == "wiz:confirm" {
                    // Determinar tipo de negocio para el usuario
                    let user_type = match wiz_state.data.user_type {
                        Some(ref t) if t == "Empresa" => "internal",
                        Some(ref t) if t == "Autónomo" => "internal",
                        _ => "external",
                    };
                    
                    // Determinar tipo para la tabla empresas
                    let business_type = match wiz_state.data.user_type {
                        Some(ref t) if t == "Empresa" => "company",
                        _ => "autonomous",
                    };
                    
                    if let Some(chat_id) = chat_id {
                        // Actualizar tipo de usuario
                        state.db.set_user_type(chat_id.0, user_type).await?;
                        
                        // Marcar como miembro de la organización (el usuario confirmó que es miembro)
                        state.db.set_user_member_status(user_id, true).await?;
                        
                        // Crear o actualizar la empresa con centros y servicios en la base de datos
                        if let Some(ref name) = wiz_state.data.name {
                            match state.db.upsert_business_complete(
                                user_id,
                                business_type,
                                name,
                                wiz_state.data.description.as_deref(),
                                wiz_state.data.cif.as_deref(),
                                wiz_state.data.phone.as_deref(),
                                wiz_state.data.email.as_deref(),
                                wiz_state.data.centros.clone(),
                                wiz_state.data.servicios.clone(),
                            ).await {
                                Ok(empresa_id) => {
                                    tracing::info!("Empresa creada/actualizada con ID: {} para usuario {}", empresa_id, user_id);
                                }
                                Err(e) => {
                                    tracing::error!("Error creando/actualizando empresa para usuario {}: {}", user_id, e);
                                    // Continuamos para no bloquear al usuario, pero logueamos el error
                                }
                            }
                        }
                    }
                    
                    wizard::clear_wizard(user_id);
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            // Construir mensaje de confirmación con resumen
                            let mut mensaje = String::from(
                                "✅ <b>¡Registro completado!</b>\n\n\
                                Tu solicitud ha sido registrada y está pendiente de aprobación.\n"
                            );
                            
                            if !wiz_state.data.centros.is_empty() {
                                mensaje.push_str(&format!("🏢 Centros registrados: {}\n", wiz_state.data.centros.len()));
                            }
                            if !wiz_state.data.servicios.is_empty() {
                                mensaje.push_str(&format!("🛎️ Servicios/productos: {}\n", wiz_state.data.servicios.len()));
                            }
                            
                            mensaje.push_str("\nTe notificaremos cuando sea aprobado.");
                            
                            bot.edit_message_text(chat_id, message_id, mensaje)
                                .parse_mode(ParseMode::Html)
                                .await?;
                        }
                    }
                }
            }
            // ===== HANDLERS DE CENTROS =====
            WizardStep::AskAddCentro => {
                if data == "wiz:centro:add" {
                    wizard::set_wizard_step(user_id, WizardStep::CentroAskNombre);
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                "🏢 <b>Nuevo centro</b>\n\n¿Cuál es el <b>nombre</b> del centro?\n\
                                (Ej: Oficina Principal, Sede Madrid, etc.)"
                            )
                            .parse_mode(ParseMode::Html)
                            .await?;
                        }
                    }
                } else if data == "wiz:centro:skip" {
                    // Saltar a servicios
                    wizard::set_wizard_step(user_id, WizardStep::AskAddServicio);
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                "🛎️ <b>Servicios y Productos</b>\n\n\
                                ¿Deseas añadir un servicio o producto?"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![
                                    InlineKeyboardButton::callback("✅ Sí, añadir", "wiz:servicio:add"),
                                    InlineKeyboardButton::callback("⏭️ No, finalizar", "wiz:servicio:skip"),
                                ],
                            ]))
                            .await?;
                        }
                    }
                }
            }
            WizardStep::CentroConfirm => {
                if data == "wiz:centro:save" {
                    if let Some(centro) = wiz_state.data.temp_centro.clone() {
                        let mut centros = wiz_state.data.centros.clone();
                        centros.push(centro);
                        let mut new_data = wiz_state.data.clone();
                        new_data.centros = centros;
                        new_data.temp_centro = None;
                        wizard::update_wizard_data(user_id, new_data.clone());
                        wizard::set_wizard_step(user_id, WizardStep::AskAddCentro);
                        
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id,
                                    message_id,
                                    format!("✅ Centro guardado. Total: {}\n\n¿Añadir otro centro?", new_data.centros.len())
                                )
                                .reply_markup(InlineKeyboardMarkup::new(vec![
                                    vec![
                                        InlineKeyboardButton::callback("✅ Sí", "wiz:centro:add"),
                                        InlineKeyboardButton::callback("⏭️ No, continuar", "wiz:centro:skip"),
                                    ],
                                ]))
                                .await?;
                            }
                        }
                    }
                } else if data == "wiz:centro:discard" {
                    let new_data = RegistrationData {
                        temp_centro: None,
                        temp_servicio: wiz_state.data.temp_servicio.clone(),
                        user_type: wiz_state.data.user_type.clone(),
                        name: wiz_state.data.name.clone(),
                        description: wiz_state.data.description.clone(),
                        cif: wiz_state.data.cif.clone(),
                        phone: wiz_state.data.phone.clone(),
                        email: wiz_state.data.email.clone(),
                        centros: wiz_state.data.centros.clone(),
                        servicios: wiz_state.data.servicios.clone(),
                        website: wiz_state.data.website.clone(),
                        address: wiz_state.data.address.clone(),
                        city: wiz_state.data.city.clone(),
                        province: wiz_state.data.province.clone(),
                        postal_code: wiz_state.data.postal_code.clone(),
                        category: wiz_state.data.category.clone(),
                        logo_url: wiz_state.data.logo_url.clone(),
                        completed: wiz_state.data.completed,
                        contact_method: wiz_state.data.contact_method.clone(),
                    };
                    wizard::update_wizard_data(user_id, new_data);
                    wizard::set_wizard_step(user_id, WizardStep::AskAddCentro);
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                "🗑️ Centro descartado.\n\n¿Añadir otro centro?"
                            )
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![
                                    InlineKeyboardButton::callback("✅ Sí", "wiz:centro:add"),
                                    InlineKeyboardButton::callback("⏭️ No, continuar", "wiz:centro:skip"),
                                ],
                            ]))
                            .await?;
                        }
                    }
                }
            }
            // ===== HANDLERS DE SERVICIOS =====
            WizardStep::AskAddServicio => {
                if data == "wiz:servicio:add" {
                    wizard::set_wizard_step(user_id, WizardStep::ServicioAskTipo);
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                "🛎️ <b>Nuevo servicio/producto</b>\n\n¿Es un <b>servicio</b> o un <b>producto</b>?"
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![
                                    InlineKeyboardButton::callback("🔧 Servicio", "wiz:servicio:tipo:servicio"),
                                    InlineKeyboardButton::callback("📦 Producto", "wiz:servicio:tipo:bien"),
                                ],
                            ]))
                            .await?;
                        }
                    }
                } else if data == "wiz:servicio:skip" {
                    // Ir a confirmación final
                    wizard::set_wizard_step(user_id, WizardStep::Confirm);
                    let summary = wiz_state.data.to_summary();
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                format!("{}\n\n✅ <b>Registro completo</b>\n\n¿Todo correcto?", summary)
                            )
                            .parse_mode(ParseMode::Html)
                            .reply_markup(wizard_confirm_keyboard())
                            .await?;
                        }
                    }
                }
            }
            WizardStep::ServicioAskTipo => {
                let tipo = if data == "wiz:servicio:tipo:servicio" {
                    "servicio"
                } else if data == "wiz:servicio:tipo:bien" {
                    "bien"
                } else {
                    "servicio"
                };
                
                let temp_servicio = crate::wizard::ServicioPendiente {
                    tipo: tipo.to_string(),
                    ..Default::default()
                };
                let new_data = RegistrationData {
                    temp_servicio: Some(temp_servicio),
                    user_type: wiz_state.data.user_type.clone(),
                    name: wiz_state.data.name.clone(),
                    description: wiz_state.data.description.clone(),
                    cif: wiz_state.data.cif.clone(),
                    phone: wiz_state.data.phone.clone(),
                    email: wiz_state.data.email.clone(),
                    centros: wiz_state.data.centros.clone(),
                    servicios: wiz_state.data.servicios.clone(),
                    temp_centro: wiz_state.data.temp_centro.clone(),
                    website: wiz_state.data.website.clone(),
                    address: wiz_state.data.address.clone(),
                    city: wiz_state.data.city.clone(),
                    province: wiz_state.data.province.clone(),
                    postal_code: wiz_state.data.postal_code.clone(),
                    category: wiz_state.data.category.clone(),
                    logo_url: wiz_state.data.logo_url.clone(),
                    completed: wiz_state.data.completed,
                    contact_method: wiz_state.data.contact_method.clone(),
                };
                wizard::update_wizard_data(user_id, new_data);
                wizard::set_wizard_step(user_id, WizardStep::ServicioAskCategoria);
                
                if let Some(chat_id) = chat_id {
                    if let Some(message_id) = message_id {
                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            format!("✅ Tipo: <b>{}</b>\n\n¿Qué <b>categoría</b> describe mejor este {}?\n\
                            Ejemplos: Informática, Construcción, Limpieza, Consultoría...", tipo, tipo)
                        )
                        .parse_mode(ParseMode::Html)
                        .await?;
                    }
                }
            }
            WizardStep::ServicioConfirm => {
                if data == "wiz:servicio:save" {
                    if let Some(servicio) = wiz_state.data.temp_servicio.clone() {
                        let mut servicios = wiz_state.data.servicios.clone();
                        servicios.push(servicio);
                        let new_data = RegistrationData {
                            servicios,
                            temp_servicio: None,
                            temp_centro: wiz_state.data.temp_centro.clone(),
                            user_type: wiz_state.data.user_type.clone(),
                            name: wiz_state.data.name.clone(),
                            description: wiz_state.data.description.clone(),
                            cif: wiz_state.data.cif.clone(),
                            phone: wiz_state.data.phone.clone(),
                            email: wiz_state.data.email.clone(),
                            centros: wiz_state.data.centros.clone(),
                            website: wiz_state.data.website.clone(),
                            address: wiz_state.data.address.clone(),
                            city: wiz_state.data.city.clone(),
                            province: wiz_state.data.province.clone(),
                            postal_code: wiz_state.data.postal_code.clone(),
                            category: wiz_state.data.category.clone(),
                            logo_url: wiz_state.data.logo_url.clone(),
                            completed: wiz_state.data.completed,
                            contact_method: wiz_state.data.contact_method.clone(),
                        };
                        wizard::update_wizard_data(user_id, new_data.clone());
                        wizard::set_wizard_step(user_id, WizardStep::AskAddServicio);
                        
                        if let Some(chat_id) = chat_id {
                            if let Some(message_id) = message_id {
                                bot.edit_message_text(
                                    chat_id,
                                    message_id,
                                    format!("✅ Servicio guardado. Total: {}\n\n¿Añadir otro servicio/producto?", new_data.servicios.len())
                                )
                                .reply_markup(InlineKeyboardMarkup::new(vec![
                                    vec![
                                        InlineKeyboardButton::callback("✅ Sí", "wiz:servicio:add"),
                                        InlineKeyboardButton::callback("⏭️ No, finalizar", "wiz:servicio:skip"),
                                    ],
                                ]))
                                .await?;
                            }
                        }
                    }
                } else if data == "wiz:servicio:discard" {
                    let new_data = RegistrationData {
                        temp_servicio: None,
                        temp_centro: wiz_state.data.temp_centro.clone(),
                        user_type: wiz_state.data.user_type.clone(),
                        name: wiz_state.data.name.clone(),
                        description: wiz_state.data.description.clone(),
                        cif: wiz_state.data.cif.clone(),
                        phone: wiz_state.data.phone.clone(),
                        email: wiz_state.data.email.clone(),
                        centros: wiz_state.data.centros.clone(),
                        servicios: wiz_state.data.servicios.clone(),
                        website: wiz_state.data.website.clone(),
                        address: wiz_state.data.address.clone(),
                        city: wiz_state.data.city.clone(),
                        province: wiz_state.data.province.clone(),
                        postal_code: wiz_state.data.postal_code.clone(),
                        category: wiz_state.data.category.clone(),
                        logo_url: wiz_state.data.logo_url.clone(),
                        completed: wiz_state.data.completed,
                        contact_method: wiz_state.data.contact_method.clone(),
                    };
                    wizard::update_wizard_data(user_id, new_data);
                    wizard::set_wizard_step(user_id, WizardStep::AskAddServicio);
                    
                    if let Some(chat_id) = chat_id {
                        if let Some(message_id) = message_id {
                            bot.edit_message_text(
                                chat_id,
                                message_id,
                                "🗑️ Servicio descartado.\n\n¿Añadir otro servicio/producto?"
                            )
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![
                                    InlineKeyboardButton::callback("✅ Sí", "wiz:servicio:add"),
                                    InlineKeyboardButton::callback("⏭️ No, finalizar", "wiz:servicio:skip"),
                                ],
                            ]))
                            .await?;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

fn wizard_contact_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("📱 Teléfono", "wiz:contact:phone"),
            InlineKeyboardButton::callback("📧 Email", "wiz:contact:email"),
        ],
        vec![
            InlineKeyboardButton::callback("📱📧 Ambos", "wiz:contact:both"),
            InlineKeyboardButton::callback("⏭️ Omitir", "wiz:contact:none"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

fn wizard_confirm_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("✅ Confirmar registro", "wiz:confirm"),
            InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

// ===== CHAT CON IA =====

pub async fn handle_chat(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    text: String,
) -> anyhow::Result<()> {
    // Verificar límite de uso
    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;

    if user.is_none() {
        bot.send_message(
            msg.chat.id,
            "Por favor, usa /start primero para registrarte.",
        )
        .await?;
        return Ok(());
    }

    // Verificar si la IA está disponible
    let ia = match &state.ia {
        Some(ia) => ia,
        None => {
            bot.send_message(
                msg.chat.id,
                "La funcionalidad de IA no está disponible en este momento.",
            )
            .await?;
            return Ok(());
        }
    };

    let user = user.unwrap();
    let today = Utc::now().naive_utc().date();
    let current_usage = state.db.get_ia_usage(user.telegram_id, today).await?;

    if current_usage >= state.config.limits.max_ia_messages_per_day {
        bot.send_message(
            msg.chat.id,
            format!(
                "Has alcanzado el límite de {} mensajes de IA por hoy. Vuelve mañana!",
                state.config.limits.max_ia_messages_per_day
            ),
        )
        .await?;
        return Ok(());
    }

    // Mostrar typing
    bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
        .await?;

    // Contexto del sistema basado en la entidad
    let context = format!(
        "Eres el asistente virtual de {}. Ayuda a usuarios con información sobre servicios, empresas y autónomos. Responde siempre en español, de forma útil y concisa.",
        state.config.bot.name  // No necesita escape aquí, es solo para la IA
    );

    // Llamar a la IA
    match ia.chat(&text, Some(&context)).await {
        Ok(response) => {
            // Incrementar contador
            let new_count = state.db.increment_ia_usage(user.telegram_id, today).await?;
            let remaining = state.config.limits.max_ia_messages_per_day - new_count;

            let full_response = format!("{}\n\n<i>Quedan {} mensajes hoy</i>", response, remaining);

            bot.send_message(msg.chat.id, full_response)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        Err(e) => {
            tracing::error!("IA error: {}", e);
            bot.send_message(
                msg.chat.id,
                "Lo siento, hubo un error al procesar tu mensaje. Inténtalo de nuevo más tarde.",
            )
            .await?;
        }
    }

    Ok(())
}

// ===== REGISTRO DE EMPRESAS =====

pub async fn handle_register(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if !state.config.features.enable_company_registration {
        bot.send_message(
            msg.chat.id,
            "El registro de empresas no está habilitado en este momento.",
        )
        .await?;
        return Ok(());
    }

    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;

    if user.is_none() {
        bot.send_message(msg.chat.id, "Por favor, usa /start primero.").await?;
        return Ok(());
    }

    // Verificar si ya tiene empresa registrada (activa o pendiente)
    let existing = state.db.get_empresas_by_user(telegram_id).await?;
    let pending = existing.iter().find(|e| !e.activa);
    let active = existing.iter().find(|e| e.activa);
    
    if active.is_some() {
        bot.send_message(
            msg.chat.id,
            "Ya tienes una empresa registrada y activa.\n\
            Usa /misdatos para ver tu información.",
        )
        .await?;
        return Ok(());
    }
    
    if pending.is_some() {
        bot.send_message(
            msg.chat.id,
            "⏳ Tu solicitud de registro está pendiente de aprobación.\n\
            Un administrador revisará tu información en 24-48 horas.\n\
            Te notificaremos cuando sea aprobada.",
        )
        .await?;
        return Ok(());
    }

    // Mostrar aviso de membresía antes de iniciar el wizard
    bot.send_message(
        msg.chat.id,
        format!(
            "📋 <b>Requisitos para registrarte</b>\n\n\
            • Debes ser miembro de <b>{}</b>\n\n\
            ⏳ <b>Proceso de aprobación:</b>\n\
            • Revisaremos tu solicitud en 24-48 horas\n\
            • Te notificaremos cuando sea aprobada\n\n\
            ¿Eres miembro de {}?",
            state.config.bot.name,
            state.config.bot.name
        )
    )
    .parse_mode(ParseMode::Html)
    .reply_markup(InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("✅ Sí, soy miembro", "register:member"),
            InlineKeyboardButton::callback("💳 No soy miembro", "register:non_member"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar", "register:cancel"),
        ],
    ]))
    .await?;

    Ok(())
}

fn wizard_type_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("🏢 Empresa", "wiz:type:empresa"),
            InlineKeyboardButton::callback("👤 Autónomo", "wiz:type:autonomo"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

#[allow(dead_code)]
pub async fn handle_business_registration_text(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    text: String,
) -> anyhow::Result<bool> {
    // Detectar si es un mensaje de registro de empresa
    if !text.starts_with("TIPO:") && !text.starts_with("tipo:") {
        return Ok(false); // No es registro, continuar con otro handler
    }

    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;

    if user.is_none() {
        return Ok(false);
    }

    let user = user.unwrap();
    let lines: Vec<&str> = text.lines().collect();

    let mut business_type = "";
    let mut name = "";
    let mut description = None;
    let mut cif = None;
    let mut phone = None;
    let mut email = None;

    for line in lines {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        let key = parts[0].trim().to_lowercase();
        let value = parts[1].trim();

        match key.as_str() {
            "tipo" => business_type = if value == "empresa" { "company" } else { "autonomous" },
            "nombre" => name = value,
            "descripcion" => description = Some(value),
            "cif" => cif = Some(value),
            "telefono" => phone = Some(value),
            "email" => email = Some(value),
            _ => {}
        }
    }

    if business_type.is_empty() || name.is_empty() {
        bot.send_message(
            msg.chat.id,
            "❌ Faltan datos obligatorios. Asegúrate de incluir TIPO y NOMBRE.",
        )
        .await?;
        return Ok(true);
    }

    match state
        .db
        .create_business(user.telegram_id, business_type, name, description, cif, phone, email)
        .await
    {
        Ok(id) => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ ¡Solicitud recibida!\n\n\
                    ID de solicitud: {}\n\
                    Nombre: {}\n\n\
                    ⏳ Tu registro está <b>pendiente de aprobación</b>.\n\
                    Un administrador revisará tu información en 24-48 horas.\n\
                    Te notificaremos cuando puedas empezar a publicar servicios.",
                    id, name
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        Err(e) => {
            tracing::error!("Error creando empresa: {}", e);
            bot.send_message(
                msg.chat.id,
                "❌ Error al registrar la empresa. Inténtalo de nuevo.",
            )
            .await?;
        }
    }

    Ok(true)
}

// ===== BÚSQUEDA DE SERVICIOS =====

pub async fn handle_search(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    let text = "🔍 <b>Buscar servicios</b>\n\n\
        Selecciona el campo por el que quieres buscar:";

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🏢 Por nombre", "search:field:name"),
            InlineKeyboardButton::callback("📍 Por dirección", "search:field:address"),
        ],
        vec![
            InlineKeyboardButton::callback("🛎️ Por servicio", "search:field:service"),
            InlineKeyboardButton::callback("🏙️ Por ciudad", "search:field:city"),
        ],
        vec![
            InlineKeyboardButton::callback("🔍 Todo", "search:field:all"),
        ],
    ]);

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    dialogue.update(BotDialogueState::Search(SearchState::WaitingForField)).await?;

    Ok(())
}

pub async fn handle_search_query(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    query: String,
) -> anyhow::Result<()> {
    let mut category = None;
    let mut search_terms = query.clone();

    // Extraer categoría si existe
    if let Some(cat_pos) = query.to_lowercase().find("categoria:") {
        let after_cat = &query[cat_pos + 10..];
        if let Some(space_pos) = after_cat.find(' ') {
            category = Some(after_cat[..space_pos].trim());
            search_terms = format!(
                "{}{}",
                &query[..cat_pos].trim(),
                &after_cat[space_pos..].trim()
            );
        } else {
            category = Some(after_cat.trim());
            search_terms = query[..cat_pos].trim().to_string();
        }
    }

    let search_terms = search_terms.trim();
    if search_terms.is_empty() && category.is_none() {
        bot.send_message(msg.chat.id, "Por favor, indica qué quieres buscar. Ejemplo: `/buscar consultoría`").await?;
        return Ok(());
    }

    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;
    let is_admin = state.config.bot.admins.contains(&telegram_id);
    let is_member = user.as_ref().map(|u| u.is_member).unwrap_or(false);
    let can_search_users = is_admin || is_member;

    let mut response = String::new();
    let mut total_results = 0;

    // Buscar empresas
    let businesses = state.db.search_businesses(search_terms).await?;
    if !businesses.is_empty() {
        response.push_str(&format!("🏢 <b>Empresas encontradas ({}):</b>\n\n", businesses.len()));
        for biz in businesses.iter().take(5) {
            let nombre = biz.nombre_comercial.as_ref()
                .unwrap_or(&biz.nombre_fiscal);
            response.push_str(&format!(
                "• <b>{}</b>\n  └ {}\n\n",
                escape_html(nombre),
                escape_html(biz.descripcion.as_deref().unwrap_or("Sin descripción"))
            ));
        }
        total_results += businesses.len();
    }

    // Buscar servicios
    let services = state.db.search_services(search_terms, category).await?;
    if !services.is_empty() {
        if !response.is_empty() {
            response.push('\n');
        }
        response.push_str(&format!("📋 <b>Servicios encontrados ({}):</b>\n\n", services.len()));
        for (service, business) in services.iter().take(5) {
            let nombre_biz = business.nombre_comercial.as_ref()
                .unwrap_or(&business.nombre_fiscal);
            response.push_str(&format!(
                "• <b>{}</b> ({})\n  └ {}\n\n",
                escape_html(&service.nombre),
                escape_html(nombre_biz),
                escape_html(service.descripcion.as_deref().unwrap_or("Sin descripción"))
            ));
        }
        total_results += services.len();
    }

    // Buscar usuarios (solo si es admin o miembro)
    if can_search_users {
        let users = state.db.search_users(search_terms).await?;
        if !users.is_empty() {
            if !response.is_empty() {
                response.push('\n');
            }
            response.push_str(&format!("👥 <b>Usuarios encontrados ({}):</b>\n\n", users.len()));
            for user in users.iter().take(5) {
                let member_status = if user.is_member { "✅" } else { "⚪" };
                let name = user.first_name.clone().unwrap_or_else(|| "Sin nombre".to_string());
                let username = user.username.as_deref().map(|u| format!("@{}", u)).unwrap_or_default();
                response.push_str(&format!(
                    "• {} <b>{}</b> {}\n  └ ID: <code>{}</code>\n\n",
                    member_status,
                    escape_html(&name),
                    username,
                    user.telegram_id
                ));
            }
            total_results += users.len();
        }
    }

    if total_results == 0 {
        bot.send_message(
            msg.chat.id,
            "No encontré resultados con esos criterios. Intenta con otras palabras.",
        )
        .await?;
        return Ok(());
    }

    response.push_str(&format!("\n📊 <b>Total: {} resultados</b>", total_results));

    let mut keyboard_rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    for biz in businesses.iter().take(5) {
        let nombre = biz.nombre_comercial.as_ref()
            .unwrap_or(&biz.nombre_fiscal);
        keyboard_rows.push(vec![
            InlineKeyboardButton::callback(
                format!("📋 Ver detalles: {}", nombre.chars().take(25).collect::<String>()),
                format!("search:details:{}", biz.id)
            ),
        ]);
    }

    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🔍 Nueva búsqueda", "menu:buscar"),
    ]);

    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🏠 Volver al inicio", "menu:start"),
    ]);

    let keyboard = InlineKeyboardMarkup::new(keyboard_rows);

    bot.send_message(msg.chat.id, response)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

pub async fn handle_search_by_field_query(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    field: SearchField,
    query: String,
) -> anyhow::Result<()> {
    let search_terms = query.trim();
    if search_terms.is_empty() {
        bot.send_message(msg.chat.id, "Por favor, indica qué quieres buscar. Ejemplo: `consultoría`").await?;
        return Ok(());
    }

    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;
    let is_admin = state.config.bot.admins.contains(&telegram_id);
    let is_member = user.as_ref().map(|u| u.is_member).unwrap_or(false);
    let can_search_users = is_admin || is_member;

    let mut response = String::new();
    let mut total_results = 0;

    let field_name = match field {
        SearchField::Name => "nombre",
        SearchField::Address => "dirección",
        SearchField::Service => "servicio",
        SearchField::City => "ciudad",
        SearchField::All => "todos los campos",
    };

    let businesses = match field {
        SearchField::Name => state.db.search_businesses_by_name(search_terms).await?,
        SearchField::Address => state.db.search_businesses_by_address(search_terms).await?,
        SearchField::City => state.db.search_businesses_by_city(search_terms).await?,
        SearchField::Service => state.db.search_businesses_by_service(search_terms).await?,
        SearchField::All => state.db.search_businesses(search_terms).await?,
    };

    if !businesses.is_empty() {
        response.push_str(&format!("🏢 <b>Empresas encontradas ({}):</b>\n\n", businesses.len()));
        for biz in businesses.iter().take(5) {
            let nombre = biz.nombre_comercial.as_ref()
                .unwrap_or(&biz.nombre_fiscal);
            response.push_str(&format!(
                "• <b>{}</b>\n  └ {}\n\n",
                escape_html(nombre),
                escape_html(biz.descripcion.as_deref().unwrap_or("Sin descripción"))
            ));
        }
        total_results += businesses.len();
    }

    if matches!(field, SearchField::Service | SearchField::All) {
        let services = state.db.search_services(search_terms, None).await?;
        if !services.is_empty() {
            if !response.is_empty() {
                response.push('\n');
            }
            response.push_str(&format!("📋 <b>Servicios encontrados ({}):</b>\n\n", services.len()));
            for (service, business) in services.iter().take(5) {
                let nombre_biz = business.nombre_comercial.as_ref()
                    .unwrap_or(&business.nombre_fiscal);
                response.push_str(&format!(
                    "• <b>{}</b> ({})\n  └ {}\n\n",
                    escape_html(&service.nombre),
                    escape_html(nombre_biz),
                    escape_html(service.descripcion.as_deref().unwrap_or("Sin descripción"))
                ));
            }
            total_results += services.len();
        }
    }

    if can_search_users && matches!(field, SearchField::Name | SearchField::All) {
        let users = state.db.search_users(search_terms).await?;
        if !users.is_empty() {
            if !response.is_empty() {
                response.push('\n');
            }
            response.push_str(&format!("👥 <b>Usuarios encontrados ({}):</b>\n\n", users.len()));
            for user in users.iter().take(5) {
                let member_status = if user.is_member { "✅" } else { "⚪" };
                let name = user.first_name.clone().unwrap_or_else(|| "Sin nombre".to_string());
                let username = user.username.as_deref().map(|u| format!("@{}", u)).unwrap_or_default();
                response.push_str(&format!(
                    "• {} <b>{}</b> {}\n  └ ID: <code>{}</code>\n\n",
                    member_status,
                    escape_html(&name),
                    username,
                    user.telegram_id
                ));
            }
            total_results += users.len();
        }
    }

    if total_results == 0 {
        bot.send_message(
            msg.chat.id,
            format!("No encontré resultados buscando por <b>{}</b>. Intenta con otras palabras.", field_name),
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    response.push_str(&format!("\n📊 <b>Total: {} resultados</b> (buscando por: {})", total_results, field_name));

    let mut keyboard_rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    
    for biz in businesses.iter().take(5) {
        let nombre = biz.nombre_comercial.as_ref()
            .unwrap_or(&biz.nombre_fiscal);
        keyboard_rows.push(vec![
            InlineKeyboardButton::callback(
                format!("📋 Ver detalles: {}", nombre.chars().take(25).collect::<String>()),
                format!("search:details:{}", biz.id)
            ),
        ]);
    }
    
    // Botones para buscar por diferente campo
    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🏢 Por nombre", "search:field:name"),
        InlineKeyboardButton::callback("📍 Por dirección", "search:field:address"),
    ]);
    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🛎️ Por servicio", "search:field:service"),
        InlineKeyboardButton::callback("🏙️ Por ciudad", "search:field:city"),
    ]);
    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🔍 Todos los campos", "search:field:all"),
    ]);
    keyboard_rows.push(vec![
        InlineKeyboardButton::callback("🏠 Volver al inicio", "menu:start"),
    ]);
    
    let keyboard = InlineKeyboardMarkup::new(keyboard_rows);

    bot.send_message(msg.chat.id, response)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_search_details_callback(
    bot: Bot,
    chat_id: teloxide::types::ChatId,
    state: Arc<BotState>,
    empresa_id: i64,
) -> anyhow::Result<()> {
    let empresa = match state.db.get_empresa_by_id(empresa_id).await? {
        Some(e) => e,
        None => {
            bot.send_message(chat_id, "❌ No se encontró la empresa.").await?;
            return Ok(());
        }
    };
    
    let centros = state.db.get_centros_by_empresa(empresa.id).await.unwrap_or_default();
    let servicios = state.db.get_servicios_by_empresa(empresa.id).await.unwrap_or_default();
    
    let mut texto = format!(
        "🏢 <b>{}</b>\n\n\
        <b>NIF/CIF:</b> {}\n\
        <b>Descripción:</b> {}\n\
        <b>Teléfono:</b> {}\n\
        <b>Email:</b> {}\n\
        <b>Web:</b> {}\n\
        <b>Dirección:</b> {}\n\
        <b>Ciudad:</b> {}\n\
        <b>Provincia:</b> {}\n\
        <b>Código Postal:</b> {}",
        escape_html(&empresa.nombre_fiscal),
        empresa.cif_nif.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
        empresa.descripcion.as_deref().map(escape_html).unwrap_or_else(|| "Sin descripción".to_string()),
        empresa.telefono.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
        empresa.email.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
        empresa.web.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
        empresa.direccion.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
        empresa.ciudad.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
        empresa.provincia.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
        empresa.codigo_postal.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string())
    );
    
    if centros.is_empty() {
        texto.push_str("\n\n📍 <b>Centros de trabajo:</b>\n   No hay centros registrados.");
    } else {
        texto.push_str(&format!("\n\n📍 <b>Centros de trabajo ({})</b>:", centros.len()));
        for (id, nombre) in &centros {
            texto.push_str(&format!("\n   • {} <code>[ID: {}]</code>", escape_html(nombre), id));
        }
    }
    
    if servicios.is_empty() {
        texto.push_str("\n\n📋 <b>Servicios/Productos:</b>\n   No hay servicios registrados.");
    } else {
        texto.push_str(&format!("\n\n📋 <b>Servicios/Productos ({})</b>:", servicios.len()));
        for (id, nombre, categoria) in &servicios {
            texto.push_str(&format!(
                "\n   • {} <i>({})</i> <code>[ID: {}]</code>",
                escape_html(nombre),
                escape_html(categoria),
                id
            ));
        }
    }
    
    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("⬅️ Volver a resultados", "menu:buscar"),
        ],
    ]);
    
    bot.send_message(chat_id, texto)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    
    Ok(())
}

// ===== MENSAJERÍA =====

pub async fn handle_messages(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if !state.config.features.enable_messaging {
        bot.send_message(msg.chat.id, "La mensajería no está habilitada.").await?;
        return Ok(());
    }

    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = state.db.get_user(telegram_id).await?;

    if user.is_none() {
        bot.send_message(msg.chat.id, "Por favor, usa /start primero.").await?;
        return Ok(());
    }

    let user = user.unwrap();
    let messages = state.db.get_unread_messages(user.telegram_id).await?;

    if messages.is_empty() {
        bot.send_message(msg.chat.id, "📭 No tienes mensajes nuevos.").await?;
    } else {
        let mut response = format!("<b>Tienes {} mensajes nuevos:</b>\n\n", messages.len());
        for mensaje in messages.iter().take(5) {
            response.push_str(&format!(
                "📩 <b>{}</b>\n{}\n\n",
                mensaje.asunto,
                mensaje.contenido
            ));
        }
        bot.send_message(msg.chat.id, response)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
    }

    Ok(())
}

// ===== ADMIN COMMANDS =====

/// Listar empresas pendientes de aprobación (solo admin)
pub async fn handle_pending_businesses(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

    if !state.config.bot.admins.contains(&telegram_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    let pending = state.db.get_pending_businesses().await?;

    if pending.is_empty() {
        bot.send_message(msg.chat.id, "📭 No hay empresas pendientes de aprobación.").await?;
        return Ok(());
    }

    let mut text = String::from("<b>📋 Empresas pendientes de aprobación:</b>\n\n");
    for emp in pending {
        text.push_str(&format!(
            "🆔 <b>ID:</b> {}\n\
            🏢 <b>Nombre:</b> {}\n\
            📧 <b>Email:</b> {}\n\
            📞 <b>Tel:</b> {}\n\
            📅 <b>Solicitado:</b> {}\n\n\
            ✅ Aprobar: <code>/aprobar {}</code>\n\
            ❌ Rechazar: <code>/rechazar {}</code>\n\
            ──────────────────\n\n",
            emp.id,
            emp.nombre_fiscal,
            emp.email.as_deref().unwrap_or("N/A"),
            emp.telefono.as_deref().unwrap_or("N/A"),
            emp.created_at.format("%d/%m/%Y %H:%M"),
            emp.id,
            emp.id
        ));
    }

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;

    Ok(())
}

/// Aprobar una empresa (solo admin)
pub async fn handle_approve_business(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    empresa_id: i64,
) -> anyhow::Result<()> {
    let admin_telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

    if !state.config.bot.admins.contains(&admin_telegram_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    // Obtener info de la empresa antes de aprobar
    let empresas = state.db.get_pending_businesses().await?;
    let empresa = empresas.iter().find(|e| e.id == empresa_id);

    if empresa.is_none() {
        bot.send_message(msg.chat.id, "❌ No se encontró la empresa pendiente con ese ID.").await?;
        return Ok(());
    }
    
    let empresa = empresa.unwrap();
    let empresa_nombre = empresa.nombre_fiscal.clone();
    let user_telegram_id = empresa.telegram_id;

    // Obtener centros y servicios antes de aprobar
    let centros = state.db.get_centros_by_empresa(empresa_id).await.unwrap_or_default();
    let servicios = state.db.get_servicios_by_empresa(empresa_id).await.unwrap_or_default();

    match state.db.approve_business(empresa_id).await {
        Ok(true) => {
            // Construir mensaje de confirmación para el admin
            let mut admin_msg = format!(
                "✅ <b>Empresa aprobada correctamente</b>\n\n\
                <b>Nombre:</b> {}\n\
                <b>ID:</b> <code>{}</code>\n\
                <b>Usuario:</b> <code>{}</code>\n\n",
                escape_html(&empresa_nombre),
                empresa_id,
                user_telegram_id
            );
            
            if !centros.is_empty() {
                admin_msg.push_str(&format!("🏢 <b>Centros activados:</b> {}\n", centros.len()));
                for (id, nombre) in &centros {
                    admin_msg.push_str(&format!("  • {} (ID: {})\n", escape_html(nombre), id));
                }
                admin_msg.push('\n');
            }
            
            if !servicios.is_empty() {
                admin_msg.push_str(&format!("🛎️ <b>Servicios activados:</b> {}\n", servicios.len()));
                for (id, nombre, categoria) in &servicios {
                    admin_msg.push_str(&format!("  • {} - {} (ID: {})\n", 
                        escape_html(categoria), 
                        escape_html(nombre), 
                        id
                    ));
                }
            }
            
            admin_msg.push_str("\n👤 El usuario ha sido notificado.");

            bot.send_message(msg.chat.id, admin_msg)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;

            // Construir mensaje de notificación para el usuario
            let mut user_msg = format!(
                "🎉 <b>¡Tu registro ha sido aprobado!</b>\n\n\
                Tu empresa <b>{}</b> ha sido activada.\n\n",
                escape_html(&empresa_nombre)
            );
            
            if !centros.is_empty() {
                user_msg.push_str(&format!("🏢 <b>Centros activados:</b> {}\n", centros.len()));
                for (_, nombre) in &centros {
                    user_msg.push_str(&format!("  • {}\n", escape_html(nombre)));
                }
                user_msg.push('\n');
            }
            
            if !servicios.is_empty() {
                user_msg.push_str(&format!("🛎️ <b>Servicios/productos activados:</b> {}\n", servicios.len()));
                for (_, nombre, categoria) in &servicios {
                    user_msg.push_str(&format!("  • {} ({})\n", 
                        escape_html(nombre),
                        escape_html(categoria)
                    ));
                }
                user_msg.push('\n');
            }
            
            user_msg.push_str(
                "✅ Ya puedes usar todos los comandos disponibles:\n\
                • /nuevo_servicio - Añadir más servicios\n\
                • /mis_servicios - Ver tus servicios\n\
                • /broadcast - Enviar difusiones\n\
                • /misdatos - Ver tu ficha completa\n\n\
                ¡Bienvenido/a a la red de proveedores! 🚀"
            );

            // Notificar al usuario
            let _ = bot.send_message(
                teloxide::types::ChatId(user_telegram_id),
                user_msg
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await;
        }
        Ok(false) => {
            bot.send_message(msg.chat.id, "❌ No se encontró la empresa pendiente con ese ID.").await?;
        }
        Err(e) => {
            tracing::error!("Error aprobando empresa: {}", e);
            bot.send_message(msg.chat.id, "❌ Error al aprobar la empresa.").await?;
        }
    }

    Ok(())
}

/// Rechazar/eliminar una empresa pendiente (solo admin)
pub async fn handle_reject_business(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    empresa_id: i64,
) -> anyhow::Result<()> {
    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

    if !state.config.bot.admins.contains(&telegram_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    match state.db.reject_business(empresa_id).await {
        Ok(true) => {
            bot.send_message(msg.chat.id, "❌ Solicitud rechazada y eliminada.").await?;
        }
        Ok(false) => {
            bot.send_message(msg.chat.id, "⚠️ No se encontró empresa pendiente con ese ID.").await?;
        }
        Err(e) => {
            tracing::error!("Error rechazando empresa: {}", e);
            bot.send_message(msg.chat.id, "❌ Error al rechazar la solicitud.").await?;
        }
    }

    Ok(())
}

/// Mostrar datos registrados del usuario (/misdatos)
pub async fn handle_mis_datos(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let telegram_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    
    // Obtener datos del usuario
    let user = match state.db.get_user(telegram_id).await? {
        Some(u) => u,
        None => {
            bot.send_message(
                msg.chat.id,
                "❌ No tienes datos registrados.\n\nUsa /start para registrarte."
            ).await?;
            return Ok(());
        }
    };
    
    // Verificar si es admin por config
    let is_config_admin = state.config.bot.admins.contains(&telegram_id);
    
    // Obtener empresa si existe
    let empresas = state.db.get_empresas_by_user(telegram_id).await?;
    let empresa = empresas.into_iter().next();
    
    // Determinar tipo de usuario
    let tipo_usuario = if is_config_admin {
        "🔧 Administrador"
    } else if user.is_internal {
        "Empresa/Autónomo"
    } else {
        "Particular"
    };
    
    // Determinar estado de miembro
    let miembro_status = if is_config_admin {
        "🔧 Administrador"
    } else if user.is_member {
        "✅ Sí"
    } else {
        "❌ No"
    };
    
    // Construir mensaje
    let mut texto = format!(
        "👤 <b>Tus Datos</b>\n\n\
        <b>Nombre:</b> {}\n\
        <b>Username:</b> {}\n\
        <b>Tipo:</b> {}\n\
        <b>Miembro:</b> {}\n",
        escape_html(&user.first_name.unwrap_or_default()),
        user.username.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
        tipo_usuario,
        miembro_status
    );
    
    if let Some(emp) = empresa {
        texto.push_str(&format!(
            "\n🏢 <b>Empresa Registrada</b>\n\n\
            <b>Nombre fiscal:</b> {}\n\
            <b>NIF/CIF:</b> {}\n\
            <b>Descripción:</b> {}\n\
            <b>Teléfono:</b> {}\n\
            <b>Email:</b> {}\n\
            <b>Estado:</b> {}\n",
            escape_html(&emp.nombre_fiscal),
            emp.cif_nif.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
            emp.descripcion.as_deref().map(escape_html).unwrap_or_else(|| "Sin descripción".to_string()),
            emp.telefono.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
            emp.email.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
            if emp.activa { "✅ Activa" } else { "⏳ Pendiente de aprobación" }
        ));
    } else if user.is_internal {
        texto.push_str("\n⚠️ <b>No tienes empresa registrada</b>\n\nUsa /registrar para registrar tu empresa.");
    }
    
    bot.send_message(msg.chat.id, texto)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
    
    Ok(())
}


// ===== CALLBACKS DE REGISTRO =====

async fn handle_register_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    data: &str,
) -> anyhow::Result<()> {
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    let message_id = q.message.as_ref().map(|m| m.id);
    let user_id = q.from.id.0 as i64;
    
    match data {
        "register:member" => {
            // Usuario confirma que es miembro → iniciar wizard de registro
            if let Some(chat_id) = chat_id {
                if let Some(message_id) = message_id {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        "🏢 <b>Registro de negocio</b>\n\n\
                        Paso 1/6: ¿Qué tipo de negocio eres?"
                    )
                    .parse_mode(ParseMode::Html)
                    .reply_markup(wizard_type_keyboard())
                    .await?;
                }
                wizard::start_wizard(user_id);
            }
        }
        "register:non_member" => {
            // No es miembro → verificar si se permite registro de no-miembros
            if let Some(chat_id) = chat_id {
                if let Some(message_id) = message_id {
                    // Verificar si el bot es exclusivo para miembros
                    if state.config.membership.exclusive_to_members {
                        // No se permite registro de no-miembros
                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            format!(
                                "⛔ <b>Registro exclusivo para miembros</b>\n\n\
                                Este bot es exclusivo para miembros de <b>{}</b>.\n\n\
                                Para registrarte, primero debes ser miembro de la organización.\n\
                                Contacta con la administración para más información.",
                                state.config.bot.name
                            )
                        )
                        .parse_mode(ParseMode::Html)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![
                                InlineKeyboardButton::callback("📞 Contactar admin", "menu:contact_admin"),
                            ],
                            vec![
                                InlineKeyboardButton::callback("⬅️ Volver", "register:back"),
                            ],
                        ]))
                        .await?;
                    } else {
                        // Se permite membresía de pago
                        let membership_price = state.config.membership.price.unwrap_or(9.99);
                        
                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            format!(
                                "💳 <b>Membresía del Bot</b>\n\n\
                                Si no eres miembro de <b>{}</b>, puedes obtener acceso al bot con una membresía mensual.\n\n\
                                📋 <b>Beneficios:</b>\n\
                                • Publicar tu empresa y servicios\n\
                                • Enviar difusiones al canal\n\
                                • Chat con IA ilimitado\n\
                                • Contactar con otros profesionales\n\n\
                                💰 <b>Precio:</b> {:.2}€/mes\n\n\
                                Para contratar la membresía, contacta con un administrador.",
                                state.config.bot.name,
                                membership_price
                            )
                        )
                        .parse_mode(ParseMode::Html)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![
                                InlineKeyboardButton::callback("📞 Contactar admin", "menu:contact_admin"),
                            ],
                            vec![
                                InlineKeyboardButton::callback("⬅️ Volver", "register:back"),
                            ],
                        ]))
                        .await?;
                    }
                }
            }
        }
        "register:cancel" => {
            if let Some(chat_id) = chat_id {
                if let Some(message_id) = message_id {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        "❌ Registro cancelado.\n\nPuedes registrarte más tarde con /registrar"
                    ).await?;
                }
            }
        }
        "register:back" => {
            // Volver a mostrar opciones de tipo
            if let Some(chat_id) = chat_id {
                if let Some(message_id) = message_id {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        format!(
                            "✅ Has seleccionado <b>empresa/autónomo</b>\n\n\
                            📋 <b>Para registrarte necesitas:</b>\n\
                            • Ser miembro de <b>{}</b>\n\n\
                            ⏳ <b>Proceso de aprobación:</b>\n\
                            • Revisaremos tu solicitud en 24-48 horas\n\
                            • Te notificaremos cuando sea aprobada\n\n\
                            ¿Deseas continuar con el registro?",
                            state.config.bot.name
                        )
                    )
                    .parse_mode(ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![
                            InlineKeyboardButton::callback("✅ Sí, soy miembro", "register:member"),
                            InlineKeyboardButton::callback("💳 No soy miembro", "register:non_member"),
                        ],
                        vec![
                            InlineKeyboardButton::callback("❌ Cancelar", "register:cancel"),
                        ],
                    ]))
                    .await?;
                }
            }
        }
        _ => {}
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

// ===== CALLBACKS DEL MENÚ PRINCIPAL =====

async fn handle_menu_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    dialogue: MyDialogue,
    data: &str,
) -> anyhow::Result<()> {
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    let telegram_id = q.from.id.0 as i64;
    
    match data {
        "menu:buscar" => {
            if let Some(chat_id) = chat_id {
                let text = "🔍 <b>Buscar servicios</b>\n\n\
                    Selecciona el campo por el que quieres buscar:";

                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("🏢 Por nombre", "search:field:name"),
                        InlineKeyboardButton::callback("📍 Por dirección", "search:field:address"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("🛎️ Por servicio", "search:field:service"),
                        InlineKeyboardButton::callback("🏙️ Por ciudad", "search:field:city"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("🔍 Todo", "search:field:all"),
                    ],
                ]);

                bot.send_message(chat_id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;

                dialogue.update(BotDialogueState::Search(SearchState::WaitingForField)).await?;
            }
        }
        "menu:chat" => {
            if let Some(chat_id) = chat_id {
                bot.send_message(
                    chat_id,
                    "💬 <b>Modo chat activado</b>\n\n\
                    Escribe tu mensaje y responderé usando IA.\n\n\
                    Usa /help para volver al menú principal."
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
        }
        "menu:info" => {
            // Llamar a handle_info
            if let Some(chat_id) = chat_id {
                let org = match state.db.get_organization().await {
                    Ok(o) => o,
                    Err(_) => {
                        let info_text = format!(
                            "<b>{}</b>\n\n{}",
                            state.config.bot.name,
                            state.config.bot.description
                        );
                        bot.send_message(chat_id, info_text)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        return Ok(());
                    }
                };

                let mut info_text = format!("🏛️ <b>{}</b>\n", escape_html(&org.name));
                if let Some(full_name) = &org.full_name {
                    info_text.push_str(&format!("<i>{}</i>\n", escape_html(full_name)));
                }
                if let Some(description) = &org.description {
                    info_text.push_str(&format!("\n{}\n", escape_html(description)));
                }

                bot.send_message(chat_id, info_text)
                    .parse_mode(ParseMode::Html)
                    .await?;
            }
        }
        "menu:misdatos" => {
            // Obtener y mostrar datos completos del usuario
            if let Some(chat_id) = chat_id {
                let user = match state.db.get_user(telegram_id).await? {
                    Some(u) => u,
                    None => {
                        bot.send_message(chat_id, "❌ No tienes datos registrados.").await?;
                        return Ok(());
                    }
                };
                
                // Verificar si es admin por config
                let is_config_admin = state.config.bot.admins.contains(&telegram_id);
                
                let empresas = state.db.get_empresas_by_user(telegram_id).await?;
                let empresa = empresas.into_iter().next();
                
                // Determinar tipo de usuario
                let tipo_usuario = if is_config_admin {
                    "🔧 Administrador"
                } else if user.is_internal {
                    "Empresa/Autónomo"
                } else {
                    "Particular"
                };
                
                // Determinar estado de miembro
                let miembro_status = if is_config_admin {
                    "🔧 Administrador"
                } else if user.is_member {
                    "✅ Sí"
                } else {
                    "❌ No"
                };
                
                let mut texto = format!(
                    "👤 <b>Tus Datos</b>\n\n\
                    <b>Nombre:</b> {}\n\
                    <b>Username:</b> {}\n\
                    <b>Tipo:</b> {}\n\
                    <b>Miembro:</b> {}",
                    escape_html(user.first_name.as_deref().unwrap_or("Usuario")),
                    user.username.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
                    tipo_usuario,
                    miembro_status
                );
                
                if let Some(ref emp) = empresa {
                    texto.push_str(&format!(
                        "\n\n🏢 <b>Empresa Registrada</b>\n\n\
                        <b>Nombre fiscal:</b> {}\n\
                        <b>NIF/CIF:</b> {}\n\
                        <b>Descripción:</b> {}\n\
                        <b>Teléfono:</b> {}\n\
                        <b>Email:</b> {}\n\
                        <b>Web:</b> {}\n\
                        <b>Dirección:</b> {}\n\
                        <b>Ciudad:</b> {}\n\
                        <b>Provincia:</b> {}\n\
                        <b>Código Postal:</b> {}\n\
                        <b>Estado:</b> {}",
                        escape_html(&emp.nombre_fiscal),
                        emp.cif_nif.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                        emp.descripcion.as_deref().map(escape_html).unwrap_or_else(|| "Sin descripción".to_string()),
                        emp.telefono.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                        emp.email.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                        emp.web.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                        emp.direccion.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                        emp.ciudad.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                        emp.provincia.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                        emp.codigo_postal.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                        if emp.activa { "✅ Activa" } else { "⏳ Pendiente de aprobación" }
                    ));
                } else if user.is_internal {
                    texto.push_str("\n\n⚠️ <b>No tienes empresa registrada</b>\n\nUsa /registrar para registrar tu empresa.");
                }
                
                // Build keyboard based on whether user has an empresa
                let mut keyboard_rows = vec![
                    vec![
                        InlineKeyboardButton::callback("✏️ Actualizar datos", "misdatos:update"),
                    ],
                ];
                
                // Add "Ver mi empresa" button if user has an empresa
                if empresa.is_some() {
                    keyboard_rows.push(vec![
                        InlineKeyboardButton::callback("🏢 Ver mi empresa", "misdatos:empresa"),
                    ]);
                }
                
                keyboard_rows.push(vec![
                    InlineKeyboardButton::callback("🗑️ Eliminar mi cuenta", "misdatos:delete"),
                ]);
                
                let keyboard = InlineKeyboardMarkup::new(keyboard_rows);
                
                bot.send_message(chat_id, texto)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        "misdatos:update" => {
            // Start the wizard for updating data
            if let Some(chat_id) = chat_id {
                // Initialize wizard state for editing
                let initial_data = crate::wizard::RegistrationData {
                    user_type: None,
                    name: None,
                    description: None,
                    cif: None,
                    phone: None,
                    email: None,
                    website: None,
                    address: None,
                    city: None,
                    province: None,
                    postal_code: None,
                    category: None,
                    logo_url: None,
                    centros: vec![],
                    servicios: vec![],
                    temp_centro: None,
                    temp_servicio: None,
                    completed: false,
                    contact_method: None,
                };
                
                crate::wizard::update_wizard_state(telegram_id, crate::wizard::WizardState {
                    step: crate::wizard::WizardStep::AskType,
                    data: initial_data.clone(),
                });
                
                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![InlineKeyboardButton::callback("🏢 Empresa", "wiz:type:empresa")],
                    vec![InlineKeyboardButton::callback("👤 Autónomo", "wiz:type:autonomo")],
                    vec![InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")],
                ]);
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ <b>Actualizar tus datos</b>\n\n¿Qué tipo de perfil tienes?"
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard)
                .await?;
            }
        }
        "misdatos:delete" => {
            // Show confirmation for delete
            if let Some(chat_id) = chat_id {
                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("✅ Sí, eliminar todo", "misdatos:delete_confirm"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("❌ No, cancelar", "misdatos:cancel"),
                    ],
                ]);
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "⚠️ <b>¿Estás seguro?</b>\n\n\
                    Esto eliminará <b>permanentemente</b>:\n\
                    • Tu cuenta de usuario\n\
                    • Tu empresa (si tienes)\n\
                    • Todos tus centros y servicios\n\
                    • Tus mensajes y difusiones\n\
                    • Tu historial de pagos\n\n\
                    <b>Esta acción no se puede deshacer.</b>"
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard)
                .await?;
            }
        }
        "misdatos:delete_confirm" => {
            // Actually delete the user
            if let Some(chat_id) = chat_id {
                match state.db.delete_user_completely(telegram_id).await {
                    Ok(()) => {
                        bot.edit_message_text(
                            chat_id,
                            q.message.as_ref().unwrap().id,
                            "✅ Tu cuenta y todos tus datos han sido eliminados.\n\n\
                            Puedes registrarte de nuevo cuando quieras con /start"
                        ).await?;
                    }
                    Err(e) => {
                        tracing::error!("Error deleting user {}: {}", telegram_id, e);
                        bot.edit_message_text(
                            chat_id,
                            q.message.as_ref().unwrap().id,
                            "❌ Ha ocurrido un error al eliminar tu cuenta. Contacta con soporte."
                        ).await?;
                    }
                }
            }
        }
        "misdatos:cancel" => {
            // Cancel the delete operation
            if let Some(chat_id) = chat_id {
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✅ Operación cancelada. Tus datos están a salvo."
                ).await?;
            }
        }
        "misdatos:empresa" => {
            // Show detailed empresa data with centros and servicios
            if let Some(chat_id) = chat_id {
                let empresas = state.db.get_empresas_by_user(telegram_id).await?;
                let empresa = match empresas.into_iter().next() {
                    Some(e) => e,
                    None => {
                        bot.send_message(chat_id, "❌ No tienes una empresa registrada.").await?;
                        return Ok(());
                    }
                };
                
                // Get centros and servicios
                let centros = state.db.get_centros_by_empresa(empresa.id).await.unwrap_or_default();
                let servicios = state.db.get_servicios_by_empresa(empresa.id).await.unwrap_or_default();
                
                let mut texto = format!(
                    "🏢 <b>Datos de tu Empresa</b>\n\n\
                    <b>Nombre fiscal:</b> {}\n\
                    <b>NIF/CIF:</b> {}\n\
                    <b>Descripción:</b> {}\n\
                    <b>Teléfono:</b> {}\n\
                    <b>Email:</b> {}\n\
                    <b>Web:</b> {}\n\
                    <b>Dirección:</b> {}\n\
                    <b>Ciudad:</b> {}\n\
                    <b>Provincia:</b> {}\n\
                    <b>Código Postal:</b> {}\n\
                    <b>Estado:</b> {}",
                    escape_html(&empresa.nombre_fiscal),
                    empresa.cif_nif.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                    empresa.descripcion.as_deref().map(escape_html).unwrap_or_else(|| "Sin descripción".to_string()),
                    empresa.telefono.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                    empresa.email.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                    empresa.web.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                    empresa.direccion.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                    empresa.ciudad.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                    empresa.provincia.as_deref().map(escape_html).unwrap_or_else(|| "No especificada".to_string()),
                    empresa.codigo_postal.as_deref().map(escape_html).unwrap_or_else(|| "No especificado".to_string()),
                    if empresa.activa { "✅ Activa" } else { "⏳ Pendiente de aprobación" }
                );
                
                // Add centros
                if centros.is_empty() {
                    texto.push_str("\n\n📍 <b>Centros de trabajo:</b>\n   No tienes centros registrados.");
                } else {
                    texto.push_str(&format!("\n\n📍 <b>Centros de trabajo ({})</b>:", centros.len()));
                    for (id, nombre) in &centros {
                        texto.push_str(&format!("\n   • {} <code>[ID: {}]</code>", escape_html(nombre), id));
                    }
                }
                
                // Add servicios
                if servicios.is_empty() {
                    texto.push_str("\n\n📋 <b>Servicios/Productos:</b>\n   No tienes servicios registrados.");
                } else {
                    texto.push_str(&format!("\n\n📋 <b>Servicios/Productos ({})</b>:", servicios.len()));
                    for (id, nombre, categoria) in &servicios {
                        texto.push_str(&format!(
                            "\n   • {} <i>({})</i> <code>[ID: {}]</code>",
                            escape_html(nombre),
                            escape_html(categoria),
                            id
                        ));
                    }
                }
                
                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("⬅️ Volver", "misdatos:volver"),
                    ],
                ]);
                
                bot.edit_message_text(chat_id, q.message.as_ref().unwrap().id, texto)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        "misdatos:volver" => {
            // Go back to main misdatos view
            if let Some(chat_id) = chat_id {
                let user = match state.db.get_user(telegram_id).await? {
                    Some(u) => u,
                    None => {
                        bot.send_message(chat_id, "❌ No tienes datos registrados.").await?;
                        return Ok(());
                    }
                };
                
                let is_config_admin = state.config.bot.admins.contains(&telegram_id);
                let empresas = state.db.get_empresas_by_user(telegram_id).await?;
                let empresa = empresas.into_iter().next();
                
                let tipo_usuario = if is_config_admin {
                    "🔧 Administrador"
                } else if user.is_internal {
                    "Empresa/Autónomo"
                } else {
                    "Particular"
                };
                
                let miembro_status = if is_config_admin {
                    "🔧 Administrador"
                } else if user.is_member {
                    "✅ Sí"
                } else {
                    "❌ No"
                };
                
                let texto = format!(
                    "👤 <b>Tus Datos</b>\n\n\
                    <b>Nombre:</b> {}\n\
                    <b>Username:</b> {}\n\
                    <b>Tipo:</b> {}\n\
                    <b>Miembro:</b> {}",
                    escape_html(user.first_name.as_deref().unwrap_or("Usuario")),
                    user.username.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
                    tipo_usuario,
                    miembro_status
                );
                
                let mut keyboard_rows = vec![
                    vec![
                        InlineKeyboardButton::callback("✏️ Actualizar datos", "misdatos:update"),
                    ],
                ];
                
                if empresa.is_some() {
                    keyboard_rows.push(vec![
                        InlineKeyboardButton::callback("🏢 Ver mi empresa", "misdatos:empresa"),
                    ]);
                }
                
                keyboard_rows.push(vec![
                    InlineKeyboardButton::callback("🗑️ Eliminar mi cuenta", "misdatos:delete"),
                ]);
                
                let keyboard = InlineKeyboardMarkup::new(keyboard_rows);
                
                bot.edit_message_text(chat_id, q.message.as_ref().unwrap().id, texto)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        "menu:difusiones" => {
            
            if let Some(chat_id) = chat_id {
                let user_id = q.from.id.0 as i64;
                crate::handlers::broadcast_extended::handle_menu_difusiones(
                    bot.clone(),
                    chat_id,
                    user_id,
                    state.db.clone(),
                    state.config.clone()
                ).await?;
            }
            if let Some(chat_id) = chat_id {
                let messages = state.db.get_unread_messages(telegram_id).await?;
                
                if messages.is_empty() {
                    bot.send_message(chat_id, "📭 No tienes mensajes nuevos.").await?;
                } else {
                    let mut response = format!("<b>Tienes {} mensajes nuevos:</b>\n\n", messages.len());
                    for mensaje in messages.iter().take(5) {
                        response.push_str(&format!(
                            "📩 <b>{}</b>\n{}\n\n",
                            mensaje.asunto,
                            mensaje.contenido
                        ));
                    }
                    bot.send_message(chat_id, response)
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
            }
        }
        "menu:contact_admin" => {
            if let Some(chat_id) = chat_id {
                bot.send_message(
                    chat_id,
                    format!(
                        "📞 <b>Contactar con administración</b>\n\n\
                        Para contactar con un administrador de <b>{}</b>:\n\n\
                        • Responde a este mensaje\n\
                        • Un administrador te responderá lo antes posible\n\n\
                        Tu ID de usuario: <code>{}</code>",
                        state.config.bot.name,
                        telegram_id
                    )
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
        }
        "menu:start" => {
            if let Some(chat_id) = chat_id {
                let welcome_text = format!(
                    "👋 <b>¡Bienvenido a {}!</b>\n\n\
                    Selecciona una opción:",
                    state.config.bot.name
                );

                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("🔍 Buscar", "menu:buscar"),
                        InlineKeyboardButton::callback("💬 Chat IA", "menu:chat"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("ℹ️ Info", "menu:info"),
                        InlineKeyboardButton::callback("📋 Mis datos", "menu:misdatos"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("📢 Difusiones", "menu:difusiones"),
                        InlineKeyboardButton::callback("📩 Mensajes", "menu:mensajes"),
                    ],
                ]);

                bot.send_message(chat_id, welcome_text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        _ => {}
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

/// Handler para /admin_org - Editar datos de la organización (solo admin)
/// Uso: /admin_org campo valor
/// Campos: name, full_name, description, mission, vision, address, city, province, postal_code, phone, email, website, registration_info, benefits
pub async fn handle_admin_org(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    args: String,
) -> anyhow::Result<()> {
    // Verificar que el usuario es admin
    let admin_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if !state.config.bot.admins.contains(&admin_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    // Si no hay argumentos, mostrar ayuda y datos actuales
    if args.trim().is_empty() {
        let org = state.db.get_organization().await?;
        let help_text = format!(
            "🏛️ <b>Configuración de la Organización</b>\n\n\
            <b>Datos actuales:</b>\n\
            • name: {}\n\
            • full_name: {}\n\
            • description: {}\n\
            • mission: {}\n\
            • vision: {}\n\
            • address: {}\n\
            • city: {}\n\
            • province: {}\n\
            • postal_code: {}\n\
            • phone: {}\n\
            • email: {}\n\
            • website: {}\n\
            • registration_info: {}\n\
            • benefits: {}\n\n\
            <b>Uso:</b>\n\
            <code>/admin_org campo valor</code>\n\n\
            <b>Ejemplos:</b>\n\
            <code>/adminorg name Colegio Oficial de Ingenieros</code>\n\
            <code>/admin_org phone +34 912 345 678</code>\n\
            <code>/admin_org description Somos una asociación...</code>",
            escape_html(&org.name),
            org.full_name.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.description.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.mission.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.vision.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.address.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.city.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.province.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.postal_code.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.phone.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.email.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.website.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.registration_info.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
            org.benefits.as_deref().map(escape_html).unwrap_or_else(|| "No establecido".to_string()),
        );
        bot.send_message(msg.chat.id, help_text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        return Ok(());
    }

    // Parsear argumentos: campo valor
    let parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
    if parts.len() != 2 {
        bot.send_message(
            msg.chat.id,
            "❌ Formato incorrecto. Usa: <code>/admin_org campo valor</code>\nEjemplo: <code>/admin_org phone +34 912 345 678</code>"
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    let field = parts[0].to_lowercase();
    let value = parts[1].trim();

    // Validar campo
    let valid_fields = [
        "name", "full_name", "description", "mission", "vision",
        "address", "city", "province", "postal_code", "phone",
        "email", "website", "registration_info", "benefits"
    ];

    if !valid_fields.contains(&field.as_str()) {
        bot.send_message(
            msg.chat.id,
            format!(
                "❌ Campo no válido. Campos permitidos:\n<code>{}</code>",
                valid_fields.join(", ")
            )
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    // Actualizar el campo correspondiente
    let mut name = None;
    let mut full_name = None;
    let mut description = None;
    let mut mission = None;
    let mut vision = None;
    let mut address = None;
    let mut city = None;
    let mut province = None;
    let mut postal_code = None;
    let mut phone = None;
    let mut email = None;
    let mut website = None;
    let mut registration_info = None;
    let mut benefits = None;

    match field.as_str() {
        "name" => name = Some(value),
        "full_name" => full_name = Some(value),
        "description" => description = Some(value),
        "mission" => mission = Some(value),
        "vision" => vision = Some(value),
        "address" => address = Some(value),
        "city" => city = Some(value),
        "province" => province = Some(value),
        "postal_code" => postal_code = Some(value),
        "phone" => phone = Some(value),
        "email" => email = Some(value),
        "website" => website = Some(value),
        "registration_info" => registration_info = Some(value),
        "benefits" => benefits = Some(value),
        _ => {}
    }

    match state.db.update_organization(
        name, full_name, description, mission, vision,
        address, city, province, postal_code, phone,
        email, website, registration_info, benefits
    ).await {
        Ok(_) => {
            bot.send_message(
                msg.chat.id,
                format!("✅ Campo <b>{}</b> actualizado correctamente.", escape_html(&field))
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        Err(e) => {
            tracing::error!("Error actualizando organización: {}", e);
            bot.send_message(msg.chat.id, "❌ Error al actualizar la organización.").await?;
        }
    }

    Ok(())
}

/// Handler para /admin_member - Cambiar estado de miembro de un usuario (solo admin)
/// Uso: /admin_member <user_id> <on|off>
pub async fn handle_admin_member(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    args: String,
) -> anyhow::Result<()> {
    // Verificar que el usuario es admin
    let admin_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if !state.config.bot.admins.contains(&admin_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    // Parsear argumentos
    let parts: Vec<&str> = args.split_whitespace().collect();
    
    if parts.is_empty() {
        // Mostrar ayuda
        let help_text = "🔐 <b>Gestión de Miembros</b>\n\n\
            <b>Uso:</b>\n\
            <code>/admin_member &lt;user_id&gt; on</code> - Marcar como miembro\n\
            <code>/admin_member &lt;user_id&gt; off</code> - Quitar estado de miembro\n\n\
            <b>Ejemplo:</b>\n\
            <code>/admin_member 123456789 on</code>\n\n\
            Usa <code>/admin_users &lt;busqueda&gt;</code> para encontrar usuarios.";
        
        bot.send_message(msg.chat.id, help_text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        return Ok(());
    }

    if parts.len() < 2 {
        bot.send_message(
            msg.chat.id,
            "❌ Faltan argumentos. Usa: <code>/admin_member &lt;user_id&gt; on|off</code>"
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    // Parsear user_id
    let user_id: i64 = match parts[0].parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "❌ ID de usuario inválido.").await?;
            return Ok(());
        }
    };

    // Parsear estado
    let is_member = match parts[1].to_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => true,
        "off" | "false" | "no" | "0" => false,
        _ => {
            bot.send_message(
                msg.chat.id,
                "❌ Estado inválido. Usa 'on' o 'off'."
            ).await?;
            return Ok(());
        }
    };

    // Obtener usuario actual
    let user = match state.db.get_user(user_id).await? {
        Some(u) => u,
        None => {
            bot.send_message(msg.chat.id, "❌ Usuario no encontrado.").await?;
            return Ok(());
        }
    };

    // Cambiar estado
    match state.db.set_user_member_status(user_id, is_member).await {
        Ok(true) => {
            let status_text = if is_member { "✅ miembro" } else { "❌ no miembro" };
            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Estado actualizado\n\n\
                    <b>Usuario:</b> {} (ID: <code>{}</code>)\n\
                    <b>Nuevo estado:</b> {}",
                    escape_html(&user.first_name.unwrap_or_default()),
                    user_id,
                    status_text
                )
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

            // Notificar al usuario
            let notification = if is_member {
                format!(
                    "🎉 <b>¡Tu membresía ha sido verificada!</b>\n\n\
                    Un administrador de <b>{}</b> ha confirmado que eres miembro.\n\n\
                    Tienes acceso completo a todas las funcionalidades del bot.",
                    state.config.bot.name
                )
            } else {
                format!(
                    "⚠️ <b>Cambio de estado</b>\n\n\
                    Tu estado de miembro de <b>{}</b> ha sido actualizado.\n\
                    Si esto es un error, contacta con la administración.",
                    state.config.bot.name
                )
            };

            let _ = bot.send_message(
                teloxide::types::ChatId(user_id),
                notification
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await;
        }
        Ok(false) => {
            bot.send_message(msg.chat.id, "❌ No se pudo actualizar el estado.").await?;
        }
        Err(e) => {
            tracing::error!("Error cambiando estado de miembro: {}", e);
            bot.send_message(msg.chat.id, "❌ Error al actualizar el estado.").await?;
        }
    }

    Ok(())
}

/// Handler para /admin_users - Buscar usuarios (solo admin)
/// Uso: /admin_users <busqueda>
pub async fn handle_admin_users(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    args: String,
) -> anyhow::Result<()> {
    // Verificar que el usuario es admin
    let admin_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if !state.config.bot.admins.contains(&admin_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }

    let query = args.trim();

    let users = if query.is_empty() {
        state.db.get_all_users().await?
    } else {
        state.db.search_users(query).await?
    };

    if users.is_empty() {
        bot.send_message(msg.chat.id, "📭 No se encontraron usuarios.").await?;
        return Ok(());
    }

    let mut text = format!("👥 <b>Usuarios encontrados:</b> ({})\n\n", users.len());
    
    for user in users.iter().take(20) {
        let member_status = if user.is_member { "✅ Miembro" } else { "⚪ No miembro" };
        let type_status = if user.is_internal { "🏢" } else { "👤" };
        
        text.push_str(&format!(
            "{} {} <b>{}</b>\n\
            └ ID: <code>{}</code> | {}\n\n",
            type_status,
            member_status,
            escape_html(&user.first_name.clone().unwrap_or_else(|| "Sin nombre".to_string())),
            user.telegram_id,
            user.username.as_deref().map(|u| format!("@{}", u)).unwrap_or_default()
        ));
    }

    if users.len() > 20 {
        text.push_str(&format!("\n... y {} más.", users.len() - 20));
    }

    text.push_str("\n<b>Comandos:</b>\n\
        <code>/admin_member &lt;id&gt; on|off</code> - Cambiar estado de miembro");

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;

    Ok(())
}


// ===== BROADCAST MENU CALLBACK ROUTING =====

/// Manejar callbacks del menú de difusiones (broadcast:*)
async fn handle_broadcast_menu_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    data: &str,
) -> anyhow::Result<()> {
    let user_id = q.from.id.0 as i64;
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    let Some(chat_id) = chat_id else {
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    };
    
    if data == "broadcast:new" {
        // Iniciar proceso de nueva difusión - ir directamente al flujo de broadcast
        // unused imports removed
        
        // Obtener uso actual para verificar créditos
        let now = chrono::Utc::now();
        let year = now.year();
        let quarter = ((now.month() - 1) / 3 + 1) as i32;
        
        let (used_count, paid_extra) = state.db.get_broadcast_usage(user_id, year, quarter).await?;
        let free_limit = state.config.broadcast.quarterly_limit;
        let remaining_free = (free_limit - used_count).max(0);
        let total_remaining = remaining_free + paid_extra;
        
        if total_remaining == 0 {
            let keyboard = teloxide::types::InlineKeyboardMarkup::new(vec![
                vec![teloxide::types::InlineKeyboardButton::callback("💳 Comprar créditos", "broadcast:buy")],
                vec![teloxide::types::InlineKeyboardButton::callback("🔙 Volver", "menu:difusiones")],
            ]);
            
            bot.send_message(
                chat_id,
                format!(
                    "❌ No tienes créditos de difusión disponibles.\n\n\
                    📊 Usadas: {}/{} gratis\n\
                    💳 Pagadas: {}\n\n\
                    Compra créditos para continuar.",
                    used_count, free_limit, paid_extra
                )
            )
            .reply_markup(keyboard)
            .await?;
        } else {
            let credits_info = if remaining_free > 0 {
                format!("🆓 Gratis: {}/{} | 💳 Pagadas: {}", used_count, free_limit, paid_extra)
            } else {
                format!("💳 Usando crédito pagado (quedan: {})", paid_extra)
            };
            
            bot.send_message(
                chat_id,
                format!(
                    "📢 <b>Nueva Difusión</b>\n\
                    Créditos: {}\n\n\
                    Paso 1/3: Envía el <b>título</b> de tu anuncio:",
                    credits_info
                )
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
            
            // Actualizar diálogo para esperar el título
            // Nota: Esto requiere acceso al diálogo, pero handle_callback no lo pasa directamente.
            // Usaremos un mensaje de texto que el usuario debe enviar con /broadcast
            bot.send_message(
                chat_id,
                "💡 Usa el comando /broadcast para iniciar el proceso completo de difusión."
            ).await?;
        }
    } else if data.starts_with("broadcast:history:") {
        let page: usize = data[17..].parse().unwrap_or(0);
        handle_broadcast_history(bot.clone(), chat_id, user_id, state.db.clone(), page).await?;
    } else if data.starts_with("broadcast:view:") {
        let broadcast_id: i64 = data[15..].parse().unwrap_or(0);
        handle_broadcast_view(bot.clone(), chat_id, user_id, broadcast_id, state.db.clone(), state.config.clone()).await?;
    } else if data.starts_with("broadcast:delete_confirm:") {
        let broadcast_id: i64 = data[24..].parse().unwrap_or(0);
        handle_broadcast_delete_confirm(bot.clone(), chat_id, broadcast_id).await?;
    } else if data.starts_with("broadcast:delete:") {
        let broadcast_id: i64 = data[16..].parse().unwrap_or(0);
        handle_broadcast_delete(bot.clone(), chat_id, user_id, broadcast_id, state.db.clone()).await?;
    } else if data == "broadcast:stats" {
        handle_broadcast_stats(bot.clone(), chat_id, user_id, state.db.clone(), state.config.clone()).await?;
    } else if data == "broadcast:buy" {
        handle_broadcast_buy(bot.clone(), chat_id, user_id, state.db.clone(), state.config.clone()).await?;
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

/// Manejar callbacks de compra de créditos (buy_pack:* y calc_needs)
async fn handle_buy_pack_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    data: &str,
) -> anyhow::Result<()> {
    let user_id = q.from.id.0 as i64;
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    let Some(chat_id) = chat_id else {
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    };
    
    if data == "calc_needs" {
        handle_calc_needs(bot.clone(), chat_id, user_id, state.db.clone(), state.config.clone()).await?;
    } else if data.starts_with("buy_pack:") {
        let pack_name = &data[9..];
        handle_buy_pack(bot.clone(), chat_id, user_id, pack_name, state.db.clone(), state.config.clone()).await?;
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

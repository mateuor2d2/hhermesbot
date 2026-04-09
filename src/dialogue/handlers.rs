use teloxide::{
    prelude::*,
    types::{
        InlineKeyboardButton, InlineKeyboardMarkup, 
        ParseMode
    },
};
use std::sync::Arc;
use crate::{
    dialogue::{RegistrationDialogue, RegistrationState, RegistrationData, FieldToEdit},
    wizard::{CentroPendiente, ServicioPendiente},
    BotState,
};

// ==================== KEYBOARDS ====================

pub fn type_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("🏢 Empresa", "type:empresa"),
            InlineKeyboardButton::callback("👤 Autónomo", "type:autonomo"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar", "wizard:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

fn yes_no_keyboard(yes_action: &str, no_action: &str) -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("✅ Sí", yes_action),
            InlineKeyboardButton::callback("⏭️ No / Saltar", no_action),
        ],
        vec![
            InlineKeyboardButton::callback("🔙 Volver", "wizard:back"),
            InlineKeyboardButton::callback("❌ Cancelar", "wizard:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

fn contact_choice_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("📱 Teléfono", "contact:phone"),
            InlineKeyboardButton::callback("📧 Email", "contact:email"),
        ],
        vec![
            InlineKeyboardButton::callback("📱📧 Ambos", "contact:both"),
            InlineKeyboardButton::callback("⏭️ Omitir", "contact:none"),
        ],
        vec![
            InlineKeyboardButton::callback("🔙 Volver", "wizard:back"),
            InlineKeyboardButton::callback("❌ Cancelar", "wizard:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

fn confirm_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("✅ Confirmar registro", "wizard:confirm"),
        ],
        vec![
            InlineKeyboardButton::callback("✏️ Editar tipo", "edit:type"),
            InlineKeyboardButton::callback("✏️ Editar nombre", "edit:name"),
        ],
        vec![
            InlineKeyboardButton::callback("✏️ Editar descripción", "edit:description"),
            InlineKeyboardButton::callback("✏️ Editar CIF", "edit:cif"),
        ],
        vec![
            InlineKeyboardButton::callback("✏️ Editar teléfono", "edit:phone"),
            InlineKeyboardButton::callback("✏️ Editar email", "edit:email"),
        ],
        vec![
            InlineKeyboardButton::callback("❌ Cancelar todo", "wizard:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

fn edit_field_keyboard() -> InlineKeyboardMarkup {
    let buttons = vec![
        vec![
            InlineKeyboardButton::callback("🔙 Volver al resumen", "wizard:summary"),
            InlineKeyboardButton::callback("❌ Cancelar", "wizard:cancel"),
        ],
    ];
    InlineKeyboardMarkup::new(buttons)
}

// ==================== HANDLERS ====================

pub async fn start_registration(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
) -> anyhow::Result<()> {
    dialogue.update(RegistrationState::AskType).await?;
    
    bot.send_message(
        msg.chat.id,
        "🏢 <b>Registro de negocio</b>\n\n\
        Paso 1/6: ¿Qué tipo de negocio eres?"
    )
    .parse_mode(ParseMode::Html)
    .reply_markup(type_keyboard())
    .await?;
    
    Ok(())
}

pub async fn handle_type_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match data {
            "type:empresa" => {
                let reg_data = RegistrationData {
                    user_type: Some("Empresa".to_string()),
                    ..Default::default()
                };
                dialogue.update(RegistrationState::AskName { data: reg_data }).await?;
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🏢 <b>Registro de negocio</b>\n\n\
                    Paso 2/6: ¿Cuál es el <b>nombre</b> de tu empresa?\n\n\
                    Escribe el nombre y presiona enviar ✈️"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "type:autonomo" => {
                let reg_data = RegistrationData {
                    user_type: Some("Autónomo".to_string()),
                    ..Default::default()
                };
                dialogue.update(RegistrationState::AskName { data: reg_data }).await?;
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "👤 <b>Registro de autónomo</b>\n\n\
                    Paso 2/6: ¿Cuál es tu <b>nombre profesional</b>?\n\n\
                    Escribe el nombre y presiona enviar ✈️"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "wizard:cancel" => {
                dialogue.exit().await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "❌ Registro cancelado."
                )
                .await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_name_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(name) = msg.text() {
        let new_data = RegistrationData {
            name: Some(name.to_string()),
            ..data
        };
        
        dialogue.update(RegistrationState::AskDescription { data: new_data }).await?;
        
        bot.send_message(
            msg.chat.id,
            format!(
                "✅ Nombre guardado: <b>{}</b>\n\n\
                Paso 3/6: Escribe una <b>descripción breve</b> de tu negocio.\n\n\
                💡 Consejo: Sé conciso, máximo 2-3 líneas.",
                name
            )
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    
    Ok(())
}

pub async fn handle_description_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(desc) = msg.text() {
        let new_data = RegistrationData {
            description: Some(desc.to_string()),
            ..data
        };
        
        dialogue.update(RegistrationState::AskCifChoice { data: new_data }).await?;
        
        bot.send_message(
            msg.chat.id,
            format!(
                "✅ Descripción guardada\n\n\
                Paso 4/6: ¿Quieres añadir tu <b>CIF/NIF</b>?\n\n\
                Esto ayuda a validar tu negocio."
            )
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(yes_no_keyboard("cif:yes", "cif:no"))
        .await?;
    }
    
    Ok(())
}

pub async fn handle_cif_choice_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "cif:yes" => {
                dialogue.update(RegistrationState::AskCif { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "📝 Escribe tu <b>CIF/NIF</b>:"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "cif:no" => {
                let new_data = RegistrationData {
                    cif: None,
                    ..data
                };
                dialogue.update(RegistrationState::AskContactChoice { data: new_data }).await?;
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "⏭️ CIF omitido\n\n\
                    Paso 5/6: ¿Cómo quieres que te contacten?"
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(contact_choice_keyboard())
                .await?;
            }
            "wizard:cancel" => {
                dialogue.exit().await?;
                bot.edit_message_text(chat_id, q.message.as_ref().unwrap().id, "❌ Registro cancelado.").await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_cif_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(cif) = msg.text() {
        let new_data = RegistrationData {
            cif: Some(cif.to_string()),
            ..data
        };
        
        dialogue.update(RegistrationState::AskContactChoice { data: new_data }).await?;
        
        bot.send_message(
            msg.chat.id,
            format!(
                "✅ CIF guardado: <b>{}</b>\n\n\
                Paso 5/6: ¿Cómo quieres que te contacten?",
                cif
            )
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(contact_choice_keyboard())
        .await?;
    }
    
    Ok(())
}

pub async fn handle_contact_choice_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "contact:phone" => {
                dialogue.update(RegistrationState::AskPhone { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "📱 Escribe tu <b>número de teléfono</b>:"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "contact:email" => {
                dialogue.update(RegistrationState::AskEmail { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "📧 Escribe tu <b>email</b>:"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "contact:both" => {
                dialogue.update(RegistrationState::AskPhone { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "📱 Escribe tu <b>número de teléfono</b> (luego pediré el email):"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "contact:none" => {
                let new_data = RegistrationData {
                    phone: None,
                    email: None,
                    ..data
                };
                dialogue.update(RegistrationState::Confirm { data: new_data.clone() }).await?;
                
                let summary = new_data.to_summary();
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    format!("{}\n\n¿Todo correcto?", summary)
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(confirm_keyboard())
                .await?;
            }
            "wizard:cancel" => {
                dialogue.exit().await?;
                bot.edit_message_text(chat_id, q.message.as_ref().unwrap().id, "❌ Registro cancelado.").await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_phone_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(phone) = msg.text() {
        let new_data = RegistrationData {
            phone: Some(phone.to_string()),
            ..data.clone()
        };
        
        // Si ya tenemos email o el usuario eligió "solo teléfono", vamos a confirmar
        if data.email.is_some() || (data.phone.is_none() && data.email.is_none()) {
            // Vino de "both", ahora pedir email
            dialogue.update(RegistrationState::AskEmail { data: new_data }).await?;
            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Teléfono guardado: <b>{}</b>\n\n\
                    📧 Ahora escribe tu <b>email</b>:",
                    phone
                )
            )
            .parse_mode(ParseMode::Html)
            .await?;
        } else {
            // Solo quería teléfono
            dialogue.update(RegistrationState::Confirm { data: new_data.clone() }).await?;
            let summary = new_data.to_summary();
            bot.send_message(
                msg.chat.id,
                format!("{}\n\n¿Todo correcto?", summary)
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(confirm_keyboard())
            .await?;
        }
    }
    
    Ok(())
}

pub async fn handle_email_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(email) = msg.text() {
        let new_data = RegistrationData {
            email: Some(email.to_string()),
            ..data
        };
        
        // En lugar de ir a Confirm, vamos a AskAddCentro
        dialogue.update(RegistrationState::AskAddCentro { data: new_data }).await?;
        
        bot.send_message(
            msg.chat.id,
            "📍 <b>Centros de trabajo</b>\n\n\
            ¿Deseas añadir un centro de trabajo?\n\
            Puedes añadir tu oficina, local, o ubicación principal."
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(yes_no_keyboard("centro:add", "centro:skip"))
        .await?;
    }
    
    Ok(())
}

// ==================== CENTROS HANDLERS ====================

pub async fn handle_ask_add_centro_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "centro:add" => {
                dialogue.update(RegistrationState::CentroAskNombre { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🏢 <b>Nuevo centro</b>\n\n¿Cuál es el <b>nombre</b> del centro?\n\
                    (Ej: Oficina Principal, Sede Madrid, etc.)"
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "centro:skip" => {
                // Saltar a servicios
                dialogue.update(RegistrationState::AskAddServicio { data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🛎️ <b>Servicios y Productos</b>\n\n\
                    ¿Deseas añadir un servicio o producto?"
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(yes_no_keyboard("servicio:add", "servicio:skip"))
                .await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_centro_nombre_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(nombre) = msg.text() {
        let temp_centro = CentroPendiente {
            nombre: nombre.to_string(),
            ..Default::default()
        };
        let new_data = RegistrationData {
            temp_centro: Some(temp_centro),
            ..data
        };
        
        dialogue.update(RegistrationState::CentroAskDireccion { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Nombre guardado\n\n¿Cuál es la <b>dirección</b>? (o escribe 'saltar')"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_centro_direccion_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let direccion = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_centro = data.temp_centro.clone().unwrap_or_default();
        temp_centro.direccion = direccion;
        let new_data = RegistrationData {
            temp_centro: Some(temp_centro),
            ..data
        };
        
        dialogue.update(RegistrationState::CentroAskCiudad { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Dirección guardada\n\n¿En qué <b>ciudad</b> está? (o escribe 'saltar')"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_centro_ciudad_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let ciudad = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_centro = data.temp_centro.clone().unwrap_or_default();
        temp_centro.ciudad = ciudad;
        let new_data = RegistrationData {
            temp_centro: Some(temp_centro),
            ..data
        };
        
        dialogue.update(RegistrationState::CentroAskTelefono { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Ciudad guardada\n\n¿Hay algún <b>teléfono</b> específico para este centro? (o 'saltar')"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_centro_telefono_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let telefono = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_centro = data.temp_centro.clone().unwrap_or_default();
        temp_centro.telefono = telefono;
        let new_data = RegistrationData {
            user_type: data.user_type.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            cif: data.cif.clone(),
            phone: data.phone.clone(),
            email: data.email.clone(),
            centros: data.centros.clone(),
            servicios: data.servicios.clone(),
            website: data.website.clone(),
            address: data.address.clone(),
            city: data.city.clone(),
            province: data.province.clone(),
            postal_code: data.postal_code.clone(),
            category: data.category.clone(),
            logo_url: data.logo_url.clone(),
            completed: data.completed,
            temp_centro: Some(temp_centro),
            temp_servicio: data.temp_servicio.clone(),
            contact_method: data.contact_method.clone(),
        };
        
        dialogue.update(RegistrationState::CentroAskEmail { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Teléfono guardado\n\n¿Hay algún <b>email</b> específico para este centro? (o 'saltar')"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_centro_email_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let email = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_centro = data.temp_centro.clone().unwrap_or_default();
        temp_centro.email = email;
        
        // Mostrar resumen del centro
        let mut summary = format!("📍 <b>Resumen del centro</b>\n\n<b>Nombre:</b> {}\n", temp_centro.nombre);
        if let Some(ref d) = temp_centro.direccion {
            summary.push_str(&format!("<b>Dirección:</b> {}\n", d));
        }
        if let Some(ref c) = temp_centro.ciudad {
            summary.push_str(&format!("<b>Ciudad:</b> {}\n", c));
        }
        if let Some(ref t) = temp_centro.telefono {
            summary.push_str(&format!("<b>Teléfono:</b> {}\n", t));
        }
        if let Some(ref e) = temp_centro.email {
            summary.push_str(&format!("<b>Email:</b> {}\n", e));
        }
        
        let new_data = RegistrationData {
            user_type: data.user_type.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            cif: data.cif.clone(),
            phone: data.phone.clone(),
            email: data.email.clone(),
            centros: data.centros.clone(),
            servicios: data.servicios.clone(),
            temp_centro: Some(temp_centro),
            temp_servicio: data.temp_servicio.clone(),
            ..Default::default()
        };
        
        dialogue.update(RegistrationState::CentroConfirm { data: new_data }).await?;
        
        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("✅ Guardar centro", "centro:save"),
                InlineKeyboardButton::callback("🗑️ Descartar", "centro:discard"),
            ],
        ]);
        
        bot.send_message(
            msg.chat.id,
            format!("{}\n\n¿Guardar este centro?", summary)
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    }
    Ok(())
}

pub async fn handle_centro_confirm_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "centro:save" => {
                if let Some(centro) = data.temp_centro {
                    let mut centros = data.centros;
                    centros.push(centro);
                    let new_data = RegistrationData {
                        centros,
                        temp_centro: None,
                        ..data
                    };
                    
                    // Preguntar si quiere añadir otro centro
                    dialogue.update(RegistrationState::AskAddCentro { data: new_data.clone() }).await?;
                    bot.edit_message_text(
                        chat_id,
                        q.message.as_ref().unwrap().id,
                        format!("✅ Centro guardado. Total: {}\n\n¿Añadir otro centro?", new_data.centros.len())
                    )
                    .reply_markup(yes_no_keyboard("centro:add", "centro:skip"))
                    .await?;
                }
            }
            "centro:discard" => {
                let new_data = RegistrationData {
                    temp_centro: None,
                    ..data
                };
                dialogue.update(RegistrationState::AskAddCentro { data: new_data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🗑️ Centro descartado.\n\n¿Añadir otro centro?"
                )
                .reply_markup(yes_no_keyboard("centro:add", "centro:skip"))
                .await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

// ==================== SERVICIOS HANDLERS ====================

pub async fn handle_ask_add_servicio_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "servicio:add" => {
                dialogue.update(RegistrationState::ServicioAskTipo { data }).await?;
                
                let keyboard = InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("🔧 Servicio", "tipo:servicio"),
                        InlineKeyboardButton::callback("📦 Producto", "tipo:bien"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("🔙 Volver", "wizard:back"),
                    ],
                ]);
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🛎️ <b>Nuevo servicio/producto</b>\n\n¿Es un <b>servicio</b> o un <b>producto</b>?"
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard)
                .await?;
            }
            "servicio:skip" => {
                // Ir a confirmación final
                dialogue.update(RegistrationState::Confirm { data: data.clone() }).await?;
                let summary = data.to_summary();
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    format!("{}\n\n✅ <b>Registro completo</b>\n\n¿Todo correcto?", summary)
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(confirm_keyboard())
                .await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_servicio_tipo_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        let tipo = match cb_data {
            "tipo:servicio" => "servicio",
            "tipo:bien" => "bien",
            _ => "servicio",
        };
        
        let temp_servicio = ServicioPendiente {
            tipo: tipo.to_string(),
            ..Default::default()
        };
        let new_data = RegistrationData {
            temp_servicio: Some(temp_servicio),
            ..data
        };
        
        dialogue.update(RegistrationState::ServicioAskCategoria { data: new_data }).await?;
        bot.edit_message_text(
            chat_id,
            q.message.as_ref().unwrap().id,
            format!("✅ Tipo: <b>{}</b>\n\n¿Qué <b>categoría</b> describe mejor este {}?\n\
            Ejemplos: Informática, Construcción, Limpieza, Consultoría...", 
            tipo, tipo)
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_servicio_categoria_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(categoria) = msg.text() {
        let mut temp_servicio = data.temp_servicio.clone().unwrap_or_default();
        temp_servicio.categoria = categoria.to_string();
        let new_data = RegistrationData {
            temp_servicio: Some(temp_servicio),
            temp_centro: data.temp_centro.clone(),
            user_type: data.user_type.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            cif: data.cif.clone(),
            phone: data.phone.clone(),
            email: data.email.clone(),
            centros: data.centros.clone(),
            servicios: data.servicios.clone(),
            website: data.website.clone(),
            address: data.address.clone(),
            city: data.city.clone(),
            province: data.province.clone(),
            postal_code: data.postal_code.clone(),
            category: data.category.clone(),
            logo_url: data.logo_url.clone(),
            completed: data.completed,
            contact_method: data.contact_method.clone(),
        };
        
        dialogue.update(RegistrationState::ServicioAskNombre { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Categoría guardada\n\n¿Cuál es el <b>nombre</b> del servicio/producto?"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_servicio_nombre_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(nombre) = msg.text() {
        let mut temp_servicio = data.temp_servicio.clone().unwrap_or_default();
        temp_servicio.nombre = nombre.to_string();
        let new_data = RegistrationData {
            temp_servicio: Some(temp_servicio),
            ..data
        };
        
        dialogue.update(RegistrationState::ServicioAskDescripcion { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Nombre guardado\n\nEscribe una <b>descripción breve</b> (o 'saltar'):"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_servicio_descripcion_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let descripcion = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_servicio = data.temp_servicio.clone().unwrap_or_default();
        temp_servicio.descripcion = descripcion;
        let new_data = RegistrationData {
            temp_servicio: Some(temp_servicio),
            temp_centro: data.temp_centro.clone(),
            user_type: data.user_type.clone(),
            name: data.name.clone(),
            description: data.description.clone(),
            cif: data.cif.clone(),
            phone: data.phone.clone(),
            email: data.email.clone(),
            centros: data.centros.clone(),
            servicios: data.servicios.clone(),
            website: data.website.clone(),
            address: data.address.clone(),
            city: data.city.clone(),
            province: data.province.clone(),
            postal_code: data.postal_code.clone(),
            category: data.category.clone(),
            logo_url: data.logo_url.clone(),
            completed: data.completed,
            contact_method: data.contact_method.clone(),
        };
        
        dialogue.update(RegistrationState::ServicioAskPrecio { data: new_data }).await?;
        bot.send_message(
            msg.chat.id,
            "✅ Descripción guardada\n\n¿Cuál es el <b>precio</b>?\n\
            Ej: '50€/hora', 'A consultar', 'Desde 100€' (o 'saltar')"
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

pub async fn handle_servicio_precio_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let precio = if text.eq_ignore_ascii_case("saltar") {
            None
        } else {
            Some(text.to_string())
        };
        
        let mut temp_servicio = data.temp_servicio.clone().unwrap_or_default();
        temp_servicio.precio = precio;
        
        // Mostrar resumen del servicio
        let mut summary = format!("🛎️ <b>Resumen del servicio/producto</b>\n\n");
        summary.push_str(&format!("<b>Tipo:</b> {}\n", temp_servicio.tipo));
        summary.push_str(&format!("<b>Categoría:</b> {}\n", temp_servicio.categoria));
        summary.push_str(&format!("<b>Nombre:</b> {}\n", temp_servicio.nombre));
        if let Some(ref d) = temp_servicio.descripcion {
            summary.push_str(&format!("<b>Descripción:</b> {}\n", d));
        }
        if let Some(ref p) = temp_servicio.precio {
            summary.push_str(&format!("<b>Precio:</b> {}\n", p));
        }
        
        let mut new_data = data.clone();
        new_data.temp_servicio = Some(temp_servicio);
        
        dialogue.update(RegistrationState::ServicioConfirm { data: new_data }).await?;
        
        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("✅ Guardar", "servicio:save"),
                InlineKeyboardButton::callback("🗑️ Descartar", "servicio:discard"),
            ],
        ]);
        
        bot.send_message(
            msg.chat.id,
            format!("{}\n\n¿Guardar este servicio/producto?", summary)
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    }
    Ok(())
}

pub async fn handle_servicio_confirm_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "servicio:save" => {
                if let Some(servicio) = data.temp_servicio {
                    let mut servicios = data.servicios;
                    servicios.push(servicio);
                    let new_data = RegistrationData {
                        servicios,
                        temp_servicio: None,
                        ..data
                    };
                    
                    // Preguntar si quiere añadir otro servicio
                    dialogue.update(RegistrationState::AskAddServicio { data: new_data.clone() }).await?;
                    bot.edit_message_text(
                        chat_id,
                        q.message.as_ref().unwrap().id,
                        format!("✅ Servicio guardado. Total: {}\n\n¿Añadir otro servicio/producto?", new_data.servicios.len())
                    )
                    .reply_markup(yes_no_keyboard("servicio:add", "servicio:skip"))
                    .await?;
                }
            }
            "servicio:discard" => {
                let new_data = RegistrationData {
                    temp_servicio: None,
                    ..data
                };
                dialogue.update(RegistrationState::AskAddServicio { data: new_data }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "🗑️ Servicio descartado.\n\n¿Añadir otro servicio/producto?"
                )
                .reply_markup(yes_no_keyboard("servicio:add", "servicio:skip"))
                .await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_confirm_callback(
    bot: Bot,
    q: CallbackQuery,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let cb_data = q.data.as_deref().unwrap_or("");
    let chat_id = q.message.as_ref().map(|m| m.chat.id);
    
    if let Some(chat_id) = chat_id {
        match cb_data {
            "wizard:confirm" => {
                // Guardar en base de datos
                let user_type = match data.user_type {
                    Some(ref t) if t == "Empresa" => "internal",
                    Some(ref t) if t == "Autónomo" => "internal",
                    _ => "external",
                };
                
                state.db.set_user_type(chat_id.0, user_type).await?;
                
                // Guardar datos del negocio en la base de datos
                let business_type = match data.user_type {
                    Some(ref t) if t == "Empresa" => "company",
                    Some(ref t) if t == "Autónomo" => "autonomo",
                    _ => "company",
                };
                
                // Usar upsert_business_complete para guardar empresa + centros + servicios (actualiza si ya existe)
                let empresa_id = state.db.upsert_business_complete(
                    chat_id.0,
                    business_type,
                    data.name.as_deref().unwrap_or(""),
                    data.description.as_deref(),
                    data.cif.as_deref(),
                    data.phone.as_deref(),
                    data.email.as_deref(),
                    data.centros.clone(),
                    data.servicios.clone(),
                ).await?;
                
                dialogue.exit().await?;
                
                // Mensaje de confirmación con resumen
                let mut mensaje = format!(
                    "✅ <b>¡Registro completado!</b>\n\n\
                    Tu solicitud ha sido registrada con ID: <code>{}</code>\n\
                    Está pendiente de aprobación por un administrador.\n\n",
                    empresa_id
                );
                
                if !data.centros.is_empty() {
                    mensaje.push_str(&format!("🏢 <b>Centros registrados:</b> {}\n", data.centros.len()));
                }
                if !data.servicios.is_empty() {
                    mensaje.push_str(&format!("🛎️ <b>Servicios/productos:</b> {}\n", data.servicios.len()));
                }
                
                mensaje.push_str("\n⏳ Recibirás una notificación cuando sea aprobada.");
                
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    mensaje
                )
                .parse_mode(ParseMode::Html)
                .await?;
            }
            "edit:type" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Type }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Selecciona el nuevo tipo:"
                )
                .reply_markup(type_keyboard())
                .await?;
            }
            "edit:name" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Name }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Escribe el nuevo nombre:"
                )
                .reply_markup(edit_field_keyboard())
                .await?;
            }
            "edit:description" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Description }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Escribe la nueva descripción:"
                )
                .reply_markup(edit_field_keyboard())
                .await?;
            }
            "edit:cif" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Cif }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Escribe el nuevo CIF (o 'eliminar' para quitarlo):"
                )
                .reply_markup(edit_field_keyboard())
                .await?;
            }
            "edit:phone" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Phone }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Escribe el nuevo teléfono (o 'eliminar' para quitarlo):"
                )
                .reply_markup(edit_field_keyboard())
                .await?;
            }
            "edit:email" => {
                dialogue.update(RegistrationState::EditField { data, field: FieldToEdit::Email }).await?;
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    "✏️ Escribe el nuevo email (o 'eliminar' para quitarlo):"
                )
                .reply_markup(edit_field_keyboard())
                .await?;
            }
            "wizard:summary" => {
                dialogue.update(RegistrationState::Confirm { data: data.clone() }).await?;
                let summary = data.to_summary();
                bot.edit_message_text(
                    chat_id,
                    q.message.as_ref().unwrap().id,
                    format!("{}\n\n¿Todo correcto?", summary)
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(confirm_keyboard())
                .await?;
            }
            "wizard:cancel" => {
                dialogue.exit().await?;
                bot.edit_message_text(chat_id, q.message.as_ref().unwrap().id, "❌ Registro cancelado.").await?;
            }
            _ => {}
        }
    }
    
    bot.answer_callback_query(q.id).await?;
    Ok(())
}

pub async fn handle_edit_input(
    bot: Bot,
    msg: Message,
    dialogue: RegistrationDialogue,
    data: RegistrationData,
    field: FieldToEdit,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    if let Some(text) = msg.text() {
        let new_data = match field {
            FieldToEdit::Type => {
                // Esto se maneja por callback, no por input
                data
            }
            FieldToEdit::Name => RegistrationData {
                name: Some(text.to_string()),
                ..data
            },
            FieldToEdit::Description => RegistrationData {
                description: Some(text.to_string()),
                ..data
            },
            FieldToEdit::Cif => {
                if text.eq_ignore_ascii_case("eliminar") {
                    RegistrationData { cif: None, ..data }
                } else {
                    RegistrationData { cif: Some(text.to_string()), ..data }
                }
            }
            FieldToEdit::Phone => {
                if text.eq_ignore_ascii_case("eliminar") {
                    RegistrationData { phone: None, ..data }
                } else {
                    RegistrationData { phone: Some(text.to_string()), ..data }
                }
            }
            FieldToEdit::Email => {
                if text.eq_ignore_ascii_case("eliminar") {
                    RegistrationData { email: None, ..data }
                } else {
                    RegistrationData { email: Some(text.to_string()), ..data }
                }
            }
        };
        
        dialogue.update(RegistrationState::Confirm { data: new_data.clone() }).await?;
        let summary = new_data.to_summary();
        
        bot.send_message(
            msg.chat.id,
            format!("✅ Cambio guardado\n\n{}\n\n¿Todo correcto?", summary)
        )
        .parse_mode(ParseMode::Html)
        .reply_markup(confirm_keyboard())
        .await?;
    }
    
    Ok(())
}

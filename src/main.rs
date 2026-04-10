mod config;
mod db;
mod dialogue;
mod handlers;
mod ia;
mod payments;
mod text_processor;
mod wizard;

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{BotCommand, ParseMode};
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage};

use crate::config::Config;
use crate::db::Db;
use crate::handlers::{
    handle_callback, handle_chat, handle_help, handle_info, handle_messages,
    handle_register, handle_search, handle_search_query, handle_start,
    handle_approve_business, handle_reject_business, handle_pending_businesses,
    BotState,
    // Broadcast handlers
    start_broadcast, receive_broadcast_title, receive_broadcast_content, handle_broadcast_callback,
    handle_mis_difusiones, admin_add_credits,
    // Pagos handlers
    handle_mis_pagos,
    // Mis datos
    handle_mis_datos,
    // Admin handlers
    handle_admin_org, handle_admin_member, handle_admin_users,
};
use crate::ia::IaClient;
use crate::dialogue::states::{BotDialogueState, BroadcastState, SearchState};
use crate::handlers::handle_search_by_field_query;

/// Estado extendido del bot que incluye el diálogo combinado
type MyDialogue = Dialogue<BotDialogueState, InMemStorage<BotDialogueState>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Cargar variables de entorno desde .env
    dotenv::dotenv().ok();

    // Inicializar logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Iniciando bot...");

    // Cargar configuración
    let config = Arc::new(Config::load()?);
    tracing::info!("Configuración cargada: {}", config.bot.name);

    // Crear directorio de datos
    tokio::fs::create_dir_all("data").await.ok();

    // Inicializar base de datos
    let db = Arc::new(Db::new(&config.database.path()).await?);
    tracing::info!("Base de datos inicializada");

    // Inicializar cliente de IA (opcional)
    let ia = match std::env::var("KIMI_API_KEY") {
        Ok(api_key) => {
            match IaClient::new(Arc::new(config.api.clone()), api_key) {
                Ok(client) => {
                    tracing::info!("Cliente de IA inicializado");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    tracing::warn!("No se pudo inicializar el cliente de IA: {}", e);
                    None
                }
            }
        }
        Err(_) => {
            tracing::warn!("KIMI_API_KEY no configurada - funcionalidad de IA deshabilitada");
            None
        }
    };

    // Crear bot de Telegram (usar TELOXIDE_TOKEN_TEST si existe, sino TELOXIDE_TOKEN)
    let token = std::env::var("TELOXIDE_TOKEN_TEST")
        .or_else(|_| std::env::var("TELOXIDE_TOKEN"))
        .expect("TELOXIDE_TOKEN o TELOXIDE_TOKEN_TEST debe estar configurado");
    let bot = Bot::new(token);

    // Configurar comandos del bot
    let commands = vec![
        BotCommand::new("start", "Iniciar el bot"),
        BotCommand::new("help", "Mostrar ayuda"),
        BotCommand::new("info", "Información sobre la entidad"),
        BotCommand::new("chat", "Iniciar chat con IA"),
        BotCommand::new("buscar", "Buscar servicios (ej: /buscar consultoría)"),
        BotCommand::new("registrar", "Registrar empresa/autónomo"),
        BotCommand::new("misdatos", "Ver tus datos registrados"),
        BotCommand::new("mensajes", "Ver mensajes recibidos"),
        BotCommand::new("difundir", "Enviar difusión al canal"),
        BotCommand::new("mis_difusiones", "Ver mis difusiones y créditos"),
        BotCommand::new("comprar_difusion", "Comprar difusiones extra"),
        BotCommand::new("admin_add_credits", "[Admin] Añadir créditos a usuario"),
        BotCommand::new("admin_org", "[Admin] Configurar datos de la organización"),
        BotCommand::new("pendientes", "[Admin] Ver empresas pendientes de aprobación"),
        BotCommand::new("aprobar", "[Admin] Aprobar empresa (/aprobar <id>)"),
        BotCommand::new("rechazar", "[Admin] Rechazar empresa (/rechazar <id>)"),
    ];

    bot.set_my_commands(commands).await?;
    tracing::info!("Comandos del bot configurados");

    // Crear estado compartido
    let state = Arc::new(BotState {
        db: Arc::clone(&db),
        ia,
        config: Arc::clone(&config),
    });

    tracing::info!("Bot listo: {}", config.bot.name);

    // Dispatcher con soporte para diálogos
    let storage = InMemStorage::<BotDialogueState>::new();
    
    let handler = dptree::entry()
        .enter_dialogue::<Update, InMemStorage<BotDialogueState>, BotDialogueState>()
        .branch(
            Update::filter_message()
                .branch(
                    teloxide::filter_command::<Command, _>().endpoint(handle_commands),
                )
                .branch(dptree::endpoint(handle_text_messages)),
        )
        .branch(Update::filter_callback_query().endpoint(handle_callback_wrapper));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state, storage])
        .default_handler(|upd| async move {
            tracing::warn!("Unhandled update: {:?}", upd);
        })
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

#[derive(teloxide::macros::BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "Comandos disponibles:"
)]
enum Command {
    #[command(description = "Iniciar el bot")]
    Start,
    #[command(description = "Mostrar ayuda")]
    Help,
    #[command(description = "Información sobre la entidad")]
    Info,
    #[command(description = "Chat con IA")]
    Chat(String),
    #[command(description = "Buscar servicios")]
    Buscar(String),
    #[command(description = "Registrar empresa/autónomo")]
    Registrar,
    #[command(description = "Ver tus datos registrados")]
    MisDatos,
    #[command(description = "Ver mensajes recibidos")]
    Mensajes,
    #[command(description = "Enviar difusión al canal")]
    Difundir,
    #[command(description = "Ver mis difusiones y créditos", rename = "mis_difusiones")]
    MisDifusiones,
    #[command(description = "Comprar difusiones extra", rename = "comprar_difusion")]
    ComprarDifusion,
    #[command(description = "Ver historial de pagos", rename = "mis_pagos")]
    MisPagos,
    #[command(description = "[Admin] Añadir créditos a usuario (uso: /admin_add_credits <user_id> <cantidad>)", rename = "admin_add_credits")]
    AdminAddCredits(String),
    #[command(description = "[Admin] Ver empresas pendientes de aprobación")]
    Pendientes,
    #[command(description = "[Admin] Aprobar empresa (uso: /aprobar <empresa_id> o /aprobar para ver pendientes)")]
    Aprobar(String),
    #[command(description = "[Admin] Rechazar empresa (uso: /rechazar <empresa_id>)")]
    Rechazar(String),
    #[command(description = "[Admin] Configurar datos de la organización (uso: /admin_org <campo> <valor>)", rename = "admin_org")]
    AdminOrg(String),
    #[command(description = "[Admin] Cambiar estado de miembro (uso: /admin_member <user_id> on|off)", rename = "admin_member")]
    AdminMember(String),
    #[command(description = "[Admin] Buscar usuarios (uso: /admin_users <busqueda>)", rename = "admin_users")]
    AdminUsers(String),
}

async fn handle_commands(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    match cmd {
        Command::Start => handle_start(bot, msg, state).await,
        Command::Help => handle_help(bot, msg, state).await,
        Command::Info => handle_info(bot, msg, state).await,
        Command::Chat(text) => {
            if text.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "💬 <b>Chat con IA</b>\n\n\
                    Usa: <code>/chat tu mensaje aquí</code>\n\n\
                    Ejemplo: <code>/chat ¿Qué servicios ofreces?</code>",
                )
                .parse_mode(ParseMode::Html)
                .await?;
            } else {
                handle_chat(bot, msg, state, text).await?;
            }
            Ok(())
        }
        Command::Buscar(query) => {
            if query.is_empty() {
                // Start interactive search flow with field selection
                dialogue.update(BotDialogueState::Search(SearchState::WaitingForField)).await?;
                handle_search(bot, msg, dialogue).await
            } else {
                handle_search_query(bot, msg, state, query).await
            }
        }
        Command::Registrar => handle_register(bot, msg, state).await,
        Command::MisDatos => handle_mis_datos(bot, msg, state).await,
        Command::Mensajes => handle_messages(bot, msg, state).await,
        Command::Difundir => {
            dialogue.update(BotDialogueState::Broadcast(BroadcastState::default())).await?;
            start_broadcast(bot, msg, dialogue, state.db.clone(), state.config.clone()).await
        }
        Command::MisDifusiones => {
            handle_mis_difusiones(bot, msg, state).await
        }
        Command::ComprarDifusion => {
            buy_broadcast_info(bot, msg, state).await
        }
        Command::MisPagos => {
            handle_mis_pagos(bot, msg, state).await
        }
        Command::AdminAddCredits(args) => {
            handle_admin_add_credits(bot, msg, state, args).await
        }
        Command::Pendientes => {
            handle_pending_businesses(bot, msg, state).await
        }
        Command::Aprobar(args) => {
            let args = args.trim();
            if args.is_empty() {
                // Sin argumentos, mostrar pendientes
                handle_pending_businesses(bot, msg, state).await
            } else {
                // Intentar parsear el ID
                match args.parse::<i64>() {
                    Ok(empresa_id) => {
                        handle_approve_business(bot, msg, state, empresa_id).await
                    }
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Uso: /aprobar <id_empresa>\n\nUsa /pendientes para ver las empresas pendientes.").await?;
                        Ok(())
                    }
                }
            }
        }
        Command::Rechazar(args) => {
            let args = args.trim();
            if args.is_empty() {
                // Sin argumentos, mostrar pendientes
                handle_pending_businesses(bot, msg, state).await
            } else {
                // Intentar parsear el ID
                match args.parse::<i64>() {
                    Ok(empresa_id) => {
                        handle_reject_business(bot, msg, state, empresa_id).await
                    }
                    Err(_) => {
                        bot.send_message(msg.chat.id, "❌ Uso: /rechazar <id_empresa>\n\nUsa /pendientes para ver las empresas pendientes.").await?;
                        Ok(())
                    }
                }
            }
        }
        Command::AdminOrg(args) => {
            handle_admin_org(bot, msg, state, args).await
        }
        Command::AdminMember(args) => {
            handle_admin_member(bot, msg, state, args).await
        }
        Command::AdminUsers(query) => {
            handle_admin_users(bot, msg, state, query).await
        }
    }
}

async fn handle_text_messages(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    if let Some(text) = msg.clone().text() {
        let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
        
        // Verificar si está en wizard de registro
        if let Some(wiz_state) = crate::wizard::get_wizard_state(user_id) {
            return handle_wizard_text(bot, msg, state, wiz_state, text.to_string()).await;
        }
        
        // Obtener estado actual del diálogo
        let current_state = dialogue.get_or_default().await?;
        
        // Manejar flujos activos según el estado
        match current_state {
            BotDialogueState::Broadcast(BroadcastState::WaitingTitle) => {
                return receive_broadcast_title(bot, msg, dialogue, text.to_string()).await;
            }
            BotDialogueState::Broadcast(BroadcastState::WaitingContent { title }) => {
                return receive_broadcast_content(bot, msg, dialogue, title, text.to_string()).await;
            }
            BotDialogueState::Search(SearchState::WaitingQuery) => {
                dialogue.reset().await?;
                return handle_search_query(bot, msg, state, text.to_string()).await;
            }
            BotDialogueState::Search(SearchState::WaitingForQuery { field }) => {
                dialogue.reset().await?;
                return handle_search_by_field_query(bot, msg, state, field, text.to_string()).await;
            }
            BotDialogueState::Chat => {
                // Modo chat activo - procesar mensaje con IA
                return handle_chat(bot, msg, state, text.to_string()).await;
            }
            _ => {
                // No hay diálogo activo, procesar mensajes normales
            }
        }

        // Si no es registro ni diálogo activo, mostrar mensaje de ayuda
        bot.send_message(
            msg.chat.id,
            "🤖 No entiendo ese mensaje.\n\n\
            Usa /start para ver el menú principal.\n\
            Usa /chat <mensaje> para hablar con la IA."
        ).await?;
    }

    Ok(())
}

async fn handle_wizard_text(
    bot: Bot,
    msg: Message,
    _state: Arc<BotState>,
    wiz_state: crate::wizard::WizardState,
    text: String,
) -> anyhow::Result<()> {
    use crate::wizard::{WizardStep, RegistrationData, self};
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
    
    let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let chat_id = msg.chat.id;
    
    match wiz_state.step {
        WizardStep::AskName => {
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: Some(text.clone()),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::AskDescription);
            
            bot.send_message(
                chat_id,
                format!(
                    "✅ Nombre guardado: <b>{}</b>\n\n\
                    Paso 3/6: Escribe una <b>descripción breve</b> de tu negocio.\n\n\
                    💡 Consejo: Sé conciso, máximo 2-3 líneas.",
                    text
                )
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
            ]]))
            .await?;
        }
        WizardStep::AskDescription => {
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: Some(text.clone()),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::update_wizard_data(user_id, new_data.clone());
            wizard::set_wizard_step(user_id, WizardStep::AskCifChoice);
            
            bot.send_message(
                chat_id,
                "✅ Descripción guardada\n\n\
                Paso 4/6: ¿Quieres añadir tu <b>CIF/NIF</b>?\n\n\
                Esto ayuda a validar tu negocio."
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("✅ Sí", "wiz:cif:yes"),
                    InlineKeyboardButton::callback("⏭️ No / Saltar", "wiz:cif:no"),
                ],
                vec![
                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
                ],
            ]))
            .await?;
        }
        WizardStep::AskCif => {
            let new_data = RegistrationData {
                cif: Some(text.clone()),
                ..wiz_state.data.clone()
            };
            wizard::update_wizard_data(user_id, new_data.clone());
            wizard::set_wizard_step(user_id, WizardStep::AskContactChoice);
            
            bot.send_message(
                chat_id,
                format!(
                    "✅ CIF guardado: <b>{}</b>\n\n\
                    Paso 5/6: ¿Cómo quieres que te contacten?",
                    text
                )
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
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
            ]))
            .await?;
        }
        WizardStep::AskPhone => {
            let new_data = RegistrationData {
                phone: Some(text.clone()),
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            
            // Si el usuario eligió "both", ahora pedir email
            let wants_email = wiz_state.data.contact_method.as_deref() == Some("both");
            
            if wants_email {
                wizard::update_wizard_data(user_id, new_data.clone());
                wizard::set_wizard_step(user_id, WizardStep::AskEmail);
                
                bot.send_message(
                    chat_id,
                    format!(
                        "✅ Teléfono guardado: <b>{}</b>\n\n\
                        📧 Ahora escribe tu <b>email</b>:",
                        text
                    )
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel")
                ]]))
                .await?;
            } else {
                // Solo quería teléfono, ir a confirmación
                wizard::update_wizard_data(user_id, new_data.clone());
                wizard::set_wizard_step(user_id, WizardStep::Confirm);
                let summary = new_data.to_summary();
                
                bot.send_message(
                    chat_id,
                    format!("{}\n\n¿Todo correcto?", summary)
                )
                .parse_mode(ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![
                        InlineKeyboardButton::callback("✅ Confirmar registro", "wiz:confirm"),
                        InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
                    ],
                ]))
                .await?;
            }
        }
        WizardStep::AskEmail => {
            let new_data = RegistrationData {
                email: Some(text.clone()),
                ..wiz_state.data.clone()
            };
            wizard::update_wizard_data(user_id, new_data.clone());
            
            // En lugar de ir a Confirm, ir a AskAddCentro
            wizard::set_wizard_step(user_id, WizardStep::AskAddCentro);
            
            bot.send_message(
                chat_id,
                "📍 <b>Centros de trabajo</b>\n\n\
                ¿Deseas añadir un centro de trabajo?\n\
                Puedes añadir tu oficina, local, o ubicación principal."
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("✅ Sí, añadir centro", "wiz:centro:add"),
                    InlineKeyboardButton::callback("⏭️ No, continuar", "wiz:centro:skip"),
                ],
                vec![
                    InlineKeyboardButton::callback("❌ Cancelar", "wiz:cancel"),
                ],
            ]))
            .await?;
        }
        // ===== CENTROS =====
        WizardStep::CentroAskNombre => {
            let temp_centro = crate::wizard::CentroPendiente {
                nombre: text.clone(),
                ..Default::default()
            };
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: Some(temp_centro),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::CentroAskDireccion);
            
            bot.send_message(
                chat_id,
                format!("✅ Nombre guardado: <b>{}</b>\n\n¿Cuál es la <b>dirección</b>? (o escribe 'saltar')", text)
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::CentroAskDireccion => {
            let direccion = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_centro = wiz_state.data.temp_centro.clone().unwrap_or_default();
            temp_centro.direccion = direccion;
            let new_data = RegistrationData {
                temp_centro: Some(temp_centro),
                ..wiz_state.data.clone()
            };
            wizard::update_wizard_data(user_id, new_data);
            wizard::set_wizard_step(user_id, WizardStep::CentroAskCiudad);
            
            bot.send_message(
                chat_id,
                "✅ Dirección guardada\n\n¿En qué <b>ciudad</b> está? (o escribe 'saltar')"
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::CentroAskCiudad => {
            let ciudad = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_centro = wiz_state.data.temp_centro.clone().unwrap_or_default();
            temp_centro.ciudad = ciudad;
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: Some(temp_centro),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::CentroAskTelefono);
            
            bot.send_message(
                chat_id,
                "✅ Ciudad guardada\n\n¿Hay algún <b>teléfono</b> específico para este centro? (o 'saltar')"
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::CentroAskTelefono => {
            let telefono = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_centro = wiz_state.data.temp_centro.clone().unwrap_or_default();
            temp_centro.telefono = telefono;
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: Some(temp_centro),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::CentroAskEmail);
            
            bot.send_message(
                chat_id,
                "✅ Teléfono guardado\n\n¿Hay algún <b>email</b> específico para este centro? (o 'saltar')"
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::CentroAskEmail => {
            let email = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_centro = wiz_state.data.temp_centro.clone().unwrap_or_default();
            temp_centro.email = email;
            
            // Mostrar resumen del centro
            let mut summary = format!("📍 <b>Resumen del centro</b>\n\n<b>Nombre:</b> {}\n", temp_centro.nombre);
            if let Some(ref d) = temp_centro.direccion { summary.push_str(&format!("<b>Dirección:</b> {}\n", d)); }
            if let Some(ref c) = temp_centro.ciudad { summary.push_str(&format!("<b>Ciudad:</b> {}\n", c)); }
            if let Some(ref t) = temp_centro.telefono { summary.push_str(&format!("<b>Teléfono:</b> {}\n", t)); }
            if let Some(ref e) = temp_centro.email { summary.push_str(&format!("<b>Email:</b> {}\n", e)); }
            
            let new_data = RegistrationData {
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
                temp_centro: Some(temp_centro),
                temp_servicio: wiz_state.data.temp_servicio.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::CentroConfirm);
            
            bot.send_message(
                chat_id,
                format!("{}\n\n¿Guardar este centro?", summary)
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("✅ Guardar centro", "wiz:centro:save"),
                    InlineKeyboardButton::callback("🗑️ Descartar", "wiz:centro:discard"),
                ],
            ]))
            .await?;
        }
        // ===== SERVICIOS =====
        WizardStep::ServicioAskCategoria => {
            let mut temp_servicio = wiz_state.data.temp_servicio.clone().unwrap_or_default();
            temp_servicio.categoria = text.clone();
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
            wizard::set_wizard_step(user_id, WizardStep::ServicioAskNombre);
            
            bot.send_message(
                chat_id,
                format!("✅ Categoría guardada: <b>{}</b>\n\n¿Cuál es el <b>nombre</b> del servicio/producto?", text)
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::ServicioAskNombre => {
            let mut temp_servicio = wiz_state.data.temp_servicio.clone().unwrap_or_default();
            temp_servicio.nombre = text.clone();
            let new_data = RegistrationData {
                temp_servicio: Some(temp_servicio),
                centros: wiz_state.data.centros.clone(),
                servicios: wiz_state.data.servicios.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::ServicioAskDescripcion);
            
            bot.send_message(
                chat_id,
                "✅ Nombre guardado\n\nEscribe una <b>descripción breve</b> (o 'saltar'):"
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::ServicioAskDescripcion => {
            let descripcion = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_servicio = wiz_state.data.temp_servicio.clone().unwrap_or_default();
            temp_servicio.descripcion = descripcion;
            let new_data = RegistrationData {
                temp_servicio: Some(temp_servicio),
                centros: wiz_state.data.centros.clone(),
                servicios: wiz_state.data.servicios.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::ServicioAskPrecio);
            
            bot.send_message(
                chat_id,
                "✅ Descripción guardada\n\n¿Cuál es el <b>precio</b>?\n\
                Ej: '50€/hora', 'A consultar', 'Desde 100€' (o 'saltar')"
            )
            .parse_mode(ParseMode::Html)
            .await?;
        }
        WizardStep::ServicioAskPrecio => {
            let precio = if text.eq_ignore_ascii_case("saltar") { None } else { Some(text.clone()) };
            let mut temp_servicio = wiz_state.data.temp_servicio.clone().unwrap_or_default();
            temp_servicio.precio = precio;
            
            // Mostrar resumen del servicio
            let mut summary = "🛎️ <b>Resumen del servicio/producto</b>\n\n".to_string();
            summary.push_str(&format!("<b>Tipo:</b> {}\n", temp_servicio.tipo));
            summary.push_str(&format!("<b>Categoría:</b> {}\n", temp_servicio.categoria));
            summary.push_str(&format!("<b>Nombre:</b> {}\n", temp_servicio.nombre));
            if let Some(ref d) = temp_servicio.descripcion { summary.push_str(&format!("<b>Descripción:</b> {}\n", d)); }
            if let Some(ref p) = temp_servicio.precio { summary.push_str(&format!("<b>Precio:</b> {}\n", p)); }
            
            let new_data = RegistrationData {
                temp_servicio: Some(temp_servicio),
                centros: wiz_state.data.centros.clone(),
                servicios: wiz_state.data.servicios.clone(),
                temp_centro: wiz_state.data.temp_centro.clone(),
                user_type: wiz_state.data.user_type.clone(),
                name: wiz_state.data.name.clone(),
                description: wiz_state.data.description.clone(),
                cif: wiz_state.data.cif.clone(),
                phone: wiz_state.data.phone.clone(),
                email: wiz_state.data.email.clone(),
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
            wizard::set_wizard_step(user_id, WizardStep::ServicioConfirm);
            
            bot.send_message(
                chat_id,
                format!("{}\n\n¿Guardar este servicio/producto?", summary)
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("✅ Guardar", "wiz:servicio:save"),
                    InlineKeyboardButton::callback("🗑️ Descartar", "wiz:servicio:discard"),
                ],
            ]))
            .await?;
        }
        _ => {
            // No debería llegar aquí, pero por si acaso
            bot.send_message(chat_id, "❌ Error inesperado. Por favor, usa /registrar para empezar de nuevo.").await?;
            wizard::clear_wizard(user_id);
        }
    }
    
    Ok(())
}

async fn handle_callback_wrapper(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    if let Some(data) = q.data.clone() {
        // Obtener estado actual
        let current_state = dialogue.get_or_default().await?;
        
        // Primero intentar manejar callbacks de broadcast
        if data.starts_with("broadcast_") || data == "buy_broadcast" {
            if let BotDialogueState::Broadcast(broadcast_state) = current_state {
                handle_broadcast_callback(bot, q, dialogue, broadcast_state, state.db.clone(), state.config.clone()).await?;
                return Ok(());
            }
        }
    }
    
    // Si no es de broadcast, manejar con el handler general
    handle_callback(bot, q, state, dialogue).await
}

/// Información sobre comprar difusiones
async fn buy_broadcast_info(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let price = state.config.broadcast.payment_price.unwrap_or(5.0);
    let user_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    
    bot.send_message(
        msg.chat.id,
        format!(
            "💳 <b>Comprar difusión extra</b>\n\n\
            Precio: <b>{:.2}€</b> por difusión adicional\n\n\
            Para comprar, contacta con un administrador indicando tu ID de usuario.\n\
            Tu ID: <code>{}</code>\n\n\
            Una vez realizado el pago, un administrador te añadirá los créditos.",
            price, user_id
        )
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await?;
    
    Ok(())
}

/// Handler para comando admin_add_credits
async fn handle_admin_add_credits(
    bot: Bot,
    msg: Message,
    state: Arc<BotState>,
    args: String,
) -> anyhow::Result<()> {
    // Verificar que el usuario es admin (por config)
    let admin_id = msg.from().as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if !state.config.bot.admins.contains(&admin_id) {
        bot.send_message(msg.chat.id, "⛔ Solo administradores pueden usar este comando.").await?;
        return Ok(());
    }
    
    // Parsear argumentos: user_id credits
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() != 2 {
        bot.send_message(
            msg.chat.id,
            "❌ Uso: /admin_add_credits <user_id> <cantidad>\nEjemplo: /admin_add_credits 123456789 5"
        ).await?;
        return Ok(());
    }
    
    let target_user_id: i64 = match parts[0].parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "❌ El user_id debe ser un número válido.").await?;
            return Ok(());
        }
    };
    
    let credits: i32 = match parts[1].parse() {
        Ok(c) => c,
        Err(_) => {
            bot.send_message(msg.chat.id, "❌ La cantidad debe ser un número válido.").await?;
            return Ok(());
        }
    };
    
    if credits <= 0 {
        bot.send_message(msg.chat.id, "❌ La cantidad debe ser mayor que 0.").await?;
        return Ok(());
    }
    
    // Llamar a la función de broadcast para añadir créditos
    admin_add_credits(bot, msg, state.db.clone(), state.config.clone(), target_user_id, credits).await
}

//! Módulo de pagos simplificado (sin servidor webhook separado)
//! 
//! Los pagos se manejan vía Stripe Checkout y se confirman 
//! cuando el usuario vuelve al bot después del pago.

pub mod webhook;

use stripe::{CheckoutSession, CheckoutSessionMode, Client, CreateCheckoutSession, CreateCheckoutSessionLineItems, CreateCheckoutSessionLineItemsPriceData, CreateCheckoutSessionLineItemsPriceDataProductData, Currency};
use crate::config::StripeConfig;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct CreditPack {
    pub name: &'static str,
    pub credits: i32,
    pub price_eur: f64,
}

pub const CREDIT_PACKS: &[CreditPack] = &[
    CreditPack { name: "S", credits: 5, price_eur: 3.0 },
    CreditPack { name: "M", credits: 10, price_eur: 5.0 },
    CreditPack { name: "L", credits: 25, price_eur: 10.0 },
];

pub struct StripeClient {
    client: Client,
    config: Arc<StripeConfig>,
}

impl StripeClient {
    pub fn new(config: Arc<StripeConfig>) -> Self {
        let client = Client::new(&config.secret_key);
        Self { client, config }
    }

    pub async fn create_checkout_session(
        &self,
        user_id: i64,
        pack: &CreditPack,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let success_url = format!("{}/payment/success?session_id={{CHECKOUT_SESSION_ID}}", self.config.base_url);
        let cancel_url = format!("{}/payment/cancel", self.config.base_url);

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("telegram_id".to_string(), user_id.to_string());
        metadata.insert("credits".to_string(), pack.credits.to_string());
        metadata.insert("pack_name".to_string(), pack.name.to_string());

        let session = CheckoutSession::create(&self.client, CreateCheckoutSession {
            line_items: Some(vec![CreateCheckoutSessionLineItems {
                price_data: Some(CreateCheckoutSessionLineItemsPriceData {
                    currency: Currency::EUR,
                    product_data: Some(CreateCheckoutSessionLineItemsPriceDataProductData {
                        name: format!("{} difusiones adicionales", pack.credits),
                        description: Some(format!("Pack {} - {} créditos para difusiones", pack.name, pack.credits)),
                        ..Default::default()
                    }),
                    unit_amount: Some((pack.price_eur * 100.0) as i64),
                    ..Default::default()
                }),
                quantity: Some(1),
                ..Default::default()
            }]),
            mode: Some(CheckoutSessionMode::Payment),
            success_url: Some(&success_url),
            cancel_url: Some(&cancel_url),
            client_reference_id: Some(&format!("{}_{}", user_id, pack.credits)),
            metadata: Some(metadata),
            ..Default::default()
        }).await?;

        Ok(session.url.ok_or("No checkout URL returned")?.to_string())
    }
}

pub fn format_packs_keyboard() -> Vec<Vec<teloxide::types::InlineKeyboardButton>> {
    use teloxide::types::InlineKeyboardButton;
    
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = CREDIT_PACKS
        .iter()
        .map(|pack| {
            vec![InlineKeyboardButton::callback(
                format!("📦 {}: {} créditos - {:.0}€", pack.name, pack.credits, pack.price_eur),
                format!("buy_pack:{}", pack.name)
            )]
        })
        .collect();
    
    buttons.push(vec![InlineKeyboardButton::callback(
        "❓ ¿Cuántas necesito?",
        "calc_needs"
    )]);
    
    buttons.push(vec![InlineKeyboardButton::callback(
        "🔙 Volver",
        "menu:difusiones"
    )]);
    
    buttons
}

pub fn get_pack_by_name(name: &str) -> Option<&'static CreditPack> {
    CREDIT_PACKS.iter().find(|p| p.name == name)
}

/// Obtener pack por nombre (versión owned para broadcast_extended)
pub fn get_pack_by_name_owned(name: &str) -> Option<CreditPack> {
    CREDIT_PACKS.iter().find(|p| p.name == name).copied()
}

/// Estados de mensaje de pago
#[derive(Debug, Clone)]
pub enum PaymentStatus {
    Pending,
    Success { credits: i32, amount: f64 },
    Failed { reason: String },
    Cancelled,
}

impl PaymentStatus {
    pub fn to_message(&self) -> String {
        match self {
            PaymentStatus::Pending => {
                "⏳ *Pago en proceso*\n\n\
                Estamos esperando confirmación del pago.\n\
                Esto suele tardar unos segundos...\n\n\
                Te notificaremos cuando se complete."
                    .to_string()
            }
            PaymentStatus::Success { credits, amount } => {
                format!(
                    "✅ *¡Pago completado!*\n\n\
                    💳 Monto: *{:.2}€*\n\
                    🎁 Créditos añadidos: *{}*\n\n\
                    Ya puedes usar tus nuevas difusiones con /difundir\n\n\
                    💡 Ver tu saldo: /mis_difusiones\n\
                    📜 Ver historial: /mis_pagos",
                    amount, credits
                )
            }
            PaymentStatus::Failed { reason } => {
                format!(
                    "❌ *Pago fallido*\n\n\
                    Razón: {}\n\n\
                    Puedes intentarlo de nuevo con /comprar\n\n\
                    💡 Si el problema persiste, contacta con soporte.",
                    reason
                )
            }
            PaymentStatus::Cancelled => {
                "🚫 *Pago cancelado*\n\n\
                Has cancelado el proceso de pago.\n\n\
                Puedes intentarlo de nuevo cuando quieras con /comprar"
                    .to_string()
            }
        }
    }
}

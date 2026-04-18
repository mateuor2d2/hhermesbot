use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
    body::Bytes,
    Router,
};
use sqlx::SqlitePool;

use crate::payments::webhook::{handle_stripe_webhook_payload, WebhookConfig};

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    webhook_secret: String,
}

pub async fn start_server(port: u16, pool: SqlitePool, webhook_secret: String) {
    let state = AppState {
        pool,
        webhook_secret,
    };
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/stripe/webhook", post(stripe_webhook))
        .route("/payment/success", get(payment_success))
        .route("/payment/cancel", get(payment_cancel))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting web server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind web server: {}", e);
            return;
        }
    };

    if let Err(e) = axum::Server::from_tcp(listener.into_std().unwrap())
        .unwrap()
        .serve(app.into_make_service())
        .await
    {
        tracing::error!("Web server error: {}", e);
    }
}

async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let signature = headers
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let payload = String::from_utf8_lossy(&body);

    let config = WebhookConfig {
        stripe_webhook_secret: state.webhook_secret.clone(),
        db_pool: state.pool.clone(),
    };

    match handle_stripe_webhook_payload(&payload, signature, &config).await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            tracing::error!("Stripe webhook error: {}", e);
            StatusCode::BAD_REQUEST
        }
    }
}

async fn payment_success() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Pago completado</title></head>
<body style="font-family:sans-serif;text-align:center;padding:40px;">
  <h1>✅ Pago completado</h1>
  <p>Tu pago se ha procesado correctamente.</p>
  <p>Ya puedes volver al bot de Telegram.</p>
</body>
</html>"#)
}

async fn payment_cancel() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Pago cancelado</title></head>
<body style="font-family:sans-serif;text-align:center;padding:40px;">
  <h1>Pago cancelado</h1>
  <p>El pago fue cancelado o no se completó.</p>
  <p>Puedes intentarlo de nuevo desde el bot.</p>
</body>
</html>"#)
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

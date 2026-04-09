use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub bot: BotConfig,
    pub api: ApiConfig,
    pub limits: LimitsConfig,
    pub database: DatabaseConfig,
    pub features: FeaturesConfig,
    pub broadcast: BroadcastConfig,
    pub access: AccessConfig,
    pub subscriptions: SubscriptionsConfig,
    pub instance: InstanceConfig,
    pub membership: MembershipConfig,
    pub stripe: StripeConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BotConfig {
    pub name: String,
    pub description: String,
    pub token: String,
    pub admins: Vec<i64>,
    pub relay_timeout: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LimitsConfig {
    pub max_ia_messages_per_day: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DatabaseConfig {
    Sqlite { path: String },
    Postgres {
        host: String,
        port: u16,
        name: String,
        user: String,
        password: String,
    },
}

impl DatabaseConfig {
    /// Genera connection string para PostgreSQL o path para SQLite
    #[allow(dead_code)]
    pub fn connection_string(&self) -> String {
        match self {
            DatabaseConfig::Sqlite { path } => path.clone(),
            DatabaseConfig::Postgres { host, port, name, user, password } => {
                format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, name)
            }
        }
    }

    /// Devuelve el path del archivo para SQLite
    pub fn path(&self) -> String {
        match self {
            DatabaseConfig::Sqlite { path } => path.clone(),
            DatabaseConfig::Postgres { .. } => "postgres".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn is_postgres(&self) -> bool {
        matches!(self, DatabaseConfig::Postgres { .. })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct FeaturesConfig {
    pub enable_ia_chat: bool,
    pub enable_services: bool,
    pub enable_search: bool,
    pub enable_messaging: bool,
    pub enable_relay: bool,
    pub relay_target_chat_id: i64,
    pub enable_company_registration: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BroadcastConfig {
    pub enabled: bool,
    /// Difusiones gratuitas por trimestre
    pub quarterly_limit: i32,
    /// Canal de difusión público (donde se publican)
    pub channel_id: String,
    /// Canal de consultas IA (opcional, para separar tráfico)
    pub consultation_chat_id: Option<String>,
    /// Permitir comprar difusiones adicionales
    pub payment_enabled: bool,
    /// Precio por difusión adicional (en euros)
    pub payment_price: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AccessConfig {
    /// Cualquiera puede buscar/ver empresas sin registrar
    pub public_search: bool,
    /// Solo usuarios registrados pueden usar IA
    pub registered_only_chat: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SubscriptionsConfig {
    /// Deben estar suscritos al canal de difusión para difundir
    pub required_for_broadcast: bool,
    /// Verificar suscripción automáticamente al enviar difusión
    pub auto_check: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct StripeConfig {
    /// Clave secreta de Stripe (SK)
    pub secret_key: String,
    /// Clave pública de Stripe (PK) - para frontend/payment links
    pub publishable_key: String,
    /// Webhook secret para validar eventos
    pub webhook_secret: String,
    /// Precio por difusión adicional (en céntimos/euros)
    pub price_per_broadcast: f64,
    /// ID del producto en Stripe (opcional, para referencia)
    pub product_id: Option<String>,
    /// ID del precio en Stripe (para payment links/API)
    pub price_id: Option<String>,
    /// URL base del bot (para webhooks y redirects)
    pub base_url: String,
    /// Modo test/live
    #[serde(default = "default_stripe_test_mode")]
    pub test_mode: bool,
}

fn default_stripe_test_mode() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct InstanceConfig {
    /// Identificador único de esta instancia (ej: "colegio_ingenieros", "pimem", "camara")
    pub bot_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MembershipConfig {
    /// Colegio o institución asociada (si está vacío usa bot.name)
    pub institution: String,
    /// Número de colegiado requerido
    pub requires_membership_number: bool,
    /// Colegio/profesionales pueden buscar contactos de otros miembros
    pub enable_member_search: bool,
    /// Verificar membresía antes de permitir búsqueda
    pub verify_membership: bool,
    /// Si true, solo miembros de la organización pueden registrarse
    #[serde(default)]
    pub exclusive_to_members: bool,
    /// Precio mensual para no-miembros (si exclusive_to_members = false)
    #[serde(default)]
    pub price: Option<f64>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let content = fs::read_to_string("config.toml")?;
        // Expandir variables de entorno ${VAR} antes de parsear
        let expanded = Self::expand_env_vars(&content);
        let config: Config = toml::from_str(&expanded)?;
        Ok(config)
    }

    /// Expande variables de entorno en formato ${VAR} o ${VAR:-default}
    fn expand_env_vars(content: &str) -> String {
        use regex::Regex;
        use std::env;

        let re = Regex::new(r"\$\{(\w+)(?::-([^}]*))?\}").unwrap();
        
        re.replace_all(content, |caps: &regex::Captures| {
            let var_name = &caps[1];
            let default_val = caps.get(2).map(|m| m.as_str());
            
            match env::var(var_name) {
                Ok(val) => val,
                Err(_) => default_val.unwrap_or("").to_string(),
            }
        }).to_string()
    }
}

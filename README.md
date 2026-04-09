# Bot Multi-Entidad para Telegram

Bot de Telegram en Rust con chat IA, registro de empresas/autónomos, y sistema de mensajería.

## 🚀 Características

- **Multi-entidad**: Un bot configurable para diferentes entidades (Colegio de Ingenieros, PIMEM, etc.)
- **Chat con IA**: Integración con API externa (OpenAI, etc.) con límite diario configurable
- **Registro de empresas**: Empresas y autónomos pueden registrarse y ofrecer servicios
- **Búsqueda pública**: Usuarios externos pueden buscar servicios por categoría
- **Mensajería interna**: Contacto entre usuarios
- **Persistencia**: SQLite local

## 📁 Estructura

```
colegio-bot/
├── config.toml         # Configuración por entidad
├── .env                # Variables de entorno (tokens)
├── migrations/         # Esquema de base de datos
└── src/
    ├── main.rs         # Entry point
    ├── config.rs       # Config parser
    ├── db.rs           # Base de datos
    ├── handlers.rs     # Comandos Telegram
    └── ia.rs           # Cliente IA
```

## ⚙️ Configuración

### 1. Configurar entidad (config.toml)

```toml
[bot]
name = "Colegio de Ingenieros"
description = "Bot de servicios para empresas"
admin_username = "admin"

[limits]
ia_daily_limit = 15

[database]
path = "data/bot.db"

[api]
ia_url = "https://api.openai.com/v1/chat/completions"
ia_model = "gpt-4o-mini"
```

### 2. Variables de entorno (.env)

```bash
TELOXIDE_TOKEN=123456789:ABC...       # Token de @BotFather
IA_API_KEY=sk-...                     # API Key de OpenAI/Anthropic
```

## 🏗️ Compilar y ejecutar

```bash
# Instalar dependencias
sudo apt-get install pkg-config libssl-dev

# Compilar
cargo build --release

# Ejecutar
TELOXIDE_TOKEN=xxx IA_API_KEY=yyy cargo run
```

## 🐳 Docker

```bash
# Build
docker build -t colegio-bot .

# Run
docker run -d \
  -e TELOXIDE_TOKEN=xxx \
  -e IA_API_KEY=yyy \
  -v $(pwd)/data:/app/data \
  colegio-bot
```

## 📋 Comandos del bot

| Comando | Descripción |
|---------|-------------|
| `/start` | Iniciar y seleccionar tipo de usuario |
| `/help` | Mostrar ayuda |
| `/info` | Información de la entidad |
| `/chat` | Chat con IA (15 msgs/día) |
| `/buscar [término]` | Buscar servicios |
| `/registrar` | Registrar empresa (internos) |
| `/mensajes` | Ver mensajes recibidos |

## 🏢 Multi-entidad

Para desplegar el mismo bot para diferentes entidades:

```bash
# 1. Crear directorio por entidad
mkdir /bots/colegio-ingenieros
mkdir /bots/pimem

# 2. Copiar archivos
cp -r colegio-bot/* /bots/colegio-ingenieros/
cp -r colegio-bot/* /bots/pimem/

# 3. Editar config.toml en cada uno
# colegio-ingenieros/config.toml → name = "Colegio de Ingenieros"
# pimem/config.toml → name = "PIMEM"

# 4. Crear .env con tokens diferentes

# 5. Ejecutar independientes
cd /bots/colegio-ingenieros && docker-compose up -d
cd /bots/pimem && docker-compose up -d
```

Cada instancia es **completamente independiente**:
- Base de datos propia
- Token de bot propio
- Configuración propia

## 🗄️ Esquema de datos

### Usuarios
- `telegram_id`, `username`, `user_type` (external/internal)

### Empresas/Autónomos
- `business_type`, `name`, `cif`, `contacto`

### Servicios
- `category`, `name`, `description`, `price`

### Centros
- Ubicaciones donde se ofrecen servicios

### Horarios
- Disponibilidad por centro y servicio

## 🔒 Seguridad

- Límite de mensajes IA por usuario/día
- Validación de tipos de usuario
- Sanitización de inputs
- No se comparten datos entre entidades

## 📄 Licencia

MIT

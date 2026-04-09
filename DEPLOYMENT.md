# Colegio Bot - SaaS Deployment Guide

## Overview

Multi-tenant Telegram bot for professional associations, colleges, and business organizations.

## Features

- **Multi-entity**: Single codebase, multiple independent deployments
- **AI Chat**: Integration with Kimi/OpenAI with daily limits
- **Business Registry**: Companies and freelancers can register services
- **Public Search**: Anyone can search for services by category
- **Broadcast System**: Limited announcements to channel (with credit system)
- **Payment Integration**: Stripe for buying extra broadcast credits
- **Admin Controls**: User management, approvals, organization settings

## Architecture

```
┌─────────────────┐
│  Telegram API   │
└────────┬────────┘
         │
┌────────▼────────┐
│   Colegio Bot   │  (Rust + Teloxide)
│   (Per Entity)  │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
┌───▼───┐  ┌──▼────┐
│SQLite │  │  AI   │  (Kimi/OpenAI)
│/Postgres  └───────┘
└───────┘
```

Each entity runs its own isolated instance with:
- Dedicated Telegram bot token
- Separate database
- Independent configuration
- Isolated user base

## Quick Start

### 1. Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Docker (optional, for deployment)
curl -fsSL https://get.docker.com | sh
```

### 2. Configuration

Copy and edit configuration:

```bash
cp config.toml config.production.toml
```

Key settings to customize:
- `[bot]` - Name, description, admin IDs
- `[api]` - AI service configuration
- `[broadcast]` - Channel ID, limits
- `[stripe]` - Payment configuration
- `[membership]` - Organization details

### 3. Environment Variables

Create `.env`:

```bash
# Required
TELOXIDE_TOKEN=your_bot_token_from_botfather

# Optional - AI features
KIMI_API_KEY=your_kimi_api_key

# Optional - Payments
STRIPE_SECRET_KEY=sk_test_...
STRIPE_PUBLISHABLE_KEY=pk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...

# Optional - Logging
RUST_LOG=info
```

### 4. Run

**Development:**
```bash
cargo run
```

**Production with Docker:**
```bash
docker-compose up -d
```

## Multi-Entity Deployment

Deploy the same bot for multiple organizations:

```bash
# Create directories for each entity
mkdir -p /opt/bots/{colegio-ingenieros,pimem,camara}

# Copy files to each
cp -r colegio-bot/* /opt/bots/colegio-ingenieros/
cp -r colegio-bot/* /opt/bots/pimem/

# Customize config.toml in each directory
# - Different bot tokens
# - Different database paths
# - Different branding

# Start all
cd /opt/bots/colegio-ingenieros && docker-compose up -d
cd /opt/bots/pimem && docker-compose up -d
```

## Database Schema

Core tables:
- `users` - Telegram users
- `empresas` - Registered businesses
- `servicios` - Services/products offered
- `mensajes` - Internal messages between users
- `broadcast_credits` - Broadcast credit tracking
- `organization` - Entity information

Migrations run automatically on startup.

## Bot Commands

| Command | Description | Access |
|---------|-------------|--------|
| `/start` | Welcome & user type selection | All |
| `/help` | Show help | All |
| `/info` | Organization information | All |
| `/chat <msg>` | AI chat | Members |
| `/buscar <term>` | Search services | All |
| `/registrar` | Register business | Members |
| `/misdatos` | View my data | Registered |
| `/mensajes` | View messages | Registered |
| `/difundir` | Send broadcast | Members |
| `/mis_difusiones` | My broadcasts & credits | Members |
| `/comprar_difusion` | Buy extra credits | Members |
| `/pendientes` | Pending approvals | Admin |
| `/aprobar <id>` | Approve business | Admin |
| `/rechazar <id>` | Reject business | Admin |
| `/admin_add_credits` | Add credits to user | Admin |

## Security Considerations

1. **Bot Token**: Keep `TELOXIDE_TOKEN` secret - never commit to git
2. **API Keys**: Store AI and Stripe keys in environment variables
3. **Database**: SQLite is fine for single-bot; use PostgreSQL for high-load
4. **Webhooks**: Use HTTPS in production for Stripe webhooks
5. **Admin IDs**: Verify admin Telegram IDs carefully

## Monitoring

Logs are structured with `tracing`:

```bash
# View logs
docker-compose logs -f bot

# With specific level
RUST_LOG=debug cargo run
```

## Backup

Database is in `data/bot.db`:

```bash
# Backup
cp data/bot.db backup/$(date +%Y%m%d).db

# Restore
cp backup/20240101.db data/bot.db
```

## Troubleshooting

**Bot not responding:**
- Check `TELOXIDE_TOKEN` is correct
- Verify bot is started with BotFather
- Check logs: `docker-compose logs`

**AI not working:**
- Verify `KIMI_API_KEY` is set
- Check API credits/billing

**Database errors:**
- Ensure `data/` directory is writable
- Check disk space

## License

MIT - See LICENSE file
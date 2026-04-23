-- Migración: Crear tabla payments para historial de pagos Stripe
-- El webhook de Stripe inserta aquí tras checkout.session.completed

CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,            -- Telegram ID del usuario
    stripe_session_id TEXT NOT NULL UNIQUE,  -- ID de sesión de Stripe (para idempotencia)
    amount REAL NOT NULL,                    -- Cantidad pagada en euros
    credits INTEGER NOT NULL DEFAULT 0,      -- Créditos de difusión comprados (0 para membresía)
    pack_name TEXT,                          -- 'membership', 'pack_5', 'pack_10', etc.
    status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'completed', 'failed', 'refunded')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_payments_telegram ON payments(telegram_id);
CREATE INDEX IF NOT EXISTS idx_payments_session ON payments(stripe_session_id);
CREATE INDEX IF NOT EXISTS idx_payments_created ON payments(created_at);

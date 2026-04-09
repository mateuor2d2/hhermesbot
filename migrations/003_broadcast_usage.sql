-- Migration: Broadcast usage tracking for quarterly limits
-- Created: 2026-02-28
-- Updated: 2026-02-28 - Fixed field names to match code

-- Table to track broadcast usage per user per quarter
CREATE TABLE IF NOT EXISTS broadcast_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,            -- Telegram ID directamente (como el resto del sistema)
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL CHECK (quarter BETWEEN 1 AND 4),
    count INTEGER DEFAULT 0,                 -- Free broadcasts used this quarter (renamed from free_used)
    paid_extra INTEGER DEFAULT 0,            -- Additional paid broadcasts purchased
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(telegram_id, year, quarter)       -- One record per user per quarter
);

-- Index for fast lookups
CREATE INDEX IF NOT EXISTS idx_broadcast_user_period 
    ON broadcast_usage(telegram_id, year, quarter);

-- Index for admin queries
CREATE INDEX IF NOT EXISTS idx_broadcast_period 
    ON broadcast_usage(year, quarter);

-- Table to track individual broadcasts (audit trail)
CREATE TABLE IF NOT EXISTS broadcasts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,                -- Telegram ID del usuario
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    channel_message_id INTEGER,              -- Message ID in the broadcast channel
    is_paid BOOLEAN DEFAULT FALSE,           -- Was this a paid broadcast?
    sent_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_broadcasts_user ON broadcasts(user_id);
CREATE INDEX IF NOT EXISTS idx_broadcasts_sent ON broadcasts(sent_at);

-- Table for payment records (for extra broadcasts)
CREATE TABLE IF NOT EXISTS broadcast_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,            -- Telegram ID del usuario
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,
    amount REAL NOT NULL,                    -- Amount paid (renamed from amount_eur)
    broadcasts_added INTEGER NOT NULL,       -- How many extra broadcasts (renamed from broadcasts_granted)
    payment_method TEXT,                     -- 'stripe', 'paypal', 'manual', etc.
    payment_reference TEXT,                  -- External transaction ID
    status TEXT DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    paid_at DATETIME,                        -- Renamed from completed_at
    verified_by INTEGER                      -- Admin who verified the payment
);

CREATE INDEX IF NOT EXISTS idx_payments_user ON broadcast_payments(telegram_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON broadcast_payments(status);

-- Table for caching subscription checks (to avoid rate limits)
CREATE TABLE IF NOT EXISTS subscription_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    is_member BOOLEAN NOT NULL,
    checked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    UNIQUE(telegram_id, channel_id)
);

CREATE INDEX IF NOT EXISTS idx_subscription_checks_expires 
    ON subscription_checks(expires_at);

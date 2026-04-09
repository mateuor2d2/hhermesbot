-- Migración inicial: usuarios, empresas y servicios

-- Usuarios registrados (internos = ofertantes, externos = buscadores)
CREATE TABLE IF NOT EXISTS users (
    telegram_id INTEGER PRIMARY KEY,
    username TEXT,
    first_name TEXT,
    last_name TEXT,
    phone TEXT,
    email TEXT,
    is_internal BOOLEAN NOT NULL DEFAULT FALSE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Empresas (sociedades o autónomos)
CREATE TABLE IF NOT EXISTS empresas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('autonomo', 'sociedad')),
    nombre_fiscal TEXT NOT NULL,
    nombre_comercial TEXT,
    cif_nif TEXT UNIQUE,
    direccion TEXT,
    codigo_postal TEXT,
    ciudad TEXT,
    provincia TEXT,
    telefono TEXT,
    email TEXT,
    web TEXT,
    descripcion TEXT,
    activa BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (telegram_id) REFERENCES users(telegram_id) ON DELETE CASCADE
);

-- Centros (ubicaciones donde se ofrecen servicios)
CREATE TABLE IF NOT EXISTS centros (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    empresa_id INTEGER NOT NULL,
    nombre TEXT NOT NULL,
    direccion TEXT,
    ciudad TEXT,
    telefono TEXT,
    email TEXT,
    activo BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (empresa_id) REFERENCES empresas(id) ON DELETE CASCADE
);

-- Servicios/Bienes ofrecidos
CREATE TABLE IF NOT EXISTS servicios (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    empresa_id INTEGER NOT NULL,
    tipo TEXT NOT NULL CHECK (tipo IN ('bien', 'servicio')),
    categoria TEXT NOT NULL,
    nombre TEXT NOT NULL,
    descripcion TEXT,
    precio TEXT,  -- "A consultar", "50€/hora", etc
    disponible BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (empresa_id) REFERENCES empresas(id) ON DELETE CASCADE
);

-- Horarios de servicios por centro
CREATE TABLE IF NOT EXISTS horarios (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    servicio_id INTEGER NOT NULL,
    centro_id INTEGER,
    dia_semana INTEGER NOT NULL CHECK (dia_semana BETWEEN 0 AND 6),  -- 0=Domingo
    hora_inicio TIME,
    hora_fin TIME,
    notas TEXT,
    FOREIGN KEY (servicio_id) REFERENCES servicios(id) ON DELETE CASCADE,
    FOREIGN KEY (centro_id) REFERENCES centros(id) ON DELETE SET NULL
);

-- Contador de mensajes IA por usuario (reset diario)
CREATE TABLE IF NOT EXISTS ia_usage (
    telegram_id INTEGER PRIMARY KEY,
    messages_today INTEGER NOT NULL DEFAULT 0,
    last_reset_date DATE NOT NULL DEFAULT CURRENT_DATE,
    total_messages INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (telegram_id) REFERENCES users(telegram_id) ON DELETE CASCADE
);

-- Mensajes entre usuarios (sistema interno de mensajería)
CREATE TABLE IF NOT EXISTS mensajes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    remitente_id INTEGER NOT NULL,
    destinatario_id INTEGER NOT NULL,
    empresa_id INTEGER,  -- Opcional: mensaje relacionado con empresa
    servicio_id INTEGER, -- Opcional: mensaje relacionado con servicio
    asunto TEXT NOT NULL,
    contenido TEXT NOT NULL,
    leido BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (remitente_id) REFERENCES users(telegram_id),
    FOREIGN KEY (destinatario_id) REFERENCES users(telegram_id),
    FOREIGN KEY (empresa_id) REFERENCES empresas(id),
    FOREIGN KEY (servicio_id) REFERENCES servicios(id)
);

-- Historial de conversaciones IA
CREATE TABLE IF NOT EXISTS chat_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    tokens_used INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (telegram_id) REFERENCES users(telegram_id) ON DELETE CASCADE
);

-- Índices para búsquedas frecuentes
CREATE INDEX IF NOT EXISTS idx_empresas_telegram ON empresas(telegram_id);
CREATE INDEX IF NOT EXISTS idx_empresas_activa ON empresas(activa);
CREATE INDEX IF NOT EXISTS idx_servicios_empresa ON servicios(empresa_id);
CREATE INDEX IF NOT EXISTS idx_servicios_categoria ON servicios(categoria);
CREATE INDEX IF NOT EXISTS idx_servicios_disponible ON servicios(disponible);
CREATE INDEX IF NOT EXISTS idx_mensajes_destinatario ON mensajes(destinatario_id, leido);

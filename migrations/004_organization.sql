-- Tabla para almacenar datos de la organización (Colegio/Asociación)
CREATE TABLE IF NOT EXISTS organization (
    id INTEGER PRIMARY KEY CHECK (id = 1), -- Solo permite una fila
    name TEXT NOT NULL DEFAULT 'Colegio de Ingenieros',
    full_name TEXT,
    description TEXT,
    mission TEXT,
    vision TEXT,
    address TEXT,
    city TEXT,
    province TEXT,
    postal_code TEXT,
    phone TEXT,
    email TEXT,
    website TEXT,
    social_links TEXT, -- JSON con redes sociales
    registration_info TEXT, -- Información sobre cómo registrarse
    benefits TEXT, -- Beneficios de ser miembro
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Insertar registro por defecto si no existe
INSERT OR IGNORE INTO organization (id, name, description) 
VALUES (1, 'Colegio de Ingenieros', 'Organización profesional de ingenieros');

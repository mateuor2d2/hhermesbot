-- Añadir campo is_member a la tabla users
-- Este campo indica si el usuario es miembro de la organización

ALTER TABLE users ADD COLUMN is_member BOOLEAN NOT NULL DEFAULT FALSE;

-- Comentario: El admin puede cambiar este valor cuando verifique
-- que el usuario pertenece a la organización

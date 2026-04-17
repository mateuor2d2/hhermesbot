use chrono::{NaiveDate, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite, Row};
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use tokio::time::timeout;

/// Caché de usuarios activos con TTL de 5 minutos
#[derive(Debug)]
pub struct UserCache {
    users: DashMap<i64, (User, tokio::time::Instant)>,
    ttl: Duration,
}

impl UserCache {
    pub fn new() -> Self {
        Self {
            users: DashMap::new(),
            ttl: Duration::from_secs(300), // 5 minutos
        }
    }

    pub fn get(&self, telegram_id: i64) -> Option<User> {
        if let Some(entry) = self.users.get(&telegram_id) {
            let (user, timestamp) = entry.value();
            if timestamp.elapsed() < self.ttl {
                return Some(user.clone());
            }
            // TTL expirado, eliminar entrada
            drop(entry);
            self.users.remove(&telegram_id);
        }
        None
    }

    pub fn set(&self, telegram_id: i64, user: User) {
        self.users.insert(telegram_id, (user, tokio::time::Instant::now()));
    }

    #[allow(dead_code)]
    pub fn invalidate(&self, telegram_id: i64) {
        self.users.remove(&telegram_id);
    }
}

#[derive(Debug, Clone)]
pub struct Db {
    pub pool: Pool<Sqlite>,
    pub user_cache: Arc<UserCache>,
    pub query_timeout: Duration,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    pub telegram_id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_internal: bool,
    pub is_admin: bool,
    pub is_member: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Empresa {
    pub id: i64,
    pub telegram_id: i64,
    pub tipo: String,
    pub nombre_fiscal: String,
    pub nombre_comercial: Option<String>,
    pub cif_nif: Option<String>,
    pub direccion: Option<String>,
    pub codigo_postal: Option<String>,
    pub ciudad: Option<String>,
    pub provincia: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub web: Option<String>,
    pub descripcion: Option<String>,
    pub activa: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Servicio {
    pub id: i64,
    pub empresa_id: i64,
    pub tipo: String,
    pub categoria: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub precio: Option<String>,
    pub disponible: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Mensaje {
    pub id: i64,
    pub remitente_id: i64,
    pub destinatario_id: i64,
    pub empresa_id: Option<i64>,
    pub servicio_id: Option<i64>,
    pub asunto: String,
    pub contenido: String,
    pub leido: bool,
    pub created_at: chrono::NaiveDateTime,
}

impl Db {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Aumentar pool de conexiones a 20 y agregar timeout
        let pool = SqlitePoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&format!("sqlite:{}?mode=rwc", db_path))
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { 
            pool,
            user_cache: Arc::new(UserCache::new()),
            query_timeout: Duration::from_secs(5),
        })
    }

    // ===== USUARIOS =====

    pub async fn get_or_create_user(
        &self,
        telegram_id: i64,
        username: Option<&str>,
        first_name: Option<&str>,
        last_name: Option<&str>,
    ) -> anyhow::Result<User> {
        if let Ok(user) = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE telegram_id = ?"
        )
        .bind(telegram_id)
        .fetch_one(&self.pool)
        .await
        {
            sqlx::query(
                "UPDATE users SET username = ?, first_name = ?, last_name = ?, updated_at = CURRENT_TIMESTAMP WHERE telegram_id = ?"
            )
            .bind(username)
            .bind(first_name)
            .bind(last_name)
            .bind(telegram_id)
            .execute(&self.pool)
            .await?;

            return Ok(user);
        }

        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (telegram_id, username, first_name, last_name) VALUES (?, ?, ?, ?) RETURNING *"
        )
        .bind(telegram_id)
        .bind(username)
        .bind(first_name)
        .bind(last_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_user(&self, telegram_id: i64) -> anyhow::Result<Option<User>> {
        // Primero verificar en caché
        if let Some(cached_user) = self.user_cache.get(telegram_id) {
            return Ok(Some(cached_user));
        }

        // Si no está en caché, consultar DB con timeout
        let user = timeout(
            self.query_timeout,
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE telegram_id = ?")
                .bind(telegram_id)
                .fetch_optional(&self.pool)
        ).await??;
        
        // Guardar en caché si existe
        if let Some(ref u) = user {
            self.user_cache.set(telegram_id, u.clone());
        }
        
        Ok(user)
    }

    pub async fn set_user_type(&self, telegram_id: i64, user_type: &str) -> anyhow::Result<()> {
        let is_internal = user_type == "internal";
        sqlx::query("UPDATE users SET is_internal = ?, updated_at = CURRENT_TIMESTAMP WHERE telegram_id = ?")
            .bind(is_internal)
            .bind(telegram_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Cambiar estado de miembro de la organización
    pub async fn set_user_member_status(&self, telegram_id: i64, is_member: bool) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE users SET is_member = ?, updated_at = CURRENT_TIMESTAMP WHERE telegram_id = ?"
        )
        .bind(is_member)
        .bind(telegram_id)
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected() > 0)
    }
    
    /// Delete a user and all their associated data (empresa, servicios, centros, mensajes)
    pub async fn delete_user_completely(&self, telegram_id: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        
        // Get empresa_id if exists
        let empresa_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM empresas WHERE telegram_id = ?"
        )
        .bind(telegram_id)
        .fetch_optional(&mut *tx)
        .await?;
        
        // Delete servicios if empresa exists
        if let Some(eid) = empresa_id {
            sqlx::query("DELETE FROM servicios WHERE empresa_id = ?")
                .bind(eid)
                .execute(&mut *tx)
                .await?;
            
            // Delete centros
            sqlx::query("DELETE FROM centros WHERE empresa_id = ?")
                .bind(eid)
                .execute(&mut *tx)
                .await?;
        }
        
        // Delete empresa
        sqlx::query("DELETE FROM empresas WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        // Delete mensajes sent and received
        sqlx::query("DELETE FROM mensajes WHERE from_telegram_id = ? OR to_telegram_id = ?")
            .bind(telegram_id)
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        // Delete broadcast usage
        sqlx::query("DELETE FROM broadcast_usage WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        // Delete broadcasts
        sqlx::query("DELETE FROM broadcasts WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        // Delete payments
        sqlx::query("DELETE FROM payments WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        // Finally delete user
        sqlx::query("DELETE FROM users WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(())
    }

    /// Obtener todos los usuarios (para admin)
    pub async fn get_all_users(&self) -> anyhow::Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            "SELECT * FROM users ORDER BY created_at DESC LIMIT 100"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(users)
    }

    /// Buscar usuarios por nombre o username (para admin)
    pub async fn search_users(&self, query: &str) -> anyhow::Result<Vec<User>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let users = sqlx::query_as::<_, User>(
            "SELECT * FROM users 
             WHERE LOWER(first_name) LIKE ? OR LOWER(last_name) LIKE ? OR LOWER(username) LIKE ? OR CAST(telegram_id AS TEXT) LIKE ?
             ORDER BY created_at DESC LIMIT 50"
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(users)
    }

    // ===== USO DE IA =====

    pub async fn get_ia_usage(&self, telegram_id: i64, date: NaiveDate) -> anyhow::Result<i32> {
        let count: Option<i32> = sqlx::query_scalar(
            "SELECT messages_today FROM ia_usage WHERE telegram_id = ? AND last_reset_date = ?"
        )
        .bind(telegram_id)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    pub async fn increment_ia_usage(&self, telegram_id: i64, date: NaiveDate) -> anyhow::Result<i32> {
        sqlx::query(
            "INSERT INTO ia_usage (telegram_id, last_reset_date, messages_today, total_messages) VALUES (?, ?, 1, 1)
             ON CONFLICT(telegram_id) DO UPDATE SET 
             messages_today = CASE WHEN last_reset_date = ? THEN messages_today + 1 ELSE 1 END,
             total_messages = total_messages + 1,
             last_reset_date = ?"
        )
        .bind(telegram_id)
        .bind(date)
        .bind(date)
        .bind(date)
        .execute(&self.pool)
        .await?;

        let count: i32 = sqlx::query_scalar(
            "SELECT messages_today FROM ia_usage WHERE telegram_id = ?"
        )
        .bind(telegram_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    // ===== EMPRESAS =====

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub async fn create_business(
        &self,
        telegram_id: i64,
        business_type: &str,
        name: &str,
        description: Option<&str>,
        cif: Option<&str>,
        phone: Option<&str>,
        email: Option<&str>,
    ) -> anyhow::Result<i64> {
        let tipo = if business_type == "company" { "sociedad" } else { "autonomo" };
        let result = sqlx::query(
            "INSERT INTO empresas (telegram_id, tipo, nombre_fiscal, descripcion, cif_nif, telefono, email, activa) 
             VALUES (?, ?, ?, ?, ?, ?, ?, FALSE)"
        )
        .bind(telegram_id)
        .bind(tipo)
        .bind(name)
        .bind(description)
        .bind(cif)
        .bind(phone)
        .bind(email)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Crear empresa con centros y servicios (todo pendiente de aprobación)
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub async fn create_business_complete(
        &self,
        telegram_id: i64,
        business_type: &str,
        name: &str,
        description: Option<&str>,
        cif: Option<&str>,
        phone: Option<&str>,
        email: Option<&str>,
        centros: Vec<crate::dialogue::states::CentroPendiente>,
        servicios: Vec<crate::dialogue::states::ServicioPendiente>,
    ) -> anyhow::Result<i64> {
        let mut tx = self.pool.begin().await?;
        
        let tipo = if business_type == "company" { "sociedad" } else { "autonomo" };
        
        // Insertar empresa
        let result = sqlx::query(
            "INSERT INTO empresas (telegram_id, tipo, nombre_fiscal, descripcion, cif_nif, telefono, email, activa) 
             VALUES (?, ?, ?, ?, ?, ?, ?, FALSE)"
        )
        .bind(telegram_id)
        .bind(tipo)
        .bind(name)
        .bind(description)
        .bind(cif)
        .bind(phone)
        .bind(email)
        .execute(&mut *tx)
        .await?;
        
        let empresa_id = result.last_insert_rowid();
        
        // Insertar centros (inactivos hasta aprobación)
        for centro in centros {
            sqlx::query(
                "INSERT INTO centros (empresa_id, nombre, direccion, ciudad, telefono, email, activo) 
                 VALUES (?, ?, ?, ?, ?, ?, FALSE)"
            )
            .bind(empresa_id)
            .bind(&centro.nombre)
            .bind(&centro.direccion)
            .bind(&centro.ciudad)
            .bind(&centro.telefono)
            .bind(&centro.email)
            .execute(&mut *tx)
            .await?;
        }
        
        // Insertar servicios (no disponibles hasta aprobación)
        for servicio in servicios {
            sqlx::query(
                "INSERT INTO servicios (empresa_id, tipo, categoria, nombre, descripcion, precio, disponible) 
                 VALUES (?, ?, ?, ?, ?, ?, FALSE)"
            )
            .bind(empresa_id)
            .bind(&servicio.tipo)
            .bind(&servicio.categoria)
            .bind(&servicio.nombre)
            .bind(&servicio.descripcion)
            .bind(&servicio.precio)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(empresa_id)
    }
    
    /// Actualizar o crear empresa (elimina la anterior si existe)
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_business_complete(
        &self,
        telegram_id: i64,
        business_type: &str,
        name: &str,
        description: Option<&str>,
        cif: Option<&str>,
        phone: Option<&str>,
        email: Option<&str>,
        centros: Vec<crate::dialogue::states::CentroPendiente>,
        servicios: Vec<crate::dialogue::states::ServicioPendiente>,
    ) -> anyhow::Result<i64> {
        let mut tx = self.pool.begin().await?;
        
        // Get existing empresa_id if any
        let existing_empresa_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM empresas WHERE telegram_id = ?"
        )
        .bind(telegram_id)
        .fetch_optional(&mut *tx)
        .await?;
        
        // Delete old empresa and related data if exists
        if let Some(old_id) = existing_empresa_id {
            sqlx::query("DELETE FROM servicios WHERE empresa_id = ?")
                .bind(old_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM centros WHERE empresa_id = ?")
                .bind(old_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM empresas WHERE id = ?")
                .bind(old_id)
                .execute(&mut *tx)
                .await?;
        }
        
        let tipo = if business_type == "company" { "sociedad" } else { "autonomo" };
        
        // Insertar nueva empresa
        let result = sqlx::query(
            "INSERT INTO empresas (telegram_id, tipo, nombre_fiscal, descripcion, cif_nif, telefono, email, activa) 
             VALUES (?, ?, ?, ?, ?, ?, ?, FALSE)"
        )
        .bind(telegram_id)
        .bind(tipo)
        .bind(name)
        .bind(description)
        .bind(cif)
        .bind(phone)
        .bind(email)
        .execute(&mut *tx)
        .await?;
        
        let empresa_id = result.last_insert_rowid();
        
        // Insertar centros
        for centro in centros {
            sqlx::query(
                "INSERT INTO centros (empresa_id, nombre, direccion, ciudad, telefono, email, activo) 
                 VALUES (?, ?, ?, ?, ?, ?, FALSE)"
            )
            .bind(empresa_id)
            .bind(&centro.nombre)
            .bind(&centro.direccion)
            .bind(&centro.ciudad)
            .bind(&centro.telefono)
            .bind(&centro.email)
            .execute(&mut *tx)
            .await?;
        }
        
        // Insertar servicios
        for servicio in servicios {
            sqlx::query(
                "INSERT INTO servicios (empresa_id, tipo, categoria, nombre, descripcion, precio, disponible) 
                 VALUES (?, ?, ?, ?, ?, ?, FALSE)"
            )
            .bind(empresa_id)
            .bind(&servicio.tipo)
            .bind(&servicio.categoria)
            .bind(&servicio.nombre)
            .bind(&servicio.descripcion)
            .bind(&servicio.precio)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(empresa_id)
    }

    /// Aprobar empresa (activarla, centros, servicios y convertir usuario a interno y miembro)
    pub async fn approve_business(&self, empresa_id: i64) -> anyhow::Result<bool> {
        // Iniciar transacción
        let mut tx = self.pool.begin().await?;
        
        // Obtener telegram_id de la empresa
        let row = sqlx::query("SELECT telegram_id FROM empresas WHERE id = ?")
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await?;
        
        let telegram_id: i64 = match row {
            Some(r) => r.get("telegram_id"),
            None => return Ok(false),
        };
        
        // Activar empresa
        sqlx::query("UPDATE empresas SET activa = TRUE, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
        
        // Activar todos los centros de esta empresa
        sqlx::query("UPDATE centros SET activo = TRUE WHERE empresa_id = ?")
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
        
        // Activar todos los servicios de esta empresa
        sqlx::query("UPDATE servicios SET disponible = TRUE WHERE empresa_id = ?")
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
        
        // Convertir usuario a interno y miembro
        sqlx::query("UPDATE users SET is_internal = TRUE, is_member = TRUE, updated_at = CURRENT_TIMESTAMP WHERE telegram_id = ?")
            .bind(telegram_id)
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(true)
    }
    
    /// Obtener centros de una empresa
    pub async fn get_centros_by_empresa(&self, empresa_id: i64) -> anyhow::Result<Vec<(i64, String)>> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, nombre FROM centros WHERE empresa_id = ?"
        )
        .bind(empresa_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
    
    /// Obtener servicios de una empresa
    pub async fn get_servicios_by_empresa(&self, empresa_id: i64) -> anyhow::Result<Vec<(i64, String, String)>> {
        let rows: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT id, nombre, categoria FROM servicios WHERE empresa_id = ?"
        )
        .bind(empresa_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Rechazar/eliminar empresa pendiente
    pub async fn reject_business(&self, empresa_id: i64) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM empresas WHERE id = ? AND activa = FALSE")
            .bind(empresa_id)
            .execute(&self.pool)
            .await?;
        
        Ok(result.rows_affected() > 0)
    }

    /// Listar empresas pendientes de aprobación
    pub async fn get_pending_businesses(&self) -> anyhow::Result<Vec<Empresa>> {
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas WHERE activa = FALSE ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    #[allow(dead_code)]
    pub async fn get_empresas_by_user(&self, telegram_id: i64) -> anyhow::Result<Vec<Empresa>> {
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas WHERE telegram_id = ? ORDER BY created_at DESC"
        )
        .bind(telegram_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    pub async fn get_empresa_by_id(&self, empresa_id: i64) -> anyhow::Result<Option<Empresa>> {
        let empresa = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas WHERE id = ?"
        )
        .bind(empresa_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(empresa)
    }

    /// Buscar empresas por nombre fiscal, comercial, descripción o CIF/NIF
    pub async fn search_businesses(&self, query: &str) -> anyhow::Result<Vec<Empresa>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas 
             WHERE (LOWER(nombre_fiscal) LIKE ? OR LOWER(nombre_comercial) LIKE ? OR LOWER(descripcion) LIKE ? OR LOWER(cif_nif) LIKE ?)
             AND activa = TRUE
             ORDER BY nombre_fiscal LIMIT 20"
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    pub async fn search_businesses_by_name(&self, query: &str) -> anyhow::Result<Vec<Empresa>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas
             WHERE (LOWER(nombre_fiscal) LIKE ? OR LOWER(nombre_comercial) LIKE ?)
             AND activa = TRUE
             ORDER BY nombre_fiscal
             LIMIT 20"
        )
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    pub async fn search_businesses_by_service(&self, query: &str) -> anyhow::Result<Vec<Empresa>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT DISTINCT e.* FROM empresas e
             INNER JOIN servicios s ON e.id = s.empresa_id
             WHERE LOWER(s.nombre) LIKE ?
             AND e.activa = TRUE
             AND s.disponible = TRUE
             ORDER BY e.nombre_fiscal
             LIMIT 20"
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    pub async fn search_businesses_by_address(&self, query: &str) -> anyhow::Result<Vec<Empresa>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas
             WHERE LOWER(direccion) LIKE ?
             AND activa = TRUE
             ORDER BY nombre_fiscal
             LIMIT 20"
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    pub async fn search_businesses_by_city(&self, query: &str) -> anyhow::Result<Vec<Empresa>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let empresas = sqlx::query_as::<_, Empresa>(
            "SELECT * FROM empresas
             WHERE LOWER(ciudad) LIKE ?
             AND activa = TRUE
             ORDER BY nombre_fiscal
             LIMIT 20"
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(empresas)
    }

    // ===== SERVICIOS =====

    #[allow(dead_code)]
    pub async fn create_service(
        &self,
        empresa_id: i64,
        category: &str,
        name: &str,
        description: Option<&str>,
        price: Option<&str>,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO servicios (empresa_id, tipo, categoria, nombre, descripcion, precio) 
             VALUES (?, 'servicio', ?, ?, ?, ?)"
        )
        .bind(empresa_id)
        .bind(category)
        .bind(name)
        .bind(description)
        .bind(price)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn search_services(&self, query: &str, category: Option<&str>) -> anyhow::Result<Vec<(Servicio, Empresa)>> {
        let pattern = format!("%{}%", query.to_lowercase());

        let rows = if let Some(cat) = category {
            sqlx::query(
                "SELECT s.id as s_id, s.empresa_id, s.tipo, s.categoria, s.nombre, s.descripcion, s.precio, s.disponible, s.created_at as s_created_at, s.updated_at as s_updated_at,
                        e.id as e_id, e.telegram_id, e.tipo as e_tipo, e.nombre_fiscal, e.nombre_comercial, e.cif_nif, e.direccion, e.codigo_postal, e.ciudad, e.provincia, e.telefono, e.email, e.web, e.descripcion as e_descripcion, e.activa, e.created_at as e_created_at, e.updated_at as e_updated_at
                 FROM servicios s
                 JOIN empresas e ON s.empresa_id = e.id
                 WHERE s.disponible = true AND (LOWER(s.nombre) LIKE ? OR LOWER(s.descripcion) LIKE ?) AND LOWER(s.categoria) = LOWER(?)
                 ORDER BY s.nombre LIMIT 20"
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(cat)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT s.id as s_id, s.empresa_id, s.tipo, s.categoria, s.nombre, s.descripcion, s.precio, s.disponible, s.created_at as s_created_at, s.updated_at as s_updated_at,
                        e.id as e_id, e.telegram_id, e.tipo as e_tipo, e.nombre_fiscal, e.nombre_comercial, e.cif_nif, e.direccion, e.codigo_postal, e.ciudad, e.provincia, e.telefono, e.email, e.web, e.descripcion as e_descripcion, e.activa, e.created_at as e_created_at, e.updated_at as e_updated_at
                 FROM servicios s
                 JOIN empresas e ON s.empresa_id = e.id
                 WHERE s.disponible = true AND (LOWER(s.nombre) LIKE ? OR LOWER(s.descripcion) LIKE ?)
                 ORDER BY s.nombre LIMIT 20"
            )
            .bind(&pattern)
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?
        };

        let mut results = Vec::new();
        for row in rows {
            let servicio = Servicio {
                id: row.get("s_id"),
                empresa_id: row.get("empresa_id"),
                tipo: row.get("tipo"),
                categoria: row.get("categoria"),
                nombre: row.get("nombre"),
                descripcion: row.get("descripcion"),
                precio: row.get("precio"),
                disponible: row.get("disponible"),
                created_at: row.get("s_created_at"),
                updated_at: row.get("s_updated_at"),
            };
            let empresa = Empresa {
                id: row.get("e_id"),
                telegram_id: row.get("telegram_id"),
                tipo: row.get("e_tipo"),
                nombre_fiscal: row.get("nombre_fiscal"),
                nombre_comercial: row.get("nombre_comercial"),
                cif_nif: row.get("cif_nif"),
                direccion: row.get("direccion"),
                codigo_postal: row.get("codigo_postal"),
                ciudad: row.get("ciudad"),
                provincia: row.get("provincia"),
                telefono: row.get("telefono"),
                email: row.get("email"),
                web: row.get("web"),
                descripcion: row.get("e_descripcion"),
                activa: row.get("activa"),
                created_at: row.get("e_created_at"),
                updated_at: row.get("e_updated_at"),
            };
            results.push((servicio, empresa));
        }

        Ok(results)
    }

    // ===== MENSAJES =====

    #[allow(dead_code)]
    pub async fn send_message(
        &self,
        sender_telegram_id: i64,
        recipient_telegram_id: i64,
        subject: &str,
        content: &str,
    ) -> anyhow::Result<bool> {
        let recipient = self.get_user(recipient_telegram_id).await?;
        if recipient.is_none() {
            return Ok(false);
        }

        sqlx::query(
            "INSERT INTO mensajes (remitente_id, destinatario_id, asunto, contenido) VALUES (?, ?, ?, ?)"
        )
        .bind(sender_telegram_id)
        .bind(recipient_telegram_id)
        .bind(subject)
        .bind(content)
        .execute(&self.pool)
        .await?;

        Ok(true)
    }

    pub async fn get_unread_messages(&self, telegram_id: i64) -> anyhow::Result<Vec<Mensaje>> {
        let messages = sqlx::query_as::<_, Mensaje>(
            "SELECT * FROM mensajes
             WHERE destinatario_id = ? AND leido = false
             ORDER BY created_at DESC"
        )
        .bind(telegram_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(messages)
    }

    // ===== BROADCAST / DIFUSIONES =====

    /// Get broadcast usage for a user in a specific quarter (returns: used_count, paid_extra)
    pub async fn get_broadcast_usage(
        &self,
        telegram_id: i64,
        year: i32,
        quarter: i32,
    ) -> anyhow::Result<(i32, i32)> {
        let result: Option<(i32, i32)> = sqlx::query_as(
            "SELECT count, paid_extra FROM broadcast_usage 
             WHERE telegram_id = ? AND year = ? AND quarter = ?"
        )
        .bind(telegram_id)
        .bind(year)
        .bind(quarter)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.unwrap_or((0, 0)))
    }

    /// Increment free broadcast usage for a user
    pub async fn increment_broadcast_usage(
        &self,
        telegram_id: i64,
        year: i32,
        quarter: i32,
    ) -> anyhow::Result<(i32, i32)> {
        sqlx::query(
            "INSERT INTO broadcast_usage (telegram_id, quarter, year, count, paid_extra) 
             VALUES (?, ?, ?, 1, 0)
             ON CONFLICT(telegram_id, quarter, year) DO UPDATE SET 
             count = count + 1,
             updated_at = CURRENT_TIMESTAMP"
        )
        .bind(telegram_id)
        .bind(quarter)
        .bind(year)
        .execute(&self.pool)
        .await?;

        self.get_broadcast_usage(telegram_id, year, quarter).await
    }

    /// Use a paid broadcast credit
    pub async fn use_paid_broadcast(
        &self,
        telegram_id: i64,
        year: i32,
        quarter: i32,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE broadcast_usage 
             SET paid_extra = paid_extra - 1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE telegram_id = ? AND year = ? AND quarter = ? AND paid_extra > 0"
        )
        .bind(telegram_id)
        .bind(year)
        .bind(quarter)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if user can broadcast (has free or paid credits)
    pub async fn can_broadcast(
        &self,
        telegram_id: i64,
        year: i32,
        quarter: i32,
        free_limit: i32,
    ) -> anyhow::Result<(bool, bool)> {
        let (used_count, paid_extra) = self.get_broadcast_usage(telegram_id, year, quarter).await?;
        let has_free = used_count < free_limit;
        let has_paid = paid_extra > 0;
        Ok((has_free, has_paid))
    }

    // ===== SUBSCRIPTION CHECKS =====

    /// Cache subscription check result
    #[allow(dead_code)]
    pub async fn cache_subscription_check(
        &self,
        telegram_id: i64,
        channel_id: i64,
        is_member: bool,
        expires_hours: i32,
    ) -> anyhow::Result<()> {
        let expires_at = Utc::now() + chrono::Duration::hours(expires_hours as i64);
        
        sqlx::query(
            "INSERT INTO subscription_checks (telegram_id, channel_id, is_member, expires_at) 
             VALUES (?, ?, ?, ?)
             ON CONFLICT(telegram_id, channel_id) DO UPDATE SET 
             is_member = ?, checked_at = CURRENT_TIMESTAMP, expires_at = ?"
        )
        .bind(telegram_id)
        .bind(channel_id)
        .bind(is_member)
        .bind(expires_at.naive_utc())
        .bind(is_member)
        .bind(expires_at.naive_utc())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get cached subscription check (if not expired)
    #[allow(dead_code)]
    pub async fn get_cached_subscription(
        &self,
        telegram_id: i64,
        channel_id: i64,
    ) -> anyhow::Result<Option<bool>> {
        let result: Option<(bool,)> = sqlx::query_as(
            "SELECT is_member FROM subscription_checks 
             WHERE telegram_id = ? AND channel_id = ? AND expires_at > CURRENT_TIMESTAMP"
        )
        .bind(telegram_id)
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0))
    }

    // ===== PAYMENTS =====

    /// Create a pending payment for extra broadcasts
    #[allow(dead_code)]
    pub async fn create_payment(
        &self,
        telegram_id: i64,
        year: i32,
        quarter: i32,
        amount: f64,
        broadcasts_added: i32,
        payment_method: &str,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO broadcast_payments 
             (telegram_id, year, quarter, amount, broadcasts_added, payment_method, status) 
             VALUES (?, ?, ?, ?, ?, ?, 'pending')"
        )
        .bind(telegram_id)
        .bind(year)
        .bind(quarter)
        .bind(amount)
        .bind(broadcasts_added)
        .bind(payment_method)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Complete a payment and grant broadcast credits
    #[allow(dead_code)]
    pub async fn complete_payment(
        &self,
        payment_id: i64,
        payment_reference: &str,
        verified_by: i64,
    ) -> anyhow::Result<bool> {
        let payment: Option<BroadcastPayment> = sqlx::query_as(
            "SELECT * FROM broadcast_payments WHERE id = ? AND status = 'pending'"
        )
        .bind(payment_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(payment) = payment {
            // Update payment status
            sqlx::query(
                "UPDATE broadcast_payments 
                 SET status = 'completed', payment_reference = ?, paid_at = CURRENT_TIMESTAMP, verified_by = ?
                 WHERE id = ?"
            )
            .bind(payment_reference)
            .bind(verified_by)
            .bind(payment_id)
            .execute(&self.pool)
            .await?;

            // Add paid credits to broadcast_usage
            sqlx::query(
                "INSERT INTO broadcast_usage (telegram_id, quarter, year, count, paid_extra) 
                 VALUES (?, ?, ?, 0, ?)
                 ON CONFLICT(telegram_id, quarter, year) DO UPDATE SET 
                 paid_extra = paid_extra + ?"
            )
            .bind(payment.telegram_id)
            .bind(payment.quarter)
            .bind(payment.year)
            .bind(payment.broadcasts_added)
            .bind(payment.broadcasts_added)
            .execute(&self.pool)
            .await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get pending payments for a user
    #[allow(dead_code)]
    pub async fn get_pending_payments(&self, telegram_id: i64) -> anyhow::Result<Vec<BroadcastPayment>> {
        let payments = sqlx::query_as::<_, BroadcastPayment>(
            "SELECT * FROM broadcast_payments 
             WHERE telegram_id = ? AND status = 'pending' 
             ORDER BY created_at DESC"
        )
        .bind(telegram_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(payments)
    }
}

pub type SharedDb = Arc<Db>;

// ===== DATA STRUCTURES =====

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct BroadcastPayment {
    pub id: i64,
    pub telegram_id: i64,
    pub quarter: i32,
    pub year: i32,
    pub amount: f64,
    pub broadcasts_added: i32,
    pub payment_method: String,
    pub payment_reference: Option<String>,
    pub status: String,
    pub paid_at: Option<chrono::NaiveDateTime>,
    pub verified_by: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
}

// ===== ORGANIZACIÓN =====

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Organization {
    #[allow(dead_code)]
    pub id: i64,
    pub name: String,
    pub full_name: Option<String>,
    pub description: Option<String>,
    pub mission: Option<String>,
    pub vision: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    #[allow(dead_code)]
    pub social_links: Option<String>,
    pub registration_info: Option<String>,
    pub benefits: Option<String>,
    #[allow(dead_code)]
    pub created_at: chrono::NaiveDateTime,
    #[allow(dead_code)]
    pub updated_at: chrono::NaiveDateTime,
}

impl Db {
    /// Obtener datos de la organización
    pub async fn get_organization(&self) -> anyhow::Result<Organization> {
        let org = sqlx::query_as::<_, Organization>(
            "SELECT * FROM organization WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;
        
        match org {
            Some(o) => Ok(o),
            None => {
                // Crear registro por defecto si no existe
                sqlx::query(
                    "INSERT INTO organization (id, name, description) VALUES (1, 'Colegio de Ingenieros', 'Organización profesional')"
                )
                .execute(&self.pool)
                .await?;
                
                Ok(Organization {
                    id: 1,
                    name: "Colegio de Ingenieros".to_string(),
                    full_name: None,
                    description: Some("Organización profesional".to_string()),
                    mission: None,
                    vision: None,
                    address: None,
                    city: None,
                    province: None,
                    postal_code: None,
                    phone: None,
                    email: None,
                    website: None,
                    registration_info: None,
                    benefits: None,
                    social_links: None,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                })
            }
        }
    }

    /// Actualizar datos de la organización (solo admin)
    #[allow(clippy::too_many_arguments)]
    pub async fn update_organization(
        &self,
        name: Option<&str>,
        full_name: Option<&str>,
        description: Option<&str>,
        mission: Option<&str>,
        vision: Option<&str>,
        address: Option<&str>,
        city: Option<&str>,
        province: Option<&str>,
        postal_code: Option<&str>,
        phone: Option<&str>,
        email: Option<&str>,
        website: Option<&str>,
        registration_info: Option<&str>,
        benefits: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE organization SET
             name = COALESCE(?, name),
             full_name = COALESCE(?, full_name),
             description = COALESCE(?, description),
             mission = COALESCE(?, mission),
             vision = COALESCE(?, vision),
             address = COALESCE(?, address),
             city = COALESCE(?, city),
             province = COALESCE(?, province),
             postal_code = COALESCE(?, postal_code),
             phone = COALESCE(?, phone),
             email = COALESCE(?, email),
             website = COALESCE(?, website),
             registration_info = COALESCE(?, registration_info),
             benefits = COALESCE(?, benefits),
             updated_at = CURRENT_TIMESTAMP
             WHERE id = 1"
        )
        .bind(name)
        .bind(full_name)
        .bind(description)
        .bind(mission)
        .bind(vision)
        .bind(address)
        .bind(city)
        .bind(province)
        .bind(postal_code)
        .bind(phone)
        .bind(email)
        .bind(website)
        .bind(registration_info)
        .bind(benefits)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

# Colegio-Bot — QA Report Pre-Producción

**Target:** colegio-bot (Rust + Teloxide + SQLite/Postgres)
**Date:** 2026-04-22
**Scope:** Revisión exhaustiva de código, configuración de despliegue, flujos de pago (Stripe test), roles de usuario y robustez.
**Tester:** Hermes Agent (análisis estático + compilación + tests)
**Estado:** ❌ **NO APTO PARA PRODUCCIÓN** — Se detectaron bugs críticos que bloquean el despliegue.

---

## Executive Summary

| Severity | Count |
|----------|-------|
| 🔴 Critical | 4 |
| 🟠 High | 6 |
| 🟡 Medium | 7 |
| 🔵 Low | 4 |
| **Total** | **21** |

**Bloqueantes para producción:**
1. El webhook de Stripe intenta escribir en tabla `payments` que **no existe** en las migraciones (solo existe `broadcast_payments`). Los pagos se recibirán pero fallarán al guardarse.
2. Inconsistencia de puertos entre `config.toml` (3001), `Dockerfile` (3000) y `docker-compose.cima20paas.yml` (3000). El healthcheck de Docker y Traefik fallarán.
3. El wizard de registro y los diálogos usan almacenamiento **en memoria** (`InMemStorage`, HashMap global). Si el contenedor se reinicia (deploy, crash, auto-restart), los usuarios pierden su progreso de registro.
4. No hay idempotencia en el webhook de Stripe: si Stripe reenvía el evento `checkout.session.completed`, se duplican créditos/membresía.

---

## Issues

### Issue #1: Tabla `payments` no existe — Webhook de Stripe falla al registrar pagos

| Field | Value |
|-------|-------|
| **Severity** | 🔴 Critical |
| **Category** | Functional / Database |
| **Archivos** | `src/payments/webhook.rs`, `migrations/003_broadcast_usage.sql`, `src/handlers/pagos.rs` |

**Descripción:**
El webhook de Stripe (`webhook.rs`, líneas 151 y 210) ejecuta:
```sql
INSERT INTO payments (telegram_id, stripe_session_id, amount, credits, pack_name, status, created_at)
```
Pero las migraciones **solo crean `broadcast_payments`**, con un esquema completamente diferente:
```sql
CREATE TABLE IF NOT EXISTS broadcast_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id INTEGER NOT NULL,
    year INTEGER NOT NULL,
    quarter INTEGER NOT NULL,
    amount REAL NOT NULL,
    broadcasts_added INTEGER NOT NULL,
    payment_method TEXT,
    payment_reference TEXT,
    status TEXT DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    paid_at DATETIME,
    verified_by INTEGER
);
```

Además, el handler `/mis_pagos` (`src/handlers/pagos.rs`) lee de `broadcast_payments`, pero espera columnas como `broadcasts_added` y `created_at` como string, mientras que el webhook intenta insertar en tabla inexistente.

**Impacto:**
- Cuando un usuario pague por Stripe, el webhook recibirá HTTP 200 (la firma es válida), pero fallará silenciosamente al insertar en `payments`.
- Los créditos **sí se añaden** en `broadcast_usage` (línea 190 del webhook), pero no queda registro de pago.
- El usuario no puede ver su historial de pagos (`/mis_pagos`) porque `broadcast_payments` nunca se llena desde el webhook.

**Fix recomendado:**
1. Crear migración `007_payments_table.sql` con tabla `payments` que coincida con el webhook, **O**
2. Cambiar el webhook para insertar en `broadcast_payments` mapeando los campos correctamente.

**Se recomienda la opción 2** para mantener un solo historial de pagos.

---

### Issue #2: Inconsistencia de puertos web server (3000 vs 3001)

| Field | Value |
|-------|-------|
| **Severity** | 🔴 Critical |
| **Category** | DevOps / Configuration |
| **Archivos** | `config.toml`, `Dockerfile`, `docker-compose.cima20paas.yml` |

**Descripción:**
- `config.toml`: `web_server_port = 3001`
- `Dockerfile`: `EXPOSE 3000` + healthcheck a `localhost:3000/health`
- `docker-compose.cima20paas.yml`: `traefik.http.services.colegio-bot.loadbalancer.server.port=3000`

En producción con Docker, el binario escuchará en **3001**, pero Traefik y el healthcheck de Docker apuntan a **3000**.

**Impacto:**
- Traefik no puede enrutar al bot → Stripe webhooks fallan (`502 Bad Gateway`).
- Docker healthcheck falla continuamente → contenedor marcado como unhealthy → posibles reinicios infinitos.

**Fix recomendado:**
Unificar todo a **3000** (estándar del Dockerfile y compose):
```toml
# config.toml
web_server_port = 3000
```

---

### Issue #3: Estado en memoria — pérdida de datos al reiniciar

| Field | Value |
|-------|-------|
| **Severity** | 🔴 Critical |
| **Category** | Functional / Data Loss |
| **Archivos** | `src/main.rs`, `src/wizard.rs`, `src/dialogue/` |

**Descripción:**
- Los diálogos usan `InMemStorage::<BotDialogueState>::new()`.
- El wizard de registro usa un HashMap global en memoria (`crate::wizard::get_wizard_state`).

**Impacto:**
- Si el bot se reinicia (despliegue, crash, OOM kill), cualquier usuario en medio de:
  - Registro de empresa (`/registrar` wizard)
  - Creación de difusión (`/difundir` título/contenido)
  - Búsqueda interactiva (`/buscar` sin query)
  pierde su progreso completamente.
- Esto es especialmente grave en producción donde los deploys son frecuentes.

**Fix recomendado:**
Migrar a almacenamiento persistente. Opciones:
1. Implementar `SqliteStorage` para diálogos (guardar estado en SQLite).
2. Almacenar el estado del wizard en la base de datos (tabla `wizard_state`).

Para MVP rápido: al menos documentar esta limitación y evitar reinicios durante horas de uso.

---

### Issue #4: Webhook de Stripe no es idempotente — duplicación de créditos

| Field | Value |
|-------|-------|
| **Severity** | 🔴 Critical |
| **Category** | Functional / Payments |
| **Archivos** | `src/payments/webhook.rs` |

**Descripción:**
El webhook procesa `checkout.session.completed` sin verificar si ya fue procesado:
```rust
if event_type == "checkout.session.completed" {
    let session = &event["data"]["object"];
    process_checkout_session(session, &config.db_pool).await?;
}
```

Stripe puede reenviar el mismo evento si:
- Hay timeout en la respuesta HTTP 200.
- Se reintenta manualmente desde el dashboard.
- Hay problemas de red.

**Impacto:**
- El mismo pago puede añadir créditos múltiples veces.
- El mismo pago puede activar membresía múltiples veces (aunque `UPDATE` es idempotente, el insert en `payments` fallaría por tabla inexistente).

**Fix recomendado:**
Verificar `stripe_session_id` antes de procesar:
```sql
SELECT 1 FROM broadcast_payments WHERE payment_reference = ?
```
Si ya existe, retornar `Ok(())` sin procesar.

---

### Issue #5: `relay_target_chat_id = 0` puede causar errores silenciosos

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | Configuration / Functional |
| **Archivos** | `config.toml` |

**Descripción:**
```toml
relay_target_chat_id = 0
```
Si el sistema de relay está habilitado (`enable_relay = true`) pero `relay_target_chat_id` es 0, cualquier intento de reenvío enviará mensajes a ChatId(0), lo cual es inválido en Telegram.

**Impacto:**
- Posibles errores de API no manejados.
- Logs llenos de warnings.

**Fix recomendado:**
Validar que `relay_target_chat_id != 0` antes de intentar reenviar, o deshabilitar relay hasta que se configure.

---

### Issue #6: Broadcast envía a TODOS los usuarios sin segmentación

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | UX / Privacy |
| **Archivos** | `src/handlers/broadcast.rs` |

**Descripción:**
```rust
let users = db.get_all_users().await?;
for user in &users {
    bot.send_message(ChatId(user.telegram_id), &broadcast_message).await?;
}
```

**Impacto:**
- Usuarios "externos" (particulares que solo buscan servicios) reciben difusiones comerciales.
- Si la base de datos crece, este bucle bloquea el bot durante segundos/minutos.
- No hay rate limiting: Telegram puede rate-limitar al bot si hay muchos usuarios.

**Fix recomendado:**
1. Filtrar solo usuarios `is_internal = TRUE` o `is_member = TRUE`.
2. Añadir paginación/batch (ej. 30 usuarios/segundo).
3. Usar `tokio::spawn` para envío asíncrono no bloqueante.

---

### Issue #7: Falta validación de inputs en wizard de registro

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | Functional / Data Quality |
| **Archivos** | `src/main.rs` (wizard handlers), `src/wizard.rs` |

**Descripción:**
- Email: no se valida formato RFC 5322.
- Teléfono: no se valida formato.
- CIF/NIF: se acepta cualquier texto.
- Web: no se valida que sea URL válida.
- Código postal: no se valida.

**Impacto:**
- Datos basura en la base de datos.
- Posibles errores de parseo al mostrar información.
- Usuarios pueden insertar payloads maliciosos (aunque hay `escape_html`).

**Fix recomendado:**
Añadir validaciones con `regex` (ya es dependencia del proyecto):
- Email: regex básico.
- Teléfono: dígitos y prefijo opcional.
- URL: `reqwest::Url::parse()` o regex.

---

### Issue #8: No hay rate limiting por usuario

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | Security |
| **Archivos** | `src/handlers.rs`, `src/main.rs` |

**Descripción:**
Ningún comando tiene rate limiting. Un usuario podría:
- Ejecutar `/chat` 1000 veces/segundo (aunque hay límite diario, no hay límite de frecuencia).
- Ejecutar `/buscar` continuamente.
- Spammear `/start` o callbacks.

**Impacto:**
- Abuso de recursos (llamadas a IA, queries a DB).
- Posible bloqueo temporal por parte de Telegram API.
- Denegación de servicio accidental o intencionada.

**Fix recomendado:**
Implementar un `DashMap<user_id, Vec<Instant>>` para limitar comandos por ventana de tiempo (ej. max 10 comandos/minuto).

---

### Issue #9: `channel_id` es placeholder en config.toml

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | Configuration |
| **Archivos** | `config.toml` |

**Descripción:**
```toml
channel_id = "-1001234567890"
```
Este es un ID de ejemplo. En producción, si no se cambia, las funciones de:
- Verificación de suscripción (`check_channel_subscription`)
- Envío de difusiones al canal
fallarán.

**Impacto:**
- Los usuarios no pueden difundir porque la verificación de membresía falla.
- Las difusiones no llegan al canal público.

**Fix recomendado:**
Documentar claramente en `DEPLOYMENT.md` que este valor DEBE cambiarse. Añadir validación al startup que aborte si es el placeholder.

---

### Issue #10: No se maneja el caso de usuario bloqueado por Telegram

| Field | Value |
|-------|-------|
| **Severity** | 🟠 High |
| **Category** | Functional |
| **Archivos** | `src/handlers/broadcast.rs` |

**Descripción:**
En el bucle de broadcast:
```rust
match bot.send_message(chat_id, &broadcast_message).await {
    Ok(_) => success_count += 1,
    Err(e) => error_count += 1,
}
```
Si un usuario ha bloqueado al bot, Telegram devuelve `Forbidden: bot was blocked by the user`. Esto incrementa `error_count` pero no hace nada más.

**Impacto:**
- La base de datos conserva usuarios inactivos/bloqueados para siempre.
- Cada broadcast futuro seguirá intentando enviarles mensajes, desperdiciando recursos.

**Fix recomendado:**
Detectar errores `Forbidden` y marcar al usuario como `inactive` en la tabla `users`:
```rust
Err(teloxide::RequestError::Api(ApiError::BotBlocked)) => {
    db.mark_user_inactive(user.telegram_id).await.ok();
}
```

---

### Issue #11: Handler `/mis_pagos` lee tabla vacía

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | Functional |
| **Archivos** | `src/handlers/pagos.rs` |

**Descripción:**
Como consecuencia del Issue #1, `broadcast_payments` nunca se llena desde el webhook. El handler `/mis_pagos` siempre mostrará "No tienes pagos registrados" aunque el usuario haya pagado.

**Fix recomendado:**
Arreglar Issue #1.

---

### Issue #12: Falta `created_at` en tabla `broadcasts` (migración 006 la añade pero no verificada)

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | Database |
| **Archivos** | `migrations/006_fix_broadcast_created_at.sql` |

**Descripción:**
La migración 006 existe para añadir `created_at`, pero el código usa `sent_at` en la inserción (`src/handlers/broadcast.rs`, línea 500+). Hay inconsistencia de nombres de columnas.

**Fix recomendado:**
Verificar que el código y el schema estén alineados. `broadcast_extended.rs` usa `created_at` para leer.

---

### Issue #13: `test_mode = true` en config.toml para producción

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | Configuration |
| **Archivos** | `config.toml` |

**Descripción:**
```toml
[stripe]
test_mode = true
```

El usuario dijo que quiere pagos en modo test, pero para producción real esto debería ser `false`. El flag no se usa activamente en el código (no hay bifurcación lógica basada en él), pero es confuso.

**Fix recomendado:**
Documentar que este flag es informativo y que las claves de Stripe determinan el modo real.

---

### Issue #14: No hay manejo de rollback en wizard

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | UX |
| **Archivos** | `src/wizard.rs` |

**Descripción:**
Durante el registro wizard (6 pasos), el usuario no puede "volver atrás" a un paso anterior sin cancelar todo y empezar de nuevo.

**Impacto:**
- UX pobre. Si el usuario se equivoca en paso 5, debe cancelar y rellenar todo.

**Fix recomendado:**
Añadir botón "↩️ Volver" en cada paso del wizard que permita editar el paso anterior.

---

### Issue #15: `bot.log` antiguo en el repo

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | Maintenance |
| **Archivos** | `bot.log` |

**Descripción:**
Existe un archivo `bot.log` en el directorio del proyecto. En Docker, los logs deberían ir a stdout/stderr (lo cual ya hace `tracing-subscriber`), no a archivo.

**Fix recomendado:**
Añadir `*.log` a `.gitignore` y eliminar el archivo del repo.

---

### Issue #16: Falta `.env.example` actualizado

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | DevOps |
| **Archivos** | `.env.example` |

**Descripción:**
El `.env.example` no incluye todas las variables usadas:
- Falta `GLM_API_KEY` (se usa en código?).
- Falta `BOT_TOKEN` (usado en config.toml).
- No documenta `TELOXIDE_TOKEN_TEST`.

**Fix recomendado:**
Actualizar `.env.example` con todas las variables obligatorias y opcionales.

---

### Issue #17: Broadcast no notifica al emisor sobre errores de envío

| Field | Value |
|-------|-------|
| **Severity** | 🔵 Low |
| **Category** | UX |
| **Archivos** | `src/handlers/broadcast.rs` |

**Descripción:**
Al finalizar broadcast, solo se muestra:
```
Difusión completada: X exitosos, Y fallidos
```
No se detalla qué usuarios fallaron ni por qué.

**Fix recomendado:**
Incluir lista de usuarios con error (truncada si es muy larga) o sugerencia de contactar al admin si Y > 0.

---

### Issue #18: `escape_html` puede doblar escapes

| Field | Value |
|-------|-------|
| **Severity** | 🔵 Low |
| **Category** | Visual / Content |
| **Archivos** | `src/text_processor.rs` |

**Descripción:**
Si el usuario ya escapó caracteres (ej. escribió `&lt;`), `escape_html` los convertirá a `&amp;lt;`, mostrando texto incorrecto.

**Impacto:**
- Mínimo. La mayoría de usuarios no escriben HTML.

**Fix recomendado:**
Usar una librería de escaping que maneje entidades pre-escapadas, o documentar el comportamiento.

---

### Issue #19: `async-stripe` versión puede tener deprecaciones

| Field | Value |
|-------|-------|
| **Severity** | 🔵 Low |
| **Category** | Maintenance |
| **Archivos** | `Cargo.toml` |

**Descripción:**
```toml
async-stripe = { version = "0.39", features = ["runtime-tokio-hyper"] }
```
Versión 0.39 puede tener APIs deprecadas. Verificar compatibilidad con Stripe API versión actual.

---

### Issue #20: Falta documentación del flujo de membresía

| Field | Value |
|-------|-------|
| **Severity** | 🔵 Low |
| **Category** | Documentation |
| **Archivos** | `README.md`, `DEPLOYMENT.md` |

**Descripción:**
El sistema tiene lógica de membresía (`membership.price = 9.99`, `requires_membership_number = true`) pero no hay comando `/membresia` o `/pagar_membresia` expuesto en el menú. Solo existe a través del webhook indirecto.

**Fix recomendado:**
Documentar cómo se activa el flujo de membresía, o eliminar si no está implementado del todo.

---

### Issue #21: `docker-compose.cima20paas.yml` usa `TELOXIDE_TOKEN_TEST` como fallback

| Field | Value |
|-------|-------|
| **Severity** | 🟡 Medium |
| **Category** | DevOps |
| **Archivos** | `docker-compose.cima20paas.yml` |

**Descripción:**
```yaml
- TELOXIDE_TOKEN_TEST=${TELO...T:-}
```
El token de test se expone como variable de entorno en producción. Si no está vacío, el bot usará el token de test en lugar del de producción.

**Fix recomendado:**
En producción, eliminar `TELOXIDE_TOKEN_TEST` del compose o asegurar que no esté seteado.

---

## Testing Coverage

### Roles identificados y flujos revisados

| Rol | Flujos revisados | Estado |
|-----|------------------|--------|
| **Usuario nuevo (externo)** | `/start` → seleccionar tipo → `/help` → `/info` → `/chat` → `/buscar` | ✅ Código revisado |
| **Usuario nuevo (interno)** | `/start` → `/registrar` wizard completo | ✅ Código revisado |
| **Empresa pendiente** | Interacción post-registro, menú limitado | ✅ Código revisado |
| **Empresa aprobada** | `/misdatos`, `/mensajes`, `/difundir`, `/mis_difusiones` | ✅ Código revisado |
| **Admin** | `/pendientes`, `/aprobar`, `/rechazar`, `/admin_add_credits`, `/admin_org` | ✅ Código revisado |
| **Pagos (Stripe test)** | `/comprar_difusion` → checkout → webhook → créditos | ❌ **BUG CRÍTICO** (#1, #4) |

### Compilación y tests

- ✅ `cargo check` — OK
- ✅ `cargo build --release` — OK
- ✅ `cargo test` — 4/4 tests pasan

### Not Tested / Limitaciones

- **Interacción real con Telegram API**: No se pudo iniciar el bot y probar con usuarios reales desde este entorno. Se realizó análisis estático.
- **Stripe Checkout end-to-end**: No se pudo crear sesión de pago real porque requiere iniciar el bot y claves de test válidas expuestas.
- **Rendimiento con >100 usuarios**: El broadcast usa bucle secuencial; no se testeó escala.
- **Carga concurrente**: No se realizaron stress tests.

---

## Recomendaciones antes de desplegar en cima20paas

### Must Fix (bloqueantes)

1. **Fix Issue #2 (puertos)**: Cambiar `config.toml` a `web_server_port = 3000`.
2. **Fix Issue #1 (tabla payments)**: Crear migración para tabla `payments` O unificar con `broadcast_payments`.
3. **Fix Issue #4 (idempotencia webhook)**: Verificar `stripe_session_id` antes de procesar.
4. **Fix Issue #9 (channel_id)**: Configurar ID real del canal de Telegram.

### Should Fix (alta prioridad)

5. **Fix Issue #3 (estado en memoria)**: Documentar que reinicios pierden diálogos, o implementar persistencia.
6. **Fix Issue #6 (broadcast a todos)**: Filtrar por `is_internal` o `is_member`.
7. **Fix Issue #10 (usuarios bloqueados)**: Marcar inactivos en DB.
8. **Fix Issue #7 (validación inputs)**: Validar email, teléfono, CIF.

### Nice to have

9. Rate limiting (Issue #8).
10. Botón "volver" en wizard (Issue #14).
11. Limpiar `bot.log` del repo (Issue #15).

---

## Plan de despliegue corregido

```bash
# 1. Fix críticos en código
# - Unificar puerto a 3000
# - Crear migración 007_payments_table.sql (o fix webhook)
# - Añadir idempotencia en webhook

# 2. Commit y push
git add .
git commit -m "fix: pre-produccion - puertos, pagos, idempotencia webhook"
git push origin main

# 3. En cima20paas (Dokploy)
# - Crear proyecto desde GitHub repo
# - Configurar variables de entorno en Dokploy UI:
#   TELOXIDE_TOKEN=...
#   KIMI_API_KEY=...
#   STRIPE_SECRET_KEY=...
#   STRIPE_PUBLISHABLE_KEY=...
#   STRIPE_WEBHOOK_SECRET=...
#   BOT_TOKEN=...
# - Configurar webhook de Stripe apuntando a:
#   https://colegio-bot.paas.cima20.io/stripe/webhook
# - Asegurar que el canal de Telegram existe y el bot es admin

# 4. Verificar healthcheck
# curl https://colegio-bot.paas.cima20.io/health
```

---

*Reporte generado automáticamente por Hermes Agent. Requiere revisión humana antes de aplicar fixes.*

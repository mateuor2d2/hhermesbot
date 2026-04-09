//! Procesador centralizado de texto de Telegram
//!
//! Sanitiza texto de entrada y detecta comandos/funciones

use teloxide::types::{Message, ParseMode};

/// Resultado del procesamiento de texto
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessedText {
    /// Texto original
    pub original: String,
    /// Texto sanitizado para MarkdownV2
    pub sanitized: String,
    /// Comando detectado (si existe)
    pub command: Option<CommandDetected>,
    /// Texto sin el comando (args)
    pub args: String,
}

/// Comando detectado en el mensaje
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CommandDetected {
    Start,
    Help,
    Info,
    Chat,
    Buscar,
    MisDatos,
    Registrar,
    Mensajes,
    Pagar,
    Admin,
    Broadcast,
    Unknown(String),
}

impl ProcessedText {
    /// Procesa un mensaje de Telegram
    #[allow(dead_code)]
    pub fn from_message(msg: &Message) -> Option<Self> {
        let text = msg.text()?;
        Self::from_str(text)
    }

    /// Procesa un string directamente
    #[allow(dead_code)]
    pub fn from_str(text: &str) -> Option<Self> {
        let original = text.to_string();

        // Detectar comando (solo el primero si existe)
        let (command, args) = Self::extract_command(text);

        // Sanitizar para MarkdownV2
        let sanitized = escape_markdown_v2(text);
        let args_sanitized = escape_markdown_v2(&args);

        Some(Self {
            original,
            sanitized,
            command,
            args: args_sanitized,
        })
    }

    /// Extrae comando del texto
    #[allow(dead_code)]
    fn extract_command(text: &str) -> (Option<CommandDetected>, String) {
        let trimmed = text.trim();

        if !trimmed.starts_with('/') {
            return (None, text.to_string());
        }

        // Separar comando de argumentos
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"").to_string();

        let command = match cmd.as_str() {
            "/start" => CommandDetected::Start,
            "/help" => CommandDetected::Help,
            "/info" => CommandDetected::Info,
            "/chat" => CommandDetected::Chat,
            "/buscar" => CommandDetected::Buscar,
            "/misdatos" => CommandDetected::MisDatos,
            "/registrar" => CommandDetected::Registrar,
            "/mensajes" => CommandDetected::Mensajes,
            "/pagar" => CommandDetected::Pagar,
            "/admin" => CommandDetected::Admin,
            "/broadcast" => CommandDetected::Broadcast,
            other => CommandDetected::Unknown(other.to_string()),
        };

        (Some(command), args)
    }

    /// Verifica si el texto contiene un comando específico
    #[allow(dead_code)]
    pub fn is_command(&self, cmd: CommandDetected) -> bool {
        self.command.as_ref() == Some(&cmd)
    }

    /// Obtiene el texto a mostrar al usuario (sanitizado)
    #[allow(dead_code)]
    pub fn display_text(&self) -> &str {
        &self.sanitized
    }

    /// Obtiene el texto para enviar a IA (sin formato, pero seguro)
    #[allow(dead_code)]
    pub fn text_for_ai(&self) -> String {
        // Para IA quitamos el comando si existe
        self.args.clone()
    }
}

/// Escapa caracteres reservados de MarkdownV2
/// Caracteres: `_ * [ ] ( ) ~ ` > # + - = | { } . !`
pub fn escape_markdown_v2(text: &str) -> String {
    let reserved = [
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];

    let mut result = String::with_capacity(text.len() * 2);

    for ch in text.chars() {
        if reserved.contains(&ch) {
            result.push('\\');
        }
        result.push(ch);
    }

    result
}

/// Escapa caracteres para HTML
/// Solo necesita escapar: < > &
#[allow(dead_code)]
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Construye mensaje con parse mode seguro
#[allow(dead_code)]
pub fn safe_message(text: &str, use_html: bool) -> (String, ParseMode) {
    if use_html {
        (escape_html(text), ParseMode::Html)
    } else {
        (escape_markdown_v2(text), ParseMode::MarkdownV2)
    }
}

/// Limpia input de usuario para prevenir inyección
/// Elimina caracteres de control y limita longitud
#[allow(dead_code)]
pub fn sanitize_input(text: &str, max_len: usize) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t') // Permitir newline y tab
        .take(max_len)
        .collect();

    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_markdown_v2() {
        assert_eq!(escape_markdown_v2("hello-world"), "hello\\-world");
        assert_eq!(escape_markdown_v2("test.com"), "test\\.com");
        assert_eq!(escape_markdown_v2("(parentesis)"), "\\(parentesis\\)");
    }

    #[test]
    fn test_extract_command() {
        let (cmd, args) = ProcessedText::extract_command("/help");
        assert_eq!(cmd, Some(CommandDetected::Help));
        assert_eq!(args, "");

        let (cmd, args) = ProcessedText::extract_command("/buscar fontanero");
        assert_eq!(cmd, Some(CommandDetected::Buscar));
        assert_eq!(args, "fontanero");
    }

    #[test]
    fn test_no_command() {
        let (cmd, args) = ProcessedText::extract_command("hola mundo");
        assert_eq!(cmd, None);
        assert_eq!(args, "hola mundo");
    }
}

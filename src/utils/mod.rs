//! Utilidades generales del bot

/// Limpia y escapa texto para MarkdownV2 de Telegram
/// Escapa todos los caracteres reservados: _ * [ ] ( ) ~ ` > # + - = | { } . !
pub fn clean_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+' | '-' | '=' | '|' | '{' | '}' | '.' | '!' => {
                format!("\\{}", c)
            }
            _ => c.to_string(),
        })
        .collect()
}

/// Limpia texto pero preserva formato markdown intencional (*bold*, _italic_)
/// Solo escapa caracteres que NO son parte de marcas de formato conocidas
pub fn clean_text_preserve_formatting(text: &str) -> String {
    // Por ahora, misma implementación - podemos mejorarla luego
    clean_text(text)
}

/// Extrae comandos/funciones del texto del usuario
/// Devuelve (texto_limpio, función_detectada)
/// Por ahora no implementamos parsing de funciones - futura mejora
pub fn parse_user_input(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim();
    
    // Detectar si es un comando
    if trimmed.starts_with('/') {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let command = parts[0].to_string();
        let rest = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };
        return (rest, Some(command));
    }
    
    // Texto normal sin función
    (trimmed.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text_escapes_all() {
        let input = "Hello-world (test) [link] *bold* _italic_";
        let expected = "Hello\\-world \\(test\\) \\[link\\] \\*bold\\* \_italic_";
        assert_eq!(clean_text(input), expected);
    }

    #[test]
    fn test_parse_user_input_command() {
        let (text, func) = parse_user_input("/start hola mundo");
        assert_eq!(text, "hola mundo");
        assert_eq!(func, Some("/start".to_string()));
    }

    #[test]
    fn test_parse_user_input_normal() {
        let (text, func) = parse_user_input("hola mundo");
        assert_eq!(text, "hola mundo");
        assert_eq!(func, None);
    }
}

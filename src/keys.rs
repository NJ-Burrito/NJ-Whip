use global_hotkey::hotkey::HotKey;

/// Parse e.g. "Ctrl+Alt+W" or "Shift+F9". `Win` is accepted as a modifier name.
pub fn parse_hotkey(s: &str) -> Result<HotKey, String> {
    let s = s
        .split('+')
        .map(|t| {
            let t = t.trim();
            if t.eq_ignore_ascii_case("win") { "Super" } else { t }
        })
        .collect::<Vec<_>>()
        .join("+");
    s.parse::<HotKey>().map_err(|e| e.to_string())
}

/// Windows virtual-key code for a key name used in macro steps.
pub fn key_vk(name: &str) -> Option<u16> {
    let n = name.trim().to_ascii_uppercase();
    Some(match n.as_str() {
        "ESC" | "ESCAPE" => 0x1B,
        "ENTER" | "RETURN" => 0x0D,
        "TAB" => 0x09,
        "SPACE" => 0x20,
        "BACKSPACE" => 0x08,
        "CTRL" | "CONTROL" => 0x11,
        "ALT" => 0x12,
        "SHIFT" => 0x10,
        "WIN" => 0x5B,
        _ => {
            if let Some(f) = n.strip_prefix('F').and_then(|d| d.parse::<u16>().ok()) {
                return (1..=24).contains(&f).then(|| 0x70 + f - 1);
            }
            let b = n.as_bytes();
            if b.len() == 1 && b[0].is_ascii_alphanumeric() {
                return Some(b[0] as u16); // VK for A-Z and 0-9 equals ASCII
            }
            return None;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hotkey_ok() {
        assert!(parse_hotkey("Ctrl+Alt+W").is_ok());
        assert!(parse_hotkey("Shift+F9").is_ok());
        assert!(parse_hotkey("Win+Q").is_ok());
    }

    #[test]
    fn parse_hotkey_trims() {
        assert_eq!(parse_hotkey("Ctrl + Alt + W").unwrap(), parse_hotkey("Ctrl+Alt+W").unwrap());
    }

    #[test]
    fn parse_hotkey_bad() {
        assert!(parse_hotkey("Ctrl+").is_err());
        assert!(parse_hotkey("").is_err());
    }

    #[test]
    fn vk_names() {
        assert_eq!(key_vk("Esc"), Some(0x1B));
        assert_eq!(key_vk("Enter"), Some(0x0D));
        assert_eq!(key_vk("F1"), Some(0x70));
        assert_eq!(key_vk("F24"), Some(0x87));
        assert_eq!(key_vk("F25"), None);
        assert_eq!(key_vk("a"), Some(0x41));
        assert_eq!(key_vk("7"), Some(0x37));
        assert_eq!(key_vk("Win"), Some(0x5B));
        assert_eq!(key_vk("Nope"), None);
    }
}

//! 快捷键解析与校验

use crate::error::{LumarisError, LumarisResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyBinding {
    pub action: String,
    pub accelerator: Option<String>,
}

/// 规范化：Ctrl+Alt+Up 等形式
pub fn normalize_accelerator(input: &str) -> LumarisResult<String> {
    let s = input.trim();
    if s.is_empty() {
        return Err(LumarisError::hotkey("快捷键不能为空"));
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut key: Option<String> = None;

    for part in s.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" | "controlkey" => ctrl = true,
            "alt" | "option" | "menu" => alt = true,
            "shift" => shift = true,
            "win" | "super" | "meta" | "cmd" | "command" => meta = true,
            other => {
                if key.is_some() {
                    return Err(LumarisError::hotkey("快捷键只能包含一个主键"));
                }
                key = Some(normalize_key(other)?);
            }
        }
    }

    let key = key.ok_or_else(|| LumarisError::hotkey("不能只设置修饰键"))?;
    if !ctrl && !alt && !shift && !meta {
        // 允许 F 键单独，其他需要修饰键
        if !key.starts_with('F') || key.len() > 3 {
            return Err(LumarisError::hotkey("请配合修饰键使用"));
        }
    }

    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if meta {
        parts.push("Super");
    }
    parts.push(&key);
    Ok(parts.join("+"))
}

fn normalize_key(raw: &str) -> LumarisResult<String> {
    let u = raw.to_ascii_uppercase();
    let mapped = match u.as_str() {
        "UP" | "ARROWUP" => "ArrowUp",
        "DOWN" | "ARROWDOWN" => "ArrowDown",
        "LEFT" | "ARROWLEFT" => "ArrowLeft",
        "RIGHT" | "ARROWRIGHT" => "ArrowRight",
        "PGUP" | "PAGEUP" => "PageUp",
        "PGDN" | "PAGEDOWN" => "PageDown",
        "ESC" | "ESCAPE" => "Escape",
        "INS" | "INSERT" => "Insert",
        "DEL" | "DELETE" => "Delete",
        "SPACE" | "SPACEBAR" => "Space",
        "PLUS" | "ADD" | "=" => "Plus",
        "MINUS" | "SUBTRACT" | "-" => "Minus",
        "RETURN" | "ENTER" => "Enter",
        "TAB" => "Tab",
        "BACKSPACE" => "Backspace",
        k if k.len() == 1 && k.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) => {
            return Ok(k.to_string());
        }
        k if k.starts_with('F') && k[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
            return Ok(k.to_string());
        }
        k if k.starts_with("NUM") || k.starts_with("NUMPAD") => {
            return Ok(k.to_string());
        }
        _ => {
            // 保留原始大小写敏感的标准名
            if raw.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Ok(u);
            }
            return Err(LumarisError::hotkey(format!("不支持的按键: {raw}")));
        }
    };
    Ok(mapped.into())
}

/// 系统保留组合粗略拦截
pub fn is_system_reserved(accel: &str) -> bool {
    let lower = accel.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "ctrl+alt+delete"
            | "ctrl+shift+escape"
            | "alt+f4"
            | "alt+tab"
            | "win+l"
            | "super+l"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default() {
        let a = normalize_accelerator("Ctrl+Alt+Up").unwrap();
        assert_eq!(a, "Ctrl+Alt+ArrowUp");
    }

    #[test]
    fn reject_modifier_only() {
        assert!(normalize_accelerator("Ctrl+Alt").is_err());
    }
}

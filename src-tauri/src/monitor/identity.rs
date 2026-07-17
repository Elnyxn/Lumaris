//! 稳定显示器 ID：EDID / 设备路径组合哈希

use sha2::{Digest, Sha256};

/// 由多字段组合生成稳定 ID（非随机 UUID）
pub fn stable_monitor_id(parts: &[&str]) -> String {
    let joined = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("|");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    let hash = hasher.finalize();
    format!("mon_{}", hex::encode(&hash[..12]))
}

/// 从 DeviceID 字符串尽量解析有用片段
pub fn parse_device_id_fragments(device_id: &str) -> Vec<String> {
    device_id
        .split(&['\\', '#', '&'][..])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_unique() {
        let a = stable_monitor_id(&["DEL", "A123", "SN001", "\\\\.\\DISPLAY1"]);
        let b = stable_monitor_id(&["DEL", "A123", "SN002", "\\\\.\\DISPLAY2"]);
        let a2 = stable_monitor_id(&["DEL", "A123", "SN001", "\\\\.\\DISPLAY1"]);
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert!(a.starts_with("mon_"));
    }
}

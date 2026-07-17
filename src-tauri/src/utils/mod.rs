//! 通用工具

pub mod target;

/// 百分比与设备原始范围互转
pub fn percent_to_raw(percent: u32, min: u32, max: u32) -> u32 {
    if max <= min {
        return min;
    }
    let p = percent.min(100) as u64;
    let span = (max - min) as u64;
    min + ((p * span + 50) / 100) as u32
}

pub fn raw_to_percent(raw: u32, min: u32, max: u32) -> u32 {
    if max <= min {
        return 0;
    }
    let raw = raw.clamp(min, max);
    let span = (max - min) as u64;
    let val = (raw - min) as u64;
    ((val * 100 + span / 2) / span) as u32
}

pub fn clamp_percent(v: i32) -> u32 {
    v.clamp(0, 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_roundtrip_0_100() {
        assert_eq!(percent_to_raw(0, 0, 100), 0);
        assert_eq!(percent_to_raw(100, 0, 100), 100);
        assert_eq!(percent_to_raw(50, 0, 100), 50);
        assert_eq!(raw_to_percent(50, 0, 100), 50);
    }

    #[test]
    fn range_custom() {
        // 某些显示器 0-255
        let raw = percent_to_raw(50, 0, 255);
        assert!((120..=130).contains(&raw));
        let pct = raw_to_percent(raw, 0, 255);
        assert!((48..=52).contains(&pct));
    }
}

//! SDR 内容亮度百分比与 Windows SDR white level 原始值的纯转换。

const SDR_MIN_NITS: u32 = 80;
const SDR_MAX_NITS: u32 = 480;

pub fn percent_to_raw(percent: u32) -> u32 {
    let nits = SDR_MIN_NITS + percent.min(100) * (SDR_MAX_NITS - SDR_MIN_NITS) / 100;
    nits * 1000 / 80
}

pub fn raw_to_percent(raw: u32) -> u32 {
    let nits = raw.saturating_mul(80) / 1000;
    nits.saturating_sub(SDR_MIN_NITS).saturating_mul(100) / (SDR_MAX_NITS - SDR_MIN_NITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_matches_windows_range() {
        assert_eq!(percent_to_raw(0), 1000);
        assert_eq!(percent_to_raw(50), 3500);
        assert_eq!(percent_to_raw(100), 6000);
        assert_eq!(percent_to_raw(150), 6000);
        assert_eq!(raw_to_percent(1000), 0);
        assert_eq!(raw_to_percent(3500), 50);
        assert_eq!(raw_to_percent(6000), 100);
    }
}

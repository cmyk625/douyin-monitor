use anyhow::{anyhow, Result};
use chrono::{Local, TimeZone};
use std::path::PathBuf;

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn today_start_ts() -> i64 {
    let now = Local::now();
    let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap_or_default();
    Local
        .from_local_datetime(&start)
        .single()
        .map(|dt| dt.timestamp())
        .unwrap_or_else(now_ts)
}

pub fn fmt_ts(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// 精确到分钟的时间（消息里的「发布时间」用）。
pub fn fmt_minute(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// 格式化发布时间：若缺失或为 0，明确返回「未知」。
pub fn fmt_publish_time(ts: Option<i64>) -> String {
    match ts {
        Some(t) if t > 0 => fmt_minute(t),
        _ => "未知".to_string(),
    }
}

/// 把 "1.2万" / "3.4亿" / "1,234" / "123" 解析为 (数值, 是否为带单位的近似值)
pub fn parse_count_detailed(text: &str) -> Option<(i64, bool)> {
    let text = text.trim().replace([',', '，', ' '], "");
    if text.is_empty() {
        return None;
    }
    let (num_part, factor, is_approximate) = if let Some(rest) = text.strip_suffix('亿') {
        (rest, 100_000_000f64, true)
    } else if let Some(rest) = text.strip_suffix('万') {
        (rest, 10_000f64, true)
    } else if let Some(rest) = text.strip_suffix('w') {
        (rest, 10_000f64, true)
    } else if let Some(rest) = text.strip_suffix('W') {
        (rest, 10_000f64, true)
    } else {
        (text.as_str(), 1f64, false)
    };
    let value: f64 = num_part.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(((value * factor).round() as i64, is_approximate))
}

/// 把 "1.2万" / "3.4亿" / "1,234" / "123" 解析为整数
pub fn parse_count(text: &str) -> Option<i64> {
    parse_count_detailed(text).map(|(v, _)| v)
}

/// 格式化数值展示：满 1 万展示为 x.x万（如 1.2万、1万），满 1 亿展示为 x.x亿，不足 1 万展示具体整数（如 500）
pub fn fmt_count_display(n: i64) -> String {
    let abs_n = n.abs();
    if abs_n >= 100_000_000 {
        let val = (n as f64) / 100_000_000.0;
        let rounded = (val * 10.0).round() / 10.0;
        let formatted = format!("{:.1}", rounded);
        let trimmed = formatted.trim_end_matches(".0").trim_end_matches('0');
        let final_str = if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.')
        } else {
            trimmed
        };
        format!("{final_str}亿")
    } else if abs_n >= 10_000 {
        let val = (n as f64) / 10_000.0;
        let rounded = (val * 10.0).round() / 10.0;
        let formatted = format!("{:.1}", rounded);
        let trimmed = formatted.trim_end_matches(".0").trim_end_matches('0');
        let final_str = if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.')
        } else {
            trimmed
        };
        format!("{final_str}万")
    } else {
        n.to_string()
    }
}

pub fn sanitize_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if !ch.is_whitespace() {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("item");
    }
    out.truncate(48);
    out
}

pub fn ensure_dir(path: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|err| anyhow!("创建目录失败 {}：{err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_counts() {
        assert_eq!(parse_count("1.2万"), Some(12000));
        assert_eq!(parse_count("3.4亿"), Some(340_000_000));
        assert_eq!(parse_count("1,234"), Some(1234));
        assert_eq!(parse_count("42"), Some(42));
        assert_eq!(parse_count("暂无"), None);

        assert_eq!(parse_count_detailed("1.2万"), Some((12000, true)));
        assert_eq!(parse_count_detailed("3.4亿"), Some((340_000_000, true)));
        assert_eq!(parse_count_detailed("1,234"), Some((1234, false)));
        assert_eq!(parse_count_detailed("42"), Some((42, false)));
    }

    #[test]
    fn formats_count_display() {
        assert_eq!(fmt_count_display(0), "0");
        assert_eq!(fmt_count_display(500), "500");
        assert_eq!(fmt_count_display(9999), "9999");
        assert_eq!(fmt_count_display(10000), "1万");
        assert_eq!(fmt_count_display(12000), "1.2万");
        assert_eq!(fmt_count_display(12345), "1.2万");
        assert_eq!(fmt_count_display(20000), "2万");
        assert_eq!(fmt_count_display(25000), "2.5万");
        assert_eq!(fmt_count_display(100_000_000), "1亿");
        assert_eq!(fmt_count_display(150_000_000), "1.5亿");
    }

    #[test]
    fn formats_publish_time() {
        assert_eq!(fmt_publish_time(None), "未知");
        assert_eq!(fmt_publish_time(Some(0)), "未知");
        assert_eq!(fmt_publish_time(Some(-1)), "未知");
        assert!(fmt_publish_time(Some(1_700_000_000)).starts_with("2023-"));
    }
}

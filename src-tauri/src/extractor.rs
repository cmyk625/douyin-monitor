use crate::extractor_js::SCRIPT_TEMPLATE;
use crate::models::{ExtractPreview, RawWork};
use crate::settings::Settings;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    pub card_selectors: Vec<String>,
    pub labels: HashMap<String, Vec<String>>,
    pub max_works: usize,
}

impl ExtractorConfig {
    pub fn from_settings(settings: &Settings) -> Self {
        let card_selectors = settings
            .card_selectors
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();
        let labels = if settings.metric_labels.trim().is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(&settings.metric_labels).unwrap_or_default()
        };
        Self {
            card_selectors,
            labels,
            max_works: settings.max_works.clamp(1, 500) as usize,
        }
    }

    /// 生成注入页面的脚本；账号配置了自定义脚本时优先使用。
    pub fn script(&self, custom_script: &str) -> String {
        if !custom_script.trim().is_empty() {
            return custom_script.to_string();
        }
        let config = json!({
            "cardSelectors": self.card_selectors,
            "labels": self.labels,
            "maxWorks": self.max_works,
        });
        SCRIPT_TEMPLATE.replace("__CFG__", &config.to_string())
    }
}

/// 解析页面脚本返回值中的 works 数组。
pub fn parse_works(value: &Value) -> Result<Vec<RawWork>> {
    let works_value = match value.get("works") {
        Some(Value::Array(items)) => Value::Array(items.clone()),
        Some(other) => other.clone(),
        None => {
            if value.is_array() {
                value.clone()
            } else {
                return Err(anyhow!("解析脚本未返回 works 数组"));
            }
        }
    };
    let works: Vec<RawWork> = serde_json::from_value(works_value)
        .map_err(|err| anyhow!("作品数据格式不正确：{err}"))?;
    Ok(works
        .into_iter()
        .filter(|work| !work.metrics.is_empty() || !work.url.is_empty())
        .collect())
}

pub struct ParsedPage {
    pub works: Vec<RawWork>,
    pub preview: ExtractPreview,
}

/// 解析页面脚本返回值，得到完整作品列表与预览信息（预览只保留前 20 条）。
pub fn parse_page(value: &Value) -> Result<ParsedPage> {
    let works = parse_works(value)?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let preview = ExtractPreview {
        ok: value
            .get("ok")
            .and_then(|item| item.as_bool())
            .unwrap_or(!works.is_empty()),
        blocked: value
            .get("blocked")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        reason: text("reason"),
        page_url: text("pageUrl"),
        page_title: text("pageTitle"),
        works: works.len(),
        sample: works.iter().take(20).cloned().collect(),
        debug: value.get("debug").cloned().unwrap_or(Value::Null),
    };
    Ok(ParsedPage { works, preview })
}

pub fn parse_preview(value: &Value) -> Result<ExtractPreview> {
    Ok(parse_page(value)?.preview)
}

/// 解析 /aweme/v1/web/aweme/detail/ 接口返回的原生 JSON 数据。
pub fn parse_aweme_detail_json(value: &Value, expected_id: &str) -> Result<RawWork> {
    let detail = value
        .get("aweme_detail")
        .ok_or_else(|| anyhow!("响应缺少 aweme_detail 节点"))?;

    let aweme_id = detail
        .get("aweme_id")
        .and_then(|v| v.as_str())
        .unwrap_or(expected_id);
    if !expected_id.is_empty() && aweme_id != expected_id {
        return Err(anyhow!("作品 ID 不匹配：期望 {expected_id}，实际返回 {aweme_id}"));
    }

    // 检查下架或私密状态
    if let Some(status) = detail.get("status") {
        if status.get("is_delete").and_then(|v| v.as_bool()) == Some(true)
            || status.get("status_code").and_then(|v| v.as_i64()) == Some(2004)
        {
            return Err(anyhow!("视频已下架或删除"));
        }
        if status.get("private_status").and_then(|v| v.as_i64()) == Some(1)
            || status.get("status_code").and_then(|v| v.as_i64()) == Some(2005)
        {
            return Err(anyhow!("视频为私密状态"));
        }
    }

    let stats = detail
        .get("statistics")
        .ok_or_else(|| anyhow!("aweme_detail 缺少 statistics 字段"))?;

    let play_val = stats.get("play_count").and_then(|v| v.as_i64()).filter(|&v| v > 0);
    let like_val = stats.get("digg_count").and_then(|v| v.as_i64());
    let comment_val = stats.get("comment_count").and_then(|v| v.as_i64());
    let share_val = stats.get("share_count").and_then(|v| v.as_i64());
    let collect_val = stats.get("collect_count").and_then(|v| v.as_i64());

    let metrics = crate::models::Metrics {
        play: play_val,
        like: like_val,
        comment: comment_val,
        share: share_val,
        collect: collect_val,
    };

    let author_name = detail
        .get("author")
        .and_then(|a| a.get("nickname").or_else(|| a.get("name")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let title = detail
        .get("desc")
        .or_else(|| detail.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let published_at = detail
        .get("create_time")
        .and_then(|v| v.as_i64())
        .filter(|&t| t > 0);

    Ok(RawWork {
        aweme_id: aweme_id.to_string(),
        title,
        author_name,
        url: format!("https://www.douyin.com/video/{aweme_id}"),
        published_at,
        metrics,
        is_approximate: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_works_payload() {
        let payload = json!({
            "ok": true,
            "blocked": false,
            "pageUrl": "https://creator.douyin.com/x",
            "works": [
                {
                    "aweme_id": "123",
                    "title": "作品一",
                    "url": "https://www.douyin.com/video/123",
                    "published_at": 1700000000,
                    "metrics": { "play": 12000, "like": 300, "comment": null, "share": 5, "collect": null }
                },
                {
                    "aweme_id": "456",
                    "title": "没有指标",
                    "url": "",
                    "published_at": null,
                    "metrics": { "play": null, "like": null, "comment": null, "share": null, "collect": null }
                }
            ]
        });
        let preview = parse_preview(&payload).unwrap();
        assert!(preview.ok);
        assert_eq!(preview.works, 1);
        assert_eq!(preview.sample[0].metrics.play, Some(12000));
        assert_eq!(preview.sample[0].metrics.comment, None);
    }

    #[test]
    fn parses_aweme_detail_payload() {
        let payload = json!({
            "status_code": 0,
            "aweme_detail": {
                "aweme_id": "7687985192954640366",
                "desc": "每天体验一种人生",
                "create_time": 1790437680,
                "author": {
                    "nickname": "水水木"
                },
                "statistics": {
                    "digg_count": 50894,
                    "comment_count": 448,
                    "share_count": 21757,
                    "collect_count": 9218,
                    "recommend_count": 247,
                    "play_count": 0
                },
                "status": {
                    "is_delete": false,
                    "private_status": 0
                }
            }
        });
        let work = parse_aweme_detail_json(&payload, "7687985192954640366").unwrap();
        assert_eq!(work.aweme_id, "7687985192954640366");
        assert_eq!(work.author_name, "水水木");
        assert_eq!(work.title, "每天体验一种人生");
        assert_eq!(work.published_at, Some(1790437680));
        assert_eq!(work.metrics.like, Some(50894));
        assert_eq!(work.metrics.comment, Some(448));
        assert_eq!(work.metrics.share, Some(21757));
        assert_eq!(work.metrics.collect, Some(9218));
        assert_eq!(work.metrics.play, None);
        assert!(!work.is_approximate);
    }

    #[test]
    fn custom_script_wins() {
        let config = ExtractorConfig {
            card_selectors: vec![],
            labels: HashMap::new(),
            max_works: 10,
        };
        assert_eq!(config.script("() => []"), "() => []");
        assert!(config.script("").contains("cardSelectors"));
    }
}

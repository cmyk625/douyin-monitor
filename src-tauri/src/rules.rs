use crate::models::{Alert, AlertCandidate};
use crate::repo_alert;
use anyhow::Result;
use rusqlite::Connection;

/// 报警消息：按固定格式拼装（负责人 / 增量提示 / 作者 / 发布时间 / 各指标增量）。
#[allow(clippy::too_many_arguments)]
pub fn build_message(
    candidate: &AlertCandidate,
    keyword: &str,
    account_name: &str,
    work_title: &str,
    work_url: &str,
    published_at: Option<i64>,
    _now: i64,
) -> String {
    let mut lines = Vec::new();
    let owner = candidate.owner_name.trim();
    if !owner.is_empty() {
        lines.push(format!("尊敬的负责人 {owner}："));
        lines.push(String::new());
    }
    let keyword = keyword.trim();
    lines.push(if keyword.is_empty() {
        "监测到发布的媒体内容有新的媒体增量".to_string()
    } else {
        // 机器人开了「自定义关键词」安全设置时，消息里必须包含关键词
        format!("监测到发布的媒体内容有新的媒体增量（{keyword}）")
    });
    lines.push(String::new());

    let author_display = if !candidate.author_name.trim().is_empty() {
        candidate.author_name.trim()
    } else if !account_name.trim().is_empty() {
        account_name.trim()
    } else {
        "未知作者"
    };
    let mut author = format!("作者：{author_display} | ▶ {work_title}");
    if !work_url.trim().is_empty() {
        author.push(' ');
        author.push_str(work_url.trim());
    }
    lines.push(author);
    lines.push(String::new());

    // 采集不到发布时间时明确显示「未知」，避免伪造发布时间造成误导
    lines.push(format!(
        "发布时间：{}",
        crate::util::fmt_publish_time(published_at)
    ));
    lines.push(String::new());

    lines.push(format!(
        "👍{}|💬{}",
        fmt_metric(&candidate.windows[1]),
        fmt_metric(&candidate.windows[2])
    ));
    lines.push(String::new());
    lines.push(format!(
        "⭐{}|🔄{}",
        fmt_metric(&candidate.windows[4]),
        fmt_metric(&candidate.windows[3])
    ));
    lines.join("\n")
}

/// 形如 `898(+176)` 或 `约1.2万(+约500)`；满 1 万转为万，不足 1 万保留具体数值；若是简写近似值带「约」；窗口基线缺失时只显示现值，指标不可用显示 `—`。
fn fmt_metric(window: &crate::models::MetricWindow) -> String {
    let prefix = if window.is_approximate { "约" } else { "" };
    match (window.current, window.delta()) {
        (Some(current), Some(delta)) => {
            let cur_str = crate::util::fmt_count_display(current);
            let sign = if delta >= 0 { "+" } else { "" };
            let delta_str = crate::util::fmt_count_display(delta);
            format!("{prefix}{cur_str}({sign}{prefix}{delta_str})")
        }
        (Some(current), None) => {
            let cur_str = crate::util::fmt_count_display(current);
            format!("{prefix}{cur_str}(—)")
        }
        (None, _) => "—".to_string(),
    }
}

/// 评估全部启用规则，命中且未处于本周期已通知保护期的写入 alerts 表并返回。
pub fn evaluate(conn: &Connection, now: i64) -> Result<Vec<Alert>> {
    let rules = repo_alert::list_rules(conn)?;
    let mut created = Vec::new();

    for rule in rules.into_iter().filter(|rule| rule.enabled) {
        let candidates = repo_alert::find_candidates(conn, &rule, now)?;
        for mut candidate in candidates {
            let dedup_key = format!("{}:{}:{}", candidate.rule_id, candidate.work_id, candidate.metric);
            let cooldown_since = now - rule.window_minutes.max(1) * 60;
            if repo_alert::cooldown_active(conn, &dedup_key, cooldown_since)? {
                continue;
            }
            let (work_title, work_url, published_at, author_name) = crate::repo_work::work_brief(conn, candidate.work_id)?;
            if candidate.author_name.is_empty() && !author_name.is_empty() {
                candidate.author_name = author_name;
            }
            let account_name = crate::repo::get_account(conn, candidate.account_id)?
                .map(|account| account.name)
                .unwrap_or_else(|| format!("#{}", candidate.account_id));
            let keyword = crate::config::AppConfig::load().feishu.keyword;
            let message = build_message(
                &candidate,
                &keyword,
                &account_name,
                &work_title,
                &work_url,
                published_at,
                now,
            );
            let alert_id = repo_alert::insert_alert(conn, &candidate, &dedup_key, &message, now)?;
            if let Some(alert) = repo_alert::get_alert(conn, alert_id)? {
                created.push(alert);
            }
        }
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AccountInput, Metrics, RawWork};
    use crate::util::parse_count;

    fn setup() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        let input = AccountInput {
            id: None,
            name: "测试账号".into(),
            sec_uid: String::new(),
            target_url: String::new(),
            parse_script: String::new(),
            enabled: true,
            note: String::new(),
        };
        crate::repo::create_account(&conn, &input, "profile", 0).unwrap();
        conn
    }

    /// 加入监控名单（只有名单内的视频才会参与报警评估）。
    fn monitor(conn: &Connection, owner_name: &str) {
        let input = crate::models::MonitoredInput {
            id: None,
            account_id: 1,
            target: "123".into(),
            owner_name: owner_name.into(),
            enabled: true,
        };
        crate::repo_monitored::save_monitored(conn, &input, None, 0).unwrap();
    }

    fn work(conn: &Connection, now: i64, baseline: i64, current: i64) -> i64 {
        let raw = RawWork {
            aweme_id: "123".into(),
            title: "一条作品".into(),
            author_name: "真实作者A".into(),
            url: "https://www.douyin.com/video/123".into(),
            published_at: None,
            metrics: Metrics {
                play: Some(baseline),
                ..Default::default()
            },
            is_approximate: false,
        };
        let change = crate::repo_work::upsert_work(conn, 1, &raw, now - 3600).unwrap();
        let now_metrics = Metrics {
            play: Some(current),
            ..Default::default()
        };
        crate::repo_work::insert_snapshot(conn, change.work_id, 1, now, &now_metrics, false).unwrap();
        change.work_id
    }

    fn rule(conn: &Connection, threshold: i64) {
        let input = crate::models::RuleInput {
            id: None,
            name: "播放飙升".into(),
            account_id: None,
            metric: "play".into(),
            window_minutes: 30,
            threshold,
            cooldown_minutes: 60,
            enabled: true,
        };
        crate::repo_alert::save_rule(conn, &input, 0).unwrap();
    }

    #[test]
    fn triggers_when_delta_exceeds_threshold() {
        let conn = setup();
        let now = 1_700_000_000;
        work(&conn, now, 1_000, 20_000);
        monitor(&conn, "张三");
        rule(&conn, 10_000);

        let alerts = evaluate(&conn, now).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].delta, 19_000);
        assert!(alerts[0].message.contains("尊敬的负责人 张三："));
        assert!(alerts[0].message.contains("监测到发布的媒体内容有新的媒体增量"));
        assert!(alerts[0].message.contains("作者：真实作者A | ▶ 一条作品 https://www.douyin.com/video/123"));
        assert!(alerts[0].message.contains("发布时间：未知"), "{}", alerts[0].message);
        assert!(!alerts[0].message.contains("▶️"), "飞书通知不再包含播放量：{}", alerts[0].message);
        assert!(!alerts[0].message.contains("<at"), "不再使用飞书 @ 标签");
        assert_eq!(alerts[0].owner_name, "张三");
        assert_eq!(alerts[0].author_name, "真实作者A");

        // 窗口防重保护期内再次评估不应重复报警
        let again = evaluate(&conn, now + 60).unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn renders_all_metric_deltas_in_template() {
        let conn = setup();
        let now = 1_700_000_000;
        let raw = RawWork {
            aweme_id: "123".into(),
            title: "一条作品".into(),
            author_name: "博主小王".into(),
            url: "https://www.douyin.com/video/123".into(),
            published_at: Some(now - 7200),
            metrics: Metrics {
                play: Some(1_000),
                like: Some(150),
                comment: Some(20),
                share: Some(45),
                collect: Some(60),
            },
            is_approximate: false,
        };
        let change = crate::repo_work::upsert_work(&conn, 1, &raw, now - 3600).unwrap();
        crate::repo_work::insert_snapshot(
            &conn,
            change.work_id,
            1,
            now,
            &Metrics {
                play: Some(20_000),
                like: Some(200),
                comment: Some(30),
                share: Some(60),
                collect: Some(80),
            },
            false,
        )
        .unwrap();
        monitor(&conn, "关孝雅");
        rule(&conn, 10_000);

        let alerts = evaluate(&conn, now).unwrap();
        assert_eq!(alerts.len(), 1);
        let message = alerts[0].message.clone();
        assert!(message.contains("作者：博主小王 | ▶ 一条作品 https://www.douyin.com/video/123"), "{message}");
        assert!(message.contains("👍200(+50)|💬30(+10)"), "{message}");
        assert!(message.contains("⭐80(+20)|🔄60(+15)"), "{message}");
        assert!(!message.contains("▶️"), "飞书通知不再包含播放量：{message}");
        assert!(
            message.contains(&format!("发布时间：{}", crate::util::fmt_minute(now - 7200))),
            "{message}"
        );
    }

    #[test]
    fn skips_works_outside_monitor_list() {
        let conn = setup();
        let now = 1_700_000_000;
        work(&conn, now, 1_000, 20_000);
        rule(&conn, 10_000);

        // 没有加入监控名单的视频不参与评估
        let alerts = evaluate(&conn, now).unwrap();
        assert!(alerts.is_empty(), "未配置监控的视频不应报警");

        // 名单中的视频被停用后同样不评估
        monitor(&conn, "张三");
        conn.execute("UPDATE monitored_works SET enabled = 0", []).unwrap();
        assert!(evaluate(&conn, now).unwrap().is_empty(), "停用的监控视频不应报警");

        conn.execute("UPDATE monitored_works SET enabled = 1", []).unwrap();
        assert_eq!(evaluate(&conn, now).unwrap().len(), 1);
    }

    #[test]
    fn skips_when_baseline_missing() {
        let conn = setup();
        let now = 1_700_000_000;
        // 唯一的历史快照落在 30 分钟窗口内部（10 分钟前），没有可用的窗口起点基线
        let raw = RawWork {
            aweme_id: "123".into(),
            title: "一条作品".into(),
            author_name: "博主小王".into(),
            url: String::new(),
            published_at: None,
            metrics: Metrics {
                play: Some(1_000),
                ..Default::default()
            },
            is_approximate: false,
        };
        let change = crate::repo_work::upsert_work(&conn, 1, &raw, now - 600).unwrap();
        crate::repo_work::insert_snapshot(
            &conn,
            change.work_id,
            1,
            now,
            &Metrics {
                play: Some(20_000),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        monitor(&conn, "张三");
        rule(&conn, 10_000);

        let alerts = evaluate(&conn, now).unwrap();
        assert!(alerts.is_empty(), "窗口数据不足时不应报警");
    }

    #[test]
    fn skips_when_below_threshold() {
        let conn = setup();
        let now = 1_700_000_000;
        work(&conn, now, 1_000, 5_000);
        monitor(&conn, "张三");
        rule(&conn, 10_000);
        let alerts = evaluate(&conn, now).unwrap();
        assert!(alerts.is_empty(), "增量低于阈值时不应报警");
    }

    #[test]
    fn unavailable_current_metric_does_not_alert_from_history() {
        let conn = setup();
        let now = 1_700_000_000;
        work(&conn, now, 1_000, 20_000);
        monitor(&conn, "负责人");
        rule(&conn, 10_000);
        conn.execute("UPDATE works SET play = NULL", []).unwrap();
        assert!(evaluate(&conn, now).unwrap().is_empty());
    }

    #[test]
    fn parses_douyin_units() {
        assert_eq!(parse_count("1.2万"), Some(12_000));
        assert_eq!(parse_count("9,876"), Some(9_876));
    }

    #[test]
    fn test_window_cycle_and_subsequent_growth_alert() {
        let conn = setup();
        let now = 1_700_000_000;
        monitor(&conn, "张三");
        // 规则：30 分钟增长 100
        let input = crate::models::RuleInput {
            id: None,
            name: "点赞增长".into(),
            account_id: None,
            metric: "like".into(),
            window_minutes: 30,
            threshold: 100,
            cooldown_minutes: 0,
            enabled: true,
        };
        crate::repo_alert::save_rule(&conn, &input, 0).unwrap();

        // 1. T=0 时录入快照：点赞 1000
        let raw = RawWork {
            aweme_id: "123".into(),
            title: "爆款测试作品".into(),
            author_name: "作者A".into(),
            url: "https://www.douyin.com/video/123".into(),
            published_at: None,
            metrics: Metrics {
                like: Some(1000),
                ..Default::default()
            },
            is_approximate: false,
        };
        let change = crate::repo_work::upsert_work(&conn, 1, &raw, now).unwrap();
        let work_id = change.work_id;

        // 2. T=30 分钟 (now + 1800) 时新增快照：点赞 1120（增长 120 >= 100）
        let m1 = Metrics {
            like: Some(1120),
            ..Default::default()
        };
        crate::repo_work::insert_snapshot(&conn, work_id, 1, now + 1800, &m1, false).unwrap();

        // 此时评估：触发第 1 次报警！
        let alerts1 = evaluate(&conn, now + 1800).unwrap();
        assert_eq!(alerts1.len(), 1);
        assert_eq!(alerts1[0].current_value, 1120);
        assert_eq!(alerts1[0].baseline_value, 1000);
        assert_eq!(alerts1[0].delta, 120);

        // 3. T=40 分钟 (now + 2400) 仍在 30 分钟保护期内，且仅微增到 1130
        let m2 = Metrics {
            like: Some(1130),
            ..Default::default()
        };
        crate::repo_work::insert_snapshot(&conn, work_id, 1, now + 2400, &m2, false).unwrap();
        let alerts2 = evaluate(&conn, now + 2400).unwrap();
        assert!(alerts2.is_empty(), "30分钟保护期内不应重复报警");

        // 4. T=60 分钟 (now + 3600)，过了 30 分钟周期，但点赞仅为 1150（相对上次报警 1120 仅增 30 < 100）
        let m3 = Metrics {
            like: Some(1150),
            ..Default::default()
        };
        crate::repo_work::insert_snapshot(&conn, work_id, 1, now + 3600, &m3, false).unwrap();
        let alerts3 = evaluate(&conn, now + 3600).unwrap();
        assert!(alerts3.is_empty(), "下个 30 分钟内未增长 100 不应触发报警");

        // 5. T=60 分钟若点赞爆发到了 1250（相对上次报警 1120 净增 130 >= 100）
        let m4 = Metrics {
            like: Some(1250),
            ..Default::default()
        };
        crate::repo_work::insert_snapshot(&conn, work_id, 1, now + 3600, &m4, false).unwrap();
        let alerts4 = evaluate(&conn, now + 3600).unwrap();
        assert_eq!(alerts4.len(), 1, "下个 30 分钟增长超过 100 应成功触发第 2 次报警");
        assert_eq!(alerts4[0].current_value, 1250);
        assert_eq!(alerts4[0].baseline_value, 1120);
        assert_eq!(alerts4[0].delta, 130);
    }
}

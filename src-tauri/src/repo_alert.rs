use crate::models::{Alert, AlertCandidate, MetricWindow, Rule, RuleInput};
use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};

/// 指标名到快照列的映射（白名单，用于拼接 SQL）。
pub fn metric_column(metric: &str) -> Option<&'static str> {
    match metric {
        "play" => Some("play"),
        "like" => Some("like_cnt"),
        "comment" => Some("comment_cnt"),
        "share" => Some("share_cnt"),
        "collect" => Some("collect_cnt"),
        _ => None,
    }
}

fn map_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<Rule> {
    Ok(Rule {
        id: row.get(0)?,
        name: row.get(1)?,
        account_id: row.get(2)?,
        account_name: row.get(3)?,
        metric: row.get(4)?,
        window_minutes: row.get(5)?,
        threshold: row.get(6)?,
        cooldown_minutes: row.get(7)?,
        enabled: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
    })
}

pub fn list_rules(conn: &Connection) -> Result<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.name, r.account_id, a.name, r.metric, r.window_minutes, r.threshold,
                r.cooldown_minutes, r.enabled, r.created_at
         FROM rules r LEFT JOIN accounts a ON a.id = r.account_id
         ORDER BY r.enabled DESC, r.id ASC",
    )?;
    let rows = stmt.query_map([], map_rule)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn save_rule(conn: &Connection, input: &RuleInput, now: i64) -> Result<i64> {
    if metric_column(&input.metric).is_none() {
        bail!("不支持的指标：{}", input.metric);
    }
    if input.window_minutes < 1 || input.threshold < 1 {
        bail!("窗口与阈值必须为正整数");
    }
    let cooldown = if input.cooldown_minutes > 0 {
        input.cooldown_minutes
    } else {
        input.window_minutes
    };
    let name = if input.name.trim().is_empty() {
        format!("{} 增长监控", input.metric)
    } else {
        input.name.trim().to_string()
    };
    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE rules SET name = ?2, account_id = ?3, metric = ?4, window_minutes = ?5,
                    threshold = ?6, cooldown_minutes = ?7, enabled = ?8 WHERE id = ?1",
                params![
                    id,
                    name,
                    input.account_id,
                    input.metric,
                    input.window_minutes,
                    input.threshold,
                    cooldown,
                    input.enabled as i64
                ],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO rules(name, account_id, metric, window_minutes, threshold, cooldown_minutes, enabled, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    name,
                    input.account_id,
                    input.metric,
                    input.window_minutes,
                    input.threshold,
                    cooldown,
                    input.enabled as i64,
                    now
                ],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn delete_rule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM rules WHERE id = ?1", params![id])?;
    Ok(())
}

/// 查询所有命中「窗口内增量 ≥ 阈值」的**监控名单内**作品，并带上全部指标的现值/窗口基线。
pub fn find_candidates(conn: &Connection, rule: &Rule, now: i64) -> Result<Vec<AlertCandidate>> {
    let Some(column) = metric_column(&rule.metric) else {
        return Ok(Vec::new());
    };
    let baseline_ts = now - rule.window_minutes * 60;
    let min_seen = now - 90 * 86_400;
    // 消息里要展示每个指标的「现值(+增量)」，所以这里对 5 个指标都取最新值与窗口起点基线。
    let mut metric_columns = String::new();
    for (index, key) in crate::models::METRIC_KEYS.iter().enumerate() {
        let Some(col) = metric_column(key) else { continue };
        metric_columns.push_str(&format!(
            ",
                (SELECT s.{col} FROM snapshots s
                  WHERE s.work_id = w.id AND s.{col} IS NOT NULL
                  ORDER BY s.ts DESC LIMIT 1) AS cur{index},
                (SELECT s.{col} FROM snapshots s
                  WHERE s.work_id = w.id AND s.{col} IS NOT NULL AND s.ts <= ?1
                  ORDER BY s.ts DESC LIMIT 1) AS base{index}"
        ));
    }
    let sql = format!(
        "SELECT id, account_id, cur,
                CASE
                    WHEN last_alert_val IS NOT NULL AND raw_base IS NOT NULL
                        THEN MAX(raw_base, last_alert_val)
                    ELSE raw_base
                END AS base,
                owner_name, owner_open_id, author_name, is_approximate,
                cur0, base0, cur1, base1, cur2, base2, cur3, base3, cur4, base4
         FROM (
            SELECT w.id AS id, w.account_id AS account_id,
                (SELECT s.{column} FROM snapshots s
                  WHERE s.work_id = w.id AND s.{column} IS NOT NULL
                  ORDER BY s.ts DESC LIMIT 1) AS cur,
                (SELECT s.{column} FROM snapshots s
                  WHERE s.work_id = w.id AND s.{column} IS NOT NULL AND s.ts <= ?1
                  ORDER BY s.ts DESC LIMIT 1) AS raw_base,
                (SELECT a.current_value FROM alerts a
                  WHERE a.rule_id = ?5 AND a.work_id = w.id AND a.metric = ?6
                  ORDER BY a.created_at DESC LIMIT 1) AS last_alert_val,
                m.owner_name AS owner_name,
                m.owner_open_id AS owner_open_id,
                COALESCE(NULLIF(w.author_name, ''), NULLIF(m.author_name, ''), '') AS author_name,
                w.is_approximate AS is_approximate{metric_columns}
            FROM works w
            JOIN monitored_works m ON m.account_id = w.account_id AND m.aweme_id = w.aweme_id
            WHERE (?2 IS NULL OR w.account_id = ?2) AND w.last_seen_at >= ?3 AND m.enabled = 1 AND w.{column} IS NOT NULL
        )
        WHERE cur IS NOT NULL
          AND (CASE WHEN last_alert_val IS NOT NULL AND raw_base IS NOT NULL THEN MAX(raw_base, last_alert_val) ELSE raw_base END) IS NOT NULL
          AND cur - (CASE WHEN last_alert_val IS NOT NULL AND raw_base IS NOT NULL THEN MAX(raw_base, last_alert_val) ELSE raw_base END) >= ?4
        ORDER BY (cur - (CASE WHEN last_alert_val IS NOT NULL AND raw_base IS NOT NULL THEN MAX(raw_base, last_alert_val) ELSE raw_base END)) DESC
        LIMIT 200"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![baseline_ts, rule.account_id, min_seen, rule.threshold, rule.id, rule.metric],
        |row| {
            let is_approximate = row.get::<_, i64>(7)? != 0;
            let mut windows = [MetricWindow::default(); 5];
            for (index, window) in windows.iter_mut().enumerate() {
                *window = MetricWindow {
                    current: row.get(8 + index * 2)?,
                    baseline: row.get(9 + index * 2)?,
                    is_approximate,
                };
            }
            Ok(AlertCandidate {
                rule_id: rule.id,
                account_id: row.get(1)?,
                work_id: row.get(0)?,
                metric: rule.metric.clone(),
                window_minutes: rule.window_minutes,
                baseline_value: row.get(3)?,
                current_value: row.get(2)?,
                delta: row.get::<_, i64>(2)? - row.get::<_, i64>(3)?,
                owner_name: row.get(4)?,
                owner_open_id: row.get(5)?,
                author_name: row.get(6)?,
                is_approximate,
                windows,
            })
        },
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// 窗口周期内是否已有同一规则 + 作品 + 指标的报警（以规则自身的窗口时间作为通知防重周期）。
pub fn cooldown_active(conn: &Connection, dedup_key: &str, since: i64) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM alerts WHERE dedup_key = ?1 AND created_at > ?2",
        params![dedup_key, since],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[allow(clippy::too_many_arguments)]
pub fn insert_alert(
    conn: &Connection,
    candidate: &AlertCandidate,
    dedup_key: &str,
    message: &str,
    now: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO alerts(rule_id, account_id, work_id, metric, window_minutes, baseline_value,
            current_value, delta, message, notify_state, dedup_key, created_at, owner_name, owner_open_id, author_name)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10, ?11, ?12, ?13, ?14)",
        params![
            candidate.rule_id,
            candidate.account_id,
            candidate.work_id,
            candidate.metric,
            candidate.window_minutes,
            candidate.baseline_value,
            candidate.current_value,
            candidate.delta,
            message,
            dedup_key,
            now,
            candidate.owner_name,
            candidate.owner_open_id,
            candidate.author_name
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn mark_notify(conn: &Connection, alert_id: i64, state: &str, detail: &str) -> Result<()> {
    conn.execute(
        "UPDATE alerts SET notify_state = ?2, notify_detail = ?3 WHERE id = ?1",
        params![alert_id, state, detail],
    )?;
    Ok(())
}

const ALERT_SELECT: &str = "SELECT al.id, al.rule_id, COALESCE(r.name, '已删除规则'), al.account_id,
        COALESCE(acc.name, ''), al.work_id, COALESCE(w.title, ''), COALESCE(w.url, ''), al.metric,
        al.window_minutes, al.baseline_value, al.current_value, al.delta, al.message,
        al.notify_state, al.notify_detail, al.created_at, al.owner_name, al.owner_open_id,
        COALESCE(NULLIF(al.author_name, ''), NULLIF(w.author_name, ''), '')
     FROM alerts al
     LEFT JOIN rules r ON r.id = al.rule_id
     LEFT JOIN accounts acc ON acc.id = al.account_id
     LEFT JOIN works w ON w.id = al.work_id";

fn map_alert(row: &rusqlite::Row<'_>) -> rusqlite::Result<Alert> {
    Ok(Alert {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        rule_name: row.get(2)?,
        account_id: row.get(3)?,
        account_name: row.get(4)?,
        work_id: row.get(5)?,
        work_title: row.get(6)?,
        work_url: row.get(7)?,
        metric: row.get(8)?,
        window_minutes: row.get(9)?,
        baseline_value: row.get(10)?,
        current_value: row.get(11)?,
        delta: row.get(12)?,
        message: row.get(13)?,
        notify_state: row.get(14)?,
        notify_detail: row.get(15)?,
        created_at: row.get(16)?,
        owner_name: row.get(17)?,
        owner_open_id: row.get(18)?,
        author_name: row.get(19)?,
    })
}

pub fn list_alerts(conn: &Connection, limit: i64) -> Result<Vec<Alert>> {
    let sql = format!("{ALERT_SELECT} ORDER BY al.id DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], map_alert)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_alert(conn: &Connection, id: i64) -> Result<Option<Alert>> {
    let sql = format!("{ALERT_SELECT} WHERE al.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    Ok(stmt.query_row(params![id], map_alert).optional()?)
}

pub fn count_alerts_since(conn: &Connection, since: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM alerts WHERE created_at >= ?1",
        params![since],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn clear_alerts(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM alerts", [])?;
    Ok(())
}

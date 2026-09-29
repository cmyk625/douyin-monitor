//! 监控名单：只有名单中的视频会被采集、参与规则评估，并在报警时 @ 对应负责人。

use crate::models::{Metrics, MonitoredInput, MonitoredWork};
use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};

const MONITORED_SELECT: &str = "SELECT m.id, m.account_id, COALESCE(a.name, ''), m.aweme_id, m.title, m.url,
        m.owner_name, m.owner_open_id, m.enabled, m.created_at, m.last_collect_at, m.last_error,
        w.id, w.last_seen_at, w.play, w.like_cnt, w.comment_cnt, w.share_cnt, w.collect_cnt,
        COALESCE(NULLIF(w.author_name, ''), NULLIF(m.author_name, ''), '') AS author_name,
        w.published_at
     FROM monitored_works m
     LEFT JOIN accounts a ON a.id = m.account_id
     LEFT JOIN works w ON w.account_id = m.account_id AND w.aweme_id = m.aweme_id";

fn map_monitored(row: &rusqlite::Row<'_>) -> rusqlite::Result<MonitoredWork> {
    Ok(MonitoredWork {
        id: row.get(0)?,
        account_id: row.get(1)?,
        account_name: row.get(2)?,
        aweme_id: row.get(3)?,
        title: row.get(4)?,
        author_name: row.get(19)?,
        url: row.get(5)?,
        owner_name: row.get(6)?,
        owner_open_id: row.get(7)?,
        enabled: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        last_collect_at: row.get(10)?,
        last_error: row.get(11)?,
        work_id: row.get(12)?,
        published_at: row.get(20)?,
        last_seen_at: row.get(13)?,
        metrics: Metrics {
            play: row.get(14)?,
            like: row.get(15)?,
            comment: row.get(16)?,
            share: row.get(17)?,
            collect: row.get(18)?,
        },
    })
}

pub fn list_monitored(conn: &Connection) -> Result<Vec<MonitoredWork>> {
    let sql = format!("{MONITORED_SELECT} ORDER BY m.enabled DESC, m.id DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_monitored)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_monitored(conn: &Connection, id: i64) -> Result<Option<MonitoredWork>> {
    let sql = format!("{MONITORED_SELECT} WHERE m.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    Ok(stmt.query_row(params![id], map_monitored).optional()?)
}

/// 从「视频链接或作品 ID」中解析作品 ID。
pub fn extract_aweme_id(target: &str) -> Option<String> {
    let text = target.trim();
    if text.is_empty() {
        return None;
    }
    if text.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(text.to_string());
    }
    for key in ["/video/", "/note/", "item_id=", "modal_id=", "vid="] {
        if let Some(rest) = text.split(key).nth(1) {
            let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return Some(digits);
            }
        }
    }
    None
}

/// 从文本中提取可能包含的分享短链接 URL（如 https://v.douyin.com/xxxx/）
pub fn extract_short_url(target: &str) -> Option<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"https?://v\.douyin\.com/[a-zA-Z0-9_\-]+/?").unwrap());
    re.find(target).map(|m| m.as_str().to_string())
}

/// 从文本中提取可能包含的抖音作品长链接（如 https://www.douyin.com/video/xxxx 或 note）
pub fn extract_long_url(target: &str) -> Option<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"https?://(?:www\.|creator\.)?douyin\.com/(?:video|note)/\d+").unwrap()
    });
    re.find(target).map(|m| m.as_str().to_string())
}

/// 请求短链接跟随 302 重定向，获取最终地址并提取出作品 ID。
pub async fn resolve_short_url(client: &reqwest::Client, short_url: &str) -> Result<(String, String)> {
    let res = client
        .get(short_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        )
        .send()
        .await?;
    let final_url = res.url().to_string();
    if let Some(id) = extract_aweme_id(&final_url) {
        let canonical_url = format!("https://www.douyin.com/video/{id}");
        return Ok((id, canonical_url));
    }
    bail!("无法从短链接跳转地址中识别作品 ID：{final_url}");
}

/// 统一解析用户输入的 target（长链接、短链接、App分享文案、纯数字作品 ID）。
pub async fn resolve_target(client: &reqwest::Client, target: &str) -> Result<(String, String)> {
    let trimmed = target.trim();
    // 1. 若文本中包含短链（含各种 App 分享杂质文本），优先提取并跟随 302 重定向
    if let Some(short_url) = extract_short_url(trimmed) {
        return resolve_short_url(client, &short_url).await;
    }
    // 2. 若文本中包含标准网页长链
    if let Some(long_url) = extract_long_url(trimmed) {
        if let Some(id) = extract_aweme_id(&long_url) {
            let canonical_url = format!("https://www.douyin.com/video/{id}");
            return Ok((id, canonical_url));
        }
    }
    // 3. 若直接是纯数字作品 ID 或含 /video/ 的格式
    if let Some(id) = extract_aweme_id(trimmed) {
        let canonical_url = format!("https://www.douyin.com/video/{id}");
        return Ok((id, canonical_url));
    }
    bail!("无法识别作品链接或作品 ID，请确认输入的是长链接、分享短链接或作品 ID");
}

pub fn save_monitored(
    conn: &Connection,
    input: &MonitoredInput,
    resolved: Option<(String, String)>,
    now: i64,
) -> Result<i64> {
    // 负责人是使用者自己填的名字：去掉多余空白（含换行），避免破坏报警消息排版。
    let owner_name = input
        .owner_name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let account_exists: bool = conn
        .query_row(
            "SELECT 1 FROM accounts WHERE id = ?1",
            params![input.account_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !account_exists {
        bail!("账号不存在，请先选择账号");
    }

    let (aweme_id, url) = match resolved {
        Some((id, canonical_url)) => (id, canonical_url),
        None => {
            let id = extract_aweme_id(&input.target)
                .ok_or_else(|| anyhow::anyhow!("无法识别视频 ID，请粘贴作品链接或作品 ID"))?;
            let url = format!("https://www.douyin.com/video/{id}");
            (id, url)
        }
    };

    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE monitored_works SET account_id = ?2, aweme_id = ?3, url = ?4,
                    owner_name = ?5, owner_open_id = ?6, enabled = ?7 WHERE id = ?1",
                params![
                    id,
                    input.account_id,
                    aweme_id,
                    url,
                    owner_name,
                    "",
                    input.enabled as i64
                ],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO monitored_works(account_id, aweme_id, url, owner_name, owner_open_id,
                    enabled, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(account_id, aweme_id) DO UPDATE SET
                    url = CASE WHEN excluded.url = '' THEN monitored_works.url ELSE excluded.url END,
                    owner_name = excluded.owner_name,
                    owner_open_id = excluded.owner_open_id,
                    enabled = excluded.enabled",
                params![
                    input.account_id,
                    aweme_id,
                    url,
                    owner_name,
                    "",
                    input.enabled as i64,
                    now
                ],
            )?;
            let id: i64 = conn.query_row(
                "SELECT id FROM monitored_works WHERE account_id = ?1 AND aweme_id = ?2",
                params![input.account_id, aweme_id],
                |row| row.get(0),
            )?;
            Ok(id)
        }
    }
}

pub fn delete_monitored(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM monitored_works WHERE id = ?1", params![id])?;
    Ok(())
}

/// 账号下启用的监控视频 ID 列表。
pub fn enabled_aweme_ids(conn: &Connection, account_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT aweme_id FROM monitored_works WHERE account_id = ?1 AND enabled = 1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![account_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// 存在启用监控视频的账号 ID 列表。
pub fn accounts_with_monitors(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT DISTINCT account_id FROM monitored_works WHERE enabled = 1")?;
    let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn count_enabled(conn: &Connection) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM monitored_works WHERE enabled = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// 采集命中：补全标题/作者/链接，记录采集时间并清除旧错误。
pub fn mark_seen(
    conn: &Connection,
    account_id: i64,
    aweme_id: &str,
    title: &str,
    author_name: &str,
    url: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE monitored_works SET
            title = CASE WHEN ?3 = '' THEN title ELSE ?3 END,
            author_name = CASE WHEN ?4 = '' THEN author_name ELSE ?4 END,
            url = CASE WHEN ?5 = '' THEN url ELSE ?5 END,
            last_collect_at = ?6, last_error = ''
         WHERE account_id = ?1 AND aweme_id = ?2",
        params![account_id, aweme_id, title, author_name, url, now],
    )?;
    Ok(())
}

/// 本次采集未在页面上找到该视频。
pub fn mark_missing(conn: &Connection, account_id: i64, aweme_id: &str, message: &str) -> Result<()> {
    conn.execute(
        "UPDATE monitored_works SET last_error = ?3 WHERE account_id = ?1 AND aweme_id = ?2",
        params![account_id, aweme_id, message],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aweme_id_from_links() {
        assert_eq!(
            extract_aweme_id("https://www.douyin.com/video/7412345678901234567").as_deref(),
            Some("7412345678901234567")
        );
        assert_eq!(
            extract_aweme_id("https://www.douyin.com/note/7412345678901234567?x=1").as_deref(),
            Some("7412345678901234567")
        );
        assert_eq!(
            extract_aweme_id("https://creator.douyin.com/creator-micro/content/manage?item_id=7412").as_deref(),
            Some("7412")
        );
        assert_eq!(extract_aweme_id("  7412345678901234567 ").as_deref(), Some("7412345678901234567"));
        assert_eq!(extract_aweme_id("https://v.douyin.com/abcdef/"), None);
        assert_eq!(extract_aweme_id(""), None);
    }

    #[test]
    fn extracts_short_url_correctly() {
        assert_eq!(
            extract_short_url("https://v.douyin.com/abcdef12/").as_deref(),
            Some("https://v.douyin.com/abcdef12/")
        );
        assert_eq!(
            extract_short_url("7.89 复制打开抖音，看看【xxx的作品】 https://v.douyin.com/iABC123/ 05/12 a@b.c :0pm").as_deref(),
            Some("https://v.douyin.com/iABC123/")
        );
        assert_eq!(
            extract_short_url("0.58 :2pm uSL:/ 05/27 k@P.Kw 每天体验一种人生： 辞职开水果店的真实人生# 青年创作者计划  https://v.douyin.com/gnU2yTg_HFs/ 复制此链接，打开Dou音搜索，直接观看视频！").as_deref(),
            Some("https://v.douyin.com/gnU2yTg_HFs/")
        );
        assert_eq!(extract_short_url("7412345678901234567"), None);
    }

    #[test]
    fn extracts_long_url_correctly() {
        assert_eq!(
            extract_long_url("来看看这个搞笑视频 https://www.douyin.com/video/7412345678901234567 真的很赞").as_deref(),
            Some("https://www.douyin.com/video/7412345678901234567")
        );
        assert_eq!(
            extract_long_url("https://creator.douyin.com/video/7412345678901234567").as_deref(),
            Some("https://creator.douyin.com/video/7412345678901234567")
        );
    }

    #[test]
    fn requires_owner_and_valid_target() {
        let conn = crate::db::open_in_memory().unwrap();
        let input = crate::models::AccountInput {
            id: None,
            name: "账号".into(),
            sec_uid: String::new(),
            target_url: String::new(),
            parse_script: String::new(),
            enabled: true,
            note: String::new(),
        };
        crate::repo::create_account(&conn, &input, "profile", 0).unwrap();

        let mut monitored = MonitoredInput {
            id: None,
            account_id: 1,
            target: "https://www.douyin.com/video/7412345678901234567".into(),
            owner_name: "  关孝雅  ".into(),
            enabled: true,
        };
        monitored.target = "https://v.douyin.com/abcdef/".into();
        assert!(save_monitored(&conn, &monitored, None, 0).is_err(), "未提供解析结果且无法识别 ID 应报错");

        // 提供解析后的短链接结果进行保存
        let short_resolved = Some(("7412345678901234567".to_string(), "https://www.douyin.com/video/7412345678901234567".to_string()));
        let id_from_short = save_monitored(&conn, &monitored, short_resolved, 0).unwrap();
        assert_eq!(enabled_aweme_ids(&conn, 1).unwrap(), vec!["7412345678901234567"]);

        monitored.target = "7412345678901234567".into();
        let id = save_monitored(&conn, &monitored, None, 0).unwrap();
        assert_eq!(id, id_from_short);
        assert_eq!(enabled_aweme_ids(&conn, 1).unwrap(), vec!["7412345678901234567"]);
        assert_eq!(accounts_with_monitors(&conn).unwrap(), vec![1]);
        assert_eq!(count_enabled(&conn).unwrap(), 1);

        // 同一视频重复添加不应产生第二条记录
        let again = save_monitored(&conn, &monitored, None, 0).unwrap();
        assert_eq!(again, id);
        assert_eq!(list_monitored(&conn).unwrap().len(), 1);

        let listed = get_monitored(&conn, id).unwrap().unwrap();
        assert_eq!(listed.owner_name, "关孝雅", "负责人名字会去掉首尾空白");
        assert_eq!(listed.owner_open_id, "", "不再使用飞书 Open ID");
        assert_eq!(listed.title, "", "标题由采集自动学习，不再手工填写");
        assert!(listed.work_id.is_none());

        // 名字中间的换行/连续空格会被收敛成单个空格，避免破坏报警排版
        let mut second = monitored.clone();
        second.target = "7412345678901234568".into();
        second.owner_name = "张\n三".into();
        let second_id = save_monitored(&conn, &second, None, 0).unwrap();
        assert_eq!(get_monitored(&conn, second_id).unwrap().unwrap().owner_name, "张 三");

        delete_monitored(&conn, second_id).unwrap();
        delete_monitored(&conn, id).unwrap();
        assert!(enabled_aweme_ids(&conn, 1).unwrap().is_empty());
    }

    #[test]
    fn test_delete_monitored_preserves_works_history() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO accounts(name, created_at) VALUES('test', 0)", []).unwrap();

        let m = MonitoredInput {
            id: None,
            account_id: 1,
            target: "7412345678901234567".into(),
            owner_name: "张三".into(),
            enabled: true,
        };
        let m_id = save_monitored(&conn, &m, None, 0).unwrap();

        // 模拟采集写入 works 表和快照
        conn.execute(
            "INSERT INTO works(account_id, aweme_id, title, url, first_seen_at, last_seen_at) VALUES(1, '7412345678901234567', '测试', '', 0, 100)",
            [],
        ).unwrap();
        let work_id: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO snapshots(work_id, account_id, ts, like_cnt) VALUES(?1, 1, 100, 50)",
            rusqlite::params![work_id],
        ).unwrap();

        // 移除监控
        delete_monitored(&conn, m_id).unwrap();
        assert_eq!(list_monitored(&conn).unwrap().len(), 0);

        // 验证 works 和 snapshots 仍完整保留（历史作品不丢失，可在存储清理时统一淘汰或在历史作品中查看）
        let work_count: i64 = conn.query_row("SELECT COUNT(*) FROM works WHERE id = ?1", rusqlite::params![work_id], |r| r.get(0)).unwrap();
        let snap_count: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots WHERE work_id = ?1", rusqlite::params![work_id], |r| r.get(0)).unwrap();
        assert_eq!(work_count, 1);
        assert_eq!(snap_count, 1);
    }
}

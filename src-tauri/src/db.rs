use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 4;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("创建数据目录失败：{}", parent.display()))?;
    }
    let conn = Connection::open(path).with_context(|| format!("打开数据库失败：{}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "cache_size", "-20000")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL,
                sec_uid         TEXT NOT NULL DEFAULT '',
                target_url      TEXT NOT NULL DEFAULT '',
                parse_script    TEXT NOT NULL DEFAULT '',
                profile_dir     TEXT NOT NULL DEFAULT '',
                login_state     TEXT NOT NULL DEFAULT 'unknown',
                enabled         INTEGER NOT NULL DEFAULT 1,
                note            TEXT NOT NULL DEFAULT '',
                interval_secs   INTEGER NOT NULL DEFAULT 600,
                created_at      INTEGER NOT NULL,
                last_collect_at INTEGER,
                last_ok_at      INTEGER,
                last_error      TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS works (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                aweme_id      TEXT NOT NULL,
                title         TEXT NOT NULL DEFAULT '',
                url           TEXT NOT NULL DEFAULT '',
                published_at  INTEGER,
                first_seen_at INTEGER NOT NULL,
                last_seen_at  INTEGER NOT NULL,
                play          INTEGER,
                like_cnt      INTEGER,
                comment_cnt   INTEGER,
                share_cnt     INTEGER,
                collect_cnt   INTEGER,
                UNIQUE(account_id, aweme_id)
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                work_id     INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
                account_id  INTEGER NOT NULL,
                ts          INTEGER NOT NULL,
                play        INTEGER,
                like_cnt    INTEGER,
                comment_cnt INTEGER,
                share_cnt   INTEGER,
                collect_cnt INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_work_ts ON snapshots(work_id, ts);

            CREATE TABLE IF NOT EXISTS rules (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                name             TEXT NOT NULL,
                account_id       INTEGER,
                metric           TEXT NOT NULL,
                window_minutes   INTEGER NOT NULL,
                threshold        INTEGER NOT NULL,
                cooldown_minutes INTEGER NOT NULL DEFAULT 120,
                enabled          INTEGER NOT NULL DEFAULT 1,
                created_at       INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS alerts (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id        INTEGER NOT NULL,
                account_id     INTEGER NOT NULL,
                work_id        INTEGER NOT NULL,
                metric         TEXT NOT NULL,
                window_minutes INTEGER NOT NULL,
                baseline_value INTEGER NOT NULL,
                current_value  INTEGER NOT NULL,
                delta          INTEGER NOT NULL,
                message        TEXT NOT NULL DEFAULT '',
                notify_state   TEXT NOT NULL DEFAULT 'pending',
                notify_detail  TEXT NOT NULL DEFAULT '',
                dedup_key      TEXT NOT NULL DEFAULT '',
                created_at     INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_alerts_dedup ON alerts(dedup_key, created_at);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            PRAGMA user_version = 1;
            "#,
        )
        .context("初始化数据库结构失败")?;
    }
    if version < 2 {
        // v2：只监控「配置过的视频」，并记录每条视频的负责人（用于飞书 @）。
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS monitored_works (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                aweme_id        TEXT NOT NULL,
                title           TEXT NOT NULL DEFAULT '',
                url             TEXT NOT NULL DEFAULT '',
                owner_name      TEXT NOT NULL DEFAULT '',
                owner_open_id   TEXT NOT NULL DEFAULT '',
                enabled         INTEGER NOT NULL DEFAULT 1,
                created_at      INTEGER NOT NULL,
                last_collect_at INTEGER,
                last_error      TEXT NOT NULL DEFAULT '',
                UNIQUE(account_id, aweme_id)
            );
            CREATE INDEX IF NOT EXISTS idx_monitored_account ON monitored_works(account_id, enabled);
            "#,
        )
        .context("创建监控名单表失败")?;
        add_column_if_missing(conn, "alerts", "owner_name", "TEXT NOT NULL DEFAULT ''")?;
        add_column_if_missing(conn, "alerts", "owner_open_id", "TEXT NOT NULL DEFAULT ''")?;
        conn.execute_batch("PRAGMA user_version = 2;")
            .context("更新数据库版本失败")?;
    }
    if version < 3 {
        // v3：补充真实视频作者字段与近似值标记
        add_column_if_missing(conn, "works", "author_name", "TEXT NOT NULL DEFAULT ''")?;
        add_column_if_missing(conn, "works", "is_approximate", "INTEGER NOT NULL DEFAULT 0")?;
        add_column_if_missing(conn, "monitored_works", "author_name", "TEXT NOT NULL DEFAULT ''")?;
        add_column_if_missing(conn, "alerts", "author_name", "TEXT NOT NULL DEFAULT ''")?;
        add_column_if_missing(conn, "snapshots", "is_approximate", "INTEGER NOT NULL DEFAULT 0")?;
        conn.execute_batch("PRAGMA user_version = 3;")
            .context("更新数据库版本至 3 失败")?;
    }
    if version < 4 {
        // v4：快照时间戳索引，加速按时间范围查询与覆盖清理
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snapshots_ts ON snapshots(ts);
            PRAGMA user_version = 4;
            "#,
        )
        .context("更新数据库版本至 4 失败")?;
    }
    Ok(())
}

/// 幂等加列：迁移中途失败再次运行时不会因为「列已存在」而报错。
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }
    conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])?;
    Ok(())
}

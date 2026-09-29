use crate::models::{Account, AccountInput, AccountRename};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

const ACCOUNT_COLUMNS: &str = "a.id, a.name, a.sec_uid, a.target_url, a.parse_script, a.profile_dir,
     a.login_state, a.enabled, a.note, a.interval_secs, a.created_at,
     a.last_collect_at, a.last_ok_at, a.last_error,
     (SELECT COUNT(*) FROM works w WHERE w.account_id = a.id) AS work_count";

fn map_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        sec_uid: row.get(2)?,
        target_url: row.get(3)?,
        parse_script: row.get(4)?,
        profile_dir: row.get(5)?,
        login_state: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        note: row.get(8)?,
        interval_secs: row.get(9)?,
        created_at: row.get(10)?,
        last_collect_at: row.get(11)?,
        last_ok_at: row.get(12)?,
        last_error: row.get(13)?,
        work_count: row.get(14)?,
    })
}

pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM accounts a ORDER BY a.id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_account)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_account(conn: &Connection, id: i64) -> Result<Option<Account>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM accounts a WHERE a.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let account = stmt.query_row(params![id], map_account).optional()?;
    Ok(account)
}

pub fn create_account(
    conn: &Connection,
    input: &AccountInput,
    profile_dir: &str,
    now: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO accounts(name, sec_uid, target_url, parse_script, profile_dir, enabled, note, interval_secs, created_at, login_state)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'unknown')",
        params![
            input.name.trim(),
            input.sec_uid.trim(),
            input.target_url.trim(),
            input.parse_script,
            profile_dir,
            input.enabled as i64,
            input.note.trim(),
            crate::collect::COLLECT_INTERVAL_SECS,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_account(conn: &Connection, input: &AccountRename) -> Result<()> {
    let id = input.id;
    if input.name.trim().is_empty() {
        anyhow::bail!("账号名称不能为空");
    }
    conn.execute(
        "UPDATE accounts SET name = ?2 WHERE id = ?1",
        params![id, input.name.trim()],
    )?;
    Ok(())
}

pub fn delete_account(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM snapshots WHERE account_id = ?1", params![id])?;
    conn.execute("DELETE FROM works WHERE account_id = ?1", params![id])?;
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_profile_dir(conn: &Connection, id: i64, profile_dir: &str) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET profile_dir = ?2 WHERE id = ?1",
        params![id, profile_dir],
    )?;
    Ok(())
}

pub fn update_login_state(conn: &Connection, id: i64, state: &str, error: &str) -> Result<()> {
    // 登录成功时不要把「已登录创作者中心（…）」写进最近错误列
    let detail = if state == "ok" { "" } else { error };
    conn.execute(
        "UPDATE accounts SET login_state = ?2, last_error = ?3 WHERE id = ?1",
        params![id, state, detail],
    )?;
    Ok(())
}

pub fn mark_collected(conn: &Connection, id: i64, now: i64, ok: bool, error: &str) -> Result<()> {
    if ok {
        conn.execute(
            "UPDATE accounts SET last_collect_at = ?2, last_ok_at = ?2, last_error = '' WHERE id = ?1",
            params![id, now],
        )?;
    } else {
        conn.execute(
            "UPDATE accounts SET last_collect_at = ?2, last_error = ?3 WHERE id = ?1",
            params![id, now, error],
        )?;
    }
    Ok(())
}

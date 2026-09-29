use crate::models::{CleanStorageResult, Metrics, RawWork, SnapshotPoint, StorageStats, Work};
use crate::settings::Settings;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct WorkChange {
    pub work_id: i64,
    pub is_new: bool,
    pub changed: bool,
}

/// 插入或更新作品；返回本次是否首次见到、指标是否有增长。
/// 核心原则：趋势记录与作品指标优先使用接口数据（is_approximate == false）。
pub fn upsert_work(conn: &Connection, account_id: i64, raw: &RawWork, now: i64) -> Result<WorkChange> {
    let existing: Option<(i64, Metrics, bool)> = conn
        .query_row(
            "SELECT id, play, like_cnt, comment_cnt, share_cnt, collect_cnt, is_approximate FROM works
             WHERE account_id = ?1 AND aweme_id = ?2",
            params![account_id, raw.aweme_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    Metrics {
                        play: row.get(1)?,
                        like: row.get(2)?,
                        comment: row.get(3)?,
                        share: row.get(4)?,
                        collect: row.get(5)?,
                    },
                    row.get::<_, i64>(6)? != 0,
                ))
            },
        )
        .optional()?;

    match existing {
        Some((work_id, current, existing_is_approx)) => {
            let incoming_exact = !raw.is_approximate;

            if incoming_exact {
                // 1. 本次为接口精准数据（is_approximate == false）：拥有最高优先级！
                let growth = raw.metrics.has_growth_over(&current);
                // 如果历史数据是近似值（DOM 抓取），或者指标有真实增长，则视为变动并记录快照
                let changed = existing_is_approx || growth;

                conn.execute(
                    "UPDATE works SET title = CASE WHEN ?2 = '' THEN title ELSE ?2 END,
                        author_name = CASE WHEN ?3 = '' THEN author_name ELSE ?3 END,
                        url = CASE WHEN ?4 = '' THEN url ELSE ?4 END,
                        published_at = COALESCE(published_at, ?5),
                        last_seen_at = ?6,
                        play = ?7, like_cnt = ?8, comment_cnt = ?9, share_cnt = ?10, collect_cnt = ?11,
                        is_approximate = 0
                     WHERE id = ?1",
                    params![
                        work_id,
                        raw.title,
                        raw.author_name,
                        raw.url,
                        raw.published_at,
                        now,
                        raw.metrics.play,
                        raw.metrics.like,
                        raw.metrics.comment,
                        raw.metrics.share,
                        raw.metrics.collect,
                    ],
                )?;

                if existing_is_approx {
                    // 校准此前所有因 DOM 四舍五入估算而高过本次真实接口值的近似快照
                    correct_approximate_snapshots(conn, work_id, &raw.metrics)?;
                }

                if changed {
                    insert_snapshot(conn, work_id, account_id, now, &raw.metrics, false)?;
                }

                Ok(WorkChange { work_id, is_new: false, changed })
            } else {
                // 2. 本次为 DOM 近似数据（is_approximate == true）：
                if !existing_is_approx {
                    // 库内已有接口精准数据，决不允许被 DOM 粗略近似（如 5.1万 => 51000）覆盖污染！
                    conn.execute(
                        "UPDATE works SET
                            title = CASE WHEN title = '' AND ?2 != '' THEN ?2 ELSE title END,
                            author_name = CASE WHEN author_name = '' AND ?3 != '' THEN ?3 ELSE author_name END,
                            url = CASE WHEN url = '' AND ?4 != '' THEN ?4 ELSE url END,
                            published_at = COALESCE(published_at, ?5),
                            last_seen_at = ?6
                         WHERE id = ?1",
                        params![work_id, raw.title, raw.author_name, raw.url, raw.published_at, now],
                    )?;
                    Ok(WorkChange { work_id, is_new: false, changed: false })
                } else {
                    // 库内也是近似数据：按常规增长逻辑更新
                    let changed = raw.metrics.has_growth_over(&current);
                    let current = raw.metrics.clone();
                    conn.execute(
                        "UPDATE works SET title = CASE WHEN ?2 = '' THEN title ELSE ?2 END,
                            author_name = CASE WHEN ?3 = '' THEN author_name ELSE ?3 END,
                            url = CASE WHEN ?4 = '' THEN url ELSE ?4 END,
                            published_at = COALESCE(published_at, ?5),
                            last_seen_at = ?6,
                            play = ?7, like_cnt = ?8, comment_cnt = ?9, share_cnt = ?10, collect_cnt = ?11,
                            is_approximate = 1
                         WHERE id = ?1",
                        params![
                            work_id,
                            raw.title,
                            raw.author_name,
                            raw.url,
                            raw.published_at,
                            now,
                            current.play,
                            current.like,
                            current.comment,
                            current.share,
                            current.collect,
                        ],
                    )?;
                    if changed {
                        insert_snapshot(conn, work_id, account_id, now, &current, true)?;
                    }
                    Ok(WorkChange { work_id, is_new: false, changed })
                }
            }
        }
        None => {
            conn.execute(
                "INSERT INTO works(account_id, aweme_id, title, author_name, url, published_at, first_seen_at, last_seen_at,
                    play, like_cnt, comment_cnt, share_cnt, collect_cnt, is_approximate)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    account_id,
                    raw.aweme_id,
                    raw.title,
                    raw.author_name,
                    raw.url,
                    raw.published_at,
                    now,
                    raw.metrics.play,
                    raw.metrics.like,
                    raw.metrics.comment,
                    raw.metrics.share,
                    raw.metrics.collect,
                    raw.is_approximate as i64
                ],
            )?;
            let work_id = conn.last_insert_rowid();
            insert_snapshot(conn, work_id, account_id, now, &raw.metrics, raw.is_approximate)?;
            Ok(WorkChange { work_id, is_new: true, changed: true })
        }
    }
}

fn correct_approximate_snapshots(conn: &Connection, work_id: i64, exact: &Metrics) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM snapshots WHERE work_id = ?1",
        params![work_id],
        |row| row.get(0),
    )?;
    if count == 1 {
        conn.execute(
            "UPDATE snapshots SET play = ?2, like_cnt = ?3, comment_cnt = ?4, share_cnt = ?5, collect_cnt = ?6, is_approximate = 0
             WHERE work_id = ?1 AND is_approximate = 1",
            params![work_id, exact.play, exact.like, exact.comment, exact.share, exact.collect],
        )?;
        return Ok(());
    }

    if let Some(like) = exact.like {
        conn.execute(
            "UPDATE snapshots SET like_cnt = ?2 WHERE work_id = ?1 AND is_approximate = 1 AND like_cnt > ?2",
            params![work_id, like],
        )?;
    }
    if let Some(comment) = exact.comment {
        conn.execute(
            "UPDATE snapshots SET comment_cnt = ?2 WHERE work_id = ?1 AND is_approximate = 1 AND comment_cnt > ?2",
            params![work_id, comment],
        )?;
    }
    if let Some(share) = exact.share {
        conn.execute(
            "UPDATE snapshots SET share_cnt = ?2 WHERE work_id = ?1 AND is_approximate = 1 AND share_cnt > ?2",
            params![work_id, share],
        )?;
    }
    if let Some(collect) = exact.collect {
        conn.execute(
            "UPDATE snapshots SET collect_cnt = ?2 WHERE work_id = ?1 AND is_approximate = 1 AND collect_cnt > ?2",
            params![work_id, collect],
        )?;
    }
    if let Some(play) = exact.play {
        conn.execute(
            "UPDATE snapshots SET play = ?2 WHERE work_id = ?1 AND is_approximate = 1 AND play > ?2",
            params![work_id, play],
        )?;
    }
    Ok(())
}

pub fn insert_snapshot(
    conn: &Connection,
    work_id: i64,
    account_id: i64,
    ts: i64,
    metrics: &Metrics,
    is_approximate: bool,
) -> Result<i64> {
    // 覆盖去重：如果已有相同 (work_id, ts) 的快照，则覆盖更新，避免多余冗余点
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM snapshots WHERE work_id = ?1 AND ts = ?2",
            params![work_id, ts],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(id) = existing {
        conn.execute(
            "UPDATE snapshots SET play = ?2, like_cnt = ?3, comment_cnt = ?4, share_cnt = ?5, collect_cnt = ?6, is_approximate = ?7
             WHERE id = ?1",
            params![
                id,
                metrics.play,
                metrics.like,
                metrics.comment,
                metrics.share,
                metrics.collect,
                is_approximate as i64
            ],
        )?;
        return Ok(id);
    }

    conn.execute(
        "INSERT INTO snapshots(work_id, account_id, ts, play, like_cnt, comment_cnt, share_cnt, collect_cnt, is_approximate)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            work_id,
            account_id,
            ts,
            metrics.play,
            metrics.like,
            metrics.comment,
            metrics.share,
            metrics.collect,
            is_approximate as i64
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_works(
    conn: &Connection,
    account_id: Option<i64>,
    keyword: &str,
    limit: i64,
) -> Result<Vec<Work>> {
    list_work_page(conn, account_id, keyword, limit, false, 0)
}

pub fn list_work_page(conn: &Connection, account_id: Option<i64>, keyword: &str, limit: i64, history: bool, offset: i64) -> Result<Vec<Work>> {
    let keyword = keyword.trim();
    let pattern = format!("%{keyword}%");
    let mut stmt = conn.prepare(
        "SELECT w.id, w.account_id, COALESCE(a.name, ''), w.aweme_id, w.title, w.url, w.published_at,
                w.first_seen_at, w.last_seen_at, w.play, w.like_cnt, w.comment_cnt, w.share_cnt, w.collect_cnt,
                COALESCE(w.author_name, ''), w.is_approximate
         FROM works w LEFT JOIN accounts a ON a.id = w.account_id
         WHERE (?1 IS NULL OR w.account_id = ?1)
            AND (?2 = '' OR w.title LIKE ?3 OR w.aweme_id LIKE ?3)
            AND (?5 = 0 OR NOT EXISTS (SELECT 1 FROM monitored_works m WHERE m.account_id = w.account_id AND m.aweme_id = w.aweme_id))
         ORDER BY w.last_seen_at DESC, w.id DESC
          LIMIT ?4 OFFSET ?6",
    )?;
    let rows = stmt.query_map(params![account_id, keyword, pattern, limit, history, offset], |row| {
        Ok(Work {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            aweme_id: row.get(3)?,
            title: row.get(4)?,
            url: row.get(5)?,
            published_at: row.get(6)?,
            first_seen_at: row.get(7)?,
            last_seen_at: row.get(8)?,
            metrics: Metrics {
                play: row.get(9)?,
                like: row.get(10)?,
                comment: row.get(11)?,
                share: row.get(12)?,
                collect: row.get(13)?,
            },
            author_name: row.get(14)?,
            is_approximate: row.get::<_, i64>(15)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn work_trend(conn: &Connection, work_id: i64, limit: i64) -> Result<Vec<SnapshotPoint>> {
    let mut stmt = conn.prepare(
        "SELECT ts, play, like_cnt, comment_cnt, share_cnt, collect_cnt, is_approximate FROM snapshots
         WHERE work_id = ?1 ORDER BY ts DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![work_id, limit], |row| {
        Ok(SnapshotPoint {
            ts: row.get(0)?,
            play: row.get(1)?,
            like: row.get(2)?,
            comment: row.get(3)?,
            share: row.get(4)?,
            collect: row.get(5)?,
            is_approximate: row.get::<_, i64>(6)? != 0,
        })
    })?;
    let mut points = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    points.reverse();
    Ok(points)
}

/// 取作品标题、链接、发布时间与真实作者（拼装消息用）。
pub fn work_brief(conn: &Connection, work_id: i64) -> Result<(String, String, Option<i64>, String)> {
    let mut stmt = conn.prepare("SELECT title, url, published_at, COALESCE(author_name, '') FROM works WHERE id = ?1")?;
    let brief = stmt
        .query_row(params![work_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .optional()?;
    Ok(brief.unwrap_or_default())
}

pub fn delete_work(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM snapshots WHERE work_id = ?1", params![id])?;
    conn.execute("DELETE FROM works WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn totals(conn: &Connection) -> Result<(i64, i64)> {
    let works: i64 = conn.query_row("SELECT COUNT(*) FROM works", [], |row| row.get(0))?;
    let snapshots: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    Ok((works, snapshots))
}

/// 淘汰指定时间戳之前的超期快照（自动滚动覆盖清理）
pub fn clean_expired_snapshots(conn: &Connection, cutoff_ts: i64) -> Result<usize> {
    let deleted = conn.execute("DELETE FROM snapshots WHERE ts < ?1", params![cutoff_ts])?;
    Ok(deleted)
}

/// 清理未在监控名单中的孤立作品及其关联快照（级联删除）
pub fn clean_orphaned_works(conn: &Connection) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM works WHERE NOT EXISTS (
            SELECT 1 FROM monitored_works m
            WHERE m.account_id = works.account_id AND m.aweme_id = works.aweme_id
        )",
        [],
    )?;
    Ok(deleted)
}

/// 获取当前本地存储统计（数据库体积、快照总量、最早记录时间等）
pub fn get_storage_stats(conn: &Connection, db_path: &Path, settings: &Settings) -> Result<StorageStats> {
    let db_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let snapshots_count: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    let works_count: i64 = conn.query_row("SELECT COUNT(*) FROM works", [], |row| row.get(0))?;
    let monitored_count: i64 = conn.query_row("SELECT COUNT(*) FROM monitored_works", [], |row| row.get(0))?;
    let alerts_count: i64 = conn.query_row("SELECT COUNT(*) FROM alerts", [], |row| row.get(0))?;
    let oldest_snapshot_ts: Option<i64> = conn
        .query_row("SELECT MIN(ts) FROM snapshots", [], |row| row.get(0))
        .optional()?
        .flatten();

    Ok(StorageStats {
        db_size_bytes,
        snapshots_count,
        works_count,
        monitored_count,
        alerts_count,
        oldest_snapshot_ts,
        retention_days: settings.retention_days,
        auto_clean: settings.auto_clean,
    })
}

/// 执行覆盖清理与 SQLite 空间回收 (VACUUM)
pub fn clean_storage(conn: &Connection, db_path: &Path, retention_days: i64) -> Result<CleanStorageResult> {
    let db_size_before = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let now = crate::util::now_ts();
    let cutoff_ts = now - retention_days * 86400;

    let deleted_snapshots = clean_expired_snapshots(conn, cutoff_ts)?;
    let deleted_works = clean_orphaned_works(conn)?;

    // 运行 VACUUM 整理碎片并回收已删除数据占用的磁盘空间
    conn.execute_batch("VACUUM;")?;

    let db_size_after = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let freed_bytes = db_size_before.saturating_sub(db_size_after);

    Ok(CleanStorageResult {
        deleted_snapshots,
        deleted_works,
        db_size_before,
        db_size_after,
        freed_bytes,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn history_filters_before_pagination() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO accounts(name, created_at) VALUES('test', 0)", []).unwrap();
        for i in 0..450 {
            conn.execute("INSERT INTO works(account_id, aweme_id, first_seen_at, last_seen_at) VALUES(1, ?1, 0, ?2)", rusqlite::params![i.to_string(), i]).unwrap();
            if i >= 250 {
                conn.execute("INSERT INTO monitored_works(account_id, aweme_id, created_at) VALUES(1, ?1, 0)", [i.to_string()]).unwrap();
            }
        }
        let first = super::list_work_page(&conn, None, "", 200, true, 0).unwrap();
        let second = super::list_work_page(&conn, None, "", 200, true, 200).unwrap();
        assert_eq!(first.len(), 200);
        assert_eq!(second.len(), 50);
        assert_eq!(first[0].aweme_id, "249");
        assert_eq!(second[49].aweme_id, "0");
        assert!(!first.iter().any(|a| second.iter().any(|b| a.id == b.id)));
    }

    #[test]
    fn interface_data_prioritized_over_approximate_data() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO accounts(name, created_at) VALUES('test', 0)", []).unwrap();

        // 1. 首次采集：从 DOM 解析到近似值 "5.1万" => 51000
        let dom_raw = crate::models::RawWork {
            aweme_id: "test12345".into(),
            title: "测试视频".into(),
            metrics: crate::models::Metrics {
                play: None,
                like: Some(51000),
                comment: Some(450),
                share: Some(21800),
                collect: Some(9200),
            },
            is_approximate: true,
            ..Default::default()
        };
        let c1 = super::upsert_work(&conn, 1, &dom_raw, 1000).unwrap();
        assert!(c1.is_new);
        assert!(c1.changed);

        let w1 = super::list_works(&conn, Some(1), "test12345", 10).unwrap();
        assert_eq!(w1.len(), 1);
        assert!(w1[0].is_approximate);
        assert_eq!(w1[0].metrics.like, Some(51000));

        // 2. 接口采集成功：获取到真实精准数值 50894
        // 50894 小于近似值 51000，但由于是官方接口数据，必须无条件优先入库并校准历史快照
        let api_raw = crate::models::RawWork {
            aweme_id: "test12345".into(),
            title: "测试视频".into(),
            metrics: crate::models::Metrics {
                play: None,
                like: Some(50894),
                comment: Some(448),
                share: Some(21757),
                collect: Some(9218),
            },
            is_approximate: false,
            ..Default::default()
        };
        let c2 = super::upsert_work(&conn, 1, &api_raw, 1600).unwrap();
        assert!(!c2.is_new);
        assert!(c2.changed);

        let w2 = super::list_works(&conn, Some(1), "test12345", 10).unwrap();
        assert_eq!(w2.len(), 1);
        assert!(!w2[0].is_approximate);
        assert_eq!(w2[0].metrics.like, Some(50894));
        assert_eq!(w2[0].metrics.comment, Some(448));
        assert_eq!(w2[0].metrics.share, Some(21757));
        assert_eq!(w2[0].metrics.collect, Some(9218));

        // 3. 检查趋势点：单快照已被校准为接口真实值，新增快照也是接口真实值，杜绝虚假跌落
        let trend = super::work_trend(&conn, w2[0].id, 10).unwrap();
        assert_eq!(trend.last().unwrap().like, Some(50894));
        assert!(!trend.last().unwrap().is_approximate);

        // 4. 后续如果 DOM 粗略抓取再次到来（5.1万 => 51000），绝不允许污染已有的接口数据
        let dom_repeat = crate::models::RawWork {
            aweme_id: "test12345".into(),
            title: "测试视频".into(),
            metrics: crate::models::Metrics {
                play: None,
                like: Some(51000),
                comment: Some(450),
                share: Some(21800),
                collect: Some(9200),
            },
            is_approximate: true,
            ..Default::default()
        };
        let c3 = super::upsert_work(&conn, 1, &dom_repeat, 2000).unwrap();
        assert!(!c3.changed);

        let w3 = super::list_works(&conn, Some(1), "test12345", 10).unwrap();
        assert!(!w3[0].is_approximate);
        assert_eq!(w3[0].metrics.like, Some(50894));
    }

    #[test]
    fn test_storage_cleanup_and_deduplication() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute("INSERT INTO accounts(name, created_at) VALUES('test', 0)", []).unwrap();
        conn.execute("INSERT INTO monitored_works(account_id, aweme_id, created_at) VALUES(1, 'monitored1', 0)", []).unwrap();

        // 插入属于监控名单的作品
        conn.execute("INSERT INTO works(account_id, aweme_id, first_seen_at, last_seen_at) VALUES(1, 'monitored1', 0, 1000)", []).unwrap();
        let work_id: i64 = conn.last_insert_rowid();

        // 插入不属于监控名单的孤立作品
        conn.execute("INSERT INTO works(account_id, aweme_id, first_seen_at, last_seen_at) VALUES(1, 'orphan1', 0, 1000)", []).unwrap();

        // 插入多条快照（包括超期快照与同时间戳快照）
        let m = crate::models::Metrics {
            play: None,
            like: Some(100),
            comment: Some(10),
            share: Some(5),
            collect: Some(2),
        };
        // 8 天前的超期快照 (ts = 1000)
        super::insert_snapshot(&conn, work_id, 1, 1000, &m, false).unwrap();
        // 2 天前的有效快照 (ts = 10000)
        super::insert_snapshot(&conn, work_id, 1, 10000, &m, false).unwrap();
        // 同时间戳覆盖测试 (ts = 10000)
        let m_updated = crate::models::Metrics {
            play: None,
            like: Some(150),
            comment: Some(15),
            share: Some(8),
            collect: Some(4),
        };
        super::insert_snapshot(&conn, work_id, 1, 10000, &m_updated, false).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots WHERE work_id = ?1", rusqlite::params![work_id], |r| r.get(0)).unwrap();
        // 同时间戳覆盖更新，总快照应为 2 条而非 3 条
        assert_eq!(count, 2);

        // 验证覆盖后的最新值
        let latest_like: i64 = conn.query_row("SELECT like_cnt FROM snapshots WHERE work_id = ?1 AND ts = 10000", rusqlite::params![work_id], |r| r.get(0)).unwrap();
        assert_eq!(latest_like, 150);

        // 测试清理 7 天前（以 ts = 10000 为基准，cutoff = 5000）
        let deleted = super::clean_expired_snapshots(&conn, 5000).unwrap();
        assert_eq!(deleted, 1);

        let remaining: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots WHERE work_id = ?1", rusqlite::params![work_id], |r| r.get(0)).unwrap();
        assert_eq!(remaining, 1);

        // 测试清理孤立作品
        let deleted_orphans = super::clean_orphaned_works(&conn).unwrap();
        assert_eq!(deleted_orphans, 1);
        let works_left: i64 = conn.query_row("SELECT COUNT(*) FROM works", [], |r| r.get(0)).unwrap();
        assert_eq!(works_left, 1);
    }
}

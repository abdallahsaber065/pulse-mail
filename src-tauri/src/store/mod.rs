//! Local SQLite cache. The UI reads only from here, so opening the window
//! never waits on the network.
//!
//! The store is synchronous (rusqlite); callers keep lock scopes short and
//! never hold the connection across an `.await`.

mod migrations;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::Path;

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

/// A fully parsed message ready to be cached.
#[derive(Debug, Clone, Default)]
pub struct NewMessage {
    pub id: String,
    pub thread_id: String,
    pub internal_date: i64,
    pub from_name: String,
    pub from_email: String,
    pub to_list: String,
    pub subject: String,
    pub snippet: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub label_ids: Vec<String>,
    pub has_attachments: bool,
    pub otp_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub history_id: Option<String>,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub unread: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRow {
    pub thread_id: String,
    pub last_message_id: String,
    pub subject: String,
    pub from_name: String,
    pub from_email: String,
    pub snippet: String,
    pub internal_date: i64,
    pub message_count: i64,
    pub unread: bool,
    pub starred: bool,
    pub otp_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub thread_id: String,
    pub internal_date: i64,
    pub from_name: String,
    pub from_email: String,
    pub to_list: String,
    pub subject: String,
    pub snippet: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub label_ids: Vec<String>,
    pub unread: bool,
    pub starred: bool,
    pub has_attachments: bool,
    pub otp_code: Option<String>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -2000;", // ~2 MB page cache: keep the daemon small
        )?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        let current: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in migrations::MIGRATIONS.iter().enumerate() {
            let version = i as i64 + 1;
            if version <= current {
                continue;
            }
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))
    }

    // ---- accounts -------------------------------------------------------

    pub fn upsert_account(&self, email: &str, now_ms: i64) -> Result<Account> {
        self.conn.execute(
            "INSERT INTO accounts(email, created_at) VALUES (?1, ?2)
             ON CONFLICT(email) DO NOTHING",
            params![email, now_ms],
        )?;
        self.account_by_email(email).map(|a| a.expect("account just inserted"))
    }

    pub fn account_by_email(&self, email: &str) -> Result<Option<Account>> {
        self.conn
            .query_row(
                "SELECT id, email, history_id, last_sync_at FROM accounts WHERE email = ?1",
                [email],
                map_account,
            )
            .optional()
    }

    /// Phase 1 is single-account: the first account is the active one.
    pub fn primary_account(&self) -> Result<Option<Account>> {
        self.conn
            .query_row(
                "SELECT id, email, history_id, last_sync_at FROM accounts ORDER BY id LIMIT 1",
                [],
                map_account,
            )
            .optional()
    }

    pub fn delete_account(&self, account_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
        Ok(())
    }

    pub fn set_history_id(&self, account_id: i64, history_id: &str, now_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET history_id = ?2, last_sync_at = ?3 WHERE id = ?1",
            params![account_id, history_id, now_ms],
        )?;
        Ok(())
    }

    pub fn clear_history_id(&self, account_id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE accounts SET history_id = NULL WHERE id = ?1", [account_id])?;
        Ok(())
    }

    // ---- labels ---------------------------------------------------------

    pub fn replace_labels(&mut self, account_id: i64, labels: &[LabelRow]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM labels WHERE account_id = ?1", [account_id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO labels(account_id, id, name, kind, unread, total) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for l in labels {
                stmt.execute(params![account_id, l.id, l.name, l.kind, l.unread, l.total])?;
            }
        }
        tx.commit()
    }

    /// Labels with unread counts computed from the local cache (always fresh,
    /// unlike Gmail's counters which are only refreshed on a labels fetch).
    pub fn list_labels(&self, account_id: i64) -> Result<Vec<LabelRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.id, l.name, l.kind,
                    (SELECT COUNT(*) FROM message_labels ml JOIN messages m
                       ON m.account_id = ml.account_id AND m.id = ml.message_id
                     WHERE ml.account_id = l.account_id AND ml.label_id = l.id AND m.is_unread = 1) AS unread,
                    l.total
             FROM labels l WHERE l.account_id = ?1
             ORDER BY l.kind DESC, l.name COLLATE NOCASE",
        )?;
        stmt.query_map([account_id], |r| {
            Ok(LabelRow {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                unread: r.get(3)?,
                total: r.get(4)?,
            })
        })?
        .collect()
    }

    // ---- messages -------------------------------------------------------

    pub fn upsert_message(&mut self, account_id: i64, m: &NewMessage) -> Result<()> {
        let tx = self.conn.transaction()?;
        let unread = m.label_ids.iter().any(|l| l == "UNREAD");
        let starred = m.label_ids.iter().any(|l| l == "STARRED");
        tx.execute(
            "INSERT INTO messages(account_id, id, thread_id, internal_date, from_name, from_email, to_list,
                                  subject, snippet, body_text, body_html, is_unread, is_starred,
                                  has_attachments, otp_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(account_id, id) DO UPDATE SET
                thread_id = excluded.thread_id, internal_date = excluded.internal_date,
                from_name = excluded.from_name, from_email = excluded.from_email, to_list = excluded.to_list,
                subject = excluded.subject, snippet = excluded.snippet, body_text = excluded.body_text,
                body_html = excluded.body_html, is_unread = excluded.is_unread, is_starred = excluded.is_starred,
                has_attachments = excluded.has_attachments, otp_code = excluded.otp_code",
            params![
                account_id,
                m.id,
                m.thread_id,
                m.internal_date,
                m.from_name,
                m.from_email,
                m.to_list,
                m.subject,
                m.snippet,
                m.body_text,
                m.body_html,
                unread,
                starred,
                m.has_attachments,
                m.otp_code
            ],
        )?;
        write_labels(&tx, account_id, &m.id, &m.label_ids)?;
        tx.commit()
    }

    /// Apply a label change from history sync without refetching the body.
    pub fn set_message_labels(&mut self, account_id: i64, message_id: &str, label_ids: &[String]) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let unread = label_ids.iter().any(|l| l == "UNREAD");
        let starred = label_ids.iter().any(|l| l == "STARRED");
        let changed = tx.execute(
            "UPDATE messages SET is_unread = ?3, is_starred = ?4 WHERE account_id = ?1 AND id = ?2",
            params![account_id, message_id, unread, starred],
        )?;
        if changed > 0 {
            write_labels(&tx, account_id, message_id, label_ids)?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn has_message(&self, account_id: i64, message_id: &str) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT 1 FROM messages WHERE account_id = ?1 AND id = ?2",
                params![account_id, message_id],
                |_| Ok(()),
            )
            .optional()
            .map(|o| o.is_some())
    }

    pub fn delete_message(&self, account_id: i64, message_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE account_id = ?1 AND id = ?2",
            params![account_id, message_id],
        )?;
        Ok(())
    }

    pub fn list_threads(&self, account_id: i64, label_id: &str, limit: i64, offset: i64) -> Result<Vec<ThreadRow>> {
        let mut stmt = self.conn.prepare_cached(
            "WITH lm AS (
                SELECT m.*,
                       ROW_NUMBER() OVER (PARTITION BY m.thread_id ORDER BY m.internal_date DESC) AS rn,
                       COUNT(*)     OVER (PARTITION BY m.thread_id) AS cnt,
                       MAX(m.is_unread)  OVER (PARTITION BY m.thread_id) AS any_unread,
                       MAX(m.is_starred) OVER (PARTITION BY m.thread_id) AS any_starred
                FROM messages m
                JOIN message_labels ml ON ml.account_id = m.account_id AND ml.message_id = m.id
                WHERE m.account_id = ?1 AND ml.label_id = ?2
             )
             SELECT thread_id, id, subject, from_name, from_email, snippet, internal_date,
                    cnt, any_unread, any_starred, otp_code
             FROM lm WHERE rn = 1
             ORDER BY internal_date DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        stmt.query_map(params![account_id, label_id, limit, offset], map_thread)?
            .collect()
    }

    /// Full-text search over cached mail (FTS5 query syntax, sanitized).
    pub fn search(&self, account_id: i64, query: &str, limit: i64) -> Result<Vec<ThreadRow>> {
        let q = fts_query(query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.thread_id, m.id, m.subject, m.from_name, m.from_email, m.snippet, m.internal_date,
                    1, m.is_unread, m.is_starred, m.otp_code
             FROM messages_fts f JOIN messages m ON m.rowid = f.rowid
             WHERE messages_fts MATCH ?2 AND m.account_id = ?1
             ORDER BY m.internal_date DESC
             LIMIT ?3",
        )?;
        stmt.query_map(params![account_id, q, limit], map_thread)?.collect()
    }

    pub fn get_thread(&self, account_id: i64, thread_id: &str) -> Result<Vec<MessageRow>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, thread_id, internal_date, from_name, from_email, to_list, subject, snippet,
                    body_text, body_html, is_unread, is_starred, has_attachments, otp_code,
                    (SELECT group_concat(label_id, ' ') FROM message_labels ml
                      WHERE ml.account_id = m.account_id AND ml.message_id = m.id)
             FROM messages m WHERE account_id = ?1 AND thread_id = ?2
             ORDER BY internal_date ASC",
        )?;
        stmt.query_map(params![account_id, thread_id], |r| {
            let labels: Option<String> = r.get(14)?;
            Ok(MessageRow {
                id: r.get(0)?,
                thread_id: r.get(1)?,
                internal_date: r.get(2)?,
                from_name: r.get(3)?,
                from_email: r.get(4)?,
                to_list: r.get(5)?,
                subject: r.get(6)?,
                snippet: r.get(7)?,
                body_text: r.get(8)?,
                body_html: r.get(9)?,
                unread: r.get(10)?,
                starred: r.get(11)?,
                has_attachments: r.get(12)?,
                otp_code: r.get(13)?,
                label_ids: labels
                    .map(|s| s.split(' ').map(str::to_owned).collect())
                    .unwrap_or_default(),
            })
        })?
        .collect()
    }

    pub fn thread_id_of(&self, account_id: i64, message_id: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT thread_id FROM messages WHERE account_id = ?1 AND id = ?2",
                params![account_id, message_id],
                |r| r.get(0),
            )
            .optional()
    }

    // ---- settings -------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

fn write_labels(tx: &rusqlite::Transaction<'_>, account_id: i64, message_id: &str, label_ids: &[String]) -> Result<()> {
    tx.execute(
        "DELETE FROM message_labels WHERE account_id = ?1 AND message_id = ?2",
        params![account_id, message_id],
    )?;
    let mut stmt = tx
        .prepare_cached("INSERT OR IGNORE INTO message_labels(account_id, label_id, message_id) VALUES (?1, ?2, ?3)")?;
    for l in label_ids {
        stmt.execute(params![account_id, l, message_id])?;
    }
    Ok(())
}

fn map_account(r: &rusqlite::Row<'_>) -> Result<Account> {
    Ok(Account {
        id: r.get(0)?,
        email: r.get(1)?,
        history_id: r.get(2)?,
        last_sync_at: r.get(3)?,
    })
}

fn map_thread(r: &rusqlite::Row<'_>) -> Result<ThreadRow> {
    Ok(ThreadRow {
        thread_id: r.get(0)?,
        last_message_id: r.get(1)?,
        subject: r.get(2)?,
        from_name: r.get(3)?,
        from_email: r.get(4)?,
        snippet: r.get(5)?,
        internal_date: r.get(6)?,
        message_count: r.get(7)?,
        unread: r.get(8)?,
        starred: r.get(9)?,
        otp_code: r.get(10)?,
    })
}

/// Turn free text into a safe FTS5 prefix query: every word quoted, last one
/// as a prefix, so user input can never inject FTS operators.
fn fts_query(input: &str) -> String {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| !w.is_empty())
        .take(12)
        .collect();
    let n = words.len();
    words
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            if i + 1 == n {
                format!("\"{w}\"*")
            } else {
                format!("\"{w}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, thread: &str, date: i64, labels: &[&str], subject: &str) -> NewMessage {
        NewMessage {
            id: id.into(),
            thread_id: thread.into(),
            internal_date: date,
            from_name: "Alice".into(),
            from_email: "alice@example.com".into(),
            subject: subject.into(),
            snippet: format!("snippet of {subject}"),
            body_text: format!("body of {subject}"),
            label_ids: labels.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn setup() -> (Store, i64) {
        let s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("me@example.com", 1).unwrap();
        (s, a.id)
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let v1 = Store::open(&path).unwrap().schema_version().unwrap();
        let v2 = Store::open(&path).unwrap().schema_version().unwrap();
        assert_eq!(v1, migrations::MIGRATIONS.len() as i64);
        assert_eq!(v1, v2);
    }

    #[test]
    fn account_upsert_and_history() {
        let (s, id) = setup();
        let again = s.upsert_account("me@example.com", 2).unwrap();
        assert_eq!(again.id, id);
        s.set_history_id(id, "12345", 10).unwrap();
        let a = s.primary_account().unwrap().unwrap();
        assert_eq!(a.history_id.as_deref(), Some("12345"));
        s.clear_history_id(id).unwrap();
        assert_eq!(s.primary_account().unwrap().unwrap().history_id, None);
    }

    #[test]
    fn threads_group_latest_message_and_unread() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "Hello"))
            .unwrap();
        s.upsert_message(id, &msg("m2", "t1", 200, &["INBOX", "UNREAD"], "Re: Hello"))
            .unwrap();
        s.upsert_message(id, &msg("m3", "t2", 150, &["INBOX", "STARRED"], "Other"))
            .unwrap();
        s.upsert_message(id, &msg("m4", "t3", 300, &["SENT"], "Sent one"))
            .unwrap();

        let threads = s.list_threads(id, "INBOX", 50, 0).unwrap();
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].thread_id, "t1");
        assert_eq!(threads[0].last_message_id, "m2");
        assert_eq!(threads[0].message_count, 2);
        assert!(threads[0].unread);
        assert!(threads[1].starred);

        let thread = s.get_thread(id, "t1").unwrap();
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[0].id, "m1");
        assert!(thread[1].label_ids.contains(&"UNREAD".to_string()));
    }

    #[test]
    fn label_changes_and_delete() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX", "UNREAD"], "Hi"))
            .unwrap();
        assert!(s.set_message_labels(id, "m1", &["INBOX".into()]).unwrap());
        assert!(!s.list_threads(id, "INBOX", 10, 0).unwrap()[0].unread);
        assert!(!s.set_message_labels(id, "missing", &["INBOX".into()]).unwrap());

        s.delete_message(id, "m1").unwrap();
        assert!(!s.has_message(id, "m1").unwrap());
        assert!(s.list_threads(id, "INBOX", 10, 0).unwrap().is_empty());
    }

    #[test]
    fn full_text_search_with_prefix_and_injection_safety() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "Quarterly invoice"))
            .unwrap();
        s.upsert_message(id, &msg("m2", "t2", 200, &["INBOX"], "Lunch plans"))
            .unwrap();
        let hits = s.search(id, "invo", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].last_message_id, "m1");
        // FTS operators in user input must not error
        assert!(s.search(id, "\"unbalanced OR NEAR(", 10).is_ok());
        assert!(s.search(id, "   ", 10).unwrap().is_empty());
        // updates keep the index in sync
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "Renamed"))
            .unwrap();
        assert!(s.search(id, "invoice", 10).unwrap().is_empty());
    }

    #[test]
    fn local_unread_counts() {
        let (mut s, id) = setup();
        s.replace_labels(
            id,
            &[LabelRow {
                id: "INBOX".into(),
                name: "INBOX".into(),
                kind: "system".into(),
                unread: 0,
                total: 0,
            }],
        )
        .unwrap();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX", "UNREAD"], "a"))
            .unwrap();
        s.upsert_message(id, &msg("m2", "t2", 100, &["INBOX"], "b")).unwrap();
        assert_eq!(s.list_labels(id).unwrap()[0].unread, 1);
    }

    #[test]
    fn settings_roundtrip() {
        let (s, _) = setup();
        assert_eq!(s.get_setting("close_behavior").unwrap(), None);
        s.set_setting("close_behavior", "hide").unwrap();
        s.set_setting("close_behavior", "destroy").unwrap();
        assert_eq!(s.get_setting("close_behavior").unwrap().as_deref(), Some("destroy"));
    }
}

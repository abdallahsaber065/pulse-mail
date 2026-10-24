//! Local SQLite cache. The UI reads only from here, so opening the window
//! never waits on the network.
//!
//! The store is synchronous (rusqlite); callers keep lock scopes short and
//! never hold the connection across an `.await`.

mod migrations;
pub mod query;
pub mod selection;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactSuggestion {
    pub name: String,
    pub email: String,
    pub frequency: i64,
}

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
    /// OTP detector confidence (0-100) for `otp_code` / `otp_link`.
    pub otp_score: i32,
    /// One-click sign-in / verification link, when no code was found.
    pub otp_link: Option<String>,
    /// "Expires in N minutes" (not stored; used for the toast).
    pub otp_expires_min: Option<u32>,
    pub cc_list: String,
    pub reply_to: String,
    /// `<id@host>` of this message, for replies.
    pub rfc_message_id: String,
    /// Space-separated `<id>` list from the References header.
    pub rfc_references: String,
    /// Raw `List-Unsubscribe` header value (angle-bracket URL list), if any.
    pub list_unsubscribe: Option<String>,
    pub attachments: Vec<NewAttachment>,
    /// Gmail sizeEstimate / raw size in bytes (0 = unknown).
    pub size: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NewAttachment {
    pub part: u32,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub content_id: Option<String>,
    pub inline: bool,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRow {
    pub part: u32,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub inline: bool,
    /// A copy is saved on this PC.
    pub stored: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub history_id: Option<String>,
    pub last_sync_at: Option<i64>,
    /// "30d" | "90d" | "1y" | "all": how much history to download.
    pub sync_window: String,
    /// "all" | "primary" (Primary tab + codes) | "otp" | "off"
    pub notify: String,
    /// "setup" (onboarding not finished) | "pending" | "running" | "paused" | "done"
    pub backfill_state: String,
    #[serde(skip)]
    pub backfill_page: Option<String>,
    pub backfill_done: i64,
    pub backfill_total: i64,
    /// Gmail profile `messagesTotal` at sign-in (for estimates).
    pub messages_total: i64,
    /// History download phase: "" | "count" | "index" | "bodies" | "files".
    pub backfill_phase: String,
    /// Last history-download error shown to the user (None = fine).
    pub backfill_error: Option<String>,
    /// Attachments kept offline: "none" | "limit" | "all".
    pub attach_mode: String,
    /// Per-file limit for "limit", in MB.
    pub attach_max_mb: i64,
    pub display_name: String,
    /// Locally cached Google profile photo (a raster data URL).
    pub avatar_data: Option<String>,
    /// Account color ("" = automatic).
    pub color: String,
    /// Plain-text signature appended to new messages and replies.
    pub signature: String,
    /// Show Gmail's inbox tabs (Primary, Promotions, ...).
    pub category_tabs: bool,
}

/// Per-account settings the UI can change.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSettings {
    pub display_name: Option<String>,
    pub color: Option<String>,
    pub signature: Option<String>,
    pub attach_mode: Option<String>,
    pub attach_max_mb: Option<i64>,
    pub category_tabs: Option<bool>,
}

pub const ATTACH_MODES: &[&str] = &["none", "limit", "all"];
pub const BACKFILL_PHASES: &[&str] = &["", "count", "index", "bodies", "files"];

/// Estimated download sizes for an account's range (bytes).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SizeEstimate {
    /// Messages with a known size.
    pub sized: i64,
    pub total_bytes: i64,
    /// Same, counting messages with attachments as text only.
    pub text_bytes: i64,
}

pub const SYNC_WINDOWS: &[&str] = &["30d", "90d", "1y", "all"];
pub const BACKFILL_STATES: &[&str] = &["setup", "pending", "running", "paused", "done"];

const ACCOUNT_COLS: &str = "id, email, history_id, last_sync_at, sync_window, notify, backfill_state, backfill_page, \
                            backfill_done, backfill_total, messages_total, backfill_phase, backfill_error, \
                            attach_mode, attach_max_mb, COALESCE(NULLIF(display_name, ''), profile_name), \
                            color, signature, category_tabs, NULLIF(avatar_data, '')";

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
    /// Owning account (threads from several accounts mix in the unified inbox).
    pub account_id: i64,
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
    /// Gmail marked a message of the thread IMPORTANT.
    pub important: bool,
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
    pub otp_link: Option<String>,
    pub body_loaded: bool,
    pub cc_list: String,
    pub reply_to: String,
    pub rfc_message_id: String,
    pub rfc_references: String,
    /// Raw `List-Unsubscribe` header (angle-bracket URL list), if any.
    pub list_unsubscribe: Option<String>,
    /// Real attachments (inline `cid:` images are not listed).
    pub attachments: Vec<AttachmentRow>,
}

/// A cached message for the OTP re-scan.
pub struct OtpRescanRow {
    pub rowid: i64,
    pub account_id: i64,
    pub id: String,
    pub subject: String,
    pub from: String,
    /// Body text, or the snippet when the body isn't cached.
    pub text: String,
    pub html: Option<String>,
    pub body_loaded: bool,
    pub promotional: bool,
    pub current_code: Option<String>,
    pub current_link: Option<String>,
    pub current_score: i32,
}

/// A changed OTP result to write back in batch.
pub struct OtpRescanUpdate {
    pub account_id: i64,
    pub id: String,
    pub from: String,
    pub code: Option<String>,
    pub link: Option<String>,
    pub score: i32,
}

/// (from_email, body_html, body_text, body_loaded)
pub type CachedBody = (String, Option<String>, String, bool);

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
             PRAGMA cache_size = -8000;", // ~8 MB page cache: FTS scans on big mailboxes stay warm, still small for a daemon
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
            "INSERT INTO accounts(email, created_at, backfill_state, category_tabs) VALUES (?1, ?2, 'setup', 0)
             ON CONFLICT(email) DO NOTHING",
            params![email, now_ms],
        )?;
        self.account_by_email(email).map(|a| a.expect("account just inserted"))
    }

    pub fn account_by_email(&self, email: &str) -> Result<Option<Account>> {
        self.conn
            .query_row(
                &format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE email = ?1"),
                [email],
                map_account,
            )
            .optional()
    }

    pub fn account(&self, account_id: i64) -> Result<Option<Account>> {
        self.conn
            .query_row(
                &format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE id = ?1"),
                [account_id],
                map_account,
            )
            .optional()
    }

    /// Apply an inferred name only if the user hasn't already chosen one.
    /// The conditional write also protects against a delayed network lookup.
    pub fn set_account_name_if_empty(&self, account_id: i64, name: &str) -> Result<bool> {
        let name: String = name.chars().filter(|c| !c.is_control()).take(80).collect();
        let name = name.trim();
        if name.is_empty() {
            return Ok(false);
        }
        self.conn
            .execute(
                "UPDATE accounts SET profile_name = ?2 WHERE id = ?1 AND display_name = '' AND profile_name = ''",
                params![account_id, name],
            )
            .map(|n| n != 0)
    }

    /// Prefer the most frequent From name on this account's own sent mail.
    /// Never infer identity from incoming mail, aliases, or another account.
    pub fn fill_account_name_from_sent(&self, account_id: i64) -> Result<bool> {
        let Some(account) = self.account(account_id)?.filter(|a| a.display_name.is_empty()) else {
            return Ok(false);
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT trim(m.from_name) AS name
             FROM message_labels ml
             JOIN messages m ON m.account_id = ml.account_id AND m.id = ml.message_id
             WHERE ml.account_id = ?1 AND ml.label_id = 'SENT'
               AND m.from_email = ?2 COLLATE NOCASE
               AND trim(m.from_name) != '' AND instr(m.from_name, '@') = 0
             GROUP BY trim(m.from_name)
             ORDER BY count(*) DESC, max(m.internal_date) DESC, name ASC",
        )?;
        let names = stmt.query_map(params![account_id, account.email], |r| r.get::<_, String>(0))?;
        for name in names {
            if self.set_account_name_if_empty(account_id, &name?)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Google can refresh the default name/photo; the user override is separate.
    pub fn set_google_profile(&self, account_id: i64, name: &str, avatar: Option<&str>) -> Result<bool> {
        let name: String = name.chars().filter(|c| !c.is_control()).take(80).collect();
        self.conn
            .execute(
                "UPDATE accounts
             SET profile_name = CASE WHEN ?2 = '' THEN profile_name ELSE ?2 END,
                 avatar_data = COALESCE(?3, avatar_data)
             WHERE id = ?1",
                params![account_id, name.trim(), avatar],
            )
            .map(|n| n != 0)
    }

    /// All accounts, oldest first (the first is the default account).
    pub fn accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!("SELECT {ACCOUNT_COLS} FROM accounts ORDER BY id"))?;
        stmt.query_map([], map_account)?.collect()
    }

    /// The default account (the oldest), used when a caller names none.
    pub fn primary_account(&self) -> Result<Option<Account>> {
        self.conn
            .query_row(
                &format!("SELECT {ACCOUNT_COLS} FROM accounts ORDER BY id LIMIT 1"),
                [],
                map_account,
            )
            .optional()
    }

    pub fn set_account_window(&self, account_id: i64, window: &str) -> Result<()> {
        debug_assert!(SYNC_WINDOWS.contains(&window));
        // A new window restarts the history download from the newest page;
        // messages already on this PC are skipped quickly.
        self.conn.execute(
            "UPDATE accounts SET sync_window = ?2, backfill_page = NULL, backfill_done = 0, backfill_total = 0,
                    backfill_phase = '', backfill_error = NULL,
                    backfill_state = CASE WHEN backfill_state = 'setup' THEN 'setup' ELSE 'pending' END
             WHERE id = ?1",
            params![account_id, window],
        )?;
        self.conn
            .execute("DELETE FROM hist_queue WHERE account_id = ?1", [account_id])?;
        Ok(())
    }

    pub fn set_backfill_phase(&self, account_id: i64, phase: &str) -> Result<()> {
        debug_assert!(BACKFILL_PHASES.contains(&phase));
        self.conn.execute(
            "UPDATE accounts SET backfill_phase = ?2 WHERE id = ?1",
            params![account_id, phase],
        )?;
        Ok(())
    }

    pub fn set_backfill_error(&self, account_id: i64, error: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET backfill_error = ?2 WHERE id = ?1",
            params![account_id, error],
        )?;
        Ok(())
    }

    /// Apply the fields that are set; values are validated by the caller.
    pub fn set_account_settings(&self, account_id: i64, s: &AccountSettings) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (col, v) in [
            ("display_name", s.display_name.as_deref()),
            ("color", s.color.as_deref()),
            ("signature", s.signature.as_deref()),
            ("attach_mode", s.attach_mode.as_deref()),
        ] {
            if let Some(v) = v {
                // column names come from the fixed list above
                tx.execute(
                    &format!("UPDATE accounts SET {col} = ?2 WHERE id = ?1"),
                    params![account_id, v],
                )?;
            }
        }
        if let Some(mb) = s.attach_max_mb {
            tx.execute(
                "UPDATE accounts SET attach_max_mb = ?2 WHERE id = ?1",
                params![account_id, mb],
            )?;
        }
        if let Some(on) = s.category_tabs {
            tx.execute(
                "UPDATE accounts SET category_tabs = ?2 WHERE id = ?1",
                params![account_id, on],
            )?;
        }
        tx.commit()
    }

    /// Queue ids (from the counting pass) that aren't cached yet.
    pub fn queue_missing(&mut self, account_id: i64, ids: &[String]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut n = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO hist_queue(account_id, id)
                 SELECT ?1, ?2 WHERE NOT EXISTS (SELECT 1 FROM messages WHERE account_id = ?1 AND id = ?2)",
            )?;
            for id in ids {
                n += stmt.execute(params![account_id, id])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    pub fn queue_take(&self, account_id: i64, limit: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id FROM hist_queue WHERE account_id = ?1 LIMIT ?2")?;
        stmt.query_map(params![account_id, limit], |r| r.get(0))?.collect()
    }

    pub fn queue_remove(&mut self, account_id: i64, ids: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached("DELETE FROM hist_queue WHERE account_id = ?1 AND id = ?2")?;
            for id in ids {
                stmt.execute(params![account_id, id])?;
            }
        }
        tx.commit()
    }

    pub fn queue_len(&self, account_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM hist_queue WHERE account_id = ?1",
            [account_id],
            |r| r.get(0),
        )
    }

    /// Sizes of the messages newer than `since_ms` (0 = all) on this PC.
    pub fn size_estimate(&self, account_id: i64, since_ms: i64) -> Result<SizeEstimate> {
        self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0),
                    COALESCE(SUM(CASE WHEN has_attachments THEN MIN(size, 80000) ELSE size END), 0)
             FROM messages WHERE account_id = ?1 AND internal_date >= ?2 AND size > 0",
            params![account_id, since_ms],
            |r| {
                Ok(SizeEstimate {
                    sized: r.get(0)?,
                    total_bytes: r.get(1)?,
                    text_bytes: r.get(2)?,
                })
            },
        )
    }

    /// Offline-copy candidates: no body yet, newer than `since_ms`, at most
    /// `max_size` bytes (0 = any size; unknown sizes always qualify).
    pub fn body_candidates(&self, account_id: i64, since_ms: i64, max_size: i64, limit: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id FROM messages WHERE account_id = ?1 AND body_loaded = 0 AND internal_date >= ?2
               AND (?3 = 0 OR size <= ?3)
             ORDER BY internal_date DESC LIMIT ?4",
        )?;
        stmt.query_map(params![account_id, since_ms, max_size, limit], |r| r.get(0))?
            .collect()
    }

    /// Messages whose body is cached but some attachment allowed by the
    /// policy (`max_file` bytes, 0 = any) isn't saved yet.
    pub fn file_candidates(&self, account_id: i64, since_ms: i64, max_file: i64, limit: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT m.id FROM messages m JOIN attachments a ON a.account_id = m.account_id AND a.message_id = m.id
             WHERE m.account_id = ?1 AND m.body_loaded = 1 AND m.internal_date >= ?2
               AND a.inline = 0 AND a.stored = 0 AND (?3 = 0 OR a.size <= ?3)
             ORDER BY m.internal_date DESC LIMIT ?4",
        )?;
        stmt.query_map(params![account_id, since_ms, max_file, limit], |r| r.get(0))?
            .collect()
    }

    pub fn mark_stored(&self, account_id: i64, message_id: &str, part: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE attachments SET stored = 1 WHERE account_id = ?1 AND message_id = ?2 AND part = ?3",
            params![account_id, message_id, part],
        )?;
        Ok(())
    }

    pub fn attachment_stored(&self, account_id: i64, message_id: &str, part: u32) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT stored FROM attachments WHERE account_id = ?1 AND message_id = ?2 AND part = ?3",
                params![account_id, message_id, part],
                |r| r.get(0),
            )
            .optional()
            .map(|v| v.unwrap_or(false))
    }

    /// Bytes of attachments saved on disk (all accounts, or one).
    pub fn stored_attachment_bytes(&self, account_id: Option<i64>) -> Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(size), 0) FROM attachments WHERE stored = 1 AND (?1 IS NULL OR account_id = ?1)",
            [account_id],
            |r| r.get(0),
        )
    }

    /// Rough bytes this account uses in the database (bodies + metadata).
    pub fn account_db_bytes(&self, account_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(body_text) + COALESCE(LENGTH(body_html), 0) + LENGTH(snippet) + 600), 0)
             FROM messages WHERE account_id = ?1",
            [account_id],
            |r| r.get(0),
        )
    }

    pub fn set_account_notify(&self, account_id: i64, mode: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET notify = ?2 WHERE id = ?1",
            params![account_id, mode],
        )?;
        Ok(())
    }

    pub fn set_messages_total(&self, account_id: i64, total: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET messages_total = ?2 WHERE id = ?1",
            params![account_id, total],
        )?;
        Ok(())
    }

    pub fn set_backfill_state(&self, account_id: i64, state: &str) -> Result<()> {
        debug_assert!(BACKFILL_STATES.contains(&state));
        self.conn.execute(
            "UPDATE accounts SET backfill_state = ?2 WHERE id = ?1",
            params![account_id, state],
        )?;
        Ok(())
    }

    /// Persist the history-download cursor after each page so it resumes
    /// after a restart.
    pub fn set_backfill_progress(&self, account_id: i64, page: Option<&str>, done: i64, total: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET backfill_page = ?2, backfill_done = ?3, backfill_total = ?4 WHERE id = ?1",
            params![account_id, page, done, total],
        )?;
        Ok(())
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

    /// Cheap maintenance after a write-heavy sync pass: refresh SQLite's
    /// query-plan statistics and merge pending FTS5 segments (ROADMAP H3).
    /// Both are no-ops (or near) when there is nothing to consolidate.
    pub fn optimize(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA optimize;
             INSERT INTO messages_fts(messages_fts) VALUES ('merge');",
        )?;
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

    /// Inbox tabs for an account (None = all): (category, unread threads, threads).
    pub fn category_counts(&self, account_id: Option<i64>) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "WITH inbox AS (
                SELECT m.account_id, m.thread_id, m.is_unread, {cat} AS cat
                FROM messages m
                JOIN message_labels ml ON ml.account_id = m.account_id AND ml.message_id = m.id AND ml.label_id = 'INBOX'
                WHERE (?1 IS NULL OR m.account_id = ?1)
             )
             SELECT cat, COUNT(DISTINCT CASE WHEN is_unread = 1 THEN account_id || ':' || thread_id END),
                    COUNT(DISTINCT account_id || ':' || thread_id)
             FROM inbox GROUP BY cat",
            cat = CATEGORY_EXPR
        ))?;
        stmt.query_map([account_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect()
    }

    /// Learned adjustment for codes from this sender (+ copied, - "not a code").
    pub fn otp_bias(&self, account_id: i64, sender: &str) -> Result<i32> {
        otp_bias(&self.conn, account_id, sender)
    }

    /// Record feedback on a detected code. `useful = false` also clears the
    /// code / link of `message_id`.
    pub fn otp_feedback(&self, account_id: i64, sender: &str, useful: bool, message_id: Option<&str>) -> Result<()> {
        let delta = if useful { 10 } else { -30 };
        self.conn.execute(
            "INSERT INTO otp_feedback(account_id, sender, score) VALUES (?1, lower(?2), ?3)
             ON CONFLICT(account_id, sender) DO UPDATE SET score = MAX(-90, MIN(20, score + ?3))",
            params![account_id, sender, delta],
        )?;
        if !useful && let Some(id) = message_id {
            self.conn.execute(
                "UPDATE messages SET otp_code = NULL, otp_link = NULL, otp_score = 0 WHERE account_id = ?1 AND id = ?2",
                params![account_id, id],
            )?;
        }
        Ok(())
    }

    /// Cached messages to re-run OTP detection on, in rowid order.
    pub fn otp_rescan_batch(&self, after_rowid: i64, limit: i64) -> Result<Vec<OtpRescanRow>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.rowid, m.account_id, m.id, m.subject, m.from_email,
                    CASE WHEN m.body_loaded = 1 THEN m.body_text ELSE m.snippet END, m.body_html, m.body_loaded,
                    EXISTS (SELECT 1 FROM message_labels c WHERE c.account_id = m.account_id AND c.message_id = m.id
                            AND c.label_id = 'CATEGORY_PROMOTIONS'),
                    m.otp_code, m.otp_link, m.otp_score
             FROM messages m WHERE m.rowid > ?1 ORDER BY m.rowid LIMIT ?2",
        )?;
        stmt.query_map(params![after_rowid, limit], |r| {
            Ok(OtpRescanRow {
                rowid: r.get(0)?,
                account_id: r.get(1)?,
                id: r.get(2)?,
                subject: r.get(3)?,
                from: r.get(4)?,
                text: r.get(5)?,
                html: r.get(6)?,
                body_loaded: r.get(7)?,
                promotional: r.get(8)?,
                current_code: r.get(9)?,
                current_link: r.get(10)?,
                current_score: r.get(11)?,
            })
        })?
        .collect()
    }

    /// Batch-update re-run OTP results inside a single transaction to avoid disk thrashing.
    pub fn set_otp_batch(&mut self, updates: &[OtpRescanUpdate]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE messages SET otp_code = ?3, otp_link = ?4, otp_score = ?5 WHERE account_id = ?1 AND id = ?2",
            )?;
            for u in updates {
                let bias = otp_bias(&tx, u.account_id, &u.from)?;
                let keep = u.score + bias >= crate::otp::STORE_THRESHOLD;
                stmt.execute(params![
                    u.account_id,
                    u.id,
                    u.code.as_deref().filter(|_| keep),
                    u.link.as_deref().filter(|_| keep),
                    u.score,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Store a re-run OTP result (feedback still applies).
    pub fn set_otp(
        &mut self,
        account_id: i64,
        id: &str,
        from: &str,
        code: Option<&str>,
        link: Option<&str>,
        score: i32,
    ) -> Result<()> {
        let bias = otp_bias(&self.conn, account_id, from)?;
        let keep = score + bias >= crate::otp::STORE_THRESHOLD;
        self.conn.execute(
            "UPDATE messages SET otp_code = ?3, otp_link = ?4, otp_score = ?5 WHERE account_id = ?1 AND id = ?2",
            params![account_id, id, code.filter(|_| keep), link.filter(|_| keep), score],
        )?;
        Ok(())
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
        let (otp_code, otp_link) = otp_after_feedback(&tx, account_id, m)?;
        let unread = m.label_ids.iter().any(|l| l == "UNREAD");
        let starred = m.label_ids.iter().any(|l| l == "STARRED");
        tx.execute(
            "INSERT INTO messages(account_id, id, thread_id, internal_date, from_name, from_email, to_list,
                                  subject, snippet, body_text, body_html, is_unread, is_starred,
                                  has_attachments, otp_code, body_loaded,
                                  cc_list, reply_to, rfc_message_id, rfc_references, size, otp_score, otp_link,
                                  list_unsubscribe)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
             ON CONFLICT(account_id, id) DO UPDATE SET
                thread_id = excluded.thread_id, internal_date = excluded.internal_date,
                from_name = excluded.from_name, from_email = excluded.from_email, to_list = excluded.to_list,
                subject = excluded.subject, snippet = excluded.snippet, body_text = excluded.body_text,
                body_html = excluded.body_html, is_unread = excluded.is_unread, is_starred = excluded.is_starred,
                has_attachments = excluded.has_attachments, otp_code = excluded.otp_code,
                body_loaded = 1, cc_list = excluded.cc_list, reply_to = excluded.reply_to,
                rfc_message_id = excluded.rfc_message_id, rfc_references = excluded.rfc_references,
                size = CASE WHEN excluded.size > 0 THEN excluded.size ELSE messages.size END,
                otp_score = excluded.otp_score, otp_link = excluded.otp_link,
                list_unsubscribe = excluded.list_unsubscribe",
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
                otp_code,
                m.cc_list,
                m.reply_to,
                m.rfc_message_id,
                m.rfc_references,
                m.size,
                m.otp_score,
                otp_link,
                m.list_unsubscribe
            ],
        )?;
        write_labels(&tx, account_id, &m.id, &m.label_ids)?;
        // Keep the "saved on this PC" flag of attachments that are unchanged.
        let kept: Vec<u32> = {
            let mut stmt = tx.prepare_cached(
                "SELECT part FROM attachments WHERE account_id = ?1 AND message_id = ?2 AND stored = 1",
            )?;
            stmt.query_map(params![account_id, m.id], |r| r.get(0))?
                .collect::<Result<_>>()?
        };
        tx.execute(
            "DELETE FROM attachments WHERE account_id = ?1 AND message_id = ?2",
            params![account_id, m.id],
        )?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO attachments(account_id, message_id, part, filename, mime, size, content_id, inline, data, stored)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for a in &m.attachments {
                let stored = kept.contains(&a.part);
                stmt.execute(params![
                    account_id,
                    m.id,
                    a.part,
                    a.filename,
                    a.mime,
                    a.size,
                    a.content_id,
                    a.inline,
                    a.data,
                    stored
                ])?;
            }
        }
        tx.commit()
    }

    /// Insert or refresh a message from `format=metadata` (headers, snippet,
    /// labels). Never overwrites a body that is already cached.
    pub fn upsert_metadata(&mut self, account_id: i64, m: &NewMessage) -> Result<()> {
        let tx = self.conn.transaction()?;
        let (otp_code, _) = otp_after_feedback(&tx, account_id, m)?;
        let unread = m.label_ids.iter().any(|l| l == "UNREAD");
        let starred = m.label_ids.iter().any(|l| l == "STARRED");
        tx.execute(
            "INSERT INTO messages(account_id, id, thread_id, internal_date, from_name, from_email, to_list,
                                  subject, snippet, body_text, is_unread, is_starred, otp_code, body_loaded,
                                  size, has_attachments, otp_score, list_unsubscribe)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, 0, ?13, ?14, ?15, ?16)
             ON CONFLICT(account_id, id) DO UPDATE SET
                thread_id = excluded.thread_id, internal_date = excluded.internal_date,
                from_name = excluded.from_name, from_email = excluded.from_email, to_list = excluded.to_list,
                subject = excluded.subject, snippet = excluded.snippet,
                is_unread = excluded.is_unread, is_starred = excluded.is_starred,
                otp_score = CASE WHEN messages.otp_code IS NULL THEN excluded.otp_score ELSE messages.otp_score END,
                otp_code = COALESCE(messages.otp_code, excluded.otp_code),
                size = CASE WHEN excluded.size > 0 THEN excluded.size ELSE messages.size END,
                has_attachments = CASE WHEN messages.body_loaded = 1 THEN messages.has_attachments
                                       ELSE excluded.has_attachments END,
                list_unsubscribe = COALESCE(excluded.list_unsubscribe, messages.list_unsubscribe)",
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
                unread,
                starred,
                otp_code,
                m.size,
                m.has_attachments,
                m.otp_score,
                m.list_unsubscribe
            ],
        )?;
        write_labels(&tx, account_id, &m.id, &m.label_ids)?;
        tx.commit()
    }

    /// Message ids in a thread whose body hasn't been downloaded yet.
    pub fn ids_without_body(&self, account_id: i64, thread_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id FROM messages WHERE account_id = ?1 AND thread_id = ?2 AND body_loaded = 0
             ORDER BY internal_date DESC",
        )?;
        stmt.query_map(params![account_id, thread_id], |r| r.get(0))?.collect()
    }

    /// Newest INBOX messages still missing a body (for prefetch).
    pub fn inbox_ids_without_body(&self, account_id: i64, limit: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.id FROM messages m
             JOIN message_labels ml ON ml.account_id = m.account_id AND ml.message_id = m.id AND ml.label_id = 'INBOX'
             WHERE m.account_id = ?1 AND m.body_loaded = 0
             ORDER BY m.internal_date DESC LIMIT ?2",
        )?;
        stmt.query_map(params![account_id, limit], |r| r.get(0))?.collect()
    }

    pub fn message_count(&self, account_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1",
            [account_id],
            |r| r.get(0),
        )
    }

    /// Add/remove labels on every message of a thread (optimistic local
    /// update before the Gmail call). Returns how many messages changed.
    pub fn modify_thread_labels(
        &mut self,
        account_id: i64,
        thread_id: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let rows: Vec<(String, Option<String>)> = {
            let mut stmt = tx.prepare(
                "SELECT m.id, (SELECT group_concat(label_id, ' ') FROM message_labels ml
                                WHERE ml.account_id = m.account_id AND ml.message_id = m.id)
                 FROM messages m WHERE m.account_id = ?1 AND m.thread_id = ?2",
            )?;
            stmt.query_map(params![account_id, thread_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_>>()?
        };
        for (id, labels) in &rows {
            let mut set: Vec<String> = labels
                .as_deref()
                .unwrap_or_default()
                .split(' ')
                .filter(|l| !l.is_empty() && !remove.iter().any(|r| r == l))
                .map(str::to_owned)
                .collect();
            for a in add {
                if !set.contains(a) {
                    set.push(a.clone());
                }
            }
            let unread = set.iter().any(|l| l == "UNREAD");
            let starred = set.iter().any(|l| l == "STARRED");
            tx.execute(
                "UPDATE messages SET is_unread = ?3, is_starred = ?4 WHERE account_id = ?1 AND id = ?2",
                params![account_id, id, unread, starred],
            )?;
            write_labels(&tx, account_id, id, &set)?;
        }
        tx.commit()?;
        Ok(rows.len())
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

    /// Thread id of a cached message.
    pub fn message_thread(&self, account_id: i64, message_id: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT thread_id FROM messages WHERE account_id = ?1 AND id = ?2",
                params![account_id, message_id],
                |r| r.get(0),
            )
            .optional()
    }

    /// All message ids in a thread.
    pub fn message_ids_for_thread(&self, account_id: i64, thread_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM messages WHERE account_id = ?1 AND thread_id = ?2")?;
        let rows = stmt.query_map(params![account_id, thread_id], |r| r.get(0))?;
        rows.collect()
    }

    /// Search frequent contacts matching a prefix in name or email.
    /// If prefix is empty, returns the top most frequent contacts.
    pub fn search_contacts(
        &self,
        account_id: Option<i64>,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<ContactSuggestion>> {
        let clean = prefix.trim();
        let pattern = format!("%{clean}%");
        let mut stmt = self.conn.prepare(
            "WITH candidates AS (
                SELECT from_name AS name, from_email AS email, COUNT(*) AS cnt, MAX(internal_date) AS latest,
                       CASE WHEN ?1 IS NULL OR account_id = ?1 THEN 2 ELSE 1 END AS acct_weight
                FROM messages
                WHERE from_email != ''
                  AND (?2 = '' OR from_email LIKE ?3 OR from_name LIKE ?3)
                GROUP BY from_email
                UNION ALL
                SELECT display_name AS name, email, 100 AS cnt, 9999999999999 AS latest, 3 AS acct_weight
                FROM accounts
                WHERE email != ''
                  AND (?2 = '' OR email LIKE ?3 OR display_name LIKE ?3)
            )
            SELECT name, email, MAX(cnt) AS frequency
            FROM candidates
            GROUP BY lower(email)
            ORDER BY MAX(acct_weight) DESC, frequency DESC, MAX(latest) DESC
            LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![account_id, clean, pattern, limit as i64], |r| {
            Ok(ContactSuggestion {
                name: r.get(0)?,
                email: r.get(1)?,
                frequency: r.get(2)?,
            })
        })?;
        rows.collect()
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

    /// Threads carrying `label_id`, newest first. `account_id` None = every
    /// account (the unified inbox).
    pub fn list_threads(
        &self,
        account_id: Option<i64>,
        label_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ThreadRow>> {
        self.list_threads_in(account_id, label_id, None, limit, offset)
    }

    /// Like [`Store::list_threads`], limited to one Gmail inbox tab
    /// (`CATEGORY_PERSONAL` also takes mail without any category label).
    pub fn list_threads_in(
        &self,
        account_id: Option<i64>,
        label_id: &str,
        category: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ThreadRow>> {
        let mut stmt = self.conn.prepare_cached(
            "WITH lm AS (
                SELECT m.*,
                       ROW_NUMBER() OVER (PARTITION BY m.account_id, m.thread_id ORDER BY m.internal_date DESC) AS rn,
                       COUNT(*)     OVER (PARTITION BY m.account_id, m.thread_id) AS cnt,
                       MAX(m.is_unread)  OVER (PARTITION BY m.account_id, m.thread_id) AS any_unread,
                       MAX(m.is_starred) OVER (PARTITION BY m.account_id, m.thread_id) AS any_starred
                FROM messages m
                JOIN message_labels ml ON ml.account_id = m.account_id AND ml.message_id = m.id
                WHERE (?1 IS NULL OR m.account_id = ?1) AND ml.label_id = ?2
                  AND (?5 IS NULL OR COALESCE((SELECT c.label_id FROM message_labels c
                                WHERE c.account_id = m.account_id AND c.message_id = m.id
                                  AND c.label_id IN ('CATEGORY_SOCIAL', 'CATEGORY_PROMOTIONS', 'CATEGORY_UPDATES', 'CATEGORY_FORUMS')
                                LIMIT 1), 'CATEGORY_PERSONAL') = ?5)
             )
             SELECT thread_id, id, subject, from_name, from_email, snippet, internal_date,
                    cnt, any_unread, any_starred, otp_code, account_id,
                    EXISTS (SELECT 1 FROM message_labels mi WHERE mi.account_id = lm.account_id AND mi.label_id = 'IMPORTANT'
                       AND mi.message_id IN (SELECT id FROM messages t WHERE t.account_id = lm.account_id AND t.thread_id = lm.thread_id))
             FROM lm WHERE rn = 1
             ORDER BY internal_date DESC, account_id, thread_id
             LIMIT ?3 OFFSET ?4",
        )?;
        stmt.query_map(params![account_id, label_id, limit, offset, category], map_thread)?
            .collect()
    }

    /// Search cached mail with a Gmail-style query (words, phrases and the
    /// operators `from:`, `subject:`, `is:unread/read/starred`,
    /// `has:attachment`), one row per thread (its newest match).
    /// `account_id` None = all. Queries needing the server return nothing.
    pub fn search(&self, account_id: Option<i64>, query: &str, limit: i64) -> Result<Vec<ThreadRow>> {
        self.search_page(account_id, query, limit, 0)
    }

    pub fn search_page(&self, account_id: Option<i64>, query: &str, limit: i64, offset: i64) -> Result<Vec<ThreadRow>> {
        let q = query::parse(query);
        if q.is_empty() || !q.local_ok {
            return Ok(Vec::new());
        }
        let unread: Option<i64> = q.unread.map(i64::from);
        // Two phases so the page limit pushes down: `hits` keeps only narrow
        // columns (account, thread, last match date) grouped over the matches,
        // so a broad prefix query no longer materializes every matching full
        // row with window functions before the LIMIT; `lm` then re-applies the
        // same filters but restricted to the ~50 selected threads.
        let mut stmt = self.conn.prepare_cached(
            "WITH hits AS (
                SELECT m.account_id AS acct, m.thread_id AS tid, MAX(m.internal_date) AS last_date
                FROM messages m
                WHERE (?1 IS NULL OR m.account_id = ?1)
                  AND (?2 = '' OR m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?2))
                  AND (?4 IS NULL OR m.is_unread = ?4)
                  AND (?5 = 0 OR m.is_starred = 1)
                  AND (?6 = 0 OR m.has_attachments = 1)
                  AND NOT EXISTS (SELECT 1 FROM message_labels ml
                                  WHERE ml.account_id = m.account_id AND ml.message_id = m.id
                                    AND ml.label_id IN ('SPAM', 'TRASH'))
                GROUP BY m.account_id, m.thread_id
                ORDER BY last_date DESC
                LIMIT ?3 OFFSET ?7
             ),
             lm AS (
                SELECT m.*,
                       ROW_NUMBER() OVER (PARTITION BY m.account_id, m.thread_id ORDER BY m.internal_date DESC) AS rn,
                       COUNT(*) OVER (PARTITION BY m.account_id, m.thread_id) AS cnt,
                       MAX(m.is_unread)  OVER (PARTITION BY m.account_id, m.thread_id) AS any_unread,
                       MAX(m.is_starred) OVER (PARTITION BY m.account_id, m.thread_id) AS any_starred
                FROM messages m
                WHERE (?1 IS NULL OR m.account_id = ?1)
                  AND m.thread_id IN (SELECT tid FROM hits)
                  AND (?2 = '' OR m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?2))
                  AND (?4 IS NULL OR m.is_unread = ?4)
                  AND (?5 = 0 OR m.is_starred = 1)
                  AND (?6 = 0 OR m.has_attachments = 1)
                  AND NOT EXISTS (SELECT 1 FROM message_labels ml
                                  WHERE ml.account_id = m.account_id AND ml.message_id = m.id
                                    AND ml.label_id IN ('SPAM', 'TRASH'))
             )
             SELECT thread_id, id, subject, from_name, from_email, snippet, internal_date,
                    cnt, is_unread, is_starred, otp_code, account_id,
                    EXISTS (SELECT 1 FROM message_labels mi WHERE mi.account_id = lm.account_id AND mi.label_id = 'IMPORTANT'
                       AND mi.message_id IN (SELECT id FROM messages t WHERE t.account_id = lm.account_id AND t.thread_id = lm.thread_id))
             FROM lm WHERE rn = 1
             ORDER BY internal_date DESC, account_id, thread_id",
        )?;
        stmt.query_map(
            params![account_id, q.fts, limit, unread, q.starred, q.attachment, offset],
            map_thread,
        )?
        .collect()
    }

    /// Of `ids`, the ones not cached for this account (server search fills them in).
    pub fn missing_ids(&self, account_id: i64, ids: &[String]) -> Result<Vec<String>> {
        let json = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
        let mut stmt = self.conn.prepare_cached(
            "SELECT j.value FROM json_each(?2) j
             WHERE NOT EXISTS (SELECT 1 FROM messages m WHERE m.account_id = ?1 AND m.id = j.value)",
        )?;
        stmt.query_map(params![account_id, json], |r| r.get(0))?.collect()
    }

    /// Thread rows (newest message of each thread) for the threads that
    /// contain any of `message_ids`; used to show server search results.
    pub fn threads_for_messages(&self, account_id: i64, message_ids: &[String]) -> Result<Vec<ThreadRow>> {
        let json = serde_json::to_string(message_ids).unwrap_or_else(|_| "[]".into());
        let mut stmt = self.conn.prepare_cached(
            "WITH lm AS (
                SELECT m.*,
                       ROW_NUMBER() OVER (PARTITION BY m.thread_id ORDER BY m.internal_date DESC) AS rn,
                       COUNT(*)     OVER (PARTITION BY m.thread_id) AS cnt,
                       MAX(m.is_unread)  OVER (PARTITION BY m.thread_id) AS any_unread,
                       MAX(m.is_starred) OVER (PARTITION BY m.thread_id) AS any_starred
                FROM messages m
                WHERE m.account_id = ?1 AND m.thread_id IN (
                    SELECT thread_id FROM messages WHERE account_id = ?1 AND id IN (SELECT value FROM json_each(?2)))
             )
             SELECT thread_id, id, subject, from_name, from_email, snippet, internal_date,
                    cnt, any_unread, any_starred, otp_code, account_id,
                    EXISTS (SELECT 1 FROM message_labels mi WHERE mi.account_id = lm.account_id AND mi.label_id = 'IMPORTANT'
                       AND mi.message_id IN (SELECT id FROM messages t WHERE t.account_id = lm.account_id AND t.thread_id = lm.thread_id))
             FROM lm WHERE rn = 1
             ORDER BY internal_date DESC",
        )?;
        stmt.query_map(params![account_id, json], map_thread)?.collect()
    }

    /// Unread INBOX threads per account (for the tray badge and the rail).
    pub fn inbox_unread(&self) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT m.account_id, COUNT(DISTINCT m.thread_id)
             FROM messages m
             JOIN message_labels ml ON ml.account_id = m.account_id AND ml.message_id = m.id AND ml.label_id = 'INBOX'
             WHERE m.is_unread = 1
             GROUP BY m.account_id",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
    }

    /// Messages newer than `since_ms` that have no body yet (offline copies).
    pub fn ids_without_body_since(&self, account_id: i64, since_ms: i64, limit: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id FROM messages WHERE account_id = ?1 AND body_loaded = 0 AND internal_date >= ?2
             ORDER BY internal_date DESC LIMIT ?3",
        )?;
        stmt.query_map(params![account_id, since_ms, limit], |r| r.get(0))?
            .collect()
    }

    /// Size of the database file in bytes (page_count * page_size).
    pub fn db_size(&self) -> Result<i64> {
        self.conn.query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [],
            |r| r.get(0),
        )
    }

    pub fn get_thread(&self, account_id: i64, thread_id: &str) -> Result<Vec<MessageRow>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, thread_id, internal_date, from_name, from_email, to_list, subject, snippet,
                    body_text, body_html, is_unread, is_starred, has_attachments, otp_code, body_loaded,
                    (SELECT group_concat(label_id, ' ') FROM message_labels ml
                      WHERE ml.account_id = m.account_id AND ml.message_id = m.id),
                    cc_list, reply_to, rfc_message_id, rfc_references, otp_link, list_unsubscribe
             FROM messages m WHERE account_id = ?1 AND thread_id = ?2
             ORDER BY internal_date ASC",
        )?;
        let mut rows = stmt
            .query_map(params![account_id, thread_id], |r| {
                let labels: Option<String> = r.get(15)?;
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
                    otp_link: r.get(20)?,
                    body_loaded: r.get(14)?,
                    label_ids: labels
                        .map(|s| s.split(' ').map(str::to_owned).collect())
                        .unwrap_or_default(),
                    cc_list: r.get(16)?,
                    reply_to: r.get(17)?,
                    rfc_message_id: r.get(18)?,
                    rfc_references: r.get(19)?,
                    list_unsubscribe: r.get(21)?,
                    attachments: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        for m in &mut rows {
            m.attachments = self.attachments(account_id, &m.id, false)?;
        }
        Ok(rows)
    }

    /// Attachment list of a message (optionally including inline images).
    pub fn attachments(&self, account_id: i64, message_id: &str, with_inline: bool) -> Result<Vec<AttachmentRow>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT part, filename, mime, size, inline, stored FROM attachments
             WHERE account_id = ?1 AND message_id = ?2 AND (?3 OR inline = 0) ORDER BY part",
        )?;
        stmt.query_map(params![account_id, message_id, with_inline], |r| {
            Ok(AttachmentRow {
                part: r.get(0)?,
                filename: r.get(1)?,
                mime: r.get(2)?,
                size: r.get(3)?,
                inline: r.get(4)?,
                stored: r.get(5)?,
            })
        })?
        .collect()
    }

    /// Cached inline images of a message: (content id, mime, bytes).
    pub fn inline_images(&self, account_id: i64, message_id: &str) -> Result<Vec<(String, String, Vec<u8>)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT content_id, mime, data FROM attachments
             WHERE account_id = ?1 AND message_id = ?2 AND content_id IS NOT NULL AND data IS NOT NULL",
        )?;
        stmt.query_map(params![account_id, message_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect()
    }

    /// One message's cached body for rendering.
    pub fn message_body(&self, account_id: i64, message_id: &str) -> Result<Option<CachedBody>> {
        self.conn
            .query_row(
                "SELECT from_email, body_html, body_text, body_loaded FROM messages WHERE account_id = ?1 AND id = ?2",
                params![account_id, message_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
    }

    pub fn images_allowed_from(&self, account_id: i64, email: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM image_senders WHERE account_id = ?1 AND email = ?2",
                params![account_id, email.to_ascii_lowercase()],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn set_images_allowed(&self, account_id: i64, email: &str, allowed: bool) -> Result<()> {
        let email = email.to_ascii_lowercase();
        if allowed {
            self.conn.execute(
                "INSERT OR IGNORE INTO image_senders(account_id, email) VALUES (?1, ?2)",
                params![account_id, email],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM image_senders WHERE account_id = ?1 AND email = ?2",
                params![account_id, email],
            )?;
        }
        Ok(())
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
        sync_window: r.get(4)?,
        notify: r.get(5)?,
        backfill_state: r.get(6)?,
        backfill_page: r.get(7)?,
        backfill_done: r.get(8)?,
        backfill_total: r.get(9)?,
        messages_total: r.get(10)?,
        backfill_phase: r.get(11)?,
        backfill_error: r.get(12)?,
        attach_mode: r.get(13)?,
        attach_max_mb: r.get(14)?,
        display_name: r.get(15)?,
        color: r.get(16)?,
        signature: r.get(17)?,
        category_tabs: r.get(18)?,
        avatar_data: r.get(19)?,
    })
}

/// Gmail inbox tab of message `m`: Social / Promotions / Updates / Forums,
/// else Primary (Gmail files uncategorized mail under Primary too).
const CATEGORY_EXPR: &str = "COALESCE((SELECT c.label_id FROM message_labels c
     WHERE c.account_id = m.account_id AND c.message_id = m.id
       AND c.label_id IN ('CATEGORY_SOCIAL', 'CATEGORY_PROMOTIONS', 'CATEGORY_UPDATES', 'CATEGORY_FORUMS')
     LIMIT 1), 'CATEGORY_PERSONAL')";

pub const CATEGORIES: &[&str] = &[
    "CATEGORY_PERSONAL",
    "CATEGORY_PROMOTIONS",
    "CATEGORY_SOCIAL",
    "CATEGORY_UPDATES",
    "CATEGORY_FORUMS",
];

fn otp_bias(conn: &Connection, account_id: i64, sender: &str) -> Result<i32> {
    if sender.is_empty() {
        return Ok(0);
    }
    conn.query_row(
        "SELECT score FROM otp_feedback WHERE account_id = ?1 AND sender = lower(?2)",
        params![account_id, sender],
        |r| r.get(0),
    )
    .optional()
    .map(Option::unwrap_or_default)
}

/// The message's code/link unless "not a code" feedback for its sender pulls
/// the confidence under the threshold.
fn otp_after_feedback(conn: &Connection, account_id: i64, m: &NewMessage) -> Result<(Option<String>, Option<String>)> {
    if m.otp_code.is_none() && m.otp_link.is_none() {
        return Ok((None, None));
    }
    let bias = otp_bias(conn, account_id, &m.from_email)?;
    Ok(if m.otp_score + bias >= crate::otp::STORE_THRESHOLD {
        (m.otp_code.clone(), m.otp_link.clone())
    } else {
        (None, None)
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
        account_id: r.get(11)?,
        important: r.get(12)?,
    })
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
    fn account_name_uses_own_sent_mail_and_preserves_user_choice() {
        let (mut s, id) = setup();
        let other = s.upsert_account("other@example.com", 1).unwrap().id;
        for (account, mid, name, email, date, labels) in [
            (id, "s1", "Alex Saber", "ME@EXAMPLE.COM", 1, vec!["SENT"]),
            (id, "s2", "Alex Saber", "me@example.com", 2, vec!["SENT"]),
            (id, "s3", "Old Name", "me@example.com", 3, vec!["SENT"]),
            (id, "s4", "Someone Else", "stranger@example.com", 4, vec!["SENT"]),
            (id, "s5", "Incoming", "me@example.com", 5, vec!["INBOX"]),
            (other, "s6", "Wrong Account", "me@example.com", 6, vec!["SENT"]),
            (id, "s7", "me@example.com", "me@example.com", 7, vec!["SENT"]),
        ] {
            let mut m = msg(mid, mid, date, &labels, "Synthetic fixture");
            m.from_name = name.into();
            m.from_email = email.into();
            s.upsert_metadata(account, &m).unwrap();
        }
        assert!(s.fill_account_name_from_sent(id).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "Alex Saber");
        assert!(!s.fill_account_name_from_sent(id).unwrap());
        s.set_account_settings(
            id,
            &AccountSettings {
                display_name: Some("My choice".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!s.set_account_name_if_empty(id, "Late Gmail result").unwrap());
        assert!(!s.fill_account_name_from_sent(id).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "My choice");
        assert_eq!(s.account(other).unwrap().unwrap().display_name, "");
    }

    #[test]
    fn account_name_ties_use_recency_then_name_and_handle_unicode() {
        let (mut s, id) = setup();
        assert!(!s.fill_account_name_from_sent(id).unwrap());
        for (mid, name, date) in [("s1", "Old Name", 1), ("s2", "  زيد\n أحمد  ", 2), ("s3", "علي", 2)] {
            let mut m = msg(mid, mid, date, &["SENT"], "Synthetic fixture");
            m.from_name = name.into();
            m.from_email = "me@example.com".into();
            s.upsert_metadata(id, &m).unwrap();
        }
        assert!(s.fill_account_name_from_sent(id).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "زيد أحمد");
        assert!(!s.set_account_name_if_empty(999, "Missing account").unwrap());
    }

    #[test]
    fn inferred_account_names_are_bounded_and_blank_names_are_ignored() {
        let (s, id) = setup();
        assert!(!s.set_account_name_if_empty(id, " \n\t ").unwrap());
        assert!(s.set_account_name_if_empty(id, &"🔐".repeat(100)).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "🔐".repeat(80));
    }

    #[test]
    fn google_profile_replaces_inference_but_preserves_custom_names() {
        let (s, id) = setup();
        s.set_account_name_if_empty(id, "Name from sent mail").unwrap();
        assert!(s.set_google_profile(id, " Google Name ", None).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "Google Name");
        s.set_account_settings(
            id,
            &AccountSettings {
                display_name: Some("My choice".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.set_google_profile(id, "New Google Name", None).unwrap());
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "My choice");
        s.set_account_settings(
            id,
            &AccountSettings {
                display_name: Some("".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "New Google Name");
        s.set_google_profile(id, "", None).unwrap();
        assert_eq!(s.account(id).unwrap().unwrap().display_name, "New Google Name");
        assert!(!s.set_google_profile(999, "Name", None).unwrap());
    }

    #[test]
    fn google_profile_photo_roundtrips_and_survives_an_unavailable_photo() {
        let (s, id) = setup();
        let other = s.upsert_account("other@example.com", 1).unwrap().id;
        let data = "data:image/png;base64,iVBORw0KGgo=";
        assert!(s.account(id).unwrap().unwrap().avatar_data.is_none());
        s.set_google_profile(id, "Google Name", Some(data)).unwrap();
        assert_eq!(s.accounts().unwrap()[0].avatar_data.as_deref(), Some(data));
        s.set_google_profile(id, "Google Name", None).unwrap();
        assert_eq!(s.account(id).unwrap().unwrap().avatar_data.as_deref(), Some(data));
        assert!(s.account(other).unwrap().unwrap().avatar_data.is_none());
    }

    #[test]
    fn google_profile_migration_preserves_existing_names_and_mail() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &migrations::MIGRATIONS[..9] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 9).unwrap();
        conn.execute("INSERT INTO accounts(id, email, created_at, display_name) VALUES (1, 'me@example.com', 1, 'My existing name')", []).unwrap();
        conn.execute("INSERT INTO messages(account_id, id, thread_id, internal_date, body_text) VALUES (1, 'm1', 't1', 1, 'Offline fixture')", []).unwrap();
        let s = Store::init(conn).unwrap();
        assert_eq!(s.account(1).unwrap().unwrap().display_name, "My existing name");
        assert!(s.account(1).unwrap().unwrap().avatar_data.is_none());
        assert_eq!(s.get_thread(1, "t1").unwrap()[0].body_text, "Offline fixture");
        s.set_google_profile(1, "Google Name", None).unwrap();
        assert_eq!(s.account(1).unwrap().unwrap().display_name, "My existing name");
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

        let threads = s.list_threads(Some(id), "INBOX", 50, 0).unwrap();
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
        assert!(!s.list_threads(Some(id), "INBOX", 10, 0).unwrap()[0].unread);
        assert!(!s.set_message_labels(id, "missing", &["INBOX".into()]).unwrap());

        s.delete_message(id, "m1").unwrap();
        assert!(!s.has_message(id, "m1").unwrap());
        assert!(s.list_threads(Some(id), "INBOX", 10, 0).unwrap().is_empty());
    }

    #[test]
    fn full_text_search_with_prefix_and_injection_safety() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "Quarterly invoice"))
            .unwrap();
        s.upsert_message(id, &msg("m2", "t2", 200, &["INBOX"], "Lunch plans"))
            .unwrap();
        let hits = s.search(Some(id), "invo", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].last_message_id, "m1");
        // FTS operators in user input must not error
        assert!(s.search(Some(id), "\"unbalanced OR NEAR(", 10).is_ok());
        assert!(s.search(Some(id), "   ", 10).unwrap().is_empty());
        // updates keep the index in sync
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "Renamed"))
            .unwrap();
        assert!(s.search(Some(id), "invoice", 10).unwrap().is_empty());
    }

    #[test]
    fn search_pagination_pages_through_threads_consistently() {
        let (mut s, id) = setup();
        for (i, (mid, tid)) in [("m1", "t1"), ("m2", "t2"), ("m3", "t3"), ("m4", "t4")]
            .into_iter()
            .enumerate()
        {
            s.upsert_message(id, &msg(mid, tid, 100 + i as i64, &["INBOX"], "Invoice page test"))
                .unwrap();
        }
        let all = s.search(Some(id), "invoice page", 10).unwrap();
        assert_eq!(all.len(), 4, "newest first");
        assert_eq!(all[0].last_message_id, "m4");
        // Pages must tile the full result without gaps or repeats.
        let page1 = s.search_page(Some(id), "invoice page", 2, 0).unwrap();
        let page2 = s.search_page(Some(id), "invoice page", 2, 2).unwrap();
        let paged: Vec<String> = page1
            .iter()
            .chain(page2.iter())
            .map(|t| t.last_message_id.clone())
            .collect();
        assert_eq!(paged, all.iter().map(|t| t.last_message_id.clone()).collect::<Vec<_>>());
        // An offset past the end is empty, and per-thread message counts survive paging.
        assert!(s.search_page(Some(id), "invoice page", 2, 4).unwrap().is_empty());
        s.upsert_message(id, &msg("m5", "t4", 500, &["INBOX"], "Invoice page test two"))
            .unwrap();
        let with_cnt = s.search_page(Some(id), "invoice page", 10, 0).unwrap();
        let t4 = with_cnt.iter().find(|t| t.thread_id == "t4").unwrap();
        assert_eq!(t4.message_count, 2);
    }

    #[test]
    fn search_operators_and_server_helpers() {
        let (mut s, id) = setup();
        let mut a = msg("m1", "t1", 100, &["INBOX", "UNREAD"], "Invoice March");
        a.has_attachments = true;
        s.upsert_message(id, &a).unwrap();
        let mut b = msg("m2", "t2", 200, &["INBOX", "STARRED"], "Invoice April");
        b.from_name = "Bob".into();
        b.from_email = "bob@corp.test".into();
        s.upsert_message(id, &b).unwrap();
        s.upsert_message(id, &msg("m3", "t3", 300, &["TRASH"], "Invoice trash"))
            .unwrap();

        let ids = |q: &str| {
            s.search(Some(id), q, 10)
                .unwrap()
                .into_iter()
                .map(|t| t.last_message_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("invoice"), ["m2", "m1"], "trash excluded, newest first");
        assert_eq!(ids("from:bob invoice"), ["m2"]);
        assert_eq!(ids("is:unread"), ["m1"]);
        assert_eq!(ids("is:starred"), ["m2"]);
        assert_eq!(ids("has:attachment invoice"), ["m1"]);
        assert_eq!(ids(r#"subject:"invoice april""#), ["m2"]);
        assert!(ids("in:sent invoice").is_empty(), "server-only query");

        let missing = s.missing_ids(id, &["m1".into(), "zz".into()]).unwrap();
        assert_eq!(missing, ["zz"]);
        s.upsert_message(id, &msg("m4", "t1", 400, &["SENT"], "Re: Invoice March"))
            .unwrap();
        let rows = s.threads_for_messages(id, &["m1".into(), "m2".into()]).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].last_message_id, "m4", "thread shows its newest message");
        assert_eq!(rows[0].message_count, 2);
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
    fn optimize_keeps_search_working() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX"], "invoice tomorrow"))
            .unwrap();
        s.optimize().unwrap();
        // The maintenance pass must not break FTS or further writes.
        s.upsert_message(id, &msg("m2", "t2", 200, &["INBOX"], "second invoice"))
            .unwrap();
        s.optimize().unwrap();
        let hits = s.search(Some(id), "invoice", 10).unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn list_unsubscribe_roundtrip() {
        let (mut s, id) = setup();
        let mut full = msg("m1", "t1", 100, &["INBOX"], "deal");
        full.list_unsubscribe = Some("<https://shop.test/leave>, <mailto:leave@shop.test>".into());
        s.upsert_message(id, &full).unwrap();
        // A metadata refresh without the header must not clear a stored value.
        let meta = msg("m1", "t1", 100, &["INBOX"], "deal");
        s.upsert_metadata(id, &meta).unwrap();
        assert_eq!(
            s.get_thread(id, "t1").unwrap()[0].list_unsubscribe.as_deref(),
            Some("<https://shop.test/leave>, <mailto:leave@shop.test>")
        );
        // No header anywhere -> None.
        s.upsert_message(id, &msg("m2", "t2", 100, &["INBOX"], "x")).unwrap();
        assert_eq!(s.get_thread(id, "t2").unwrap()[0].list_unsubscribe, None);
    }

    #[test]
    fn metadata_then_body_then_metadata_keeps_body() {
        let (mut s, id) = setup();
        let mut meta = msg("m1", "t1", 100, &["INBOX", "UNREAD"], "Hi");
        meta.body_text = String::new();
        s.upsert_metadata(id, &meta).unwrap();
        assert_eq!(s.ids_without_body(id, "t1").unwrap(), vec!["m1".to_string()]);
        assert_eq!(s.inbox_ids_without_body(id, 10).unwrap().len(), 1);
        assert!(!s.get_thread(id, "t1").unwrap()[0].body_loaded);

        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX", "UNREAD"], "Hi"))
            .unwrap();
        assert!(s.get_thread(id, "t1").unwrap()[0].body_loaded);

        // a later metadata refresh (labels changed) must not drop the body
        s.upsert_metadata(id, &msg("m1", "t1", 100, &["INBOX"], "Hi")).unwrap();
        let m = &s.get_thread(id, "t1").unwrap()[0];
        assert!(m.body_loaded);
        assert_eq!(m.body_text, "body of Hi");
        assert!(!m.unread);
        assert_eq!(s.message_count(id).unwrap(), 1);
    }

    #[test]
    fn modify_thread_labels_archive_star_read() {
        let (mut s, id) = setup();
        s.upsert_message(id, &msg("m1", "t1", 100, &["INBOX", "UNREAD"], "a"))
            .unwrap();
        s.upsert_message(id, &msg("m2", "t1", 200, &["INBOX", "UNREAD"], "b"))
            .unwrap();
        let n = s
            .modify_thread_labels(id, "t1", &["STARRED".into()], &["UNREAD".into(), "INBOX".into()])
            .unwrap();
        assert_eq!(n, 2);
        assert!(s.list_threads(Some(id), "INBOX", 10, 0).unwrap().is_empty());
        let starred = s.list_threads(Some(id), "STARRED", 10, 0).unwrap();
        assert_eq!(starred.len(), 1);
        assert!(!starred[0].unread && starred[0].starred);
        // undo
        s.modify_thread_labels(id, "t1", &["INBOX".into(), "UNREAD".into()], &["STARRED".into()])
            .unwrap();
        assert!(s.list_threads(Some(id), "INBOX", 10, 0).unwrap()[0].unread);
    }

    #[test]
    fn attachments_and_image_allowlist() {
        let (mut s, id) = setup();
        let mut m = msg("m1", "t1", 1, &["INBOX"], "Files");
        m.attachments = vec![
            NewAttachment {
                part: 0,
                filename: "logo.png".into(),
                mime: "image/png".into(),
                size: 3,
                content_id: Some("logo@x".into()),
                inline: true,
                data: Some(vec![1, 2, 3]),
            },
            NewAttachment {
                part: 1,
                filename: "r.pdf".into(),
                mime: "application/pdf".into(),
                size: 9,
                content_id: None,
                inline: false,
                data: None,
            },
        ];
        s.upsert_message(id, &m).unwrap();
        let t = s.get_thread(id, "t1").unwrap();
        assert_eq!(t[0].attachments.len(), 1);
        assert_eq!(t[0].attachments[0].filename, "r.pdf");
        assert_eq!(
            s.inline_images(id, "m1").unwrap(),
            vec![("logo@x".into(), "image/png".into(), vec![1, 2, 3])]
        );
        // re-upsert replaces, doesn't duplicate
        s.upsert_message(id, &m).unwrap();
        assert_eq!(s.attachments(id, "m1", true).unwrap().len(), 2);

        assert!(!s.images_allowed_from(id, "News@Shop.test").unwrap());
        s.set_images_allowed(id, "News@Shop.test", true).unwrap();
        assert!(s.images_allowed_from(id, "news@shop.test").unwrap());
        s.set_images_allowed(id, "news@shop.test", false).unwrap();
        assert!(!s.images_allowed_from(id, "news@shop.test").unwrap());
    }

    #[test]
    fn settings_roundtrip() {
        let (s, _) = setup();
        assert_eq!(s.get_setting("close_behavior").unwrap(), None);
        s.set_setting("close_behavior", "hide").unwrap();
        s.set_setting("close_behavior", "destroy").unwrap();
        assert_eq!(s.get_setting("close_behavior").unwrap().as_deref(), Some("destroy"));
    }

    #[test]
    fn accounts_window_backfill_and_unified_inbox() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("a@example.com", 1).unwrap();
        let b = s.upsert_account("b@example.com", 2).unwrap();
        assert_eq!(a.backfill_state, "setup");
        assert_eq!(s.accounts().unwrap().len(), 2);
        s.set_account_window(a.id, "1y").unwrap();
        s.set_backfill_progress(a.id, Some("tok"), 40, 100).unwrap();
        let a2 = s.account(a.id).unwrap().unwrap();
        assert_eq!(
            (a2.sync_window.as_str(), a2.backfill_done, a2.backfill_page.as_deref()),
            ("1y", 40, Some("tok"))
        );
        s.set_account_window(a.id, "all").unwrap();
        let a3 = s.account(a.id).unwrap().unwrap();
        assert_eq!(
            (a3.backfill_done, a3.backfill_page, a3.backfill_state.as_str()),
            (0, None, "setup")
        );
        s.set_backfill_state(a.id, "running").unwrap();
        s.set_account_window(a.id, "30d").unwrap();
        assert_eq!(s.account(a.id).unwrap().unwrap().backfill_state, "pending");

        s.upsert_message(a.id, &msg("m1", "t1", 10, &["INBOX", "UNREAD"], "hi"))
            .unwrap();
        s.upsert_message(b.id, &msg("m1", "t1", 20, &["INBOX"], "hi")).unwrap();
        let all = s.list_threads(None, "INBOX", 10, 0).unwrap();
        assert_eq!(all.len(), 2, "same Gmail ids in two accounts stay separate");
        assert_eq!(all[0].account_id, b.id);
        assert_eq!(s.list_threads(Some(a.id), "INBOX", 10, 0).unwrap().len(), 1);
        assert_eq!(s.inbox_unread().unwrap(), vec![(a.id, 1)]);
        assert!(s.db_size().unwrap() > 0);
    }

    #[test]
    fn inbox_tabs_important_and_otp_feedback() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("a@example.com", 1).unwrap();
        s.upsert_message(
            a.id,
            &msg(
                "p1",
                "t1",
                10,
                &["INBOX", "UNREAD", "CATEGORY_PERSONAL", "IMPORTANT"],
                "hi",
            ),
        )
        .unwrap();
        s.upsert_message(
            a.id,
            &msg("n1", "t2", 20, &["INBOX", "UNREAD"], "no category = primary"),
        )
        .unwrap();
        s.upsert_message(a.id, &msg("x1", "t3", 30, &["INBOX", "CATEGORY_PROMOTIONS"], "sale"))
            .unwrap();
        s.upsert_message(
            a.id,
            &msg("u1", "t4", 40, &["INBOX", "UNREAD", "CATEGORY_UPDATES"], "receipt"),
        )
        .unwrap();
        let ids = |cat: &str| -> Vec<String> {
            s.list_threads_in(Some(a.id), "INBOX", Some(cat), 50, 0)
                .unwrap()
                .into_iter()
                .map(|t| t.thread_id)
                .collect()
        };
        assert_eq!(ids("CATEGORY_PERSONAL"), vec!["t2".to_string(), "t1".to_string()]);
        assert_eq!(ids("CATEGORY_PROMOTIONS"), vec!["t3".to_string()]);
        assert!(ids("CATEGORY_FORUMS").is_empty());
        let all = s.list_threads(Some(a.id), "INBOX", 50, 0).unwrap();
        assert_eq!(all.len(), 4);
        assert!(all.iter().find(|t| t.thread_id == "t1").unwrap().important);
        assert!(!all.iter().find(|t| t.thread_id == "t2").unwrap().important);
        let counts = s.category_counts(Some(a.id)).unwrap();
        let get = |c: &str| counts.iter().find(|(x, _, _)| x == c).map(|(_, u, t)| (*u, *t));
        assert_eq!(get("CATEGORY_PERSONAL"), Some((2, 2)));
        assert_eq!(get("CATEGORY_PROMOTIONS"), Some((0, 1)));
        assert_eq!(get("CATEGORY_UPDATES"), Some((1, 1)));

        // "Not a code" twice for a sender vetoes its codes from then on.
        let mut otp = msg("o1", "t5", 50, &["INBOX"], "code");
        otp.from_email = "Alerts@Shop.example".into();
        otp.otp_code = Some("482913".into());
        otp.otp_score = 70;
        s.upsert_message(a.id, &otp).unwrap();
        assert_eq!(s.get_thread(a.id, "t5").unwrap()[0].otp_code.as_deref(), Some("482913"));
        s.otp_feedback(a.id, "alerts@shop.example", false, Some("o1")).unwrap();
        assert_eq!(
            s.get_thread(a.id, "t5").unwrap()[0].otp_code,
            None,
            "cleared on feedback"
        );
        s.otp_feedback(a.id, "alerts@shop.example", false, None).unwrap();
        assert_eq!(s.otp_bias(a.id, "ALERTS@shop.example").unwrap(), -60);
        s.upsert_message(a.id, &otp).unwrap();
        assert_eq!(
            s.get_thread(a.id, "t5").unwrap()[0].otp_code,
            None,
            "vetoed for this sender"
        );
        // A copy nudges it back up, capped.
        for _ in 0..10 {
            s.otp_feedback(a.id, "alerts@shop.example", true, None).unwrap();
        }
        assert_eq!(s.otp_bias(a.id, "alerts@shop.example").unwrap(), 20);
    }

    #[test]
    fn history_queue_sizes_and_stored_files() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("a@example.com", 1).unwrap();
        s.upsert_message(a.id, &msg("m1", "t1", 10, &["INBOX"], "hi")).unwrap();
        let ids: Vec<String> = ["m1", "m2", "m3"].iter().map(|x| x.to_string()).collect();
        assert_eq!(s.queue_missing(a.id, &ids).unwrap(), 2, "cached ids are not queued");
        assert_eq!(s.queue_missing(a.id, &ids).unwrap(), 0, "idempotent");
        assert_eq!(s.queue_len(a.id).unwrap(), 2);
        let take = s.queue_take(a.id, 1).unwrap();
        s.queue_remove(a.id, &take).unwrap();
        assert_eq!(s.queue_len(a.id).unwrap(), 1);
        // a new range clears the queue and the phase
        s.set_backfill_phase(a.id, "index").unwrap();
        s.set_account_window(a.id, "all").unwrap();
        assert_eq!(s.queue_len(a.id).unwrap(), 0);
        assert_eq!(s.account(a.id).unwrap().unwrap().backfill_phase, "");

        let mut big = msg("m4", "t4", 20, &["INBOX"], "files");
        big.size = 5_000_000;
        big.has_attachments = true;
        big.attachments = vec![NewAttachment {
            part: 0,
            filename: "a.pdf".into(),
            mime: "application/pdf".into(),
            size: 4_900_000,
            ..Default::default()
        }];
        s.upsert_metadata(
            a.id,
            &NewMessage {
                size: 5_000_000,
                has_attachments: true,
                ..big.clone()
            },
        )
        .unwrap();
        let est = s.size_estimate(a.id, 0).unwrap();
        assert_eq!((est.sized, est.total_bytes, est.text_bytes), (1, 5_000_000, 80_000));
        assert_eq!(
            s.body_candidates(a.id, 0, 1_000_000, 10).unwrap(),
            Vec::<String>::new(),
            "too big for the policy"
        );
        assert_eq!(s.body_candidates(a.id, 0, 0, 10).unwrap(), vec!["m4".to_string()]);
        s.upsert_message(a.id, &big).unwrap();
        assert_eq!(
            s.file_candidates(a.id, 0, 10_000_000, 10).unwrap(),
            vec!["m4".to_string()]
        );
        assert!(s.file_candidates(a.id, 0, 1_000_000, 10).unwrap().is_empty());
        s.mark_stored(a.id, "m4", 0).unwrap();
        assert!(s.attachment_stored(a.id, "m4", 0).unwrap());
        assert_eq!(s.stored_attachment_bytes(Some(a.id)).unwrap(), 4_900_000);
        // re-caching the body keeps the stored flag
        s.upsert_message(a.id, &big).unwrap();
        assert!(s.file_candidates(a.id, 0, 0, 10).unwrap().is_empty());

        s.set_account_settings(
            a.id,
            &AccountSettings {
                attach_mode: Some("all".into()),
                signature: Some("-- A".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let acc = s.account(a.id).unwrap().unwrap();
        assert_eq!(
            (acc.attach_mode.as_str(), acc.signature.as_str(), acc.attach_max_mb),
            ("all", "-- A", 10)
        );
    }

    #[test]
    fn search_contacts_and_accounts() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("main@example.com", 1).unwrap();
        let mut m1 = msg("m1", "t1", 100, &["INBOX"], "hi");
        m1.from_email = "friend@example.com".into();
        m1.from_name = "Best Friend".into();
        s.upsert_message(a.id, &m1).unwrap();

        let res_empty = s.search_contacts(Some(a.id), "", 10).unwrap();
        assert!(res_empty.iter().any(|c| c.email == "main@example.com"));
        assert!(res_empty.iter().any(|c| c.email == "friend@example.com"));

        let res_query = s.search_contacts(Some(a.id), "friend", 10).unwrap();
        assert_eq!(res_query.len(), 1);
        assert_eq!(res_query[0].name, "Best Friend");

        let res_name = s.search_contacts(Some(a.id), "best", 10).unwrap();
        assert_eq!(res_name.len(), 1);
        assert_eq!(res_name[0].email, "friend@example.com");
    }
}

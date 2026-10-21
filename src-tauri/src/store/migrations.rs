//! Schema migrations, applied in order and tracked with `PRAGMA user_version`.
//! Never edit a shipped migration; append a new one.

pub const MIGRATIONS: &[&str] = &[
    // 1: initial schema
    r#"
    CREATE TABLE accounts (
        id              INTEGER PRIMARY KEY,
        email           TEXT NOT NULL UNIQUE,
        history_id      TEXT,
        last_sync_at    INTEGER,
        created_at      INTEGER NOT NULL
    );

    CREATE TABLE labels (
        account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        id          TEXT NOT NULL,
        name        TEXT NOT NULL,
        kind        TEXT NOT NULL,           -- 'system' | 'user'
        unread      INTEGER NOT NULL DEFAULT 0,
        total       INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (account_id, id)
    );

    CREATE TABLE messages (
        account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        id              TEXT NOT NULL,
        thread_id       TEXT NOT NULL,
        internal_date   INTEGER NOT NULL,    -- ms since epoch (Gmail internalDate)
        from_name       TEXT NOT NULL DEFAULT '',
        from_email      TEXT NOT NULL DEFAULT '',
        to_list         TEXT NOT NULL DEFAULT '',
        subject         TEXT NOT NULL DEFAULT '',
        snippet         TEXT NOT NULL DEFAULT '',
        body_text       TEXT NOT NULL DEFAULT '',
        body_html       TEXT,
        is_unread       INTEGER NOT NULL DEFAULT 0,
        is_starred      INTEGER NOT NULL DEFAULT 0,
        has_attachments INTEGER NOT NULL DEFAULT 0,
        otp_code        TEXT,
        PRIMARY KEY (account_id, id)
    );
    CREATE INDEX messages_thread ON messages(account_id, thread_id);
    CREATE INDEX messages_date   ON messages(account_id, internal_date DESC);

    CREATE TABLE message_labels (
        account_id  INTEGER NOT NULL,
        label_id    TEXT NOT NULL,
        message_id  TEXT NOT NULL,
        PRIMARY KEY (account_id, label_id, message_id),
        FOREIGN KEY (account_id, message_id) REFERENCES messages(account_id, id) ON DELETE CASCADE
    );
    CREATE INDEX message_labels_msg ON message_labels(account_id, message_id);

    CREATE VIRTUAL TABLE messages_fts USING fts5(
        subject, from_name, from_email, snippet, body_text,
        content='messages', content_rowid='rowid', tokenize='unicode61 remove_diacritics 2'
    );
    CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
        INSERT INTO messages_fts(rowid, subject, from_name, from_email, snippet, body_text)
        VALUES (new.rowid, new.subject, new.from_name, new.from_email, new.snippet, new.body_text);
    END;
    CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
        INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_email, snippet, body_text)
        VALUES ('delete', old.rowid, old.subject, old.from_name, old.from_email, old.snippet, old.body_text);
    END;
    CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
        INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_email, snippet, body_text)
        VALUES ('delete', old.rowid, old.subject, old.from_name, old.from_email, old.snippet, old.body_text);
        INSERT INTO messages_fts(rowid, subject, from_name, from_email, snippet, body_text)
        VALUES (new.rowid, new.subject, new.from_name, new.from_email, new.snippet, new.body_text);
    END;

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
    // 2: metadata-first sync. Rows fetched with format=metadata have no body
    // yet; bodies load lazily on open (or by prefetch).
    r#"
    ALTER TABLE messages ADD COLUMN body_loaded INTEGER NOT NULL DEFAULT 1;
    CREATE INDEX messages_nobody ON messages(account_id, body_loaded) WHERE body_loaded = 0;
    "#,
    // 3: attachments, reply/threading headers, per-sender image allowlist.
    r#"
    ALTER TABLE messages ADD COLUMN cc_list TEXT NOT NULL DEFAULT '';
    ALTER TABLE messages ADD COLUMN reply_to TEXT NOT NULL DEFAULT '';
    ALTER TABLE messages ADD COLUMN rfc_message_id TEXT NOT NULL DEFAULT '';
    ALTER TABLE messages ADD COLUMN rfc_references TEXT NOT NULL DEFAULT '';

    CREATE TABLE attachments (
        account_id  INTEGER NOT NULL,
        message_id  TEXT NOT NULL,
        part        INTEGER NOT NULL,        -- index in mail-parser's attachment list
        filename    TEXT NOT NULL,
        mime        TEXT NOT NULL,
        size        INTEGER NOT NULL,
        content_id  TEXT,                    -- normalized, for cid: images
        inline      INTEGER NOT NULL DEFAULT 0,
        data        BLOB,                    -- only small inline images are cached
        PRIMARY KEY (account_id, message_id, part),
        FOREIGN KEY (account_id, message_id) REFERENCES messages(account_id, id) ON DELETE CASCADE
    );

    CREATE TABLE image_senders (
        account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        email       TEXT NOT NULL,
        PRIMARY KEY (account_id, email)
    );

    -- Bodies cached before attachments were indexed: re-download on next open.
    UPDATE messages SET body_loaded = 0
     WHERE body_loaded = 1 AND (has_attachments = 1 OR body_html LIKE '%cid:%');
    "#,
    // 4: multi-account + resumable history download (H1). Each account has its
    // own download window, notification mode and backfill cursor.
    r#"
    ALTER TABLE accounts ADD COLUMN sync_window TEXT NOT NULL DEFAULT '90d';
    ALTER TABLE accounts ADD COLUMN notify TEXT NOT NULL DEFAULT 'all';
    -- setup | pending | running | paused | done
    ALTER TABLE accounts ADD COLUMN backfill_state TEXT NOT NULL DEFAULT 'pending';
    ALTER TABLE accounts ADD COLUMN backfill_page TEXT;
    ALTER TABLE accounts ADD COLUMN backfill_done INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE accounts ADD COLUMN backfill_total INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE accounts ADD COLUMN messages_total INTEGER NOT NULL DEFAULT 0;

    UPDATE accounts SET notify = COALESCE((SELECT value FROM settings WHERE key = 'notifications'), 'all');
    UPDATE accounts SET backfill_state = 'done'
     WHERE EXISTS (SELECT 1 FROM settings WHERE key = 'backfill_done:' || accounts.id AND value = '1');
    "#,
    // 5: the pre-H1 backfill was capped (500 messages, metadata only), so an
    // account it marked done still needs the real download of its range
    // (and its offline bodies). Only rows the new job never touched.
    r#"
    UPDATE accounts SET backfill_state = 'pending'
     WHERE backfill_state = 'done' AND backfill_total = 0 AND backfill_done = 0
       AND backfill_page IS NULL;
    "#,
];

//! Lightweight selection queries: keys only, never message bodies or row rendering data.

use super::{Result, Store, query};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadKey {
    pub account_id: i64,
    pub thread_id: String,
}

impl Store {
    pub fn selection_keys(
        &self,
        account: Option<i64>,
        label: &str,
        category: Option<&str>,
        text: &str,
    ) -> Result<Vec<ThreadKey>> {
        let search = !text.trim().is_empty();
        let q = query::parse(text);
        if search && (!q.local_ok || q.is_empty()) {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT m.account_id, m.thread_id FROM messages m
             WHERE (?1 IS NULL OR m.account_id = ?1)
               AND ((?2 = 0
                 AND EXISTS (SELECT 1 FROM message_labels ml WHERE ml.account_id=m.account_id AND ml.message_id=m.id AND ml.label_id=?3)
                 AND (?4 IS NULL OR COALESCE((SELECT c.label_id FROM message_labels c
                     WHERE c.account_id=m.account_id AND c.message_id=m.id
                     AND c.label_id IN ('CATEGORY_SOCIAL','CATEGORY_PROMOTIONS','CATEGORY_UPDATES','CATEGORY_FORUMS') LIMIT 1), 'CATEGORY_PERSONAL')=?4))
                OR (?2 = 1
                 AND (?5 = '' OR m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?5))
                 AND (?6 IS NULL OR m.is_unread=?6)
                 AND (?7 = 0 OR m.is_starred=1) AND (?8 = 0 OR m.has_attachments=1)
                 AND NOT EXISTS (SELECT 1 FROM message_labels ml WHERE ml.account_id=m.account_id AND ml.message_id=m.id AND ml.label_id IN ('SPAM','TRASH'))))
             ORDER BY m.account_id, m.thread_id"
        )?;
        stmt.query_map(
            params![
                account,
                search,
                label,
                category,
                q.fts,
                q.unread.map(i64::from),
                q.starred,
                q.attachment
            ],
            |r| {
                Ok(ThreadKey {
                    account_id: r.get(0)?,
                    thread_id: r.get(1)?,
                })
            },
        )?
        .collect()
    }

    pub fn thread_has_unread(&self, account: i64, thread: &str) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE account_id=?1 AND thread_id=?2 AND is_unread=1)",
            params![account, thread],
            |r| r.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::store::{NewMessage, Store};

    #[test]
    fn selection_matches_folder_category_and_search_without_page_limit() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.upsert_account("one@example.test", 0).unwrap().id;
        let b = s.upsert_account("two@example.test", 0).unwrap().id;
        for i in 0..240 {
            s.upsert_message(
                a,
                &NewMessage {
                    id: format!("m{i}"),
                    thread_id: format!("t{i}"),
                    subject: "فاتورة invoice".into(),
                    label_ids: vec!["INBOX".into(), "UNREAD".into()],
                    ..Default::default()
                },
            )
            .unwrap();
        }
        s.upsert_message(
            b,
            &NewMessage {
                id: "b".into(),
                thread_id: "t0".into(),
                subject: "invoice".into(),
                label_ids: vec!["INBOX".into(), "CATEGORY_PROMOTIONS".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let all = s.selection_keys(None, "INBOX", None, "").unwrap();
        assert_eq!(all.len(), 241);
        assert_eq!(s.selection_keys(Some(a), "INBOX", None, "").unwrap().len(), 240);
        assert_eq!(
            s.selection_keys(None, "INBOX", Some("CATEGORY_PERSONAL"), "")
                .unwrap()
                .len(),
            240
        );
        assert_eq!(
            s.selection_keys(None, "INBOX", Some("CATEGORY_PROMOTIONS"), "")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            s.selection_keys(Some(a), "INBOX", None, "invoice is:unread")
                .unwrap()
                .len(),
            240
        );
        let first = s.search_page(Some(a), "invoice", 200, 0).unwrap();
        let last = s.search_page(Some(a), "invoice", 200, 200).unwrap();
        assert_eq!((first.len(), last.len()), (200, 40));
        assert!(last.iter().all(|t| !first.iter().any(|f| f.thread_id == t.thread_id)));
        assert!(
            s.selection_keys(None, "INBOX", None, "in:sent invoice")
                .unwrap()
                .is_empty()
        );
        s.modify_thread_labels(a, "t0", &[], &["UNREAD".into()]).unwrap();
        assert!(!s.thread_has_unread(a, "t0").unwrap());
        assert!(s.thread_has_unread(a, "t1").unwrap());
        assert_eq!(
            s.selection_keys(Some(a), "INBOX", None, "invoice is:unread")
                .unwrap()
                .len(),
            239
        );
        // A selection is a frozen set of keys; later mail never joins it.
        s.upsert_message(
            a,
            &NewMessage {
                id: "later".into(),
                thread_id: "later".into(),
                label_ids: vec!["INBOX".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(all.len(), 241);
    }
}

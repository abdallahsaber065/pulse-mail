//! Minimal Gmail REST v1 client: just the calls the sync engine needs.
//! https://developers.google.com/workspace/gmail/api/reference/rest

use crate::auth::secret::Secret;
use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use serde::Deserialize;

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

#[derive(Debug, thiserror::Error)]
pub enum GmailError {
    /// historyId too old / unknown: caller must do a full resync.
    #[error("history id expired")]
    HistoryExpired,
    #[error("access token rejected")]
    Unauthorized,
    #[error("rate limited")]
    RateLimited,
    #[error("Gmail API error {0}")]
    Status(u16),
    #[error("network error")]
    Network(#[from] reqwest::Error),
    #[error("could not decode message")]
    Decode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub email_address: String,
    pub history_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub messages_total: i64,
    #[serde(default)]
    pub messages_unread: i64,
}

#[derive(Debug, Deserialize)]
struct LabelList {
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageList {
    #[serde(default)]
    messages: Vec<MessageRef>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMessage {
    pub id: String,
    pub thread_id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub snippet: String,
    /// ms since epoch, as a string
    #[serde(default)]
    pub internal_date: String,
    /// base64url RFC 822 bytes
    #[serde(default)]
    pub raw: String,
}

impl RawMessage {
    pub fn decode_raw(&self) -> Result<Vec<u8>, GmailError> {
        let s = self.raw.trim_end_matches('=');
        URL_SAFE_NO_PAD
            .decode(s)
            .or_else(|_| URL_SAFE.decode(&self.raw))
            .map_err(|_| GmailError::Decode)
    }
    pub fn internal_date_ms(&self) -> i64 {
        self.internal_date.parse().unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    #[serde(default)]
    pub history: Vec<HistoryRecord>,
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub history_id: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    #[serde(default)]
    pub messages_added: Vec<MessageWrap>,
    #[serde(default)]
    pub messages_deleted: Vec<MessageWrap>,
    #[serde(default)]
    pub labels_added: Vec<MessageWrap>,
    #[serde(default)]
    pub labels_removed: Vec<MessageWrap>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MessageWrap {
    pub message: MessageRef,
}

/// The net effect of a run of history records.
#[derive(Debug, Default, PartialEq)]
pub struct HistoryDelta {
    /// Newly added message ids (to fetch and maybe notify), oldest first.
    pub added: Vec<String>,
    /// Deleted message ids.
    pub deleted: Vec<String>,
    /// Messages whose labels changed (id, latest label set).
    pub relabeled: Vec<(String, Vec<String>)>,
}

impl HistoryDelta {
    pub fn from_records(records: &[HistoryRecord]) -> Self {
        use std::collections::{HashMap, HashSet};
        let mut added: Vec<String> = Vec::new();
        let mut added_set: HashSet<String> = HashSet::new();
        let mut deleted: HashSet<String> = HashSet::new();
        let mut relabeled: HashMap<String, Vec<String>> = HashMap::new();
        let mut relabel_order: Vec<String> = Vec::new();

        for r in records {
            for m in &r.messages_added {
                if added_set.insert(m.message.id.clone()) {
                    added.push(m.message.id.clone());
                }
                deleted.remove(&m.message.id);
            }
            for m in &r.messages_deleted {
                deleted.insert(m.message.id.clone());
            }
            for m in r.labels_added.iter().chain(&r.labels_removed) {
                if !relabeled.contains_key(&m.message.id) {
                    relabel_order.push(m.message.id.clone());
                }
                relabeled.insert(m.message.id.clone(), m.message.label_ids.clone());
            }
        }
        let added: Vec<String> = added.into_iter().filter(|id| !deleted.contains(id)).collect();
        let relabeled = relabel_order
            .into_iter()
            .filter(|id| !deleted.contains(id) && !added_set.contains(id))
            .map(|id| {
                let labels = relabeled.remove(&id).unwrap_or_default();
                (id, labels)
            })
            .collect();
        let mut deleted: Vec<String> = deleted.into_iter().collect();
        deleted.sort();
        Self {
            added,
            deleted,
            relabeled,
        }
    }
}

pub struct Gmail {
    http: reqwest::Client,
}

impl Gmail {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        token: &Secret,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, GmailError> {
        let resp = self
            .http
            .get(format!("{BASE}{path}"))
            .bearer_auth(token.expose())
            .query(query)
            .send()
            .await?;
        match resp.status().as_u16() {
            200..=299 => Ok(resp.json().await?),
            401 => Err(GmailError::Unauthorized),
            404 if path == "/history" => Err(GmailError::HistoryExpired),
            429 => Err(GmailError::RateLimited),
            s => Err(GmailError::Status(s)),
        }
    }

    pub async fn profile(&self, token: &Secret) -> Result<Profile, GmailError> {
        self.get(token, "/profile", &[]).await
    }

    pub async fn labels(&self, token: &Secret) -> Result<Vec<Label>, GmailError> {
        Ok(self.get::<LabelList>(token, "/labels", &[]).await?.labels)
    }

    /// Ids of the newest messages matching `q` (e.g. `in:inbox newer_than:30d`).
    pub async fn list_message_ids(&self, token: &Secret, q: &str, max: usize) -> Result<Vec<MessageRef>, GmailError> {
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        while out.len() < max {
            let per_page = (max - out.len()).min(500).to_string();
            let mut query = vec![("q", q), ("maxResults", per_page.as_str())];
            if let Some(p) = page.as_deref() {
                query.push(("pageToken", p));
            }
            let list: MessageList = self.get(token, "/messages", &query).await?;
            out.extend(list.messages);
            match list.next_page_token {
                Some(p) => page = Some(p),
                None => break,
            }
        }
        out.truncate(max);
        Ok(out)
    }

    pub async fn get_raw(&self, token: &Secret, id: &str) -> Result<RawMessage, GmailError> {
        self.get(token, &format!("/messages/{id}"), &[("format", "raw")]).await
    }

    /// All history pages since `start_history_id`. Returns the records and
    /// the newest history id to persist.
    pub async fn history_since(
        &self,
        token: &Secret,
        start_history_id: &str,
    ) -> Result<(Vec<HistoryRecord>, String), GmailError> {
        let mut records = Vec::new();
        let mut page: Option<String> = None;
        let mut latest = start_history_id.to_string();
        loop {
            let mut query = vec![("startHistoryId", start_history_id), ("maxResults", "500")];
            if let Some(p) = page.as_deref() {
                query.push(("pageToken", p));
            }
            let hp: HistoryPage = self.get(token, "/history", &query).await?;
            if !hp.history_id.is_empty() {
                latest = hp.history_id;
            }
            records.extend(hp.history);
            match hp.next_page_token {
                Some(p) => page = Some(p),
                None => break,
            }
        }
        Ok((records, latest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_history_page_and_folds_delta() {
        let json = r#"{
          "history": [
            {"id":"10","messagesAdded":[{"message":{"id":"a","threadId":"t","labelIds":["INBOX","UNREAD"]}}]},
            {"id":"11","labelsRemoved":[{"message":{"id":"old","threadId":"t0","labelIds":["INBOX"]},"labelIds":["UNREAD"]}]},
            {"id":"12","messagesAdded":[{"message":{"id":"b","threadId":"t","labelIds":["INBOX"]}}]},
            {"id":"13","messagesDeleted":[{"message":{"id":"b","threadId":"t"}}]},
            {"id":"14","labelsAdded":[{"message":{"id":"a","threadId":"t","labelIds":["INBOX","STARRED"]},"labelIds":["STARRED"]}]}
          ],
          "historyId":"14"
        }"#;
        let page: HistoryPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.history_id, "14");
        let d = HistoryDelta::from_records(&page.history);
        assert_eq!(d.added, vec!["a".to_string()]);
        assert_eq!(d.deleted, vec!["b".to_string()]);
        // "a" is fetched fresh anyway, so only "old" needs a label update
        assert_eq!(d.relabeled, vec![("old".to_string(), vec!["INBOX".to_string()])]);
    }

    #[test]
    fn empty_history_page() {
        let page: HistoryPage = serde_json::from_str(r#"{"historyId":"99"}"#).unwrap();
        assert!(page.history.is_empty());
        assert_eq!(HistoryDelta::from_records(&page.history), HistoryDelta::default());
    }

    #[test]
    fn decodes_raw_base64url() {
        let m = RawMessage {
            id: "x".into(),
            thread_id: "t".into(),
            label_ids: vec![],
            snippet: String::new(),
            internal_date: "1729500000000".into(),
            raw: URL_SAFE.encode(b"Subject: hi\r\n\r\nbody?>"),
        };
        assert_eq!(m.decode_raw().unwrap(), b"Subject: hi\r\n\r\nbody?>");
        assert_eq!(m.internal_date_ms(), 1_729_500_000_000);
    }
}

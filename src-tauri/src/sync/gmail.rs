//! Minimal Gmail REST v1 client: just the calls the sync engine needs.
//! https://developers.google.com/workspace/gmail/api/reference/rest

use crate::auth::secret::Secret;
use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use serde::Deserialize;

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const UPLOAD_BASE: &str = "https://gmail.googleapis.com/upload/gmail/v1/users/me";
/// Drafts bigger than this use the upload endpoint.
const UPLOAD_ABOVE: usize = 3 * 1024 * 1024;

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
    #[serde(default)]
    pub messages_total: i64,
    #[serde(default)]
    pub threads_total: i64,
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
pub struct DraftRef {
    pub id: String,
    pub message: MessageRef,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageList {
    #[serde(default)]
    messages: Vec<MessageRef>,
    next_page_token: Option<String>,
    #[serde(default)]
    result_size_estimate: i64,
}

/// One page of `messages.list`.
#[derive(Debug)]
pub struct MessagePage {
    pub ids: Vec<String>,
    pub next: Option<String>,
    /// Gmail's (rough) estimate of the total number of matches.
    pub estimate: i64,
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
    #[serde(default)]
    pub size_estimate: i64,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaMessage {
    pub id: String,
    pub thread_id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub internal_date: String,
    #[serde(default)]
    pub size_estimate: i64,
    #[serde(default)]
    pub payload: MetaPayload,
}

#[derive(Debug, Deserialize, Default)]
pub struct MetaPayload {
    #[serde(default)]
    pub headers: Vec<Header>,
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl MetaMessage {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.payload
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
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

const BATCH_URL: &str = "https://gmail.googleapis.com/batch/gmail/v1";
/// Google recommends at most 50 calls per batch; more trips rate limits.
pub const BATCH_MAX: usize = 50;
/// Query for metadata-only gets: the three headers the list needs + snippet.
const META_QUERY: &str = "format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Subject\
                          &metadataHeaders=Content-Type\
                          &fields=id,threadId,labelIds,snippet,internalDate,sizeEstimate,payload/headers";

/// Split a `multipart/mixed` batch response into per-request results, ordered
/// by our `Content-ID: <m{i}>` (Google answers `<response-m{i}>`). Missing or
/// unparseable parts become `Err(0)`.
fn parse_batch_response(content_type: &str, body: &str, n: usize) -> Vec<Result<MetaMessage, u16>> {
    let mut out: Vec<Result<MetaMessage, u16>> = (0..n).map(|_| Err(0)).collect();
    let Some(boundary) = content_type
        .split(';')
        .filter_map(|p| p.trim().strip_prefix("boundary="))
        .map(|b| b.trim_matches('"'))
        .next()
    else {
        return out;
    };
    let delim = format!("--{boundary}");
    for part in body.split(delim.as_str()) {
        let Some(idx) = part
            .find("<response-m")
            .and_then(|p| part[p + 11..].split('>').next())
            .and_then(|d| d.parse::<usize>().ok())
            .filter(|&i| i < n)
        else {
            continue;
        };
        // Inner response: status line, headers, blank line, JSON.
        let Some(http) = part.find("HTTP/") else { continue };
        let inner = &part[http..];
        let status = inner
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        if !(200..300).contains(&status) {
            out[idx] = Err(status);
            continue;
        }
        let json = inner
            .split_once("\r\n\r\n")
            .or_else(|| inner.split_once("\n\n"))
            .map(|(_, b)| b.trim())
            .unwrap_or_default();
        out[idx] = serde_json::from_str(json).map_err(|_| 0);
    }
    out
}

pub struct Gmail {
    http: reqwest::Client,
}

/// Map a non-2xx response to an error. Gmail reports quota/concurrency limits
/// either as 429 or as 403 with reason `rateLimitExceeded` /
/// `userRateLimitExceeded`; both are retryable. Only Google's machine-readable
/// reason is logged (never headers or tokens).
async fn classify_error(resp: reqwest::Response) -> GmailError {
    let status = resp.status().as_u16();
    if status == 401 {
        return GmailError::Unauthorized;
    }
    let body = resp.text().await.unwrap_or_default();
    let reason = error_reason(&body);
    if status == 429 || (status == 403 && reason.as_deref().is_some_and(|r| r.contains("ateLimitExceeded"))) {
        return GmailError::RateLimited;
    }
    tracing::warn!(status, reason = reason.as_deref().unwrap_or("-"), "Gmail API error");
    GmailError::Status(status)
}

fn error_reason(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let e = &v["error"];
    e["errors"][0]["reason"]
        .as_str()
        .or_else(|| e["status"].as_str())
        .map(str::to_owned)
}

impl Gmail {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// GET with retry on 429 / 5xx (exponential backoff, 3 attempts), as
    /// Gmail's quota docs recommend.
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        token: &Secret,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, GmailError> {
        let mut delay = std::time::Duration::from_millis(400);
        let mut attempt = 0;
        loop {
            match self.get_once(token, path, query).await {
                Err(GmailError::RateLimited | GmailError::Status(500..=599)) if attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
                r => return r,
            }
        }
    }

    async fn get_once<T: serde::de::DeserializeOwned>(
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
            404 if path == "/history" => Err(GmailError::HistoryExpired),
            _ => Err(classify_error(resp).await),
        }
    }

    pub async fn profile(&self, token: &Secret) -> Result<Profile, GmailError> {
        self.get(token, "/profile", &[]).await
    }

    pub async fn labels(&self, token: &Secret) -> Result<Vec<Label>, GmailError> {
        Ok(self.get::<LabelList>(token, "/labels", &[]).await?.labels)
    }

    /// Ids of the newest messages, optionally limited to a label and/or a
    /// search query (e.g. `newer_than:60d`). Newest first.
    pub async fn list_message_ids(
        &self,
        token: &Secret,
        label: Option<&str>,
        q: Option<&str>,
        max: usize,
    ) -> Result<Vec<MessageRef>, GmailError> {
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        while out.len() < max {
            let per_page = (max - out.len()).min(500).to_string();
            let mut query = vec![("maxResults", per_page.as_str())];
            if let Some(l) = label {
                query.push(("labelIds", l));
            }
            if let Some(q) = q {
                query.push(("q", q));
            }
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

    /// One page (up to 500 ids) of messages matching `q`, newest first.
    pub async fn list_page(&self, token: &Secret, q: &str, page: Option<&str>, max: u32) -> Result<MessagePage, GmailError> {
        let max = max.clamp(1, 500).to_string();
        let mut query = vec![("maxResults", max.as_str())];
        if !q.is_empty() {
            query.push(("q", q));
        }
        if let Some(p) = page {
            query.push(("pageToken", p));
        }
        let list: MessageList = self.get(token, "/messages", &query).await?;
        Ok(MessagePage {
            ids: list.messages.into_iter().map(|m| m.id).collect(),
            next: list.next_page_token,
            estimate: list.result_size_estimate,
        })
    }

    pub async fn get_raw(&self, token: &Secret, id: &str) -> Result<RawMessage, GmailError> {
        self.get(token, &format!("/messages/{id}"), &[("format", "raw")]).await
    }

    /// Headers + snippet + labels only: ~1-2 KB instead of the full message.
    pub async fn get_metadata(&self, token: &Secret, id: &str) -> Result<MetaMessage, GmailError> {
        self.get(
            token,
            &format!("/messages/{id}"),
            &[
                ("format", "metadata"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "To"),
                ("metadataHeaders", "Subject"),
                ("metadataHeaders", "Content-Type"),
                ("fields", "id,threadId,labelIds,snippet,internalDate,sizeEstimate,payload/headers"),
            ], // keep in sync with META_QUERY
        )
        .await
    }

    /// Up to 50 metadata gets in ONE HTTP round trip via Gmail's batch
    /// endpoint. Quota is still charged per inner request (5 units each).
    /// Returns one entry per id, in order: the message or the inner HTTP
    /// status (e.g. 404 deleted, 429 throttled -> caller retries singly).
    pub async fn get_metadata_batch(
        &self,
        token: &Secret,
        ids: &[String],
    ) -> Result<Vec<Result<MetaMessage, u16>>, GmailError> {
        debug_assert!(ids.len() <= BATCH_MAX);
        let boundary = "pulse_mail_batch";
        let mut body = String::with_capacity(ids.len() * 220);
        for (i, id) in ids.iter().enumerate() {
            // Ids are hex from Gmail; refuse anything else rather than escape it.
            if !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Err(GmailError::Status(400));
            }
            body.push_str(&format!(
                "--{boundary}\r\nContent-Type: application/http\r\nContent-ID: <m{i}>\r\n\r\n\
                 GET /gmail/v1/users/me/messages/{id}?{META_QUERY}\r\n\r\n"
            ));
        }
        body.push_str(&format!("--{boundary}--\r\n"));
        let resp = self
            .http
            .post(BATCH_URL)
            .bearer_auth(token.expose())
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/mixed; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(classify_error(resp).await);
        }
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let text = resp.text().await?;
        Ok(parse_batch_response(&ct, &text, ids.len()))
    }

    /// Add/remove labels on a whole thread (archive, star, read/unread).
    pub async fn modify_thread(
        &self,
        token: &Secret,
        thread_id: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<(), GmailError> {
        let resp = self
            .http
            .post(format!("{BASE}/threads/{thread_id}/modify"))
            .bearer_auth(token.expose())
            .json(&serde_json::json!({ "addLabelIds": add, "removeLabelIds": remove }))
            .send()
            .await?;
        match resp.status().as_u16() {
            200..=299 => Ok(()),
            _ => Err(classify_error(resp).await),
        }
    }

    /// POST/PUT a JSON body (no automatic retry: sends must not repeat).
    async fn write_json<T: serde::de::DeserializeOwned>(
        &self,
        token: &Secret,
        method: reqwest::Method,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, GmailError> {
        let resp = self
            .http
            .request(method, format!("{BASE}{path}"))
            .bearer_auth(token.expose())
            .json(body)
            .send()
            .await?;
        match resp.status().as_u16() {
            200..=299 => Ok(resp.json().await?),
            _ => Err(classify_error(resp).await),
        }
    }

    /// Create (`draft_id` None) or replace a draft with this RFC 822 message.
    pub async fn save_draft(
        &self,
        token: &Secret,
        draft_id: Option<&str>,
        raw: &[u8],
        thread_id: Option<&str>,
    ) -> Result<DraftRef, GmailError> {
        // Gmail's JSON endpoint takes messages up to ~5 MB; bigger ones
        // (attachments) go through the upload endpoint.
        if raw.len() > UPLOAD_ABOVE {
            return self.upload_draft(token, draft_id, raw, thread_id).await;
        }
        let mut message = serde_json::json!({ "raw": URL_SAFE_NO_PAD.encode(raw) });
        if let Some(t) = thread_id {
            message["threadId"] = t.into();
        }
        let body = serde_json::json!({ "message": message });
        match draft_id {
            Some(id) => self.write_json(token, reqwest::Method::PUT, &format!("/drafts/{id}"), &body).await,
            None => self.write_json(token, reqwest::Method::POST, "/drafts", &body).await,
        }
    }

    /// drafts.create / drafts.update via `uploadType=multipart`
    /// (metadata JSON + the message as `message/rfc822`), up to 35 MB.
    async fn upload_draft(
        &self,
        token: &Secret,
        draft_id: Option<&str>,
        raw: &[u8],
        thread_id: Option<&str>,
    ) -> Result<DraftRef, GmailError> {
        let mut meta = serde_json::json!({ "message": {} });
        if let Some(t) = thread_id {
            meta["message"]["threadId"] = t.into();
        }
        if let Some(id) = draft_id {
            meta["id"] = id.into();
        }
        let boundary = "pulse_mail_upload_7d1f";
        let mut body = format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{boundary}\r\nContent-Type: message/rfc822\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(raw);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());
        let (method, url) = match draft_id {
            Some(id) => (reqwest::Method::PUT, format!("{UPLOAD_BASE}/drafts/{id}?uploadType=multipart")),
            None => (reqwest::Method::POST, format!("{UPLOAD_BASE}/drafts?uploadType=multipart")),
        };
        let resp = self
            .http
            .request(method, url)
            .bearer_auth(token.expose())
            .header("Content-Type", format!("multipart/related; boundary={boundary}"))
            .timeout(std::time::Duration::from_secs(300))
            .body(body)
            .send()
            .await?;
        match resp.status().as_u16() {
            200..=299 => Ok(resp.json().await?),
            _ => Err(classify_error(resp).await),
        }
    }

    /// Send a saved draft (Gmail moves it to Sent).
    pub async fn send_draft(&self, token: &Secret, draft_id: &str) -> Result<MessageRef, GmailError> {
        self.write_json(token, reqwest::Method::POST, "/drafts/send", &serde_json::json!({ "id": draft_id }))
            .await
    }

    /// Delete a draft (only drafts this app created in a compose session the
    /// user discarded).
    pub async fn delete_draft(&self, token: &Secret, draft_id: &str) -> Result<(), GmailError> {
        let resp = self
            .http
            .delete(format!("{BASE}/drafts/{draft_id}"))
            .bearer_auth(token.expose())
            .send()
            .await?;
        match resp.status().as_u16() {
            200..=299 | 404 => Ok(()),
            _ => Err(classify_error(resp).await),
        }
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
    fn parses_batch_response_out_of_order_with_errors() {
        let ct = "multipart/mixed; boundary=batch_abc";
        let body = "--batch_abc\r\nContent-Type: application/http\r\nContent-ID: <response-m1>\r\n\r\n\
            HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n\
            {\"id\":\"b2\",\"threadId\":\"t\",\"labelIds\":[\"INBOX\"],\"snippet\":\"hi\",\"internalDate\":\"5\",\
            \"payload\":{\"headers\":[{\"name\":\"Subject\",\"value\":\"S\"}]}}\r\n\
            --batch_abc\r\nContent-Type: application/http\r\nContent-ID: <response-m0>\r\n\r\n\
            HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n\r\n{\"error\":{}}\r\n\
            --batch_abc--\r\n";
        let r = parse_batch_response(ct, body, 3);
        assert_eq!(r[0].as_ref().err(), Some(&404));
        let m = r[1].as_ref().expect("parsed");
        assert_eq!(m.id, "b2");
        assert_eq!(m.header("subject"), Some("S"));
        assert_eq!(r[2].as_ref().err(), Some(&0));
        // No boundary -> all missing, never panics.
        assert!(
            parse_batch_response("text/html", "<html>", 2)
                .iter()
                .all(|x| x.is_err())
        );
    }

    #[test]
    fn reads_google_error_reason() {
        let body =
            r#"{"error":{"code":403,"errors":[{"reason":"userRateLimitExceeded"}],"status":"PERMISSION_DENIED"}}"#;
        assert_eq!(error_reason(body).as_deref(), Some("userRateLimitExceeded"));
        assert_eq!(
            error_reason(r#"{"error":{"status":"FAILED_PRECONDITION"}}"#).as_deref(),
            Some("FAILED_PRECONDITION")
        );
        assert_eq!(error_reason("<html>"), None);
    }

    #[test]
    fn parses_metadata_message() {
        let json = r#"{"id":"m","threadId":"t","labelIds":["INBOX","UNREAD"],"snippet":"Your code is 1234",
            "internalDate":"1729500000000","payload":{"headers":[
              {"name":"From","value":"\"GitHub\" <noreply@github.com>"},{"name":"subject","value":"Verify"}]}}"#;
        let m: MetaMessage = serde_json::from_str(json).unwrap();
        assert_eq!(m.header("Subject"), Some("Verify"));
        assert_eq!(m.header("from"), Some("\"GitHub\" <noreply@github.com>"));
        assert_eq!(m.header("To"), None);
        assert_eq!(m.internal_date_ms(), 1_729_500_000_000);
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
            size_estimate: 0,
            raw: URL_SAFE.encode(b"Subject: hi\r\n\r\nbody?>"),
        };
        assert_eq!(m.decode_raw().unwrap(), b"Subject: hi\r\n\r\nbody?>");
        assert_eq!(m.internal_date_ms(), 1_729_500_000_000);
    }
}

//! Blocking the author of a community item (`/v1/community/blocks`), so the signed-in user stops seeing that author's works in every community catalog.
//!
//! A block names an item, not a person: the server resolves the item's owner and never returns the owner's account id. Blocking needs the signed-in session; the device's anonymous account does not count, because catalogs are filtered by the token the list request carries, and an anonymous block would neither follow the user to another device nor survive a reinstall. Blocking the same author again answers the existing block.

use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::community::report::CommunityReportKind;
use crate::community::valid_text;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Most authors one account may block; the server answers `409 block_limit_reached` past it.
pub const MAX_COMMUNITY_BLOCKS: usize = 500;

/// One blocked author: the block's own id (what unblocking names), the author's display name as it reads now, and when the block was made (RFC 3339).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityBlock {
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
}

impl CommunityBlock {
    fn validate(&self) -> Result<(), AccountError> {
        if self.id.is_nil()
            || !valid_text(&self.name, 1, 128, false)
            || !valid_text(&self.created_at, 1, 64, false)
        {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct BlockRequest<'a> {
    kind: CommunityReportKind,
    item_id: &'a Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockResponse {
    block: CommunityBlock,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlocksResponse {
    items: Vec<CommunityBlock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnblockResponse {
    blocked: bool,
}

pub trait CommunityBlockApi: Send + Sync + 'static {
    fn block_community_author(
        &self,
        kind: CommunityReportKind,
        item_id: Uuid,
        token: &str,
    ) -> Result<CommunityBlock, AccountError>;
    fn community_blocks(&self, token: &str) -> Result<Vec<CommunityBlock>, AccountError>;
    fn unblock_community_author(&self, id: Uuid, token: &str) -> Result<(), AccountError>;
}

impl CommunityBlockApi for BackendAccountClient {
    fn block_community_author(
        &self,
        kind: CommunityReportKind,
        item_id: Uuid,
        token: &str,
    ) -> Result<CommunityBlock, AccountError> {
        if item_id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let response = self.json::<BlockResponse, _>(
            Method::POST,
            "/v1/community/blocks",
            Some(token),
            Some(&BlockRequest {
                kind,
                item_id: &item_id,
            }),
        )?;
        response.block.validate()?;
        Ok(response.block)
    }

    fn community_blocks(&self, token: &str) -> Result<Vec<CommunityBlock>, AccountError> {
        let response = self.json::<BlocksResponse, ()>(
            Method::GET,
            "/v1/community/blocks",
            Some(token),
            None,
        )?;
        if response.items.len() > MAX_COMMUNITY_BLOCKS {
            return Err(AccountError::Unavailable);
        }
        for block in &response.items {
            block.validate()?;
        }
        Ok(response.items)
    }

    fn unblock_community_author(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let response = self.json::<UnblockResponse, ()>(
            Method::DELETE,
            &format!("/v1/community/blocks/{id}"),
            Some(token),
            None,
        )?;
        if response.blocked {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }
}

pub struct BackendCommunityBlockService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCommunityBlockService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCommunityBlockService<A, S>
where
    A: AccountApi + CommunityBlockApi,
    S: AccountSessionStorage,
{
    /// Blocks the author of the item `item_id` of `kind`. `Unauthorized` without a signed-in session, `Invalid` for one's own item or a malformed id, `NotFound` when the item is gone or not visible, `Conflict` past [`MAX_COMMUNITY_BLOCKS`].
    pub fn block(
        &self,
        kind: CommunityReportKind,
        item_id: &str,
    ) -> Result<CommunityBlock, AccountError> {
        let item_id = Uuid::parse_str(item_id).map_err(|_| AccountError::Invalid)?;
        if item_id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.block_community_author(kind, item_id, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// The signed-in account's blocked authors, newest first.
    pub fn list(&self) -> Result<Vec<CommunityBlock>, AccountError> {
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.community_blocks(token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// Lifts the block `id`; lifting one that is already gone succeeds.
    pub fn unblock(&self, id: &str) -> Result<(), AccountError> {
        let id = Uuid::parse_str(id).map_err(|_| AccountError::Invalid)?;
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.unblock_community_author(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    const BLOCK: &str = "20000000-0000-4000-8000-000000000002";

    fn item() -> Uuid {
        Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap()
    }

    fn serve(status: &'static str, body: String) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (sent, received) = mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut request = vec![0; length];
            reader.read_exact(&mut request).unwrap();
            sent.send(format!("{head}\n{}", String::from_utf8(request).unwrap()))
                .unwrap();
            write!(
                reader.get_mut(),
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        (origin, received)
    }

    fn token() -> String {
        "a".repeat(43)
    }

    fn block_json(name: &str) -> String {
        format!(r#"{{"id":"{BLOCK}","name":"{name}","created_at":"2026-10-31T08:00:00Z"}}"#)
    }

    #[test]
    fn blocking_posts_the_item_and_reads_back_the_author() {
        let (origin, received) = serve(
            "201 Created",
            format!(r#"{{"block":{}}}"#, block_json("小鹿")),
        );
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let block = client
            .block_community_author(CommunityReportKind::CandidateSkins, item(), &token())
            .unwrap();
        assert_eq!(block.id.to_string(), BLOCK);
        assert_eq!(block.name, "小鹿");
        let request = received.recv().unwrap();
        assert!(request.starts_with("POST /v1/community/blocks HTTP/1.1"));
        assert!(request.contains(&format!("authorization: Bearer {}", token())));
        let body: serde_json::Value =
            serde_json::from_str(request.rsplit_once('\n').unwrap().1).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "kind": "candidate-skins", "item_id": item().hyphenated().to_string() })
        );

        // Blocking an author who is already blocked answers 200 with the same block.
        let (origin, _received) = serve("200 OK", format!(r#"{{"block":{}}}"#, block_json("小鹿")));
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert!(client
            .block_community_author(CommunityReportKind::Phrases, item(), &token())
            .is_ok());
    }

    #[test]
    fn the_list_and_unblocking_use_the_block_id() {
        let (origin, received) = serve(
            "200 OK",
            format!(r#"{{"items":[{},{}]}}"#, block_json("甲"), block_json("乙")),
        );
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let blocks = client.community_blocks(&token()).unwrap();
        assert_eq!(
            blocks
                .iter()
                .map(|block| block.name.as_str())
                .collect::<Vec<_>>(),
            ["甲", "乙"]
        );
        assert!(received
            .recv()
            .unwrap()
            .starts_with("GET /v1/community/blocks HTTP/1.1"));

        let (origin, received) = serve("200 OK", r#"{"blocked":false}"#.into());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        client
            .unblock_community_author(Uuid::parse_str(BLOCK).unwrap(), &token())
            .unwrap();
        assert!(received
            .recv()
            .unwrap()
            .starts_with(&format!("DELETE /v1/community/blocks/{BLOCK} HTTP/1.1")));
    }

    #[test]
    fn malformed_answers_are_unavailable() {
        for body in [
            r#"{"block":{"id":"00000000-0000-0000-0000-000000000000","name":"甲","created_at":"2026-10-31T08:00:00Z"}}"#.to_owned(),
            r#"{"block":{"id":"20000000-0000-4000-8000-000000000002","name":"","created_at":"2026-10-31T08:00:00Z"}}"#.to_owned(),
            format!(r#"{{"block":{},"owner_id":"u"}}"#, block_json("甲")),
        ] {
            let (origin, _received) = serve("201 Created", body.clone());
            let client = BackendAccountClient::loopback(&origin).unwrap();
            assert_eq!(
                client.block_community_author(CommunityReportKind::Skins, item(), &token()),
                Err(AccountError::Unavailable),
                "{body}"
            );
        }
        let (origin, _received) = serve("200 OK", r#"{"blocked":true}"#.into());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client.unblock_community_author(Uuid::parse_str(BLOCK).unwrap(), &token()),
            Err(AccountError::Unavailable)
        );
    }

    #[test]
    fn server_refusals_map_to_distinct_errors() {
        for (status, body, expected) in [
            (
                "400 Bad Request",
                r#"{"error":{"code":"cannot_block_self"}}"#,
                AccountError::Invalid,
            ),
            (
                "404 Not Found",
                r#"{"error":{"code":"item_not_found"}}"#,
                AccountError::NotFound,
            ),
            (
                "409 Conflict",
                r#"{"error":{"code":"block_limit_reached"}}"#,
                AccountError::Conflict,
            ),
            (
                "401 Unauthorized",
                r#"{"error":{"code":"user_session_required"}}"#,
                AccountError::Unauthorized,
            ),
            (
                "429 Too Many Requests",
                r#"{"error":{"code":"rate_limited"}}"#,
                AccountError::RateLimited,
            ),
        ] {
            let (origin, _received) = serve(status, body.into());
            let client = BackendAccountClient::loopback(&origin).unwrap();
            assert_eq!(
                client.block_community_author(CommunityReportKind::Plugins, item(), &token()),
                Err(expected),
                "{status}"
            );
        }
    }

    #[test]
    fn nil_ids_never_leave_the_device() {
        let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();
        assert_eq!(
            client.block_community_author(CommunityReportKind::Skins, Uuid::nil(), &token()),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            client.unblock_community_author(Uuid::nil(), &token()),
            Err(AccountError::Invalid)
        );
    }
}

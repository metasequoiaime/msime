//! Reporting another user's community item to the moderators (`POST /v1/community/reports`).
//!
//! Reporting needs a signed-in session; the device's anonymous account counts. The reason is one of a fixed list ([`CommunityReportReason`]) sent as its Chinese label, with an optional free-text detail. Reporting the same item again is accepted without a second record.

use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::community::valid_text;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Longest detail the server accepts, in characters.
pub const MAX_REPORT_DETAIL_CHARS: usize = 1000;

/// The kind of item reported, as the server names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommunityReportKind {
    /// Touch keyboard skins.
    Skins,
    /// Candidate window skin packages.
    CandidateSkins,
    /// Plugin packs.
    Plugins,
    /// Shared dictionaries (community resources of kind `dictionary`).
    Dictionaries,
    /// Shared reply templates (community resources of kind `reply`).
    Replies,
}

/// The fixed reasons a report offers, serialized as the label the moderators see.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunityReportReason {
    #[serde(rename = "侵权/抄袭")]
    Infringement,
    #[serde(rename = "色情低俗")]
    Sexual,
    #[serde(rename = "违法违规")]
    Illegal,
    #[serde(rename = "垃圾广告")]
    Spam,
    #[serde(rename = "恶意插件")]
    MaliciousPlugin,
    #[serde(rename = "其他")]
    Other,
}

impl CommunityReportReason {
    /// Every reason, in the order a report dialog lists them.
    pub const ALL: [Self; 6] = [
        Self::Infringement,
        Self::Sexual,
        Self::Illegal,
        Self::Spam,
        Self::MaliciousPlugin,
        Self::Other,
    ];

    /// The label shown to the user and sent to the server.
    pub fn label(self) -> &'static str {
        match self {
            Self::Infringement => "侵权/抄袭",
            Self::Sexual => "色情低俗",
            Self::Illegal => "违法违规",
            Self::Spam => "垃圾广告",
            Self::MaliciousPlugin => "恶意插件",
            Self::Other => "其他",
        }
    }

    /// The reason whose label is `label`.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.label() == label)
    }
}

/// One report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommunityReport {
    pub kind: CommunityReportKind,
    pub item_id: Uuid,
    pub reason: CommunityReportReason,
    /// Optional; at most [`MAX_REPORT_DETAIL_CHARS`] characters, line breaks allowed.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl CommunityReport {
    /// The report with its detail trimmed, or `Invalid` when the item id is nil or the detail is too long or holds control characters.
    pub fn new(
        kind: CommunityReportKind,
        item_id: Uuid,
        reason: CommunityReportReason,
        detail: &str,
    ) -> Result<Self, AccountError> {
        let report = Self {
            kind,
            item_id,
            reason,
            detail: detail.trim().to_owned(),
        };
        report.validate()?;
        Ok(report)
    }

    fn validate(&self) -> Result<(), AccountError> {
        if self.item_id.is_nil() || !valid_text(&self.detail, 0, MAX_REPORT_DETAIL_CHARS, true) {
            return Err(AccountError::Invalid);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunityReportResponse {
    reported: bool,
}

pub trait CommunityReportApi: Send + Sync + 'static {
    fn report_community_item(
        &self,
        report: &CommunityReport,
        token: &str,
    ) -> Result<(), AccountError>;
}

impl CommunityReportApi for BackendAccountClient {
    fn report_community_item(
        &self,
        report: &CommunityReport,
        token: &str,
    ) -> Result<(), AccountError> {
        report.validate()?;
        let response = self.json::<CommunityReportResponse, _>(
            Method::POST,
            "/v1/community/reports",
            Some(token),
            Some(report),
        )?;
        if !response.reported {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }
}

pub struct BackendCommunityReportService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCommunityReportService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCommunityReportService<A, S>
where
    A: AccountApi + CommunityReportApi,
    S: AccountSessionStorage,
{
    /// Files `report` with the signed-in session. `Unauthorized` without one, `NotFound` when the item is gone or not visible, `RateLimited` past the server's hourly limit.
    pub fn report(&self, report: &CommunityReport) -> Result<(), AccountError> {
        report.validate()?;
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.report_community_item(report, token.ok_or(AccountError::Unauthorized)?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    fn item() -> Uuid {
        Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap()
    }

    fn serve(status: &'static str, body: &'static str) -> (String, mpsc::Receiver<String>) {
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

    #[test]
    fn reasons_are_the_fixed_list_sent_as_their_labels() {
        let labels: Vec<_> = CommunityReportReason::ALL
            .iter()
            .map(|reason| reason.label())
            .collect();
        assert_eq!(
            labels,
            [
                "侵权/抄袭",
                "色情低俗",
                "违法违规",
                "垃圾广告",
                "恶意插件",
                "其他"
            ]
        );
        for reason in CommunityReportReason::ALL {
            assert_eq!(
                serde_json::to_value(reason).unwrap(),
                serde_json::Value::String(reason.label().into())
            );
            assert_eq!(
                CommunityReportReason::from_label(reason.label()),
                Some(reason)
            );
        }
        assert_eq!(CommunityReportReason::from_label("不喜欢"), None);
    }

    #[test]
    fn a_report_is_validated_before_it_leaves() {
        let reason = CommunityReportReason::Spam;
        assert_eq!(
            CommunityReport::new(CommunityReportKind::Skins, Uuid::nil(), reason, ""),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            CommunityReport::new(
                CommunityReportKind::Skins,
                item(),
                reason,
                &"字".repeat(1001)
            ),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            CommunityReport::new(CommunityReportKind::Skins, item(), reason, "a\u{0}b"),
            Err(AccountError::Invalid)
        );
        let report = CommunityReport::new(
            CommunityReportKind::Plugins,
            item(),
            reason,
            &format!("  {}\n第二行  ", "字".repeat(990)),
        )
        .unwrap();
        assert!(report.detail.starts_with('字') && report.detail.ends_with("第二行"));
    }

    #[test]
    fn the_report_is_posted_with_the_session_and_kind_names_the_server_uses() {
        let (origin, received) = serve("201 Created", r#"{"reported":true}"#);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let report = CommunityReport::new(
            CommunityReportKind::CandidateSkins,
            item(),
            CommunityReportReason::Infringement,
            "抄袭了我的皮肤",
        )
        .unwrap();
        client.report_community_item(&report, &token()).unwrap();
        let request = received.recv().unwrap();
        assert!(request.starts_with("POST /v1/community/reports HTTP/1.1"));
        assert!(request.contains(&format!("authorization: Bearer {}", token())));
        let body: serde_json::Value =
            serde_json::from_str(request.rsplit_once('\n').unwrap().1).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "kind": "candidate-skins",
                "item_id": item().hyphenated().to_string(),
                "reason": "侵权/抄袭",
                "detail": "抄袭了我的皮肤",
            })
        );

        // A repeated report answers 200 and is just as successful; an empty detail is left out.
        let (origin, received) = serve("200 OK", r#"{"reported":true}"#);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let report = CommunityReport::new(
            CommunityReportKind::Replies,
            item(),
            CommunityReportReason::Other,
            " ",
        )
        .unwrap();
        client.report_community_item(&report, &token()).unwrap();
        let request = received.recv().unwrap();
        assert!(!request.contains("detail"));
        assert!(request.contains("\"kind\":\"replies\""));
    }

    #[test]
    fn server_refusals_map_to_distinct_errors() {
        let report = CommunityReport::new(
            CommunityReportKind::Dictionaries,
            item(),
            CommunityReportReason::Illegal,
            "",
        )
        .unwrap();
        for (status, body, expected) in [
            (
                "404 Not Found",
                r#"{"error":{"code":"item_not_found","message":"item_not_found"}}"#,
                AccountError::NotFound,
            ),
            (
                "429 Too Many Requests",
                r#"{"error":{"code":"rate_limit_exceeded","message":"rate_limit_exceeded"}}"#,
                AccountError::RateLimited,
            ),
            (
                "403 Forbidden",
                r#"{"error":{"code":"account_banned","message":"account_banned"}}"#,
                AccountError::Banned,
            ),
        ] {
            let (origin, _received) = serve(status, body);
            let client = BackendAccountClient::loopback(&origin).unwrap();
            assert_eq!(
                client.report_community_item(&report, &token()),
                Err(expected),
                "{status}"
            );
        }
    }
}

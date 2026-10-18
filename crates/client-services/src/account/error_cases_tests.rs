//! 账号错误码的共享用例（#8201 迁移计划第 6 步）：逐条跑 `shared/contracts/account-errors/cases.json` 的 `from`，Rust 从那些输入得出的正是那个码和那个 Retry-After 秒数；[`AccountError`] 的每个变体和 [`AccountFailure`] 的每个本地码都在用例里，各平台的文案表（共享账号页、Windows、Swift、Java）才不会漏掉 Rust 会发的码。

use super::*;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Contract {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    code: String,
    retry_after_seconds: Option<u64>,
    from: Vec<Source>,
    // 三份文案表的答案由各平台的测试读，这里只要求它们都在。
    #[serde(rename = "shared")]
    _shared: String,
    #[serde(rename = "apple")]
    _apple: String,
    #[serde(rename = "android")]
    _android: String,
    #[serde(rename = "note")]
    _note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    status: Option<u16>,
    server_code: Option<String>,
    retry_after: Option<String>,
    #[serde(default)]
    network: bool,
    local: Option<String>,
}

fn contract_cases() -> Vec<Case> {
    let contract: Contract = serde_json::from_str(include_str!(
        "../../../../shared/contracts/account-errors/cases.json"
    ))
    .unwrap();
    contract.cases
}

fn failure(source: &Source) -> AccountFailure {
    if source.network {
        return AccountFailure::network();
    }
    if let Some(local) = &source.local {
        return match local.as_str() {
            "storage" => AccountError::Storage.into(),
            "cancelled" => AccountError::Cancelled.into(),
            "too_large" => AccountFailure::too_large(),
            "identity_changed" => AccountFailure::identity_changed(),
            "busy" => AccountFailure::busy(),
            other => panic!("unknown local failure {other}"),
        };
    }
    let body = source
        .server_code
        .as_ref()
        .map(|code| serde_json::json!({ "error": { "code": code } }).to_string())
        .unwrap_or_default();
    AccountFailure::from_http_response(
        source.status.expect("an HTTP source needs a status"),
        body.as_bytes(),
        source.retry_after.as_deref(),
    )
}

/// 每个变体都要列在这里：加一个变体而不加进去时这里编译不过，于是也得给它写一条用例。
fn every_variant() -> [AccountError; 12] {
    let variants = [
        AccountError::Invalid,
        AccountError::Unauthorized,
        AccountError::Forbidden,
        AccountError::Conflict,
        AccountError::NotFound,
        AccountError::RateLimited,
        AccountError::Unavailable,
        AccountError::Storage,
        AccountError::Cancelled,
        AccountError::BlockedContent,
        AccountError::ScreeningUnavailable,
        AccountError::Banned,
    ];
    for variant in &variants {
        match variant {
            AccountError::Invalid
            | AccountError::Unauthorized
            | AccountError::Forbidden
            | AccountError::Conflict
            | AccountError::NotFound
            | AccountError::RateLimited
            | AccountError::Unavailable
            | AccountError::Storage
            | AccountError::Cancelled
            | AccountError::BlockedContent
            | AccountError::ScreeningUnavailable
            | AccountError::Banned => {}
        }
    }
    variants
}

#[test]
fn every_source_in_the_shared_cases_gives_its_code_and_retry_after() {
    let cases = contract_cases();
    for case in &cases {
        for source in &case.from {
            let failure = failure(source);
            assert_eq!(failure.code(), case.code, "{:?}", source.status);
            let expected = source.retry_after.as_ref().and(case.retry_after_seconds);
            assert_eq!(failure.retry_after_seconds, expected, "{}", case.code);
        }
    }
    // 至少一条用例真的带着 Retry-After，否则上面的秒数从没核对过。
    assert!(cases.iter().any(|case| case.retry_after_seconds.is_some()
        && case.from.iter().any(|source| source.retry_after.is_some())));
}

#[test]
fn every_code_rust_can_give_has_a_shared_case() {
    let codes: BTreeSet<String> = contract_cases().into_iter().map(|case| case.code).collect();
    let mut given = BTreeSet::new();
    for variant in every_variant() {
        given.insert(variant.code().to_owned());
        given.insert(AccountFailure::from(variant).code().to_owned());
    }
    for local in [
        AccountFailure::network(),
        AccountFailure::too_large(),
        AccountFailure::identity_changed(),
        AccountFailure::busy(),
    ] {
        given.insert(local.code().to_owned());
    }
    // `AccountFailure::code` 认的服务端码和状态码，配上每个会出现的状态。
    for status in [400, 401, 403, 404, 409, 413, 422, 429, 500, 502, 503] {
        for server in [
            None,
            Some("recent_login_required"),
            Some("no_verified_email"),
            Some("qq_link_code_not_found"),
            Some("qq_link_limit"),
            Some("server_busy"),
            Some("feedback_too_large"),
            Some("screenshot_too_large"),
            Some("snapshot_too_large"),
            Some("blocked_content"),
            Some("screening_unavailable"),
            Some("account_banned"),
        ] {
            given.insert(
                failure(&Source {
                    status: Some(status),
                    server_code: server.map(str::to_owned),
                    retry_after: None,
                    network: false,
                    local: None,
                })
                .code()
                .to_owned(),
            );
        }
    }
    let missing: Vec<_> = given.difference(&codes).collect();
    assert!(
        missing.is_empty(),
        "codes without a shared case: {missing:?}"
    );
}

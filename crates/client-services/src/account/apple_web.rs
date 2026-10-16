//! 没有 Apple 原生登录的宿主（安卓）走的 Apple 网页登录：PKCE 式的一次性授权码，与 msime-cloud `internal/account/apple_web.go` 的约定相同。
//!
//! 本机生成 32 字节随机数的 `code_verifier`，只把它的 S256 摘要交给 `POST /v1/auth/apple/web`；Apple 回调到服务端后，服务端经 `<app>://auth/apple?grant=…` 把一枚一次性 `grant` 交回应用，应用再用 grant 加上只在本机的 verifier 经 `POST /v1/auth/apple/web/login` 换会话。回调地址里没有 challenge id 也没有 id_token，伪造的回调因为没有对应的 verifier 换不到任何东西。浏览器打开期间宿主进程可能被系统杀掉，所以 verifier 交给宿主保存，这里不留状态。

use super::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

const APPLE_AUTHORIZATION_PREFIX: &str = "https://appleid.apple.com/";
const APPLE_AUTHORIZATION_PATH: &str = "/auth/authorize";
/// 服务端的授权码是 32 字节随机数的无填充 base64url（`randomGrant`）。
const APPLE_WEB_GRANT_BYTES: usize = 32;
/// 本机生成的 verifier 用的随机字节数；编码后是 43 个字符，RFC 7636 允许的最短长度。
const CODE_VERIFIER_BYTES: usize = 32;
const MAX_APP_BYTES: usize = 128;

/// `POST /v1/auth/apple/web` 的响应。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountAppleWebChallenge {
    pub authorization_url: String,
    pub expires_in: u64,
}

/// 一次 Apple 网页登录的开头：在浏览器里打开 `authorization_url`；`code_verifier` 由宿主私下保存，回调带着 grant 回来时与它一起交给 [`BackendAccountSession::sign_in_apple_web`]。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleWebSignIn {
    pub authorization_url: String,
    pub expires_in: u64,
    pub code_verifier: String,
}

/// 32 字节系统随机数的无填充 base64url。取不到随机数时是 [`AccountError::Unavailable`]，绝不拿可预测的值顶替。
pub(super) fn new_code_verifier() -> Result<String, AccountError> {
    let mut bytes = [0_u8; CODE_VERIFIER_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| AccountError::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// RFC 7636 的 S256：`base64url(SHA-256(ASCII(verifier)))`，无填充，与服务端的 `pkceChallenge` 相同。
pub(super) fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// 与服务端的 `validPKCEVerifier` 相同：43–128 个 RFC 7636 的 unreserved 字符。
pub(super) fn valid_code_verifier(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
}

/// 恰好 `bytes` 字节的规范无填充 base64url，与服务端的 `base64URLToken` 相同。
pub(super) fn valid_base64url_token(value: &str, bytes: usize) -> bool {
    value.len() == (bytes * 4).div_ceil(3)
        && URL_SAFE_NO_PAD
            .decode(value)
            .is_ok_and(|decoded| decoded.len() == bytes)
}

/// 回调里交回来的一次性授权码。
pub(super) fn valid_apple_web_grant(value: &str) -> bool {
    valid_base64url_token(value, APPLE_WEB_GRANT_BYTES)
}

/// 发起登录的安卓 applicationId 的形状。哪些包名可以用由服务端决定（不认识的是 `400 invalid_app`），这里只拦住不可能是包名的值。
pub(super) fn valid_apple_web_app(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APP_BYTES
        && value.split('.').all(|part| {
            part.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

/// 服务端给的授权链接要在用户的浏览器里打开，所以只接受 Apple 自己的 HTTPS 授权页、带一个 `state`，别的形状一律当作服务端不可用，不打开。
pub(super) fn validate_apple_web_challenge(
    challenge: &AccountAppleWebChallenge,
) -> Result<(), AccountError> {
    let url = challenge.authorization_url.as_str();
    if challenge.expires_in == 0
        || !crate::text::is_bounded_text(url, 4096)
        || !url.starts_with(APPLE_AUTHORIZATION_PREFIX)
        || url.bytes().any(|byte| {
            byte <= b' '
                || byte >= 0x7f
                || matches!(byte, b'"' | b'\'' | b'`' | b'|' | b'<' | b'>' | b'\\')
        })
    {
        return Err(AccountError::Unavailable);
    }
    let parsed = Url::parse(url).map_err(|_| AccountError::Unavailable)?;
    let mut states = parsed.query_pairs().filter(|(key, _)| key == "state");
    let single_state =
        states.next().is_some_and(|(_, value)| !value.is_empty()) && states.next().is_none();
    if !crate::endpoint::valid_endpoint_shape(url, &parsed, "https")
        || parsed.host_str() != Some("appleid.apple.com")
        || parsed.port().is_some()
        || parsed.path() != APPLE_AUTHORIZATION_PATH
        || !single_state
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

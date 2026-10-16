//! `GET/PUT /v1/users/me/phrases`：无编码常用语的整列表同步，与偏好同一套 revision 比较并交换（msime-cloud `internal/account/phrases.go`）。发送前按服务端的规则完整校验，请求体预检 256 KiB；冲突（409 `revision_conflict`）是 `account_conflict`，不重试。

use super::*;

pub const PHRASES_PATH: &str = "/v1/users/me/phrases";
/// 列表最多的条数（服务端 `maximumPhrases`）。
pub const MAX_PHRASES: usize = 500;
/// 请求体上限（服务端 `maximumPhrasesBytes`）。
pub const MAX_PHRASES_BYTES: usize = 256 * 1024;
const MAX_PHRASE_TEXT_UNITS: usize = 2000;
const MAX_PHRASE_GROUP_UNITS: usize = 32;
const MAX_PHRASE_ID_BYTES: usize = 64;
const MAX_PHRASE_POSITION: i64 = 1_000_000;

/// 一条常用语。`id` 由客户端生成、在列表内唯一；`text` 可以多行，只有空白也合法；`group` 为空表示未分组；`position` 是排序键。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPhrase {
    pub id: String,
    pub text: String,
    pub group: String,
    pub position: i64,
}

/// 云端的常用语列表和它的修订号；新用户是 `revision: 0` 和空列表。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPhrases {
    pub revision: i64,
    pub phrases: Vec<AccountPhrase>,
}

/// id 是 1–64 个 ASCII 字母、数字、`-` 或 `_`（服务端 `validPhraseID`）。
pub fn valid_phrase_id(id: &str) -> bool {
    crate::text::is_bounded_ascii_identifier(id, MAX_PHRASE_ID_BYTES)
}

/// 整份列表是否满足服务端的 `validPhrases`：最多 500 条、id 唯一，text 为 1–2000 个 UTF-16 单元且不含 NUL，group 最多 32 个 UTF-16 单元且不含控制字符，position 在 0..=1000000。
pub fn valid_phrases(phrases: &[AccountPhrase]) -> bool {
    if phrases.len() > MAX_PHRASES {
        return false;
    }
    let mut seen = HashSet::with_capacity(phrases.len());
    phrases.iter().all(|phrase| {
        valid_phrase_id(&phrase.id)
            && seen.insert(phrase.id.as_str())
            && !phrase.text.is_empty()
            && !phrase.text.contains('\0')
            && crate::is_bounded_utf16(&phrase.text, MAX_PHRASE_TEXT_UNITS)
            && !phrase.group.chars().any(char::is_control)
            && crate::is_bounded_utf16(&phrase.group, MAX_PHRASE_GROUP_UNITS)
            && (0..=MAX_PHRASE_POSITION).contains(&phrase.position)
    })
}

/// 校验要上传的列表并给出请求体。不合法是 `account_invalid`，超过 256 KiB 是 `account_too_large`。
pub fn phrases_request_body(phrases: &AccountPhrases) -> Result<Vec<u8>, AccountFailure> {
    if phrases.revision < 0 || !valid_phrases(&phrases.phrases) {
        return Err(AccountError::Invalid.into());
    }
    let body = serde_json::to_vec(phrases).map_err(|_| AccountError::Invalid)?;
    if body.len() > MAX_PHRASES_BYTES {
        return Err(AccountFailure::too_large());
    }
    Ok(body)
}

fn validate_response(phrases: AccountPhrases) -> Result<AccountPhrases, AccountFailure> {
    if phrases.revision < 0 || !valid_phrases(&phrases.phrases) {
        return Err(AccountError::Unavailable.into());
    }
    Ok(phrases)
}

impl BackendAccountClient {
    pub(super) fn load_phrases(
        &self,
        access_token: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let phrases = self.json_detailed::<AccountPhrases, ()>(
            Method::GET,
            PHRASES_PATH,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_response(phrases)
    }

    pub(super) fn store_phrases(
        &self,
        phrases: &AccountPhrases,
        access_token: &str,
    ) -> Result<AccountPhrases, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let body = phrases_request_body(phrases)?;
        let bytes = self.request_detailed(
            Method::PUT,
            PHRASES_PATH,
            Some(access_token),
            Some(body),
            MAX_PHRASES_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )?;
        let saved: AccountPhrases =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        validate_response(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrase(id: &str, text: &str) -> AccountPhrase {
        AccountPhrase {
            id: id.into(),
            text: text.into(),
            group: String::new(),
            position: 0,
        }
    }

    #[test]
    fn phrases_follow_the_server_rules() {
        assert!(valid_phrases(&[phrase("a-1", "你好"), phrase("b_2", "  ")]));
        assert!(valid_phrases(&[phrase("a", "第一行\n第二行")]));
        let rejected = [
            vec![phrase("", "x")],
            vec![phrase("a b", "x")],
            vec![phrase(&"a".repeat(65), "x")],
            vec![phrase("a", "x"), phrase("a", "y")],
            vec![phrase("a", "")],
            vec![phrase("a", "nul\0")],
            vec![phrase("a", &"😀".repeat(1001))],
        ];
        for list in rejected {
            assert!(!valid_phrases(&list), "{list:?}");
        }
        assert!(valid_phrases(&[phrase("a", &"😀".repeat(1000))]));
        let mut grouped = phrase("a", "x");
        grouped.group = "g\u{7}".into();
        assert!(!valid_phrases(&[grouped.clone()]));
        grouped.group = "组".repeat(33);
        assert!(!valid_phrases(&[grouped.clone()]));
        grouped.group = "组".repeat(32);
        grouped.position = 1_000_001;
        assert!(!valid_phrases(&[grouped.clone()]));
        grouped.position = -1;
        assert!(!valid_phrases(&[grouped]));
        let many: Vec<_> = (0..=MAX_PHRASES)
            .map(|index| phrase(&index.to_string(), "x"))
            .collect();
        assert!(!valid_phrases(&many));
        assert!(valid_phrases(&many[..MAX_PHRASES]));
    }

    #[test]
    fn an_oversized_list_is_refused_locally() {
        let list: Vec<_> = (0..200)
            .map(|index| phrase(&index.to_string(), &"长".repeat(2000)))
            .collect();
        let failure = phrases_request_body(&AccountPhrases {
            revision: 1,
            phrases: list,
        })
        .unwrap_err();
        assert_eq!(failure.code(), "account_too_large");
        assert_eq!(
            phrases_request_body(&AccountPhrases {
                revision: -1,
                phrases: Vec::new(),
            })
            .unwrap_err()
            .code(),
            "account_invalid"
        );
    }
}

//! 账号页读到的脱敏响应：iOS、三个桌面宿主的 Tauri 命令和原生设置窗口经 C ABI（`msime_client_account`）返回的都是这几种形状，设置同步卡片读到的字段表（[`PreferenceSchemaResponse`]）也在这里。
//!
//! 它们就是 `packages/ui/src/account/account-page.tsx` 声明的 `AccountUser`、`AccountProviders`、`AccountChallenge` 和 `AccountProfile`，外加 `{ user }` 状态包装。令牌、nonce 和授权链接都不会进到这里。provider 形状比 `AccountProviders` 多一个 `apple_web`（Apple 网页登录的开关，给原生 Android 宿主经 C ABI 读），共享页面不读它。Tauri 的 Android 宿主用同一个 provider 形状但省掉可选的 `apple`、`google` 和 `apple_web`：只有宿主也实现了那种登录时，页面才给出相应按钮。

use super::{account_avatar_is_uploaded, account_avatar_url_allowed};
use super::{
    AccountChallenge, AccountChatModels, AccountPreferenceField, AccountPreferenceSchema,
    AccountProfile, AccountUser,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub user: Option<UserResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    id: String,
    display_name: String,
    /// 共享昵称规则给出的显示名；原始昵称仍保留给编辑草稿。
    preferred_name: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    /// Not loadable by the page itself, whose content security policy blocks remote images; it changes whenever the avatar does, so the page uses it to know when to ask the host for the image again.
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    /// Whether the avatar is one the user uploaded, which the page can offer to remove, rather than the Google picture.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    avatar_uploaded: bool,
}

impl From<AccountUser> for UserResponse {
    fn from(user: AccountUser) -> Self {
        let preferred_name = super::name::present_name(&user.display_name, &user.id).preferred;
        // A URL this client would never fetch is not passed on either, so the page cannot show a placeholder for an avatar that will not load.
        let avatar_url = user
            .avatar_url
            .filter(|url| account_avatar_url_allowed(url));
        Self {
            id: user.id,
            display_name: user.display_name,
            preferred_name,
            created_at: user.created_at,
            email: user.email,
            avatar_uploaded: avatar_url
                .as_deref()
                .is_some_and(account_avatar_is_uploaded),
            avatar_url,
        }
    }
}

#[derive(Serialize)]
pub struct ProvidersResponse {
    email: bool,
    phone: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    apple: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    google: Option<bool>,
    /// 服务端的 Apple 网页登录开关（`/v1/auth/providers` 的 `apple_web`），给没有 Apple 原生登录的 Android 宿主决定要不要给出 Apple 按钮。共享页面不读它。
    #[serde(skip_serializing_if = "Option::is_none")]
    apple_web: Option<bool>,
}

pub fn provider_flags(providers: &HashMap<String, bool>) -> (bool, bool) {
    (
        providers.get("email") == Some(&true),
        providers.get("phone") == Some(&true) || providers.get("sms") == Some(&true),
    )
}

pub fn providers_response(providers: HashMap<String, bool>) -> ProvidersResponse {
    let (email, phone) = provider_flags(&providers);
    ProvidersResponse {
        email,
        phone,
        apple: Some(providers.get("apple") == Some(&true)),
        google: Some(providers.get("google") == Some(&true)),
        apple_web: Some(providers.get("apple_web") == Some(&true)),
    }
}

pub fn providers_response_without_apple(providers: HashMap<String, bool>) -> ProvidersResponse {
    let (email, phone) = provider_flags(&providers);
    ProvidersResponse {
        email,
        phone,
        apple: None,
        google: None,
        apple_web: None,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeResponse {
    challenge_id: String,
    expires_in: u64,
}

impl From<AccountChallenge> for ChallengeResponse {
    fn from(challenge: AccountChallenge) -> Self {
        Self {
            challenge_id: challenge.challenge_id,
            expires_in: challenge.expires_in,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    user: UserResponse,
    providers: Vec<String>,
    /// 已知登录方式及本次资料中新增标识的显示名称；未关联的已知方式也供关联入口使用。
    provider_names: BTreeMap<String, String>,
}

impl From<AccountProfile> for ProfileResponse {
    fn from(profile: AccountProfile) -> Self {
        let mut providers = Vec::with_capacity(profile.identities.len());
        let mut provider_names: BTreeMap<String, String> = [
            ("apple", "Apple"),
            ("google", "Google"),
            ("email", "邮箱"),
            ("phone", "手机号"),
            ("sms", "手机号"),
        ]
        .into_iter()
        .map(|(provider, name)| (provider.into(), name.into()))
        .collect();
        for identity in profile.identities {
            provider_names
                .entry(identity.provider.clone())
                .or_insert_with(|| identity.provider.clone());
            if !providers.contains(&identity.provider) {
                providers.push(identity.provider);
            }
        }
        Self {
            user: profile.user.into(),
            providers,
            provider_names,
        }
    }
}

/// 对话可用的一个模型。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelResponse {
    id: String,
}

/// 对话可用的模型和默认模型：`{data:[{id}], defaultModel}`。iOS、Android 的 Tauri 账号命令和 `msime_client_account` 的 `chat_models` 返回的都是它。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelsResponse {
    data: Vec<ChatModelResponse>,
    default_model: String,
}

impl From<AccountChatModels> for ChatModelsResponse {
    fn from(models: AccountChatModels) -> Self {
        Self {
            data: models
                .data
                .into_iter()
                .map(|model| ChatModelResponse { id: model.id })
                .collect(),
            default_model: models.default_model,
        }
    }
}

/// 一次对话的回答：`{content}`。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub content: String,
}

/// 设置同步卡片读到的字段表：Windows 的共享应用和原生设置窗口、iOS、Android 都返回这个形状。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceSchemaResponse {
    fields: BTreeMap<String, AccountPreferenceField>,
    maximum_bytes: usize,
    update_mode: String,
    revision_required: bool,
}

impl From<AccountPreferenceSchema> for PreferenceSchemaResponse {
    fn from(schema: AccountPreferenceSchema) -> Self {
        Self {
            fields: schema.fields,
            maximum_bytes: schema.maximum_bytes,
            update_mode: schema.update_mode,
            revision_required: schema.revision_required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{providers_response, ChallengeResponse, ProfileResponse, StatusResponse};
    use crate::account::{AccountChallenge, AccountProfile, AccountProfileIdentity, AccountUser};
    use serde_json::json;
    use std::collections::HashMap;

    fn user() -> AccountUser {
        AccountUser {
            id: "synthetic-user".into(),
            display_name: "测试账号".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        }
    }

    #[test]
    fn user_response_includes_the_shared_preferred_name_without_changing_the_draft() {
        for (name, id, expected) in [
            ("\u{feff} 合成昵称 \u{3000}", "synthetic-user", "合成昵称"),
            ("", "abc123-rest", "水杉小鹿·ABC123"),
            ("\u{feff}\u{a0}\u{3000}", "abc123-rest", "水杉小鹿·ABC123"),
            ("", "ab😀cde-tail", "水杉小鹿·AB😀CDE"),
            ("", "abßcde-tail", "水杉小鹿·ABßCDE"),
        ] {
            let mut account = user();
            account.id = id.into();
            account.display_name = name.into();
            let response = serde_json::to_value(StatusResponse {
                user: Some(account.into()),
            })
            .unwrap();
            assert_eq!(response["user"]["displayName"], name);
            assert_eq!(response["user"]["preferredName"], expected);
        }
    }

    #[test]
    fn account_responses_match_the_shared_webview_contract() {
        let status = serde_json::to_value(StatusResponse {
            user: Some(user().into()),
        })
        .unwrap();
        assert_eq!(
            status,
            json!({"user":{"id":"synthetic-user","displayName":"测试账号","preferredName":"测试账号","createdAt":"2026-01-01T00:00:00Z"}})
        );

        // A signed-out status is `{ user: null }`, which the webview's `user?: AccountUser | null` accepts.
        let signed_out = serde_json::to_value(StatusResponse { user: None }).unwrap();
        assert_eq!(signed_out, json!({"user":null}));

        // The Google email and the avatar reach the page in camelCase; an avatar from a host the client never fetches does not.
        let mut google = user();
        google.email = Some("person@example.test".into());
        google.avatar_url = Some("https://media.msime.app/avatars/abc.jpg".into());
        assert_eq!(
            serde_json::to_value(StatusResponse {
                user: Some(google.clone().into())
            })
            .unwrap()["user"],
            json!({"id":"synthetic-user","displayName":"测试账号","preferredName":"测试账号","createdAt":"2026-01-01T00:00:00Z","email":"person@example.test","avatarUrl":"https://media.msime.app/avatars/abc.jpg","avatarUploaded":true})
        );
        google.avatar_url = Some("https://lh3.googleusercontent.com/a/person".into());
        let from_google = serde_json::to_value(StatusResponse {
            user: Some(google.clone().into()),
        })
        .unwrap();
        assert_eq!(
            from_google["user"]["avatarUrl"],
            "https://lh3.googleusercontent.com/a/person"
        );
        assert!(from_google["user"].get("avatarUploaded").is_none());
        google.avatar_url = Some("https://example.test/elsewhere.png".into());
        let elsewhere = serde_json::to_value(StatusResponse {
            user: Some(google.into()),
        })
        .unwrap();
        assert!(elsewhere["user"].get("avatarUrl").is_none());

        let challenge = serde_json::to_value(ChallengeResponse::from(AccountChallenge {
            challenge_id: "synthetic-challenge".into(),
            expires_in: 300,
            nonce: Some("not-exposed".into()),
            authorization_url: Some("https://invalid.example".into()),
        }))
        .unwrap();
        assert_eq!(
            challenge,
            json!({"challengeId":"synthetic-challenge","expiresIn":300})
        );
    }

    #[test]
    fn profile_response_carries_shared_provider_names_and_preserves_identity_order() {
        let response = serde_json::to_value(ProfileResponse::from(AccountProfile {
            user: user(),
            identities: ["sms", "email", "synthetic-provider", "sms", "phone"]
                .into_iter()
                .enumerate()
                .map(|(index, provider)| AccountProfileIdentity {
                    provider: provider.into(),
                    subject: format!("synthetic-subject-{index}"),
                })
                .collect(),
        }))
        .unwrap();
        assert_eq!(
            response["providers"],
            json!(["sms", "email", "synthetic-provider", "phone"])
        );
        assert_eq!(
            response["providerNames"],
            json!({
                "apple": "Apple",
                "google": "Google",
                "email": "邮箱",
                "phone": "手机号",
                "sms": "手机号",
                "synthetic-provider": "synthetic-provider"
            })
        );
    }

    #[test]
    fn provider_and_profile_responses_normalize_backend_names() {
        let providers = providers_response(HashMap::from([
            ("email".into(), true),
            ("sms".into(), true),
            ("apple_web".into(), true),
        ]));
        // "sms" is the backend's name for the phone provider, and apple is reported even when absent so the client can tell "this account has no Apple identity" from "this build does not know about the provider".
        assert_eq!(
            serde_json::to_value(providers).unwrap(),
            json!({"email":true,"phone":true,"apple":false,"google":false,"apple_web":true})
        );

        let profile = ProfileResponse::from(AccountProfile {
            user: user(),
            identities: vec![
                AccountProfileIdentity {
                    provider: "email".into(),
                    subject: "synthetic-one".into(),
                },
                AccountProfileIdentity {
                    provider: "email".into(),
                    subject: "synthetic-two".into(),
                },
                AccountProfileIdentity {
                    provider: "phone".into(),
                    subject: "synthetic-three".into(),
                },
            ],
        });
        assert_eq!(
            serde_json::to_value(profile).unwrap(),
            json!({
                "user":{"id":"synthetic-user","displayName":"测试账号","preferredName":"测试账号","createdAt":"2026-01-01T00:00:00Z"},
                "providers":["email","phone"],
                "providerNames":{"apple":"Apple","google":"Google","email":"邮箱","phone":"手机号","sms":"手机号"}
            })
        );
    }
}

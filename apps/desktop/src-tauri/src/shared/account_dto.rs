//! Redacted account responses the webview reads, shared by the iOS host and the three desktop hosts.
//!
//! These are the JSON shapes `packages/ui/src/account/account-page.tsx` declares as `AccountUser`, `AccountProviders`, `AccountChallenge` and `AccountProfile`, plus the `{ user }` status wrapper. Tokens, nonces and authorization URLs never reach them. The Android host reuses the same provider DTO while omitting the optional `apple` and `google` fields; the page only offers either button when the host client also implements that sign-in.

use msime_client_core::account::{
    AccountChallenge, AccountChatModels, AccountPreferenceSchema, AccountProfile, AccountUser,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub(crate) user: Option<UserResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserResponse {
    id: String,
    display_name: String,
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
        // A URL this client would never fetch is not passed on either, so the page cannot show a placeholder for an avatar that will not load.
        let avatar_url = user
            .avatar_url
            .filter(|url| msime_client_core::account::account_avatar_url_allowed(url));
        Self {
            id: user.id,
            display_name: user.display_name,
            created_at: user.created_at,
            email: user.email,
            avatar_uploaded: avatar_url
                .as_deref()
                .is_some_and(msime_client_core::account::account_avatar_is_uploaded),
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
}

pub(crate) fn provider_flags(providers: &HashMap<String, bool>) -> (bool, bool) {
    (
        providers.get("email") == Some(&true),
        providers.get("phone") == Some(&true) || providers.get("sms") == Some(&true),
    )
}

pub(crate) fn providers_response(providers: HashMap<String, bool>) -> ProvidersResponse {
    let (email, phone) = provider_flags(&providers);
    ProvidersResponse {
        email,
        phone,
        apple: Some(providers.get("apple") == Some(&true)),
        google: Some(providers.get("google") == Some(&true)),
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
pub(crate) fn providers_response_without_apple(
    providers: HashMap<String, bool>,
) -> ProvidersResponse {
    let (email, phone) = provider_flags(&providers);
    ProvidersResponse {
        email,
        phone,
        apple: None,
        google: None,
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
}

impl From<AccountProfile> for ProfileResponse {
    fn from(profile: AccountProfile) -> Self {
        let mut providers = Vec::with_capacity(profile.identities.len());
        for identity in profile.identities {
            if !providers.contains(&identity.provider) {
                providers.push(identity.provider);
            }
        }
        Self {
            user: profile.user.into(),
            providers,
        }
    }
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelResponse {
    id: String,
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
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

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub(crate) content: String,
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceSchemaResponse {
    fields: BTreeMap<String, msime_client_core::account::AccountPreferenceField>,
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
    use msime_client_core::account::{
        AccountChallenge, AccountProfile, AccountProfileIdentity, AccountUser,
    };
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
    fn account_responses_match_the_shared_webview_contract() {
        let status = serde_json::to_value(StatusResponse {
            user: Some(user().into()),
        })
        .unwrap();
        assert_eq!(
            status,
            json!({"user":{"id":"synthetic-user","displayName":"测试账号","createdAt":"2026-01-01T00:00:00Z"}})
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
            json!({"id":"synthetic-user","displayName":"测试账号","createdAt":"2026-01-01T00:00:00Z","email":"person@example.test","avatarUrl":"https://media.msime.app/avatars/abc.jpg","avatarUploaded":true})
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
    fn provider_and_profile_responses_normalize_backend_names() {
        let providers = providers_response(HashMap::from([
            ("email".into(), true),
            ("sms".into(), true),
        ]));
        // "sms" is the backend's name for the phone provider, and apple is reported even when absent so the client can tell "this account has no Apple identity" from "this build does not know about the provider".
        assert_eq!(
            serde_json::to_value(providers).unwrap(),
            json!({"email":true,"phone":true,"apple":false,"google":false})
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
                "user":{"id":"synthetic-user","displayName":"测试账号","createdAt":"2026-01-01T00:00:00Z"},
                "providers":["email","phone"]
            })
        );
    }
}

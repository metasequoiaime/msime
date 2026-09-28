//! Redacted account responses the webview reads, shared by the iOS host and the three desktop hosts.
//!
//! These are the JSON shapes `packages/ui/src/account/account-page.tsx` declares as `AccountUser`, `AccountProviders`, `AccountChallenge` and `AccountProfile`, plus the `{ user }` status wrapper. Tokens, nonces and authorization URLs never reach them. Android keeps its own provider wrapper because its providers response has no `apple` field, while reusing the common user, status, challenge and profile DTOs.

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
}

impl From<AccountUser> for UserResponse {
    fn from(user: AccountUser) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name,
            created_at: user.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct ProvidersResponse {
    email: bool,
    phone: bool,
    apple: bool,
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
        apple: providers.get("apple") == Some(&true),
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
        let mut providers = Vec::new();
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelResponse {
    id: String,
}

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub(crate) content: String,
}

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
            json!({"email":true,"phone":true,"apple":false})
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

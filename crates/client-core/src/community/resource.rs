//! Bounded community dictionaries and reply templates.
//! Network, credentials, and UI state stay outside client-core.

use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::cloud::dictionary::{percent_encode, DictionaryKind};
use crate::community::{
    valid_author, valid_description, valid_name, valid_query, valid_rating, valid_text,
    MAXIMUM_JAVASCRIPT_INTEGER, MAXIMUM_PAGE_ITEMS,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

const MAXIMUM_CONTENT_BYTES: usize = 350_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CommunityResourceKind {
    Dictionary,
    Reply,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedWord {
    pub kind: DictionaryKind,
    pub code: String,
    pub word: String,
    pub weight: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityResourceContent {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<SharedWord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityResource {
    pub id: Uuid,
    pub kind: CommunityResourceKind,
    pub name: String,
    pub description: String,
    pub author: String,
    pub content: CommunityResourceContent,
    pub revision: u32,
    pub saves: u64,
    pub saved: bool,
    pub owned: bool,
    pub rating_count: u64,
    pub rating_average: f64,
    pub my_rating: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityResourcePage {
    pub items: Vec<CommunityResource>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityResourcePublication {
    pub id: Uuid,
    pub revision: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityResourceApplication {
    pub revision: i64,
    pub imported: u32,
    pub resource_revision: u32,
}

pub struct CommunityResourcePublicationRequest<'a> {
    pub id: Uuid,
    pub kind: CommunityResourceKind,
    pub name: &'a str,
    pub description: &'a str,
    pub content: &'a CommunityResourceContent,
    pub revision: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CommunityResourceScope {
    #[default]
    All,
    Mine,
    Saved,
}

impl CommunityResourceScope {
    fn query(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Mine => "mine",
            Self::Saved => "saved",
        }
    }
}

pub trait CommunityResourceApi: Send + Sync + 'static {
    fn community_resources(
        &self,
        kind: CommunityResourceKind,
        scope: CommunityResourceScope,
        search: &str,
        offset: usize,
        token: Option<&str>,
    ) -> Result<CommunityResourcePage, AccountError>;
    fn community_resource(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CommunityResource, AccountError>;
    fn publish_community_resource(
        &self,
        request: &CommunityResourcePublicationRequest<'_>,
        token: &str,
    ) -> Result<CommunityResourcePublication, AccountError>;
    fn apply_community_resource(
        &self,
        id: Uuid,
        resource_revision: u32,
        dictionary_revision: i64,
        token: &str,
    ) -> Result<CommunityResourceApplication, AccountError>;
    fn dictionary_revision(&self, token: &str) -> Result<i64, AccountError>;
    fn save_community_resource(
        &self,
        id: Uuid,
        saved: bool,
        token: &str,
    ) -> Result<(), AccountError>;
    fn rate_community_resource(&self, id: Uuid, stars: u8, token: &str)
        -> Result<(), AccountError>;
    fn delete_community_resource(&self, id: Uuid, token: &str) -> Result<(), AccountError>;
}

impl CommunityResourceApi for BackendAccountClient {
    fn community_resources(
        &self,
        kind: CommunityResourceKind,
        scope: CommunityResourceScope,
        search: &str,
        offset: usize,
        token: Option<&str>,
    ) -> Result<CommunityResourcePage, AccountError> {
        validate_query(offset, search, scope, token)?;
        let path = format!(
            "/v1/community/resources?kind={}&scope={}&q={}&offset={offset}",
            kind_name(kind),
            scope.query(),
            percent_encode(search)
        );
        let page = self.json_with_limit::<CommunityResourcePage, ()>(
            Method::GET,
            &path,
            token,
            None,
            48 * 1024 * 1024,
        )?;
        validate_page(&page, kind)?;
        Ok(page)
    }

    fn community_resource(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CommunityResource, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let value = self.json_with_limit::<CommunityResource, ()>(
            Method::GET,
            &format!("/v1/community/resources/{}", id.hyphenated()),
            token,
            None,
            3 * 1024 * 1024,
        )?;
        validate_resource(&value)?;
        if value.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(value)
    }

    fn publish_community_resource(
        &self,
        request: &CommunityResourcePublicationRequest<'_>,
        token: &str,
    ) -> Result<CommunityResourcePublication, AccountError> {
        validate_publication(
            request.id,
            request.kind,
            request.name,
            request.description,
            request.content,
            request.revision,
        )?;
        #[derive(Serialize)]
        struct Body<'a> {
            id: Uuid,
            kind: CommunityResourceKind,
            name: &'a str,
            description: &'a str,
            content: &'a CommunityResourceContent,
            revision: u32,
        }
        let body = Body {
            id: request.id,
            kind: request.kind,
            name: request.name,
            description: request.description,
            content: request.content,
            revision: request.revision,
        };
        let result = self.json::<CommunityResourcePublication, _>(
            Method::POST,
            "/v1/community/resources",
            Some(token),
            Some(&body),
        )?;
        if result.id != request.id || result.revision == 0 {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    fn apply_community_resource(
        &self,
        id: Uuid,
        resource_revision: u32,
        dictionary_revision: i64,
        token: &str,
    ) -> Result<CommunityResourceApplication, AccountError> {
        if id.is_nil() || resource_revision == 0 || dictionary_revision < 0 {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body {
            resource_revision: u32,
            dictionary_revision: i64,
        }
        let result = self.json::<CommunityResourceApplication, _>(
            Method::POST,
            &format!("/v1/community/resources/{id}/apply"),
            Some(token),
            Some(&Body {
                resource_revision,
                dictionary_revision,
            }),
        )?;
        if result.resource_revision != resource_revision
            || result.imported > 128
            || result.revision < dictionary_revision
            || result.revision - dictionary_revision != i64::from(result.imported)
        {
            return Err(AccountError::Unavailable);
        }
        Ok(result)
    }

    fn dictionary_revision(&self, token: &str) -> Result<i64, AccountError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Catalog {
            revision: i64,
        }
        let catalog = self.json::<Catalog, ()>(
            Method::GET,
            "/v1/users/me/dictionaries/quick/catalog?q=&offset=0&limit=100&scheme=pinyin&profile=xiaohe",
            Some(token),
            None,
        )?;
        if catalog.revision < 0 {
            return Err(AccountError::Unavailable);
        }
        Ok(catalog.revision)
    }

    fn save_community_resource(
        &self,
        id: Uuid,
        saved: bool,
        token: &str,
    ) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body {
            saved: bool,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ResultBody {
            saved: bool,
        }
        let result = self.json::<ResultBody, _>(
            Method::PUT,
            &format!("/v1/community/resources/{id}/save"),
            Some(token),
            Some(&Body { saved }),
        )?;
        if result.saved != saved {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }

    fn rate_community_resource(
        &self,
        id: Uuid,
        stars: u8,
        token: &str,
    ) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct Body {
            stars: u8,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ResultBody {
            stars: u8,
        }
        let result = self.json::<ResultBody, _>(
            Method::PUT,
            &format!("/v1/community/resources/{id}/rating"),
            Some(token),
            Some(&Body { stars }),
        )?;
        if result.stars != stars {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }

    fn delete_community_resource(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ResultBody {
            deleted: bool,
        }
        let result = self.json::<ResultBody, ()>(
            Method::DELETE,
            &format!("/v1/community/resources/{id}"),
            Some(token),
            None,
        )?;
        if !result.deleted {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }
}

pub struct BackendCommunityResourceService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCommunityResourceService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCommunityResourceService<A, S>
where
    A: AccountApi + CommunityResourceApi,
    S: AccountSessionStorage,
{
    pub fn list(
        &self,
        kind: CommunityResourceKind,
        scope: CommunityResourceScope,
        search: &str,
        offset: usize,
    ) -> Result<CommunityResourcePage, AccountError> {
        request_with_account_session(
            &self.api,
            &self.session,
            scope != CommunityResourceScope::All,
            |api, token| api.community_resources(kind, scope, search, offset, token),
        )
    }

    pub fn detail(&self, id: Uuid) -> Result<CommunityResource, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, false, |api, token| {
            api.community_resource(id, token)
        })
    }

    pub fn publish(
        &self,
        id: Uuid,
        kind: CommunityResourceKind,
        name: &str,
        description: &str,
        content: &CommunityResourceContent,
        revision: u32,
    ) -> Result<CommunityResourcePublication, AccountError> {
        let request = CommunityResourcePublicationRequest {
            id,
            kind,
            name,
            description,
            content,
            revision,
        };
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.publish_community_resource(&request, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn apply(
        &self,
        id: Uuid,
        resource_revision: u32,
    ) -> Result<CommunityResourceApplication, AccountError> {
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            let token = token.ok_or(AccountError::Unauthorized)?;
            let dictionary_revision = api.dictionary_revision(token)?;
            api.apply_community_resource(id, resource_revision, dictionary_revision, token)
        })
    }

    pub fn save(&self, id: Uuid, saved: bool) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.save_community_resource(id, saved, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn rate(&self, id: Uuid, stars: u8) -> Result<(), AccountError> {
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.rate_community_resource(id, stars, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn delete(&self, id: Uuid) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.delete_community_resource(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }
}

fn kind_name(kind: CommunityResourceKind) -> &'static str {
    match kind {
        CommunityResourceKind::Dictionary => "dictionary",
        CommunityResourceKind::Reply => "reply",
    }
}

fn validate_query(
    offset: usize,
    search: &str,
    scope: CommunityResourceScope,
    token: Option<&str>,
) -> Result<(), AccountError> {
    if !valid_query(offset, search) || scope != CommunityResourceScope::All && token.is_none() {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_page(
    page: &CommunityResourcePage,
    kind: CommunityResourceKind,
) -> Result<(), AccountError> {
    if page.items.len() > MAXIMUM_PAGE_ITEMS || (page.has_more && page.items.is_empty()) {
        return Err(AccountError::Unavailable);
    }
    let mut ids = std::collections::BTreeSet::new();
    if page
        .items
        .iter()
        .any(|item| item.kind != kind || !ids.insert(item.id) || validate_resource(item).is_err())
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_resource(value: &CommunityResource) -> Result<(), AccountError> {
    if value.id.is_nil()
        || value.revision == 0
        || !valid_name(&value.name)
        || !valid_description(&value.description)
        || !valid_author(&value.author)
        || value.saves > MAXIMUM_JAVASCRIPT_INTEGER
        || !valid_rating(value.rating_count, value.rating_average, value.my_rating)
        || validate_content(value.kind, &value.content).is_err()
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_publication(
    id: Uuid,
    kind: CommunityResourceKind,
    name: &str,
    description: &str,
    content: &CommunityResourceContent,
    revision: u32,
) -> Result<(), AccountError> {
    if id.is_nil()
        || revision > 50_000
        || !valid_name(name)
        || !valid_description(description)
        || validate_content(kind, content).is_err()
        || serde_json::to_vec(content)
            .map(|bytes| bytes.len() > MAXIMUM_CONTENT_BYTES)
            .unwrap_or(true)
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_content(
    kind: CommunityResourceKind,
    content: &CommunityResourceContent,
) -> Result<(), AccountError> {
    match kind {
        CommunityResourceKind::Reply => {
            if !content.entries.is_empty()
                || content
                    .prompt
                    .as_deref()
                    .is_none_or(|prompt| !valid_text(prompt, 1, 2_000, true))
            {
                return Err(AccountError::Unavailable);
            }
        }
        CommunityResourceKind::Dictionary => {
            if content.prompt.is_some() || !(1..=128).contains(&content.entries.len()) {
                return Err(AccountError::Unavailable);
            }
            let mut seen = std::collections::BTreeSet::new();
            for entry in &content.entries {
                if !valid_text(&entry.code, 1, 256, false)
                    || !valid_text(&entry.word, 1, 1_024, false)
                    || entry.weight < 0
                    || !seen.insert((
                        format!("{:?}", entry.kind),
                        entry.code.clone(),
                        entry.word.clone(),
                    ))
                {
                    return Err(AccountError::Unavailable);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn resource_query_preserves_utf8_and_scope() {
        assert_eq!(percent_encode("C++ 词"), "C%2B%2B%20%E8%AF%8D");
        assert!(validate_query(0, "词", CommunityResourceScope::All, None).is_ok());
        assert!(validate_query(0, "词", CommunityResourceScope::Mine, None).is_err());
    }

    #[test]
    fn resource_content_is_kind_specific_and_bounded() {
        let reply = CommunityResourceContent {
            prompt: Some("请简洁回复".into()),
            ..Default::default()
        };
        assert!(validate_content(CommunityResourceKind::Reply, &reply).is_ok());
        assert!(validate_content(
            CommunityResourceKind::Reply,
            &CommunityResourceContent {
                entries: vec![SharedWord {
                    kind: DictionaryKind::Quick,
                    code: "x".into(),
                    word: "y".into(),
                    weight: 1,
                }],
                prompt: None,
            }
        )
        .is_err());
    }

    #[test]
    fn resource_api_rejects_nil_ids_before_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert!(matches!(
            CommunityResourceApi::community_resource(&client, Uuid::nil(), None),
            Err(AccountError::Invalid)
        ));
        assert!(matches!(
            CommunityResourceApi::save_community_resource(&client, Uuid::nil(), true, "fixture"),
            Err(AccountError::Invalid)
        ));
        assert!(matches!(
            CommunityResourceApi::delete_community_resource(&client, Uuid::nil(), "fixture"),
            Err(AccountError::Invalid)
        ));
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }
}

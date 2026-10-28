//! Bounded community keyboard-skin browsing independent of UI and platform hosts.
//! Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
//! (`SkinCommunityAPI.swift`, `CustomKeyboardSkin.swift`).

use super::category::{SkinCategory, INCLUDE_CATEGORY};
use crate::account::{
    request_with_account_session, with_shared_access_token, AccountApi, AccountError,
    AccountFailure, AccountSessionStorage, BackendAccountClient, BackendAccountSession,
};
use crate::cloud::dictionary::percent_encode;
use crate::community::write_actions::{delete_community_item, rate_community_item};
use crate::community::{
    valid_author, valid_description, valid_name, valid_query, valid_rating,
    CommunityDeletionResponse, CommunityModeration, CommunityRatingResponse,
    MAXIMUM_JAVASCRIPT_INTEGER, MAXIMUM_PAGE_ITEMS, MODERATION_FIELDS,
};
use crate::preferences::TouchKeyboardSkinDesign;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunitySkin {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub author: String,
    pub design: TouchKeyboardSkinDesign,
    pub downloads: u64,
    pub rating_count: u64,
    pub rating_average: f64,
    pub owned: bool,
    pub my_rating: u8,
    /// The moderation state, sent only for the signed-in user's own item and only to a request that asked for it with `fields=moderation`; other users' items and older servers leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation: Option<CommunityModeration>,
    /// 发布分类。客户端总是带 `include=category` 请求，早于分类功能的服务端不返回它，此时为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<SkinCategory>,
    /// 作品的来源，只在请求排除 AI 生成的皮肤时（[`EXCLUDE_AI_GENERATED`]）带 `fields=origin` 要它，其他请求不返回，此时为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<SkinOrigin>,
}

/// 社区皮肤的来源，由服务端在发布时判定：比对 AI 皮肤生成留下的插画指纹，或者发布请求声明了 AI 生成。`unknown` 是服务端推不出来源的作品：来源功能上线前、以及滚动升级期间旧副本写入的带照片的作品，和服务端后台回填还没处理到的作品；服务端将来新增的值也读作 `unknown`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinOrigin {
    User,
    Ai,
    #[serde(other)]
    Unknown,
}

/// 排除 AI 生成皮肤的查询参数。服务端在分页之前只留下 `origin` 为 `user` 的作品，`ai` 和所有 `unknown` 都去掉，详情和下载对去掉的作品答 404。
const EXCLUDE_AI_GENERATED: &str = "exclude=ai";
/// 认得 [`EXCLUDE_AI_GENERATED`] 的服务端在响应里带的能力头。旧部署忽略 `exclude` 照常作答，所以没有这个头的成功响应不能当作已经排除；旧部署的 `fields` 不认 `origin`，列表和详情会答 400 `invalid_fields`。
const ORIGIN_HEADER: &str = "X-MSIME-Skin-Origin";
const ORIGIN_VERSION: &str = "v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunitySkinPage {
    pub skins: Vec<CommunitySkin>,
    pub has_more: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunitySkinDownload {
    design: TouchKeyboardSkinDesign,
}

#[derive(Serialize)]
struct CommunitySkinPublishRequest<'a> {
    id: Uuid,
    name: &'a str,
    description: &'a str,
    design: &'a TouchKeyboardSkinDesign,
    /// `None` 时不发送该字段，由服务端归入默认分类。
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<SkinCategory>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunitySkinPublishResponse {
    id: Uuid,
}

pub trait CommunitySkinApi: Send + Sync + 'static {
    /// One page of published skins; `mine` lists only the signed-in user's own, removed ones included, with their moderation state. `exclude_ai_generated` 为真时只列来源为 `user` 的皮肤，服务端没有声明支持时是 [`AccountError::Unavailable`]。
    fn community_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<SkinCategory>,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<CommunitySkinPage, AccountError>;
    /// `exclude_ai_generated` 为真时来源不是 `user` 的皮肤（AI 生成的和来源不明的）是 [`AccountError::NotFound`]。
    fn community_skin(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<CommunitySkin, AccountError>;
    /// `exclude_ai_generated` 为真时来源不是 `user` 的皮肤（AI 生成的和来源不明的）是 [`AccountError::NotFound`]。
    fn download_community_skin(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
        token: &str,
    ) -> Result<TouchKeyboardSkinDesign, AccountError>;
    fn rate_community_skin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError>;
    fn publish_community_skin(
        &self,
        id: Uuid,
        name: &str,
        description: &str,
        design: &TouchKeyboardSkinDesign,
        category: Option<SkinCategory>,
        token: &str,
    ) -> Result<(), AccountError>;
    fn unpublish_community_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError>;
    fn set_community_skin_category(
        &self,
        id: Uuid,
        category: SkinCategory,
        token: &str,
    ) -> Result<CommunitySkin, AccountError>;
}

impl BackendAccountClient {
    /// 排除 AI 生成皮肤的请求：带上 [`EXCLUDE_AI_GENERATED`]，并要求响应带着 [`ORIGIN_HEADER`]；不排除时与以前完全相同。排除时的 400 一律按 [`AccountError::Unavailable`] 报告：查询参数由本端拼出、事先校验过，服务端答 400 只会是旧部署不认 `fields=origin`（`invalid_fields`），与没有能力头是同一回事，页面显示「社区暂不可用」而不是参数错误。
    fn community_skin_json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<T, AccountError> {
        if exclude_ai_generated {
            self.json_with_required_header(method, path, token, ORIGIN_HEADER, ORIGIN_VERSION)
                .map_err(|error| match error {
                    AccountError::Invalid => AccountError::Unavailable,
                    other => other,
                })
        } else {
            self.json::<T, ()>(method, path, token, None)
        }
    }
}

/// 排除 AI 生成皮肤时要的 `fields`：作者自己的作品另要审核状态。
fn origin_fields(moderation: bool) -> &'static str {
    if moderation {
        "fields=moderation,origin"
    } else {
        "fields=origin"
    }
}

impl CommunitySkinApi for BackendAccountClient {
    fn community_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<SkinCategory>,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<CommunitySkinPage, AccountError> {
        validate_query(offset, search)?;
        if mine && token.is_none() {
            return Err(AccountError::Unauthorized);
        }
        let scope = match (mine, exclude_ai_generated) {
            (true, false) => format!("&scope=mine&{MODERATION_FIELDS}"),
            (true, true) => format!("&scope=mine&{}", origin_fields(true)),
            (false, false) => String::new(),
            (false, true) => format!("&{}", origin_fields(false)),
        };
        let filter = category
            .map(|category| format!("&category={}", category.as_str()))
            .unwrap_or_default();
        let exclude = if exclude_ai_generated {
            format!("&{EXCLUDE_AI_GENERATED}")
        } else {
            String::new()
        };
        let path = format!(
            "/v1/community/skins?offset={offset}&q={}{scope}{filter}&{INCLUDE_CATEGORY}{exclude}",
            percent_encode(search)
        );
        let mut page = self.community_skin_json::<CommunitySkinPage>(
            Method::GET,
            &path,
            exclude_ai_generated,
            token,
        )?;
        validate_page(&page)?;
        if exclude_ai_generated {
            // 服务端已经在分页前只留下 `user`；它若仍交来别的来源（`ai`、`unknown`、没有 `origin` 或将来的新值），这里再去掉一次，不让它出现在列表里。
            page.skins
                .retain(|skin| skin.origin == Some(SkinOrigin::User));
        }
        Ok(page)
    }

    fn community_skin(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<CommunitySkin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = if exclude_ai_generated {
            format!(
                "/v1/community/skins/{}?{}&{INCLUDE_CATEGORY}&{EXCLUDE_AI_GENERATED}",
                id.hyphenated(),
                origin_fields(true)
            )
        } else {
            format!(
                "/v1/community/skins/{}?{MODERATION_FIELDS}&{INCLUDE_CATEGORY}",
                id.hyphenated()
            )
        };
        let skin = self.community_skin_json::<CommunitySkin>(
            Method::GET,
            &path,
            exclude_ai_generated,
            token,
        )?;
        validate_skin(&skin)?;
        if skin.id != id {
            return Err(AccountError::Unavailable);
        }
        if exclude_ai_generated && skin.origin != Some(SkinOrigin::User) {
            return Err(AccountError::NotFound);
        }
        Ok(skin)
    }

    fn download_community_skin(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
        token: &str,
    ) -> Result<TouchKeyboardSkinDesign, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = if exclude_ai_generated {
            format!(
                "/v1/community/skins/{}/download?{EXCLUDE_AI_GENERATED}",
                id.hyphenated()
            )
        } else {
            format!("/v1/community/skins/{}/download", id.hyphenated())
        };
        let result = self.community_skin_json::<CommunitySkinDownload>(
            Method::POST,
            &path,
            exclude_ai_generated,
            Some(token),
        )?;
        if !result.design.validate() {
            return Err(AccountError::Unavailable);
        }
        Ok(result.design.normalized())
    }

    fn rate_community_skin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError> {
        rate_community_item("/v1/community/skins", id, stars, |path, request| {
            self.json::<CommunityRatingResponse, _>(Method::PUT, path, Some(token), Some(request))
        })
    }

    fn publish_community_skin(
        &self,
        id: Uuid,
        name: &str,
        description: &str,
        design: &TouchKeyboardSkinDesign,
        category: Option<SkinCategory>,
        token: &str,
    ) -> Result<(), AccountError> {
        validate_publish(id, name, description, design)?;
        let path = "/v1/community/skins";
        let result = self.json::<CommunitySkinPublishResponse, _>(
            Method::POST,
            path,
            Some(token),
            Some(&CommunitySkinPublishRequest {
                id,
                name,
                description,
                design,
                category,
            }),
        )?;
        if result.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }

    fn unpublish_community_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        delete_community_item("/v1/community/skins", id, |path| {
            self.json::<CommunityDeletionResponse, ()>(Method::DELETE, path, Some(token), None)
        })
    }

    fn set_community_skin_category(
        &self,
        id: Uuid,
        category: SkinCategory,
        token: &str,
    ) -> Result<CommunitySkin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct CategoryRequest {
            category: SkinCategory,
        }
        let path = format!("/v1/community/skins/{}?{INCLUDE_CATEGORY}", id.hyphenated());
        let skin = self.json::<CommunitySkin, _>(
            Method::PATCH,
            &path,
            Some(token),
            Some(&CategoryRequest { category }),
        )?;
        validate_skin(&skin)?;
        // 回显的分类不一致说明修改没有生效。
        if skin.id != id || skin.category != Some(category) {
            return Err(AccountError::Unavailable);
        }
        Ok(skin)
    }
}

pub struct BackendCommunitySkinService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCommunitySkinService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCommunitySkinService<A, S>
where
    A: AccountApi + CommunitySkinApi,
    S: AccountSessionStorage,
{
    /// One page of published skins, newest first. `mine` lists only the signed-in user's own, removed ones included, and so requires a session. `category` 为 `Some` 时只列出该分类。`exclude_ai_generated` 为真时只列来源为 `user` 的皮肤，AI 生成的和来源不明的都不列（HarmonyOS 在 AI 生成类资质到位之前），服务端没有声明支持这项筛选时是 [`AccountError::Unavailable`]，宁可不显示也不把没筛过的列表交出去。
    pub fn list(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<SkinCategory>,
        exclude_ai_generated: bool,
    ) -> Result<CommunitySkinPage, AccountError> {
        validate_query(offset, search)?;
        request_with_account_session(&self.api, &self.session, mine, |api, token| {
            api.community_skins(offset, search, mine, category, exclude_ai_generated, token)
        })
    }

    /// `exclude_ai_generated` 与 [`Self::list`] 相同，来源不是 `user` 的皮肤是 [`AccountError::NotFound`]。
    pub fn detail(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
    ) -> Result<CommunitySkin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, false, |api, token| {
            api.community_skin(id, exclude_ai_generated, token)
        })
    }

    /// `exclude_ai_generated` 与 [`Self::detail`] 相同。
    pub fn download(
        &self,
        id: Uuid,
        exclude_ai_generated: bool,
    ) -> Result<TouchKeyboardSkinDesign, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.download_community_skin(
                id,
                exclude_ai_generated,
                token.ok_or(AccountError::Unauthorized)?,
            )
        })
    }

    /// 与 [`Self::download`] 相同，只是没有登录时用本机匿名账号 `anonymous` 下载（#8201 的决定 13，服务端的 `communityDownload` 收匿名账号）：登录账号优先，取令牌和 401 后的重试与释义相同（[`with_shared_access_token`]）。两个会话都没有时仍是 [`AccountError::Unauthorized`]。下载次数记在下载所用的账号上。
    pub fn download_with_anonymous<S2: AccountSessionStorage>(
        &self,
        anonymous: &BackendAccountSession<A, S2>,
        id: Uuid,
        exclude_ai_generated: bool,
    ) -> Result<TouchKeyboardSkinDesign, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        with_shared_access_token(&self.session, anonymous, None, |api, token| {
            api.download_community_skin(id, exclude_ai_generated, token)
                .map_err(AccountFailure::from)
        })
        .map(|(design, _)| design)
        .map_err(|failure| failure.error)
    }

    pub fn rate(&self, id: Uuid, stars: u8) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.rate_community_skin(id, stars, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// `category` 为 `None` 时不发送分类，由服务端归入默认分类。
    pub fn publish(
        &self,
        id: Uuid,
        name: &str,
        description: &str,
        design: &TouchKeyboardSkinDesign,
        category: Option<SkinCategory>,
    ) -> Result<(), AccountError> {
        validate_publish(id, name, description, design)?;
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.publish_community_skin(
                id,
                name,
                description,
                design,
                category,
                token.ok_or(AccountError::Unauthorized)?,
            )
        })
    }

    pub fn unpublish(&self, id: Uuid) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.unpublish_community_skin(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// 修改自己作品的发布分类。
    pub fn set_category(
        &self,
        id: Uuid,
        category: SkinCategory,
    ) -> Result<CommunitySkin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.set_community_skin_category(id, category, token.ok_or(AccountError::Unauthorized)?)
        })
    }
}

fn validate_query(offset: usize, search: &str) -> Result<(), AccountError> {
    if !valid_query(offset, search) {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_publish(
    id: Uuid,
    name: &str,
    description: &str,
    design: &TouchKeyboardSkinDesign,
) -> Result<(), AccountError> {
    if id.is_nil() || !valid_name(name) || !valid_description(description) || !design.validate() {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_page(page: &CommunitySkinPage) -> Result<(), AccountError> {
    if page.skins.len() > MAXIMUM_PAGE_ITEMS
        || (page.has_more && page.skins.is_empty())
        || page.skins.iter().any(|skin| validate_skin(skin).is_err())
    {
        return Err(AccountError::Unavailable);
    }
    let mut ids = HashSet::with_capacity(page.skins.len());
    if page.skins.iter().any(|skin| !ids.insert(skin.id)) {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_skin(skin: &CommunitySkin) -> Result<(), AccountError> {
    if skin.id.is_nil()
        || !valid_name(&skin.name)
        || !valid_description(&skin.description)
        || !valid_author(&skin.author)
        || !skin.design.validate()
        || skin.downloads > MAXIMUM_JAVASCRIPT_INTEGER
        || !valid_rating(skin.rating_count, skin.rating_average, skin.my_rating)
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod anonymous_tests;

#[cfg(test)]
mod origin_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        AccountChallenge, AccountProfile, AccountTokens, AccountUser, SavedAccountSession,
    };
    use crate::community::MAXIMUM_OFFSET;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn token(byte: u8) -> String {
        std::iter::repeat_n(char::from(byte), 64).collect()
    }

    fn user() -> AccountUser {
        AccountUser {
            id: "fixture-user".into(),
            display_name: "Fixture".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        }
    }

    fn tokens(access: u8, refresh: u8) -> AccountTokens {
        AccountTokens {
            access_token: token(access),
            refresh_token: token(refresh),
            token_type: "Bearer".into(),
            expires_in: 900,
            user: user(),
        }
    }

    fn valid_future_expiry() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 60_000
    }

    fn skin() -> CommunitySkin {
        CommunitySkin {
            id: Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
            name: "合成皮肤".into(),
            description: "仅用于协议测试".into(),
            author: "Fixture".into(),
            design: TouchKeyboardSkinDesign::default(),
            downloads: 2,
            rating_count: 1,
            rating_average: 4.0,
            owned: false,
            my_rating: 0,
            moderation: None,
            category: Some(SkinCategory::Nature),
            origin: None,
        }
    }

    /// 在本机起一个只应答一次的 HTTP 服务，返回它的地址和收到的完整请求。
    fn serve_once(response: Vec<u8>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (sent, received) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let length = stream.read(&mut request).unwrap();
            sent.send(String::from_utf8_lossy(&request[..length]).into_owned())
                .unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
                response.len()
            )
            .unwrap();
            stream.write_all(&response).unwrap();
        });
        (origin, received)
    }

    #[derive(Clone, Default)]
    struct MemoryStorage(Arc<Mutex<Option<SavedAccountSession>>>);

    impl AccountSessionStorage for MemoryStorage {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            self.0
                .lock()
                .map(|value| value.clone())
                .map_err(|_| AccountError::Storage)
        }
        fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
            *self.0.lock().map_err(|_| AccountError::Storage)? = Some(session.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), AccountError> {
            *self.0.lock().map_err(|_| AccountError::Storage)? = None;
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct FakeApi {
        skin_calls: Arc<AtomicUsize>,
    }

    impl AccountApi for FakeApi {
        fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
            Ok(HashMap::new())
        }
        fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
            Ok(tokens(b'c', b'd'))
        }
        fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
            Err(AccountError::Unavailable)
        }
        fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }
        fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
            Ok(())
        }
        fn delete_account(&self, _: &str) -> Result<(), AccountError> {
            Ok(())
        }
    }

    impl CommunitySkinApi for FakeApi {
        fn community_skins(
            &self,
            _: usize,
            _: &str,
            _: bool,
            _: Option<SkinCategory>,
            _: bool,
            bearer: Option<&str>,
        ) -> Result<CommunitySkinPage, AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            if bearer == Some(token(b'a').as_str()) {
                return Err(AccountError::Unauthorized);
            }
            Ok(CommunitySkinPage {
                skins: vec![skin()],
                has_more: false,
            })
        }
        fn community_skin(
            &self,
            id: Uuid,
            _: bool,
            _: Option<&str>,
        ) -> Result<CommunitySkin, AccountError> {
            let mut value = skin();
            value.id = id;
            Ok(value)
        }
        fn download_community_skin(
            &self,
            _: Uuid,
            _: bool,
            bearer: &str,
        ) -> Result<TouchKeyboardSkinDesign, AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            if bearer == token(b'a') {
                return Err(AccountError::Unauthorized);
            }
            Ok(TouchKeyboardSkinDesign::default())
        }
        fn rate_community_skin(&self, _: Uuid, _: u8, _: &str) -> Result<(), AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn publish_community_skin(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
            _: &TouchKeyboardSkinDesign,
            _: Option<SkinCategory>,
            _: &str,
        ) -> Result<(), AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn unpublish_community_skin(&self, _: Uuid, _: &str) -> Result<(), AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn set_community_skin_category(
            &self,
            id: Uuid,
            category: SkinCategory,
            bearer: &str,
        ) -> Result<CommunitySkin, AccountError> {
            self.skin_calls.fetch_add(1, Ordering::SeqCst);
            if bearer == token(b'a') {
                return Err(AccountError::Unauthorized);
            }
            let mut value = skin();
            value.id = id;
            value.category = Some(category);
            Ok(value)
        }
    }

    #[test]
    fn authenticated_reads_refresh_once_after_unauthorized() {
        let storage = MemoryStorage::default();
        *storage.0.lock().unwrap() = Some(SavedAccountSession {
            tokens: tokens(b'a', b'b'),
            expires_at_unix_ms: valid_future_expiry(),
            session_id: None,
        });
        let api = FakeApi::default();
        let calls = Arc::clone(&api.skin_calls);
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
        let service = BackendCommunitySkinService::new(api, session);
        assert_eq!(
            service.list(0, "", false, None, false).unwrap().skins.len(),
            1
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn anonymous_reads_do_not_require_or_create_a_session() {
        let storage = MemoryStorage::default();
        let api = FakeApi::default();
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage.clone()));
        let service = BackendCommunitySkinService::new(api, session);
        assert_eq!(service.detail(skin().id, false).unwrap().name, "合成皮肤");
        assert!(storage.load().unwrap().is_none());
        // The user's own list needs a session and never sends the request without one.
        assert_eq!(
            service.list(0, "", true, None, false),
            Err(AccountError::Unauthorized)
        );
    }

    #[test]
    fn writes_require_login_refresh_once_and_validate_stars() {
        let storage = MemoryStorage::default();
        let api = FakeApi::default();
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage.clone()));
        let service = BackendCommunitySkinService::new(api.clone(), session);
        assert_eq!(
            service.detail(Uuid::nil(), false),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            service.download(Uuid::nil(), false),
            Err(AccountError::Invalid)
        );
        assert_eq!(service.rate(Uuid::nil(), 5), Err(AccountError::Invalid));
        assert_eq!(service.unpublish(Uuid::nil()), Err(AccountError::Invalid));
        assert_eq!(
            service.download(skin().id, false),
            Err(AccountError::Unauthorized)
        );
        assert_eq!(service.rate(skin().id, 0), Err(AccountError::Invalid));
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 0);

        *storage.0.lock().unwrap() = Some(SavedAccountSession {
            tokens: tokens(b'a', b'b'),
            expires_at_unix_ms: valid_future_expiry(),
            session_id: None,
        });
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
        let service = BackendCommunitySkinService::new(api.clone(), session);
        assert_eq!(
            service.download(skin().id, false).unwrap(),
            TouchKeyboardSkinDesign::default()
        );
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 2);
        service.rate(skin().id, 5).unwrap();
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 3);
        service
            .publish(
                skin().id,
                "发布皮肤",
                "公开说明",
                &TouchKeyboardSkinDesign::default(),
                Some(SkinCategory::Cute),
            )
            .unwrap();
        service.unpublish(skin().id).unwrap();
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn rejects_invalid_queries_and_skin_metadata() {
        assert_eq!(
            validate_query(MAXIMUM_OFFSET + 1, ""),
            Err(AccountError::Invalid)
        );
        assert_eq!(validate_query(0, "bad\nquery"), Err(AccountError::Invalid));
        assert_eq!(
            validate_publish(
                Uuid::nil(),
                "名称",
                "说明",
                &TouchKeyboardSkinDesign::default()
            ),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            validate_publish(
                Uuid::new_v4(),
                "名称\n",
                "说明",
                &TouchKeyboardSkinDesign::default()
            ),
            Err(AccountError::Invalid)
        );
        let mut value = skin();
        value.rating_average = f64::NAN;
        assert_eq!(validate_skin(&value), Err(AccountError::Unavailable));
        value = skin();
        value.id = Uuid::nil();
        assert_eq!(validate_skin(&value), Err(AccountError::Unavailable));
        value = skin();
        value.downloads = MAXIMUM_JAVASCRIPT_INTEGER + 1;
        assert_eq!(validate_skin(&value), Err(AccountError::Unavailable));
        let page = CommunitySkinPage {
            skins: vec![skin(), skin()],
            has_more: false,
        };
        assert_eq!(validate_page(&page), Err(AccountError::Unavailable));
    }

    #[test]
    fn transport_percent_encodes_search_and_validates_response() {
        let response = serde_json::to_vec(&CommunitySkinPage {
            skins: vec![skin()],
            has_more: false,
        })
        .unwrap();
        let (origin, received) = serve_once(response);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let page = client
            .community_skins(7, "C++ 星", false, None, false, None)
            .unwrap();
        assert_eq!(page.skins.len(), 1);
        assert!(received.recv().unwrap().starts_with(
            "GET /v1/community/skins?offset=7&q=C%2B%2B%20%E6%98%9F&include=category HTTP/1.1"
        ));
        assert_eq!(
            client.community_skins(0, "", true, None, false, None),
            Err(AccountError::Unauthorized)
        );
    }

    #[test]
    fn transport_uses_authenticated_write_contracts() {
        let id = skin().id;
        let (origin, received) = serve_once(
            serde_json::to_vec(&serde_json::json!({
                "design": TouchKeyboardSkinDesign::default()
            }))
            .unwrap(),
        );
        let client = BackendAccountClient::loopback(&origin).unwrap();
        client
            .download_community_skin(id, false, &token(b'a'))
            .unwrap();
        let request = received.recv().unwrap();
        assert!(request.starts_with(&format!(
            "POST /v1/community/skins/{}/download HTTP/1.1",
            id.hyphenated()
        )));
        assert!(request.contains("authorization: Bearer "));

        let (origin, received) = serve_once(br#"{"stars":4}"#.to_vec());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        client.rate_community_skin(id, 4, &token(b'b')).unwrap();
        let request = received.recv().unwrap();
        assert!(request.starts_with(&format!(
            "PUT /v1/community/skins/{}/rating HTTP/1.1",
            id.hyphenated()
        )));
        assert!(request.ends_with("\r\n\r\n{\"stars\":4}"));

        let (origin, received) =
            serve_once(serde_json::to_vec(&serde_json::json!({ "id": id })).unwrap());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        client
            .publish_community_skin(
                id,
                "发布皮肤",
                "公开说明",
                &TouchKeyboardSkinDesign::default(),
                Some(SkinCategory::Guofeng),
                &token(b'c'),
            )
            .unwrap();
        let request = received.recv().unwrap();
        assert!(request.starts_with("POST /v1/community/skins HTTP/1.1"));
        assert!(request.contains("authorization: Bearer "));
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["id"], id.to_string());
        assert_eq!(body["name"], "发布皮肤");
        assert_eq!(body["description"], "公开说明");
        assert!(body["design"].is_object());
        assert_eq!(body["category"], "guofeng");

        let (origin, received) = serve_once(br#"{"deleted":true}"#.to_vec());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        client.unpublish_community_skin(id, &token(b'd')).unwrap();
        let request = received.recv().unwrap();
        assert!(request.starts_with(&format!(
            "DELETE /v1/community/skins/{} HTTP/1.1",
            id.hyphenated()
        )));
        assert!(request.contains("authorization: Bearer "));
    }

    #[test]
    fn categories_round_trip_and_unknown_ones_read_as_other() {
        let ids = [
            "nature", "guofeng", "acg", "cute", "food", "tech", "minimal", "other",
        ];
        for (category, id) in SkinCategory::ALL.into_iter().zip(ids) {
            assert_eq!(category.as_str(), id);
            assert_eq!(serde_json::to_value(category).unwrap(), id);
        }
        let value = skin();
        let parsed: CommunitySkin =
            serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
        assert_eq!(parsed, value);

        // 服务端将来新增的分类读作其他，旧客户端仍能读出条目。
        let mut json = serde_json::to_value(skin()).unwrap();
        json["category"] = serde_json::json!("seasonal");
        let parsed: CommunitySkin = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.category, Some(SkinCategory::Other));
    }

    #[test]
    fn skins_without_a_category_still_read_and_other_unknown_fields_are_refused() {
        let mut json = serde_json::to_value(skin()).unwrap();
        json.as_object_mut().unwrap().remove("category");
        let parsed: CommunitySkin = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed.category, None);
        // 不带分类的条目也不会把 `category` 写回给页面。
        assert!(!serde_json::to_value(&parsed)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("category"));
        json["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CommunitySkin>(json).is_err());
    }

    #[test]
    fn a_publish_without_a_category_omits_it() {
        let design = TouchKeyboardSkinDesign::default();
        let body = serde_json::to_value(CommunitySkinPublishRequest {
            id: skin().id,
            name: "发布皮肤",
            description: "",
            design: &design,
            category: None,
        })
        .unwrap();
        assert!(!body.as_object().unwrap().contains_key("category"));
    }

    #[test]
    fn transport_filters_by_category_and_always_includes_it() {
        let response = serde_json::to_vec(&CommunitySkinPage {
            skins: vec![skin()],
            has_more: false,
        })
        .unwrap();
        let (origin, received) = serve_once(response);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let page = client
            .community_skins(0, "", false, Some(SkinCategory::Acg), false, None)
            .unwrap();
        assert_eq!(page.skins, vec![skin()]);
        assert!(received.recv().unwrap().starts_with(
            "GET /v1/community/skins?offset=0&q=&category=acg&include=category HTTP/1.1"
        ));

        let (origin, received) = serve_once(serde_json::to_vec(&skin()).unwrap());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client.community_skin(skin().id, false, None).unwrap(),
            skin()
        );
        assert!(received.recv().unwrap().starts_with(&format!(
            "GET /v1/community/skins/{}?fields=moderation&include=category HTTP/1.1",
            skin().id.hyphenated()
        )));
    }

    #[test]
    fn transport_sets_the_category_by_id() {
        let mut food = skin();
        food.category = Some(SkinCategory::Food);
        let (origin, received) = serve_once(serde_json::to_vec(&food).unwrap());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client
                .set_community_skin_category(skin().id, SkinCategory::Food, &token(b'c'))
                .unwrap(),
            food
        );
        let request = received.recv().unwrap();
        assert!(request.starts_with(&format!(
            "PATCH /v1/community/skins/{}?include=category HTTP/1.1",
            skin().id.hyphenated()
        )));
        assert!(request.contains("authorization: Bearer "));
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body).unwrap(),
            serde_json::json!({ "category": "food" })
        );

        // 回显的分类不是请求的分类，说明修改没有生效。
        let (origin, _received) = serve_once(serde_json::to_vec(&skin()).unwrap());
        let client = BackendAccountClient::loopback(&origin).unwrap();
        assert_eq!(
            client.set_community_skin_category(skin().id, SkinCategory::Food, &token(b'c')),
            Err(AccountError::Unavailable)
        );
        assert_eq!(
            client.set_community_skin_category(Uuid::nil(), SkinCategory::Food, &token(b'c')),
            Err(AccountError::Invalid)
        );
    }

    #[test]
    fn setting_a_category_requires_login_and_refreshes_once() {
        let storage = MemoryStorage::default();
        let api = FakeApi::default();
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage.clone()));
        let service = BackendCommunitySkinService::new(api.clone(), session);
        assert_eq!(
            service.set_category(Uuid::nil(), SkinCategory::Food),
            Err(AccountError::Invalid)
        );
        assert_eq!(
            service.set_category(skin().id, SkinCategory::Food),
            Err(AccountError::Unauthorized)
        );
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 0);

        *storage.0.lock().unwrap() = Some(SavedAccountSession {
            tokens: tokens(b'a', b'b'),
            expires_at_unix_ms: valid_future_expiry(),
            session_id: None,
        });
        let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
        let service = BackendCommunitySkinService::new(api.clone(), session);
        let updated = service.set_category(skin().id, SkinCategory::Food).unwrap();
        assert_eq!(updated.category, Some(SkinCategory::Food));
        assert_eq!(api.skin_calls.load(Ordering::SeqCst), 2);
    }
}

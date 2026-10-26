//! 把已安装的数据类插件包发布到社区，并列出、删除账号自己发布的包。
//!
//! 账号会话与皮肤社区工具相同：用户在设置应用里登录后保存的那份，位置见 [`crate::community::session_location`]，本服务器从不登录。发布走设置页「发布我的插件」对话框同一条路：`community::pack_preview` 先按分享规则在本机打一次包（被拒时给出具体原因，也拿到默认的标题和简介），再由 `BackendCommunityPluginService::publish_installed` 打包上传，字段与对话框发送的相同（类型、插件 id、发布 ID、标题、简介）。插件社区没有私有或同步：发布即公开，所以工具说明要求代理先征得用户同意。

use crate::community::{session_location, SIGN_IN};
use crate::plugins::{not_installed, parse_kind, root};
use msime_client_core::edition::Edition;
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionFileLayout, BackendAccountClient,
    BackendAccountSession, FileAccountSessionStorage,
};
use msime_client_services::plugins::community::{
    self, BackendCommunityPluginService, CommunityPluginApi, PUBLISHABLE_KINDS,
};
use msime_client_services::plugins::{self, PluginFailure};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// 社区列表标题的字符数上限，与 client-services 的 `valid_name` 相同。
const MAX_NAME_CHARS: usize = 32;
/// 社区列表简介的字符数上限，与 client-services 的 `valid_description` 相同。
const MAX_DESCRIPTION_CHARS: usize = 280;
/// 列出自己的包时最多翻几页。一个账号最多发布 20 个包，一页就装得下，这里只防服务器一直回 `has_more`。
const MAX_PAGES: usize = 10;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PublishPluginRequest {
    /// The installed pack's kind, as list_plugins gives it: sound, music, command_table, effect, phrase_table, helpcode, wordbook or symbol_set.
    pub kind: String,
    /// The installed pack's id, as list_plugins gives it.
    pub id: String,
    /// The listing's title, 1 to 32 characters. Defaults to the manifest's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The listing's text, at most 280 characters. Defaults to the manifest's description.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PublishedPlugin {
    /// The listing's id in the community, which delete_community_plugin takes.
    pub id: String,
    pub kind: String,
    /// The pack's manifest id.
    pub plugin_id: String,
    /// The listing's title.
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunityPluginList {
    /// Newest first, removed ones included.
    pub plugins: Vec<CommunityPluginView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunityPluginView {
    /// The listing's id in the community.
    pub id: String,
    pub kind: String,
    /// The pack's manifest id, the folder it installs into.
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub downloads: u64,
    /// When it was published, as the server reports it (RFC 3339).
    pub created_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CommunityPluginRequest {
    /// The community id list_community_plugins or publish_plugin gave.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeletedCommunityPlugin {
    pub id: String,
}

/// 插件社区服务和它用的账号会话：会话单独留一份，好在联网之前只读会话文件判断有没有人登录。
pub struct PluginCommunity<A: AccountApi> {
    session: Arc<BackendAccountSession<A, FileAccountSessionStorage>>,
    service: BackendCommunityPluginService<A, FileAccountSessionStorage>,
}

/// 用 `api` 和 `directory` 里的会话文件组成插件社区服务。
pub fn community_at<A>(
    api: A,
    directory: &Path,
    layout: AccountSessionFileLayout,
) -> PluginCommunity<A>
where
    A: AccountApi + CommunityPluginApi + Clone,
{
    let storage = FileAccountSessionStorage::new(directory, layout);
    let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
    PluginCommunity {
        service: BackendCommunityPluginService::new(api, Arc::clone(&session)),
        session,
    }
}

/// 设置应用登录后的插件社区服务，请求经正式的账号服务器。
pub fn community(
    state_dir: &Path,
    edition: &Edition,
) -> Result<PluginCommunity<BackendAccountClient>, String> {
    let (directory, layout) = session_location(state_dir, edition, |name| std::env::var_os(name))?;
    let client = BackendAccountClient::new()
        .map_err(|_| "cannot set up the connection to the community server")?;
    Ok(community_at(client, &directory, layout))
}

impl<A: AccountApi + CommunityPluginApi> PluginCommunity<A> {
    /// 有人登录时什么都不返回；没登录时是让用户去设置应用登录的说明。只读会话文件，不联网。
    fn signed_in(&self) -> Result<(), String> {
        match self.session.status() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(SIGN_IN.into()),
            Err(error) => Err(account_problem(error)),
        }
    }
}

/// 发布一个已安装的数据类插件包。先确认是用户装的、能分享的类型、已登录，再按分享规则在本机打包检查，都过了才联网。
pub fn publish<A: AccountApi + CommunityPluginApi>(
    state_dir: &Path,
    community: &PluginCommunity<A>,
    request: &PublishPluginRequest,
) -> Result<PublishedPlugin, String> {
    let kind = parse_kind(&request.kind)?;
    let id = request.id.as_str();
    if !PUBLISHABLE_KINDS.contains(&kind) {
        return Err("a script plugin cannot be published to the community; only data packs (sound, music, command_table, effect, phrase_table, helpcode, wordbook, symbol_set) can".into());
    }
    if plugins::is_builtin(kind, id) {
        return Err(format!(
            "{id} ships with the input method and cannot be published"
        ));
    }
    let root = root(state_dir);
    if !crate::plugins::valid_id(id)
        || std::fs::symlink_metadata(plugins::kind_directory(&root, kind).join(id)).is_err()
    {
        return Err(not_installed(kind, id));
    }
    community.signed_in()?;
    let preview = community::pack_preview(&root, kind, id).map_err(pack_problem)?;
    let name = match request.name.as_deref().map(str::trim) {
        Some(name) => name.to_owned(),
        None => preview.suggested_name,
    };
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("name must be 1 to {MAX_NAME_CHARS} characters"));
    }
    let description = match request.description.as_deref().map(str::trim) {
        Some(description) => description.to_owned(),
        None => preview.suggested_description,
    };
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description must be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    let item = community
        .service
        .publish_installed(&root, kind, id, Uuid::new_v4(), name, description)
        .map_err(pack_problem)?;
    Ok(PublishedPlugin {
        id: item.id.hyphenated().to_string(),
        kind: item.kind.as_str().to_owned(),
        plugin_id: item.plugin_id,
        name: item.name,
        version: item.version,
    })
}

/// 账号自己发布的全部包，被下架的也在内。
pub fn list<A: AccountApi + CommunityPluginApi>(
    community: &PluginCommunity<A>,
) -> Result<CommunityPluginList, String> {
    community.signed_in()?;
    let mut listed = Vec::new();
    let mut offset = 0;
    for _ in 0..MAX_PAGES {
        let page = community
            .service
            .list(offset, "", None, true)
            .map_err(account_problem)?;
        offset += page.plugins.len() + page.skipped;
        listed.extend(page.plugins.into_iter().map(|item| CommunityPluginView {
            id: item.id.hyphenated().to_string(),
            kind: item.kind.as_str().to_owned(),
            plugin_id: item.plugin_id,
            name: item.name,
            version: item.version,
            downloads: item.downloads,
            created_at: item.created_at,
        }));
        if !page.has_more {
            break;
        }
    }
    Ok(CommunityPluginList { plugins: listed })
}

/// 从社区删掉账号自己发布的一个包。本机装着的那份不动。
pub fn delete<A: AccountApi + CommunityPluginApi>(
    community: &PluginCommunity<A>,
    request: &CommunityPluginRequest,
) -> Result<DeletedCommunityPlugin, String> {
    let id = Uuid::parse_str(request.id.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            format!(
                "{:?} is not a community id; list_community_plugins gives the ids",
                request.id
            )
        })?;
    community.signed_in()?;
    community.service.delete(id).map_err(account_problem)?;
    Ok(DeletedCommunityPlugin {
        id: id.hyphenated().to_string(),
    })
}

/// 打包或发布被拒的原因：包的规则带着 client-services 报告的那一条，社区的拒绝按 `community_*` 代码说成代理知道下一步的话。
fn pack_problem(failure: PluginFailure) -> String {
    let detail = failure.detail.unwrap_or_default();
    match failure.code {
        "plugin_invalid" => format!("the pack does not pass the sharing rules: {detail}; fix it with get_plugin and create_plugin (replace: true)"),
        "plugin_archive" => format!("the pack cannot be packed for sharing: {detail}"),
        "plugin_reserved" => "this id belongs to a pack that ships with the input method and cannot be published".into(),
        "plugin_community_kind" => "a script plugin cannot be published to the community".into(),
        "plugin_community_too_large" => "too large to share: a pack may be at most 8 MB once zipped".into(),
        "plugin_community_mismatch" => "the pack changed while it was being packed; try again".into(),
        "plugin_storage" | "storage" => "the plugins folder could not be read".into(),
        code => match code.strip_prefix("community_") {
            Some(_) => community_problem(code),
            None => format!("the pack cannot be shared ({code})"),
        },
    }
}

fn community_problem(code: &str) -> String {
    match code {
        "community_unauthorized" => SIGN_IN.into(),
        "community_cancelled" => "the user signed out or switched accounts in the settings app while this ran; try again".into(),
        "community_forbidden" => "the community server does not allow this account to do that".into(),
        "community_not_found" => "this account has no such plugin in the community; list_community_plugins lists the ones it published".into(),
        "community_conflict" => "the community refused it: the listing changed, or the account has reached its limit of 20 published packs and 32 MB together. Delete one with delete_community_plugin first".into(),
        "community_rate_limited" => "an account may publish at most 10 times an hour; try again later".into(),
        "community_invalid" => "the community server did not accept the pack: check that the name, description and the pack's content meet the publishing rules".into(),
        "community_blocked_content" => "the community does not allow some words in the name, description or the pack; change them and try again".into(),
        "community_screening_unavailable" => "the community's content check is unavailable right now; try again later".into(),
        "community_account_banned" => "this account is banned from the community".into(),
        "community_storage" => "the account session on this computer could not be read or written".into(),
        _ => "the community server cannot be reached right now or gave an unexpected answer; try again later".into(),
    }
}

fn account_problem(error: AccountError) -> String {
    community_problem(error.community_code())
}

#[cfg(test)]
mod tests;

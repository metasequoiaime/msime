//! 把手机键盘皮肤发布到社区，并列出、删除账号自己发布的键盘皮肤。
//!
//! 键盘皮肤是 iOS、Android 和 HarmonyOS 触屏键盘的外观，一份 `TouchKeyboardSkinDesign` 文档（配色、键形和一张可选的背景照片），与候选窗皮肤包（`skin.toml` 加图片，见 [`crate::community`]）是两种东西，社区里也是两张表、两套 id。桌面上没有触屏键盘，所以这里不写本机的键盘皮肤库和偏好，只把代理给的设计交给共享的 `skin::community` 服务，与手机设置页的发布走同一个接口。
//!
//! 账号会话与其他社区工具相同：用户在设置应用里登录后保存的那份，位置见 [`crate::community::session_location`]，本服务器从不登录。键盘皮肤社区没有私有状态也没有编辑：发布即公开，之后只能删除再发，所以工具说明要求代理先征得用户同意。
//!
//! 照片只读不转码：文件必须已经是符合社区上限的 JPEG，这里按 [`crate::plugins`] 读本机文件的规则安全地读出来，写成 `photo` 字段要的标准 base64。读到的字节、令牌和会话都不进结果、错误和 stderr 的审计行。

use crate::community::{session_location, Category, SIGN_IN};
use msime_client_core::edition::Edition;
use msime_client_core::preferences::{
    TouchKeyboardSkinDesign, TouchSkinKeyMaterial, TouchSkinKeyShape,
};
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionFileLayout, BackendAccountClient,
    BackendAccountSession, FileAccountSessionStorage,
};
use msime_client_services::community::CommunityModeration;
use msime_client_services::skin::community::{BackendCommunitySkinService, CommunitySkinApi};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// 社区列表标题的字符数上限，与 client-services 的 `valid_name` 相同。
const MAX_NAME_CHARS: usize = 32;
/// 社区列表简介的字符数上限，与 client-services 的 `valid_description` 相同。
const MAX_DESCRIPTION_CHARS: usize = 280;
/// 照片文件的字节数上限，与 `TouchKeyboardSkinDesign::validate`（`crates/client-core/src/preferences.rs`）解码 `photo` 之后检查的 512,000 相同；那里是写在函数里的字面量，没有公开的常量可用，所以在这里照写一份。社区服务器对 JPEG 用的也是这个数。
const MAX_PHOTO_BYTES: u64 = 512_000;
/// JPEG 文件开头的 SOI 标记和紧跟的标记前缀。
const JPEG_START: [u8; 3] = [0xFF, 0xD8, 0xFF];
/// 列出自己的皮肤时最多翻几页。一个账号最多 50 款，一页 20 款，三页就装得下，这里只防服务器一直回 `has_more`。
const MAX_PAGES: usize = 5;
/// 一次列出的皮肤数上限，也就是 [`MAX_PAGES`] 页各 20 款。
const MAX_LISTED: usize = MAX_PAGES * 20;

const PHOTO_FILE_RULES: &str = "photo_file must be the absolute path of a regular file on this computer that this user can read, a JPEG of at most 512,000 bytes, with no symbolic link anywhere on the way (system links such as /tmp on macOS excepted); write the file there and pass that path";
const NOT_A_JPEG: &str = "photo_file is not a JPEG; save it as a baseline JPEG no larger than 1024 pixels on its long side and 512,000 bytes";
const PHOTO_TOO_LARGE: &str =
    "is larger than 512,000 bytes; scale it down or save it with more compression";
const OUT_OF_RANGE: &str = "the design is out of range and was not adjusted: cornerRadius 0 to 20, borderWidth 0 to 2, shadow 0 to 0.4, pattern 0 to 3, keyOpacity 0.25 to 1, patternOpacity 0 to 0.5, photoShade 0 to 0.8, photoPosition 0 to 1";

pub const PUBLISH_DESCRIPTION: &str = "Publish a phone keyboard skin to the 水杉输入法 keyboard skin community: the look of the touch keyboard on iOS, Android and HarmonyOS (its background, key colours, key shape and an optional background photo). This is not a candidate-window skin; those are made with create_candidate_skin and shared with publish_candidate_skin. Uses the account the user signed in to in the 水杉输入法 settings app. This server never signs in or handles passwords: when no one is signed in it says so, and you ask the user to sign in there and try again. A published keyboard skin is always public at once: everyone can see, download and rate it, and there is no private state. It cannot be edited afterwards, only deleted with delete_community_keyboard_skin and published again, which loses its downloads and ratings. Before publishing, ask the user in plain words to confirm that everyone may see it and that they made the photo or have the right to share it. Do not publish AI-generated artwork with this tool: a skin whose photo an image model made has to be declared as AI-generated, and this tool cannot declare it. name is the listing's title (1-32 characters) and description its text (at most 280); category is nature, guofeng, acg, cute, food, tech, minimal or other, and other when left out. design takes the colours background, keyBackground, keyForeground, accent and actionBackground (the return key) as \"#RRGGBB\", and optionally gradientEnd and customBorderColor (\"#RRGGBB\"), gradientHorizontal and monospaced (true or false), cornerRadius (0 to 20, default 8), borderWidth (0 to 2, default 0), shadow (0 to 0.4, default 0), pattern (0 plain, 1 dots, 2 grid, 3 waves, drawn in the accent colour), patternOpacity (0 to 0.5), keyShape (rounded, capsule, ticket or pebble), keyMaterial (flat, raised, glass or paper), keyOpacity (0.25 to 1), photoShade (0 to 0.8, a dark layer over the photo) and photoPosition (0 to 1, where the photo is cropped; 0.5 is the centre). A value outside its range is refused, not adjusted. photo_file is the absolute path of the background photo on this computer, which must already be a JPEG no larger than 1024 pixels on its long side and 512,000 bytes, saved as a baseline JPEG: make or convert it with code first, because this server does not convert, scale or re-encode images and only reads the file. It refuses a path with a symbolic link anywhere on the way or that is not a regular file, and refuses a file over 512,000 bytes before reading it. The photo covers the whole keyboard behind the keys and hides the gradient. Keep keyForeground clearly readable on keyBackground (a contrast of 4.5 to 1 or more), and with a photo keep keyOpacity around 0.5 to 0.7 so the letters stay readable. Each call makes a new skin, and an account keeps at most 50 keyboard skins; list_community_keyboard_skins shows them. id is only for repeating a publish that got no answer: pass the id the error gave, with the same arguments. Only offered when the user started the server with --allow-write.";

pub const LIST_DESCRIPTION: &str = "List the phone keyboard skins the signed-in account published to the keyboard skin community, removed ones included: id, name, description, category, moderation (removed means the moderators took it down; any other state is public), downloads, and the number and average of its ratings. These are touch keyboard skins for iOS, Android and HarmonyOS, not the candidate-window skins list_community_skins lists. Uses the account the user signed in to in the 水杉输入法 settings app; this server never signs in itself, so when no one is signed in, ask the user to sign in there.";

pub const DELETE_DESCRIPTION: &str = "Delete a phone keyboard skin the signed-in account published from the keyboard skin community for good, with its downloads and ratings; copies other people already downloaded stay on their phones. Takes the id list_community_keyboard_skins or publish_keyboard_skin gave; a candidate-window skin's community id goes to delete_community_skin instead. Confirm with the user first. A published keyboard skin cannot be edited, so deleting it and publishing again is the only way to change one. Only offered when the user started the server with --allow-write.";

// 键帽的形状。单独定义而不是直接用 `TouchSkinKeyShape`：schema 里要列出可选的值，写错的值要被拒绝。
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum KeyShape {
    Rounded,
    Capsule,
    Ticket,
    Pebble,
}

impl From<KeyShape> for TouchSkinKeyShape {
    fn from(value: KeyShape) -> Self {
        match value {
            KeyShape::Rounded => Self::Rounded,
            KeyShape::Capsule => Self::Capsule,
            KeyShape::Ticket => Self::Ticket,
            KeyShape::Pebble => Self::Pebble,
        }
    }
}

// 键帽的材质，单独定义的理由与 `KeyShape` 相同。
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum KeyMaterial {
    Flat,
    Raised,
    Glass,
    Paper,
}

impl From<KeyMaterial> for TouchSkinKeyMaterial {
    fn from(value: KeyMaterial) -> Self {
        match value {
            KeyMaterial::Flat => Self::Flat,
            KeyMaterial::Raised => Self::Raised,
            KeyMaterial::Glass => Self::Glass,
            KeyMaterial::Paper => Self::Paper,
        }
    }
}

// 代理写的键盘设计。键名与手机上的设计 JSON 相同，两处不同：颜色写成 `"#RRGGBB"`（手机上是整数，让模型换算十进制容易出错），没有 `photo` 键（照片只从 `photo_file` 来，不让模型把几十万字符的 base64 抄进调用）。
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KeyboardSkinDesign {
    /// The keyboard's background colour, "#RRGGBB". With a photo, use the photo's dominant colour: lists show the skin in this colour.
    pub background: String,
    /// The colour of the keys, "#RRGGBB".
    pub key_background: String,
    /// The colour of the letters on the keys, "#RRGGBB". Keep it clearly readable on keyBackground.
    pub key_foreground: String,
    /// The colour of hints, the pattern and the default key border, "#RRGGBB".
    pub accent: String,
    /// The colour of the return key, "#RRGGBB".
    pub action_background: String,
    /// The colour the background fades to, "#RRGGBB". Without it the background is one colour.
    #[serde(default)]
    pub gradient_end: Option<String>,
    /// The colour of the key border, "#RRGGBB". Without it the border uses accent.
    #[serde(default)]
    pub custom_border_color: Option<String>,
    /// 0 to 20. Default 8.
    #[serde(default)]
    pub corner_radius: Option<f64>,
    /// 0 to 2. Default 0.
    #[serde(default)]
    pub border_width: Option<f64>,
    /// 0 to 0.4. Default 0.
    #[serde(default)]
    pub shadow: Option<f64>,
    /// 0 plain, 1 dots, 2 grid, 3 waves. Default 0.
    #[serde(default)]
    pub pattern: Option<u8>,
    /// Whether the key letters use a monospaced font. Default false.
    #[serde(default)]
    pub monospaced: Option<bool>,
    #[serde(default)]
    pub key_shape: Option<KeyShape>,
    #[serde(default)]
    pub key_material: Option<KeyMaterial>,
    /// 0.25 to 1.
    #[serde(default)]
    pub key_opacity: Option<f64>,
    /// Whether the gradient runs left to right rather than top to bottom.
    #[serde(default)]
    pub gradient_horizontal: Option<bool>,
    /// 0 to 0.5.
    #[serde(default)]
    pub pattern_opacity: Option<f64>,
    /// 0 to 0.8: how dark the layer over the photo is.
    #[serde(default)]
    pub photo_shade: Option<f64>,
    /// 0 to 1: where the photo is cropped. 0.5 is the centre.
    #[serde(default)]
    pub photo_position: Option<f64>,
}

impl KeyboardSkinDesign {
    /// 换成共享的设计文档，照片留空。没写的键取 `TouchKeyboardSkinDesign::default()` 的值，范围由调用方交给共享的 `validate` 判定。
    fn to_shared(&self) -> Result<TouchKeyboardSkinDesign, String> {
        let defaults = TouchKeyboardSkinDesign::default();
        let optional = |field: &str, value: &Option<String>| {
            value
                .as_deref()
                .map(|value| color(field, value))
                .transpose()
        };
        Ok(TouchKeyboardSkinDesign {
            background: color("background", &self.background)?,
            key_background: color("keyBackground", &self.key_background)?,
            key_foreground: color("keyForeground", &self.key_foreground)?,
            accent: color("accent", &self.accent)?,
            action_background: color("actionBackground", &self.action_background)?,
            corner_radius: self.corner_radius.unwrap_or(defaults.corner_radius),
            border_width: self.border_width.unwrap_or(defaults.border_width),
            shadow: self.shadow.unwrap_or(defaults.shadow),
            pattern: self.pattern.unwrap_or(defaults.pattern),
            monospaced: self.monospaced.unwrap_or(defaults.monospaced),
            key_shape: self.key_shape.map(Into::into),
            key_material: self.key_material.map(Into::into),
            key_opacity: self.key_opacity,
            gradient_end: optional("gradientEnd", &self.gradient_end)?,
            gradient_horizontal: self.gradient_horizontal,
            pattern_opacity: self.pattern_opacity,
            custom_border_color: optional("customBorderColor", &self.custom_border_color)?,
            photo: None,
            photo_shade: self.photo_shade,
            photo_position: self.photo_position,
        })
    }
}

/// 把 `"#RRGGBB"` 读成设计文档里的整数颜色。
fn color(field: &str, value: &str) -> Result<u32, String> {
    let value = value.trim();
    value
        .strip_prefix('#')
        .filter(|_| msime_client_core::is_hex_color(value, &[6]))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| {
            format!("design.{field} = {value:?} is not a colour; write it as \"#RRGGBB\"")
        })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PublishKeyboardSkinRequest {
    /// The listing's title, 1 to 32 characters.
    pub name: String,
    /// The listing's text, at most 280 characters. It may have line breaks.
    #[serde(default)]
    pub description: Option<String>,
    /// nature, guofeng, acg, cute, food, tech, minimal or other. Without it the skin is listed under other.
    #[serde(default)]
    pub category: Option<Category>,
    /// The keyboard's look: colours as "#RRGGBB" and the optional shape, material and photo placement.
    pub design: KeyboardSkinDesign,
    /// The absolute path of the background photo on this computer: a JPEG no larger than 1024 pixels on its long side and 512,000 bytes. It is only read, never converted. Leave it out for a skin without a photo.
    #[serde(default)]
    pub photo_file: Option<String>,
    /// Only for repeating a publish that got no answer: the id the error gave. Leave it out otherwise, and a new id is made.
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PublishedKeyboardSkin {
    /// The skin's id in the keyboard skin community, which delete_community_keyboard_skin takes.
    pub id: String,
    /// The listing's title.
    pub name: String,
    /// The size of the photo that was sent, or null for a skin without a photo.
    pub photo_bytes: Option<u64>,
}

// 作品在审核方那里的状态。单独定义是为了让 schema 列出这几个值；服务端将来新增的状态读作 `unknown`。
#[derive(Clone, Copy, Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum Moderation {
    Approved,
    Pending,
    Removed,
    Unknown,
}

impl From<CommunityModeration> for Moderation {
    fn from(value: CommunityModeration) -> Self {
        match value {
            CommunityModeration::Approved => Self::Approved,
            CommunityModeration::Pending => Self::Pending,
            CommunityModeration::Removed => Self::Removed,
            CommunityModeration::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunityKeyboardSkinList {
    /// Newest first, removed ones included.
    pub skins: Vec<CommunityKeyboardSkinView>,
}

// 列表里的一款皮肤。不带设计和照片，也不带来源：`scope=mine` 的列表不返回来源，这里不去猜。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunityKeyboardSkinView {
    /// The skin's id in the keyboard skin community.
    pub id: String,
    pub name: String,
    pub description: String,
    /// The category the server reports, or null from a server that predates categories.
    pub category: Option<Category>,
    /// removed means the moderators took it down; approved and pending skins are public. Null when the server does not say.
    pub moderation: Option<Moderation>,
    pub downloads: u64,
    pub rating_count: u64,
    pub rating_average: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CommunityKeyboardSkinRequest {
    /// The id list_community_keyboard_skins or publish_keyboard_skin gave.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeletedCommunityKeyboardSkin {
    pub id: String,
}

/// 键盘皮肤社区服务和它用的账号会话：会话单独留一份，好在读文件和联网之前只读会话文件判断有没有人登录。
pub struct KeyboardSkinCommunity<A: AccountApi> {
    session: Arc<BackendAccountSession<A, FileAccountSessionStorage>>,
    service: BackendCommunitySkinService<A, FileAccountSessionStorage>,
}

/// 用 `api` 和 `directory` 里的会话文件组成键盘皮肤社区服务。
pub fn community_at<A>(
    api: A,
    directory: &Path,
    layout: AccountSessionFileLayout,
) -> KeyboardSkinCommunity<A>
where
    A: AccountApi + CommunitySkinApi + Clone,
{
    let storage = FileAccountSessionStorage::new(directory, layout);
    let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
    KeyboardSkinCommunity {
        service: BackendCommunitySkinService::new(api, Arc::clone(&session)),
        session,
    }
}

/// 设置应用登录后的键盘皮肤社区服务，请求经正式的账号服务器。
pub fn community(
    state_dir: &Path,
    edition: &Edition,
) -> Result<KeyboardSkinCommunity<BackendAccountClient>, String> {
    let (directory, layout) = session_location(state_dir, edition, |name| std::env::var_os(name))?;
    let client = BackendAccountClient::new()
        .map_err(|_| "cannot set up the connection to the community server")?;
    Ok(community_at(client, &directory, layout))
}

/// 按运行时选项里的状态目录和版本组成服务，三个工具每次调用都重新读一遍。
pub fn open(
    config: &crate::config::Config,
) -> Result<KeyboardSkinCommunity<BackendAccountClient>, String> {
    let document = config.read_options()?;
    let state_dir = config.state_dir(&document)?;
    community(&state_dir, config.edition(&document)?)
}

impl<A: AccountApi + CommunitySkinApi> KeyboardSkinCommunity<A> {
    /// 有人登录时什么都不返回；没登录时是让用户去设置应用登录的说明。只读会话文件，不联网。
    fn signed_in(&self) -> Result<(), String> {
        match self.session.status() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(SIGN_IN.into()),
            Err(error) => Err(account_problem(error)),
        }
    }
}

fn skin_id(id: &str, hint: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| format!("{id:?} is not a keyboard skin id; {hint}"))
}

/// 发布一款键盘皮肤。先查标题、简介、id、颜色和取值范围，再确认已登录，然后才读照片，最后只发一次请求。范围不对就拒绝，不替代理收进范围里。
pub fn publish<A: AccountApi + CommunitySkinApi>(
    community: &KeyboardSkinCommunity<A>,
    request: &PublishKeyboardSkinRequest,
) -> Result<PublishedKeyboardSkin, String> {
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("name must be 1 to {MAX_NAME_CHARS} characters"));
    }
    let description = request.description.as_deref().map_or("", str::trim);
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description must be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    if msime_client_core::has_disallowed_control_with_allowed(name, &[])
        || msime_client_core::has_disallowed_control_with_allowed(description, &['\n', '\t'])
    {
        return Err("name and description cannot contain control characters; a description may have line breaks (\\n) and tabs".into());
    }
    let id = match request.id.as_deref() {
        Some(id) => skin_id(
            id,
            "leave id out for a new skin, and pass one only to repeat a publish that got no answer",
        )?,
        None => Uuid::new_v4(),
    };
    let mut design = request.design.to_shared()?;
    // 手机发布和社区服务用的是同一个 `validate`；这时还没有照片，不过只可能是取值范围。
    if !design.validate() {
        return Err(OUT_OF_RANGE.into());
    }
    // 没登录时不读照片文件，也不发请求。
    community.signed_in()?;
    let photo_bytes = match request.photo_file.as_deref() {
        Some(file) => {
            let bytes = read_photo(file)?;
            design.photo = Some(TouchKeyboardSkinDesign::encode_photo(&bytes));
            Some(bytes.len() as u64)
        }
        None => None,
    };
    community
        .service
        .publish(
            id,
            name,
            description,
            &design,
            request.category.map(Into::into),
        )
        .map_err(|error| publish_problem(error, id))?;
    Ok(PublishedKeyboardSkin {
        id: id.hyphenated().to_string(),
        name: name.to_owned(),
        photo_bytes,
    })
}

/// 账号自己发布的全部键盘皮肤，被下架的也在内。
pub fn list<A: AccountApi + CommunitySkinApi>(
    community: &KeyboardSkinCommunity<A>,
) -> Result<CommunityKeyboardSkinList, String> {
    community.signed_in()?;
    let mut listed = Vec::new();
    let mut offset = 0;
    for _ in 0..MAX_PAGES {
        let page = community
            .service
            .list(offset, "", true, None, false)
            .map_err(account_problem)?;
        offset += page.skins.len();
        let finished = !page.has_more || page.skins.is_empty();
        listed.extend(
            page.skins
                .into_iter()
                .map(|skin| CommunityKeyboardSkinView {
                    id: skin.id.hyphenated().to_string(),
                    name: skin.name,
                    description: skin.description,
                    category: skin.category.map(Into::into),
                    moderation: skin.moderation.map(Into::into),
                    downloads: skin.downloads,
                    rating_count: skin.rating_count,
                    rating_average: skin.rating_average,
                }),
        );
        if finished || listed.len() >= MAX_LISTED {
            break;
        }
    }
    listed.truncate(MAX_LISTED);
    Ok(CommunityKeyboardSkinList { skins: listed })
}

/// 从社区删掉账号自己发布的一款键盘皮肤。别人已经下载到手机上的那份不动。
pub fn delete<A: AccountApi + CommunitySkinApi>(
    community: &KeyboardSkinCommunity<A>,
    request: &CommunityKeyboardSkinRequest,
) -> Result<DeletedCommunityKeyboardSkin, String> {
    let id = skin_id(&request.id, "list_community_keyboard_skins gives the ids")?;
    community.signed_in()?;
    community.service.unpublish(id).map_err(account_problem)?;
    Ok(DeletedCommunityKeyboardSkin {
        id: id.hyphenated().to_string(),
    })
}

/// 读代理给的照片文件，规则与 create_plugin 的 files 相同：绝对路径，路径上没有符号链接（系统别名除外），普通文件，读之前按大小拒绝，读时再按同一上限截断。读出来的必须以 JPEG 的 SOI 标记开头；这里不解码，尺寸和扫描数由社区服务器判定。
fn read_photo(file: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(file);
    let problem = |what: &str| format!("photo_file = {file:?} {what}; {PHOTO_FILE_RULES}");
    if !path.is_absolute() {
        return Err(problem("is not an absolute path"));
    }
    match std::fs::symlink_metadata(path) {
        Err(_) => return Err(problem("does not exist or cannot be reached")),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(problem("is a symbolic link; pass the file it points to"))
        }
        Ok(metadata) if !metadata.is_file() => return Err(problem("is not a regular file")),
        Ok(metadata) if metadata.len() > MAX_PHOTO_BYTES => return Err(problem(PHOTO_TOO_LARGE)),
        Ok(_) => {}
    }
    msime_path_trust::reject_symlinked_components(path)
        .map_err(|_| problem("has a symbolic link on the way"))?;
    let handle = msime_client_core::file_lock::open_private_file_in(path)
        .map_err(|_| problem("cannot be opened"))?;
    let metadata = handle.metadata().map_err(|_| problem("cannot be read"))?;
    if !metadata.is_file() {
        return Err(problem("is not a regular file"));
    }
    if metadata.len() > MAX_PHOTO_BYTES {
        return Err(problem(PHOTO_TOO_LARGE));
    }
    let bytes = crate::bounded::read(handle, MAX_PHOTO_BYTES).map_err(|error| match error {
        crate::bounded::ReadError::TooLarge => problem(PHOTO_TOO_LARGE),
        crate::bounded::ReadError::Io => problem("cannot be read"),
    })?;
    if !bytes.starts_with(&JPEG_START) {
        return Err(NOT_A_JPEG.into());
    }
    Ok(bytes)
}

/// 没拿到明确结果的发布说明都以它开头，[`publish_outcome`] 靠它把这种情况和被拒分开。别的错误没有以它开头的：照片的说明以 `photo_file` 开头，id 的说明以引号开头。
const UNCONFIRMED: &str = "the outcome of this publish is unknown: ";

/// stderr 审计行里发布结果的那个词。没拿到明确结果时皮肤可能已经公开，记成 `unconfirmed` 而不是 `refused`，本机唯一的记录才不会说成什么都没发出去。
pub fn publish_outcome<T>(result: &Result<T, String>) -> &'static str {
    match result {
        Ok(_) => "published",
        Err(error) if error.starts_with(UNCONFIRMED) => "unconfirmed",
        Err(_) => "refused",
    }
}

/// 发布被拒的原因。409 有两种来源而客户端分不出来，两种都说；没拿到明确回答时皮肤可能已经发出去了，把这次的 id 交给代理，原样再发一次不会多出一款。会话层在请求成功之后还会再读一次会话（`request_with_account_session`）：用户这期间退出或换了账号是 `Cancelled`，会话文件那一刻读不出来是 `Storage`，这两种也可能已经发出去了，同样给 id。
fn publish_problem(error: AccountError, id: Uuid) -> String {
    let id = id.hyphenated();
    match error {
        AccountError::Conflict => "the community refused it: either this account already holds the maximum of 50 keyboard skins (delete one with delete_community_keyboard_skin first), or the id belongs to a different skin (leave id out to publish a new one)".into(),
        AccountError::Invalid => "the community server did not accept the skin: the photo may be larger than 1024 pixels on a side or not a baseline JPEG, or the name, description or design may not be accepted".into(),
        AccountError::BlockedContent => "the community does not allow some words in the name or description; change them and try again".into(),
        AccountError::Unavailable | AccountError::NotFound => format!("{UNCONFIRMED}the community server cannot be reached right now or gave an unexpected answer, so the skin may or may not have been published. Call publish_keyboard_skin again with the same arguments and id \"{id}\", which does not make a second skin, or check list_community_keyboard_skins"),
        AccountError::Cancelled => format!("{UNCONFIRMED}the user signed out or switched accounts in the settings app while this ran, so the skin may or may not have been published, and if it was, it is public under the account that was signed in when the call started. Tell the user, and do not publish it again without an id. With that account signed in, check list_community_keyboard_skins; if the skin is not there, call publish_keyboard_skin again with the same arguments and id \"{id}\", which does not make a second skin, and if it is there and should not be, delete_community_keyboard_skin removes it"),
        AccountError::Storage => format!("{UNCONFIRMED}the account session on this computer could not be read or written while this ran, so the skin may or may not have been published. Do not publish it again without an id: check list_community_keyboard_skins, and if the skin is not there call publish_keyboard_skin again with the same arguments and id \"{id}\", which does not make a second skin"),
        other => account_problem(other),
    }
}

/// 账号或社区服务器的错误，说成用户听得懂、代理知道下一步的话。
fn account_problem(error: AccountError) -> String {
    match error {
        AccountError::Unauthorized => SIGN_IN.into(),
        AccountError::Cancelled => "the user signed out or switched accounts in the settings app while this ran; try again".into(),
        AccountError::Forbidden => "the community server does not allow this account to do that".into(),
        AccountError::NotFound => "this account has no such keyboard skin in the community; list_community_keyboard_skins lists the ones it published".into(),
        AccountError::Conflict => "the community refused the request because of a conflict; check list_community_keyboard_skins and try again".into(),
        AccountError::RateLimited => "too many requests reached the community server in the last minute; wait a minute and try again".into(),
        AccountError::Invalid => "the community server did not accept the request".into(),
        AccountError::BlockedContent => "the community does not allow some words in the request; change them and try again".into(),
        AccountError::ScreeningUnavailable => "the community's content check is unavailable right now; try again later".into(),
        AccountError::Banned => "this account is banned from the community".into(),
        AccountError::Storage => "the account session on this computer could not be read or written".into(),
        AccountError::Unavailable => "the community server cannot be reached right now or gave an unexpected answer; try again later".into(),
    }
}

#[cfg(test)]
mod tests;

//! The tools, and the limits around the ones that write.
//!
//! 每个工具都重新读运行时选项，所以设置页搬走数据目录或改了词库之后，服务器不用重启就能跟上。输入的内容和凭据都不经这里离开输入法：社区工具只发送用户要求发布的那款皮肤或插件包，用的是设置应用保存的账号会话，会话里的令牌从不出现在结果里；除快捷短语外的词库内容，只有用户以 --allow-dictionary-read 启动服务器时才读。诊断日志总是可读：里面只有事件名、耗时和错误码，由用户自己打开，Windows TIP 记下的按键在返回前清空；stderr 上的审计行只写工具名和结果，从不写写入了什么。

use crate::community::{
    self, CommunitySkinList, CommunitySkinRequest, DeletedCommunitySkin, PublishRequest,
    PublishedSkin, VisibilityChanged, VisibilityRequest,
};
use crate::config::Config;
use crate::diagnostics::{self, LogRequest, LogView, SwitchRequest, SwitchView};
use crate::plugin_community::{
    self, CommunityPluginList, CommunityPluginRequest, DeletedCommunityPlugin,
    PublishPluginRequest, PublishedPlugin,
};
use crate::plugins::{
    self, BuiltinPaths, CreatePluginRequest, DeletedPlugin, InstallPluginRequest, InstalledPlugin,
    PluginFiles, PluginList, PluginRequest,
};
use crate::preferences::{self, PreferencesChange, PreferencesView};
use crate::prompts::WRITE_PROMPTS;
use crate::skins::{
    self, CreateSkinRequest, CreatedSkin, DeletedSkin, SkinFiles, SkinList, SkinRequest,
};
use crate::statistics::{self, StatisticsRequest, StatisticsView};
use crate::words;
use crate::words::{
    LookupRequest, LookupView, WordEditRequest, WordImportOutcome, WordImportRequest, WordListPage,
    WordListRequest,
};
use msime_client_core::file_lock;
use msime_client_services::dictionary::quiesce::QuiescedHosts;
use msime_host_api::{DictionaryOptions, QuickPhrase, QuickPhraseEdit, WordEdit};
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::schemars::JsonSchema;
use rmcp::{prompt_handler, tool, tool_handler, tool_router, Json, ServerHandler};
use serde::{Deserialize, Serialize};
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_PAGE: usize = 100;
const MAX_PAGE: usize = 1000;
/// The most edits one call may carry. An agent tidying a handful of phrases stays well inside it; one rewriting the whole collection has to show its work across calls.
const MAX_EDITS: usize = 50;
/// The shortest gap between two writing calls, so an agent stuck in a loop cannot rewrite the dictionary or the preferences many times a second.
const WRITE_INTERVAL: Duration = Duration::from_secs(1);
/// In the state directory: when the last write began, shared by every server and command line on this computer.
const WRITE_LOCK: &str = "mcp-write.lock";
/// 写锁只保存一个毫秒时间戳，拒绝异常膨胀的内容以免无界分配内存。
const WRITE_LOCK_READ_LIMIT: u64 = 128;

const WRITE_TOOLS: [&str; 12] = [
    "create_candidate_skin",
    "create_plugin",
    "delete_candidate_skin",
    "delete_community_plugin",
    "delete_community_skin",
    "delete_plugin",
    "edit_quick_phrases",
    "install_plugin_folder",
    "publish_candidate_skin",
    "publish_plugin",
    "set_community_skin_visibility",
    "update_preferences",
];
/// Offered with --allow-dictionary-read.
const DICTIONARY_READ_TOOLS: [&str; 2] = ["list_dictionary_words", "lookup_candidates"];
/// Offered with --allow-write and --allow-dictionary-read together: an edit or import also tells whether a word is there.
const DICTIONARY_WRITE_TOOLS: [&str; 2] = ["edit_dictionary_words", "import_dictionary_words"];

const INSTRUCTIONS: &str = "Manages 水杉输入法 (MSIME), a Chinese input method: its quick phrases (a short code the user types that expands to a longer text), a few of its preferences, and aggregate typing statistics. With --allow-dictionary-read it also lists the user's own dictionary words and shows which candidates a code offers and why, which is how to explain a candidate's rank; with --allow-write as well it adds, reweights, removes and imports words. To import a word list the user gives you, read it yourself and send the words, 200 at a time. It also helps with problems the user runs into: lag, a candidate window that is missing or in the wrong place, the input method stopping or switching by itself. The user may not be technical, so do the steps yourself rather than asking them to open files, settings or a terminal: read the log with read_diagnostic_log; if it is off, turn it on with set_diagnostic_log, ask the user in plain words to do again what went wrong and to tell you when they have, then read the log again and explain what you found in plain words. Turn the log off with set_diagnostic_log when you are done. It also lists the user's candidate-window skins and, with --allow-write, makes new ones: write a skin.toml and bounded PNG/JPEG/APNG/GIF/WebP resources with a PNG/JPEG preview, save the images as files, and pass the manifest and the files' paths to create_candidate_skin. update_preferences switches the input method to a skin, get_candidate_skin shows an installed skin's manifest and image files so it can be changed and installed again, and delete_candidate_skin removes one the user no longer wants. The app can sync skins to the user's cloud library as private packages when the user requests sync. With --allow-write, publish_candidate_skin uploads an installed skin to the skin community, public or private, using the account the user signed in to in the 水杉输入法 settings app; this server never signs in to the account itself, so if no one is signed in, ask the user to sign in there. Before publishing publicly, confirm with the user that everyone may see the skin and that they have the right to share its images. list_community_skins lists the account's community skins, set_community_skin_visibility makes one private again and delete_community_skin removes one. It also lists the user's plugin packs (list_plugins, get_plugin) and, with --allow-write, makes them. The kinds are sound and music packs, command tables (/ commands, such as one that types today's date), typing effects, phrase tables (K-mode phrases, such as an email signature), helpcode tables, word books, symbol sets (such as a group of arrows) and, on desktop, experimental script plugins. For an ordinary request like these, write the plugin.toml yourself by the rules in create_plugin's description, install it with create_plugin, and tell the user in plain words how to turn it on, as its result says: phrase tables and command tables need 启用 in 设置 → 插件 → 我的插件, and K mode or / mode on in 设置 → 输入 → 快捷模式. install_plugin_folder imports a pack folder or .zip, such as a script plugin built with msime-pack, which runs only after the user turns it on and grants it in 设置 → 插件 → 脚本插件（实验）. delete_plugin removes a pack, and publish_plugin shares a data pack with the plugin community, always publicly, once the user agrees. Extended resources require explicit server support. Changes take effect in the input method within a few seconds. Apart from set_diagnostic_log, writing tools are only offered when the user started the server with --allow-write.";

#[derive(Clone)]
pub struct MsimeServer {
    config: Arc<Config>,
    last_write: Arc<Mutex<Option<Instant>>>,
    /// Set while a write runs. rmcp runs each request as its own task and a write can outlast the interval, so spacing alone would let two overlap on the quiesce lease and on a check-then-write edit.
    writing: Arc<AtomicBool>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ListRequest {
    /// Only phrases whose code starts with this. Codes are lowercase.
    pub code_prefix: Option<String>,
    /// How many matching phrases to skip.
    pub offset: Option<usize>,
    /// At most this many phrases, 1 to 1000. Defaults to 100.
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct Phrase {
    /// What the user types, lowercase letters.
    pub code: String,
    /// What it expands to.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct PhrasePage {
    pub phrases: Vec<Phrase>,
    /// More phrases match; ask again with a larger offset.
    pub has_more: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Add a phrase. Refused if the same code already expands to the same text.
    Add { code: String, text: String },
    /// Change a phrase, found by its current code and text, keeping its rank.
    Replace {
        previous: Phrase,
        replacement: Phrase,
    },
    /// Remove a phrase, found by its code and text.
    Remove { code: String, text: String },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    /// Applied in order, at most 50. The first that fails stops the rest; those before it stay applied.
    pub edits: Vec<Edit>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct EditOutcome {
    /// How many edits were applied, from the start of the list.
    pub applied: usize,
    /// The index of the edit that failed, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<QuickPhrase> for Phrase {
    fn from(value: QuickPhrase) -> Self {
        Self {
            code: value.code,
            text: value.text,
        }
    }
}

impl From<Phrase> for QuickPhrase {
    fn from(value: Phrase) -> Self {
        Self {
            code: value.code,
            text: value.text,
        }
    }
}

impl From<Edit> for QuickPhraseEdit {
    fn from(value: Edit) -> Self {
        match value {
            Edit::Add { code, text } => Self::Add(QuickPhrase { code, text }),
            Edit::Replace {
                previous,
                replacement,
            } => Self::Replace {
                previous: previous.into(),
                replacement: replacement.into(),
            },
            Edit::Remove { code, text } => Self::Remove(QuickPhrase { code, text }),
        }
    }
}

#[tool_router]
impl MsimeServer {
    pub fn new(config: Config) -> Self {
        let mut tool_router = Self::tool_router();
        let hidden = [
            (!config.allow_write, &WRITE_TOOLS[..]),
            (!config.allow_dictionary_read, &DICTIONARY_READ_TOOLS[..]),
            (
                !(config.allow_write && config.allow_dictionary_read),
                &DICTIONARY_WRITE_TOOLS[..],
            ),
        ];
        for name in hidden
            .into_iter()
            .filter(|(hide, _)| *hide)
            .flat_map(|(_, names)| names)
        {
            tool_router.remove_route(name);
        }
        let mut prompt_router = Self::prompt_router();
        if !config.allow_write {
            for name in WRITE_PROMPTS {
                prompt_router.remove_route(name);
            }
        }
        Self {
            config: Arc::new(config),
            last_write: Arc::new(Mutex::new(None)),
            writing: Arc::new(AtomicBool::new(false)),
            tool_router,
            prompt_router,
        }
    }

    #[tool(
        name = "list_quick_phrases",
        description = "List the user's own quick phrases: short codes that expand to longer text when typed. Built-in phrases are not included. A phrase's text may hold the marker ${cursor}: the first one is removed and the caret is left there after the phrase is typed, for example title:\"${cursor}\" types title:\"\" with the caret between the quotes; where the editor cannot place the caret it stays at the end. Write $${cursor} to type the literal text ${cursor}; a phrase saved before this marker existed that holds a literal ${cursor} now moves the caret instead.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_quick_phrases(
        &self,
        Parameters(request): Parameters<ListRequest>,
    ) -> Result<Json<PhrasePage>, String> {
        let limit = request.limit.unwrap_or(DEFAULT_PAGE);
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_PAGE}"));
        }
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let page = msime_host_api::user_quick_phrases(
                &options,
                request.code_prefix.as_deref().unwrap_or(""),
                request.offset.unwrap_or(0),
                limit,
            )?;
            Ok(Json(PhrasePage {
                phrases: page.phrases.into_iter().map(Phrase::from).collect(),
                has_more: page.has_more,
            }))
        })
        .await
    }

    #[tool(
        name = "edit_quick_phrases",
        description = "Add, replace or remove the user's quick phrases. Each phrase is found by its exact code and text, so list them first. Edits are applied in order and the first failure stops the rest. A phrase's text may hold the marker ${cursor}: the first one is removed and the caret is left there after the phrase is typed, for example title:\"${cursor}\" types title:\"\" with the caret between the quotes; where the editor cannot place the caret it stays at the end. Write $${cursor} to type the literal text ${cursor}; to keep a literal ${cursor} in an existing phrase, replace it with $${cursor}.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_quick_phrases(
        &self,
        Parameters(request): Parameters<EditRequest>,
    ) -> Result<Json<EditOutcome>, String> {
        if request.edits.is_empty() || request.edits.len() > MAX_EDITS {
            return Err(format!("send between 1 and {MAX_EDITS} edits"));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let edits: Vec<QuickPhraseEdit> = request.edits.into_iter().map(Into::into).collect();
        let outcome = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            Ok(apply_edits(
                &options,
                &edits,
                msime_host_api::edit_user_quick_phrase,
            ))
        })
        .await?;
        eprintln!(
            "msime-mcp: edit_quick_phrases applied {}{}",
            outcome.applied,
            if outcome.error.is_some() {
                " then failed"
            } else {
                ""
            }
        );
        Ok(Json(outcome))
    }

    #[tool(
        name = "get_preferences",
        description = "Read the preferences an agent may see: input scheme, shuangpin layout, candidate page size, font size and layout, the starting mode, character width, punctuation, Traditional Chinese output, Wubi switches, whether the AI assistant is on, the global theme and the candidate-window skin chosen for light and for dark mode, and a few more. Pass the returned revision to update_preferences.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_preferences(&self) -> Result<Json<PreferencesView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            preferences::load(&state_dir, config.edition(&document)?).map(Json)
        })
        .await
    }

    #[tool(
        name = "update_preferences",
        description = "Change some of the preferences get_preferences returns. Only the fields given change. Refused when the preferences changed since expected_revision was read, for a scheme the installed edition of the input method does not offer, and for turning the AI assistant on. Turn the AI assistant off when the user finds the AI candidate slow or in the way: it holds one candidate slot showing … while it waits for the model. To use an installed skin (an id list_candidate_skins lists), set candidate_skin for light mode and candidate_skin_dark for dark mode; candidate_skin also serves dark mode while candidate_skin_dark is null, and null takes a skin out of its slot. A skin is drawn only in the mode of its manifest base: on night, ink or shuishan only in dark mode, on light or paper only in light mode, on system in both, so a slot refuses a skin that cannot show in its mode, and it refuses an id that is not an installed, usable skin. Choosing a skin also switches global_theme to custom, the theme that draws skins, unless global_theme is given; a built-in theme (system, shuishan, light, paper, night, ink) is chosen with global_theme alone and keeps the skin choice for later.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_preferences(
        &self,
        Parameters(change): Parameters<PreferencesChange>,
    ) -> Result<Json<PreferencesView>, String> {
        // Refused before the slot is taken, so a mistaken call does not hold up the corrected one.
        if change.is_empty() {
            return Err("no preference to change".into());
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let edition = config.edition(&document)?;
            preferences::update(&state_dir, &config.options, edition, &change).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: update_preferences {}",
            if result.is_ok() { "saved" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "list_candidate_skins",
        description = "List the candidate-window skins installed as folders in the input method's skin directory: id, schema version, name, version, whether it is a canvas (base = canvas, full-image) skin, notices about keys that were ignored or are not drawn, layouts, colour modes, bounded animation presets (with which of enter, exit and selection the manifest's [animation] table actually writes; the others are filled-in defaults that macOS does not play), for a skin with a decoration image its idle motion, motion period and frame count (more than 1 for an animated PNG) and whether the desktop app has synced it to the user's cloud library. Folders that are not a usable skin are listed with the reason.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_candidate_skins(&self) -> Result<Json<SkinList>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            Ok(Json(skins::list(&state_dir)))
        })
        .await
    }

    #[tool(
        name = "get_candidate_skin",
        description = "Read one installed skin so it can be changed: its whole skin.toml, the absolute path of its folder, whether the desktop app has synced it to the user's cloud library, and for each image the manifest references its path in the manifest, the absolute path of the file on this computer, its size in bytes, its format as read from its content (png, apng for a PNG of more than one frame, jpeg, gif or webp, or null when unrecognised), the width and height of one frame in pixels and its frame count. Files the manifest does not reference are not listed. To change the skin, edit the manifest, write any new or changed images to files, and call create_candidate_skin with replace: true, passing each unchanged image by the file given here in image_files: it reads every image before it replaces the folder. Refused for a built-in theme or look, an id that is not installed, and a folder list_candidate_skins reports as an issue.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_candidate_skin(
        &self,
        Parameters(request): Parameters<SkinRequest>,
    ) -> Result<Json<SkinFiles>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            skins::get(&state_dir, &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "create_candidate_skin",
        description = "Install a candidate-window skin from a skin.toml and its images. Schema version 1 remains supported; version 2 additionally accepts [animation] with enter/exit/selection = none, fade, fade_scale, slide_up, spring or glow, duration_ms 40-600 and stagger_ms 0-80, and the decoration motion and animated PNG below, with any base. Write each preset you want explicitly: macOS plays only the presets the [animation] table writes, while Windows and the web engine fill omitted ones with a default. The manifest is TOML: id (equal to package_id); name (at most 80 bytes); version; base, the look drawn under it: a built-in theme (system, shuishan, light, paper, night or ink) or an msime-windows built-in look (fluent, wechat, wechat_scroll, graphite, willow_green, autumn_osmanthus or microsoft), whose colours fill the ones the manifest leaves out, or canvas, msime-windows' full-image base, which requires schema_version = 2 (version 1 with canvas is refused) and is drawn on the system base with the canvas look described below; preview, the path of a PNG or JPEG shown in the skin list; optional author, description and [license] code, assets and source (empty author or description reads as not written; syncing these or new bases requires explicit server support); [supports] layouts (horizontal, vertical) and themes (dark, light); [candidate_window] min_width_dip (0-1000), optional corner_radius_dip (0-32), optional [candidate_window.decoration] top_inset_dip, width_dip, image and align (left, center, right), and in schema version 2 motion (none, float, sway, breathe or bounce: a small idle movement of the whole decoration image while the candidate window is on screen) and motion_period_ms (1200-8000, default 3000); the decoration image may be an animated PNG (APNG, at most 120 frames, each frame counted as a whole image toward the pixel limit), which a host with a decoration animation renderer plays with its frame delays and loop count, while other entries show its first frame; reduced-motion settings still win. Frame playback and idle decoration transforms are separate host capabilities. Optional [candidate_window.background] image, fit (cover, contain, stretch) and opacity (0-1). [candidate_window] also accepts msime-windows' font_family (at most 64 bytes, without quotes, backslash, comma, semicolon, braces, angle brackets or backtick), which Windows draws; item_corner_radius_dip (0-16), which macOS also draws; and these msime-windows layout keys, which all hosts validate and render where their native entry supports each primitive: border_width_dip (0-4), shadow (none, soft or strong), backdrop (none, acrylic or mica; Windows draws the system material behind the card, mica as acrylic before Windows 11, and an opaque surface where the material is unavailable, transparency effects are off or high contrast is on), page_arrows, fit_content and expand_panel (true or false; expand_panel puts a button at the end of the horizontal list that opens it into a scrolling grid), expand_rows (2-8), the geometry keys padding, item_gap, item_extra_height, number_inset, item_padding_right and number_gap (0-64 as a number of dip, or 0-4em as a string such as '0.5em'), item_min_width (0-500, or 0-40em; a desktop candidate window's cell width, which the iOS, Android and HarmonyOS touch keyboard strips ignore, sizing each candidate to its content), selected_bar_width (0-16, or 0-1em), number_scale and selected_bar_height_em (0-2), selected_bar_inside, item_corners_follow_card, justify_rows and trim_hidden_preedit_gap (true or false), page_arrow_style (chevron or triangle) and preedit_margin_dip (1, 2 or 4 numbers 0-500), and the same geometry keys in [candidate_window.horizontal] or [candidate_window.vertical] for one layout only. Only with base = canvas: [candidate_window.border_image], a nine-slice panel image, with source and optional light_source (PNG/JPEG/APNG/GIF/WebP), slice_dip and optional stretch_dip and width_dip (each 1, 2 or 4 numbers 0-500) and fill (true or false, default true); content_insets_dip (top and bottom 0-200, left and right 0-500), outer_insets_dip (top, right, bottom and left 0-500) and line_gap_dip (0-100); and up to 32 [[candidate_window.decorations]] free widgets, each with image (PNG/JPEG/APNG/GIF/WebP), anchor (top-left, top-right, bottom-left, bottom-right, left, right, top, bottom or center, default right), width_dip and height_dip (above 0, at most 2000), offset_x_dip and offset_y_dip (-2000 to 2000) and z_index (-1000 to 1000, default 2). With any other base these canvas-only keys are ignored. The Web engine and the macOS, Windows, iOS, Android, HarmonyOS 2in1 and Linux X11 candidate windows draw these canvas textures using their platform image codecs; a host without that renderer keeps its existing static or readable opaque canvas look. Linux native rendering requires a real X11 input context, focus and classicui ownership; Wayland, IBus and third-party UIs keep their existing fallback; list_candidate_skins reports such ignored or undrawn keys as notices. 装饰图候选词叠加使用 [candidate_window.decoration] 的 candidate_overlay = true（v1、v2 均可用）：candidate_overlay_index 为整数 0-100，默认 1，0 跟随高亮，越出当前页时不显示；candidate_overlay_font_size_dip 与 candidate_overlay_min_font_size_dip 为 (0,200]，默认 16 与 8；candidate_overlay_max_width_dip 为 0-2000，0 使用装饰框宽；candidate_overlay_color 缺省白色；candidate_overlay_align_h 为 left/center/right、candidate_overlay_align_v 为 top/center/bottom，均默认 center；candidate_overlay_offset_x_dip 与 candidate_overlay_offset_y_dip 为 -2000 至 2000；candidate_overlay_strip_badge 默认 true，只画原文，false 追加独立标注。词太宽时缩小字号，最小字号仍过宽则不换行、允许超框。关闭叠加时忽略其余键；启用需要装饰图片与正尺寸。列表返回声明不代表每个原生宿主已经绘制，须核对目标宿主。 Optional [toolbar] corner_radius_dip (0-32) and icon_corner_radius_dip (0-16, not drawn yet), with [toolbar.dark] and [toolbar.light] colours background, border, handle, divider, icon and hover for the floating toolbar. [toolbar] mode = bar (default) or image is validated on every platform. Image mode needs image (package-relative PNG/APNG/GIF/WebP, never JPEG), image_width_dip and image_height_dip (above 0, at most 2000). Optional [toolbar.button_defaults] width_dip, height_dip and icon_size_dip (above 0, at most 500) and background; [toolbar.button_flow] x_dip/y_dip (0-2000), direction (horizontal or vertical) and gap_dip (0-500). [toolbar.buttons.<key>] accepts visible, x_dip/y_dip, the default look fields and image; keys are status, fullwidth, punctuation, character_set, emoji, screen_keyboard and settings. Status images states are cn, en, ja, caps and en-candidate; fullwidth states full/half; punctuation cn/en; character_set simplified/traditional. The other buttons cannot declare images. A matching state image wins over image, then the host icon. Fixed coordinates do not consume flow; buttons without a complete coordinate pair need flow. User-hidden buttons remain hidden; windows include buttons outside the background. The shared preview renders this layout; native host drawing is a separate capability and must be checked before promising an image toolbar. Bar mode validates but ignores image fields. Community upload stays at 37 images and requires X-MSIME-Candidate-Toolbar: image-v1 for image mode; older deployments keep it local. [candidate.dark] and [candidate.light] colours accent, selected, hover, surface, border, text, number and translation, plus show_selected_bar, which every host draws; the detail colours candidate_text, preedit_text, preedit_caret, preedit_background, preedit_divider, translation_pos, translation_separator, selected_text, selected_number, selected_translation, selected_translation_pos, selected_translation_separator, selected_bar and page_arrow_divider, backdrop_opacity and backdrop_luminosity (numbers 0-1), and a [candidate.dark.menu] or [candidate.light.menu] table with background, border, text and hover for the candidate window's menu, which a host draws where it has that slot and otherwise checks and replaces with the base colours. A colour is text of at most 80 bytes: #RGB, #RGBA, #RRGGBB, #RRGGBBAA, rgb(r, g, b) or rgba(r, g, b, a) with channels 0-255 and alpha 0-1, or transparent; #RRGGBB and #RRGGBBAA are read everywhere, and a colour a host cannot read is not drawn. images (standard base64) and image_files (the absolute path of a file on this computer) together must hold exactly the images the manifest references (the preview, decoration and background images and, with canvas, the border_image sources, each decorations image and every toolbar image), each keyed by the path the manifest uses and given in only one of the two, at most 64 in all, each a .png, .jpg, .jpeg, .apng, .gif or .webp; the preview stays PNG/JPEG. Prefer image_files: write each image to a file with code and pass its path, since base64 copied into the call by hand is easily corrupted. The server only reads those files: it refuses a path with a symbolic link anywhere on the way or that is not a regular file, refuses a file over 1 MiB before reading it, and checks the bytes exactly as it checks base64. Each image is at most 1 MiB and 2048 pixels on each side, all images at most 2 MiB and 8,000,000 pixels, animations at most 120 frames counted as full canvases, and the preview at most 256 KiB. Extended resources and new bases require explicit server support before upload; older deployments leave these skins available locally. A stylesheet is not accepted, so the skin can be synced and shared. Refused when the skin exists, unless replace is true.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn create_candidate_skin(
        &self,
        Parameters(request): Parameters<CreateSkinRequest>,
    ) -> Result<Json<CreatedSkin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let state_dir = config.state_dir(&config.read_options()?)?;
            skins::create(&state_dir, config.linux_host_options(), &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: create_candidate_skin {}",
            match &result {
                Ok(created) if created.0.replaced => "replaced",
                Ok(_) => "installed",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "delete_candidate_skin",
        description = "Delete one skin the user installed: the folder named package_id directly under the input method's skin directory, with everything in it. Refused for a built-in theme, an msime-windows built-in look, canvas or default, for a name that is not installed, for anything there that is not a real folder (a file or a symbolic link), and for a skin chosen as candidate_skin or candidate_skin_dark in the preferences, even while another theme is shown: choose another skin or set the slot to null with update_preferences first. Says whether the desktop app had synced the skin to the user's cloud library; if it had, the app's next sync removes the private copy there too, leaves a publicly published copy published, and downloads it again if it was changed on another device since. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn delete_candidate_skin(
        &self,
        Parameters(request): Parameters<SkinRequest>,
    ) -> Result<Json<DeletedSkin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            skins::delete(
                &state_dir,
                config.linux_host_options(),
                config.edition(&document)?,
                &request,
            )
            .map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: delete_candidate_skin {}",
            if result.is_ok() { "deleted" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "publish_candidate_skin",
        description = "Upload an installed candidate-window skin (an id list_candidate_skins lists) to the 水杉输入法 skin community, using the account the user signed in to in the 水杉输入法 settings app. This server never signs in or handles passwords: when no one is signed in it says so, and you ask the user to sign in there and try again. visibility is required and has no default: public lists the skin in the community, where everyone can see, download and rate it; private keeps it in the user's own cloud library, visible only to them. Before publishing publicly, ask the user in plain words to confirm that everyone may see it and that they made the images or have the right to share them; a skin someone else published, downloaded from the community, is not theirs to publish. A public skin must say who may reuse its images in skin.toml's [license] assets: when the manifest has none, ask the user and pass license_assets, an SPDX id such as CC-BY-4.0 (reuse with credit) or CC0-1.0 (no conditions) or their own words, which is written into the manifest before the upload. name (1-32 characters) and description (at most 280) are the listing's title and text, by default the manifest's name and description; category is nature, guofeng, acg, cute, food, tech, minimal or other. The skin is checked on this computer before anything is sent, by the rules the settings app's publish dialog uses: a PNG or JPEG preview of at most 256 KiB, only the images the manifest references, each at most 1 MiB, at most 37 together of at most 2 MiB and 8,000,000 pixels, and no toolbar_stylesheet; more than 3 images, APNG, GIF or WebP files and the canvas and wechat_scroll bases also need a community server that accepts them. When this account's library already has the skin (synced by the settings app or published before, matched by package_id), that entry is replaced and given this visibility and category instead of a second one being made; the result says which and gives the entry's community id. An account keeps at most 100 skins, at most 20 of them public, and may publish publicly at most 10 times an hour. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn publish_candidate_skin(
        &self,
        Parameters(request): Parameters<PublishRequest>,
    ) -> Result<Json<PublishedSkin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let service = community::service(&state_dir, config.edition(&document)?)?;
            community::publish(&state_dir, &service, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: publish_candidate_skin {}",
            match &result {
                Ok(published) if published.0.replaced => "replaced",
                Ok(_) => "published",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "list_community_skins",
        description = "List the candidate-window skins in the signed-in account's community library: the ones the settings app synced and the ones published, private ones included. Gives each one's community id, package_id (the installed skin id it was made from), visibility (public or private) and when it last changed. Uses the account the user signed in to in the 水杉输入法 settings app; this server never signs in itself, so when no one is signed in, ask the user to sign in there.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn list_community_skins(&self) -> Result<Json<CommunitySkinList>, String> {
        let config = self.config.clone();
        blocking(move || {
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let service = community::service(&state_dir, config.edition(&document)?)?;
            community::list(&service).map(Json)
        })
        .await
    }

    #[tool(
        name = "set_community_skin_visibility",
        description = "Make a skin in the signed-in account's community library private, so only the user sees it while its downloads and ratings are kept, or public again. Takes the community id list_community_skins or publish_candidate_skin gave. Making a skin public shows it to everyone: confirm with the user first, as for publish_candidate_skin. It needs [license] assets in the skin.toml that was uploaded, so for a skin without one use publish_candidate_skin with visibility public and license_assets instead. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn set_community_skin_visibility(
        &self,
        Parameters(request): Parameters<VisibilityRequest>,
    ) -> Result<Json<VisibilityChanged>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let service = community::service(&state_dir, config.edition(&document)?)?;
            community::set_visibility(&service, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: set_community_skin_visibility {}",
            match &result {
                Ok(changed) if changed.0.visibility == community::Visibility::Public => "public",
                Ok(_) => "private",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "delete_community_skin",
        description = "Delete a skin from the signed-in account's community library for good, with its downloads and ratings; copies other people already installed stay on their computers. Takes the community id list_community_skins or publish_candidate_skin gave. Confirm with the user first; to only hide a public skin, use set_community_skin_visibility with private instead. The installed skin on this computer is left alone, and the next time the settings app syncs it uploads it again as a new private skin; delete it here with delete_candidate_skin as well if the user wants it gone everywhere. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn delete_community_skin(
        &self,
        Parameters(request): Parameters<CommunitySkinRequest>,
    ) -> Result<Json<DeletedCommunitySkin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let service = community::service(&state_dir, config.edition(&document)?)?;
            community::delete(&state_dir, &service, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: delete_community_skin {}",
            if result.is_ok() { "deleted" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "list_plugins",
        description = "List the plugin packs of the input method, the user's own and those that ship with it: kind, id, name, version, licence, author, description, whether it is built in or one of the official packs, a short summary of what it holds and whether the preferences use it now. Also folders under the plugins folder that are not a loadable pack, with the reason, and the current selection: the key sound, melody, music and effect packs, the enabled command and phrase tables, the helpcode tables for quanpin and shuangpin, and whether / mode and K mode are on. The kinds are sound (key sounds or a key melody), music (background music), command_table (/ commands), effect (typing effect parameters), phrase_table (K-mode phrases), helpcode (a helpcode table), wordbook (a book for 背单词), symbol_set (groups for the symbol panel) and, on desktop, script (an experimental sandboxed WebAssembly plugin).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_plugins(&self) -> Result<Json<PluginList>, String> {
        let config = self.config.clone();
        blocking(move || {
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let preferences = plugins::preferences(&state_dir, config.edition(&document)?)?;
            Ok(Json(plugins::list(
                &state_dir,
                &BuiltinPaths::from_options(&document),
                &preferences,
            )))
        })
        .await
    }

    #[tool(
        name = "get_plugin",
        description = "Read one pack so it can be changed: its whole plugin.toml, the absolute path of its folder, and every other file in it with its size and absolute path. To change a pack, edit the manifest and call create_plugin with replace: true, passing each unchanged file by the path given here in files: every file is read before the pack is replaced. Packs that ship with the input method can be read too, as a model for one of the user's own under another id. Refused for a pack that is not installed and for a folder list_plugins reports as an issue.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_plugin(
        &self,
        Parameters(request): Parameters<PluginRequest>,
    ) -> Result<Json<PluginFiles>, String> {
        let config = self.config.clone();
        blocking(move || {
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            plugins::get(&state_dir, &BuiltinPaths::from_options(&document), &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "create_plugin",
        description = "Install a data plugin pack from a plugin.toml and its files, by exactly the rules the settings page's import uses, so the settings app accepts what this accepts. A pack is plugin.toml, the audio or data files it names and optional .txt or .md notices (at most 64 KiB each), at most 16 files and nothing else; file names are ASCII letters, digits, `_`, `-` and `.`, starting with a letter or digit, at most 64 bytes. Every manifest has schema_version = 1; kind; id (1-64 bytes of lowercase ASCII letters, digits, `.`, `-` and `_`, starting with a letter or digit; the installed folder is named after it); name (at most 80 bytes); version (at most 32 bytes); license (an SPDX expression such as CC0-1.0, CC-BY-4.0 or MIT); optional author (at most 120 bytes) and description (at most 500 bytes); permissions only [] or left out. Any other key, at the top or inside a kind's table, refuses the pack. Ids reserved for packs that ship with the input method: sound default, twinkle, msime-typewriter, msime-bubble, msime-8bit, msime-woodblock, msime-pentatonic, msime-canon and msime-ode-to-joy, music msime-music-lofi and msime-music-ambient, phrase_table latex-symbols. The kinds: phrase_table (K mode: Shift+K, then a key) has 1-2000 [[phrases]] rows of key (1-32 lowercase ASCII letters) and text (1-199 UTF-16 units, not blank, no control characters, newline or tab); a key may have several texts, a key and text pair appears once; a text may hold ${cursor} to leave the caret there after it is typed (only the first one; $${cursor} types the literal ${cursor}); no files. command_table (/ mode) has 1-256 [[commands]] rows of trigger (1-32 lowercase ASCII letters, unique in the pack), title (at most 48 bytes) and template (at most 199 UTF-16 units, no control characters): literal text and the placeholders {date} or {date:FORMAT} (strftime, default %Y-%m-%d), {time} or {time:FORMAT} (default %H:%M) and {weekday}; a lone { or } is refused; no files. symbol_set (the symbol panel) has 1-32 [[groups]] of tab (symbols or kaomoji), title (at most 48 bytes, unique within a tab), optional keywords (at most 256 bytes, for search) and items (1-512 strings of 1-64 UTF-16 units, unique in the group), at most 2048 items in all; no files. effect has [effect] style (flash, sparks or power_mode) and optional intensity (0-100), colors (1-4 '#RRGGBB' strings), duration_ms (60-1500) and particles (0-64); no files. helpcode has [helpcode] table naming a .txt file of lines 字=码 (exactly one non-ASCII character, `=` without spaces, a code of 1-2 lowercase ASCII letters, each character once), # comments and empty lines, UTF-8, at most 1 MiB. wordbook has [wordbook] file naming a .tsv file of word<TAB>meaning or word<TAB>phonetic<TAB>meaning lines (word 1-64 characters, phonetic at most 64 and may be empty, meaning 1-256, no control characters, each word once, 1-20000 words), UTF-8, at most 4 MiB; a word book's id is only lowercase letters, digits and `-`, not starting or ending with `-`, at most 59 characters, and its name at most 64 characters. sound has mode = 'keys' (the default) with [sounds] default (required) and optional space, enter, backspace, commit and achievement, or mode = 'sequence' with [sequence] sample, semitones (1-128 integers from -24 to 24) and optional advance (key or commit); samples are RIFF/WAVE .wav files, at most 8 distinct, each at most 512 KiB and 1.5 seconds, 4 MiB together. music has [music] tracks, 1-8 distinct .wav or .ogg files, each at most 16 MiB and 15 minutes, 64 MiB together. Data files, audio and notices come in files (the name in the pack to the absolute path of a file on this computer, which the server only reads: it refuses a path with a symbolic link anywhere on the way or that is not a regular file, and a file over 16 MiB before reading it) or texts (the name to its UTF-8 text, at most 1 MiB each, for a helpcode table, a word book or a notice), each name in only one of the two. A script plugin (kind = \"script\") is refused here: build its folder with msime-pack and import it with install_plugin_folder. Refused when a pack of the same kind and id is installed, unless replace is true; a refusal says which rule failed, in Chinese. The result says what the user has to do for the pack to take effect, for example turning on 启用 for a phrase or command table in 设置 → 插件 → 我的插件 and K mode or / mode in 设置 → 输入 → 快捷模式; tell the user in plain words. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn create_plugin(
        &self,
        Parameters(request): Parameters<CreatePluginRequest>,
    ) -> Result<Json<InstalledPlugin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let preferences = plugins::preferences(&state_dir, config.edition(&document)?)?;
            plugins::create(&state_dir, &preferences, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: create_plugin {}",
            match &result {
                Ok(installed) if installed.0.replaced => "replaced",
                Ok(_) => "installed",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "install_plugin_folder",
        description = "Import a pack that already exists on this computer, a folder or a .zip holding its files at the top level or inside one folder, by the rules of the settings page's import. This is how a script plugin built with msime-pack is installed, and how a data pack the user downloaded or made elsewhere is installed. path is an absolute path with no symbolic link on the way; the server only reads it. Refused when a pack of the same kind and id is installed, unless replace is true. A script plugin does not run after this: tell the user plainly that it stays off until they turn it on and grant the capabilities it asks for in 设置 → 插件 → 脚本插件（实验）. This server never grants capabilities. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn install_plugin_folder(
        &self,
        Parameters(request): Parameters<InstallPluginRequest>,
    ) -> Result<Json<InstalledPlugin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let preferences = plugins::preferences(&state_dir, config.edition(&document)?)?;
            plugins::install_folder(&state_dir, &preferences, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: install_plugin_folder {}",
            match &result {
                Ok(installed) if installed.0.replaced => "replaced",
                Ok(_) => "installed",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "delete_plugin",
        description = "Delete a pack the user installed: its folder under the plugins folder, with everything in it. Refused for packs that ship with the input method (the built-in sound and music packs and the official packs) and for a pack that is not installed. A deleted script plugin's grant is removed with it. When the preferences still select or enable the pack, the result says where; the input method then falls back to the default there, as after the settings page removes a pack in use. Confirm with the user first. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn delete_plugin(
        &self,
        Parameters(request): Parameters<PluginRequest>,
    ) -> Result<Json<DeletedPlugin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let preferences = plugins::preferences(&state_dir, config.edition(&document)?)?;
            plugins::delete(&state_dir, &preferences, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: delete_plugin {}",
            if result.is_ok() { "deleted" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "publish_plugin",
        description = "Publish an installed data pack the user made (not one that ships with the input method, not a script plugin) to the 水杉输入法 plugin community, using the account the user signed in to in the 水杉输入法 settings app. This server never signs in or handles passwords: when no one is signed in it says so, and you ask the user to sign in there and try again. Publishing is always public: everyone can see, download and rate the pack. Before publishing, ask the user in plain words to confirm that everyone may see it, that they made its audio and text or have the right to share them, and that others may download and use it under the licence in its manifest; a pack downloaded from the community is not theirs to publish. name (1-32 characters) and description (at most 280) are the listing's title and text, by default the manifest's name and description. The pack is checked on this computer by the rules of the settings app's publish dialog before anything is sent; zipped it may be at most 8 MB. An account publishes at most 20 packs, 32 MB together, and at most 10 times an hour; list_community_plugins shows what it has published. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn publish_plugin(
        &self,
        Parameters(request): Parameters<PublishPluginRequest>,
    ) -> Result<Json<PublishedPlugin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let community = plugin_community::community(&state_dir, config.edition(&document)?)?;
            plugin_community::publish(&state_dir, &community, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: publish_plugin {}",
            if result.is_ok() {
                "published"
            } else {
                "refused"
            }
        );
        result
    }

    #[tool(
        name = "list_community_plugins",
        description = "List the plugin packs the signed-in account published to the community, removed ones included: community id, kind, plugin id, title, version, downloads and when it was published. Uses the account the user signed in to in the 水杉输入法 settings app; this server never signs in itself, so when no one is signed in, ask the user to sign in there.",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn list_community_plugins(&self) -> Result<Json<CommunityPluginList>, String> {
        let config = self.config.clone();
        blocking(move || {
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let community = plugin_community::community(&state_dir, config.edition(&document)?)?;
            plugin_community::list(&community).map(Json)
        })
        .await
    }

    #[tool(
        name = "delete_community_plugin",
        description = "Delete a pack the signed-in account published from the plugin community for good, with its downloads and ratings; copies other people installed stay on their computers, and the pack installed here is left alone. Takes the community id list_community_plugins or publish_plugin gave. Confirm with the user first. Only offered when the user started the server with --allow-write.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn delete_community_plugin(
        &self,
        Parameters(request): Parameters<CommunityPluginRequest>,
    ) -> Result<Json<DeletedCommunityPlugin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let community = plugin_community::community(&state_dir, config.edition(&document)?)?;
            plugin_community::delete(&community, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: delete_community_plugin {}",
            if result.is_ok() { "deleted" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "get_typing_statistics",
        description = "Read aggregate typing statistics: characters per day, per category and per input scheme, time spent typing and typing speed, the distribution over the hours of the day, and how often each candidate position is chosen. Counts only; no text is ever recorded.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_typing_statistics(
        &self,
        Parameters(request): Parameters<StatisticsRequest>,
    ) -> Result<Json<StatisticsView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            statistics::load(&state_dir, &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "read_diagnostic_log",
        description = "Read the most recent lines of the input method's diagnostic log, the record its hosts keep of focus changes, slow requests, candidate window and dictionary events and failures, to look into a problem the user describes. Filter by text and by local time. Also tells whether the log is on; nothing is recorded while it is off.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_diagnostic_log(
        &self,
        Parameters(request): Parameters<LogRequest>,
    ) -> Result<Json<LogView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            diagnostics::load(&state_dir, &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "set_diagnostic_log",
        description = "Turn the input method's diagnostic log on, so that what goes wrong next is recorded, or off once the problem is understood. Only changes whether the input method writes its log on this computer.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_diagnostic_log(
        &self,
        Parameters(request): Parameters<SwitchRequest>,
    ) -> Result<Json<SwitchView>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let document = config.read_options()?;
            let state_dir = config.state_dir(&document)?;
            let edition = config.edition(&document)?;
            diagnostics::set(&state_dir, &config.options, edition, request.enabled).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: set_diagnostic_log {}",
            match (&result, request.enabled) {
                (Err(_), _) => "refused",
                (Ok(_), true) => "on",
                (Ok(_), false) => "off",
            }
        );
        result
    }

    #[tool(
        name = "list_dictionary_words",
        description = "List the words the user added to a typing dictionary, or with include_bundled and a code_prefix every word the dictionary has under that code, with the weights that rank them.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_dictionary_words(
        &self,
        Parameters(request): Parameters<WordListRequest>,
    ) -> Result<Json<WordListPage>, String> {
        let limit = request.limit.unwrap_or(DEFAULT_PAGE);
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_PAGE}"));
        }
        let code_prefix = request.code_prefix.unwrap_or_default();
        let include_bundled = request.include_bundled.unwrap_or(false);
        if include_bundled && code_prefix.trim().is_empty() {
            return Err("include_bundled needs a code_prefix".into());
        }
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let page = msime_host_api::dictionary_words(
                &options,
                request.dictionary.into(),
                &code_prefix,
                include_bundled,
                request.offset.unwrap_or(0),
                limit,
            )?;
            Ok(Json(WordListPage {
                words: page.words.into_iter().map(Into::into).collect(),
                has_more: page.has_more,
            }))
        })
        .await
    }

    #[tool(
        name = "edit_dictionary_words",
        description = "Add words to a typing dictionary, give a word another weight, or remove one. A word is found by its exact code and word, so list or look it up first. Edits are applied in order and the first failure stops the rest.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_dictionary_words(
        &self,
        Parameters(request): Parameters<WordEditRequest>,
    ) -> Result<Json<EditOutcome>, String> {
        if request.edits.is_empty() || request.edits.len() > MAX_EDITS {
            return Err(format!("send between 1 and {MAX_EDITS} edits"));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let edits: Vec<WordEdit> = request.edits.into_iter().map(Into::into).collect();
        let outcome = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            Ok(apply_edits(
                &options,
                &edits,
                msime_host_api::edit_dictionary_word,
            ))
        })
        .await?;
        eprintln!(
            "msime-mcp: edit_dictionary_words applied {}{}",
            outcome.applied,
            if outcome.error.is_some() {
                " then failed"
            } else {
                ""
            }
        );
        Ok(Json(outcome))
    }

    #[tool(
        name = "import_dictionary_words",
        description = "Add up to 200 words to one typing dictionary. Words already there are left as they are and counted; words the dictionary refuses are named by index and the rest still go in, so sending the same words again is safe.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn import_dictionary_words(
        &self,
        Parameters(request): Parameters<WordImportRequest>,
    ) -> Result<Json<WordImportOutcome>, String> {
        if request.words.is_empty() || request.words.len() > words::MAX_IMPORT {
            return Err(format!("send between 1 and {} words", words::MAX_IMPORT));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let kind = request.dictionary.into();
        let new_words = request.new_words();
        let result = blocking(move || {
            let _guard = guard;
            let _shared = claim_shared_write(&config)?;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let request_id = msime_client_core::uuid::Uuid::new_v4().simple().to_string();
            let mut hosts = QuiescedHosts::new(Some(options.user_data()), || {});
            hosts.run(|| {
                msime_host_api::import_dictionary_words(&options, kind, &new_words, &request_id)
            })
        })
        .await;
        match &result {
            Ok(outcome) => eprintln!(
                "msime-mcp: import_dictionary_words added {}, found {}, refused {}",
                outcome.added,
                outcome.existing,
                outcome.rejected.len()
            ),
            Err(_) => eprintln!("msime-mcp: import_dictionary_words failed"),
        }
        result.map(|outcome| Json(outcome.into()))
    }

    #[tool(
        name = "lookup_candidates",
        description = "Show the candidates typing a code offers from the local dictionaries, in the input method's order, with where each came from and the weight of each dictionary word. Explains why a word ranks where it does. Types into a session of its own that learns nothing; no cloud or AI suggestions are included. With the experimental Japanese mixing option on, the Japanese conversion it adds for whole romaji is included, with origin japanese.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn lookup_candidates(
        &self,
        Parameters(request): Parameters<LookupRequest>,
    ) -> Result<Json<LookupView>, String> {
        let limit = request.limit.unwrap_or(words::DEFAULT_LOOKUP);
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let candidates = msime_host_api::lookup_candidates(
                &options,
                request.scheme.map(Into::into),
                &request.code,
                limit,
            )?;
            Ok(Json(LookupView {
                candidates: candidates.into_iter().map(Into::into).collect(),
            }))
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for MsimeServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            self.config.server_name(),
            env!("MSIME_APP_VERSION"),
        ))
        .with_instructions(INSTRUCTIONS)
    }
}

impl MsimeServer {
    /// Take the next write slot, or refuse when another write is still running or the previous one was too recent. The guard goes into the blocking work, not the handler future: a cancelled request drops the future while the work runs on.
    fn claim_write(&self) -> Result<WriteGuard, String> {
        if self
            .writing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err("another change is still being applied; try again when it finishes".into());
        }
        let guard = WriteGuard(self.writing.clone());
        let mut last = self
            .last_write
            .lock()
            .map_err(|_| "the server is shutting down")?;
        let now = Instant::now();
        if last.is_some_and(|previous| now.duration_since(previous) < WRITE_INTERVAL) {
            return Err("writes are limited to one a second; try again shortly".into());
        }
        *last = Some(now);
        Ok(guard)
    }
}

/// 跨进程串行写入，并限制两次写入开始的最短间隔；守卫覆盖整个写入，包括错误返回和展开。
fn claim_shared_write(config: &Config) -> Result<file_lock::ScopedFileLock, String> {
    let state_dir = config.state_dir(&config.read_options()?)?;
    let file = file_lock::open_private_lock_file(state_dir.join(WRITE_LOCK))
        .map_err(|_| "cannot open the write lock in the state directory")?;
    let held =
        file_lock::ScopedFileLock::exclusive(file).map_err(|_| "cannot take the write lock")?;
    let mut file = held.file();
    let bytes = crate::bounded::read(file, WRITE_LOCK_READ_LIMIT)
        .map_err(|_| "cannot read the write lock")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "cannot read the write lock")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "the system clock is before 1970")?
        .as_millis();
    // 时间戳在当前时间之后表示时钟回拨，不算最近一次写入。
    if let Ok(previous) = text.trim().parse::<u128>() {
        if previous <= now && now - previous < WRITE_INTERVAL.as_millis() {
            return Err("writes are limited to one a second; try again shortly".into());
        }
    }
    file.set_len(0)
        .and_then(|()| file.seek(SeekFrom::Start(0)))
        .and_then(|_| file.write_all(now.to_string().as_bytes()))
        .map_err(|_| "cannot record the write")?;
    Ok(held)
}

/// Clears the write-in-progress flag when the write it covers ends, however it ends.
struct WriteGuard(Arc<AtomicBool>);

impl Drop for WriteGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Apply `edits` in order under one release, so the input hosts let go of the dictionary once for the whole call rather than once per edit, and are let back when this returns. On Linux and macOS that is a lease the hosts find on their timers; the macOS input method finds it within a second, and the desktop app also tells it at once, which would take the macOS host crate here for a second's difference. On Windows the Server is asked over its pipe and answers once its sessions are gone.
fn apply_edits<E>(
    options: &DictionaryOptions,
    edits: &[E],
    apply: impl Fn(&DictionaryOptions, &E, &str) -> Result<(), String>,
) -> EditOutcome {
    let mut hosts = QuiescedHosts::new(Some(options.user_data()), || {});
    for (index, edit) in edits.iter().enumerate() {
        let request_id = msime_client_core::uuid::Uuid::new_v4().simple().to_string();
        if let Err(error) = hosts.run(|| apply(options, edit, &request_id)) {
            return EditOutcome {
                applied: index,
                failed_index: Some(index),
                error: Some(error),
            };
        }
    }
    EditOutcome {
        applied: edits.len(),
        failed_index: None,
        error: None,
    }
}

/// Run file and dictionary work off the protocol task, so a slow disk or a busy dictionary does not stall the stdio loop.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| "the request was interrupted".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(unix, not(target_os = "android")))]
    use std::fs::File;
    use std::path::PathBuf;

    fn server(allow_write: bool) -> MsimeServer {
        server_with(allow_write, false)
    }

    fn server_with(allow_write: bool, allow_dictionary_read: bool) -> MsimeServer {
        MsimeServer::new(Config {
            options: PathBuf::from("/nonexistent/runtime-options.json"),
            state_dir: None,
            allow_write,
            allow_dictionary_read,
        })
    }

    #[test]
    fn write_tools_are_offered_only_when_allowed() {
        let names = |server: &MsimeServer| {
            let mut names: Vec<String> = server
                .tool_router
                .list_all()
                .into_iter()
                .map(|tool| tool.name.to_string())
                .collect();
            names.sort();
            names
        };
        assert_eq!(
            names(&server(false)),
            [
                "get_candidate_skin",
                "get_plugin",
                "get_preferences",
                "get_typing_statistics",
                "list_candidate_skins",
                "list_community_plugins",
                "list_community_skins",
                "list_plugins",
                "list_quick_phrases",
                "read_diagnostic_log",
                "set_diagnostic_log"
            ]
        );
        assert_eq!(
            names(&server(true)),
            [
                "create_candidate_skin",
                "create_plugin",
                "delete_candidate_skin",
                "delete_community_plugin",
                "delete_community_skin",
                "delete_plugin",
                "edit_quick_phrases",
                "get_candidate_skin",
                "get_plugin",
                "get_preferences",
                "get_typing_statistics",
                "install_plugin_folder",
                "list_candidate_skins",
                "list_community_plugins",
                "list_community_skins",
                "list_plugins",
                "list_quick_phrases",
                "publish_candidate_skin",
                "publish_plugin",
                "read_diagnostic_log",
                "set_community_skin_visibility",
                "set_diagnostic_log",
                "update_preferences"
            ]
        );
        assert_eq!(
            names(&server_with(false, true)),
            [
                "get_candidate_skin",
                "get_plugin",
                "get_preferences",
                "get_typing_statistics",
                "list_candidate_skins",
                "list_community_plugins",
                "list_community_skins",
                "list_dictionary_words",
                "list_plugins",
                "list_quick_phrases",
                "lookup_candidates",
                "read_diagnostic_log",
                "set_diagnostic_log"
            ]
        );
        assert_eq!(
            names(&server_with(true, true)),
            [
                "create_candidate_skin",
                "create_plugin",
                "delete_candidate_skin",
                "delete_community_plugin",
                "delete_community_skin",
                "delete_plugin",
                "edit_dictionary_words",
                "edit_quick_phrases",
                "get_candidate_skin",
                "get_plugin",
                "get_preferences",
                "get_typing_statistics",
                "import_dictionary_words",
                "install_plugin_folder",
                "list_candidate_skins",
                "list_community_plugins",
                "list_community_skins",
                "list_dictionary_words",
                "list_plugins",
                "list_quick_phrases",
                "lookup_candidates",
                "publish_candidate_skin",
                "publish_plugin",
                "read_diagnostic_log",
                "set_community_skin_visibility",
                "set_diagnostic_log",
                "update_preferences"
            ]
        );
    }

    #[test]
    fn skin_tools_describe_decoration_motion_and_animated_png() {
        let description = |name: &str| {
            server(true)
                .tool_router
                .list_all()
                .into_iter()
                .find(|tool| tool.name == name)
                .and_then(|tool| tool.description)
                .unwrap()
                .to_string()
        };
        let create = description("create_candidate_skin");
        for word in [
            "motion (none, float, sway, breathe or bounce",
            "motion_period_ms (1200-8000",
            "animated PNG (APNG, at most 120 frames",
            // Version 2 motion is not tied to canvas.
            "decoration motion and animated PNG below, with any base",
            "wechat_scroll",
            "or canvas, msime-windows' full-image base, which requires schema_version = 2 (version 1 with canvas is refused)",
            "Only with base = canvas: [candidate_window.border_image]",
            "[[candidate_window.decorations]]",
            "With any other base these canvas-only keys are ignored",
            "a real X11 input context, focus and classicui ownership",
            "Wayland, IBus and third-party UIs keep their existing fallback",
            "The Web engine and the macOS, Windows, iOS, Android, HarmonyOS 2in1 and Linux X11 candidate windows draw these canvas textures using their platform image codecs",
            // 共享校验不等于每个宿主已绘制全部布局原语。
            "which all hosts validate and render where their native entry supports each primitive",
            "expand_panel puts a button at the end of the horizontal list",
            "candidate_text",
            "[candidate.dark.menu]",
            "#RGB, #RGBA, #RRGGBB, #RRGGBBAA, rgb(r, g, b) or rgba(r, g, b, a)",
            "require explicit server support before upload",
            "backdrop_opacity and backdrop_luminosity (numbers 0-1)",
            // Windows draws acrylic and mica as system materials, with msime-windows' opaque fallback.
            "Windows draws the system material behind the card, mica as acrylic before Windows 11",
            "an opaque surface where the material is unavailable, transparency effects are off or high contrast is on",
            "item_corner_radius_dip (0-16)",
            // 创建与社区安装共用扩展资源预算，预览保持旧格式。
            "at most 64 in all",
            "the preview stays PNG/JPEG",
            "empty author or description reads as not written",
            // 图片可以给本机文件的路径，规则与 base64 相同。
            "images (standard base64) and image_files (the absolute path of a file on this computer)",
            "Prefer image_files",
            "refuses a path with a symbolic link anywhere on the way or that is not a regular file",
            "refuses a file over 1 MiB before reading it",
        ] {
            assert!(create.contains(word), "{word}");
        }
        let get = description("get_candidate_skin");
        for word in [
            "its whole skin.toml",
            "the absolute path of the file on this computer",
            "png, apng for a PNG of more than one frame, jpeg, gif or webp",
            "call create_candidate_skin with replace: true, passing each unchanged image by the file given here in image_files",
        ] {
            assert!(get.contains(word), "{word}");
        }
        let delete = description("delete_candidate_skin");
        for word in [
            "directly under the input method's skin directory",
            "anything there that is not a real folder (a file or a symbolic link)",
            "chosen as candidate_skin or candidate_skin_dark in the preferences",
            "the app's next sync removes the private copy there too",
            "Only offered when the user started the server with --allow-write",
        ] {
            assert!(delete.contains(word), "{word}");
        }
        let list = description("list_candidate_skins");
        assert!(list.contains("idle motion, motion period and frame count"));
        assert!(list.contains("whether it is a canvas"));
        assert!(list.contains("notices about keys that were ignored or are not drawn"));
        assert!(create.contains("reports such ignored or undrawn keys as notices"));
    }

    #[test]
    fn preference_tools_describe_choosing_a_skin() {
        let description = |name: &str| {
            server(true)
                .tool_router
                .list_all()
                .into_iter()
                .find(|tool| tool.name == name)
                .and_then(|tool| tool.description)
                .unwrap()
                .to_string()
        };
        assert!(description("get_preferences")
            .contains("the candidate-window skin chosen for light and for dark mode"));
        let update = description("update_preferences");
        for word in [
            "set candidate_skin for light mode and candidate_skin_dark for dark mode",
            "candidate_skin also serves dark mode while candidate_skin_dark is null",
            "on night, ink or shuishan only in dark mode, on light or paper only in light mode, on system in both",
            "switches global_theme to custom",
        ] {
            assert!(update.contains(word), "{word}");
        }
    }

    #[test]
    fn community_tools_ask_before_sharing_and_never_sign_in() {
        let description = |name: &str| {
            server(true)
                .tool_router
                .list_all()
                .into_iter()
                .find(|tool| tool.name == name)
                .and_then(|tool| tool.description)
                .unwrap()
                .to_string()
        };
        let publish = description("publish_candidate_skin");
        for word in [
            "the account the user signed in to in the 水杉输入法 settings app",
            "This server never signs in or handles passwords",
            "visibility is required and has no default",
            "ask the user in plain words to confirm that everyone may see it and that they made the images or have the right to share them",
            "pass license_assets",
            "that entry is replaced",
            "Only offered when the user started the server with --allow-write",
        ] {
            assert!(publish.contains(word), "{word}");
        }
        assert!(
            description("set_community_skin_visibility").contains("confirm with the user first")
        );
        assert!(description("delete_community_skin").contains("Confirm with the user first"));
        assert!(description("list_community_skins").contains("this server never signs in itself"));
        // 服务器说明仍然声明它不登录。
        assert!(INSTRUCTIONS.contains("this server never signs in to the account itself"));
        assert!(INSTRUCTIONS.contains("confirm with the user"));
    }

    #[test]
    fn plugin_tools_explain_enabling_grants_and_publishing() {
        let description = |name: &str| {
            server(true)
                .tool_router
                .list_all()
                .into_iter()
                .find(|tool| tool.name == name)
                .and_then(|tool| tool.description)
                .unwrap()
                .to_string()
        };
        let create = description("create_plugin");
        for word in [
            "by exactly the rules the settings page's import uses",
            "[[phrases]]",
            "[[commands]]",
            "{weekday}",
            "[[groups]]",
            "[helpcode] table",
            "[wordbook] file",
            "RIFF/WAVE .wav files",
            "refuses a path with a symbolic link anywhere on the way",
            "A script plugin (kind = \"script\") is refused here",
            "设置 → 输入 → 快捷模式",
        ] {
            assert!(create.contains(word), "{word}");
        }
        let install = description("install_plugin_folder");
        assert!(install.contains("设置 → 插件 → 脚本插件（实验）"));
        assert!(install.contains("This server never grants capabilities"));
        let publish = description("publish_plugin");
        for word in [
            "This server never signs in or handles passwords",
            "Publishing is always public",
            "ask the user in plain words to confirm",
            "Only offered when the user started the server with --allow-write",
        ] {
            assert!(publish.contains(word), "{word}");
        }
        assert!(description("delete_plugin").contains("Confirm with the user first"));
        assert!(description("delete_community_plugin").contains("Confirm with the user first"));
        assert!(description("list_community_plugins").contains("this server never signs in"));
        // 服务器说明告诉代理自己写清单、装好之后怎么打开。
        for word in [
            "write the plugin.toml yourself by the rules in create_plugin's description",
            "设置 → 插件 → 我的插件",
            "K mode or / mode on in 设置 → 输入 → 快捷模式",
            "脚本插件（实验）",
        ] {
            assert!(INSTRUCTIONS.contains(word), "{word}");
        }
    }

    #[test]
    fn writes_are_spaced_out() {
        let server = server(true);
        drop(server.claim_write().unwrap());
        assert!(server.claim_write().is_err());
        *server.last_write.lock().unwrap() = Some(Instant::now() - WRITE_INTERVAL);
        server.claim_write().unwrap();
    }

    #[test]
    fn a_running_write_holds_off_the_next() {
        let server = server(true);
        let guard = server.claim_write().unwrap();
        *server.last_write.lock().unwrap() = Some(Instant::now() - WRITE_INTERVAL);
        assert!(server.claim_write().is_err());
        // The refusal did not take the slot.
        drop(guard);
        server.claim_write().unwrap();
    }

    #[test]
    fn oversized_shared_write_lock_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, b"{}").unwrap();
        std::fs::write(directory.path().join(WRITE_LOCK), vec![b'x'; 129]).unwrap();
        let config = Config {
            options,
            state_dir: Some(directory.path().to_owned()),
            allow_write: true,
            allow_dictionary_read: false,
        };

        assert_eq!(
            claim_shared_write(&config).unwrap_err(),
            "cannot read the write lock"
        );
    }

    #[cfg(all(unix, not(target_os = "android")))]
    fn try_shared_write_test_lock(file: &File) -> bool {
        match file.try_lock() {
            Ok(()) => true,
            Err(std::fs::TryLockError::WouldBlock) => false,
            Err(error) => panic!("合成写锁取锁失败：{error}"),
        }
    }

    #[cfg(all(unix, not(target_os = "android")))]
    fn assert_shared_write_scope_releases_lock(unwind: bool) {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(&options, b"{}").unwrap();
        let config = Config {
            options,
            state_dir: Some(directory.path().to_owned()),
            allow_write: true,
            allow_dictionary_read: false,
        };
        let path = directory.path().join(WRITE_LOCK);
        let held = claim_shared_write(&config).unwrap();
        let inherited = held.file().try_clone().unwrap();
        let contender = file_lock::open_private_lock_file(&path).unwrap();
        assert!(!try_shared_write_test_lock(&contender));

        let result = std::panic::catch_unwind(move || {
            let _held = held;
            if unwind {
                panic!("合成的写入展开");
            }
        });
        assert_eq!(result.is_err(), unwind);
        assert!(
            try_shared_write_test_lock(&contender),
            "MCP 写入作用域结束后不能由旧描述符副本继续占锁"
        );
        drop(inherited);
        let next = file_lock::open_private_lock_file(&path).unwrap();
        assert!(!try_shared_write_test_lock(&next));
        contender.unlock().unwrap();
        assert!(try_shared_write_test_lock(&next));
        next.unlock().unwrap();
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn a_finished_shared_write_scope_releases_an_inherited_lock() {
        assert_shared_write_scope_releases_lock(false);
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn an_unwinding_shared_write_scope_releases_an_inherited_lock() {
        assert_shared_write_scope_releases_lock(true);
    }

    #[tokio::test]
    async fn an_empty_preferences_change_does_not_take_the_slot() {
        let server = server(true);
        let change: PreferencesChange =
            serde_json::from_value(serde_json::json!({ "expected_revision": 0 })).unwrap();
        assert_eq!(
            server
                .update_preferences(Parameters(change))
                .await
                .err()
                .as_deref(),
            Some("no preference to change")
        );
        server.claim_write().unwrap();
    }

    #[test]
    fn edits_read_as_the_engine_edits() {
        let request: EditRequest = serde_json::from_value(serde_json::json!({ "edits": [
            { "op": "add", "code": "yx", "text": "邮箱" },
            { "op": "replace", "previous": { "code": "yx", "text": "邮箱" }, "replacement": { "code": "yx", "text": "someone@example.com" } },
            { "op": "remove", "code": "yx", "text": "someone@example.com" }
        ]}))
        .unwrap();
        let edits: Vec<QuickPhraseEdit> = request.edits.into_iter().map(Into::into).collect();
        let phrase = |text: &str| QuickPhrase {
            code: "yx".into(),
            text: text.into(),
        };
        assert_eq!(
            edits,
            [
                QuickPhraseEdit::Add(phrase("邮箱")),
                QuickPhraseEdit::Replace {
                    previous: phrase("邮箱"),
                    replacement: phrase("someone@example.com")
                },
                QuickPhraseEdit::Remove(phrase("someone@example.com")),
            ]
        );
        assert!(serde_json::from_value::<EditRequest>(
            serde_json::json!({ "edits": [{ "op": "add", "code": "a", "text": "b", "weight": 1 }] })
        )
        .is_err());
    }
}

//! Host capabilities and the shared surface route vocabulary.
//!
//! Every platform host reaches the shared management UI by naming a route
//! instead of opening its own window, and the shared UI decides what to render
//! from injected capabilities instead of sniffing the user agent. Both sides of
//! that agreement live here so no host re-implements the strings.

use crate::edition::Edition;
use crate::preferences::InputScheme;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Routes are embedded in command lines and environment variables, so they stay
/// short and free of anything a shell or a line-framed channel would reinterpret.
const MAX_ROUTE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RouteError {
    #[error("route is empty")]
    Empty,
    #[error("route is too long")]
    TooLong,
    #[error("route contains an unsupported character")]
    IllegalCharacter,
    #[error("route is not a known surface")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostPlatform {
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
    /// The HarmonyOS phone host. Its keyboard is an InputMethodExtensionAbility panel, so it groups with the mobile hosts rather than the desktop ones. A HarmonyOS 2in1 host would group with the desktop side instead, but it gets its own variant only once that host exists: every capability keyed off `is_desktop` would otherwise claim a surface no HarmonyOS code has written yet.
    Harmony,
}

impl HostPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            HostPlatform::Windows => "windows",
            HostPlatform::Macos => "macos",
            HostPlatform::Linux => "linux",
            HostPlatform::Android => "android",
            HostPlatform::Ios => "ios",
            HostPlatform::Harmony => "harmony",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RouteError> {
        match value {
            "windows" => Ok(HostPlatform::Windows),
            "macos" => Ok(HostPlatform::Macos),
            "linux" => Ok(HostPlatform::Linux),
            "android" => Ok(HostPlatform::Android),
            "ios" => Ok(HostPlatform::Ios),
            "harmony" => Ok(HostPlatform::Harmony),
            "" => Err(RouteError::Empty),
            _ => Err(RouteError::Unknown),
        }
    }

    /// Desktop hosts own separate panel windows; mobile hosts render the shared
    /// UI inside a single activity or container app.
    pub fn is_desktop(self) -> bool {
        matches!(
            self,
            HostPlatform::Windows | HostPlatform::Macos | HostPlatform::Linux
        )
    }
}

/// Why a Linux desktop's candidate panel ignores the candidate font, colour and skin settings. The Linux hosts do not draw the candidate list: IBus hands it to whichever panel the desktop runs and Fcitx5 to whichever user interface it loaded, and some of those draw it their own way.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePanelLimit {
    /// GNOME Shell starts IBus with its panel disabled and draws the candidate popup itself, styled by the shell theme: it reads neither the IBus panel font nor the text attributes that carry the colours.
    GnomeShell,
    /// Fcitx5's classic UI is drawing a theme the user picked, which the host never replaces, so the colours and skin do not reach it. The font still does.
    FcitxTheme,
    /// Fcitx5 hands the list to the desktop's Kimpanel, which draws it with the desktop's own font and theme.
    Kimpanel,
}

impl CandidatePanelLimit {
    /// The file the running Linux host writes its finding to: `candidate-panel.json` under `$XDG_RUNTIME_DIR/msime-client`, the per-session directory that goes away with the session the finding describes. A relative or missing runtime directory yields nothing.
    ///
    /// 目录名随本进程所在安装包的版本（`Edition::linux_package_identity_or_full`，full 是 `msime-client`），读的是同一版本宿主写的那一份。
    pub fn status_file(runtime_directory: Option<&std::ffi::OsStr>) -> Option<std::path::PathBuf> {
        let directory = std::path::PathBuf::from(runtime_directory?);
        directory.is_absolute().then(|| {
            directory
                .join(&crate::edition::Edition::linux_package_identity_or_full().client_directory)
                .join("candidate-panel.json")
        })
    }

    /// Reads the host's report, `{"host": "ibus" | "fcitx5", "limit": <name> | null}`. Anything else - no file, a panel that honours the settings, a name this build does not know - reads as no limit, so the page never warns on a guess.
    pub fn from_host_status(document: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(document).ok()?;
        serde_json::from_value(value.get("limit")?.clone()).ok()
    }
}

/// What the surrounding host can actually do. The shared UI renders from this
/// rather than guessing from `navigator.userAgent`, which previously hid working
/// controls on Windows and macOS and left 打字统计 dead on every desktop.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostCapabilities {
    pub platform: HostPlatform,
    /// The shared settings surface uses phone navigation and touch-oriented copy. This is usually
    /// the inverse of `is_desktop`, but a host may override it from the actual device form factor:
    /// one HarmonyOS HAP runs on both phones and 2-in-1 machines.
    pub mobile_settings: bool,
    /// The host can restart the input method service from the settings page.
    pub restart_input_method: bool,
    /// The host can open the shared panels as separate always-on-top windows.
    pub panel_windows: bool,
    /// The host keeps per-application versus global Chinese/English mode state.
    pub ime_mode_scope: bool,
    /// The host records typing statistics.
    pub typing_statistics: bool,
    /// The host exposes the shared fuzzy-pinyin settings.
    pub fuzzy_pinyin: bool,
    /// The host can enumerate installed font families.
    pub system_fonts: bool,
    /// The shared UI draws its own titlebar and resize handles.
    pub window_chrome: bool,
    /// The host presents a floating toolbar in some form. On Linux that is the
    /// IBus property menu rather than a drawn window, so the enable switch and
    /// component visibility are the meaningful controls there.
    pub floating_toolbar: bool,
    /// The toolbar surface honours scale and icon size. An IBus property menu
    /// cannot express either value.
    pub floating_toolbar_appearance: bool,
    /// The host can independently show or hide toolbar components. Linux
    /// expresses this through the IBus property menu even though it cannot
    /// draw the Windows/macOS floating window.
    pub floating_toolbar_components: bool,
    /// The toolbar carries a button that opens the handwriting panel. The
    /// reference's toolbar has six components and this is not one of them, so
    /// only the host that draws the button offers the switch for it.
    pub floating_toolbar_handwriting: bool,
    /// The toolbar carries a button that starts and stops voice input, for the
    /// same reason as `floating_toolbar_handwriting`.
    pub floating_toolbar_voice: bool,
    /// 工具栏带切换输入方案的按钮。目前只有 macOS 的工具栏画它，其它宿主不提供这个开关。
    pub floating_toolbar_input_scheme: bool,
    /// The host consumes the shared `keybindings` preferences to switch
    /// Chinese/English and simplified/traditional mode.
    pub mode_switch_shortcuts: bool,
    /// The desktop environment forwards a shortcut that opens a shared panel.
    pub panel_shortcuts: bool,
    /// The host can let the user release number-row candidate selection back
    /// to the focused application.
    pub number_row_selection: bool,
    /// The host can enumerate audio capture devices for voice input.
    pub voice_capture_devices: bool,
    /// The host can apply candidate font family, fallback family and size preferences.
    pub candidate_font_controls: bool,
    /// The composition drawn beside the candidates has its own size. Linux reads the family and
    /// candidate size into the desktop panel's single font, but the composition itself is drawn by
    /// the focused application, so a separate preedit size would have nothing to change there.
    pub candidate_preedit_font: bool,
    /// The host can hide the candidate panel's page indicator without changing pagination.
    pub candidate_page_number: bool,
    /// 宿主按 `show_app_logo` 显示或隐藏候选窗和悬浮工具栏左端的水杉 logo。
    pub app_logo: bool,
    /// The host can apply candidate foreground/background RGB row colors.
    /// Linux exposes these through IBusText attributes even though it cannot
    /// draw the native card geometry or hover state.
    pub candidate_row_colors: bool,
    /// The host can apply candidate accent, selection, hover and border appearance.
    pub candidate_selection_appearance: bool,
    /// The host outlines the candidate panel in the border colour. Separate from `candidate_selection_appearance` because Linux draws the border (the Fcitx5 classic UI theme carries it) while neither Linux panel has a hover state.
    pub candidate_border_color: bool,
    /// The host draws its own floating candidate window and multiplies its font and geometry by `candidate_scale_percent`. A host whose list lives in a desktop panel, or in a strip on the keyboard that already follows the font size, has nothing else to scale.
    pub candidate_window_scale: bool,
    /// The host can lower the alpha of its candidate card fill, border and skin background by `candidate_opacity_percent` while keeping text opaque. A panel the desktop draws, or a strip that is part of an opaque keyboard, cannot.
    pub candidate_window_opacity: bool,
    /// The host rounds its candidate card by `candidate_corner_radius`, ahead of the skin package's radius and its own constant.
    pub candidate_corner_radius: bool,
    /// The host places its own candidate window and can therefore pin it where
    /// it first appeared. A host whose desktop owns the placement - IBus draws
    /// and positions the candidate list itself - cannot honour the choice, so
    /// it does not offer it.
    pub candidate_follow_cursor: bool,
    /// The host has more than one way to put a recognized result into the
    /// focused editor, so choosing between them is a real choice. A host with a
    /// single commit path does not offer it: a control with one outcome reads
    /// as a setting that is being ignored.
    pub voice_commit_mode: bool,
    /// The host renders the Engine's composition text itself, so the choice
    /// between the raw shuangpin keys and the expanded pinyin is visible there.
    /// Every host's Engine honours the preference; this says which of them draw
    /// the result where a user would see the difference. A host that hands the
    /// snapshot's `preedit` to a desktop panel still decides which string goes
    /// there, so the difference is its to show.
    pub shuangpin_preedit: bool,
    /// The host tells the runtime which character width it is in, so the Engine
    /// widens what it commits. The preference is the width a session starts at;
    /// the host's own toolbar, menu or chord moves it from there. A host that
    /// never makes that call cannot honour the preference at all, and offering
    /// the switch there would be a control with nothing behind it.
    pub character_width: bool,
    /// The host runs the configured transcription provider itself, so the provider, model and
    /// credential controls have something behind them.
    ///
    /// Was a platform name on the settings page, and it read `!android` because that host once had
    /// only the platform recogniser. It runs the configured provider now — the OpenAI-compatible
    /// uploads and the streaming socket both — and a page keyed on the name would still be hiding
    /// the controls.
    /// The host routes the Ctrl+Shift+Alt maintenance chords: delete the candidate in a numbered
    /// slot, and drop the cached candidate list.
    ///
    /// Touch reaches both by gesture — a long press on the candidate, and nothing at all for the
    /// cache — so a keyboard needs the chords or cannot reach them. Declared rather than inferred
    /// from "draws desktop panels", which is what it used to be read off and is a different fact.
    pub maintenance_shortcuts: bool,
    /// The host reserves the Option/Alt+Shift+H chord for the character width, so the switch that
    /// gives it back to the application belongs on its settings page.
    pub fullwidth_chord: bool,
    pub voice_provider_settings: bool,
    /// The host draws the recogniser's interim text while the user is still speaking.
    ///
    /// Every host can ask a streaming provider for partial results; this says which of them has
    /// somewhere to put one. A host without that surface would be offering a switch whose only
    /// effect is on a display it does not have.
    pub voice_stream_preedit: bool,
    /// The host shows read-only English word completions while typing directly
    /// in English, governed by the shared `english_suggestions` preference. iOS
    /// offers the same surface but keeps its switch in the native App Group
    /// store, so it reads this as false and shows its own control.
    pub english_suggestions: bool,
    /// A letter becomes a helper code because the user held Shift for it, rather
    /// than because of where it sits in the spelling. Windows appends helper
    /// codes directly to a finished pinyin and needs no gesture; a keyboard host
    /// does, or the letter would be eaten as more pinyin. The hosts that mark
    /// them this way are the ones running the ported ChineseHelpcodePolicy, and
    /// the settings page explains the gesture only where it applies.
    pub helpcode_shift_entry: bool,
    /// A skin arrives by being picked rather than by being dropped into a
    /// folder. The source opens its skin folder so the user can put one there;
    /// a host whose folder is inside an application sandbox has nothing to
    /// open, so it asks the user to point at the skin instead. The page needs
    /// to know which of the two it is, because the button says so.
    pub skin_directory_import: bool,
    /// The one candidate page size the host draws, when it offers no choice. The iOS keyboard numbers its strip's chips 1-9 to match the digits on its symbol layer and lays the expanded panel out in nines, so it holds the Engine to nine whatever the shared setting says; the page shows the count instead of a selector that would do nothing. Absent on a host that pages by the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_candidate_page_size: Option<u8>,
    /// The one candidate layout the host draws, when it offers no choice. The iOS candidate strip is a horizontal row above the keys, so an external skin is adopted there only for its horizontal layout; the skin page has to judge compatibility by that rather than by the shared setting, which defaults to vertical. Absent on a host that follows the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_candidate_layout: Option<crate::preferences::CandidateLayout>,
    /// The touch keyboard reads `touch_toolbar` to choose the buttons on the row above its keys. iOS 与 Android 的键盘读它；其他宿主上这些开关什么也不会隐藏。
    pub touch_toolbar_components: bool,
    /// The host applies a separate family for Latin text in the candidate panel.
    /// A host whose renderer resolves one family list per glyph, or which draws
    /// Latin from its own font, can honour this; one with a single typeface for
    /// the whole row cannot, and does not offer the choice.
    pub candidate_english_font: bool,
    /// The AI service's credential lives with the host's provider rather than in
    /// the settings document, so the settings page must not ask for a token and
    /// must not gate the service controls on having one. The host still reaches
    /// the service - through that provider - so the model listing and the polish
    /// test are offered; what it cannot do is hold the secret.
    pub ai_provider_credentials: bool,
    /// The host draws a short, non-activating badge near the caret after the
    /// Chinese/English mode changes. A host with no way to put a window beside
    /// the caret, or one whose keyboard already shows the mode on its own key
    /// faces, has nothing to switch on and does not offer the choice.
    pub input_mode_hud: bool,
    /// The host can run a 背单词 review session — that is, it has wired the shared vocabulary
    /// entry point and can reach the review store.
    pub vocabulary_review: bool,
    /// 背单词书目里列出单词本插件（`pack-<插件 id>` 词书）：宿主把插件目录交给背单词的入口。
    pub wordbook_packs: bool,
    /// 宿主的符号面板显示已安装的符号集插件。没打开时插件详情说明本机的符号面板不显示插件符号集。
    pub symbol_set_packs: bool,
    /// The host plays the sound packs in `plugins`: a sample per key class, the melody, the commit sound and the achievement jingle. Only an input process that sees the keys can, and only where it has somewhere to play them; a host without the player keeps the settings but offers no switches for them.
    pub key_sound: bool,
    /// The host routes the `/` command and `@` mention modes: it hands `/` and `@` to the runtime, stops treating digits as candidate numbers while a mode spells with them, and loads the enabled command tables and the name list into the Engine. The `V` mode needs only the digit routing and is covered by the same flag.
    pub plugin_triggers: bool,
    /// The host streams the selected music pack while it is the active input method.
    pub music: bool,
    /// The host draws the typing effects and the combo count that `msime_client_typing_effect` answers with. Each host flips this only in the change that wires the call, as with the flags above.
    pub typing_effects: bool,
    /// The operating system release, as the machine reports it, for the feedback
    /// page to attach. Not a platform assumption like the flags above -- the host
    /// fills it in after `for_platform`, the way `system_fonts` is filled in --
    /// so a host with no cheap way to read it simply leaves it out and the page
    /// falls back to what the web view knows about itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// The CPU architecture the host was built for, as Rust names it (`std::env::consts::ARCH`: `x86_64`, `aarch64`). A Linux release carries one package per architecture, and the update check picks this machine's by it. Filled in at runtime like `os_version`; absent from a host that does not report it, where the check offers a package only when the release has a single one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Why the desktop's candidate panel on this machine ignores the candidate font, colour and skin settings, when the running Linux host has found that it does. Filled in at runtime from what the host reports, the way `os_version` is; absent when the panel honours them or nothing has been reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_panel_limit: Option<CandidatePanelLimit>,
    /// The input schemes this host offers; the settings page shows the others disabled. A host may narrow the list at runtime the way it fills `os_version`, for instance when the Cantonese, Zhuyin or Stroke dictionary is not installed.
    ///
    /// 不是 full 的版本还会经 [`HostCapabilities::narrow_to_edition`] 去掉本版本不含的方案；那些方案在本版本里不存在，设置页应该直接不列出，而不是显示为禁用，`edition` 就是用来区分这两种情况的。
    pub input_schemes: Vec<InputScheme>,
    /// 运行中的版本，不是 full 时才有。缺省（包括引入版本之前的宿主）就是 full：所有方案都属于本版本，`input_schemes` 之外的方案只是这个宿主暂不支持，显示为禁用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<EditionInfo>,
    /// 输入页辅助码设置可选的内置辅助码方案（`HelpcodeSchema` 的 id），按设置页列出的顺序。目前只有 Android 宿主列出，其他宿主为空、不写进文档，序列化结果与加这一项之前相同。郑码只在有带授权的内置码表（按 `resources/helpcodes/NOTICE.md` 登记）时列入，现在没有。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub helpcode_schemas: Vec<String>,
}

/// Android 输入页列出的内置辅助码方案。不含郑码：仓库里还没有带授权的郑码码表。
const ANDROID_HELPCODE_SCHEMAS: [&str; 6] = [
    "ziranma",
    "xiaohe",
    "lantian",
    "shouyou2_0",
    "shouyouplus",
    "jiajia",
];

/// 设置页需要知道的版本信息。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditionInfo {
    /// 版本 id。
    pub id: String,
    /// 版本的中文产品名（版本表 `display_name.zh-Hans`），例如「水杉五笔」。macOS 设置页用它称呼本版本在输入法菜单里的各个入口。
    pub display_name: String,
    /// 本版本提供的方案；不在其中的方案在本版本里不存在。
    pub input_schemes: Vec<InputScheme>,
    /// 本版本的默认方案，偏好里的方案不可用时回退到它。
    pub default_scheme: InputScheme,
    /// 本版本是否带临时日文。不带时设置页不列出临时日语开关，host-api 也始终把它关掉。
    pub temporary_japanese: bool,
    /// 本版本是否带键盘神经联想用的模型（`sentence-model.safetensors`）。不带时设置页不列出触屏宿主的神经联想开关，host-api 也始终把它关掉。桌面的神经联想用资源目录旁的 settled 模型，不归这一项管。
    pub neural_keyboard: bool,
    /// 本版本是否带非英文目标语言的离线候选释义。不带时宿主不打包这些数据库，候选翻译的非英文目标只靠联网服务。
    pub offline_glosses: bool,
    /// 本版本是否提供手写。不提供时设置页不列出手写设置页和悬浮工具栏的手写按钮，宿主也不打开手写面板、不下载手写模型。
    pub handwriting: bool,
    /// 本版本里五笔混拼的默认值，偏好文档缺这一项时设置页按它显示。
    pub wubi_mixed_pinyin_default: bool,
}

/// 基础方案加上粤拼、注音、越南文、藏文和笔画，所有宿主都提供。
const ALL_INPUT_SCHEMES: [InputScheme; 10] = [
    InputScheme::Quanpin,
    InputScheme::Shuangpin,
    InputScheme::Wubi,
    InputScheme::Japanese,
    InputScheme::Korean,
    InputScheme::Cantonese,
    InputScheme::Zhuyin,
    InputScheme::Vietnamese,
    InputScheme::Tibetan,
    InputScheme::Stroke,
];

/// 本构建交给 Engine 的方案：所有宿主都是全部十个，因为每个宿主都路由粤拼、注音、越南文、藏文和笔画的按键，并放置粤拼、注音和笔画的词库（越南文和藏文不需要词库）。偏好文档里写的其他方案由 host-api 回退；粤拼、注音和笔画在词库没装时仍然回退。
pub fn compiled_input_schemes() -> &'static [InputScheme] {
    &ALL_INPUT_SCHEMES
}

/// `edition` 交给 Engine 的方案：[`compiled_input_schemes`] 里本版本提供的那些，顺序不变。full 得到的就是全部八个。host-api 对偏好里其他的方案一律回退到本版本的方案，所以在不是 full 的版本里，任何偏好文档都不会让 Engine 跑一个本版本不含的方案。
pub fn offered_input_schemes(edition: &Edition) -> Vec<InputScheme> {
    ALL_INPUT_SCHEMES
        .into_iter()
        .filter(|scheme| edition.offers(*scheme))
        .collect()
}

impl HostCapabilities {
    /// Capabilities as they stand today for each shipped host. Slices that add a
    /// capability to a host flip its flag here, and every consumer follows.
    pub fn for_platform(platform: HostPlatform) -> Self {
        HostCapabilities {
            platform,
            mobile_settings: !platform.is_desktop(),
            // Linux restarts IBus; Windows sends a request to the supervised
            // native Server over its session-less auxiliary pipe; macOS starts
            // a fresh bundle instance with --reregister-input-source so
            // InputMethodKit can discover and enable the current source.
            restart_input_method: matches!(
                platform,
                HostPlatform::Windows | HostPlatform::Linux | HostPlatform::Macos
            ),
            panel_windows: platform.is_desktop(),
            // IBus keeps a session-wide mode. Windows keeps a cross-application
            // CN/EN authority, while macOS switches between its per-application
            // map and a process-wide authority when a client activates.
            // The HarmonyOS keyboard learns which application an editor belongs to from the
            // editor attribute's bundle name, so it can keep the same per-application map the
            // desktop hosts do. A touch host that cannot name the editor's application has nothing
            // to key one on and keeps a single mode.
            ime_mode_scope: matches!(
                platform,
                HostPlatform::Linux
                    | HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Android
                    | HostPlatform::Harmony
            ),
            typing_statistics: true,
            // Every input host consumes the shared fuzzy-pinyin options. The
            // iOS Tauri settings surface writes the same PreferencesStore that
            // the keyboard extension reloads before applying its session.
            fuzzy_pinyin: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Linux
                    | HostPlatform::Macos
                    | HostPlatform::Android
                    | HostPlatform::Ios
                    | HostPlatform::Harmony
            ),
            // ArkUI hands the installed family names straight to the settings page, so the font
            // inputs can offer a list there rather than asking for an exact name to be typed.
            system_fonts: platform.is_desktop() || platform == HostPlatform::Harmony,
            window_chrome: platform.is_desktop(),
            // A 2in1 draws one from the input method's own status-bar panel, which needs none of
            // the window permissions a desktop floating window would. A HarmonyOS phone has no use
            // for one: the surfaces are on the keyboard's own key faces there.
            floating_toolbar: platform.is_desktop() || platform == HostPlatform::Harmony,
            // macOS FloatingToolbarPanel.mm and the Windows FloatingToolbarWindow
            // both read scale_percent and font_size; the Linux host has no
            // equivalent surface for those two values. The HarmonyOS panel
            // scales its own frame by the former and draws its faces at the
            // latter.
            floating_toolbar_appearance: matches!(
                platform,
                HostPlatform::Windows | HostPlatform::Macos | HostPlatform::Harmony
            ),
            // Linux maps the component switches to IBus menu entries; the
            // native-window hosts apply them to their own toolbar buttons. The
            // HarmonyOS panel hides the button and narrows itself, and its
            // emoji and screen-keyboard buttons open the same surfaces its
            // phone keyboard reaches from a key face.
            floating_toolbar_components: platform.is_desktop() || platform == HostPlatform::Harmony,
            // Only this client's macOS toolbar draws these two.
            floating_toolbar_handwriting: platform == HostPlatform::Macos,
            floating_toolbar_voice: platform == HostPlatform::Macos,
            floating_toolbar_input_scheme: platform == HostPlatform::Macos,
            // The IBus host consumes these directly, and the Windows TIP reads them from the shared preferences document at activation. The HarmonyOS host reads all four in its hardware key router, which only a machine with a physical keyboard has anything to route.
            mode_switch_shortcuts: matches!(
                platform,
                HostPlatform::Linux
                    | HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Android
                    | HostPlatform::Harmony
            ),
            // Windows handles Ctrl+Shift+Win+K on its maintenance hook; Linux
            // uses the current IBus context, and macOS uses the current IMK
            // context with Command in place of the Windows/Super modifier. A
            // HarmonyOS keyboard extension has no global hook and sees keys only
            // while attached to an editor, which turns out to be the wrong
            // reason to withhold this: the panel inserts into the focused
            // editor, so an editor is the precondition for it being useful at
            // all rather than a restriction on when the chord may fire.
            panel_shortcuts: matches!(
                platform,
                HostPlatform::Linux
                    | HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
            ),
            // Harmony 2-in-1 hardware keyboards use the same candidate number row as Windows: the ArkTS router picks with 1 through 9 while this preference is on and gives the digits to the focused editor once it is turned off.
            number_row_selection: matches!(
                platform,
                HostPlatform::Linux | HostPlatform::Android | HostPlatform::Harmony
            ),
            // HarmonyOS records through its own AudioCapturer for the HTTP and Doubao providers, so
            // the routing manager's input devices are both enumerable and selectable there. The
            // system speech recognizer keeps its audio inside the service and is unaffected either
            // way; nothing else on this host owns a microphone.
            voice_capture_devices: matches!(
                platform,
                HostPlatform::Linux
                    | HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
            ),
            // Native Windows/macOS candidate windows consume the shared font
            // controls. Harmony's desktop candidate panel and Android's and
            // iOS's native candidate bars also apply the family chain and both
            // candidate/preedit sizes. Linux writes the family chain and
            // candidate size into the panel's font: the IBus panel settings or
            // the Fcitx5 classic UI.
            candidate_font_controls: true,
            candidate_page_number: matches!(platform, HostPlatform::Linux),
            // 目前只有 macOS 按这个开关画 logo；Windows、Linux 和鸿蒙的候选窗照常显示 logo，不提供这个开关。
            app_logo: platform == HostPlatform::Macos,
            // The iOS strip scales its composition line by `candidate_preedit_font_size`.
            candidate_preedit_font: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Android
                    | HostPlatform::Ios
            ),
            // IBus exposes candidate and label foreground/background RGB
            // attributes, but not native hover state or card borders. The iOS strip resolves every candidate colour once the keyboard's 「候选栏使用主题配色」 switch is on, which the shared skin page now carries.
            candidate_row_colors: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Linux
                    | HostPlatform::Harmony
                    | HostPlatform::Android
                    | HostPlatform::Ios
            ),
            candidate_selection_appearance: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Android
                    | HostPlatform::Ios
            ),
            candidate_border_color: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Android
                    | HostPlatform::Ios
                    | HostPlatform::Linux
            ),
            // The Windows and macOS candidate windows are drawn by the host, so every style control reaches them. The Fcitx5 classic UI theme carries a corner radius but no scale, and on X11 without a compositor a translucent fill shows as black, so Linux offers only the radius. The HarmonyOS 2in1 candidate card takes a radius and a fill alpha but its size follows the font size alone. The iOS and Android candidate strips sit on the keyboard rather than float, so none of the three applies there.
            candidate_window_scale: matches!(platform, HostPlatform::Windows | HostPlatform::Macos),
            candidate_window_opacity: matches!(
                platform,
                HostPlatform::Windows | HostPlatform::Macos | HostPlatform::Harmony
            ),
            candidate_corner_radius: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Linux
                    | HostPlatform::Harmony
            ),
            // macOS CandidatePanel and the HarmonyOS candidate panel track the current insertion
            // rect themselves; expose the shared toggle on both hosts.
            candidate_follow_cursor: matches!(
                platform,
                HostPlatform::Windows | HostPlatform::Macos | HostPlatform::Harmony
            ),
            // macOS shows it from a non-activating panel. Harmony shows the same badge from a
            // status-bar panel on a 2in1, which is the one form factor with a hardware keyboard and
            // therefore the one where nothing else on screen says the mode changed; its phone
            // keyboard says so on its own key faces and needs no badge. Linux does not draw the
            // badge itself either - the Fcitx5 panel offers exactly this popup for "input method
            // that has internal switches", which is what the Chinese/English mode is, so the host
            // asks the panel rather than placing a window of its own.
            input_mode_hud: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Harmony | HostPlatform::Linux
            ),
            // macOS draws its own composition, and the HarmonyOS keyboard draws the Engine's editing text on its composition row, so both show the difference. The other hosts hand the text to the application or to the desktop, which decides how it looks.
            // Windows chooses between TSF, SendInput and a paste; macOS between system events and its input session. The Linux hosts commit through the IBus or Fcitx5 input context, the desktop voice panel hands its text to the active host the way every panel does, and the voice provider only recognizes, so a stored mode would change nothing there. A keyboard extension commits through its input client and has nothing to choose between either.
            voice_commit_mode: matches!(platform, HostPlatform::Windows | HostPlatform::Macos),
            // The Linux hosts write the snapshot's `preedit` into the IBus and
            // Fcitx5 preedit themselves, and both already carry their own
            // toggle for this in the native status menu - a setting the shared
            // page was hiding could only be reached from there, and only while
            // the shuangpin scheme was active.
            shuangpin_preedit: matches!(
                platform,
                HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Linux
                    | HostPlatform::Ios
            ),
            // Every host calls `msime_client_set_character_width` when its session starts and
            // from its own width switch, so the preference always has something to act on.
            character_width: true,
            // The Android recognition window shows the transcript once it is settled and has no row for a partial one; putting half-written text there that the final result may contradict is worse than waiting for it. iOS records in the app, because a keyboard extension cannot use the microphone, and hands the keyboard only the final transcript, so it has no row for one either. Every other host draws its own composition.
            // Every host reaches its provider one way or another: the desktops and both mobile
            // hosts call it themselves, and Linux hands the same configuration to its provider
            // service. None of them wants these controls hidden.
            // Harmony reaches this through its form-factor projection of `panel_windows`, which
            // the page still consults, so it is not named here and its behaviour is unchanged.
            maintenance_shortcuts: platform.is_desktop() || platform == HostPlatform::Android,
            // macOS has reserved it since it shipped; the Android host reads the same preference
            // for its own Alt+Shift+H. No other host binds that chord.
            fullwidth_chord: matches!(platform, HostPlatform::Macos | HostPlatform::Android),
            voice_provider_settings: true,
            voice_stream_preedit: !matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            english_suggestions: matches!(platform, HostPlatform::Android | HostPlatform::Harmony),
            // Android, HarmonyOS and the iOS keyboard extension share one gesture: Shift during a quanpin or shuangpin composition hands the next letter to the Engine as a helper code. The desktop hosts append the code to a finished spelling instead of marking it.
            helpcode_shift_entry: matches!(
                platform,
                HostPlatform::Android | HostPlatform::Harmony | HostPlatform::Ios
            ),
            // The skin folder is inside the sandbox on HarmonyOS and in the App Group container on iOS, where no file manager reaches it, so the skin is picked and copied in instead.
            skin_directory_import: matches!(platform, HostPlatform::Harmony | HostPlatform::Ios),
            fixed_candidate_page_size: (platform == HostPlatform::Ios).then_some(9),
            fixed_candidate_layout: (platform == HostPlatform::Ios)
                .then_some(crate::preferences::CandidateLayout::Horizontal),
            // The iOS shortcut bar is the touch counterpart of the Windows floating toolbar, and its buttons follow the same kind of per-component switches. Android 的原生键盘工具栏也按 `touch_toolbar` 选按钮。
            touch_toolbar_components: matches!(platform, HostPlatform::Ios | HostPlatform::Android),
            // Linux keeps AI credentials in the provider service's owner-only
            // configuration file and passes only non-sensitive options over its
            // socket. Every other host holds the token itself.
            ai_provider_credentials: platform == HostPlatform::Linux,
            // Windows draws Latin from its own family, macOS and Android name it ahead of the primary one, and ArkUI resolves a family list per glyph, so HarmonyOS reaches the same result the same way. Both Linux hosts write one Pango font description for the desktop panel, and Pango resolves its family list per glyph too, so they name it first there.
            candidate_english_font: true,
            // Every host reaches the same shared store through the same entry point, so there is no platform here that can and one that cannot.
            vocabulary_review: true,
            // 桌面宿主的背单词由 Tauri 层传入插件目录；HarmonyOS 在自己的设置投影里按形态打开；Android 和 iOS 不传插件目录。
            wordbook_packs: platform.is_desktop(),
            // Windows 和 Linux 桌面的符号面板是 Tauri 层的表情面板（`load_emoji_catalog`），Linux 的 Fcitx5 菜单和 macOS 的原生表情与符号面板另外读同一批插件组。HarmonyOS 在自己的设置投影里按形态打开；Android 和 iOS 没有接入。
            symbol_set_packs: platform.is_desktop(),
            // The three desktop hosts play the packs, route V, / and @ by the Engine's spelling symbols and stream music while they are the active input method. HarmonyOS claims key sounds, music and the triggers per form factor in its own settings projection (2in1 only); the phone and tablet hosts wire none of them. A switch with nothing behind it reads as a setting being ignored, so each host flips here only in the change that wires it.
            // Android 的按键音由 IME 进程的 SoundPool 播放（与本项同一波接入），所以 Android 也声明；手机和平板上的 HarmonyOS 与 iOS 仍未接入。
            key_sound: platform.is_desktop() || platform == HostPlatform::Android,
            plugin_triggers: platform.is_desktop(),
            music: platform.is_desktop(),
            // macOS draws the sparks, the card flash and the combo badge (TypingEffectPanel.mm), Windows the flash and the badge on its candidate window (CandidateWindow.cpp), both Linux hosts the combo count in the candidate aux line (KeySound.h), and HarmonyOS the flash and the combo badge on its KeyboardView. Linux draws no style, only the count; the settings page hides the style controls there itself (`showTypingEffectStyles`). HarmonyOS still narrows this per form factor in its own settings projection; Android and iOS wire none.
            typing_effects: platform.is_desktop() || platform == HostPlatform::Harmony,
            os_version: None,
            arch: None,
            candidate_panel_limit: None,
            // 每个宿主都路由粤拼、注音、越南文、藏文和笔画的按键，并附带粤拼、注音和笔画需要的词库。
            input_schemes: ALL_INPUT_SCHEMES.to_vec(),
            edition: None,
            helpcode_schemas: if platform == HostPlatform::Android {
                ANDROID_HELPCODE_SCHEMAS
                    .iter()
                    .map(|schema| (*schema).to_owned())
                    .collect()
            } else {
                Vec::new()
            },
        }
    }

    /// 收窄到 `edition`：去掉本版本不含的方案，并在不是 full 时带上版本信息。对 full 什么也不改，序列化结果与引入版本之前相同。
    pub fn narrow_to_edition(&mut self, edition: &Edition) {
        if edition.is_full() {
            return;
        }
        self.input_schemes.retain(|scheme| edition.offers(*scheme));
        // 手写面板只认汉字，不提供手写的版本（日文、越南文和藏文版）在悬浮工具栏上也不放手写按钮。
        self.floating_toolbar_handwriting &= edition.features.handwriting;
        self.edition = Some(EditionInfo {
            id: edition.id.clone(),
            display_name: edition.display_name.zh_hans.clone(),
            input_schemes: offered_input_schemes(edition),
            default_scheme: edition.default_scheme,
            temporary_japanese: edition.features.temporary_japanese,
            neural_keyboard: edition.features.neural_keyboard,
            offline_glosses: edition.features.offline_glosses,
            handwriting: edition.features.handwriting,
            wubi_mixed_pinyin_default: edition
                .preference_defaults
                .wubi_mixed_pinyin
                .unwrap_or(crate::preferences::Preferences::default().wubi_mixed_pinyin),
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsCategory {
    /// The account page, the AI conversation and the community browser are pages of the same shared
    /// settings surface as the rest. They were missing from this list, so no host could route to them and
    /// macOS opened its own account window instead of the page the other platforms show.
    Account,
    Chat,
    Community,
    Appearance,
    Input,
    /// 标点与翻译.
    Expression,
    TypingStatistics,
    Shortcuts,
    Dictionary,
    /// 背单词. Next to the dictionary because both are word lists the user manages, and away from
    /// the input pages because nothing on it changes how typing behaves.
    Vocabulary,
    Skin,
    ScreenKeyboard,
    Handwriting,
    Voice,
    /// AI 辅助, a page of the 工具 group and the parent of the AI conversation page.
    Ai,
    Tools,
    /// 插件: sound packs, background music, command tables and the @ name list.
    Plugins,
    FloatingToolbar,
    /// 开发者选项, which holds the local MCP server.
    Developer,
    Help,
    About,
    Feedback,
}

impl SettingsCategory {
    /// Matches the category identifiers the shared settings page renders.
    pub fn as_str(self) -> &'static str {
        match self {
            SettingsCategory::Account => "account",
            SettingsCategory::Chat => "chat",
            SettingsCategory::Community => "community",
            SettingsCategory::Appearance => "appearance",
            SettingsCategory::Input => "input",
            SettingsCategory::Expression => "expression",
            SettingsCategory::TypingStatistics => "typing-statistics",
            SettingsCategory::Shortcuts => "shortcuts",
            SettingsCategory::Dictionary => "dictionary",
            SettingsCategory::Vocabulary => "vocabulary",
            SettingsCategory::Skin => "skin",
            SettingsCategory::ScreenKeyboard => "screen-keyboard",
            SettingsCategory::Handwriting => "handwriting",
            SettingsCategory::Voice => "voice",
            SettingsCategory::Ai => "ai",
            SettingsCategory::Tools => "tools",
            SettingsCategory::Plugins => "plugins",
            SettingsCategory::FloatingToolbar => "floating-toolbar",
            SettingsCategory::Developer => "developer",
            SettingsCategory::Help => "help",
            SettingsCategory::About => "about",
            SettingsCategory::Feedback => "feedback",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RouteError> {
        match value {
            "account" => Ok(SettingsCategory::Account),
            "chat" => Ok(SettingsCategory::Chat),
            "community" => Ok(SettingsCategory::Community),
            "appearance" => Ok(SettingsCategory::Appearance),
            "input" => Ok(SettingsCategory::Input),
            "expression" => Ok(SettingsCategory::Expression),
            "typing-statistics" => Ok(SettingsCategory::TypingStatistics),
            "shortcuts" => Ok(SettingsCategory::Shortcuts),
            "dictionary" => Ok(SettingsCategory::Dictionary),
            "vocabulary" => Ok(SettingsCategory::Vocabulary),
            "skin" => Ok(SettingsCategory::Skin),
            "screen-keyboard" => Ok(SettingsCategory::ScreenKeyboard),
            "handwriting" => Ok(SettingsCategory::Handwriting),
            "voice" => Ok(SettingsCategory::Voice),
            "ai" => Ok(SettingsCategory::Ai),
            "tools" => Ok(SettingsCategory::Tools),
            "plugins" => Ok(SettingsCategory::Plugins),
            "floating-toolbar" => Ok(SettingsCategory::FloatingToolbar),
            "developer" => Ok(SettingsCategory::Developer),
            "help" => Ok(SettingsCategory::Help),
            "about" => Ok(SettingsCategory::About),
            "feedback" => Ok(SettingsCategory::Feedback),
            "" => Err(RouteError::Empty),
            _ => Err(RouteError::Unknown),
        }
    }

    pub const ALL: [SettingsCategory; 22] = [
        SettingsCategory::Account,
        SettingsCategory::Chat,
        SettingsCategory::Community,
        SettingsCategory::Appearance,
        SettingsCategory::Input,
        SettingsCategory::Expression,
        SettingsCategory::TypingStatistics,
        SettingsCategory::Shortcuts,
        SettingsCategory::Dictionary,
        SettingsCategory::Vocabulary,
        SettingsCategory::Skin,
        SettingsCategory::ScreenKeyboard,
        SettingsCategory::Handwriting,
        SettingsCategory::Voice,
        SettingsCategory::Ai,
        SettingsCategory::Tools,
        SettingsCategory::Plugins,
        SettingsCategory::FloatingToolbar,
        SettingsCategory::Developer,
        SettingsCategory::Help,
        SettingsCategory::About,
        SettingsCategory::Feedback,
    ];
}

/// A surface a platform host can ask the shared shell to present.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "surface", content = "category")]
pub enum SurfaceRoute {
    Settings(Option<SettingsCategory>),
    Emoji,
    Keyboard,
    Handwriting,
    Voice,
    Clipboard,
    CloudClipboard,
    CloudDictionary,
}

/// Where a panel window opens on the work area, for hosts that place panels themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelPlacement {
    /// Centred horizontally, 12 pixels above the bottom of the work area.
    BottomCenter,
    /// Centred on the work area.
    Center,
}

/// Geometry and identity of a panel surface, so the window size lives beside the
/// route instead of being repeated per host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelSurface {
    pub label: &'static str,
    pub query: &'static str,
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
    pub placement: PanelPlacement,
}

impl SurfaceRoute {
    /// Canonical route string. Round-trips through [`SurfaceRoute::parse`].
    pub fn as_arg(self) -> String {
        match self {
            SurfaceRoute::Settings(None) => "settings".to_string(),
            SurfaceRoute::Settings(Some(category)) => format!("settings:{}", category.as_str()),
            SurfaceRoute::Emoji => "emoji".to_string(),
            SurfaceRoute::Keyboard => "keyboard".to_string(),
            SurfaceRoute::Handwriting => "handwriting".to_string(),
            SurfaceRoute::Voice => "voice".to_string(),
            SurfaceRoute::Clipboard => "clipboard".to_string(),
            SurfaceRoute::CloudClipboard => "cloud-clipboard".to_string(),
            SurfaceRoute::CloudDictionary => "cloud-dictionary".to_string(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, RouteError> {
        if value.is_empty() {
            return Err(RouteError::Empty);
        }
        if value.len() > MAX_ROUTE_BYTES {
            return Err(RouteError::TooLong);
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b':'
        }) {
            return Err(RouteError::IllegalCharacter);
        }
        let mut parts = value.splitn(2, ':');
        let head = parts.next().unwrap_or_default();
        let tail = parts.next();
        if let Some(tail) = tail {
            // Only settings takes a sub-route, and it must name a real category.
            if head != "settings" || tail.contains(':') {
                return Err(RouteError::Unknown);
            }
            return Ok(SurfaceRoute::Settings(Some(SettingsCategory::parse(tail)?)));
        }
        match head {
            "settings" => Ok(SurfaceRoute::Settings(None)),
            "emoji" => Ok(SurfaceRoute::Emoji),
            "keyboard" => Ok(SurfaceRoute::Keyboard),
            "handwriting" => Ok(SurfaceRoute::Handwriting),
            "voice" => Ok(SurfaceRoute::Voice),
            "clipboard" => Ok(SurfaceRoute::Clipboard),
            "cloud-clipboard" => Ok(SurfaceRoute::CloudClipboard),
            "cloud-dictionary" => Ok(SurfaceRoute::CloudDictionary),
            _ => Err(RouteError::Unknown),
        }
    }

    /// The settings section this route names, if it names one. Hosts use it to
    /// open the settings window directly on the section a menu entry asked for.
    pub fn settings_category(self) -> Option<SettingsCategory> {
        match self {
            SurfaceRoute::Settings(category) => category,
            _ => None,
        }
    }

    /// The panel window this route opens, or `None` when the route targets the
    /// main settings window.
    pub fn panel(self) -> Option<PanelSurface> {
        match self {
            SurfaceRoute::Settings(_) => None,
            SurfaceRoute::Keyboard => Some(PanelSurface {
                label: "keyboard-panel",
                query: "keyboard",
                title: "水杉屏幕键盘",
                width: 1100,
                height: 400,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::Handwriting => Some(PanelSurface {
                label: "handwriting-panel",
                query: "handwriting",
                title: "水杉手写识别板",
                width: 980,
                height: 650,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::Emoji => Some(PanelSurface {
                label: "emoji-panel",
                query: "emoji",
                title: "Emoji and more",
                width: 720,
                height: 720,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::Voice => Some(PanelSurface {
                label: "voice-panel",
                query: "voice",
                title: "水杉语音输入",
                width: 620,
                height: 520,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::Clipboard => Some(PanelSurface {
                label: "clipboard-panel",
                query: "clipboard",
                title: "水杉本地剪贴板",
                width: 560,
                height: 620,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::CloudClipboard => Some(PanelSurface {
                label: "cloud-clipboard-panel",
                query: "cloud-clipboard",
                title: "水杉云剪贴板",
                width: 560,
                height: 560,
                placement: PanelPlacement::BottomCenter,
            }),
            SurfaceRoute::CloudDictionary => Some(PanelSurface {
                label: "cloud-dictionary-panel",
                query: "cloud-dictionary",
                title: "水杉云词库",
                width: 760,
                height: 700,
                placement: PanelPlacement::BottomCenter,
            }),
        }
    }

    /// The panel window this route opens on the given host. Every host opens [`SurfaceRoute::panel`] except Windows, which follows the shipped native panels: the emoji panel opens at 550 by 610 and the handwriting panel at its shared size, both centred on the work area, while the keyboard and the rest stay bottom-centred.
    pub fn panel_for(self, platform: HostPlatform) -> Option<PanelSurface> {
        let panel = self.panel()?;
        if platform != HostPlatform::Windows {
            return Some(panel);
        }
        Some(match self {
            SurfaceRoute::Emoji => PanelSurface {
                width: 550,
                height: 610,
                placement: PanelPlacement::Center,
                ..panel
            },
            SurfaceRoute::Handwriting => PanelSurface {
                placement: PanelPlacement::Center,
                ..panel
            },
            _ => panel,
        })
    }

    pub const ALL: [SurfaceRoute; 8] = [
        SurfaceRoute::Settings(None),
        SurfaceRoute::Emoji,
        SurfaceRoute::Keyboard,
        SurfaceRoute::Handwriting,
        SurfaceRoute::Voice,
        SurfaceRoute::Clipboard,
        SurfaceRoute::CloudClipboard,
        SurfaceRoute::CloudDictionary,
    ];
}

#[cfg(test)]
mod tests;

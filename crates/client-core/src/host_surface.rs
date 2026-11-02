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

pub mod environment;

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
    /// 这个宿主每页最多排几个候选，见 `HostCapabilities::max_candidate_page_size`。
    pub fn max_candidate_page_size(self) -> u8 {
        match self {
            HostPlatform::Macos | HostPlatform::Linux | HostPlatform::Android => {
                crate::preferences::MAX_CANDIDATE_PAGE_SIZE
            }
            HostPlatform::Windows | HostPlatform::Ios | HostPlatform::Harmony => 9,
        }
    }

    /// 这个宿主是否显示上屏后联想（偏好 `next_word_prediction`），见 `HostCapabilities::next_word_prediction`。联想没有组字，宿主要能在没有预编辑时显示候选，并且像普通候选一样能点选、按数字键或空格选中联想，回车和标点照常输入，Esc 关闭联想。每个宿主在接好这些的那次改动里加上自己；host-api 只在这里为真的宿主上把偏好交给 Engine。
    ///
    /// Windows 没有预编辑时由 TIP 在光标处给 Server 发 Show，候选窗画 Server 会话自己的联想；点选、数字键和空格都由 Server 在它的会话里选中，TIP 把文字插在光标处；回车、标点照常交给应用，Esc 由 TIP 吃下并关掉联想。
    pub fn shows_next_word_prediction(self) -> bool {
        matches!(self, HostPlatform::Android | HostPlatform::Windows)
    }

    /// 这个宿主是否读偏好里的中英混输触发字符数，见 `HostCapabilities::mixed_input_minimum_prefix`。只有触屏宿主（Android、iOS、HarmonyOS）读：九键、十四键上触发字符数是按键次数，在玻璃上多打几个字母的代价更大，Android 也是应用户要求才加的这一项。三个桌面宿主（macOS、Windows、Linux）不读，固定用 [`crate::preferences::DEFAULT_MIXED_INPUT_MINIMUM_PREFIX`]。
    pub fn reads_mixed_input_minimum_prefix(self) -> bool {
        matches!(
            self,
            HostPlatform::Android | HostPlatform::Ios | HostPlatform::Harmony
        )
    }

    /// 这个宿主固定使用的 AI 联想候选数量，不读 `ai_assistant.candidate_limit`；`None` 表示按偏好。见 `HostCapabilities::fixed_ai_candidate_limit`。
    pub fn fixed_ai_candidate_limit(self) -> Option<u8> {
        // 共享的 engine/input-runtime 只让第一条 AI 回答坐第一页最后一格，同一批多出来的回答排到候选列表末尾，数量改了几乎看不出差别，所以桌面三家和鸿蒙固定为默认值 3，设置页不再给这一项。鸿蒙的候选条横向滚动，排在末尾的回答更难看到。Android、iOS 这一轮没有跟进，仍按偏好。
        matches!(
            self,
            HostPlatform::Macos
                | HostPlatform::Windows
                | HostPlatform::Linux
                | HostPlatform::Harmony
        )
        .then_some(3)
    }

    /// 这个宿主固定使用的打字效果强度（0-100），不读 `plugins.effect_intensity`；`None` 表示按偏好。特效包自带的强度不受影响。见 `HostCapabilities::fixed_effect_intensity`。
    pub fn fixed_effect_intensity(self) -> Option<u8> {
        // macOS 和 Windows 的三种样式（闪光、火花、Power Mode）本身就是强度的粗调，鸿蒙把三种样式都画成亮度不同的卡片闪光，特效包也会换掉强度，所以强度固定为默认值 50。Linux 不画样式，只在辅助行里显示连击数，没有强度可调，仍是 `None`，能力文档不变。
        matches!(
            self,
            HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Harmony
        )
        .then_some(50)
    }

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
    /// IBus 交给桌面面板（ibus-ui-gtk3 一类）的只有候选文字和 RGB 前景、背景属性：字体和颜色生效，候选窗的圆角、边框颜色和外部皮肤的装饰图都画不出来。`candidate_corner_radius` 是按平台给的，Linux 上它说的是 Fcitx5 的 classicui 主题，所以由运行中的 IBus 宿主用这一项说明自己的面板。
    IbusPanel,
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

/// 运行中的 Fcitx5 宿主在 X11 会话里用自己的原生画布画候选窗时，状态文件带 `"renderer": "native_canvas"`（platforms/linux 的 `candidate_panel_status_document`）。只有这条路径画得出展开的候选网格、拼音字号和选中外观；IBus 面板、classicui 主题和 Wayland 上的面板都画不出来，所以这几项不按平台给，由这份报告决定。
pub fn linux_native_canvas_renderer(document: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(document)
        .ok()
        .and_then(|value| {
            value
                .get("renderer")
                .and_then(|renderer| renderer.as_str())
                .map(|renderer| renderer == "native_canvas")
        })
        .unwrap_or(false)
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
    /// 宿主的实体键盘会话接入了韩语罗马字布局。
    pub korean_romaja: bool,
    /// The host can restart the input method service from the settings page.
    pub restart_input_method: bool,
    /// The host can open the shared panels as separate always-on-top windows.
    pub panel_windows: bool,
    /// The host keeps per-application versus global Chinese/English mode state.
    pub ime_mode_scope: bool,
    /// 宿主认得前台应用并执行 `app_input_mode_rules`（应用例外）：macOS 按 bundle id，Windows 按进程基名，Linux 按输入法框架报告的程序名（Fcitx5 的 `InputContext::program()`；IBus 只有经 IBus GTK 输入法模块输入的程序经 1.5.27 起的客户端名报告，GNOME Wayland 下经 GNOME Shell 转发的程序是匿名的）。
    pub app_input_mode_rules: bool,
    /// 宿主认得前台应用并对 `vim_mode_apps` 名单里的应用把无修饰 Esc 搭车切英文（目前只有 macOS）。
    pub vim_mode_apps: bool,
    /// The host records typing statistics.
    pub typing_statistics: bool,
    /// The host exposes the shared fuzzy-pinyin settings.
    pub fuzzy_pinyin: bool,
    /// The host can enumerate installed font families.
    pub system_fonts: bool,
    /// The shared UI draws its own titlebar and resize handles.
    pub window_chrome: bool,
    /// 宿主以某种形式提供悬浮工具栏。Linux 在 X11（以及有 Xwayland 的 Wayland 会话）上画一块与 macOS 相同的悬浮窗口，画不出来时只有 IBus 属性菜单和 Fcitx5 状态区里的「工具栏」子菜单。
    pub floating_toolbar: bool,
    /// 工具栏按 scale_percent 和 font_size 缩放。Linux 的悬浮窗口读这两项；只剩「工具栏」子菜单的会话里它们不起作用。
    pub floating_toolbar_appearance: bool,
    /// 宿主能单独显示或隐藏工具栏的各个按钮。Linux 的悬浮窗口和「工具栏」子菜单都按这些开关排。
    pub floating_toolbar_components: bool,
    /// The toolbar carries a button that opens the handwriting panel. The
    /// reference's toolbar has six components and this is not one of them, so
    /// only the host that draws the button offers the switch for it.
    pub floating_toolbar_handwriting: bool,
    /// The toolbar carries a button that starts and stops voice input, for the
    /// same reason as `floating_toolbar_handwriting`.
    pub floating_toolbar_voice: bool,
    /// 工具栏带切换输入方案的按钮。macOS、Windows 和 Linux 的悬浮工具栏画它，Linux 的「工具栏」子菜单另列出同样内容的方案子菜单，其它宿主不提供这个开关。
    pub floating_toolbar_input_scheme: bool,
    /// 宿主按 `floating_toolbar.auto_hide` / `auto_hide_delay` / `shadow` 控制自己的悬浮条。
    /// 目前只有 macOS 消费这三个值：Windows 与 HarmonyOS 的面板还没读，Linux 的工具栏是输入法菜单。
    /// 为假时设置页不列这三行，免得改了没有效果。
    pub floating_toolbar_idle_behavior: bool,
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
    /// The host can hide the candidate panel's page indicator without changing pagination. On macOS and in the Linux Fcitx5 theme that also hides the ‹ › arrows sharing the card's top row.
    pub candidate_page_number: bool,
    /// 宿主按 `show_app_logo` 显示或隐藏候选窗和悬浮工具栏左端的水杉 logo。
    pub app_logo: bool,
    /// 宿主按 `traditional_chinese_variant` 选 OpenCC 配置（`s2t`、`s2tw`、`s2twp`）做繁体转换；其它宿主一律用 `s2t`，设置页只给简体和繁体两项。
    pub traditional_chinese_variants: bool,
    /// The host can apply candidate foreground/background RGB row colors.
    /// Linux exposes these through IBusText attributes even though it cannot
    /// draw the native card geometry or hover state.
    pub candidate_row_colors: bool,
    /// The host can apply candidate accent, selection, hover and border appearance.
    pub candidate_selection_appearance: bool,
    /// 宿主按用户在自定义主题里选的边框色（`custom_theme.candidate_colors.border`）给候选窗描边。与 `candidate_selection_appearance` 分开，因为两者各自成立：Linux 的 Fcitx5 经典界面画边框而没有悬停状态。macOS、Windows、Linux 和 HarmonyOS 不读这个槽位：边框是发丝线，一律跟随主题和皮肤，设置页不给取色器；只有 Android 和 iOS 照旧读。
    pub candidate_border_color: bool,
    /// 宿主按用户在自定义主题里选的编号色（`custom_theme.candidate_colors.number`）画候选编号。macOS、Windows、Linux 和 HarmonyOS 不读这个槽位：选了文字颜色时编号色由它推出，否则跟随主题和皮肤，设置页不给取色器；只有 Android 和 iOS 照旧读。
    pub candidate_number_color: bool,
    /// 宿主应用 `candidate_scale_percent` 调整候选显示。自绘浮动窗口缩放字体和几何尺寸；桌面面板可以缩放自己拥有的字体描述。
    pub candidate_window_scale: bool,
    /// The host can lower the alpha of its candidate card fill, border and skin background by `candidate_opacity_percent` while keeping text opaque. A panel the desktop draws, or a strip that is part of an opaque keyboard, cannot.
    pub candidate_window_opacity: bool,
    /// The host rounds its candidate card by `candidate_corner_radius`, ahead of the skin package's radius and its own constant.
    pub candidate_corner_radius: bool,
    /// The host insets its candidates from the card's edge by `candidate_padding`, ahead of the skin package's padding and its own constant.
    pub candidate_padding: bool,
    /// 宿主支持窗口分轴内边距、候选项内边距、独立圆角和间距。
    #[serde(default)]
    pub candidate_item_geometry: bool,
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
    /// 宿主在双拼组字时按共享偏好 `shuangpin_keymap_hint` 在候选窗旁画当前方案的键位图，所以设置页给出「输入时显示双拼键位提示」。
    pub shuangpin_keymap_hint: bool,
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
    /// 宿主按 `keybindings.toggle_english_candidates` / `toggle_full_width` / `toggle_punctuation`
    /// 认三个切换热键（经 `msime_client_toggle_hotkey_chords` 解码），设置页给出录键控件。
    /// 目前只有 macOS；其他宿主的路由仍写死，给出控件等于让用户改一个不认的值。
    #[serde(default)]
    pub toggle_hotkey_chords: bool,
    pub voice_provider_settings: bool,
    /// The host draws the recogniser's interim text while the user is still speaking.
    ///
    /// Every host can ask a streaming provider for partial results; this says which of them has
    /// somewhere to put one. A host without that surface would be offering a switch whose only
    /// effect is on a display it does not have.
    pub voice_stream_preedit: bool,
    /// 宿主读 `voice_input.hotkey_hold_space_lock`，设置页才给出「长按录音时按空格锁定」开关。macOS、Windows、Linux（IBus、Fcitx5）和 HarmonyOS 把它固定为开：空格只在按住录音组合键期间才被拦下，平常打字不受影响，这些宿主不再读存下的 false。只有 Android、iOS 还读它。
    pub voice_space_lock_switch: bool,
    /// 宿主分别读开始、结束提示音（`voice_input.start_sound`、`end_sound`），设置页才在总开关之外给出这两个开关。macOS、Windows、HarmonyOS 只读总开关 `sound_enabled`，Linux 的转发方不再把这两项交给语音服务，服务按自己的缺省（都响）播放；只有 Android、iOS 还读它们。
    pub voice_cue_switches: bool,
    /// 宿主有不止一种录音后端可选，设置页才给出「录音后端」和手填的设备名。macOS（CoreAudio）、Windows、HarmonyOS 各只有一种录音方式，`""`、`auto` 和本平台名效果相同，设置页只给一个「麦克风」下拉，第一项是系统默认设备。Linux 的语音服务在 PulseAudio、PipeWire、ALSA 之间真有得选，设备名也要手填，所以保留。
    pub voice_capture_backends: bool,
    /// The host shows read-only English word completions while typing directly
    /// in English, governed by the shared `english_suggestions` preference. iOS
    /// offers the same surface but keeps its switch in the native App Group
    /// store, so it reads this as false and shows its own control.
    pub english_suggestions: bool,
    /// 宿主已接通普通英文、专用英文和临时英文的在线候选。
    pub english_online_candidates: bool,
    /// A letter becomes a helper code because the user held Shift for it, rather
    /// than because of where it sits in the spelling. Windows appends helper
    /// codes directly to a finished pinyin and needs no gesture; a keyboard host
    /// does, or the letter would be eaten as more pinyin. The hosts that mark
    /// them this way are the ones running the ported ChineseHelpcodePolicy, and
    /// the settings page explains the gesture only where it applies.
    pub helpcode_shift_entry: bool,
    /// 宿主通过拼写符号优先路由接通可配置的辅助码前导键。
    pub helpcode_trigger_key: bool,
    /// A skin arrives by being picked rather than by being dropped into a
    /// folder. The source opens its skin folder so the user can put one there;
    /// a host whose folder is inside an application sandbox has nothing to
    /// open, so it asks the user to point at the skin instead. The page needs
    /// to know which of the two it is, because the button says so.
    pub skin_directory_import: bool,
    /// 设置页的皮肤页提供「制作皮肤」标签：在页面里编辑候选窗皮肤，经 `msime_client_services::skin::candidate_editor` 用社区下载和 MCP 同一个安装器装进皮肤目录。三个桌面宿主的共享设置页（Tauri）能调用它；HarmonyOS 只在 2in1 上由自己的设置投影打开，手机继续用原来的皮肤界面；iOS 和 Android 没有这个入口。
    pub candidate_skin_editor: bool,
    /// The one candidate page size the host draws, when it offers no choice. The iOS keyboard numbers its strip's chips 1-9 to match the digits on its symbol layer and lays the expanded panel out in nines, so it holds the Engine to nine whatever the shared setting says; the page shows the count instead of a selector that would do nothing. Absent on a host that pages by the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_candidate_page_size: Option<u8>,
    /// 宿主能画、能用数字键选的每页候选数上限，共享文档里更大的值在这个宿主上按它截断，设置页也只给到它。macOS、Linux 和 Android 的实体键盘用 0 键选第十个，上限是 `MAX_CANDIDATE_PAGE_SIZE`（10）；Windows 的 TSF 只把 1–9 当作选词键、候选窗口也只排得下九个，iOS 固定九个，鸿蒙没有这项设置、实体键盘也只认 1–9，这三个宿主是 9。
    pub max_candidate_page_size: u8,
    /// 宿主固定使用的 AI 联想候选数量（`HostPlatform::fixed_ai_candidate_limit`，macOS、Windows、Linux、鸿蒙），这时设置页不显示「候选数量」，宿主也不读文档里存的值。按偏好的宿主没有这一项。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_ai_candidate_limit: Option<u8>,
    /// 宿主固定使用的打字效果强度（`HostPlatform::fixed_effect_intensity`，macOS、Windows、鸿蒙），这时设置页不显示「效果强度」，宿主也不读文档里存的值。按偏好的宿主没有这一项。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_effect_intensity: Option<u8>,
    /// The one candidate layout the host draws, when it offers no choice. The iOS candidate strip is a horizontal row above the keys, so an external skin is adopted there only for its horizontal layout; the skin page has to judge compatibility by that rather than by the shared setting, which defaults to vertical. Absent on a host that follows the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_candidate_layout: Option<crate::preferences::CandidateLayout>,
    /// The touch keyboard reads `touch_toolbar` to choose the buttons on the row above its keys. iOS 与 Android 的键盘读它；其他宿主上这些开关什么也不会隐藏。
    pub touch_toolbar_components: bool,
    /// 触屏键盘按 `harmony_toolbar` 选空闲工具栏上的工具（表情、常用语、剪贴板、皮肤、输入方式）和显示方式（输入时显示 / 隐藏），设置页才给出这组开关。只有 HarmonyOS 的手机键盘读它：鸿蒙工具栏的工具与 `touch_toolbar` 的成员和缺省对不上（见 `Preferences::harmony_toolbar`）。同一个 HAP 在 2in1 上没有这一行，宿主的设置桥按设备形态再把它关掉。
    pub touch_toolbar_tools: bool,
    /// 宿主按 `touch_keyboard_height_adjustment` 改变屏幕键盘的高度，设置页才给出「键盘高度」滑块。Android、iOS 和 HarmonyOS 的键盘按它加减键盘高度，Windows 的 Tauri 键盘面板按它定窗口高度（`panel_window.rs` 的 `windows_panel_height`）；macOS 和 Linux 的键盘窗口高度是固定的，滑块在那里只会让设置页的预览变高，实际键盘不变。
    pub touch_keyboard_height: bool,
    /// 宿主的屏幕键盘按 `touch_key_spacing_tenths` 和 `touch_row_spacing_tenths` 画键距和行距，设置页才给出两个间距滑块、在预览上拖动调间距和「恢复默认」。macOS 不提供：鼠标操作的键盘面板用不着触屏的间距微调，键盘面板固定用默认的 6.0 / 7.0 dp，不再读存下的值。
    pub touch_keyboard_spacing: bool,
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
    /// 宿主提供 AI 生成类功能：把用户的文字发给大模型、再把生成的内容展示或上屏，包括 AI 对话、AI 润色与高情商回复、AI 联想候选、AI 翻译、语音识别后的 AI 润色和 AI 皮肤生成。为假时设置页不给 AI 辅助页、AI 对话和其余这些入口，宿主的键盘也不发这些请求，存下的偏好打开了也一样。只有 HarmonyOS 为假：华为应用市场（中国大陆）要求 AI 生成类功能先做实名认证、提交《安全评估报告》和生成内容标识材料，这些资质到位之前鸿蒙版不提供，到位后把这里改回真，见 `.agents/notes/implemented/feature/2026-10-27-harmony-hide-ai-generation.md`。鸿蒙的社区皮肤也跟着这一项：为假时设置桥（`HarmonyAccountCloudBridge.ets`）让 Rust 带 `exclude=ai`，不列 AI 生成和来源不明的作品；改回真会同时停止排除，而现在没有一端读 `origin` 画「AI 生成」标识，所以改之前先在各端画出标识，否则 AI 皮肤会不带标识地出现在鸿蒙上，见 `.agents/notes/implemented/feature/2026-10-28-harmony-withhold-community-contribution.md`。系统语音识别、云候选和机器翻译服务不是大模型生成，不归这一项管。
    pub ai_generation: bool,
    /// 宿主提供社区投稿类功能：发布键盘皮肤、词包、短语、回复模板和插件，修改已发布作品的分类，发布新版本，以及给他人作品评分和举报。为假时设置页不画这些入口，宿主的设置桥也拒绝这些请求；浏览、搜索、详情、下载、安装、收藏、导入和下架自己的作品照常。只有 HarmonyOS 为假：华为应用市场（中国大陆）要求「信息发布」类服务先做实名认证，鸿蒙版的邮箱和华为账号登录都不核验手机号，实名认证到位之前不提供，到位后把这里改回真即可，见 `.agents/notes/implemented/feature/2026-10-28-harmony-withhold-community-contribution.md`。
    pub community_contribution: bool,
    /// The host draws a short, non-activating badge near the caret after the
    /// Chinese/English mode changes. A host with no way to put a window beside
    /// the caret, or one whose keyboard already shows the mode on its own key
    /// faces, has nothing to switch on and does not offer the choice.
    pub input_mode_hud: bool,
    /// 宿主在系统菜单栏放一个常驻的状态项，显示当前输入模式。只有 macOS 需要：系统的输入法菜单图标只在用户把对应模式（例如「英」）加成输入源后才会跟着变，而 Windows 有托盘语言栏，Linux 有 Fcitx5/IBus 面板，触屏键盘的键面本身就显示模式。
    pub input_mode_status_item: bool,
    /// 宿主按 `expand_candidates` 让横排候选窗按 Tab、↓ 展开成多行候选网格。macOS 实现；Linux 的 Fcitx5 在 X11 上用原生画布画候选窗时也实现，由宿主报告后经 `apply_linux_native_canvas` 打开。其他宿主接上之前不提供这个开关，免得设置页存下一个宿主不理会的值。
    pub expand_candidates: bool,
    /// 宿主画的各个界面（设置窗口、屏幕键盘、悬浮工具栏与中英文提示、菜单、表情面板、手写识别板、语音输入弹出条）一律跟随颜色模式，不读 `settings_theme`、`screen_keyboard_theme`、`toolbar_theme`、`menu_theme`、`emoji_theme`、`handwriting_theme`、`voice_theme`，设置页也不给这几行；候选窗口仍按 `candidate_theme` 单独设明暗。桌面宿主（macOS、Windows、Linux、HarmonyOS）都是这样，Android 和 iOS 不是：固定明暗的内置主题本来就会盖过工具栏和菜单的单独设置，单给一个界面定明暗的需求由「夜」「墨」这类主题满足。
    pub surface_themes_follow: bool,
    /// The host can run a 背单词 review session — that is, it has wired the shared vocabulary
    /// entry point and can reach the review store.
    pub vocabulary_review: bool,
    /// 背单词书目里列出单词本插件（`pack-<插件 id>` 词书）：宿主把插件目录交给背单词的入口。
    pub wordbook_packs: bool,
    /// 宿主能用系统自带的语音合成在背单词卡片上读出英文词头，卡片上才画喇叭。macOS、Windows 的设置应用用网页的 `speechSynthesis`（只用本机声音），Linux 的设置应用起 speech-dispatcher 的 `spd-say`，所以 Linux 只在 `PATH` 上有它时为真（`fill_linux_environment` 填）。iOS 和 Android 的原生背单词页用 `AVSpeechSynthesizer` 和 `TextToSpeech`（Android 另外看装没装语音引擎），HarmonyOS 的设置页经设置桥交给 Core Speech Kit 的离线声音。候选菜单里的「朗读」不读这一位，各宿主按自己的引擎在不在决定。见 `.agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md`。
    pub read_aloud: bool,
    /// 宿主的符号面板显示已安装的符号集插件。没打开时插件详情说明本机的符号面板不显示插件符号集。
    pub symbol_set_packs: bool,
    /// 设置页的「脚本插件（实验）」分组：列出已安装的脚本插件、授权、调整执行顺序。三个桌面平台的 release 构建都运行脚本：macOS（输入法进程）、Linux（IBus 引擎、Fcitx5 addon）和 Windows（Server）。Windows 的设置是原生应用（platforms/windows/settings），它照这个分组画出自己的授权页；这里为真也让 Windows 上的共享设置页和它一致。
    pub script_plugins: bool,
    /// The host plays the sound packs in `plugins`: a sample per key class, the melody, the commit sound and the achievement jingle. Only an input process that sees the keys can, and only where it has somewhere to play them; a host without the player keeps the settings but offers no switches for them.
    pub key_sound: bool,
    /// The host routes the `/` command and `@` mention modes: it hands `/` and `@` to the runtime, stops treating digits as candidate numbers while a mode spells with them, and loads the enabled command tables and the name list into the Engine. The `V` mode needs only the digit routing and is covered by the same flag.
    pub plugin_triggers: bool,
    /// The host streams the selected music pack while it is the active input method.
    pub music: bool,
    /// The host draws the typing effects and the combo count that `msime_client_typing_effect` answers with. Each host flips this only in the change that wires the call, as with the flags above.
    pub typing_effects: bool,
    /// 宿主经 `msime_client_set_caps_lock` 向会话报告大写锁定状态，所以「大写锁定时使用英文标点」（`caps_lock_ascii_punctuation`）在这里有效。和上面几项一样，每个宿主只在接上这个调用的那次改动里打开它。
    pub caps_lock_punctuation: bool,
    /// 宿主实现了「单按 Caps Lock 切换中英文」（`KeybindingPreferences::switch_language_caps_lock`）：能分辨轻点和长按，并在轻点后把大写锁定撤回。
    pub caps_lock_language_switch: bool,
    /// 宿主的快捷模式只从实体键盘的 Shift+字母进入，所以「Shift+字母进入快捷模式」总开关（`LocalModePreferences::shift_letter_entry`）在这里有意义。触屏键盘从菜单打开快捷模式，走的也是 Engine 的同一个入口，不提供这个开关，以免关掉后菜单失灵。
    pub shift_letter_entry_switch: bool,
    /// 宿主把快捷短语编码追加到普通候选。macOS 和 Linux 提供，其他宿主仍从 K 模式进入。
    pub quick_phrase_candidates: bool,
    /// 宿主提供「移动光标后编辑光标后的部分」（偏好 `caret_edit_suffix`）：实体键盘能把组字光标移进拼音中间，候选窗画模式提示。三个桌面宿主提供；host-api 在其他宿主上不把偏好交给 Engine，设置页也不列出它。
    pub caret_edit_suffix: bool,
    /// 宿主显示上屏后联想（偏好 `next_word_prediction`），设置页列出它的开关；触屏宿主（`mobile_settings`）还列出联想栏按键的样式（`next_word_prediction_buttons`）。取值见 [`HostPlatform::shows_next_word_prediction`]。
    pub next_word_prediction: bool,
    /// 宿主实现了「Ctrl+Shift+J 在中文和日语之间切换」（`KeybindingPreferences::toggle_japanese_ctrl_shift_j`）：实体键盘在中文状态按下时，从中文方案切到日语方案，或从日语等外语方案回到 `last_chinese_scheme`。设置页还要看本版本是否同时提供中文方案和日语，才显示这个开关。
    pub japanese_toggle_shortcut: bool,
    /// 宿主把「切换中英文」那一组快捷键接进中日英循环（`KeybindingPreferences::trilingual_cycle` 与 `crate::language_cycle`）：中 → 日 → 英 → 中，有组字时先上屏原始字母，英文模式下交回应用。目前只有 macOS 接入；其余宿主不声明此能力。
    pub trilingual_cycle: bool,
    /// 宿主读 `navigation.tab` 和 `navigation.page_up_down`，设置页给出「Shift+Tab / Tab」和「PageUp / PageDown」两组翻页键的开关。只在关掉确实把键还给应用的宿主上为真：
    ///
    /// - macOS 不读，两组固定开启：候选窗显示时，关掉的 Tab 和 PageUp/PageDown 照样被输入法占着（`InputController.mm` 返回已处理），关掉只让它们什么也不做。候选窗没显示时 Tab 一律交给应用；关掉的 PageUp/PageDown 原本也直接交给应用（先结束失焦上屏的组字），固定开启后先作为翻页命令交给会话，会话不处理才交给应用。
    /// - Windows 不读，两组固定开启（`NavigationPolicy.h` 的 `preference_navigation`）：候选窗显示或组字时 TIP 总把这几个键交给 Server，关掉的绑定由 `navigation_action` 回答 `NavigationReply::Ignored`，键被吃掉，不会还给应用。
    /// - HarmonyOS 不读，两组固定开启（`KeyboardSession.ets`）：组字时关掉的绑定被 `HardwareKeyRouter` 判为 `IGNORED` 并消费掉，同样只是让键失效。
    /// - Linux 照旧读：fcitx5 对关掉的绑定结束组字并把键还给应用（`FcitxEngine.cpp`），开关有实际作用。Android 和 iOS 也照旧读。
    ///
    /// 这两项不进账号同步，固定下来不会让别的设备同步来的值在固定的宿主上看不见却生效。
    pub tab_page_paging_switches: bool,
    /// 宿主读 `mixed_input.minimum_prefix`，设置页给出「触发字符数」。三个桌面宿主不读，固定为默认的 5（[`HostPlatform::reads_mixed_input_minimum_prefix`]）：调低会让常见简拼被英文词抢走首选，不喜欢英文候选的人有「中英混输」开关。这一项不进账号同步。
    pub mixed_input_minimum_prefix: bool,
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
    /// 内核版本（Linux 的 `/proc/sys/kernel/osrelease`），给「关于」页的系统信息。与 `os_version` 一样由宿主在 `for_platform` 之后按实际机器填写，读不到就不写；解析见 [`environment`]。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    /// 桌面会话（Linux 的 `XDG_CURRENT_DESKTOP` 与 `XDG_SESSION_TYPE`，如 `ubuntu:GNOME (wayland)`），给「关于」页的系统信息。运行时填写，同上。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_session: Option<String>,
    /// 水杉实际挂在哪个输入法框架上（Linux 的 `IBus` / `Fcitx5`，取自运行中的宿主写的状态文件），给「关于」页的系统信息。宿主没有在运行、或者还没写过状态时缺省。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_method_framework: Option<String>,
    /// 设备型号（Linux 的 DMI 厂商与产品名），给「关于」页的系统信息。运行时填写，同上。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
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
const ANDROID_HELPCODE_SCHEMAS: [&str; 7] = [
    "ziranma",
    "xiaohe",
    "lantian",
    "shouyou2_0",
    "shouyouplus",
    "jiajia",
    "wubi86",
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
    /// Linux 的 Fcitx5 宿主报告它用原生画布画候选窗（[`linux_native_canvas_renderer`]）时，设置页也提供那条画法画得出来的几项：展开候选、拼音字号和选中外观。
    pub fn apply_linux_native_canvas(&mut self) {
        if self.platform != HostPlatform::Linux {
            return;
        }
        self.expand_candidates = true;
        self.candidate_preedit_font = true;
        self.candidate_selection_appearance = true;
    }

    /// Capabilities as they stand today for each shipped host. Slices that add a
    /// capability to a host flip its flag here, and every consumer follows.
    pub fn for_platform(platform: HostPlatform) -> Self {
        HostCapabilities {
            platform,
            mobile_settings: !platform.is_desktop(),
            korean_romaja: platform.is_desktop(),
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
            // macOS 的输入控制器在每次切换客户端时按 bundle id 查规则；Windows 由 Server 按焦点客户端的进程基名查规则，TIP 激活时也按它定起始模式。其余宿主不认规则，设置页不显示规则表。
            app_input_mode_rules: platform.is_desktop(),
            // Windows 还在 TSF 本地读自己的 `vim_mode_apps`，共享键先只给 macOS。
            vim_mode_apps: matches!(platform, HostPlatform::Macos),
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
            // macOS 的 FloatingToolbarPanel.mm、Windows 的 FloatingToolbarWindow 和 Linux 的 FloatingToolbarX11Surface 都读 scale_percent 和 font_size；HarmonyOS 的面板按前者缩放自己、按后者画按钮面。
            floating_toolbar_appearance: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Linux
            ),
            // 自己画工具栏窗口的宿主（macOS、Windows，以及 Linux 在 X11 和 Xwayland 上的悬浮工具栏）按这些开关排自己的按钮；Linux 另把它们映射到 IBus 属性菜单和 Fcitx5 状态区的「工具栏」子菜单，画不出悬浮窗口的会话靠后者。HarmonyOS 的面板隐藏对应按钮并收窄自己，表情和屏幕键盘按钮打开手机键盘从键面进入的同一些面。
            floating_toolbar_components: platform.is_desktop() || platform == HostPlatform::Harmony,
            // macOS、Windows 和 Linux 的悬浮工具栏画这三个按钮，Linux 的 IBus 与 Fcitx5「工具栏」子菜单另列出对应的三项；手写还要随版本收窄（narrow_to_edition）。
            floating_toolbar_handwriting: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Linux
            ),
            floating_toolbar_voice: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Linux
            ),
            floating_toolbar_input_scheme: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Linux
            ),
            // 只有 macOS 的面板读 auto_hide / auto_hide_delay / shadow（见 FloatingToolbarBehavior.h）。
            floating_toolbar_idle_behavior: platform == HostPlatform::Macos,
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
            // macOS 候选窗首行的「1 / 3」和 ‹ › 一起隐藏，只留翻页箭头的话首行照样占高度，候选窗不会变紧凑（#7316）。Linux 去掉辅助文本里的「当前页/总页数」，Fcitx5 的经典界面主题和原生画布也一起不画 ‹ ›；IBus 面板的翻页箭头属于面板，管不到。键盘和滚轮翻页都不受影响。
            candidate_page_number: matches!(platform, HostPlatform::Linux | HostPlatform::Macos),
            // macOS、Windows 和 Linux 按这个开关画 logo（Linux 是 Fcitx5 经典界面主题里的那个，IBus 面板本来就画不了 logo，见 `CandidatePanelLimit::IbusPanel`）；鸿蒙的候选窗照常显示 logo，不提供这个开关。
            app_logo: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Linux
            ),
            // 目前只有 Linux 的 IBus 与 Fcitx5 宿主把变体交给 `msime_client_convert_to_traditional`。
            traditional_chinese_variants: matches!(platform, HostPlatform::Linux),
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
            // 桌面宿主（macOS、Windows、Linux 的 Fcitx5 经典界面、HarmonyOS 2in1 的候选窗）的边框色固定跟随主题和皮肤，宿主不读用户选的边框色；只有 Android 和 iOS 的候选条照旧读。
            candidate_border_color: matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            // 桌面宿主的编号色固定跟随主题和皮肤（选了文字颜色时由文字颜色推出），宿主不读用户选的编号色；只有 Android 和 iOS 照旧读。
            candidate_number_color: matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            // Windows 和 macOS 自绘候选浮窗并缩放全部几何尺寸。Linux 的 Fcitx5 经典界面主题同样按它缩放圆角、内边距、行边距、阴影、翻页按钮和 logo。Android 和 iOS 的候选条按它缩放字号和候选行高度，跟随候选栏的空闲工具栏一起变矮或变高（#10137）。鸿蒙在平台层面不声明：2in1 的候选卡片只随字号调整，手机和平板的键盘按它缩放候选栏，由设置桥按设备形态打开（`SettingsFormFactorCapabilities`）。
            candidate_window_scale: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Linux
                    | HostPlatform::Android
                    | HostPlatform::Ios
            ),
            // Linux 的 Fcitx5 经典界面主题把不透明度写进卡片图片的 alpha（需要合成器，KWin 下还会模糊卡片后面的内容）；IBus 面板做不到，运行中的 IBus 宿主经 `candidate_panel_limit` 让设置页说明这一点。
            candidate_window_opacity: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Linux
            ),
            // Linux 上只有 Fcitx5 的 classicui 主题画圆角；IBus 面板画不了，运行中的 IBus 宿主经 `candidate_panel_limit`（`CandidatePanelLimit::IbusPanel`）让设置页说明这一点，能力位本身仍按平台给。
            candidate_corner_radius: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Macos
                    | HostPlatform::Linux
                    | HostPlatform::Harmony
            ),
            // macOS 候选窗和 Linux 的 Fcitx5 经典界面主题按 `candidate_padding` 改卡片内边距（#7316；IBus 面板做不到，同样经 `candidate_panel_limit` 说明）。Windows 的卡片内边距写在 `candidate_card_metrics` 里，鸿蒙卡片有自己的边距，接上之前不声明，免得设置页出现一个改了没反应的滑块。
            candidate_padding: matches!(platform, HostPlatform::Macos | HostPlatform::Linux),
            candidate_item_geometry: platform == HostPlatform::Macos,
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
            // Windows 的 Server 在光标旁画一个不抢焦点的小窗（InputModeHudWindow），配色和尺寸取悬浮工具栏的。
            input_mode_hud: matches!(
                platform,
                HostPlatform::Macos
                    | HostPlatform::Harmony
                    | HostPlatform::Linux
                    | HostPlatform::Windows
            ),
            // macOS 的输入法进程自己建 NSStatusItem。Windows 的托盘语言栏（platforms/windows/tsf/LanguageBar）和 Linux 的 Fcitx5/IBus 面板已经常驻显示模式，iOS、Android 和 HarmonyOS 的键面也显示模式，都不提供这个开关。
            input_mode_status_item: matches!(platform, HostPlatform::Macos),
            // 展开候选网格由 macOS 的候选窗画（#7307）；Linux 的 Fcitx5 在 X11 上用原生画布画候选窗时也画，由运行中的宿主经候选面板状态报告（`apply_linux_native_canvas`），不按平台给。Windows 的桌面候选窗以后可以接同一个偏好；触屏键盘本来就有自己的展开面板。
            expand_candidates: matches!(platform, HostPlatform::Macos),
            // 桌面宿主（macOS、Windows、Linux、HarmonyOS）让工具栏、菜单和各面板一律跟随颜色模式；Android 和 iOS 的键盘仍按各界面的单独明暗着色。
            surface_themes_follow: !matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            // Windows chooses between TSF, SendInput and a paste; macOS between system events and its input session. The Linux hosts commit through the IBus or Fcitx5 input context, the desktop voice panel hands its text to the active host the way every panel does, and the voice provider only recognizes, so a stored mode would change nothing there. A keyboard extension commits through its input client and has nothing to choose between either.
            voice_commit_mode: matches!(platform, HostPlatform::Windows | HostPlatform::Macos),
            // The Linux hosts write the snapshot's `preedit` into the IBus and
            // Fcitx5 preedit themselves, and both already carry their own
            // toggle for this in the native status menu - a setting the shared
            // page was hiding could only be reached from there, and only while
            // the shuangpin scheme was active.
            // Windows 的候选窗预编辑行画的是 view.preedit，TIP 的行内组字也取自己宿主会话 View 的 preedit（「原始按键」样式）或 Server 回传的预编辑（「拼音分词」样式），两处都随这个偏好变化，所以 Windows 同样给出这一项。
            shuangpin_preedit: matches!(
                platform,
                HostPlatform::Macos
                    | HostPlatform::Windows
                    | HostPlatform::Harmony
                    | HostPlatform::Linux
                    | HostPlatform::Ios
            ),
            // macOS 的 MSIMEShuangpinKeymapPanel、Windows Server 的 ShuangpinKeymapWindow 和 Linux 的 ShuangpinKeymapX11Surface 在候选窗旁画键位图（Linux 只给报根窗口坐标光标的 X11 客户端画）；其他宿主没有这块浮窗，开关在那里什么也不做。
            shuangpin_keymap_hint: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Linux
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
            // macOS 从上线起就占用这个组合键；Android 宿主的 Alt+Shift+H 读同一个偏好；Windows 的 TSF 在中文模式下同样认 Alt+Shift+H。其他宿主不绑定它。
            fullwidth_chord: matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Android | HostPlatform::Windows
            ),
            // macOS 宿主在 `applySharedInputPreferences` 时解码三个 chord 并按 `MSIMEToggleChordMatches` 认键；其余宿主仍写死。
            toggle_hotkey_chords: platform == HostPlatform::Macos,
            voice_provider_settings: true,
            voice_stream_preedit: !matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            // 空格锁定和提示音细分只有 Android、iOS 还读存下的值；录音后端只有 Linux 真有得选（见各字段说明）。
            voice_space_lock_switch: matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            voice_cue_switches: matches!(platform, HostPlatform::Android | HostPlatform::Ios),
            voice_capture_backends: !matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Harmony
            ),
            english_suggestions: matches!(platform, HostPlatform::Android | HostPlatform::Harmony),
            english_online_candidates: platform == HostPlatform::Macos,
            // HarmonyOS 的符号面板和外接键盘都按引擎发布的拼写符号把前导键当字符送进来（`HardwareKeyRouter.touchSpells`）。
            helpcode_trigger_key: matches!(platform, HostPlatform::Linux | HostPlatform::Harmony),
            // Android、HarmonyOS 和 iOS 键盘在组字时用 Shift 标记辅助码；桌面默认直接追加编码。
            helpcode_shift_entry: matches!(
                platform,
                HostPlatform::Android | HostPlatform::Harmony | HostPlatform::Ios
            ),
            // The skin folder is inside the sandbox on HarmonyOS and in the App Group container on iOS, where no file manager reaches it, so the skin is picked and copied in instead.
            skin_directory_import: matches!(platform, HostPlatform::Harmony | HostPlatform::Ios),
            // 编辑器只在桌面形态的设置页里出现；HarmonyOS 的 2in1 由 `SettingsFormFactorCapabilities` 按设备形态打开。
            candidate_skin_editor: platform.is_desktop(),
            fixed_candidate_page_size: (platform == HostPlatform::Ios).then_some(9),
            max_candidate_page_size: platform.max_candidate_page_size(),
            fixed_ai_candidate_limit: platform.fixed_ai_candidate_limit(),
            fixed_effect_intensity: platform.fixed_effect_intensity(),
            fixed_candidate_layout: (platform == HostPlatform::Ios)
                .then_some(crate::preferences::CandidateLayout::Horizontal),
            // The iOS shortcut bar is the touch counterpart of the Windows floating toolbar, and its buttons follow the same kind of per-component switches. Android 的原生键盘工具栏也按 `touch_toolbar` 选按钮。
            touch_toolbar_components: matches!(platform, HostPlatform::Ios | HostPlatform::Android),
            touch_toolbar_tools: platform == HostPlatform::Harmony,
            // Tauri 键盘面板只在 Windows 上按这一项定窗口高度；macOS 的面板和原生回退面板高度固定，Linux 同样没有接上。
            touch_keyboard_height: matches!(
                platform,
                HostPlatform::Windows
                    | HostPlatform::Android
                    | HostPlatform::Ios
                    | HostPlatform::Harmony
            ),
            // 只有 macOS 把间距固定成默认值；Windows 与 Linux 的 Tauri 键盘面板和触屏键盘照旧读用户的间距。
            touch_keyboard_spacing: platform != HostPlatform::Macos,
            // Linux keeps AI credentials in the provider service's owner-only
            // configuration file and passes only non-sensitive options over its
            // socket. Every other host holds the token itself.
            ai_provider_credentials: platform == HostPlatform::Linux,
            // 鸿蒙版在拿到 AI 生成类资质之前不提供这些功能，键盘一侧的同一道闸是 `AiGenerationGate.ts`。
            ai_generation: platform != HostPlatform::Harmony,
            // 鸿蒙版在实名认证到位之前不提供社区发布、评分和举报，设置桥一侧的同一道闸是 `CommunityContributionGate.ts`。
            community_contribution: platform != HostPlatform::Harmony,
            // Windows draws Latin from its own family, macOS and Android name it ahead of the primary one, and ArkUI resolves a family list per glyph, so HarmonyOS reaches the same result the same way. Both Linux hosts write one Pango font description for the desktop panel, and Pango resolves its family list per glyph too, so they name it first there.
            candidate_english_font: true,
            // Every host reaches the same shared store through the same entry point, so there is no platform here that can and one that cannot.
            vocabulary_review: true,
            // 桌面宿主的背单词由 Tauri 层传入插件目录；HarmonyOS 在自己的设置投影里按形态打开；Android 和 iOS 不传插件目录。
            wordbook_packs: platform.is_desktop(),
            // Linux 的取值由设置应用按 `spd-say` 在不在填，这里先当作没有。
            read_aloud: platform != HostPlatform::Linux,
            // Windows 和 Linux 桌面的符号面板是 Tauri 层的表情面板（`load_emoji_catalog`），Linux 的 Fcitx5 菜单和 macOS 的原生表情与符号面板另外读同一批插件组。HarmonyOS 在自己的设置投影里按形态打开；Android 和 iOS 没有接入。
            symbol_set_packs: platform.is_desktop(),
            // 三个桌面平台的输入法进程在启动时打开脚本插件（`msime_client_scripts_enable`），授权由设置应用经 client-services 的 `script_grants` 写入，授权页写明同一用户下的其他程序能伪造授权。Windows 的授权页在原生设置应用里（经 host-api 的 `msime_client_plugins`），共享设置页在 Windows 上也照样显示这一组。移动端没有脚本插件。各宿主接入生命周期核心（`msime_client_set_lifecycle`）之前，会话不满足运行条件，授权写下了也不会运行。
            script_plugins: platform.is_desktop(),
            // 三个桌面宿主播放音效包、按 Engine 的拼写符号路由 V、/ 和 @，并在作为当前输入法时播放音乐。HarmonyOS 在自己的设置投影里按形态声明：按键音和音乐只在 2in1，V、/、@ 每种形态都声明（实体键盘在任何设备上都路由它们，手机和平板的触屏键盘从「本地输入」进入 / 指令）。背后没有实现的开关读起来像设置被无视，所以每个宿主只在接上它的那次改动里在这里打开。
            // Android 的按键音由 IME 进程的 SoundPool 播放（与本项同一波接入），所以 Android 也声明；手机和平板上的 HarmonyOS 与 iOS 仍未接入。
            key_sound: platform.is_desktop() || platform == HostPlatform::Android,
            plugin_triggers: platform.is_desktop(),
            music: platform.is_desktop(),
            // macOS 画火花、卡片闪光和连击徽标（TypingEffectPanel.mm）；Windows 画同样的东西，火花在候选窗旁的光标浮层里（TypingEffectOverlay.cpp），卡片闪光和 Power Mode 的抖动在 CandidateWindow.cpp；两个 Linux 宿主在候选栏的辅助行里显示连击数（KeySound.h）；HarmonyOS 在 KeyboardView 上画闪光和连击徽标。Linux 不画任何样式，只显示计数，设置页自己在那里藏起样式控件（`showTypingEffectStyles`）。HarmonyOS 仍在自己的设置投影里按设备形态收窄；Android 和 iOS 都没有接入。
            typing_effects: platform.is_desktop() || platform == HostPlatform::Harmony,
            // macOS（`InputController.mm` 的 `syncCapsLock`）和 Linux 的两个宿主（IBus 的 `sync_caps_lock`、Fcitx5 的 `syncSessionCapsLock`，都从按键事件带的锁定状态读）报告大写锁定。Windows 的宿主和带实体键盘的移动端接上同一个调用后在这里加上自己。
            caps_lock_punctuation: matches!(platform, HostPlatform::Macos | HostPlatform::Linux),
            // 三个桌面宿主都在自己的按键路径上分辨 Caps Lock 的轻点和长按：macOS 用 IOKit 撤回大写锁定，Windows 用 SendInput 再按一次，Linux 在 X11 会话里用 XKB 撤回（Wayland 会话里由合成器持有锁定状态，宿主撤不回，那里轻点照常是大写锁定）。
            caps_lock_language_switch: platform.is_desktop(),
            shift_letter_entry_switch: platform.is_desktop(),
            // macOS 和 Linux 的两个宿主建会话时都请求 `quick_phrase_candidates`。
            quick_phrase_candidates: matches!(platform, HostPlatform::Macos | HostPlatform::Linux),
            // 三个桌面宿主的候选窗都画「候选编辑：光标后」提示，设置在共享设置页（macOS、Linux）和 Windows 的原生设置里。
            caret_edit_suffix: platform.is_desktop(),
            next_word_prediction: platform.shows_next_word_prediction(),
            // Windows 和 Linux 只注册了一个中文输入法，没有别的键盘快捷键能换到日语方案。macOS 把「日」声明成系统输入模式，用 Ctrl+空格和地球键轮换，不需要它。Android 实体键盘和 HarmonyOS 2in1 实体键盘也读共享快捷键，但还没有接上这个组合键，接上的那次改动再在这里加上自己。
            japanese_toggle_shortcut: matches!(
                platform,
                HostPlatform::Windows | HostPlatform::Linux
            ),
            // 目前只有 macOS 把「切换中英文」这一组快捷键接进了循环；Windows 的 TIP 与 Server 之间要新开一条协议，Linux 的两个引擎各自处理切换，都留给后续改动，等接上时再在这里加上自己。
            trilingual_cycle: platform == HostPlatform::Macos,
            // macOS（`AppearancePreferences.mm` 的 `-navigationEnabled:`）、Windows（`NavigationPolicy.h` 的 `preference_navigation`）和 HarmonyOS（`KeyboardSession.ets`）把 Tab 和 PageUp/PageDown 两组翻页键固定为开，其余宿主照旧读偏好。
            tab_page_paging_switches: !matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Windows | HostPlatform::Harmony
            ),
            mixed_input_minimum_prefix: platform.reads_mixed_input_minimum_prefix(),
            os_version: None,
            arch: None,
            kernel_version: None,
            desktop_session: None,
            input_method_framework: None,
            device_model: None,
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
    /// AI 助手连接页，提供 MCP 的设置、皮肤、词库与诊断能力。
    Mcp,
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
            SettingsCategory::Mcp => "mcp",
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
            "mcp" => Ok(SettingsCategory::Mcp),
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

    pub const ALL: [SettingsCategory; 23] = [
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
        SettingsCategory::Mcp,
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
    /// 背单词面板。`settings:vocabulary` 仍是设置里管理词书的那一页，这条路由打开复习用的面板。
    Vocabulary,
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
            SurfaceRoute::Vocabulary => "vocabulary".to_string(),
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
            "vocabulary" => Ok(SurfaceRoute::Vocabulary),
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
            SurfaceRoute::Vocabulary => Some(PanelSurface {
                label: "vocabulary-panel",
                query: "vocabulary",
                title: "水杉背单词",
                width: 560,
                height: 680,
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

    pub const ALL: [SurfaceRoute; 9] = [
        SurfaceRoute::Settings(None),
        SurfaceRoute::Emoji,
        SurfaceRoute::Keyboard,
        SurfaceRoute::Handwriting,
        SurfaceRoute::Voice,
        SurfaceRoute::Clipboard,
        SurfaceRoute::CloudClipboard,
        SurfaceRoute::CloudDictionary,
        SurfaceRoute::Vocabulary,
    ];
}

#[cfg(test)]
mod tests;

//! Safe discovery and validation of external candidate-skin manifests.
//!
//! `read_resource` supplies bytes for host resource delivery analogous to the
//! Windows `candidate-skins` virtual-folder mapping (settings_app.cpp at
//! 04a8df56f86312474a069f4335a1b58da7afaa9e). It does not register a protocol,
//! authorize a webview origin, sanitize CSS/SVG, or execute resource content.

use image::AnimationDecoder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use toml::Value;

mod factory_layout;
mod toolbar_image;
mod windows_looks;
/// 定义在 `crate::text`（偏好校验要用），从这里导回。
pub(crate) use crate::text::safe_id;
/// 定义在 `crate::theme`（偏好校验要用），从这里导回，原来的路径照常可用。
pub use crate::theme::is_selectable_id;
pub use factory_layout::ResolvedSkinLayout;
pub use toolbar_image::{
    SkinToolbarButtonFlow, SkinToolbarButtonLook, SkinToolbarImage, SkinToolbarImageButton,
    ToolbarFlowDirection, ToolbarImageButtonSlot, ToolbarImageLayout, TOOLBAR_BUTTON_KEYS,
};
pub use windows_looks::{
    is_windows_look, WINDOWS_BUILTIN_IDS, WINDOWS_DEFAULTS_FOLDER, WINDOWS_LOOK_IDS,
};

/// Manifest values, not trusted CSS. Renderers must validate color syntax before
/// inserting these strings into styles; scanning does not authorize CSS execution.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CandidatePalette {
    pub accent: Option<String>,
    pub selected: Option<String>,
    pub hover: Option<String>,
    pub surface: Option<String>,
    pub border: Option<String>,
    pub text: Option<String>,
    pub number: Option<String>,
    /// Secondary candidate text (translations, comments); the resolved theme's `secondary` slot.
    pub translation: Option<String>,
    /// 释义里的词性与义项分隔符；支持的宿主可单独指定，其他宿主可忽略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_separator: Option<String>,
    /// 候选窗的细分文字槽位；不支持该槽位的宿主沿用基础颜色。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_caret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_translation_pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_translation_separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_bar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_divider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_arrow_divider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_luminosity: Option<f64>,
    /// 候选窗右键菜单颜色；未声明时沿用候选窗基础颜色。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<CandidateMenuPalette>,
    #[serde(rename(serialize = "showSelectedBar", deserialize = "show_selected_bar"))]
    pub show_selected_bar: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CandidateMenuPalette {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CandidateColors {
    pub dark: CandidatePalette,
    pub light: CandidatePalette,
}

fn read_colors(table: &toml::map::Map<String, Value>) -> Result<CandidateColors, String> {
    let Some(value) = table.get("candidate") else {
        return Ok(CandidateColors::default());
    };
    let candidate = value.as_table().ok_or("invalid candidate colors")?;
    for theme in ["dark", "light"] {
        if candidate
            .get(theme)
            .is_some_and(|palette| !palette.is_table())
        {
            return Err("invalid candidate colors".into());
        }
    }
    let colors: CandidateColors = value
        .clone()
        .try_into()
        .map_err(|_| "invalid candidate colors")?;
    for theme in ["dark", "light"] {
        let Some(palette) = candidate.get(theme).and_then(Value::as_table) else {
            continue;
        };
        check_colors(palette, &CANDIDATE_COLOR_KEYS)?;
        if let Some(menu) = palette.get("menu") {
            check_colors(
                menu.as_table().ok_or("invalid candidate colors")?,
                &MENU_COLOR_KEYS,
            )?;
        }
    }
    Ok(colors)
}

/// `[candidate.dark]` / `[candidate.light]` 里的颜色键：基础槽位由各宿主绘制，其余细分槽位由支持它们的宿主绘制，其他宿主只校验并沿用基础颜色。
const CANDIDATE_COLOR_KEYS: [&str; 24] = [
    "accent",
    "selected",
    "hover",
    "surface",
    "border",
    "text",
    "number",
    "translation",
    "translation_pos",
    "translation_separator",
    "candidate_text",
    "preedit_text",
    "preedit_caret",
    "selected_text",
    "selected_number",
    "selected_translation",
    "selected_translation_pos",
    "selected_translation_separator",
    "selected_bar",
    "preedit_background",
    "preedit_divider",
    "page_arrow_divider",
    "backdrop_opacity",
    "backdrop_luminosity",
];

/// `[candidate.<明暗>.menu]` 的颜色键，即候选框右键菜单的配色。
const MENU_COLOR_KEYS: [&str; 4] = ["background", "border", "text", "hover"];

/// 清单里的颜色值会被 msime-windows 拼进 WebView2 页面的 CSS 声明，所以每个平台都只放行颜色写法用得到的字符，挡住 `;`、`{}` 之类能跳出声明的写法；这样同一个包在每个平台上要么都能加载，要么都被拒绝。字符合法但读不懂的颜色仍然只是不画。
pub(crate) fn css_color_text(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"#(),.% -/".contains(&byte))
}

fn check_colors(table: &toml::map::Map<String, Value>, keys: &[&str]) -> Result<(), String> {
    for key in keys {
        let Some(value) = table.get(*key) else {
            continue;
        };
        if matches!(*key, "backdrop_opacity" | "backdrop_luminosity") {
            number_in(value, 0.0..=1.0).ok_or("invalid candidate colors")?;
            continue;
        }
        let value = value.as_str().ok_or("invalid candidate colors")?;
        if value.len() > 80 {
            return Err("candidate color exceeds 80 bytes".into());
        }
        if !css_color_text(value) {
            return Err("invalid candidate colors".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinSummary {
    /// 此皮肤包使用的清单版本。版本 1 禁用动画，版本 2 增加下面的有限原生动画契约。
    pub schema_version: u8,
    pub id: String,
    pub name: String,
    pub version: String,
    /// The global theme the package's colours are drawn over: `system` (the host's native tokens) or a built-in theme. A manifest `base` that names anything else, `custom` included, is a manifest error and the package is not listed.
    pub base: super::theme::GlobalTheme,
    pub author: Option<String>,
    pub description: Option<String>,
    pub layouts: Vec<String>,
    pub themes: Vec<String>,
    pub min_width_dip: f64,
    /// `true` 表示 `base = "canvas"`；`base` 字段仍按 `system` 传给跨平台主题解析。
    #[serde(skip_serializing_if = "is_false")]
    pub canvas: bool,
    /// The candidate card's corner radius (`candidate_window.corner_radius_dip`, 0-32); `None` keeps the host's own radius.
    pub corner_radius_dip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width_dip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_corner_radius_dip: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_arrows: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit_content: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand_panel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand_rows: Option<u8>,
    #[serde(skip_serializing_if = "SkinGeometry::is_empty")]
    pub geometry: SkinGeometry,
    #[serde(skip_serializing_if = "SkinGeometry::is_empty")]
    pub horizontal: SkinGeometry,
    #[serde(skip_serializing_if = "SkinGeometry::is_empty")]
    pub vertical: SkinGeometry,
    /// 绘制专用的出厂继承结果；原始几何字段保留皮肤声明，供编辑器区分“未声明”。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_layout: Option<ResolvedSkinLayout>,
    pub decoration_top_dip: f64,
    pub decoration_width_dip: f64,
    /// The image drawn in the decoration band, relative to the package: `candidate_window.decoration.image`, or else the package `preview` when that is an image. Set only for a decorated package (both decoration sizes above zero).
    pub decoration_image: Option<String>,
    /// Where the decoration sits along the card's top edge.
    pub decoration_align: DecorationAlign,
    /// 候选窗在屏幕上时整张装饰图的待机动效（`decoration.motion`，只在 schema 2 里可写），缺省 `none`。与 `[animation]` 的显隐动效无关。
    pub decoration_motion: DecorationMotion,
    /// 待机动效一个周期的毫秒数（`decoration.motion_period_ms`），限制在 [`DECORATION_MOTION_PERIOD_MS`] 内，缺省 [`DEFAULT_DECORATION_MOTION_PERIOD_MS`]。
    pub decoration_motion_period_ms: u16,
    /// 装饰图的帧数：静态 PNG、JPEG 为 1，APNG 为它 `acTL` 声明的动画帧数，没有装饰图时为 0。能播帧动画的宿主据此决定是否播放，其他宿主只画第一帧。
    pub decoration_frames: u32,
    /// An image drawn over the card surface and under the candidates, clipped to the card's rounded outline.
    pub background: Option<SkinBackground>,
    /// The floating toolbar's own geometry and colours, drawn over what the base theme derives and under `toolbar_stylesheet`.
    pub toolbar: SkinToolbar,
    /// 图片工具栏的布局与资源；`None` 表示普通条形模式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toolbar_image: Option<SkinToolbarImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toolbar_icon_corner_radius_dip: Option<f64>,
    /// `[candidate_window.border_image]` 九宫格面板。只在 `base = "canvas"` 时读取；按 msime-windows 的规则校验并保留，供支持贴图的宿主绘制；其他宿主使用可读回退（见 [`Self::panel_candidate`]）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_image: Option<SkinBorderImage>,
    /// `candidate_window.content_insets_dip`，只在 `base = "canvas"` 时读取；由支持 canvas 的宿主消费。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_insets_dip: Option<SkinInsets>,
    /// `candidate_window.line_gap_dip`，只在 `base = "canvas"` 时读取；由支持 canvas 的宿主消费。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_gap_dip: Option<f64>,
    /// `candidate_window.outer_insets_dip`，只在 `base = "canvas"` 时读取；由支持 canvas 的宿主消费。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_insets_dip: Option<SkinInsets>,
    /// `[[candidate_window.decorations]]` 自由挂件，只在 `base = "canvas"` 时读取；由支持 canvas 的宿主绘制。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorations: Vec<SkinFreeDecoration>,
    pub toolbar_stylesheet: Option<String>,
    pub preview: Option<String>,
    pub candidate: CandidateColors,
    /// 面板贴图成功绘制时使用的配色；`candidate` 始终保留未绘制贴图时的可读回退。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel_candidate: Option<CandidateColors>,
    /// 声明式动画预设。不支持动画的宿主保持自己的动态策略，候选语义仍然一致。
    pub animation: SkinAnimation,
    /// Licence metadata the manifest declares for its code and assets. Informational only: nothing is enforced from it.
    pub license: Option<SkinLicense>,
    /// 不影响加载的提示，对应 msime-windows 的 `package.notices`：例如非 `canvas` 基底上写了只属于 `canvas` 的键而被忽略，或 `canvas` 贴图需要宿主具备对应绘制能力。包照常列出、照常可用；这些提示不进 [`SkinCatalog::issues`]，那里只放被忽略的无效目录。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

/// 使用固定动画名称，不接受任意 CSS。这样共享皮肤可以安全同步，各原生宿主也能把相同的动效意图映射到自己的合成器原语。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkinAnimationPreset {
    #[default]
    None,
    Fade,
    FadeScale,
    SlideUp,
    Spring,
    Glow,
}

impl SkinAnimationPreset {
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::FadeScale => "fade_scale",
            Self::SlideUp => "slide_up",
            Self::Spring => "spring",
            Self::Glow => "glow",
        }
    }
}

/// 版本 2 的候选窗动效契约。扫描时限制时长，避免皮肤包让候选窗无限动画。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinAnimation {
    pub enter: SkinAnimationPreset,
    pub exit: SkinAnimationPreset,
    pub selection: SkinAnimationPreset,
    pub duration_ms: u16,
    pub stagger_ms: u16,
    /// 清单的 `[animation]` 表里实际写了哪几个预设。上面的值已经补好默认，宿主据此区分“皮肤要求的动画”和“省略后套用的默认组合”；macOS 只播放写了的预设，见 .agents/notes/implemented/feature/2026-10-16-macos-candidate-declared-motion.md。
    pub declared: SkinAnimationDeclared,
}

/// `SkinAnimation` 的三个预设各自是否由清单显式写出。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkinAnimationDeclared {
    pub enter: bool,
    pub exit: bool,
    pub selection: bool,
}

impl SkinAnimationDeclared {
    pub const NONE: Self = Self {
        enter: false,
        exit: false,
        selection: false,
    };
}

impl SkinAnimation {
    pub const fn disabled() -> Self {
        Self {
            enter: SkinAnimationPreset::None,
            exit: SkinAnimationPreset::None,
            selection: SkinAnimationPreset::None,
            duration_ms: 0,
            stagger_ms: 0,
            declared: SkinAnimationDeclared::NONE,
        }
    }

    pub const fn lively() -> Self {
        Self {
            enter: SkinAnimationPreset::FadeScale,
            exit: SkinAnimationPreset::Fade,
            selection: SkinAnimationPreset::Spring,
            duration_ms: 180,
            stagger_ms: 14,
            declared: SkinAnimationDeclared::NONE,
        }
    }
}

impl Default for SkinAnimation {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DecorationAlign {
    Left,
    Center,
    /// The trailing edge, which is where every host drew the decoration before the manifest could say otherwise.
    #[default]
    Right,
}

/// 装饰图的待机动效。和 [`SkinAnimationPreset`] 一样只接受固定名字，幅度由宿主决定并保持很小（浮动约 ±3 点、摇摆约 ±3°、呼吸放大到 1.03、弹跳一次小跳），不接受任意曲线或 CSS。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationMotion {
    #[default]
    None,
    /// 上下缓慢浮动。
    Float,
    /// 以图片底边中点为轴左右摇摆。
    Sway,
    /// 以图片底边中点为基准轻微放大再收回。
    Breathe,
    /// 周期性地轻轻跳一下。
    Bounce,
}

impl DecorationMotion {
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Float => "float",
            Self::Sway => "sway",
            Self::Breathe => "breathe",
            Self::Bounce => "bounce",
        }
    }
}

/// `decoration.motion_period_ms` 的取值范围。下限避免装饰图抖动着吸引注意力，上限之外的动效慢到看不出来。
pub const DECORATION_MOTION_PERIOD_MS: std::ops::RangeInclusive<u16> = 1200..=8000;
pub const DEFAULT_DECORATION_MOTION_PERIOD_MS: u16 = 3000;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundFit {
    /// Fill the card, cropping the image's overflow.
    #[default]
    Cover,
    /// Show the whole image inside the card.
    Contain,
    /// Scale each axis to the card independently.
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinBackground {
    /// Relative to the package, an image type, confined to the package directory.
    pub image: String,
    pub fit: BackgroundFit,
    /// 0-1, multiplied into the image.
    pub opacity: f64,
}

/// Toolbar colours for one mode, each normalized by `theme::normalized_color` to `#RRGGBB` or `#RRGGBBAA` (the manifest may write `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()` or `transparent`); a colour it cannot read is left out, as candidate colours are.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct ToolbarPalette {
    pub background: Option<String>,
    pub border: Option<String>,
    /// The drag handle.
    pub handle: Option<String>,
    pub divider: Option<String>,
    pub icon: Option<String>,
    /// Hovered button fill.
    pub hover: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbar {
    /// 0-32; `None` keeps the host's own radius.
    pub corner_radius_dip: Option<f64>,
    pub dark: ToolbarPalette,
    pub light: ToolbarPalette,
}

/// 候选窗清单中的长度：可以是固定 DIP，也可以是按候选字号缩放的 em。
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinLength {
    pub value: f64,
    pub em: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinGeometry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_gap: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_extra_height: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_inset: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_padding_right: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_min_width: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_gap: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_bar_width: Option<SkinLength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_bar_height_em: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_bar_inside: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_corners_follow_card: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_rows: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_hidden_preedit_gap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_arrow_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_margin_dip: Option<SkinInsets>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinBorderImage {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_source: Option<String>,
    pub slice_dip: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stretch_dip: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_dip: Option<Vec<f64>>,
    pub fill: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinFreeDecoration {
    pub image: String,
    pub anchor: String,
    pub offset_x_dip: f64,
    pub offset_y_dip: f64,
    pub width_dip: f64,
    pub height_dip: f64,
    pub z_index: i32,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl SkinGeometry {
    fn is_empty(&self) -> bool {
        self.padding.is_none()
            && self.item_gap.is_none()
            && self.item_extra_height.is_none()
            && self.number_inset.is_none()
            && self.item_padding_right.is_none()
            && self.item_min_width.is_none()
            && self.number_gap.is_none()
            && self.number_scale.is_none()
            && self.selected_bar_width.is_none()
            && self.selected_bar_height_em.is_none()
            && self.selected_bar_inside.is_none()
            && self.item_corners_follow_card.is_none()
            && self.justify_rows.is_none()
            && self.trim_hidden_preedit_gap.is_none()
            && self.page_arrow_style.is_none()
            && self.preedit_margin_dip.is_none()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct SkinLicense {
    pub code: Option<String>,
    pub assets: Option<String>,
    pub source: Option<String>,
}

impl SkinSummary {
    fn supports_theme(&self, theme: &str) -> bool {
        self.themes.iter().any(|value| value == theme)
    }

    /// Compatibility comes from the manifest, not the base skin's capabilities.
    pub fn supports(&self, layout: &str, theme: &str) -> bool {
        self.layouts.iter().any(|value| value == layout) && self.supports_theme(theme)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkinIssue {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct SkinCatalog {
    pub packages: Vec<SkinSummary>,
    pub issues: Vec<SkinIssue>,
}

/// `id` 是否已被占用：全局主题、msime-windows 的内置外观，以及 Windows 放内置外观设置的 `default` 目录。外部皮肤不能用这些 ID，主题、内置外观和皮肤文件夹因此不会混淆，同一个包在每个平台上也都能用同一个文件夹名加载。
pub fn is_reserved(id: &str) -> bool {
    super::theme::GlobalTheme::from_id(id).is_some()
        || WINDOWS_BUILTIN_IDS.contains(&id)
        || id == WINDOWS_DEFAULTS_FOLDER
}

/// 该名字能否作为外部皮肤的文件夹名。目录扫描只列出这样的文件夹，导入时按同一规则把关，免得拷进来一个扫描随后拒绝的皮肤。
pub fn is_external_id(id: &str) -> bool {
    safe_id(id) && !is_reserved(id)
}

pub(crate) fn contained(root: &Path, child: &Path) -> bool {
    root.canonicalize()
        .ok()
        .and_then(|r| child.canonicalize().ok().map(|c| c.starts_with(r)))
        .unwrap_or(false)
}

fn required_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<String, String> {
    let value = table
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))?;
    validate_string(value, key, max)?;
    Ok(value.to_owned())
}

fn optional_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))?;
    validate_string(value, key, max)?;
    Ok(Some(value.to_owned()))
}

/// 包的介绍性文字（`author`、`description`）。与 msime-windows 的 `ReadString(..., required = false)` 相同，空字符串等于没写；长度上限照旧。
fn optional_text(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, String> {
    match table.get(key) {
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        _ => optional_string(table, key, max),
    }
}

fn validate_string(value: &str, key: &str, max: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max {
        return Err(format!("{key} has invalid length"));
    }
    Ok(())
}

pub(crate) fn safe_resource(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && crate::is_ascii_identifier_with_dots(part)
        })
}

fn enum_array(
    table: &toml::map::Map<String, Value>,
    key: &str,
    allowed: &[&str],
) -> Option<Vec<String>> {
    let items = table.get(key)?.as_array()?;
    if items.is_empty() {
        return None;
    }
    let mut values = Vec::with_capacity(items.len().min(allowed.len()));
    for item in items {
        let value = item.as_str()?;
        if !allowed.contains(&value) || values.iter().any(|existing| existing == value) {
            return None;
        }
        values.push(value.to_owned());
    }
    Some(values)
}

fn load(root: &Path, folder: &str) -> Result<SkinSummary, String> {
    if crate::storage::reject_symlink(root).is_err() {
        return Err("missing skin directory".into());
    }
    if !safe_id(folder) || is_reserved(folder) {
        return Err("invalid skin id".into());
    }
    let dir = root.join(folder);
    let manifest = dir.join("skin.toml");
    if !contained(root, &dir) || !contained(&dir, &manifest) {
        return Err("manifest escapes skin directory".into());
    }
    // Check the type before opening: opening a FIFO for reading blocks until a writer appears, which would stall the scan and every host that resolves a skin.
    if !fs::symlink_metadata(&manifest)
        .map_err(|_| "missing skin.toml".to_owned())?
        .is_file()
    {
        return Err("skin.toml is not a regular file".into());
    }
    let input = crate::storage::open_private_file_in(&manifest)
        .map_err(|_| "missing skin.toml".to_owned())?;
    if !input
        .metadata()
        .map_err(|_| "unreadable skin.toml")?
        .is_file()
    {
        return Err("skin.toml is not a regular file".into());
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        input,
        65_536,
        || "skin.toml is too large".to_owned(),
        |_| "unreadable skin.toml".to_owned(),
    )?;
    let value: Value =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| "skin.toml is not UTF-8")?)
            .map_err(|_| "invalid TOML")?;
    let table = value.as_table().ok_or("manifest must be a table")?;
    let schema_version = match table.get("schema_version").and_then(Value::as_integer) {
        Some(1) => 1,
        Some(2) => 2,
        _ => return Err("unsupported schema_version".into()),
    };
    let id = required_string(table, "id", 64)?;
    if id != folder || !safe_id(&id) {
        return Err("manifest id does not match folder".into());
    }
    let name = required_string(table, "name", 80)?;
    let version = required_string(table, "version", 32)?;
    let base = required_string(table, "base", 32)?;
    // `canvas` 是 msime-windows 的全贴图基底，九宫格面板、自由挂件和几处内边距只属于它，而且只在 v2 里可写，所以 v1 写 `canvas` 直接拒绝（与 msime-windows 相同）。msime 的 v2 本身（`[animation]`、装饰图待机动效、APNG 帧）对任何基底都有效，不受这里影响。
    let canvas = base == "canvas";
    if canvas && schema_version != 2 {
        return Err("base = canvas requires schema_version 2".into());
    }
    let author = optional_text(table, "author", 120)?;
    let description = optional_text(table, "description", 500)?;
    // `base` 也可以是 msime-windows 的内置外观，Windows 上写的包用的就是它们。外观画在 `system` 之上，包没写的颜色稍后按外观补齐（见 `windows_looks`）；`fluent` 就是 Windows 的原生配色，与 `system` 相同。只有清单这样读；其他地方 `fluent` 是已停用的 ID，一律拒绝。
    let windows_look = is_windows_look(&base).then(|| base.clone());
    let base = if windows_look.is_some() || canvas {
        "system"
    } else {
        base.as_str()
    };
    let base = super::theme::GlobalTheme::from_id(base)
        .filter(|theme| theme.is_base())
        .ok_or("base must be system or a built-in theme")?;
    let supports = table
        .get("supports")
        .and_then(Value::as_table)
        .ok_or("missing supports")?;
    let layouts =
        enum_array(supports, "layouts", &["horizontal", "vertical"]).ok_or("invalid supports")?;
    let themes = enum_array(supports, "themes", &["dark", "light"]).ok_or("invalid supports")?;
    let window = table
        .get("candidate_window")
        .and_then(Value::as_table)
        .ok_or("missing candidate_window")?;
    let number = |value: Option<&Value>| match value {
        None => 0.0,
        Some(value) => value
            .as_float()
            .or_else(|| value.as_integer().map(|n| n as f64))
            .unwrap_or(f64::NAN),
    };
    let min_width = number(window.get("min_width_dip"));
    if !min_width.is_finite() || !(0.0..=1000.0).contains(&min_width) {
        return Err("invalid min_width_dip".into());
    }
    let mut corner_radius = match window.get("corner_radius_dip") {
        None => None,
        Some(value) => Some(number_in(value, 0.0..=32.0).ok_or("invalid corner_radius_dip")?),
    };
    let border_width_dip = window
        .get("border_width_dip")
        .map(|value| number_in(value, 0.0..=4.0).ok_or("invalid border_width_dip"))
        .transpose()?;
    let item_corner_radius_dip = window
        .get("item_corner_radius_dip")
        .map(|value| number_in(value, 0.0..=16.0).ok_or("invalid item_corner_radius_dip"))
        .transpose()?;
    let shadow = match window.get("shadow") {
        None => None,
        Some(value) => Some(
            value
                .as_str()
                .filter(|value| matches!(*value, "none" | "soft" | "strong"))
                .ok_or("invalid shadow")?
                .to_owned(),
        ),
    };
    let backdrop = match window.get("backdrop") {
        None => None,
        Some(value) => Some(
            value
                .as_str()
                .filter(|value| matches!(*value, "none" | "acrylic" | "mica"))
                .ok_or("invalid backdrop")?
                .to_owned(),
        ),
    };
    let font_family = match window.get("font_family") {
        None => None,
        Some(value) => {
            let value = value.as_str().ok_or("invalid font_family")?;
            if value.is_empty()
                || value.len() > 64
                || value
                    .bytes()
                    .any(|byte| byte < 0x20 || byte == 0x7f || b"\"'\\,;{}<>`".contains(&byte))
            {
                return Err("invalid font_family".into());
            }
            Some(value.to_owned())
        }
    };
    let page_arrows = optional_bool(window, "page_arrows")?;
    let fit_content = optional_bool(window, "fit_content")?;
    let expand_panel = optional_bool(window, "expand_panel")?;
    let expand_rows = window
        .get("expand_rows")
        .map(|value| {
            value
                .as_integer()
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| (2..=8).contains(value))
                .ok_or("invalid expand_rows")
        })
        .transpose()?;
    let geometry = read_geometry(window)?;
    let horizontal = match window.get("horizontal") {
        None => SkinGeometry::default(),
        Some(value) => read_geometry(value.as_table().ok_or("invalid horizontal geometry")?)?,
    };
    let vertical = match window.get("vertical") {
        None => SkinGeometry::default(),
        Some(value) => read_geometry(value.as_table().ok_or("invalid vertical geometry")?)?,
    };
    let toolbar_icon_corner_radius_dip = match table.get("toolbar") {
        None => None,
        Some(value) => {
            let toolbar = value.as_table().ok_or("invalid toolbar")?;
            toolbar
                .get("icon_corner_radius_dip")
                .map(|value| number_in(value, 0.0..=16.0).ok_or("invalid icon_corner_radius_dip"))
                .transpose()?
        }
    };
    let (border_image, content_insets_dip, line_gap_dip, outer_insets_dip, decorations) =
        if canvas && schema_version == 2 {
            let border_image = window
                .get("border_image")
                .map(|value| read_border_image(&dir, value))
                .transpose()?;
            let content_insets_dip = read_insets(window, "content_insets_dip", 200.0, 500.0)?;
            let line_gap_dip = window
                .get("line_gap_dip")
                .map(|value| number_in(value, 0.0..=100.0).ok_or("invalid line_gap_dip"))
                .transpose()?;
            let outer_insets_dip = read_insets(window, "outer_insets_dip", 500.0, 500.0)?;
            let decorations = window
                .get("decorations")
                .map(|value| read_free_decorations(&dir, value))
                .transpose()?
                .unwrap_or_default();
            (
                border_image,
                content_insets_dip,
                line_gap_dip,
                outer_insets_dip,
                decorations,
            )
        } else {
            (None, None, None, None, Vec::new())
        };
    let mut notices = Vec::new();
    if !canvas {
        // 与 msime-windows 一样，非 `canvas` 基底上的画布专用键不校验、不生效，但要说出来，免得作者以为它们起了作用。
        let ignored: Vec<&str> = CANVAS_ONLY_WINDOW_KEYS
            .into_iter()
            .filter(|key| window.contains_key(*key))
            .collect();
        if !ignored.is_empty() {
            notices.push(format!(
                "candidate_window.{} only apply with base = \"canvas\" and schema_version = 2; ignored",
                ignored.join(", candidate_window.")
            ));
        }
    } else if border_image.is_some() || !decorations.is_empty() {
        notices.push(CANVAS_RENDERER_NOTICE.to_owned());
    }
    check_shared_window_keys(window)?;
    // A package without a decoration may leave the whole table out.
    let empty = toml::map::Map::new();
    let decoration = match window.get("decoration") {
        None => &empty,
        Some(value) => value.as_table().ok_or("invalid decoration")?,
    };
    let top = number(decoration.get("top_inset_dip"));
    let width = number(decoration.get("width_dip"));
    if !top.is_finite()
        || !width.is_finite()
        || !(0.0..=500.0).contains(&top)
        || !(0.0..=1000.0).contains(&width)
        || (top == 0.0) != (width == 0.0)
    {
        return Err("invalid decoration".into());
    }
    let decorated = top > 0.0;
    let decoration_image = optional_string(decoration, "image", 256)?;
    if let Some(image) = &decoration_image {
        if !decorated || !image_resource(&dir, image) {
            return Err("invalid decoration".into());
        }
    }
    let decoration_align = match optional_string(decoration, "align", 16)?.as_deref() {
        None | Some("right") => DecorationAlign::Right,
        Some("left") => DecorationAlign::Left,
        Some("center") => DecorationAlign::Center,
        Some(_) => return Err("invalid decoration".into()),
    };
    let (decoration_motion, decoration_motion_period_ms) =
        read_decoration_motion(decoration, decorated, schema_version)?;
    let background = match window.get("background") {
        None => None,
        Some(value) => Some(read_background(
            &dir,
            value.as_table().ok_or("invalid background")?,
        )?),
    };
    let mut toolbar = read_toolbar(table)?;
    let toolbar_image = toolbar_image::read(table, &mut notices)?;
    let license = match table.get("license") {
        None => None,
        Some(value) => {
            let license = value.as_table().ok_or("invalid license")?;
            Some(SkinLicense {
                code: optional_string(license, "code", 120)?,
                assets: optional_string(license, "assets", 120)?,
                source: optional_string(license, "source", 500)?,
            })
        }
    };
    let toolbar_stylesheet = optional_string(table, "toolbar_stylesheet", 128)?;
    if let Some(stylesheet) = &toolbar_stylesheet {
        if !safe_resource(stylesheet, 128)
            || stylesheet.contains('/')
            || stylesheet.len() <= 4
            || !stylesheet.ends_with(".css")
            || !contained(&dir, &dir.join(stylesheet))
            || !dir.join(stylesheet).is_file()
        {
            return Err("invalid toolbar_stylesheet".into());
        }
    }
    let preview = optional_string(table, "preview", 256)?;
    if let Some(preview) = &preview {
        if !safe_resource(preview, 256) || !contained(&dir, &dir.join(preview)) {
            return Err("invalid preview".into());
        }
    }
    let mut candidate = read_colors(table)?;
    let panel_candidate = if canvas && border_image.is_some() {
        let mut colors = candidate.clone();
        windows_looks::fill_from_canvas_look(&mut colors, &mut toolbar, false);
        Some(colors)
    } else {
        None
    };
    let animation = read_animation(table, schema_version)?;
    if let Some(look) = &windows_look {
        windows_looks::fill_from_look(look, &mut candidate, &mut corner_radius, &mut toolbar);
    } else if canvas {
        windows_looks::fill_from_canvas_look(&mut candidate, &mut toolbar, border_image.is_some());
    }
    let decoration_image = decoration_image.or_else(|| {
        preview
            .clone()
            .filter(|preview| decorated && is_image(preview))
    });
    let mut summary = SkinSummary {
        schema_version: schema_version as u8,
        id,
        name,
        version,
        base,
        author,
        description,
        layouts,
        themes,
        min_width_dip: min_width,
        canvas,
        corner_radius_dip: corner_radius,
        border_width_dip,
        item_corner_radius_dip,
        shadow,
        backdrop,
        font_family,
        page_arrows,
        fit_content,
        expand_panel,
        expand_rows,
        geometry,
        horizontal,
        vertical,
        resolved_layout: None,
        decoration_top_dip: top,
        decoration_width_dip: width,
        decoration_image,
        decoration_align,
        decoration_motion,
        decoration_motion_period_ms,
        decoration_frames: 0,
        background,
        toolbar,
        toolbar_image,
        toolbar_icon_corner_radius_dip,
        border_image,
        content_insets_dip,
        line_gap_dip,
        outer_insets_dip,
        decorations,
        toolbar_stylesheet,
        preview,
        candidate,
        panel_candidate,
        animation,
        license,
        notices,
    };
    summary.resolved_layout = factory_layout::resolve(
        if canvas {
            Some("canvas")
        } else {
            windows_look.as_deref()
        },
        &summary,
    );
    summary.decoration_frames = validate_image_dimensions(&dir, &summary)?;
    Ok(summary)
}

/// `[candidate_window]` 里只属于 `base = "canvas"` 的键（msime-windows 的全贴图几何）。其他基底上它们不校验、不生效，只记一条 [`SkinSummary::notices`]。
const CANVAS_ONLY_WINDOW_KEYS: [&str; 5] = [
    "border_image",
    "content_insets_dip",
    "line_gap_dip",
    "outer_insets_dip",
    "decorations",
];

/// `canvas` 的可用效果由宿主绘制能力决定；无贴图能力时使用不透明底色的可读配色。
const CANVAS_RENDERER_NOTICE: &str = "canvas textures require host renderer support; when not drawn, the candidate card uses the opaque canvas look instead";

/// `[candidate_window]` 里由部分原生宿主绘制的键：外框线宽、高亮圆角、阴影、字体和翻页箭头。其他宿主不绘制它们，但仍按共享规则校验，同一个包在每个平台上才会有同样的加载结果。
fn check_shared_window_keys(window: &toml::map::Map<String, Value>) -> Result<(), String> {
    for (key, max) in [("border_width_dip", 4.0), ("item_corner_radius_dip", 16.0)] {
        if let Some(value) = window.get(key) {
            number_in(value, 0.0..=max).ok_or_else(|| format!("invalid {key}"))?;
        }
    }
    if let Some(value) = window.get("shadow") {
        if !matches!(value.as_str(), Some("none" | "soft" | "strong")) {
            return Err("invalid shadow".into());
        }
    }
    if let Some(value) = window.get("font_family") {
        // 原生宿主和设置页都可能把字体名放进 CSS 或脚本字符串，所以挡住引号、反斜杠、逗号、分号、括号类符号、反引号和控制字符；中文字体名的非 ASCII 字节照常放行。
        let valid = value.as_str().is_some_and(|family| {
            !family.is_empty()
                && family.len() <= 64
                && family.bytes().all(|byte| {
                    byte >= 0x80
                        || (byte >= 0x20 && byte != 0x7f && !b"\"'\\,;{}<>`".contains(&byte))
                })
        });
        if !valid {
            return Err("invalid font_family".into());
        }
    }
    if window
        .get("page_arrows")
        .is_some_and(|value| !value.is_bool())
    {
        return Err("invalid page_arrows".into());
    }
    for key in ["fit_content", "expand_panel"] {
        if window.get(key).is_some_and(|value| !value.is_bool()) {
            return Err(format!("invalid {key}"));
        }
    }
    Ok(())
}

/// A TOML integer or float inside `range`.
fn number_in(value: &Value, range: std::ops::RangeInclusive<f64>) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
        .filter(|number| number.is_finite() && range.contains(number))
}

fn optional_bool(table: &toml::map::Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("invalid {key}")),
    }
}

fn read_length(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max_dip: f64,
    max_em: f64,
) -> Result<Option<SkinLength>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    if let Some(text) = value.as_str() {
        let Some(number) = text.strip_suffix("em") else {
            return Err(format!("invalid {key}"));
        };
        let number = number
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=max_em).contains(value))
            .ok_or_else(|| format!("invalid {key}"))?;
        return Ok(Some(SkinLength {
            value: number,
            em: true,
        }));
    }
    let number = number_in(value, 0.0..=max_dip).ok_or_else(|| format!("invalid {key}"))?;
    Ok(Some(SkinLength {
        value: number,
        em: false,
    }))
}

fn read_box_values(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: f64,
) -> Result<Option<Vec<f64>>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .filter(|values| matches!(values.len(), 1 | 2 | 4))
        .ok_or_else(|| format!("invalid {key}"))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        result.push(number_in(value, 0.0..=max).ok_or_else(|| format!("invalid {key}"))?);
    }
    Ok(Some(result))
}

fn expand_box(values: &[f64]) -> SkinInsets {
    let top = values[0];
    let right = values.get(1).copied().unwrap_or(top);
    let bottom = values.get(2).copied().unwrap_or(top);
    let left = values.get(3).copied().unwrap_or(right);
    SkinInsets {
        top,
        right,
        bottom,
        left,
    }
}

fn read_insets(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max_top_bottom: f64,
    max_left_right: f64,
) -> Result<Option<SkinInsets>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let values = value.as_table().ok_or_else(|| format!("invalid {key}"))?;
    let top = values
        .get("top")
        .map(|value| number_in(value, 0.0..=max_top_bottom))
        .unwrap_or(Some(0.0))
        .ok_or_else(|| format!("invalid {key}"))?;
    let right = values
        .get("right")
        .map(|value| number_in(value, 0.0..=max_left_right))
        .unwrap_or(Some(0.0))
        .ok_or_else(|| format!("invalid {key}"))?;
    let bottom = values
        .get("bottom")
        .map(|value| number_in(value, 0.0..=max_top_bottom))
        .unwrap_or(Some(0.0))
        .ok_or_else(|| format!("invalid {key}"))?;
    let left = values
        .get("left")
        .map(|value| number_in(value, 0.0..=max_left_right))
        .unwrap_or(Some(0.0))
        .ok_or_else(|| format!("invalid {key}"))?;
    Ok(Some(SkinInsets {
        top,
        right,
        bottom,
        left,
    }))
}

fn read_geometry(table: &toml::map::Map<String, Value>) -> Result<SkinGeometry, String> {
    let preedit_margin = read_box_values(table, "preedit_margin_dip", 500.0)?
        .as_deref()
        .map(expand_box);
    Ok(SkinGeometry {
        padding: read_length(table, "padding", 64.0, 4.0)?,
        item_gap: read_length(table, "item_gap", 64.0, 4.0)?,
        item_extra_height: read_length(table, "item_extra_height", 64.0, 4.0)?,
        number_inset: read_length(table, "number_inset", 64.0, 4.0)?,
        item_padding_right: read_length(table, "item_padding_right", 64.0, 4.0)?,
        item_min_width: read_length(table, "item_min_width", 500.0, 40.0)?,
        number_gap: read_length(table, "number_gap", 64.0, 4.0)?,
        number_scale: table
            .get("number_scale")
            .map(|value| number_in(value, 0.0..=2.0).ok_or("invalid number_scale"))
            .transpose()?,
        selected_bar_width: read_length(table, "selected_bar_width", 16.0, 1.0)?,
        selected_bar_height_em: table
            .get("selected_bar_height_em")
            .map(|value| number_in(value, 0.0..=2.0).ok_or("invalid selected_bar_height_em"))
            .transpose()?,
        selected_bar_inside: optional_bool(table, "selected_bar_inside")?,
        item_corners_follow_card: optional_bool(table, "item_corners_follow_card")?,
        justify_rows: optional_bool(table, "justify_rows")?,
        trim_hidden_preedit_gap: optional_bool(table, "trim_hidden_preedit_gap")?,
        page_arrow_style: table
            .get("page_arrow_style")
            .map(|value| match value.as_str() {
                Some("chevron" | "triangle") => Ok(value.as_str().unwrap().to_owned()),
                _ => Err("invalid page_arrow_style"),
            })
            .transpose()?,
        preedit_margin_dip: preedit_margin,
    })
}

fn animatable_image_resource(dir: &Path, value: &str) -> bool {
    let extension = Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase());
    matches!(extension.as_deref(), Some("png" | "apng" | "gif" | "webp"))
        && image_resource(dir, value)
}

fn read_border_image(dir: &Path, value: &Value) -> Result<SkinBorderImage, String> {
    let table = value.as_table().ok_or("invalid border_image")?;
    let source = required_string(table, "source", 256)?;
    if !animatable_image_resource(dir, &source) {
        return Err("invalid border_image source".into());
    }
    let light_source = optional_string(table, "light_source", 256)?;
    if light_source
        .as_deref()
        .is_some_and(|source| !animatable_image_resource(dir, source))
    {
        return Err("invalid border_image light_source".into());
    }
    let slice_dip =
        read_box_values(table, "slice_dip", 500.0)?.ok_or("invalid border_image slice_dip")?;
    let stretch_dip = read_box_values(table, "stretch_dip", 500.0)?;
    let width_dip = read_box_values(table, "width_dip", 500.0)?;
    let fill = optional_bool(table, "fill")?.unwrap_or(true);
    Ok(SkinBorderImage {
        source,
        light_source,
        slice_dip,
        stretch_dip,
        width_dip,
        fill,
    })
}

fn read_free_decorations(dir: &Path, value: &Value) -> Result<Vec<SkinFreeDecoration>, String> {
    let values = value.as_array().ok_or("invalid decorations")?;
    if values.len() > 32 {
        return Err("too many decorations".into());
    }
    values
        .iter()
        .map(|value| {
            let table = value.as_table().ok_or("invalid decoration")?;
            let image = required_string(table, "image", 256)?;
            if !animatable_image_resource(dir, &image) {
                return Err("invalid decoration image".into());
            }
            let anchor = match table.get("anchor") {
                None => "right".to_owned(),
                Some(value) => match value.as_str() {
                    Some(
                        value @ ("top-left" | "top-right" | "bottom-left" | "bottom-right" | "left"
                        | "right" | "top" | "bottom" | "center"),
                    ) => value.to_owned(),
                    _ => return Err("invalid decoration anchor".into()),
                },
            };
            let number = |key: &str, limit: f64| {
                table
                    .get(key)
                    .map(|value| {
                        number_in(value, -limit..=limit)
                            .ok_or_else(|| format!("invalid decoration {key}"))
                    })
                    .transpose()
                    .map(|value| value.unwrap_or(0.0))
            };
            let z_index = table
                .get("z_index")
                .map(|value| {
                    value
                        .as_integer()
                        .and_then(|value| i32::try_from(value).ok())
                        .filter(|value| (-1000..=1000).contains(value))
                        .ok_or("invalid decoration z_index")
                })
                .transpose()?
                .unwrap_or(2);
            let positive = |key: &str| {
                table
                    .get(key)
                    .map(|value| {
                        number_in(value, 0.0..=2000.0)
                            .filter(|value| *value > 0.0)
                            .ok_or_else(|| format!("invalid decoration {key}"))
                    })
                    .transpose()
                    .and_then(|value| value.ok_or_else(|| format!("missing decoration {key}")))
            };
            Ok(SkinFreeDecoration {
                image,
                anchor,
                offset_x_dip: number("offset_x_dip", 2000.0)?,
                offset_y_dip: number("offset_y_dip", 2000.0)?,
                width_dip: positive("width_dip")?,
                height_dip: positive("height_dip")?,
                z_index,
            })
        })
        .collect()
}

pub(crate) fn is_image(relative: &str) -> bool {
    resource_content_type(relative).is_some_and(|kind| kind.starts_with("image/"))
}

/// 单张图片每边最多的像素数，与社区皮肤服务端的 `maxCandidateSide` 一致。
pub(crate) const MAX_IMAGE_SIDE: u32 = 2048;
/// 一个包里引用图片解码后的像素合计上限，与社区皮肤服务端的 `maxCandidatePixels` 一致。
pub(crate) const MAX_PACKAGE_PIXELS: u64 = 8_000_000;
pub(crate) const IMAGE_DIMENSIONS_TOO_LARGE: &str = "image dimensions exceed limits";

/// 一张 APNG 最多的动画帧数。宿主播放时每帧按整张画布解码成位图，帧数和下面按帧累计的像素一起限住一个包能占的内存。
pub(crate) const MAX_ANIMATION_FRAMES: u32 = 120;
pub(crate) const ANIMATION_TOO_MANY_FRAMES: &str = "animated image has too many frames";

/// 一张 PNG 解码后有几张整幅画面：普通 PNG 和 JPEG 为 1；APNG 为 `acTL` 声明的动画帧数，默认图（`IDAT`）不属于动画时再加上它。返回 `(动画帧数, 解码画面数)`。只读到第一个 `IDAT` 之前的块，不解压任何像素；读不出的 PNG 按 1 帧处理，与上面读不出尺寸的图一样交给宿主原有的处理。
pub(crate) fn png_frames(bytes: &[u8]) -> Result<(u32, u32), &'static str> {
    let Ok(reader) = png::Decoder::new(Cursor::new(bytes)).read_info() else {
        return Ok((1, 1));
    };
    let Some(control) = reader.info().animation_control() else {
        return Ok((1, 1));
    };
    let frames = control.num_frames;
    if frames > MAX_ANIMATION_FRAMES {
        return Err(ANIMATION_TOO_MANY_FRAMES);
    }
    let frames = frames.max(1);
    let thumbnail = u32::from(reader.info().frame_control.is_none());
    Ok((frames, frames + thumbnail))
}

/// GIF 解码时允许分配的内存。逻辑画布每边不超过 [`MAX_IMAGE_SIDE`]，展开成 RGBA 是 16 MiB；逐帧迭代同时持有常驻画布、这一帧的原始缓冲和合成后的整幅画面，所以给三份。`image` 默认不给 GIF 设上限，而帧描述符的宽高不受逻辑画布约束，一个声明 65535×65535 帧的小文件就会让解码器申请约 17 GB，这道上限让它在分配前报错。
const MAX_GIF_DECODE_ALLOC: u64 = MAX_IMAGE_SIDE as u64 * MAX_IMAGE_SIDE as u64 * 4 * 3;

/// 统计 GIF/WebP 的动画帧。这些格式的帧数不在 `ImageReader` 的公共尺寸接口里，但宿主同样会把每帧展开成整张画布，所以和 APNG 一样受帧数与像素上限约束。`dimensions` 是一帧整幅画布的尺寸，`remaining_pixels` 是这个包还剩的像素额度：迭代器是惰性的，一超额度就停，不再解码后面的帧，一个包的解码量因此也不超过 [`MAX_PACKAGE_PIXELS`] 加一帧。连第一帧都读不出的旧资源按静态图处理，保持扫描对旧包的宽容；读出几帧之后才损坏的，按已读出的帧数计，因为宿主照样会播放坏帧之前的部分。帧描述符超出解码内存上限的按尺寸超限拒绝；WebP 另外在解码前核对全部内部帧几何，几何无效时拒绝，不回退为静态图。
fn decoded_animation_frames(
    bytes: &[u8],
    format: image::ImageFormat,
    dimensions: (u32, u32),
    remaining_pixels: u64,
) -> Result<(u32, u32), &'static str> {
    use image::ImageDecoder;
    let frame_pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    let frames = match format {
        image::ImageFormat::Gif => {
            let Ok(mut decoder) = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)) else {
                return Ok((1, 1));
            };
            // `Limits` is #[non_exhaustive]: start from the default and set the field.
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(MAX_GIF_DECODE_ALLOC);
            if decoder.set_limits(limits).is_err() {
                return Err(IMAGE_DIMENSIONS_TOO_LARGE);
            }
            decoder.into_frames()
        }
        image::ImageFormat::WebP => {
            // 解码器会先展开内部位流再检查画布尺寸，必须在这里先核对每一帧。
            super::webp_preflight::check(bytes, dimensions, remaining_pixels).map_err(|error| {
                match error {
                    super::webp_preflight::Error::Invalid => "invalid WebP frame geometry",
                    super::webp_preflight::Error::Limit => IMAGE_DIMENSIONS_TOO_LARGE,
                }
            })?;
            let Ok(decoder) = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)) else {
                return Ok((1, 1));
            };
            decoder.into_frames()
        }
        _ => return Ok((1, 1)),
    };
    let mut count = 0_u32;
    for frame in frames {
        match frame {
            Ok(_) => {}
            Err(image::ImageError::Limits(_)) => return Err(IMAGE_DIMENSIONS_TOO_LARGE),
            Err(_) if count == 0 => return Ok((1, 1)),
            Err(_) => break,
        }
        count = count.saturating_add(1);
        if count > MAX_ANIMATION_FRAMES {
            return Err(ANIMATION_TOO_MANY_FRAMES);
        }
        if frame_pixels.saturating_mul(u64::from(count)) > remaining_pixels {
            return Err(IMAGE_DIMENSIONS_TOO_LARGE);
        }
    }
    let count = count.max(1);
    Ok((count, count))
}

/// 检查清单引用的 PNG/JPEG/GIF/WebP 尺寸，避免宿主把声明为极大尺寸的压缩图片交给系统解码器。动画图的每一帧都按整张画布计入像素合计，帧数不超过 [`MAX_ANIMATION_FRAMES`]，所以一个包解码后的位图总量与静态图同一个上限。返回装饰图的帧数（没有装饰图时为 0）。
/// 不能解析的旧资源仍由宿主按原有方式处理；社区发布和安装会再做完整解码检查。
fn validate_image_dimensions(dir: &Path, summary: &SkinSummary) -> Result<u32, String> {
    let paths = referenced_image_paths(summary);
    let mut pixels = 0_u64;
    let mut decoration_frames = u32::from(summary.decoration_image.is_some());
    for path in paths {
        let input = crate::storage::open_private_file_in(&dir.join(path))
            .map_err(|_| "image is unavailable")?;
        let bytes = crate::bounded_io::read_bounded_file_with(
            input,
            MAX_RESOURCE_BYTES as u64,
            || "image is too large".to_owned(),
            |_| "image is unavailable".to_owned(),
        )?;
        let Ok(reader) = image::ImageReader::new(Cursor::new(&bytes[..])).with_guessed_format()
        else {
            continue;
        };
        let format = reader.format();
        if !matches!(
            format,
            Some(
                image::ImageFormat::Png
                    | image::ImageFormat::Jpeg
                    | image::ImageFormat::Gif
                    | image::ImageFormat::WebP
            )
        ) {
            continue;
        }
        let Ok((width, height)) = reader.into_dimensions() else {
            continue;
        };
        if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
            return Err(IMAGE_DIMENSIONS_TOO_LARGE.into());
        }
        let (frames, images) = match format {
            Some(image::ImageFormat::Png) => png_frames(&bytes)?,
            // `pixels <= MAX_PACKAGE_PIXELS` here: the end of every iteration returns as soon as it is exceeded.
            Some(format @ (image::ImageFormat::Gif | image::ImageFormat::WebP)) => {
                decoded_animation_frames(
                    &bytes,
                    format,
                    (width, height),
                    MAX_PACKAGE_PIXELS - pixels,
                )?
            }
            _ => (1, 1),
        };
        if summary.decoration_image.as_deref() == Some(path) {
            decoration_frames = frames;
        }
        pixels = pixels
            .checked_add(u64::from(width) * u64::from(height) * u64::from(images))
            .ok_or_else(|| IMAGE_DIMENSIONS_TOO_LARGE.to_owned())?;
        if pixels > MAX_PACKAGE_PIXELS {
            return Err(IMAGE_DIMENSIONS_TOO_LARGE.into());
        }
    }
    Ok(decoration_frames)
}

/// 清单生效的图片引用，供扫描预算、资源导出与社区整包共同使用；同一路径只列一次。
pub(crate) fn referenced_image_paths(summary: &SkinSummary) -> Vec<&str> {
    let mut paths = Vec::new();
    for path in [
        summary.decoration_image.as_deref(),
        summary
            .background
            .as_ref()
            .map(|background| background.image.as_str()),
        summary.preview.as_deref().filter(|path| is_image(path)),
        summary
            .border_image
            .as_ref()
            .map(|image| image.source.as_str()),
        summary
            .border_image
            .as_ref()
            .and_then(|image| image.light_source.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
    for decoration in &summary.decorations {
        if !paths.iter().any(|existing| existing == &decoration.image) {
            paths.push(decoration.image.as_str());
        }
    }
    if let Some(toolbar) = &summary.toolbar_image {
        for path in toolbar.image_paths() {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

/// 清单里一张图片在磁盘上的样子，见 [`package_files`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinImageFile {
    /// 清单里写的包内相对路径。
    pub path: String,
    /// 文件的字节数。
    pub bytes: u64,
    /// 按内容认出的编码：`png`、`apng`（多于一帧的 PNG）、`jpeg`、`gif` 或 `webp`；认不出时为 `None`，这样的旧资源由宿主按原有方式处理。
    pub format: Option<&'static str>,
    /// 一帧整幅画布的宽和高；认不出编码或读不出尺寸时为 `None`。
    pub dimensions: Option<(u32, u32)>,
    /// 动画帧数，静态图为 1。
    pub frames: u32,
}

/// 一款已安装皮肤的清单原文和它引用的图片，供修改后整包重装。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinPackageFiles {
    /// `skin.toml` 原文，去掉开头的字节序标记。
    pub manifest: String,
    /// 清单引用的图片，顺序与同一路径只列一次的规则同扫描时一样。
    pub images: Vec<SkinImageFile>,
}

/// 读 `root` 下已安装、能用（与 [`scan`] 同一条规则）的皮肤 `id` 的清单原文和它引用的每张图片。文件经不跟随符号链接的句柄读，清单最多 64 KiB，每张图最多 [`MAX_RESOURCE_BYTES`]；格式、尺寸和帧数按扫描时的同一套方法得出，读不出的旧资源报告为认不出，不算错误。
pub fn package_files(root: impl AsRef<Path>, id: &str) -> Result<SkinPackageFiles, String> {
    let root = root.as_ref();
    let summary = load_package(root, id)?;
    let dir = root.join(id);
    let read = |path: &Path, limit: usize| {
        let input = crate::storage::open_private_file_in(path)
            .map_err(|_| "a skin file is unavailable".to_owned())?;
        crate::bounded_io::read_bounded_file_with(
            input,
            limit as u64,
            || "a skin file is too large".to_owned(),
            |_| "a skin file is unavailable".to_owned(),
        )
    };
    let manifest = String::from_utf8(read(&dir.join("skin.toml"), 65_536)?)
        .map_err(|_| "skin.toml is not UTF-8".to_owned())?;
    let manifest = match manifest.strip_prefix('\u{feff}') {
        Some(body) => body.to_owned(),
        None => manifest,
    };
    let mut images = Vec::new();
    for path in referenced_image_paths(&summary) {
        let bytes = read(&dir.join(path), MAX_RESOURCE_BYTES)?;
        let reader = image::ImageReader::new(Cursor::new(&bytes[..]))
            .with_guessed_format()
            .ok();
        let format = reader.as_ref().and_then(|reader| reader.format());
        let dimensions = reader.and_then(|reader| reader.into_dimensions().ok());
        let frames = match (format, dimensions) {
            (Some(image::ImageFormat::Png), _) => {
                png_frames(&bytes).map_or(1, |(frames, _)| frames)
            }
            (Some(format @ (image::ImageFormat::Gif | image::ImageFormat::WebP)), Some(size)) => {
                decoded_animation_frames(&bytes, format, size, MAX_PACKAGE_PIXELS)
                    .map_or(1, |(frames, _)| frames)
            }
            _ => 1,
        };
        let format = match format {
            Some(image::ImageFormat::Png) if frames > 1 => Some("apng"),
            Some(image::ImageFormat::Png) => Some("png"),
            Some(image::ImageFormat::Jpeg) => Some("jpeg"),
            Some(image::ImageFormat::Gif) => Some("gif"),
            Some(image::ImageFormat::WebP) => Some("webp"),
            _ => None,
        };
        images.push(SkinImageFile {
            path: path.to_owned(),
            bytes: bytes.len() as u64,
            dimensions: format.and(dimensions),
            format,
            frames,
        });
    }
    Ok(SkinPackageFiles { manifest, images })
}

/// 读 `decoration.motion` 和 `decoration.motion_period_ms`。两者都只在 schema 2 里可写，并且只对有装饰的包有意义；版本 1 保持静态，与 `[animation]` 相同。
fn read_decoration_motion(
    decoration: &toml::map::Map<String, Value>,
    decorated: bool,
    schema_version: i64,
) -> Result<(DecorationMotion, u16), String> {
    let motion = decoration.get("motion");
    let period = decoration.get("motion_period_ms");
    if motion.is_none() && period.is_none() {
        return Ok((DecorationMotion::None, DEFAULT_DECORATION_MOTION_PERIOD_MS));
    }
    if schema_version < 2 {
        return Err("decoration motion requires schema_version 2".into());
    }
    let motion = match motion {
        None => DecorationMotion::None,
        Some(value) => match value.as_str() {
            Some("none") => DecorationMotion::None,
            Some("float") => DecorationMotion::Float,
            Some("sway") => DecorationMotion::Sway,
            Some("breathe") => DecorationMotion::Breathe,
            Some("bounce") => DecorationMotion::Bounce,
            _ => return Err("invalid decoration motion".into()),
        },
    };
    if !decorated {
        return Err("invalid decoration motion".into());
    }
    let period = match period {
        None => DEFAULT_DECORATION_MOTION_PERIOD_MS,
        Some(value) => value
            .as_integer()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| DECORATION_MOTION_PERIOD_MS.contains(value))
            .ok_or("invalid decoration motion_period_ms")?,
    };
    Ok((motion, period))
}

/// A package-relative image that exists inside the package directory.
fn image_resource(dir: &Path, relative: &str) -> bool {
    safe_resource(relative, 256) && is_image(relative) && contained(dir, &dir.join(relative))
}

fn read_background(
    dir: &Path,
    table: &toml::map::Map<String, Value>,
) -> Result<SkinBackground, String> {
    let invalid = || "invalid background".to_owned();
    let image = table
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| image_resource(dir, image))
        .ok_or_else(invalid)?
        .to_owned();
    let fit = match table.get("fit") {
        None => BackgroundFit::Cover,
        Some(value) => match value.as_str().ok_or_else(invalid)? {
            "cover" => BackgroundFit::Cover,
            "contain" => BackgroundFit::Contain,
            "stretch" => BackgroundFit::Stretch,
            _ => return Err(invalid()),
        },
    };
    let opacity = match table.get("opacity") {
        None => 1.0,
        Some(value) => number_in(value, 0.0..=1.0).ok_or_else(invalid)?,
    };
    Ok(SkinBackground {
        image,
        fit,
        opacity,
    })
}

fn read_toolbar(table: &toml::map::Map<String, Value>) -> Result<SkinToolbar, String> {
    let invalid = || "invalid toolbar".to_owned();
    let Some(value) = table.get("toolbar") else {
        return Ok(SkinToolbar::default());
    };
    let toolbar = value.as_table().ok_or_else(invalid)?;
    let corner_radius_dip = match toolbar.get("corner_radius_dip") {
        None => None,
        Some(value) => Some(number_in(value, 0.0..=32.0).ok_or_else(invalid)?),
    };
    let palette = |mode: &str| -> Result<ToolbarPalette, String> {
        let Some(value) = toolbar.get(mode) else {
            return Ok(ToolbarPalette::default());
        };
        let colors = value.as_table().ok_or_else(invalid)?;
        let color = |key: &str| -> Result<Option<String>, String> {
            let Some(value) = colors.get(key) else {
                return Ok(None);
            };
            let value = value.as_str().ok_or_else(invalid)?;
            if value.len() > 80 {
                return Err("toolbar color exceeds 80 bytes".into());
            }
            if !css_color_text(value) {
                return Err(invalid());
            }
            Ok(super::theme::normalized_color(value))
        };
        Ok(ToolbarPalette {
            background: color("background")?,
            border: color("border")?,
            handle: color("handle")?,
            divider: color("divider")?,
            icon: color("icon")?,
            hover: color("hover")?,
        })
    };
    Ok(SkinToolbar {
        corner_radius_dip,
        dark: palette("dark")?,
        light: palette("light")?,
    })
}

fn animation_preset(
    value: Option<&Value>,
    key: &str,
    default: SkinAnimationPreset,
) -> Result<SkinAnimationPreset, String> {
    match value {
        None => Ok(default),
        Some(Value::String(value)) => match value.as_str() {
            "none" => Ok(SkinAnimationPreset::None),
            "fade" => Ok(SkinAnimationPreset::Fade),
            "fade_scale" => Ok(SkinAnimationPreset::FadeScale),
            "slide_up" => Ok(SkinAnimationPreset::SlideUp),
            "spring" => Ok(SkinAnimationPreset::Spring),
            "glow" => Ok(SkinAnimationPreset::Glow),
            _ => Err(format!("invalid animation {key}")),
        },
        Some(_) => Err(format!("invalid animation {key}")),
    }
}

/// 解析有限的 v2 动画表。版本 1 保持逐字节兼容，不会因未知字段意外启用动画。
fn read_animation(
    table: &toml::map::Map<String, Value>,
    schema_version: i64,
) -> Result<SkinAnimation, String> {
    let Some(value) = table.get("animation") else {
        return Ok(if schema_version == 2 {
            SkinAnimation::lively()
        } else {
            SkinAnimation::disabled()
        });
    };
    if schema_version < 2 {
        return Err("animation requires schema_version 2".into());
    }
    let animation = value.as_table().ok_or("invalid animation")?;
    let duration_ms = match animation.get("duration_ms") {
        None => 180,
        Some(value) => value
            .as_integer()
            .filter(|value| (40..=600).contains(value))
            .map(|value| value as u16)
            .ok_or("invalid animation duration_ms")?,
    };
    let stagger_ms = match animation.get("stagger_ms") {
        None => 14,
        Some(value) => value
            .as_integer()
            .filter(|value| (0..=80).contains(value))
            .map(|value| value as u16)
            .ok_or("invalid animation stagger_ms")?,
    };
    Ok(SkinAnimation {
        enter: animation_preset(
            animation.get("enter"),
            "enter",
            SkinAnimationPreset::FadeScale,
        )?,
        exit: animation_preset(animation.get("exit"), "exit", SkinAnimationPreset::Fade)?,
        selection: animation_preset(
            animation.get("selection"),
            "selection",
            SkinAnimationPreset::Spring,
        )?,
        duration_ms,
        stagger_ms,
        declared: SkinAnimationDeclared {
            enter: animation.contains_key("enter"),
            exit: animation.contains_key("exit"),
            selection: animation.contains_key("selection"),
        },
    })
}

/// Maximum bytes returned for one skin asset, including stylesheets and fonts.
pub const MAX_RESOURCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ResourceError {
    InvalidPath,
    InvalidPackage,
    UnsupportedType,
    Unavailable,
    TooLarge,
    InvalidEncoding,
}

/// Read only the toolbar stylesheet declared by the current manifest. The
/// caller chooses a package, not a filesystem path. None means inheritance of
/// the built-in toolbar; an empty stylesheet is Some(""). Returned CSS is
/// untrusted and must be parsed/scoped by the host before applying it.
pub fn read_toolbar_stylesheet(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<Option<String>, ResourceError> {
    if !safe_id(id) {
        return Err(ResourceError::InvalidPath);
    }
    let root = root.as_ref();
    let directory = root.join(id);
    if !fs::symlink_metadata(directory)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        return Err(ResourceError::InvalidPackage);
    }
    let package = load(root, id).map_err(|_| ResourceError::InvalidPackage)?;
    let Some(relative) = package.toolbar_stylesheet else {
        return Ok(None);
    };
    read_stylesheet(root, id, &relative).map(Some)
}

/// Read one package-local CSS resource. This is also the boundary used for
/// relative `@import` rules: callers never submit a filesystem path, and every
/// imported sheet is revalidated against the current manifest and package.
pub fn read_stylesheet(
    root: impl AsRef<Path>,
    id: &str,
    relative: &str,
) -> Result<String, ResourceError> {
    let resource = read_resource(root, id, relative)?;
    if resource.content_type != "text/css; charset=utf-8" {
        return Err(ResourceError::UnsupportedType);
    }
    let text = String::from_utf8(resource.bytes).map_err(|_| ResourceError::InvalidEncoding)?;
    // A UTF-8 BOM is an encoding marker, not part of the first selector.
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

/// Untrusted resource data. Hosts must set the content type, disable MIME
/// sniffing, and isolate styles/SVG rather than inserting them as page markup.
#[derive(Debug, PartialEq, Eq)]
pub struct SkinResource {
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The content type a package resource is served with, by extension; `None` for a type packages may not ship.
fn resource_content_type(relative: &str) -> Option<&'static str> {
    Some(
        match relative
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "css" => "text/css; charset=utf-8",
            "png" | "apng" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "bmp" => "image/bmp",
            "avif" => "image/avif",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            _ => return None,
        },
    )
}

/// Read an asset from a currently valid package under a host-selected root.
/// Paths are decoded relative names, never URLs. Canonical containment rejects
/// symlink escapes, but is not a sandbox against concurrent hostile filesystem
/// mutation; hosts must not use this API to expose attacker-writable trees
/// across a privilege boundary.
pub fn read_resource(
    root: impl AsRef<Path>,
    id: &str,
    relative: &str,
) -> Result<SkinResource, ResourceError> {
    if !safe_id(id) || !safe_resource(relative, 256) {
        return Err(ResourceError::InvalidPath);
    }
    let content_type = resource_content_type(relative).ok_or(ResourceError::UnsupportedType)?;
    let root = root.as_ref();
    let directory = root.join(id);
    // Match scan(): symlinked package directories are not catalog entries.
    if !fs::symlink_metadata(&directory)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
        || load(root, id).is_err()
    {
        return Err(ResourceError::InvalidPackage);
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| ResourceError::Unavailable)?;
    let target = directory
        .join(relative)
        .canonicalize()
        .map_err(|_| ResourceError::Unavailable)?;
    if !target.starts_with(&directory) {
        return Err(ResourceError::InvalidPath);
    }
    if !target.is_file() {
        return Err(ResourceError::Unavailable);
    }
    let input =
        crate::storage::open_private_file_in(&target).map_err(|_| ResourceError::Unavailable)?;
    let metadata = input.metadata().map_err(|_| ResourceError::Unavailable)?;
    if !metadata.is_file() {
        return Err(ResourceError::Unavailable);
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        input,
        MAX_RESOURCE_BYTES as u64,
        || ResourceError::TooLarge,
        |_| ResourceError::Unavailable,
    )?;
    Ok(SkinResource {
        content_type,
        bytes,
    })
}

/// Validate one installed package with the same rules `scan` applies, without reading the rest of the root. Native presenters use this to resolve the selected skin, so a package the settings page lists as valid is the one they draw. Like `scan`, a symlinked package directory is not a package.
pub fn load_package(root: impl AsRef<Path>, id: &str) -> Result<SkinSummary, String> {
    let root = root.as_ref();
    if !safe_id(id) || is_reserved(id) {
        return Err("invalid skin id".into());
    }
    if !fs::symlink_metadata(root.join(id))
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        return Err("missing skin directory".into());
    }
    load(root, id)
}

pub fn scan(root: impl AsRef<Path>) -> SkinCatalog {
    let root = root.as_ref();
    let mut out = SkinCatalog::default();
    if crate::storage::reject_symlink(root).is_err() {
        return out;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        match load(root, &folder) {
            Ok(package) => out.packages.push(package),
            Err(reason) => out.issues.push(SkinIssue { folder, reason }),
        }
    }
    out.packages.sort_by(|a, b| a.name.cmp(&b.name));
    out.issues.sort_by(|a, b| a.folder.cmp(&b.folder));
    out
}

/// Most installed skins published to hosts that draw the candidate panel from `candidate_skin_catalog` (the Linux IBus and Fcitx5 hosts). Beyond this a skin menu is no longer usable, and every entry costs the document those hosts read whole.
pub const HOST_CATALOG_MAX_PACKAGES: usize = 32;

/// 原生候选宿主读取的已安装皮肤目录（Linux 运行时选项中的 `candidate_skin_catalog`）。
/// 每个条目包含 `id`、清单名称 `title`、全局主题 `base`、声明的 `layouts`、各主题的候选配色、
/// `show_selected_bar`、可选的 `corner_radius_dip`、Windows 基底的 `resolvedLayout`，以及已声明装饰的尺寸、绝对图片路径和非默认对齐。
/// 配色由 `theme::normalized_color` 规范化；把条目作为 `package` 传给 `msime_client_resolve_theme`，
/// 与从皮肤根目录加载同一个包的解析结果一致。`root` 必须对应扫描 `catalog` 的目录。
///
/// 严格条目不携带背景、工具栏、其他路径或样式表；`resolvedLayout` 只含已校验的数字和开关，以兼容旧宿主并控制整份文档的大小。
/// 没有装饰的包不写装饰键；不可解析的颜色省略，只保留清单声明主题的配色。
/// `scan` 已限制标识与名称，宿主仍复核它们和装饰尺寸；布局与明暗支持范围沿用共享解析器的规则。
///
/// 最多列出 `HOST_CATALOG_MAX_PACKAGES` 个，按目录顺序。`selected` 是自定义主题选中的皮肤（浅色、深色两个槽位）：已安装的一律列出，落在上限之外时挤掉上限内最后几个没选中的，排到末尾，因为屏幕上画的是它们的颜色。
/// 含 canvas 包且根目录为绝对 UTF-8 路径时，顶层另带 `directory`，供 Fcitx5 按需调用包 API。
/// 旧宿主继续只读 `packages`；它们严格解析的条目不增加 canvas 字段，配色仍是可读回退。
pub fn host_candidate_catalog(
    catalog: &SkinCatalog,
    root: &Path,
    selected: &[&str],
) -> serde_json::Value {
    let is_selected = |package: &SkinSummary| selected.contains(&package.id.as_str());
    let mut listed = Vec::with_capacity(HOST_CATALOG_MAX_PACKAGES + selected.len());
    listed.extend(
        catalog
            .packages
            .iter()
            .enumerate()
            .filter(|(index, package)| *index < HOST_CATALOG_MAX_PACKAGES || is_selected(package))
            .map(|(_, package)| package),
    );
    while listed.len() > HOST_CATALOG_MAX_PACKAGES {
        // 选中的比上限还多时（只有两个槽位，实际不会发生）按目录顺序截断，上限照样成立。
        let Some(dropped) = listed.iter().rposition(|package| !is_selected(package)) else {
            listed.truncate(HOST_CATALOG_MAX_PACKAGES);
            break;
        };
        listed.remove(dropped);
    }
    let mut packages = Vec::with_capacity(HOST_CATALOG_MAX_PACKAGES);
    packages.extend(listed.into_iter().map(|package| {
            let mut entry = serde_json::json!({ "id": package.id, "title": package.name, "base": package.base, "layouts": package.layouts });
            if let Some(layout) = &package.resolved_layout {
                entry["resolvedLayout"] = serde_json::to_value(layout).expect("布局结果可序列化");
            }
            let mut candidate = serde_json::Map::new();
            for (theme, palette) in [
                ("light", &package.candidate.light),
                ("dark", &package.candidate.dark),
            ] {
                if !package.supports_theme(theme) {
                    continue;
                }
                // A declared mode is published even with no colour, so the entry still says the package may be drawn in it.
                candidate.insert(
                    theme.to_owned(),
                    serde_json::Value::Object(host_palette(palette)),
                );
            }
            if !candidate.is_empty() {
                entry["candidate"] = serde_json::Value::Object(candidate);
            }
            if let Some(image) = host_decoration_image(root, package) {
                entry["decoration_top_dip"] = package.decoration_top_dip.into();
                entry["decoration_width_dip"] = package.decoration_width_dip.into();
                entry["decoration_image"] = image.into();
                if package.decoration_align != DecorationAlign::Right {
                    entry["decoration_align"] = serde_json::to_value(package.decoration_align)
                        .expect("an enum serializes");
                }
            }
            if let Some(radius) = package.corner_radius_dip {
                entry["corner_radius_dip"] = radius.into();
            }
            entry
        }));
    let mut published = serde_json::json!({ "packages": packages });
    // canvas 宿主按需复用已校验的包 API；顶层扩展不改变旧宿主严格读取的配色条目。
    if catalog.packages.iter().any(|package| package.canvas) && root.is_absolute() {
        if let Some(directory) = root.to_str() {
            published["directory"] = directory.into();
        }
    }
    published
}

/// The image a decorated package draws above its candidate window: its `decoration_image` (the manifest's `decoration.image`, or else its preview, as on Windows where candidate_presenter.cpp draws `<skins>/<id>/<preview>` in a band `decoration_top_dip` tall). `scan` has already confined it to the package directory and checked it is an image. No decoration is published when the path would not be absolute or not UTF-8, since the host could not open it.
fn host_decoration_image(root: &Path, package: &SkinSummary) -> Option<String> {
    let image = package.decoration_image.as_deref()?;
    let path = root.join(&package.id).join(image);
    if !path.is_absolute() {
        return None;
    }
    path.to_str().map(str::to_owned)
}

fn host_palette(palette: &CandidatePalette) -> serde_json::Map<String, serde_json::Value> {
    let mut colors = serde_json::Map::new();
    for (key, value) in [
        ("surface", &palette.surface),
        ("border", &palette.border),
        ("text", &palette.text),
        ("number", &palette.number),
        ("accent", &palette.accent),
        ("selected", &palette.selected),
        ("hover", &palette.hover),
        ("translation", &palette.translation),
    ] {
        if let Some(value) = value.as_deref().and_then(super::theme::normalized_color) {
            colors.insert(key.to_owned(), value.into());
        }
    }
    if let Some(value) = palette.show_selected_bar {
        colors.insert("show_selected_bar".to_owned(), value.into());
    }
    colors
}

#[cfg(test)]
mod tests;

/// `tests/fixtures/candidate-skins` 下验证皮肤的解析结果。
#[cfg(test)]
mod gallery_tests;

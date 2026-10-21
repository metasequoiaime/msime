//! The global theme: one id that colours the candidate window, the floating toolbar, the menus and the touch keyboard on every host.
//!
//! 共有八个 id。`system`（跟随系统）不带调色板，由宿主自己取色：桌面宿主画平台 token，触屏键盘按应用主题当前季节取色；`native`（原生）同样不带调色板，只画宿主系统自带的键盘 token，不跟季节，目前只有 iOS 提供；`shuishan`、`light`、`paper`、`night` 和 `ink` 是内置调色板，照抄设计稿的 `THEMES` 表；`custom` 是用户在 `Preferences::custom_theme` 里组合出来的主题（外部候选皮肤包、七个候选颜色取色器，以及编辑器、社区皮肤库或 AI 生成的键盘设计）。
//!
//! Hosts do not keep their own copy of these palettes. They read the catalog (`catalog`) to draw the picker and call `resolve` for the colours on screen, so a palette is changed here once and every host follows.
//!
//! Every colour this module emits is `#RRGGBB` or `#RRGGBBAA` (uppercase, alpha last). A `None` slot means "the host's own platform token for that slot", never "transparent".

use super::catalog::{CandidatePalette, ResolvedSkinLayout, SkinSummary};
use crate::host_surface::HostPlatform;
use crate::preferences::{CandidateLayout, CustomTheme, TouchKeyboardSkinDesign};
use crate::theme::{readable_text, rgb};
use serde::{Deserialize, Serialize};

/// 全局主题 id 和内置调色板定义在 `crate::theme`（偏好的 `global_theme` 要用，放在这里会让 `preferences` 依赖皮肤模块），从这里导回，原来的路径照常可用。
pub use crate::theme::{
    BuiltinTheme, CandidateThemePalette, GlobalTheme, KeyboardThemePalette, ThemeAppearance,
    ThemeCatalogEntry, ThemePreview, BUILTIN_CANDIDATE_BORDER, BUILTIN_THEMES,
    CUSTOM_KEYBOARD_SECONDARY_ALPHA, HOVER_ALPHA, PICKED_NUMBER_ALPHA, SELECTED_ALPHA,
};

/// Every theme, in picker order, including the ones only some hosts offer (each entry says which in `platforms`). `system`, `native` and `custom` carry no palette here: `system` and `native` never have one, and `custom` has one only once `resolve` combines it with the user's `custom_theme`.
pub fn catalog() -> Vec<ThemeCatalogEntry> {
    GlobalTheme::ALL
        .into_iter()
        .map(|id| {
            let builtin = id.builtin();
            ThemeCatalogEntry {
                id,
                title: id.title(),
                platforms: id.platforms(),
                appearance: builtin.map(|theme| theme.appearance),
                preview: builtin.map(BuiltinTheme::preview),
                candidate: builtin.map(BuiltinTheme::candidate),
                keyboard: builtin.map(BuiltinTheme::keyboard),
            }
        })
        .collect()
}

/// `platform` 的选择器列出的主题，顺序同 [`catalog`]。
pub fn catalog_for(platform: HostPlatform) -> Vec<ThemeCatalogEntry> {
    catalog()
        .into_iter()
        .filter(|entry| entry.id.is_offered_on(platform))
        .collect()
}

/// Where the resolved colours came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSource {
    System,
    Builtin,
    Custom,
}

/// The candidate colours of an installed external skin package, as far as a theme needs them, and where its manifest says it may be drawn. Built from a `SkinSummary` (hosts that scan the skin root) or from an entry of the Linux `candidate_skin_catalog` (hosts that read the published catalog).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemePackage {
    pub id: String,
    pub base: GlobalTheme,
    /// The candidate layouts the manifest declares (`supports.layouts`). In any other layout the package is not drawn.
    pub layouts: Vec<CandidateLayout>,
    /// `None` when the package does not declare light support.
    pub light: Option<CandidatePalette>,
    /// `None` when the package does not declare dark support.
    pub dark: Option<CandidatePalette>,
    /// Windows 基底皮肤的出厂版式继承结果；非 Windows 基底为 `None`。
    pub resolved_layout: Option<ResolvedSkinLayout>,
}

impl From<&SkinSummary> for ThemePackage {
    fn from(summary: &SkinSummary) -> Self {
        let declared = |mode: &str| summary.themes.iter().any(|theme| theme == mode);
        let mut layouts = Vec::with_capacity(summary.layouts.len());
        // `scan` accepts only these two names, so nothing is dropped here.
        layouts.extend(
            summary
                .layouts
                .iter()
                .filter_map(|layout| match layout.as_str() {
                    "horizontal" => Some(CandidateLayout::Horizontal),
                    "vertical" => Some(CandidateLayout::Vertical),
                    _ => None,
                }),
        );
        Self {
            id: summary.id.clone(),
            base: summary.base,
            layouts,
            light: declared("light").then(|| summary.candidate.light.clone()),
            dark: declared("dark").then(|| summary.candidate.dark.clone()),
            resolved_layout: summary.resolved_layout.clone(),
        }
    }
}

impl ThemePackage {
    /// 这个皮肤包能否在 `dark` 模式下绘制：`base` 是固定明暗的内置主题时只在那种明暗下画，`system` 底两种都画。设置页按同一规则把皮肤分到浅色、深色两个槽位（`skin_appearance`）。
    pub fn draws_in(&self, dark: bool) -> bool {
        skin_appearance(self.base)
            .is_none_or(|appearance| (appearance == ThemeAppearance::Dark) == dark)
    }

    /// Read one entry of the published `candidate_skin_catalog` (see `catalog::host_candidate_catalog`). That entry carries a palette for exactly the modes the package declares, so a missing mode reads as undeclared. Its `title`, decoration, corner radius, row radius, background and toolbar keys are not candidate theme colours and are ignored. The entry is read strictly: an unknown key anywhere, a missing `layouts`, a `base` that is not `system` or a built-in theme (as `catalog::scan` refuses the manifest) or an unsafe id is refused. In particular a `SkinSummary` from `msime_client_skin_catalog`, whose keys are camelCase (`showSelectedBar`, `minWidthDip`, `themes`), is not an entry and is refused rather than read with its selection bar and declared modes silently lost; hosts that scan the skin root pass `skins_directory` instead.
    pub fn from_host_catalog_entry(entry: serde_json::Value) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Entry {
            id: String,
            #[serde(rename = "title")]
            _title: String,
            base: String,
            layouts: Vec<CandidateLayout>,
            #[serde(default)]
            candidate: EntryCandidate,
            #[serde(default, rename = "decoration_top_dip")]
            _decoration_top_dip: serde::de::IgnoredAny,
            #[serde(default, rename = "decoration_width_dip")]
            _decoration_width_dip: serde::de::IgnoredAny,
            #[serde(default, rename = "decoration_image")]
            _decoration_image: serde::de::IgnoredAny,
            #[serde(default, rename = "decoration_align")]
            _decoration_align: serde::de::IgnoredAny,
            #[serde(default, rename = "corner_radius_dip")]
            _corner_radius_dip: serde::de::IgnoredAny,
            #[serde(default, rename = "item_corner_radius_dip")]
            _item_corner_radius_dip: serde::de::IgnoredAny,
            #[serde(default, rename = "background_image")]
            _background_image: serde::de::IgnoredAny,
            #[serde(default, rename = "background_fit")]
            _background_fit: serde::de::IgnoredAny,
            #[serde(default, rename = "background_opacity")]
            _background_opacity: serde::de::IgnoredAny,
            #[serde(default, rename = "toolbar")]
            _toolbar: serde::de::IgnoredAny,
            #[serde(default, rename = "resolvedLayout")]
            resolved_layout: Option<ResolvedSkinLayout>,
        }
        #[derive(Default, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EntryCandidate {
            light: Option<EntryPalette>,
            dark: Option<EntryPalette>,
        }
        /// The palette keys `host_palette` writes. `CandidatePalette` itself is not used here: it reads the manifest's `show_selected_bar` but writes `showSelectedBar`, and it ignores unknown keys, so a palette in the other shape would lose its selection bar without an error.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EntryPalette {
            surface: Option<String>,
            border: Option<String>,
            text: Option<String>,
            number: Option<String>,
            accent: Option<String>,
            selected: Option<String>,
            hover: Option<String>,
            translation: Option<String>,
            show_selected_bar: Option<bool>,
        }
        impl From<EntryPalette> for CandidatePalette {
            fn from(palette: EntryPalette) -> Self {
                Self {
                    accent: palette.accent,
                    selected: palette.selected,
                    hover: palette.hover,
                    surface: palette.surface,
                    border: palette.border,
                    text: palette.text,
                    number: palette.number,
                    translation: palette.translation,
                    show_selected_bar: palette.show_selected_bar,
                    ..CandidatePalette::default()
                }
            }
        }
        let entry: Entry =
            serde_json::from_value(entry).map_err(|_| "invalid candidate skin catalog entry")?;
        let base = GlobalTheme::from_id(&entry.base).filter(|base| base.is_base());
        let (Some(base), true) = (base, super::catalog::is_external_id(&entry.id)) else {
            return Err("invalid candidate skin catalog entry".into());
        };
        Ok(Self {
            id: entry.id,
            base,
            layouts: entry.layouts,
            light: entry.candidate.light.map(Into::into),
            dark: entry.candidate.dark.map(Into::into),
            resolved_layout: entry.resolved_layout,
        })
    }
}

/// 宿主可选消费的细分文字与装饰颜色。留空仍沿用该宿主既有的 token。
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct CandidateDetails {
    pub preedit_text: Option<String>,
    pub preedit_caret: Option<String>,
    pub selected_translation: Option<String>,
    pub selected_bar: Option<String>,
    /// 显式释义色与分段色；不把 `secondary` 的序号默认色误当成皮肤声明的释义色。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_translation_pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_translation_separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_divider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_arrow_divider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_surface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_hover: Option<String>,
}

/// The colours a host draws for the selected theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedTheme {
    pub id: GlobalTheme,
    pub source: ThemeSource,
    /// `None` means follow the host's current mode.
    pub appearance: Option<ThemeAppearance>,
    /// `system` 和 `native` 为 `None`：画平台 token。
    pub candidate: Option<CandidateThemePalette>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_details: Option<CandidateDetails>,
    /// `system` 和 `native` 为 `None`：画平台键盘。
    pub keyboard: Option<KeyboardThemePalette>,
    /// The external package drawn on this surface: the custom theme names it, it was found, and its manifest declares both `layout` and the mode drawn. Hosts draw the package's decoration and minimum width only when this is set.
    pub candidate_skin: Option<String>,
    /// 当前候选排列对应的 Windows 兼容版式；旧包和非 Windows 基底保持为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_layout: Option<ResolvedSkinLayout>,
}

/// 以 `base` 为底的皮肤包属于哪种明暗：固定明暗的内置主题给出它自己的明暗，`system` 为 `None`，两种模式都可以画。
pub fn skin_appearance(base: GlobalTheme) -> Option<ThemeAppearance> {
    base.builtin().map(|builtin| builtin.appearance)
}

/// Resolve the colours for `theme`.
///
/// `dark` 是宿主正在绘制的界面当前的明暗模式：它决定自定义主题取哪个槽位的皮肤包，`system` 底也按它取色。`layout` 是该界面的候选排列，只有自定义主题的皮肤包读它。`package` 是调用方已经加载好的、`custom.candidate_skin_for(dark)` 指名的那个已安装皮肤包（自定义主题没指名或没安装时为 `None`）；id 对不上的包被忽略。
///
/// 自定义主题画在一个底上：选中皮肤包清单里的 `base`，否则是 `custom.base`。皮肤包只在它 `base` 的明暗下绘制：`base` 是固定明暗的内置主题、且与 `dark` 不符时，这个模式不画它，主题落回底加取色器，所以浅色模式不会出现深色皮肤，反之亦然。设过皮肤却在这个模式下没有可画的包时，`custom.base` 也只在属于当前明暗时作底，否则按 `system`；完全没设皮肤的自定义主题照旧用 `custom.base`。`system` 底的皮肤包两种模式都画，按 `dark` 取对应配色。皮肤包也只在清单声明的排列和模式下绘制：其余情况下它不提供任何颜色，也不出现在 `candidate_skin` 里，底照常与取色器一起画。`system` 底不提供槽位颜色，按 `dark` 绘制。
///
/// Candidate colours are layered: the base, then the package palette, then every picker the user set. A text picker also sets the numbers to that colour at `PICKED_NUMBER_ALPHA` unless the number picker is set. `secondary` is the package's `translation` colour, and otherwise follows `number`. Over a built-in base the slots that base derives keep following their sources unless the package or a picker set them: `selected` is `accent` at `SELECTED_ALPHA`, `hover` is `text` at `HOVER_ALPHA`, `selected_text` is `accent` and `selected_number` is `number`. When the selected picker is set, on any base, `selected_text` is black or white by that colour's luminance and `selected_number` is the same colour at `PICKED_NUMBER_ALPHA`, so the highlighted candidate stays readable whatever colour was picked. A custom theme over `system` with no package slots and no pickers has no candidate palette at all and draws the platform's own.
///
/// The keyboard is the user's design when there is one, otherwise the base theme's keyboard (`None`, the platform keyboard, over `system`).
pub fn resolve(
    theme: GlobalTheme,
    custom: &CustomTheme,
    dark: bool,
    layout: CandidateLayout,
    package: Option<&ThemePackage>,
) -> ResolvedTheme {
    if let Some(builtin) = theme.builtin() {
        return ResolvedTheme {
            id: theme,
            source: ThemeSource::Builtin,
            appearance: Some(builtin.appearance),
            candidate: Some(builtin.candidate()),
            keyboard: Some(builtin.keyboard()),
            candidate_skin: None,
            candidate_details: None,
            candidate_layout: None,
        };
    }
    // `native` 与 `system` 一样不带调色板，两者的区别只在宿主怎么画：`id` 原样带回，宿主据此分辨。
    if matches!(theme, GlobalTheme::System | GlobalTheme::Native) {
        return ResolvedTheme {
            id: theme,
            source: ThemeSource::System,
            appearance: None,
            candidate: None,
            keyboard: None,
            candidate_skin: None,
            candidate_details: None,
            candidate_layout: None,
        };
    }
    let package = package
        .filter(|package| custom.candidate_skin_for(dark) == Some(package.id.as_str()))
        .filter(|package| package.draws_in(dark));
    let base = match package {
        Some(package) => package.base,
        // 设过皮肤、这个模式却没有能画的（槽位空着、明暗不符或包没装）：底也按同一条明暗规则，不属于当前明暗就跟随系统。应用皮肤时会把包的 `base` 写进 `custom.base`，不这样做，浅色模式会落回深色皮肤留下的 `night`。
        None if custom.candidate_skin.is_some() || custom.candidate_skin_dark.is_some() => {
            if skin_appearance(custom.base)
                .is_none_or(|appearance| (appearance == ThemeAppearance::Dark) == dark)
            {
                custom.base
            } else {
                GlobalTheme::System
            }
        }
        None => custom.base,
    }
    .builtin();
    let dark = base.map_or(dark, |base| base.appearance == ThemeAppearance::Dark);
    let drawn = package
        .filter(|package| package.layouts.contains(&layout))
        .and_then(|package| {
            if dark {
                package.dark.as_ref()
            } else {
                package.light.as_ref()
            }
            .map(|palette| (package, palette))
        });
    let mut explicit = Overrides::default();
    if let Some((_, palette)) = drawn {
        explicit.apply(Overrides::from_package(palette));
    }
    let pickers = Overrides::from_pickers(&custom.candidate_colors);
    // 选中底色取色器优先于包内的选中文字色；按实际底色推导黑白文字，避免旧的贴图配色落在新底色上失去对比。
    let picked_selected_text = pickers.selected.as_deref().map(|selected| {
        let background = u32::from_str_radix(&selected[1..7], 16).unwrap_or_default();
        rgb(readable_text(background))
    });
    let picked_number = match (&pickers.text, &pickers.number) {
        (Some(text), None) => Some(with_alpha(text, PICKED_NUMBER_ALPHA)),
        _ => None,
    };
    let picked_preedit_text = pickers.text.clone();
    explicit.apply(pickers);
    if picked_number.is_some() {
        explicit.number = picked_number;
    }
    let base_palette = base.map(BuiltinTheme::candidate).unwrap_or_default();
    let derived = base.is_some();
    let text = explicit.text.or(base_palette.text);
    let number = explicit.number.or(base_palette.number);
    let accent = explicit.accent.or(base_palette.accent);
    let candidate_details = drawn.and_then(|(_, palette)| {
        let selected_detail = |value: &Option<String>| {
            color(value).map(|authored| {
                picked_selected_text
                    .as_deref()
                    .map(|text| with_alpha(text, PICKED_NUMBER_ALPHA))
                    .unwrap_or(authored)
            })
        };
        let selected_translation = selected_detail(&palette.selected_translation);
        let details = CandidateDetails {
            preedit_text: picked_preedit_text
                .clone()
                .or_else(|| color(&palette.preedit_text)),
            preedit_caret: color(&palette.preedit_caret),
            translation: color(&palette.translation),
            translation_pos: color(&palette.translation_pos),
            translation_separator: color(&palette.translation_separator),
            selected_translation_pos: selected_detail(&palette.selected_translation_pos)
                .or_else(|| selected_translation.clone()),
            selected_translation_separator: selected_detail(
                &palette.selected_translation_separator,
            )
            .or_else(|| selected_translation.clone()),
            selected_translation,
            selected_bar: color(&palette.selected_bar),
            preedit_background: color(&palette.preedit_background),
            preedit_divider: color(&palette.preedit_divider),
            page_arrow_divider: color(&palette.page_arrow_divider),
            menu_surface: palette
                .menu
                .as_ref()
                .and_then(|menu| color(&menu.background)),
            menu_border: palette.menu.as_ref().and_then(|menu| color(&menu.border)),
            menu_text: palette.menu.as_ref().and_then(|menu| color(&menu.text)),
            menu_hover: palette.menu.as_ref().and_then(|menu| color(&menu.hover)),
        };
        (details != CandidateDetails::default()).then_some(details)
    });
    let candidate_layout = drawn.and_then(|(package, _)| package.resolved_layout.clone());
    let candidate = CandidateThemePalette {
        surface: explicit.surface.or(base_palette.surface),
        border: explicit.border.or(base_palette.border),
        secondary: explicit.translation.or_else(|| number.clone()),
        selected: explicit.selected.or_else(|| {
            accent
                .as_deref()
                .filter(|_| derived)
                .map(|accent| with_alpha(accent, SELECTED_ALPHA))
        }),
        selected_number: match &picked_selected_text {
            Some(text) => Some(with_alpha(text, PICKED_NUMBER_ALPHA)),
            None => explicit
                .selected_number
                .or_else(|| number.clone().filter(|_| derived)),
        },
        selected_text: picked_selected_text
            .or(explicit.selected_text)
            .or_else(|| accent.clone().filter(|_| derived)),
        hover: explicit.hover.or_else(|| {
            text.as_deref()
                .filter(|_| derived)
                .map(|text| with_alpha(text, HOVER_ALPHA))
        }),
        show_selected_bar: explicit.show_selected_bar,
        text,
        number,
        accent,
    };
    ResolvedTheme {
        id: theme,
        source: ThemeSource::Custom,
        appearance: base.map(|theme| theme.appearance),
        candidate: (candidate != CandidateThemePalette::default()).then_some(candidate),
        keyboard: custom
            .keyboard
            .as_ref()
            .map(custom_keyboard)
            .or_else(|| base.map(BuiltinTheme::keyboard)),
        candidate_skin: drawn.map(|(package, _)| package.id.clone()),
        candidate_details,
        candidate_layout,
    }
}

/// The candidate slots a package palette or the pickers set explicitly, normalized; unset and unparseable values are `None`.
#[derive(Default)]
struct Overrides {
    selected_text: Option<String>,
    selected_number: Option<String>,
    surface: Option<String>,
    border: Option<String>,
    text: Option<String>,
    number: Option<String>,
    accent: Option<String>,
    selected: Option<String>,
    hover: Option<String>,
    /// Only a package sets it; there is no translation picker.
    translation: Option<String>,
    show_selected_bar: Option<bool>,
}

impl Overrides {
    fn from_package(palette: &CandidatePalette) -> Self {
        Self {
            surface: color(&palette.surface),
            border: color(&palette.border),
            text: color(&palette.candidate_text).or_else(|| color(&palette.text)),
            selected_text: color(&palette.selected_text),
            selected_number: color(&palette.selected_number),
            number: color(&palette.number),
            accent: color(&palette.accent),
            selected: color(&palette.selected),
            hover: color(&palette.hover),
            translation: color(&palette.translation),
            show_selected_bar: palette.show_selected_bar,
        }
    }

    fn from_pickers(pickers: &crate::preferences::CustomCandidateColors) -> Self {
        Self {
            surface: color(&pickers.surface),
            border: color(&pickers.border),
            text: color(&pickers.text),
            number: color(&pickers.number),
            accent: color(&pickers.accent),
            selected: color(&pickers.selected),
            hover: color(&pickers.hover),
            translation: None,
            show_selected_bar: None,
            selected_text: None,
            selected_number: None,
        }
    }

    /// Lay `top` over `self`: every slot `top` sets wins.
    fn apply(&mut self, top: Self) {
        fn over<T>(target: &mut Option<T>, value: Option<T>) {
            if value.is_some() {
                *target = value;
            }
        }
        over(&mut self.selected_text, top.selected_text);
        over(&mut self.selected_number, top.selected_number);
        over(&mut self.surface, top.surface);
        over(&mut self.border, top.border);
        over(&mut self.text, top.text);
        over(&mut self.number, top.number);
        over(&mut self.accent, top.accent);
        over(&mut self.selected, top.selected);
        over(&mut self.hover, top.hover);
        over(&mut self.translation, top.translation);
        over(&mut self.show_selected_bar, top.show_selected_bar);
    }
}

fn color(value: &Option<String>) -> Option<String> {
    value.as_deref().and_then(normalized_color)
}

/// `color` (already `#RRGGBB` or `#RRGGBBAA`) with its alpha replaced by `alpha`.
fn with_alpha(color: &str, alpha: &str) -> String {
    format!("{}{alpha}", &color[..7])
}

/// The flattened palette of a keyboard editor design. Hosts that render the full design (photo, gradient, key shape and material) read `Preferences::custom_theme.keyboard` directly; this is the colour summary every host can draw, and what a touch host's candidate strip uses.
pub fn custom_keyboard(design: &TouchKeyboardSkinDesign) -> KeyboardThemePalette {
    let text = rgb(design.key_foreground);
    KeyboardThemePalette {
        background: rgb(design.background),
        key: rgb(design.key_background),
        function_key: rgb(design.action_background),
        secondary: format!("{text}{CUSTOM_KEYBOARD_SECONDARY_ALPHA}"),
        text,
        accent: rgb(design.accent),
        on_accent: rgb(readable_text(design.accent & 0xFF_FFFF)),
    }
}

/// An external package colour in the one form the theme contract emits (`#RRGGBB` or `#RRGGBBAA`, uppercase), or `None` for anything a host could not parse the same way. Packages written for the Windows presenter may also use `#RGB`, `#RGBA` (alpha last, as in `#RRGGBBAA`), `rgb()`/`rgba()` with 0-255 channels and a 0-1 alpha, and `transparent`; those are converted rather than dropped.
pub fn normalized_color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Some("#00000000".to_owned());
    }
    if let Some(hex) = value.strip_prefix('#') {
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        return match hex.len() {
            3 | 4 => Some(format!(
                "#{}",
                hex.chars()
                    .flat_map(|digit| [digit, digit])
                    .collect::<String>()
                    .to_ascii_uppercase()
            )),
            6 | 8 => Some(format!("#{}", hex.to_ascii_uppercase())),
            _ => None,
        };
    }
    let lower = value.to_ascii_lowercase();
    let (arguments, with_alpha) = if let Some(rest) = lower.strip_prefix("rgba(") {
        (rest.strip_suffix(')')?, true)
    } else {
        (lower.strip_prefix("rgb(")?.strip_suffix(')')?, false)
    };
    let parts: Vec<&str> = arguments.split(',').map(str::trim).collect();
    if parts.len() != if with_alpha { 4 } else { 3 } {
        return None;
    }
    let mut channels = [0u8; 3];
    for (channel, part) in channels.iter_mut().zip(&parts) {
        *channel = part.parse().ok()?;
    }
    let [red, green, blue] = channels;
    if !with_alpha {
        return Some(format!("#{red:02X}{green:02X}{blue:02X}"));
    }
    let alpha: f64 = parts[3].parse().ok()?;
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    let alpha = (alpha * 255.0).round() as u8;
    Some(format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}"))
}

#[cfg(test)]
mod tests;

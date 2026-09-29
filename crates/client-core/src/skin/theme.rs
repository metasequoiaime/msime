//! The global theme: one id that colours the candidate window, the floating toolbar, the menus and the touch keyboard on every host.
//!
//! There are seven ids. `system` leaves every colour to the host's own platform tokens; `shuishan`, `light`, `paper`, `night` and `ink` are the built-in palettes, copied from the design's `THEMES` table; `custom` is whatever the user assembled in `Preferences::custom_theme` (an external candidate skin package, the seven candidate colour pickers, and the keyboard design produced by the editor, the community library or the AI generator).
//!
//! Hosts do not keep their own copy of these palettes. They read the catalog (`catalog`) to draw the picker and call `resolve` for the colours on screen, so a palette is changed here once and every host follows.
//!
//! Every colour this module emits is `#RRGGBB` or `#RRGGBBAA` (uppercase, alpha last). A `None` slot means "the host's own platform token for that slot", never "transparent".

use super::catalog::{CandidatePalette, SkinSummary};
use crate::preferences::{CandidateLayout, CustomTheme, TouchKeyboardSkinDesign};
use serde::{Deserialize, Serialize};

/// The global theme id stored in `Preferences::global_theme`. The picker lists them in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GlobalTheme {
    /// Follow the platform: every slot is the host's native token, in the host's current light or dark mode.
    #[default]
    System,
    Shuishan,
    Light,
    Paper,
    Night,
    Ink,
    /// The user's own theme, assembled from `Preferences::custom_theme`.
    Custom,
}

impl GlobalTheme {
    pub const ALL: [GlobalTheme; 7] = [
        GlobalTheme::System,
        GlobalTheme::Shuishan,
        GlobalTheme::Light,
        GlobalTheme::Paper,
        GlobalTheme::Night,
        GlobalTheme::Ink,
        GlobalTheme::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            GlobalTheme::System => "system",
            GlobalTheme::Shuishan => "shuishan",
            GlobalTheme::Light => "light",
            GlobalTheme::Paper => "paper",
            GlobalTheme::Night => "night",
            GlobalTheme::Ink => "ink",
            GlobalTheme::Custom => "custom",
        }
    }

    /// The exact id, or `None` for anything else. The preference document is strict and uses serde; this is for string boundaries (a synced value, an App Group key, a manifest `base`) that must refuse a bad id. There is deliberately no lenient form: a retired or misspelt id is an error at every boundary, so a host that still writes one finds out.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.id() == id)
    }

    /// Whether a custom theme or a skin package may be drawn over this theme: `system` or a built-in theme, never `custom` itself.
    pub fn is_base(self) -> bool {
        self != GlobalTheme::Custom
    }

    /// The picker title, in the product's language.
    pub fn title(self) -> &'static str {
        match self {
            GlobalTheme::System => "跟随系统",
            GlobalTheme::Shuishan => "水杉",
            GlobalTheme::Light => "浅色",
            GlobalTheme::Paper => "纸白",
            GlobalTheme::Night => "夜青",
            GlobalTheme::Ink => "墨",
            GlobalTheme::Custom => "自定义",
        }
    }

    /// The built-in palette, for the five themes that have one.
    pub fn builtin(self) -> Option<&'static BuiltinTheme> {
        BUILTIN_THEMES.iter().find(|theme| theme.id == self)
    }
}

/// Whether a palette is drawn on a light or a dark surface. Hosts use it for chrome they cannot colour directly (the iOS keyboard appearance, scrollbars, system menus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAppearance {
    Light,
    Dark,
}

/// One built-in theme, exactly as the design's `THEMES` table lists it (dc.html L1484-1491).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTheme {
    pub id: GlobalTheme,
    pub appearance: ThemeAppearance,
    /// `bg`: the theme card and settings preview background.
    pub background: &'static str,
    /// `panel`: the candidate window, toolbar and menu surface.
    pub panel: &'static str,
    pub accent: &'static str,
    /// `text`: candidate and menu text.
    pub text: &'static str,
    /// `kb.bg`
    pub keyboard_background: &'static str,
    /// `kb.key`
    pub keyboard_key: &'static str,
    /// `kb.spec`: function keys (shift, delete, symbols, space row modifiers).
    pub keyboard_function_key: &'static str,
    /// `kb.fg`
    pub keyboard_text: &'static str,
    /// `kb.sub`: key hints, candidate numbers and secondary candidate text.
    pub keyboard_secondary: &'static str,
}

/// The five built-in palettes, in picker order.
pub const BUILTIN_THEMES: [BuiltinTheme; 5] = [
    BuiltinTheme {
        id: GlobalTheme::Shuishan,
        appearance: ThemeAppearance::Dark,
        background: "#1E1F1C",
        panel: "#2A2B27",
        accent: "#7FE08E",
        text: "#FFFFFF",
        keyboard_background: "#1E1F1C",
        keyboard_key: "#2F302C",
        keyboard_function_key: "#23241F",
        keyboard_text: "#FFFFFF",
        keyboard_secondary: "#9FB5A3",
    },
    BuiltinTheme {
        id: GlobalTheme::Light,
        appearance: ThemeAppearance::Light,
        background: "#E9E9E9",
        panel: "#FFFFFF",
        accent: "#005FB8",
        text: "#1A1A1A",
        keyboard_background: "#E4E6EA",
        keyboard_key: "#FFFFFF",
        keyboard_function_key: "#C8CCD3",
        keyboard_text: "#1A1A1A",
        keyboard_secondary: "#6A6F76",
    },
    BuiltinTheme {
        id: GlobalTheme::Paper,
        appearance: ThemeAppearance::Light,
        background: "#E8E4DA",
        panel: "#F7F5F0",
        accent: "#2C7A4B",
        text: "#1A1E1B",
        keyboard_background: "#E6E1D5",
        keyboard_key: "#FBF9F4",
        keyboard_function_key: "#D3CCBC",
        keyboard_text: "#1A1E1B",
        keyboard_secondary: "#6E6A5E",
    },
    BuiltinTheme {
        id: GlobalTheme::Night,
        appearance: ThemeAppearance::Dark,
        background: "#0F1B22",
        panel: "#16262F",
        accent: "#4FD1C5",
        text: "#E6F1F4",
        keyboard_background: "#0F1B22",
        keyboard_key: "#1D3340",
        keyboard_function_key: "#15252E",
        keyboard_text: "#E6F1F4",
        keyboard_secondary: "#86A6B0",
    },
    BuiltinTheme {
        id: GlobalTheme::Ink,
        appearance: ThemeAppearance::Dark,
        background: "#0B0B0B",
        panel: "#1A1A1A",
        accent: "#FFFFFF",
        text: "#9A9A9A",
        keyboard_background: "#111111",
        keyboard_key: "#2A2A2A",
        keyboard_function_key: "#1C1C1C",
        keyboard_text: "#F2F2F2",
        keyboard_secondary: "#9A9A9A",
    },
];

/// The candidate window border every built-in theme draws: the design's `rgba(0,0,0,.12)`.
pub const BUILTIN_CANDIDATE_BORDER: &str = "#0000001F";
/// Alpha appended to the accent for the selected candidate row: the design's `accent + '24'` (about 14%).
pub const SELECTED_ALPHA: &str = "24";
/// Alpha appended to the text colour for a hovered candidate row. The design draws no hover state; 6% of the text colour keeps hover visibly weaker than selection on both light and dark panels.
pub const HOVER_ALPHA: &str = "0F";
/// Alpha appended to a custom keyboard's key text for its secondary text (key hints). The keyboard editor has no secondary colour; 60% matches how far `kb.sub` sits from `kb.fg` in the built-in themes.
pub const CUSTOM_KEYBOARD_SECONDARY_ALPHA: &str = "99";
/// Alpha appended to a custom text picker colour for the index numbers (and so the secondary text) when the number picker is unset: an explicit text colour also sets the numbers, about 62% opaque, as every host drew it before the global theme.
pub const PICKED_NUMBER_ALPHA: &str = "9D";

/// Colours for the candidate window, and by derivation the floating toolbar and the menus (surface = `surface`, item text = `text`, hovered item = `hover`, checked or highlighted item = `selected` fill with `selected_text`, separators and outline = `border`).
///
/// A `None` slot is the host's native token for that slot. `show_selected_bar` is `None` unless an external package says otherwise, leaving the selection indicator to the platform.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateThemePalette {
    pub surface: Option<String>,
    pub border: Option<String>,
    /// Candidate text.
    pub text: Option<String>,
    /// Candidate index labels.
    pub number: Option<String>,
    /// Secondary candidate text: comments, translations, pinyin hints.
    pub secondary: Option<String>,
    pub accent: Option<String>,
    /// Selected row fill.
    pub selected: Option<String>,
    /// Text of the selected candidate.
    pub selected_text: Option<String>,
    /// Index label of the selected candidate.
    pub selected_number: Option<String>,
    /// Hovered row fill.
    pub hover: Option<String>,
    pub show_selected_bar: Option<bool>,
}

/// Colours for a touch keyboard, and on touch hosts for the candidate strip above it (strip background = `background`, candidates = `text`, hints = `secondary`, the selected candidate = `accent` text with no fill, as the design draws it: dc.html `mobCands` `mFg`).
///
/// The return key while composing is not a theme colour: the design fills it with the platform accent and white text in every theme (dc.html L2211), so hosts keep their own token there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardThemePalette {
    pub background: String,
    pub key: String,
    pub function_key: String,
    pub text: String,
    pub secondary: String,
    /// The selected candidate in the strip (text, no fill), and the accent marks a host draws on its keys (hints, toggled keys, focus).
    pub accent: String,
    /// Text readable on an `accent` fill, for hosts that fill a key or chip with the accent: black or white by the accent's luminance.
    pub on_accent: String,
}

/// The theme card preview: the design's `bg`, `panel`, `accent` and `text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemePreview {
    pub background: String,
    pub panel: String,
    pub accent: String,
    pub text: String,
}

/// One entry of `catalog()`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeCatalogEntry {
    pub id: GlobalTheme,
    pub title: &'static str,
    /// `None` for `system` and `custom`, whose appearance depends on the platform or on what the user assembled.
    pub appearance: Option<ThemeAppearance>,
    pub preview: Option<ThemePreview>,
    pub candidate: Option<CandidateThemePalette>,
    pub keyboard: Option<KeyboardThemePalette>,
}

impl BuiltinTheme {
    pub fn preview(&self) -> ThemePreview {
        ThemePreview {
            background: self.background.to_owned(),
            panel: self.panel.to_owned(),
            accent: self.accent.to_owned(),
            text: self.text.to_owned(),
        }
    }

    /// The design's non-system candidate override: `candBg = panel`, `candBorder = rgba(0,0,0,.12)`, `candSelBg = accent + '24'`, `candSelFg = accent`, secondary text in `kb.sub`.
    pub fn candidate(&self) -> CandidateThemePalette {
        CandidateThemePalette {
            surface: Some(self.panel.to_owned()),
            border: Some(BUILTIN_CANDIDATE_BORDER.to_owned()),
            text: Some(self.text.to_owned()),
            number: Some(self.keyboard_secondary.to_owned()),
            secondary: Some(self.keyboard_secondary.to_owned()),
            accent: Some(self.accent.to_owned()),
            selected: Some(format!("{}{SELECTED_ALPHA}", self.accent)),
            selected_text: Some(self.accent.to_owned()),
            selected_number: Some(self.keyboard_secondary.to_owned()),
            hover: Some(format!("{}{HOVER_ALPHA}", self.text)),
            show_selected_bar: None,
        }
    }

    pub fn keyboard(&self) -> KeyboardThemePalette {
        KeyboardThemePalette {
            background: self.keyboard_background.to_owned(),
            key: self.keyboard_key.to_owned(),
            function_key: self.keyboard_function_key.to_owned(),
            text: self.keyboard_text.to_owned(),
            secondary: self.keyboard_secondary.to_owned(),
            accent: self.accent.to_owned(),
            on_accent: rgb(super::ai::readable_text(
                u32::from_str_radix(&self.accent[1..7], 16)
                    .expect("built-in theme colours are #RRGGBB"),
            )),
        }
    }
}

/// Every theme the picker offers, in order. `system` and `custom` carry no palette here: `system` never has one, and `custom` has one only once `resolve` combines it with the user's `custom_theme`.
pub fn catalog() -> Vec<ThemeCatalogEntry> {
    GlobalTheme::ALL
        .into_iter()
        .map(|id| {
            let builtin = id.builtin();
            ThemeCatalogEntry {
                id,
                title: id.title(),
                appearance: builtin.map(|theme| theme.appearance),
                preview: builtin.map(BuiltinTheme::preview),
                candidate: builtin.map(BuiltinTheme::candidate),
                keyboard: builtin.map(BuiltinTheme::keyboard),
            }
        })
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePackage {
    pub id: String,
    pub base: GlobalTheme,
    /// The candidate layouts the manifest declares (`supports.layouts`). In any other layout the package is not drawn.
    pub layouts: Vec<CandidateLayout>,
    /// `None` when the package does not declare light support.
    pub light: Option<CandidatePalette>,
    /// `None` when the package does not declare dark support.
    pub dark: Option<CandidatePalette>,
}

impl From<&SkinSummary> for ThemePackage {
    fn from(summary: &SkinSummary) -> Self {
        let declared = |mode: &str| summary.themes.iter().any(|theme| theme == mode);
        Self {
            id: summary.id.clone(),
            base: summary.base,
            // `scan` accepts only these two names, so nothing is dropped here.
            layouts: summary
                .layouts
                .iter()
                .filter_map(|layout| match layout.as_str() {
                    "horizontal" => Some(CandidateLayout::Horizontal),
                    "vertical" => Some(CandidateLayout::Vertical),
                    _ => None,
                })
                .collect(),
            light: declared("light").then(|| summary.candidate.light.clone()),
            dark: declared("dark").then(|| summary.candidate.dark.clone()),
        }
    }
}

impl ThemePackage {
    /// Read one entry of the published `candidate_skin_catalog` (see `catalog::host_candidate_catalog`). That entry carries a palette for exactly the modes the package declares, so a missing mode reads as undeclared. Its `title` and decoration keys are not theme colours and are ignored. The entry is read strictly: an unknown key anywhere, a missing `layouts`, a `base` that is not `system` or a built-in theme (as `catalog::scan` refuses the manifest) or an unsafe id is refused. In particular a `SkinSummary` from `msime_client_skin_catalog`, whose keys are camelCase (`showSelectedBar`, `minWidthDip`, `themes`), is not an entry and is refused rather than read with its selection bar and declared modes silently lost; hosts that scan the skin root pass `skins_directory` instead.
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
                    show_selected_bar: palette.show_selected_bar,
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
        })
    }
}

/// The colours a host draws for the selected theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedTheme {
    pub id: GlobalTheme,
    pub source: ThemeSource,
    /// `None` means follow the host's current mode.
    pub appearance: Option<ThemeAppearance>,
    /// `None` for `system`: draw the platform tokens.
    pub candidate: Option<CandidateThemePalette>,
    /// `None` for `system`: draw the platform keyboard.
    pub keyboard: Option<KeyboardThemePalette>,
    /// The external package drawn on this surface: the custom theme names it, it was found, and its manifest declares both `layout` and the mode drawn. Hosts draw the package's decoration and minimum width only when this is set.
    pub candidate_skin: Option<String>,
}

/// Resolve the colours for `theme`.
///
/// `dark` is the host's effective mode for the surface being drawn; only `custom` over a `system` base reads it. `layout` is the candidate layout of that surface; only a custom theme's package reads it. `package` is the installed package `custom.candidate_skin` names, already loaded by the caller (`None` when the custom theme names none or it is not installed); a package with a different id is ignored.
///
/// A custom theme is drawn over a base: the named package's manifest `base`, or else `custom.base`. A built-in base fixes the mode, so the package palette is the one for the base's own appearance and `dark` is ignored. The package is drawn only where its manifest says it may be: in a layout it does not declare, or a mode it does not declare, it contributes nothing and is not reported in `candidate_skin`, and the base is drawn with the pickers. A `system` base contributes no slots and follows `dark`.
///
/// Candidate colours are layered: the base, then the package palette, then every picker the user set. A text picker also sets the numbers to that colour at `PICKED_NUMBER_ALPHA` unless the number picker is set. `secondary` always follows `number`. Over a built-in base the slots that base derives keep following their sources unless the package or a picker set them: `selected` is `accent` at `SELECTED_ALPHA`, `hover` is `text` at `HOVER_ALPHA`, `selected_text` is `accent` and `selected_number` is `number`. A custom theme over `system` with no package slots and no pickers has no candidate palette at all and draws the platform's own.
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
        };
    }
    if theme == GlobalTheme::System {
        return ResolvedTheme {
            id: theme,
            source: ThemeSource::System,
            appearance: None,
            candidate: None,
            keyboard: None,
            candidate_skin: None,
        };
    }
    let package = package.filter(|package| custom.candidate_skin.as_deref() == Some(&package.id));
    let base = package
        .map_or(custom.base, |package| package.base)
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
    let picked_number = match (&pickers.text, &pickers.number) {
        (Some(text), None) => Some(with_alpha(text, PICKED_NUMBER_ALPHA)),
        _ => None,
    };
    explicit.apply(pickers);
    if picked_number.is_some() {
        explicit.number = picked_number;
    }
    let base_palette = base.map(BuiltinTheme::candidate).unwrap_or_default();
    let derived = base.is_some();
    let text = explicit.text.or(base_palette.text);
    let number = explicit.number.or(base_palette.number);
    let accent = explicit.accent.or(base_palette.accent);
    let candidate = CandidateThemePalette {
        surface: explicit.surface.or(base_palette.surface),
        border: explicit.border.or(base_palette.border),
        secondary: number.clone(),
        selected: explicit.selected.or_else(|| {
            accent
                .as_deref()
                .filter(|_| derived)
                .map(|accent| with_alpha(accent, SELECTED_ALPHA))
        }),
        selected_text: accent.clone().filter(|_| derived),
        selected_number: number.clone().filter(|_| derived),
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
    }
}

/// The candidate slots a package palette or the pickers set explicitly, normalized; unset and unparseable values are `None`.
#[derive(Default)]
struct Overrides {
    surface: Option<String>,
    border: Option<String>,
    text: Option<String>,
    number: Option<String>,
    accent: Option<String>,
    selected: Option<String>,
    hover: Option<String>,
    show_selected_bar: Option<bool>,
}

impl Overrides {
    fn from_package(palette: &CandidatePalette) -> Self {
        Self {
            surface: color(&palette.surface),
            border: color(&palette.border),
            text: color(&palette.text),
            number: color(&palette.number),
            accent: color(&palette.accent),
            selected: color(&palette.selected),
            hover: color(&palette.hover),
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
            show_selected_bar: None,
        }
    }

    /// Lay `top` over `self`: every slot `top` sets wins.
    fn apply(&mut self, top: Self) {
        fn over<T>(target: &mut Option<T>, value: Option<T>) {
            if value.is_some() {
                *target = value;
            }
        }
        over(&mut self.surface, top.surface);
        over(&mut self.border, top.border);
        over(&mut self.text, top.text);
        over(&mut self.number, top.number);
        over(&mut self.accent, top.accent);
        over(&mut self.selected, top.selected);
        over(&mut self.hover, top.hover);
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
        on_accent: rgb(super::ai::readable_text(design.accent & 0xFF_FFFF)),
    }
}

fn rgb(color: u32) -> String {
    format!("#{:06X}", color & 0xFF_FFFF)
}

/// An external package colour in the one form the theme contract emits (`#RRGGBB` or `#RRGGBBAA`, uppercase), or `None` for anything a host could not parse the same way. Packages written for the Windows presenter may also use `#RGB`, `rgb()`/`rgba()` with 0-255 channels and a 0-1 alpha, and `transparent`; those are converted rather than dropped.
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
            3 => Some(format!(
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

//! msime-windows 的内置外观作为清单 `base` 时的配色。
//!
//! Windows 的皮肤包把 `base` 写成它自己的内置外观（`fluent`、`wechat`、`wechat_scroll`、`graphite`、`willow_green`、`autumn_osmanthus`、`microsoft`），跨平台的包写全局主题；两边接受同一套清单，同一个包在哪个平台都能加载。全局主题之外的外观在这里画在 `system` 之上：包没写的候选框和工具栏颜色、两处圆角，按该外观在 Windows 上的样子补齐，深浅两套各补各的。`fluent` 就是 Windows 的原生配色，与 `system` 相同，不补任何东西。
//!
//! 颜色抄自 msime-windows 的 D2D 渲染端（`server/src/window/candidate_presenter.cpp` 的 `CandSkinTokens`、`candidate_skin_palette.cpp`、`floating_toolbar_skin.cpp`），它们本身对齐 `ui-html/webview2/candwnd/skins/<外观>/` 的 CSS；带透明度的值已换算成 `#RRGGBBAA`。`selected_text` 取自仓库里那份 CSS 的副本 `packages/ui/src/upstream/candidate-themes/skins/<外观>/`（`autumn_osmanthus`、`microsoft` 没有副本，留空）。Windows 改了这些外观的配色，这里要一起改。
//!
//! `wechat_scroll` 的细分文字色和右键菜单配色、以及 `canvas` 基底的整套外观，抄自 msime-windows 的出厂清单 `skins/default/<外观>/skin.toml`（见 [`LOOK_DETAILS`] 与 [`CANVAS_LOOK`]）。

use super::{CandidateMenuPalette, CandidatePalette, SkinToolbar, ToolbarPalette};

/// 一种明暗下的候选框配色，槽位与 [`CandidatePalette`] 相同。
struct LookCandidate {
    surface: &'static str,
    border: &'static str,
    text: &'static str,
    number: &'static str,
    accent: &'static str,
    selected: &'static str,
    hover: &'static str,
    show_selected_bar: bool,
    /// 高亮候选的文字和序号色（Windows 样式表的 `.first .text, .first .num`），外观没有专门给时为 `None`，画普通的文字和序号色。补进 [`CandidatePalette::selected_text`]；网页 SDK 的 `packages/web-engine/tools/theme-catalog.mjs` 也从这张表读它。
    selected_text: Option<&'static str>,
}

/// 外观出厂清单（msime-windows `skins/default/<外观>/skin.toml`）里的细分文字色和右键菜单配色，补进包没写的同名槽位；清单没写的为 `None`。单独成表而不放进 [`LookCandidate`]，因为网页 SDK 的生成脚本逐字段解析那张表。
struct LookDetail {
    candidate_text: Option<&'static str>,
    preedit_text: Option<&'static str>,
    preedit_caret: Option<&'static str>,
    selected_number: Option<&'static str>,
    translation: Option<&'static str>,
    selected_translation: Option<&'static str>,
    menu: Option<LookMenu>,
}

/// 候选框右键菜单的配色，槽位与 [`CandidateMenuPalette`] 相同。
struct LookMenu {
    background: &'static str,
    border: &'static str,
    text: &'static str,
    hover: &'static str,
}

/// 一种明暗下的悬浮工具栏配色，槽位与 [`ToolbarPalette`] 相同。
struct LookToolbar {
    background: &'static str,
    border: &'static str,
    handle: &'static str,
    divider: &'static str,
    icon: &'static str,
    hover: &'static str,
}

struct WindowsLook {
    id: &'static str,
    corner_radius_dip: f64,
    toolbar_corner_radius_dip: f64,
    dark: LookCandidate,
    light: LookCandidate,
    toolbar_dark: LookToolbar,
    toolbar_light: LookToolbar,
}

/// Windows 默认工具栏（`fluent` 与 `microsoft` 用它）的配色。
const DEFAULT_TOOLBAR_DARK: LookToolbar = LookToolbar {
    background: "#1A1A1A",
    border: "#FFFFFF26",
    handle: "#8E8CD8",
    divider: "#FFFFFF26",
    icon: "#FFFFFF",
    hover: "#FFFFFF1A",
};
const DEFAULT_TOOLBAR_LIGHT: LookToolbar = LookToolbar {
    background: "#FFFFFF",
    border: "#0000001F",
    handle: "#8E8CD8",
    divider: "#0000001F",
    icon: "#1A1A1A",
    hover: "#00000014",
};

const WINDOWS_LOOKS: [WindowsLook; 6] = [
    WindowsLook {
        id: "wechat",
        corner_radius_dip: 5.0,
        toolbar_corner_radius_dip: 8.0,
        dark: LookCandidate {
            surface: "#151515",
            border: "#292929",
            text: "#B7B7B7",
            number: "#858585",
            accent: "#07C160",
            selected: "#07C160",
            hover: "#07C16052",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        light: LookCandidate {
            surface: "#F7F7F7",
            border: "#DEDEDE",
            text: "#333333",
            number: "#757575",
            accent: "#07C160",
            selected: "#07C160",
            hover: "#07C16024",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        toolbar_dark: LookToolbar {
            background: "#151515",
            border: "#292929",
            handle: "#07C160",
            hover: "#07C16029",
            ..DEFAULT_TOOLBAR_DARK
        },
        toolbar_light: LookToolbar {
            background: "#F7F7F7",
            border: "#DEDEDE",
            handle: "#07C160",
            hover: "#07C1601F",
            ..DEFAULT_TOOLBAR_LIGHT
        },
    },
    WindowsLook {
        id: "wechat_scroll",
        corner_radius_dip: 10.0,
        toolbar_corner_radius_dip: 8.0,
        dark: LookCandidate {
            surface: "#1A1A1A",
            border: "#2E2E2E",
            text: "#EDEDED",
            number: "#8A8A8A",
            accent: "#07C160",
            selected: "#07C160",
            hover: "#07C16033",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        light: LookCandidate {
            surface: "#FFFFFF",
            border: "#E3E3E3",
            text: "#1F1F1F",
            number: "#8C8C8C",
            accent: "#07C160",
            selected: "#07C160",
            hover: "#07C16024",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        toolbar_dark: LookToolbar {
            background: "#1A1A1A",
            border: "#2E2E2E",
            handle: "#07C160",
            divider: "#2E2E2E",
            icon: "#EDEDED",
            hover: "#FFFFFF14",
        },
        toolbar_light: LookToolbar {
            background: "#FFFFFF",
            border: "#E3E3E3",
            handle: "#07C160",
            divider: "#E3E3E3",
            icon: "#1F1F1F",
            hover: "#0000000D",
        },
    },
    WindowsLook {
        id: "graphite",
        corner_radius_dip: 3.0,
        toolbar_corner_radius_dip: 4.0,
        dark: LookCandidate {
            surface: "#1C1F23",
            border: "#30353B",
            text: "#AEB6C2",
            number: "#707987",
            accent: "#8993A0",
            selected: "#00000000",
            hover: "#FFFFFF0E",
            show_selected_bar: false,
            selected_text: Some("#F1F3F5"),
        },
        light: LookCandidate {
            surface: "#FBFBFC",
            border: "#E2E5E9",
            text: "#586476",
            number: "#8993A1",
            accent: "#5F6B7A",
            selected: "#00000000",
            hover: "#1F29370E",
            show_selected_bar: false,
            selected_text: Some("#111827"),
        },
        toolbar_dark: LookToolbar {
            background: "#1C1F23",
            border: "#30353B",
            handle: "#8993A0",
            divider: "#FFFFFF1A",
            icon: "#D7DCE2",
            hover: "#FFFFFF0E",
        },
        toolbar_light: LookToolbar {
            background: "#FBFBFC",
            border: "#E2E5E9",
            handle: "#5F6B7A",
            divider: "#1F29371A",
            icon: "#374151",
            hover: "#1F29370E",
        },
    },
    WindowsLook {
        id: "willow_green",
        corner_radius_dip: 9.0,
        toolbar_corner_radius_dip: 9.0,
        dark: LookCandidate {
            surface: "#2D2F2E",
            border: "#00000000",
            text: "#D8DBD8",
            number: "#A6ABA7",
            accent: "#65C98D",
            selected: "#65C98D",
            hover: "#65C98D38",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        light: LookCandidate {
            surface: "#F4F5F3",
            border: "#00000000",
            text: "#343936",
            number: "#686F6A",
            accent: "#58B980",
            selected: "#58B980",
            hover: "#58B98029",
            show_selected_bar: false,
            selected_text: Some("#FFFFFF"),
        },
        toolbar_dark: LookToolbar {
            background: "#2D2F2E",
            border: "#3B3E3C",
            handle: "#65C98D",
            hover: "#65C98D33",
            ..DEFAULT_TOOLBAR_DARK
        },
        toolbar_light: LookToolbar {
            background: "#F4F5F3",
            border: "#DFE3DF",
            handle: "#58B980",
            hover: "#58B98029",
            ..DEFAULT_TOOLBAR_LIGHT
        },
    },
    WindowsLook {
        id: "autumn_osmanthus",
        corner_radius_dip: 10.0,
        toolbar_corner_radius_dip: 10.0,
        dark: LookCandidate {
            surface: "#7D929F",
            border: "#00000000",
            text: "#F5F8FA",
            number: "#E1E8EC",
            accent: "#F97D0A",
            selected: "#F97D0A",
            hover: "#F97D0A4D",
            show_selected_bar: false,
            selected_text: None,
        },
        light: LookCandidate {
            surface: "#D6ECF0",
            border: "#00000000",
            text: "#1F3138",
            number: "#5B727B",
            accent: "#E6A817",
            selected: "#FFE399",
            hover: "#FFE3998C",
            show_selected_bar: false,
            selected_text: None,
        },
        toolbar_dark: LookToolbar {
            background: "#7D929F",
            border: "#8FA3AF",
            handle: "#F97D0A",
            hover: "#F97D0A47",
            ..DEFAULT_TOOLBAR_DARK
        },
        toolbar_light: LookToolbar {
            background: "#D6ECF0",
            border: "#BCD8DE",
            handle: "#E6A817",
            hover: "#FFE399B3",
            ..DEFAULT_TOOLBAR_LIGHT
        },
    },
    WindowsLook {
        id: "microsoft",
        corner_radius_dip: 8.0,
        toolbar_corner_radius_dip: 8.0,
        dark: LookCandidate {
            surface: "#2C2C2C",
            border: "#1C1C1C",
            text: "#FFFFFF",
            number: "#CFCFCF",
            accent: "#E183D9",
            selected: "#383838",
            hover: "#353535",
            show_selected_bar: true,
            selected_text: None,
        },
        light: LookCandidate {
            surface: "#F9F9F9",
            border: "#0000001A",
            text: "#1A1A1A",
            number: "#5F5F5F",
            accent: "#E183D9",
            selected: "#EAEAEA",
            hover: "#F0F0F0",
            show_selected_bar: true,
            selected_text: None,
        },
        toolbar_dark: LookToolbar {
            handle: "#E183D9",
            ..DEFAULT_TOOLBAR_DARK
        },
        toolbar_light: LookToolbar {
            handle: "#E183D9",
            ..DEFAULT_TOOLBAR_LIGHT
        },
    },
];

/// 有细分槽位的外观：`(ID, 深色, 浅色)`，值抄自各自的出厂清单。目前只有 `wechat_scroll`；其他外观的细分槽位留给宿主按基础颜色推导。
const LOOK_DETAILS: [(&str, LookDetail, LookDetail); 1] = [(
    "wechat_scroll",
    LookDetail {
        candidate_text: Some("#EDEDED"),
        preedit_text: Some("#D0D0D0"),
        preedit_caret: Some("#07C160"),
        selected_number: Some("#FFFFFFCC"),
        translation: Some("#529CCA"),
        selected_translation: Some("#FFFFFFCC"),
        menu: Some(LookMenu {
            background: "#1F1F1F",
            border: "#343434",
            text: "#D8D8D8",
            hover: "#2A2A2A",
        }),
    },
    LookDetail {
        candidate_text: Some("#1F1F1F"),
        preedit_text: Some("#555555"),
        preedit_caret: Some("#07C160"),
        selected_number: Some("#FFFFFFCC"),
        translation: Some("#0B6E99"),
        selected_translation: Some("#FFFFFFCC"),
        menu: Some(LookMenu {
            background: "#FFFFFF",
            border: "#D9D9D9",
            text: "#333333",
            hover: "#EEEEEE",
        }),
    },
)];

/// `base = "canvas"` 的外观，抄自 msime-windows `skins/default/canvas/skin.toml`。那份清单的底色是透明的（`#20202000` / `#FFFFFF00`），外观交给皮肤自己的九宫格贴图；msime 的宿主不画九宫格，所以按那份清单注释里“不能透明的使用方压平”的说法取同一 RGB 的不透明底色。`canvas` 不能单独选用，因此不在 [`WINDOWS_LOOKS`] 和 [`WINDOWS_LOOK_IDS`] 里；候选框圆角 0 是给贴图面板用的，这里不补，保留宿主自己的圆角。
const CANVAS_LOOK: WindowsLook = WindowsLook {
    id: "canvas",
    corner_radius_dip: 0.0,
    toolbar_corner_radius_dip: 8.0,
    dark: LookCandidate {
        surface: "#202020",
        border: "#00000000",
        text: "#F2F2F2",
        number: "#F2F2F29E",
        accent: "#6B69D6",
        selected: "#FFFFFF29",
        hover: "#FFFFFF17",
        show_selected_bar: true,
        selected_text: None,
    },
    light: LookCandidate {
        surface: "#FFFFFF",
        border: "#00000000",
        text: "#1A1A1A",
        number: "#1A1A1A99",
        accent: "#6B69D6",
        selected: "#0000001A",
        hover: "#0000000F",
        show_selected_bar: true,
        selected_text: None,
    },
    toolbar_dark: LookToolbar {
        handle: "#6B69D6",
        ..DEFAULT_TOOLBAR_DARK
    },
    toolbar_light: LookToolbar {
        handle: "#6B69D6",
        ..DEFAULT_TOOLBAR_LIGHT
    },
};
const CANVAS_DETAIL_DARK: LookDetail = LookDetail {
    candidate_text: None,
    preedit_text: None,
    preedit_caret: None,
    selected_number: None,
    translation: Some("#9A6DD7"),
    selected_translation: None,
    menu: Some(LookMenu {
        background: "#2D2D2D",
        border: "#9B9B9B2E",
        text: "#E9E8E8",
        hover: "#414141",
    }),
};
const CANVAS_DETAIL_LIGHT: LookDetail = LookDetail {
    candidate_text: None,
    preedit_text: None,
    preedit_caret: None,
    selected_number: None,
    translation: Some("#6940A5"),
    selected_translation: None,
    menu: Some(LookMenu {
        background: "#FFFFFF",
        border: "#0000001F",
        text: "#1A1A1A",
        hover: "#ECECEC",
    }),
};

/// msime-windows 的内置外观 ID，清单 `base` 可以写它们。外部皮肤也不能用这些 ID 当文件夹名，否则在 Windows 上会与内置外观冲突。
pub const WINDOWS_LOOK_IDS: [&str; 7] = [
    "fluent",
    "wechat",
    "wechat_scroll",
    "graphite",
    "willow_green",
    "autumn_osmanthus",
    "microsoft",
];

/// Windows 自带清单的完整 ID。`canvas` 是 v2 的基底，不是可以单独选择的外观，但仍须保留给出厂清单。
pub const WINDOWS_BUILTIN_IDS: [&str; 8] = [
    "fluent",
    "wechat",
    "wechat_scroll",
    "graphite",
    "willow_green",
    "autumn_osmanthus",
    "microsoft",
    "canvas",
];

/// msime-windows 在皮肤目录里放内置外观设置清单的子目录名，外部皮肤同样不能占用。
pub const WINDOWS_DEFAULTS_FOLDER: &str = "default";

fn fill(slot: &mut Option<String>, value: &str) {
    // 读不懂的颜色在 `theme::resolve` 里等于没写，这里也当没写，换成外观的颜色。
    if slot
        .as_deref()
        .and_then(super::super::theme::normalized_color)
        .is_none()
    {
        *slot = Some(value.to_owned());
    }
}

fn fill_candidate(palette: &mut CandidatePalette, look: &LookCandidate) {
    fill(&mut palette.surface, look.surface);
    fill(&mut palette.border, look.border);
    fill(&mut palette.text, look.text);
    fill(&mut palette.number, look.number);
    fill(&mut palette.accent, look.accent);
    fill(&mut palette.selected, look.selected);
    fill(&mut palette.hover, look.hover);
    if let Some(value) = look.selected_text {
        fill(&mut palette.selected_text, value);
    }
    palette
        .show_selected_bar
        .get_or_insert(look.show_selected_bar);
}

fn fill_detail(palette: &mut CandidatePalette, detail: &LookDetail) {
    for (slot, value) in [
        (&mut palette.candidate_text, detail.candidate_text),
        (&mut palette.preedit_text, detail.preedit_text),
        (&mut palette.preedit_caret, detail.preedit_caret),
        (&mut palette.selected_number, detail.selected_number),
        (&mut palette.translation, detail.translation),
        (
            &mut palette.selected_translation,
            detail.selected_translation,
        ),
    ] {
        if let Some(value) = value {
            fill(slot, value);
        }
    }
    if let Some(look) = &detail.menu {
        // 逐个槽位补，包只写了菜单的一部分颜色时其余的仍沿用外观。
        let menu = palette
            .menu
            .get_or_insert_with(CandidateMenuPalette::default);
        fill(&mut menu.background, look.background);
        fill(&mut menu.border, look.border);
        fill(&mut menu.text, look.text);
        fill(&mut menu.hover, look.hover);
    }
}

fn fill_toolbar(palette: &mut ToolbarPalette, look: &LookToolbar) {
    fill(&mut palette.background, look.background);
    fill(&mut palette.border, look.border);
    fill(&mut palette.handle, look.handle);
    fill(&mut palette.divider, look.divider);
    fill(&mut palette.icon, look.icon);
    fill(&mut palette.hover, look.hover);
}

/// `base` 是不是 msime-windows 的内置外观。
pub fn is_windows_look(base: &str) -> bool {
    WINDOWS_LOOK_IDS.contains(&base)
}

/// 用外观 `base` 补齐包没写的颜色与圆角；`fluent` 什么也不补。`base` 不是 Windows 外观时不做任何事。
pub(super) fn fill_from_look(
    base: &str,
    candidate: &mut super::CandidateColors,
    corner_radius_dip: &mut Option<f64>,
    toolbar: &mut SkinToolbar,
) {
    let Some(look) = WINDOWS_LOOKS.iter().find(|look| look.id == base) else {
        return;
    };
    fill_candidate(&mut candidate.dark, &look.dark);
    fill_candidate(&mut candidate.light, &look.light);
    if let Some((_, dark, light)) = LOOK_DETAILS.iter().find(|(id, _, _)| *id == base) {
        fill_detail(&mut candidate.dark, dark);
        fill_detail(&mut candidate.light, light);
    }
    corner_radius_dip.get_or_insert(look.corner_radius_dip);
    fill_toolbar(&mut toolbar.dark, &look.toolbar_dark);
    fill_toolbar(&mut toolbar.light, &look.toolbar_light);
    toolbar
        .corner_radius_dip
        .get_or_insert(look.toolbar_corner_radius_dip);
}

/// `base = "canvas"` 的包按 [`CANVAS_LOOK`] 补齐没写的候选框和工具栏颜色；候选框圆角不补（见 [`CANVAS_LOOK`]）。`panel_image` 表示包有九宫格面板：它的文字、序号和高亮色是对着自己的贴图挑的，而 msime 的宿主不画贴图、只画不透明的 `canvas` 底色，所以这时候选框的颜色整套换成 `canvas` 外观（只保留包的强调色和右键菜单），免得浅色贴图上的白字落在白底上看不见。这是 msime-windows “宿主画不了就退回能画的样子”的做法按宿主能力用在这里。
pub(super) fn fill_from_canvas_look(
    candidate: &mut super::CandidateColors,
    toolbar: &mut SkinToolbar,
    panel_image: bool,
) {
    for (palette, look, detail) in [
        (&mut candidate.dark, &CANVAS_LOOK.dark, &CANVAS_DETAIL_DARK),
        (
            &mut candidate.light,
            &CANVAS_LOOK.light,
            &CANVAS_DETAIL_LIGHT,
        ),
    ] {
        if panel_image {
            *palette = CandidatePalette {
                accent: palette.accent.take(),
                menu: palette.menu.take(),
                ..CandidatePalette::default()
            };
        }
        fill_candidate(palette, look);
        fill_detail(palette, detail);
    }
    fill_toolbar(&mut toolbar.dark, &CANVAS_LOOK.toolbar_dark);
    fill_toolbar(&mut toolbar.light, &CANVAS_LOOK.toolbar_light);
    toolbar
        .corner_radius_dip
        .get_or_insert(CANVAS_LOOK.toolbar_corner_radius_dip);
}

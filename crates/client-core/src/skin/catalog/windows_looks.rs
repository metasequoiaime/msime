//! msime-windows 的内置外观作为清单 `base` 时的配色。
//!
//! Windows 的皮肤包把 `base` 写成它自己的内置外观（`fluent`、`wechat`、`graphite`、`willow_green`、`autumn_osmanthus`、`microsoft`），跨平台的包写全局主题；两边接受同一套清单，同一个包在哪个平台都能加载。全局主题之外的外观在这里画在 `system` 之上：包没写的候选框和工具栏颜色、两处圆角，按该外观在 Windows 上的样子补齐，深浅两套各补各的。`fluent` 就是 Windows 的原生配色，与 `system` 相同，不补任何东西。
//!
//! 颜色抄自 msime-windows 的 D2D 渲染端（`server/src/window/candidate_presenter.cpp` 的 `CandSkinTokens`、`candidate_skin_palette.cpp`、`floating_toolbar_skin.cpp`），它们本身对齐 `ui-html/webview2/candwnd/skins/<外观>/` 的 CSS；带透明度的值已换算成 `#RRGGBBAA`。Windows 改了这些外观的配色，这里要一起改。

use super::{CandidatePalette, SkinToolbar, ToolbarPalette};

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

const WINDOWS_LOOKS: [WindowsLook; 5] = [
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

/// msime-windows 的内置外观 ID，清单 `base` 可以写它们。外部皮肤也不能用这些 ID 当文件夹名，否则在 Windows 上会与内置外观冲突。
pub const WINDOWS_LOOK_IDS: [&str; 6] = [
    "fluent",
    "wechat",
    "graphite",
    "willow_green",
    "autumn_osmanthus",
    "microsoft",
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
    palette
        .show_selected_bar
        .get_or_insert(look.show_selected_bar);
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
    corner_radius_dip.get_or_insert(look.corner_radius_dip);
    fill_toolbar(&mut toolbar.dark, &look.toolbar_dark);
    fill_toolbar(&mut toolbar.light, &look.toolbar_light);
    toolbar
        .corner_radius_dip
        .get_or_insert(look.toolbar_corner_radius_dip);
}

//! 应用主题（Android 本地设置文件里的 `app_theme`，不在共享 `Preferences` 里）：Android 设置应用和键盘外框的强调色与底色。它和全局主题（`skin::theme`）是两回事：全局主题给候选窗和键盘配色，应用主题给宿主自己的页面配色，键盘皮肤为「跟随系统」时键盘颜色也从它推出。
//!
//! 种子色来自设计原型的 `APPT` 表（design-tokens.md §1.2）。宿主只读 `catalog()` 画选择器、调 `resolve_app_theme` 取当季颜色，不自己存色值；`andCard`、`logoBg` 这类 `color-mix` 派生色属于宿主的呈现层，由宿主从这里给出的种子色推导。
//!
//! 颜色一律是 `#RRGGBB` 或 `#RRGGBBAA`（大写，透明度在最后）。

use super::season::Season;
use serde::{Deserialize, Serialize};

/// 偏好里保存的应用主题 ID。`siji`（水杉四季）按月份取当季配色，其余四个固定一季。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    #[default]
    Siji,
    Chunya,
    Xiayin,
    Qiushan,
    Dongxue,
}

impl AppTheme {
    /// 选择器的顺序。
    pub const ALL: [AppTheme; 5] = [
        AppTheme::Siji,
        AppTheme::Chunya,
        AppTheme::Xiayin,
        AppTheme::Qiushan,
        AppTheme::Dongxue,
    ];

    pub fn id(self) -> &'static str {
        match self {
            AppTheme::Siji => "siji",
            AppTheme::Chunya => "chunya",
            AppTheme::Xiayin => "xiayin",
            AppTheme::Qiushan => "qiushan",
            AppTheme::Dongxue => "dongxue",
        }
    }

    /// 准确的 ID，其他任何字符串都是 `None`。
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.id() == id)
    }

    pub fn title(self) -> &'static str {
        match self {
            AppTheme::Siji => "水杉四季",
            AppTheme::Chunya => "春芽",
            AppTheme::Xiayin => "夏荫",
            AppTheme::Qiushan => "秋杉",
            AppTheme::Dongxue => "冬雪",
        }
    }

    /// 固定的那一季；`siji` 随月份变化，返回 `None`。
    pub fn fixed_season(self) -> Option<Season> {
        match self {
            AppTheme::Siji => None,
            AppTheme::Chunya => Some(Season::Spring),
            AppTheme::Xiayin => Some(Season::Summer),
            AppTheme::Qiushan => Some(Season::Autumn),
            AppTheme::Dongxue => Some(Season::Winter),
        }
    }

    /// 本主题在 `season` 里实际画的那一季：固定一季的主题不看 `season`。
    pub fn season_in(self, season: Season) -> Season {
        self.fixed_season().unwrap_or(season)
    }
}

/// 一季在一种明暗模式下的种子色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppThemeSeed {
    pub accent: &'static str,
    pub background: &'static str,
    /// 设计的 `rowBg`：详情页行和卡片底色。
    pub card: &'static str,
    /// 分隔线。
    pub hair: &'static str,
}

/// 一季的浅色与深色种子色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppThemeSeason {
    pub season: Season,
    pub light: AppThemeSeed,
    pub dark: AppThemeSeed,
}

/// 深色模式的分隔线不随季节变化（设计 `hair` 深色值）。
const DARK_HAIR: &str = "#2A2F2A";

/// 四季的种子色，按 `Season::ALL` 的顺序。浅色分隔线是强调色加透明度（秋杉 20%，其余 18%）。
pub const APP_THEME_SEEDS: [AppThemeSeason; 4] = [
    AppThemeSeason {
        season: Season::Spring,
        light: AppThemeSeed {
            accent: "#4E9A3A",
            background: "#EAF4E0",
            card: "#FAFDF6",
            hair: "#4E9A3A2E",
        },
        dark: AppThemeSeed {
            accent: "#9AD983",
            background: "#16200F",
            card: "#202C18",
            hair: DARK_HAIR,
        },
    },
    AppThemeSeason {
        season: Season::Summer,
        light: AppThemeSeed {
            accent: "#17704A",
            background: "#DDEFE4",
            card: "#F6FBF8",
            hair: "#17704A2E",
        },
        dark: AppThemeSeed {
            accent: "#5FD39A",
            background: "#0C1C14",
            card: "#14281D",
            hair: DARK_HAIR,
        },
    },
    AppThemeSeason {
        season: Season::Autumn,
        light: AppThemeSeed {
            accent: "#B5562B",
            background: "#F6E9DC",
            card: "#FFFBF6",
            hair: "#B5562B33",
        },
        dark: AppThemeSeed {
            accent: "#F0975F",
            background: "#21150F",
            card: "#2E1E15",
            hair: DARK_HAIR,
        },
    },
    AppThemeSeason {
        season: Season::Winter,
        light: AppThemeSeed {
            accent: "#3A6A8A",
            background: "#E6EEF4",
            card: "#FAFCFE",
            hair: "#3A6A8A2E",
        },
        dark: AppThemeSeed {
            accent: "#93C7E6",
            background: "#0F1820",
            card: "#18232D",
            hair: DARK_HAIR,
        },
    },
];

/// `accent_soft` 的透明度：浅色 `22`（约 13%），深色 `40`（25%），即设计的 `accent + '22'` / `accent + '40'`。
pub const ACCENT_SOFT_LIGHT_ALPHA: &str = "22";
pub const ACCENT_SOFT_DARK_ALPHA: &str = "40";
/// 浅色模式强调色上的文字。
pub const LIGHT_ON_ACCENT: &str = "#FFFFFF";
/// 深色模式强调色上的文字是强调色与黑色按 25% : 75% 混合（修正原型漏出的绿色 `#003920`）。
pub const DARK_ON_ACCENT_ACCENT_PERCENT: u32 = 25;

/// 季节的种子色。
pub fn seeds(season: Season) -> &'static AppThemeSeason {
    APP_THEME_SEEDS
        .iter()
        .find(|seeds| seeds.season == season)
        .expect("every season has seeds")
}

/// 宿主在一种明暗模式下画的应用主题颜色。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppThemeColors {
    pub accent: String,
    /// 强调色的浅底（选中的 tab、色块、进度条轨道）。
    pub accent_soft: String,
    /// 强调色填充上的文字和图标。
    pub on_accent: String,
    pub background: String,
    pub card: String,
    pub hair: String,
}

/// `resolve_app_theme` 的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedAppTheme {
    pub id: AppTheme,
    /// 实际画的那一季。
    pub season: Season,
    #[serde(flatten)]
    pub colors: AppThemeColors,
}

/// `theme` 在 `season`（`siji` 才看它）和 `dark` 模式下的颜色。
pub fn resolve_app_theme(theme: AppTheme, season: Season, dark: bool) -> ResolvedAppTheme {
    let season = theme.season_in(season);
    ResolvedAppTheme {
        id: theme,
        season,
        colors: colors(seeds(season), dark),
    }
}

fn colors(seeds: &AppThemeSeason, dark: bool) -> AppThemeColors {
    let seed = if dark { &seeds.dark } else { &seeds.light };
    let (soft_alpha, on_accent) = if dark {
        (
            ACCENT_SOFT_DARK_ALPHA,
            mix_with_black(seed.accent, DARK_ON_ACCENT_ACCENT_PERCENT),
        )
    } else {
        (ACCENT_SOFT_LIGHT_ALPHA, LIGHT_ON_ACCENT.to_owned())
    };
    AppThemeColors {
        accent: seed.accent.to_owned(),
        accent_soft: format!("{}{soft_alpha}", seed.accent),
        on_accent,
        background: seed.background.to_owned(),
        card: seed.card.to_owned(),
        hair: seed.hair.to_owned(),
    }
}

/// `color-mix(in srgb, color percent%, #000)`：每个通道乘以 `percent`%，取整时与浏览器一样逢 .5 取偶。
fn mix_with_black(color: &str, percent: u32) -> String {
    let value = u32::from_str_radix(&color[1..7], 16).expect("seed colours are #RRGGBB");
    let channel = |shift: u32| {
        let scaled = ((value >> shift) & 0xFF) * percent;
        let (quotient, remainder) = (scaled / 100, scaled % 100);
        if remainder > 50 || (remainder == 50 && quotient % 2 == 1) {
            quotient + 1
        } else {
            quotient
        }
    };
    format!("#{:02X}{:02X}{:02X}", channel(16), channel(8), channel(0))
}

/// 选择器里的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppThemeCatalogEntry {
    pub id: AppTheme,
    pub title: &'static str,
    /// 固定的那一季；`siji` 为 `null`。
    pub season: Option<Season>,
    /// 是否按月份变化，只有 `siji` 为真。
    pub seasonal: bool,
    /// 浅色与深色的预览颜色。`siji` 不随时钟变化，固定画秋杉，宿主要画当季颜色时调 `resolve_app_theme`。
    pub light: AppThemeColors,
    pub dark: AppThemeColors,
}

/// 「水杉四季」在目录里固定画的那一季，保证目录不随月份变化。
pub const SEASONAL_PREVIEW: Season = Season::Autumn;

/// 应用主题的选择器条目，按 `AppTheme::ALL` 的顺序。
pub fn catalog() -> Vec<AppThemeCatalogEntry> {
    AppTheme::ALL
        .into_iter()
        .map(|theme| {
            let seeds = seeds(theme.season_in(SEASONAL_PREVIEW));
            AppThemeCatalogEntry {
                id: theme,
                title: theme.title(),
                season: theme.fixed_season(),
                seasonal: theme.fixed_season().is_none(),
                light: colors(seeds, false),
                dark: colors(seeds, true),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_siji_is_the_default() {
        assert_eq!(AppTheme::default(), AppTheme::Siji);
        for theme in AppTheme::ALL {
            assert_eq!(AppTheme::from_id(theme.id()), Some(theme));
            assert_eq!(
                serde_json::to_value(theme).unwrap(),
                serde_json::Value::String(theme.id().to_owned())
            );
            assert_eq!(
                serde_json::from_value::<AppTheme>(theme.id().into()).unwrap(),
                theme
            );
        }
        for id in ["", "seasons", "spring", "Siji", "autumn"] {
            assert_eq!(AppTheme::from_id(id), None, "{id}");
            assert!(
                serde_json::from_value::<AppTheme>(id.into()).is_err(),
                "{id}"
            );
        }
    }

    #[test]
    fn siji_follows_the_season_and_the_others_keep_their_own() {
        for season in Season::ALL {
            assert_eq!(
                resolve_app_theme(AppTheme::Siji, season, false).season,
                season
            );
            assert_eq!(
                resolve_app_theme(AppTheme::Qiushan, season, true).season,
                Season::Autumn
            );
        }
        let pairs = [
            (AppTheme::Chunya, Season::Spring),
            (AppTheme::Xiayin, Season::Summer),
            (AppTheme::Qiushan, Season::Autumn),
            (AppTheme::Dongxue, Season::Winter),
        ];
        for (theme, season) in pairs {
            for dark in [false, true] {
                let fixed = resolve_app_theme(theme, Season::Spring, dark);
                let seasonal = resolve_app_theme(AppTheme::Siji, season, dark);
                assert_eq!(fixed.colors, seasonal.colors, "{theme:?} {dark}");
                assert_eq!(fixed.id, theme);
                assert_eq!(seasonal.id, AppTheme::Siji);
            }
        }
    }

    /// 设计的种子色（design-tokens.md §1.2）与修正后的深色 `on_accent`（shared-contracts §2.4）。
    #[test]
    fn colours_copy_the_design_seeds() {
        let expected = [
            (
                Season::Spring,
                ["#4E9A3A", "#EAF4E0", "#FAFDF6", "#4E9A3A2E", "#4E9A3A22"],
                ["#9AD983", "#16200F", "#202C18", "#263621", "#9AD98340"],
            ),
            (
                Season::Summer,
                ["#17704A", "#DDEFE4", "#F6FBF8", "#17704A2E", "#17704A22"],
                ["#5FD39A", "#0C1C14", "#14281D", "#183526", "#5FD39A40"],
            ),
            (
                Season::Autumn,
                ["#B5562B", "#F6E9DC", "#FFFBF6", "#B5562B33", "#B5562B22"],
                ["#F0975F", "#21150F", "#2E1E15", "#3C2618", "#F0975F40"],
            ),
            (
                Season::Winter,
                ["#3A6A8A", "#E6EEF4", "#FAFCFE", "#3A6A8A2E", "#3A6A8A22"],
                ["#93C7E6", "#0F1820", "#18232D", "#25323A", "#93C7E640"],
            ),
        ];
        for (
            season,
            [accent, background, card, hair, soft],
            [d_accent, d_background, d_card, d_on, d_soft],
        ) in expected
        {
            let light = resolve_app_theme(AppTheme::Siji, season, false).colors;
            assert_eq!(
                light,
                AppThemeColors {
                    accent: accent.into(),
                    accent_soft: soft.into(),
                    on_accent: "#FFFFFF".into(),
                    background: background.into(),
                    card: card.into(),
                    hair: hair.into(),
                },
                "{season:?}"
            );
            let dark = resolve_app_theme(AppTheme::Siji, season, true).colors;
            assert_eq!(
                dark,
                AppThemeColors {
                    accent: d_accent.into(),
                    accent_soft: d_soft.into(),
                    on_accent: d_on.into(),
                    background: d_background.into(),
                    card: d_card.into(),
                    hair: "#2A2F2A".into(),
                },
                "{season:?}"
            );
        }
    }

    #[test]
    fn mixing_with_black_rounds_half_to_even() {
        assert_eq!(mix_with_black("#FFFFFF", 25), "#404040");
        assert_eq!(mix_with_black("#000000", 25), "#000000");
        // 154 × 25% = 38.5 → 38，131 × 25% = 32.75 → 33。
        assert_eq!(mix_with_black("#9AD983", 25), "#263621");
    }

    #[test]
    fn the_catalog_is_deterministic_and_siji_previews_autumn() {
        let catalog = catalog();
        assert_eq!(
            catalog.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            AppTheme::ALL
        );
        let siji = &catalog[0];
        assert!(siji.seasonal && siji.season.is_none());
        assert_eq!(siji.light, catalog[3].light);
        assert_eq!(siji.dark, catalog[3].dark);
        for entry in &catalog[1..] {
            assert!(!entry.seasonal && entry.season.is_some());
        }
        let value = serde_json::to_value(&catalog[0]).unwrap();
        assert_eq!(value["id"], "siji");
        assert_eq!(value["season"], serde_json::Value::Null);
        assert_eq!(value["light"]["accent"], "#B5562B");
    }

    #[test]
    fn a_resolved_theme_is_flat() {
        let value =
            serde_json::to_value(resolve_app_theme(AppTheme::Dongxue, Season::Spring, true))
                .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "id": "dongxue", "season": "winter", "accent": "#93C7E6", "accent_soft": "#93C7E640",
                "on_accent": "#25323A", "background": "#0F1820", "card": "#18232D", "hair": "#2A2F2A"
            })
        );
    }

    /// Android 宿主的配色检查（scripts/test-android-app-theme-parity.py）读这份提交进仓库的副本；它必须与 `catalog()` 一致。改了种子色后带 `MSIME_WRITE_THEME_CATALOG=1` 重跑本测试来重写它。
    #[test]
    fn app_theme_copy_matches() {
        let catalog = serde_json::to_value(serde_json::json!({
            "app_themes": catalog(),
            "default": AppTheme::default(),
        }))
        .expect("catalog serializes");
        if std::env::var_os("MSIME_WRITE_THEME_CATALOG").is_some() {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../platforms/android/tests/settings/app-theme-catalog.json"
            );
            let text = serde_json::to_string_pretty(&catalog).expect("catalog prints") + "\n";
            std::fs::write(path, text).expect("app theme catalog copy is writable");
        }
        let copy: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../platforms/android/tests/settings/app-theme-catalog.json"
        ))
        .expect("app theme catalog copy is JSON");
        assert_eq!(
            copy, catalog,
            "platforms/android/tests/settings/app-theme-catalog.json is stale; rerun this test with MSIME_WRITE_THEME_CATALOG=1"
        );
    }
}

//! 季节规则：「水杉四季」皮肤和应用主题按月份取当季的配色。规则只在这里写一份，宿主只传月份。

use serde::{Deserialize, Serialize};

/// 一年四季。按宿主本地日历的月份划分：3-5 月春，6-8 月夏，9-11 月秋，12-2 月冬（设计原型 APPT 的规则）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [
        Season::Spring,
        Season::Summer,
        Season::Autumn,
        Season::Winter,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }
}

/// `month` 所在的季节；`month` 不在 1..=12 时返回 `None`。
pub fn season_for_month(month: u8) -> Option<Season> {
    match month {
        3..=5 => Some(Season::Spring),
        6..=8 => Some(Season::Summer),
        9..=11 => Some(Season::Autumn),
        12 | 1 | 2 => Some(Season::Winter),
        _ => None,
    }
}

/// 当前的 UTC 月份（1..=12）。client-core 不知道用户所在的时区，月份应由宿主传入；只有 C ABI 边界在宿主没传月份时才用它兜底，UTC 月份与本地月份最多只在月初前后差十几个小时。
pub fn current_utc_month() -> u8 {
    u8::from(time::OffsetDateTime::now_utc().month())
}

/// 当前 UTC 月份所在的季节，供不传月份的调用方使用。
pub fn current_utc_season() -> Season {
    season_for_month(current_utc_month()).unwrap_or(Season::Autumn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_month_maps_to_the_design_season() {
        let expected = [
            (1, Season::Winter),
            (2, Season::Winter),
            (3, Season::Spring),
            (4, Season::Spring),
            (5, Season::Spring),
            (6, Season::Summer),
            (7, Season::Summer),
            (8, Season::Summer),
            (9, Season::Autumn),
            (10, Season::Autumn),
            (11, Season::Autumn),
            (12, Season::Winter),
        ];
        for (month, season) in expected {
            assert_eq!(season_for_month(month), Some(season), "{month}");
        }
        for month in [0, 13, 255] {
            assert_eq!(season_for_month(month), None, "{month}");
        }
    }

    #[test]
    fn the_utc_month_is_a_calendar_month() {
        let month = current_utc_month();
        assert!((1..=12).contains(&month));
        assert_eq!(season_for_month(month), Some(current_utc_season()));
    }

    #[test]
    fn seasons_serialize_as_their_ids() {
        for season in Season::ALL {
            assert_eq!(
                serde_json::to_value(season).unwrap(),
                serde_json::Value::String(season.id().to_owned())
            );
        }
    }
}

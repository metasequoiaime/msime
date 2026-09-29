//! Typing statistics as an agent may see them: counts per day and per category, never text.
//!
//! The document already holds nothing typed (`msime_client_core::typing_statistics` classifies commits in memory and keeps counts only). Active time, typing speed and the hourly distribution are shown as the settings page shows them, but the hours only summed over the covered days, never day by day, and the last commit instant not at all: it would tell an agent whether the user is at the keyboard right now.

use msime_client_core::typing_statistics::{TypingStatistics, TypingStatisticsStore};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_DAYS: u32 = 7;
pub const MAX_DAYS: u32 = 90;

/// The character categories speed counts, as the settings page does: digits, punctuation, emoji and symbols are not prose, and counting them reads as a burst of speed for someone entering a phone number.
const SPEED_CATEGORIES: [&str; 3] = ["han", "latin", "otherLetter"];

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct StatisticsRequest {
    /// How many of the most recent days with recorded typing to cover, 1 to 90. Defaults to 7.
    pub days: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct StatisticsView {
    /// Whether the user has turned statistics on. Nothing is recorded while it is off.
    pub enabled: bool,
    /// Characters committed over the whole retained history.
    pub total: u64,
    /// How long recorded days are kept, in days; absent when they are kept forever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    /// The covered days, oldest first. A day is the user's local date as the input method recorded it.
    pub days: Vec<DayCount>,
    /// Seconds spent typing over the covered days: the time between commits, with pauses of more than a few seconds left out.
    pub active_seconds: u64,
    /// Han, Latin and other letters per minute of active time over the covered days; absent when no active time was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters_per_minute: Option<u64>,
    /// Characters per local hour of the day, `hours[0]` being midnight to one, summed over the covered days that recorded hours; absent when none did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<Vec<u64>>,
    /// Characters over the covered days by category (han, number, punctuation, symbol and so on) and by source, the input scheme or feature that produced them (quanpin, wubi, voice, ai and so on).
    pub breakdown: Breakdown,
    /// How often each candidate position was chosen, over the whole history: `ranks[0]` is the first candidate.
    pub selections: Selections,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DayCount {
    pub day: String,
    pub characters: u64,
    /// Absent for days recorded before active time was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_seconds: Option<u64>,
    /// Han, Latin and other letters per minute of active time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters_per_minute: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct Breakdown {
    pub characters: BTreeMap<String, u64>,
    pub sources: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct Selections {
    pub ranks: Vec<u64>,
    /// Choices from beyond the first page.
    pub beyond: u64,
}

pub fn load(state_dir: &Path, request: &StatisticsRequest) -> Result<StatisticsView, String> {
    let days = request.days.unwrap_or(DEFAULT_DAYS);
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(format!("days must be between 1 and {MAX_DAYS}"));
    }
    let statistics = TypingStatisticsStore::new(state_dir)
        .load()
        .map_err(|error| error.to_string())?;
    Ok(view(&statistics, days as usize))
}

/// The most recent `days` recorded days rather than a calendar window: only the host knows the user's timezone, so the server does not guess which day today is.
fn view(statistics: &TypingStatistics, days: usize) -> StatisticsView {
    let mut recent = Vec::with_capacity(days);
    recent.extend(statistics.days.keys().rev().take(days).rev().cloned());
    let breakdown = statistics.breakdown(Some(&recent));
    let mut active_ms = 0_u64;
    let mut speed_characters = 0_u64;
    let mut hours: Option<Vec<u64>> = None;
    let mut days = Vec::with_capacity(recent.len());
    days.extend(recent.iter().map(|day| {
        let active = statistics.active_ms(day);
        let characters = speed_characters_on(statistics, day);
        if let Some(active) = active.filter(|active| *active > 0) {
            active_ms = active_ms.saturating_add(active);
            speed_characters = speed_characters.saturating_add(characters);
        }
        if let Some(day_hours) = statistics.hours(day) {
            let sums = hours.get_or_insert_with(|| vec![0; day_hours.len()]);
            for (sum, count) in sums.iter_mut().zip(day_hours) {
                *sum = sum.saturating_add(*count);
            }
        }
        DayCount {
            day: day.clone(),
            characters: statistics.days[day],
            active_seconds: active.map(|active| active / 1000),
            characters_per_minute: active.and_then(|active| per_minute(characters, active)),
        }
    }));
    StatisticsView {
        enabled: statistics.enabled,
        total: statistics.total,
        retention_days: statistics.retention.days(),
        days,
        active_seconds: active_ms / 1000,
        characters_per_minute: per_minute(speed_characters, active_ms),
        hours,
        breakdown: Breakdown {
            characters: breakdown.characters,
            sources: breakdown.sources,
        },
        selections: Selections {
            ranks: statistics.selections.ranks.clone(),
            beyond: statistics.selections.beyond,
        },
    }
}

fn speed_characters_on(statistics: &TypingStatistics, day: &str) -> u64 {
    statistics.daily_details.get(day).map_or(0, |detail| {
        SPEED_CATEGORIES
            .iter()
            .filter_map(|category| detail.characters.get(*category))
            .fold(0, |sum, count| sum.saturating_add(*count))
    })
}

/// Rounded to the nearest whole character; `None` when nothing was timed.
fn per_minute(characters: u64, active_ms: u64) -> Option<u64> {
    (active_ms > 0).then(|| {
        characters
            .saturating_mul(60_000)
            .saturating_add(active_ms / 2)
            / active_ms
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::typing_statistics::TypingBreakdown;

    #[test]
    fn only_the_most_recent_days_are_covered_and_the_last_commit_stays_out() {
        let mut statistics = TypingStatistics {
            enabled: true,
            total: 60,
            ..TypingStatistics::default()
        };
        for (day, count) in [("2026-09-20", 10), ("2026-09-21", 20), ("2026-09-22", 30)] {
            statistics.days.insert(day.into(), count);
            statistics.daily_details.insert(
                day.into(),
                TypingBreakdown {
                    characters: [("han".to_owned(), count)].into(),
                    sources: [("wubi".to_owned(), count)].into(),
                },
            );
            let mut hours = vec![0; 24];
            hours[9] = count;
            statistics.daily_hours.insert(day.into(), hours);
            statistics.daily_active_ms.insert(day.into(), count * 1000);
        }
        // A day from before active time and hours were measured.
        statistics.daily_active_ms.remove("2026-09-21");
        statistics.daily_hours.remove("2026-09-21");
        statistics.selections.ranks = vec![5, 1];
        statistics.last_commit_ms = 1;

        let view = view(&statistics, 2);
        assert_eq!(
            view.days,
            vec![
                DayCount {
                    day: "2026-09-21".into(),
                    characters: 20,
                    active_seconds: None,
                    characters_per_minute: None,
                },
                DayCount {
                    day: "2026-09-22".into(),
                    characters: 30,
                    active_seconds: Some(30),
                    characters_per_minute: Some(60),
                },
            ]
        );
        // The untimed day's characters stay out of the speed, and its missing hours out of the sum.
        assert_eq!(view.active_seconds, 30);
        assert_eq!(view.characters_per_minute, Some(60));
        let hours = view.hours.as_deref().unwrap();
        assert_eq!(
            (hours.len(), hours[9], hours.iter().sum::<u64>()),
            (24, 30, 30)
        );
        assert_eq!(view.retention_days, None);
        assert_eq!(view.breakdown.characters["han"], 50);
        assert_eq!(view.breakdown.sources["wubi"], 50);
        assert_eq!(view.selections.ranks, vec![5, 1]);
        let json = serde_json::to_string(&view).unwrap();
        for absent in ["last_commit", "lastCommit", "2026-09-20"] {
            assert!(!json.contains(absent), "{absent} leaked into {json}");
        }
    }

    #[test]
    fn huge_counts_saturate_rather_than_overflow() {
        assert_eq!(per_minute(u64::MAX, 2), Some(u64::MAX / 2));
        assert_eq!(
            per_minute(1_000_000_000_000_000, 1000),
            Some(u64::MAX / 1000)
        );
        let mut statistics = TypingStatistics {
            enabled: true,
            ..TypingStatistics::default()
        };
        let huge = u64::MAX / 2 + 1;
        statistics.days.insert("2026-09-22".into(), huge);
        statistics.daily_details.insert(
            "2026-09-22".into(),
            TypingBreakdown {
                characters: SPEED_CATEGORIES
                    .iter()
                    .map(|category| ((*category).to_owned(), huge))
                    .collect(),
                sources: Default::default(),
            },
        );
        statistics.daily_active_ms.insert("2026-09-22".into(), 1000);
        assert_eq!(speed_characters_on(&statistics, "2026-09-22"), u64::MAX);
        assert_eq!(
            view(&statistics, 1).characters_per_minute,
            Some(u64::MAX / 1000)
        );
    }

    #[test]
    fn the_day_count_is_bounded() {
        let directory = tempfile::tempdir().unwrap();
        for days in [0, MAX_DAYS + 1] {
            assert!(load(directory.path(), &StatisticsRequest { days: Some(days) }).is_err());
        }
        let empty = load(directory.path(), &StatisticsRequest::default()).unwrap();
        assert!(empty.days.is_empty());
        assert!(!empty.enabled);
        assert_eq!(empty.active_seconds, 0);
        assert_eq!((empty.characters_per_minute, empty.hours), (None, None));
    }
}

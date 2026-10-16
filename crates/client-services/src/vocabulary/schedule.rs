//! Spaced-repetition arithmetic.
//!
//! Pure functions over a card and a calendar day. The day is always an argument: an SRS is
//! entirely date arithmetic, so a scheduler that reads the clock itself can only be tested by
//! waiting, and the rest of this crate already makes the host supply the local day it resolved
//! (see [`crate::typing_statistics::TypingStatisticsStore::record`]).

use serde::{Deserialize, Serialize};

/// The ease floor, in permille.
///
/// SM-2's own floor of 1.3. Below it a card the user keeps failing has its interval collapse
/// toward zero and reappear forever, which is how a review queue stops being finishable.
pub const MIN_EASE_PERMILLE: u16 = 1300;
/// The ease a card starts at, in permille. SM-2's initial E-Factor of 2.5.
pub const INITIAL_EASE_PERMILLE: u16 = 2500;
/// What a lapse costs, in permille. SM-2 charges 0.2 for the worst recall grade.
pub const LAPSE_EASE_PENALTY_PERMILLE: u16 = 200;
/// The interval after a first recall, in days. SM-2's I(1).
pub const FIRST_INTERVAL_DAYS: u32 = 1;
/// The interval after a second consecutive recall, in days. SM-2's I(2).
pub const SECOND_INTERVAL_DAYS: u32 = 6;
/// The furthest a card may be pushed into the future, in days.
///
/// Ten years. Without a cap the multiplication runs away and the store holds due dates no user
/// will reach, which is indistinguishable from the card having been lost.
pub const MAX_INTERVAL_DAYS: u32 = 3650;

/// What the user answered on a card.
///
/// Two buttons, not SM-2's six grades. A self-graded scale that fine is guesswork at the moment of
/// answering, and the page this serves shows 认识 / 不认识.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewGrade {
    /// 认识 — recalled.
    Known,
    /// 不认识 — not recalled.
    Unknown,
}

/// What one card's schedule looks like right now.
///
/// `due` and `last_reviewed` are `YYYY-MM-DD` local days supplied by the host, never derived here
/// from a timezone. That is what keeps a store written on one machine readable on another, and it
/// is the same rule the typing statistics follow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardState {
    /// The gap that produced `due`, in days. Zero while the card is being learned or relearned.
    pub interval_days: u32,
    /// SM-2's E-Factor in permille, so the stored document holds no floats.
    pub ease_permille: u16,
    /// The day this card next comes up.
    pub due: String,
    /// Consecutive recalls. Reset by a lapse, because SM-2 relearns from I(1).
    pub reviews: u32,
    /// How many times the card has been forgotten. Never reset; it is the card's history.
    pub lapses: u32,
    /// The day the card was last answered, empty if it never has been.
    #[serde(default)]
    pub last_reviewed: String,
}

impl CardState {
    /// A card the user has not answered yet, due on `today`.
    pub fn new(today: &str) -> Self {
        Self {
            interval_days: 0,
            ease_permille: INITIAL_EASE_PERMILLE,
            due: today.to_owned(),
            reviews: 0,
            lapses: 0,
            last_reviewed: String::new(),
        }
    }

    /// Whether the card is waiting on `today` or earlier.
    ///
    /// String comparison is the date comparison here: `YYYY-MM-DD` is fixed-width and zero-padded,
    /// so lexical order is chronological order. It is only valid because every day that reaches
    /// the store has been through [`crate::calendar::is_valid_day`], which rejects every other
    /// shape.
    pub fn is_due(&self, today: &str) -> bool {
        self.due.as_str() <= today
    }

    /// Whether the user has never answered this card.
    pub fn is_new(&self) -> bool {
        self.reviews == 0 && self.lapses == 0 && self.last_reviewed.is_empty()
    }

    /// Whether this state is one this code could have written.
    pub fn is_valid(&self) -> bool {
        crate::calendar::is_valid_day(&self.due)
            && (self.last_reviewed.is_empty() || crate::calendar::is_valid_day(&self.last_reviewed))
            && self.interval_days <= MAX_INTERVAL_DAYS
            && (MIN_EASE_PERMILLE..=INITIAL_EASE_PERMILLE).contains(&self.ease_permille)
    }
}

/// `state` after the user answered `grade` on `today`. `None` when `today` is not a day.
///
/// SM-2 reduced to two grades:
///
/// - 认识 steps the interval along I(1)=1, I(2)=6, then `interval × ease`, and leaves the ease
///   alone. SM-2 adjusts the ease per grade, but with two buttons there is no signal to adjust it
///   by, and deriving one from a binary answer would be a number with nothing behind it.
/// - 不认识 charges the ease, counts a lapse, and returns the card to relearning due the same day,
///   so it stays in the session it was failed in. That is the point of a review session.
pub fn schedule(state: &CardState, grade: ReviewGrade, today: &str) -> Option<CardState> {
    if !crate::calendar::is_valid_day(today) {
        return None;
    }
    let mut next = state.clone();
    next.last_reviewed = today.to_owned();

    match grade {
        ReviewGrade::Unknown => {
            next.lapses = next.lapses.saturating_add(1);
            next.ease_permille = next
                .ease_permille
                .saturating_sub(LAPSE_EASE_PENALTY_PERMILLE)
                .max(MIN_EASE_PERMILLE);
            next.reviews = 0;
            next.interval_days = 0;
            next.due = today.to_owned();
        }
        ReviewGrade::Known => {
            next.reviews = next.reviews.saturating_add(1);
            next.interval_days = match next.reviews {
                1 => FIRST_INTERVAL_DAYS,
                2 => SECOND_INTERVAL_DAYS,
                _ => {
                    let grown = u64::from(state.interval_days) * u64::from(state.ease_permille);
                    // Round up, and never repeat an interval: an ease at the 1.3 floor applied to
                    // a one-day interval rounds back to one day, and a card whose interval cannot
                    // grow never leaves the daily queue.
                    let rounded = u32::try_from(grown.div_ceil(1000)).unwrap_or(MAX_INTERVAL_DAYS);
                    rounded.max(state.interval_days.saturating_add(1))
                }
            }
            .min(MAX_INTERVAL_DAYS);
            next.due = crate::calendar::shift_day(today, i64::from(next.interval_days))?;
        }
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-09-23";

    #[test]
    fn a_new_card_is_due_today_and_counts_as_new() {
        let card = CardState::new(TODAY);
        assert!(card.is_due(TODAY));
        assert!(card.is_new());
        assert!(card.is_valid());
        assert_eq!(card.ease_permille, INITIAL_EASE_PERMILLE);
    }

    #[test]
    fn recall_walks_the_sm2_intervals_and_leaves_the_ease_alone() {
        let mut card = CardState::new(TODAY);

        card = schedule(&card, ReviewGrade::Known, TODAY).unwrap();
        assert_eq!(card.interval_days, 1);
        assert_eq!(card.due, "2026-09-24");
        assert_eq!(card.reviews, 1);
        assert!(!card.is_new());

        card = schedule(&card, ReviewGrade::Known, "2026-09-24").unwrap();
        assert_eq!(card.interval_days, 6);
        assert_eq!(card.due, "2026-09-30");

        // 6 × 2.5 = 15
        card = schedule(&card, ReviewGrade::Known, "2026-09-30").unwrap();
        assert_eq!(card.interval_days, 15);
        assert_eq!(card.due, "2026-10-15");

        assert_eq!(card.ease_permille, INITIAL_EASE_PERMILLE);
        assert_eq!(card.lapses, 0);
        assert!(card.is_valid());
    }

    #[test]
    fn a_lapse_charges_the_ease_and_returns_the_card_to_the_same_day() {
        let mut card = CardState::new(TODAY);
        card = schedule(&card, ReviewGrade::Known, TODAY).unwrap();
        card = schedule(&card, ReviewGrade::Known, "2026-09-24").unwrap();

        card = schedule(&card, ReviewGrade::Unknown, "2026-09-30").unwrap();
        assert_eq!(card.lapses, 1);
        assert_eq!(card.reviews, 0, "a lapse relearns from I(1)");
        assert_eq!(card.interval_days, 0);
        assert_eq!(card.due, "2026-09-30", "a failed card stays in its session");
        assert!(card.is_due("2026-09-30"));
        assert_eq!(
            card.ease_permille,
            INITIAL_EASE_PERMILLE - LAPSE_EASE_PENALTY_PERMILLE
        );

        // Relearning restarts at I(1), not at the interval the card had before it lapsed.
        card = schedule(&card, ReviewGrade::Known, "2026-09-30").unwrap();
        assert_eq!(card.interval_days, 1);
    }

    #[test]
    fn the_ease_never_falls_below_the_sm2_floor() {
        let mut card = CardState::new(TODAY);
        for _ in 0..20 {
            card = schedule(&card, ReviewGrade::Unknown, TODAY).unwrap();
        }
        assert_eq!(card.ease_permille, MIN_EASE_PERMILLE);
        assert_eq!(card.lapses, 20);
        assert!(card.is_valid());
    }

    #[test]
    fn an_interval_always_grows_even_at_the_ease_floor() {
        // 1 × 1.3 rounds to 2 rather than back to 1; a card whose interval cannot grow would never
        // leave the daily queue.
        let card = CardState {
            interval_days: 1,
            ease_permille: MIN_EASE_PERMILLE,
            due: TODAY.to_owned(),
            reviews: 2,
            lapses: 3,
            last_reviewed: TODAY.to_owned(),
        };
        let next = schedule(&card, ReviewGrade::Known, TODAY).unwrap();
        assert_eq!(next.interval_days, 2);
        assert!(next.interval_days > card.interval_days);
    }

    #[test]
    fn the_interval_is_capped_so_a_due_date_stays_reachable() {
        let card = CardState {
            interval_days: MAX_INTERVAL_DAYS,
            ease_permille: INITIAL_EASE_PERMILLE,
            due: TODAY.to_owned(),
            reviews: 30,
            lapses: 0,
            last_reviewed: TODAY.to_owned(),
        };
        let next = schedule(&card, ReviewGrade::Known, TODAY).unwrap();
        assert_eq!(next.interval_days, MAX_INTERVAL_DAYS);
        assert!(next.is_valid());
        assert_eq!(next.due, "2036-09-20");
    }

    #[test]
    fn scheduling_rejects_a_day_it_cannot_parse() {
        let card = CardState::new(TODAY);
        assert!(schedule(&card, ReviewGrade::Known, "not-a-day").is_none());
        assert!(schedule(&card, ReviewGrade::Unknown, "2026-13-01").is_none());
    }

    #[test]
    fn due_comparison_is_chronological_across_a_year_boundary() {
        let card = CardState {
            due: "2026-12-31".to_owned(),
            ..CardState::new(TODAY)
        };
        assert!(!card.is_due("2026-12-30"));
        assert!(card.is_due("2026-12-31"));
        assert!(card.is_due("2027-01-01"));
    }

    #[test]
    fn an_overdue_card_is_scheduled_from_the_day_it_was_answered() {
        // A card due in July answered in September moves on from September, not from July: the
        // next gap is measured from when the user actually saw it.
        let card = CardState {
            interval_days: 6,
            ease_permille: INITIAL_EASE_PERMILLE,
            due: "2026-07-01".to_owned(),
            reviews: 2,
            lapses: 0,
            last_reviewed: "2026-06-25".to_owned(),
        };
        let next = schedule(&card, ReviewGrade::Known, TODAY).unwrap();
        assert_eq!(next.interval_days, 15);
        assert_eq!(next.due, "2026-10-08");
    }

    #[test]
    fn a_state_outside_the_stored_range_is_not_valid() {
        let card = CardState::new(TODAY);
        assert!(!CardState {
            due: "2026-13-01".to_owned(),
            ..card.clone()
        }
        .is_valid());
        assert!(!CardState {
            ease_permille: MIN_EASE_PERMILLE - 1,
            ..card.clone()
        }
        .is_valid());
        assert!(!CardState {
            ease_permille: INITIAL_EASE_PERMILLE + 1,
            ..card.clone()
        }
        .is_valid());
        assert!(!CardState {
            interval_days: MAX_INTERVAL_DAYS + 1,
            ..card.clone()
        }
        .is_valid());
        assert!(!CardState {
            last_reviewed: "yesterday".to_owned(),
            ..card
        }
        .is_valid());
    }
}

//! The community library: resources published by other users.
//!
//! Community skins live under [`crate::skin`] instead, next to the other skin
//! sources, because callers reach for them by what they are rather than by
//! where they came from.

pub(crate) fn valid_text(value: &str, minimum: usize, maximum: usize, multiline: bool) -> bool {
    let count = value.chars().count();
    let allowed_controls: &[char] = if multiline { &['\n', '\t'] } else { &[] };
    (minimum..=maximum).contains(&count)
        && !crate::has_disallowed_control_with_allowed(value, allowed_controls)
}

/// The query that asks the server to include [`CommunityModeration`] on the signed-in user's own items. It is opt-in because released clients refuse items with fields they do not know.
pub(crate) const MODERATION_FIELDS: &str = "fields=moderation";

/// Where the signed-in user's own published item stands with the moderators. Publication is moderated afterwards: an item is public as soon as it is published and moderators can remove it. Show the owner only that an item was removed (已下架); never a pending state, and the server never sends the reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommunityModeration {
    Approved,
    Pending,
    Removed,
    /// A state a newer server added; treated like an item that is not removed.
    #[serde(other)]
    Unknown,
}

impl CommunityModeration {
    /// Whether the owner should see the item as removed (已下架).
    pub fn is_removed(self) -> bool {
        self == Self::Removed
    }
}

pub(crate) const MAXIMUM_OFFSET: usize = 1_000_000;
pub(crate) const MAXIMUM_SEARCH_CHARACTERS: usize = 128;
pub(crate) const MAXIMUM_PAGE_ITEMS: usize = 20;
pub(crate) const MAXIMUM_JAVASCRIPT_INTEGER: u64 = 9_007_199_254_740_991;

pub(crate) fn valid_query(offset: usize, search: &str) -> bool {
    offset <= MAXIMUM_OFFSET && crate::text::is_bounded_chars(search, MAXIMUM_SEARCH_CHARACTERS)
}

pub(crate) fn valid_name(value: &str) -> bool {
    valid_text(value, 1, 32, false) && value.trim() == value
}

pub(crate) fn valid_description(value: &str) -> bool {
    valid_text(value, 0, 280, true)
}

pub(crate) fn valid_author(value: &str) -> bool {
    valid_text(value, 1, 128, false) && value.trim() == value
}

pub(crate) fn valid_rating(rating_count: u64, rating_average: f64, my_rating: u8) -> bool {
    rating_count <= MAXIMUM_JAVASCRIPT_INTEGER
        && my_rating <= 5
        && rating_average.is_finite()
        && (0.0..=5.0).contains(&rating_average)
        && (rating_count != 0 || rating_average == 0.0)
}

pub mod report;
pub mod resource;
pub mod resource_library;

use serde::{Deserialize, Serialize};

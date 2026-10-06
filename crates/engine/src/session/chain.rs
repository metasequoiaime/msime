//! The committed-word chain personal context learns along (`R/core/commit_chain.h`).

use std::time::Duration;

use crate::time::Instant;

/// How long a chain survives without input before the next word starts a new one.
pub const COMMIT_CHAIN_PAUSE_SECONDS: u64 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub canonical_pinyin: String,
    pub word: String,
    pub committed_at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct CommitChain {
    pub previous: Option<String>,
    pub earlier: Option<String>,
    /// The next word continues the composition the previous one came from.
    pub same_composition: bool,
    pub committed_at: Option<Instant>,
    pub last_pick: Option<Pick>,
}

impl CommitChain {
    /// Clears everything but `committed_at`.
    pub fn reset(&mut self) {
        self.previous = None;
        self.earlier = None;
        self.same_composition = false;
        self.last_pick = None;
    }

    /// Shifts `word` in as the newest context word and records whether composition input was left over after it, so the next word continues that composition (`commit_chain.h:39-45`).
    pub fn advance(&mut self, word: &str, composition_left: bool, now: Instant) {
        self.earlier = self.previous.take();
        self.previous = Some(word.to_owned());
        self.same_composition = composition_left;
        self.committed_at = Some(now);
    }

    /// Whether a new composition starting at `now` has waited long enough that it no longer follows the last word. A chain that never committed has no time to measure from, which the reference's zero time point also treated as long ago.
    pub fn paused(&self, now: Instant) -> bool {
        self.committed_at.is_none_or(|at| {
            now.saturating_duration_since(at) > Duration::from_secs(COMMIT_CHAIN_PAUSE_SECONDS)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_shifts_the_context_and_stamps_the_time() {
        let now = Instant::now();
        let mut chain = CommitChain::default();
        chain.advance("你好", false, now);
        chain.advance("世界", true, now + Duration::from_secs(1));
        assert_eq!(chain.previous.as_deref(), Some("世界"));
        assert_eq!(chain.earlier.as_deref(), Some("你好"));
        assert!(chain.same_composition);
        assert_eq!(chain.committed_at, Some(now + Duration::from_secs(1)));
    }

    #[test]
    fn reset_keeps_only_the_commit_time() {
        let now = Instant::now();
        let mut chain = CommitChain::default();
        chain.advance("你", true, now);
        chain.last_pick = Some(Pick {
            canonical_pinyin: "ni".to_owned(),
            word: "你".to_owned(),
            committed_at: now,
        });
        chain.reset();
        assert!(chain.previous.is_none() && chain.earlier.is_none());
        assert!(!chain.same_composition && chain.last_pick.is_none());
        assert_eq!(chain.committed_at, Some(now));
    }

    #[test]
    fn pause_is_strictly_longer_than_eight_seconds() {
        let now = Instant::now();
        let mut chain = CommitChain::default();
        assert!(chain.paused(now));
        chain.advance("你", false, now);
        assert!(!chain.paused(now + Duration::from_secs(8)));
        assert!(chain.paused(now + Duration::from_millis(8_001)));
        // A test clock that runs backwards must not count as a pause.
        assert!(!chain.paused(now - Duration::from_secs(1)));
    }
}

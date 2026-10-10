//! The personal word-sequence model (quanpin.md §12.2, `R/quanpin/personal_ngram.*`): in-memory counts and their scoring. Persistence, background flushing and decay are `user_dictionary::ngram_store`.
//!
//! Six tables keep their key spaces apart: a word that is both a context and a continuation (the usual case) must not read its context total as its own count, and a (u, v) context must not collide with a (v, w) pair of the same two strings. Keys are 64-bit FNV-1a of the words joined by NUL, the scheme the static tables use. Not thread-safe; the store owns the locking.

use std::collections::HashMap;

use super::ngram::{fnv1a_mix, fnv1a_separate, fnv1a_words, SENTENCE_START};

/// Constants of the personal model. The defaults are the product values; tests and tools may pass others, but nothing reads these from user configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersonalNgramOptions {
    /// Share of P(w|v) taken from the pair count; the rest comes from how often w was committed at all.
    pub lambda: f64,
    /// Confidence in a context grows as c(v) / (c(v) + confidence_k), so a context seen once barely counts.
    pub confidence_k: f64,
    /// Personal data never takes more than this share of a score, so the static model always keeps a say.
    pub max_confidence: f64,
    /// Absolute discount applied to trigram counts before backing off to the bigram estimate.
    pub trigram_discount: f64,
    /// Distinct pairs plus distinct triples allowed before the store halves every count and drops zeros, repeatedly until at most three quarters of this remain.
    pub max_transitions: usize,
}

impl Default for PersonalNgramOptions {
    fn default() -> Self {
        Self {
            lambda: 0.8,
            confidence_k: 8.0,
            max_confidence: 0.5,
            trigram_discount: 0.75,
            max_transitions: 200_000,
        }
    }
}

/// One committed word after its context. Empty `earlier`/`previous` mean the start of a chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PersonalTransition {
    pub earlier: String,
    pub previous: String,
    pub word: String,
    pub times: u32,
}

/// What the model knows about one context; `confidence == 0` means nothing. Resolved once so scoring many continuations only hashes each continuation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PersonalContext {
    pub confidence: f64,
    /// The FNV state after the previous word and its separator: a continuation's pair key is this state with the continuation's bytes mixed in.
    pub previous_key: u64,
    /// The same for the (earlier, previous) context; 0 when that context was never seen.
    pub triple_key: u64,
    pub context_total: u64,
    pub triple_total: u64,
    pub followers: u64,
}

/// Counts keyed by hash. A count that reaches zero is removed, so `len` is the number of live keys, what `entries` reports.
#[derive(Debug, Clone, Default)]
struct CountTable(HashMap<u64, u32>);

impl CountTable {
    fn get(&self, key: u64) -> u32 {
        self.0.get(&key).copied().unwrap_or(0)
    }

    /// Returns the count before the addition. Counts saturate rather than wrap (PN:56-60).
    fn add(&mut self, key: u64, delta: u32) -> u32 {
        let count = self.0.entry(key).or_insert(0);
        let before = *count;
        *count = before.saturating_add(delta);
        before
    }

    /// Returns how much was subtracted, which is less than `delta` when the count was lower (decay halved it): floored at zero rather than wrapping, the way the journal drops a row instead of storing a negative count.
    fn subtract(&mut self, key: u64, delta: u32) -> u32 {
        let Some(count) = self.0.get_mut(&key) else {
            return 0;
        };
        let removed = (*count).min(delta);
        *count -= removed;
        if *count == 0 {
            self.0.remove(&key);
        }
        removed
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.0.clear();
    }
}

fn key_of(word: &str) -> u64 {
    fnv1a_words(&[word])
}

fn key_of_pair(previous: &str, word: &str) -> u64 {
    fnv1a_words(&[previous, word])
}

fn key_of_triple(earlier: &str, previous: &str, word: &str) -> u64 {
    fnv1a_words(&[earlier, previous, word])
}

fn or_start(word: &str) -> &str {
    if word.is_empty() {
        SENTENCE_START
    } else {
        word
    }
}

#[derive(Debug, Clone, Default)]
pub struct PersonalNgram {
    options: PersonalNgramOptions,
    pairs: CountTable,
    context_totals: CountTable,
    word_counts: CountTable,
    triples: CountTable,
    triple_totals: CountTable,
    triple_followers: CountTable,
    total: u64,
}

impl PersonalNgram {
    pub fn new(options: PersonalNgramOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    pub fn options(&self) -> &PersonalNgramOptions {
        &self.options
    }

    /// PN:176-186. Transitions whose previous word is the chain start carry no trigram.
    pub fn add(&mut self, transition: &PersonalTransition) {
        if transition.word.is_empty() || transition.times == 0 {
            return;
        }
        let previous = or_start(&transition.previous);
        self.add_pair(previous, &transition.word, transition.times);
        if previous != SENTENCE_START {
            self.add_triple(
                or_start(&transition.earlier),
                previous,
                &transition.word,
                transition.times,
            );
        }
    }

    /// PN:188-196. `previous` is already resolved: the chain start is `SENTENCE_START`, not empty.
    pub fn add_pair(&mut self, previous: &str, word: &str, count: u32) {
        if count == 0 {
            return;
        }
        self.pairs.add(key_of_pair(previous, word), count);
        self.context_totals.add(key_of(previous), count);
        self.word_counts.add(key_of(word), count);
        self.total += u64::from(count);
    }

    /// PN:198-206. A triple seen for the first time adds one follower to its context.
    pub fn add_triple(&mut self, earlier: &str, previous: &str, word: &str, count: u32) {
        if count == 0 {
            return;
        }
        let context = key_of_pair(earlier, previous);
        if self
            .triples
            .add(key_of_triple(earlier, previous, word), count)
            == 0
        {
            self.triple_followers.add(context, 1);
        }
        self.triple_totals.add(context, count);
    }

    /// Takes back what `add` added (PN:208-218). The store removes journal rows pair by pair and triple by triple, so only the tests take back a whole transition.
    #[cfg(test)]
    pub fn remove(&mut self, transition: &PersonalTransition) {
        if transition.word.is_empty() || transition.times == 0 {
            return;
        }
        let previous = or_start(&transition.previous);
        self.remove_pair(previous, &transition.word, transition.times);
        if previous != SENTENCE_START {
            self.remove_triple(
                or_start(&transition.earlier),
                previous,
                &transition.word,
                transition.times,
            );
        }
    }

    /// PN:220-228. The context and word totals shrink by what the pair actually lost.
    pub fn remove_pair(&mut self, previous: &str, word: &str, count: u32) {
        let removed = self.pairs.subtract(key_of_pair(previous, word), count);
        if removed == 0 {
            return;
        }
        self.context_totals.subtract(key_of(previous), removed);
        self.word_counts.subtract(key_of(word), removed);
        self.total -= self.total.min(u64::from(removed));
    }

    /// PN:230-241.
    pub fn remove_triple(&mut self, earlier: &str, previous: &str, word: &str, count: u32) {
        let context = key_of_pair(earlier, previous);
        let key = key_of_triple(earlier, previous, word);
        let removed = self.triples.subtract(key, count);
        if removed == 0 {
            return;
        }
        if self.triples.get(key) == 0 {
            self.triple_followers.subtract(context, 1);
        }
        self.triple_totals.subtract(context, removed);
    }

    /// Drops every count and keeps the options.
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.pairs.clear();
        self.context_totals.clear();
        self.word_counts.clear();
        self.triples.clear();
        self.triple_totals.clear();
        self.triple_followers.clear();
        self.total = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// `word` 作为后继被提交过的计数之和，不论前一个词是什么（显式选词每次记两次，见 `session::learning`）。只读；九键按它判断用户用过哪些简拼词（#6185）。
    pub fn word_count(&self, word: &str) -> u32 {
        if word.is_empty() {
            return 0;
        }
        self.word_counts.get(key_of(word))
    }

    /// Sum of every pair count.
    #[cfg(test)]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Distinct pair and triple entries, what decay compares with `max_transitions`.
    pub fn entries(&self) -> usize {
        self.pairs.len() + self.triples.len()
    }

    /// PN:267-291. `None` is the chain start.
    pub fn context(&self, earlier: Option<&str>, previous: Option<&str>) -> PersonalContext {
        let mut result = PersonalContext::default();
        if self.total == 0 {
            return result;
        }
        let previous = previous.unwrap_or(SENTENCE_START);
        let previous_hash = key_of(previous);
        result.context_total = u64::from(self.context_totals.get(previous_hash));
        if result.context_total == 0 {
            return result;
        }
        let seen = result.context_total as f64;
        result.confidence =
            (seen / (seen + self.options.confidence_k)).min(self.options.max_confidence);
        result.previous_key = fnv1a_separate(previous_hash);
        if previous != SENTENCE_START {
            let triple_context = key_of_pair(earlier.unwrap_or(SENTENCE_START), previous);
            result.triple_total = u64::from(self.triple_totals.get(triple_context));
            if result.triple_total > 0 {
                result.followers = u64::from(self.triple_followers.get(triple_context));
                result.triple_key = fnv1a_separate(triple_context);
            }
        }
        result
    }

    /// PN:306-318: the discounted trigram estimate backed off to the interpolated bigram.
    pub fn probability(&self, context: &PersonalContext, word: &str) -> f64 {
        if context.context_total == 0 || word.is_empty() {
            return 0.0;
        }
        let bigram = self.bigram_estimate(
            context.previous_key,
            context.context_total,
            key_of(word),
            word,
        );
        if context.triple_total == 0 {
            return bigram;
        }
        let total = context.triple_total as f64;
        let seen = f64::from(self.triples.get(fnv1a_mix(context.triple_key, word)));
        let discount = self.options.trigram_discount;
        (seen - discount).max(0.0) / total + discount * context.followers as f64 / total * bigram
    }

    /// The model's confidence in `previous` as a context, ignoring any earlier word.
    #[cfg(test)]
    pub fn confidence(&self, previous: &str) -> f64 {
        self.context(None, Some(previous)).confidence
    }

    /// PN:293-304 for one pair, without resolving a context first.
    #[cfg(test)]
    pub fn bigram_probability(&self, previous: &str, word: &str) -> f64 {
        let previous_hash = key_of(previous);
        self.bigram_estimate(
            fnv1a_separate(previous_hash),
            u64::from(self.context_totals.get(previous_hash)),
            key_of(word),
            word,
        )
    }

    /// `probability` for an explicit (earlier, previous) context.
    #[cfg(test)]
    pub fn trigram_probability(&self, earlier: &str, previous: &str, word: &str) -> f64 {
        self.probability(&self.context(Some(earlier), Some(previous)), word)
    }

    /// `lambda * pair / context_total + (1 - lambda) * word_count / total` (PN:293-304).
    fn bigram_estimate(
        &self,
        pair_state: u64,
        context_total: u64,
        word_key: u64,
        word: &str,
    ) -> f64 {
        let mut probability = 0.0;
        if context_total > 0 {
            probability += self.options.lambda
                * f64::from(self.pairs.get(fnv1a_mix(pair_state, word)))
                / context_total as f64;
        }
        if self.total > 0 {
            probability += (1.0 - self.options.lambda) * f64::from(self.word_counts.get(word_key))
                / self.total as f64;
        }
        probability
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn near(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    pub(crate) fn transition(
        earlier: &str,
        previous: &str,
        word: &str,
        times: u32,
    ) -> PersonalTransition {
        PersonalTransition {
            earlier: earlier.to_owned(),
            previous: previous.to_owned(),
            word: word.to_owned(),
            times,
        }
    }

    /// One committed run of words, each word counted `times`.
    pub(crate) fn record_run(model: &mut PersonalNgram, words: &[&str], times: u32) {
        let mut earlier = "";
        let mut previous = "";
        for word in words {
            model.add(&transition(earlier, previous, word, times));
            earlier = previous;
            previous = word;
        }
    }

    #[test]
    fn model_math() {
        let mut model = PersonalNgram::default();
        assert!(model.is_empty() && model.confidence(SENTENCE_START) == 0.0);

        // START -> 我 -> 想, each counted twice.
        record_run(&mut model, &["我", "想"], 2);
        assert_eq!(model.total(), 4);
        assert_eq!(model.entries(), 3);
        // c(我) = 2, so mu = 2 / (2 + 8).
        assert!(near(model.confidence("我"), 0.2));
        // P2 = 0.8 * 2/2 + 0.2 * 2/4.
        assert!(near(model.bigram_probability("我", "想"), 0.9));
        // Seen (START, 我) context: (2 - 0.75) / 2 + 0.75 * 1 / 2 * P2.
        assert!(near(
            model.trigram_probability(SENTENCE_START, "我", "想"),
            0.625 + 0.375 * 0.9
        ));
        // An unseen (u, v) context falls back to the bigram estimate unchanged.
        assert!(near(model.trigram_probability("别", "我", "想"), 0.9));
        // An unseen continuation of a seen trigram context gets only the back-off mass.
        assert!(near(
            model.trigram_probability(SENTENCE_START, "我", "他"),
            0.375 * model.bigram_probability("我", "他")
        ));

        let mut capped = PersonalNgram::default();
        for _ in 0..100 {
            capped.add(&transition("", "甲", "乙", 1));
        }
        assert!(
            near(capped.confidence("甲"), 0.5),
            "confidence is capped at max_confidence"
        );

        // A chain start carries no trigram, so the entries are the pair alone.
        let mut opening = PersonalNgram::default();
        opening.add(&transition("", "", "甲", 3));
        assert_eq!(opening.entries(), 1);
        assert!(near(
            opening.trigram_probability("任意", SENTENCE_START, "甲"),
            opening.bigram_probability(SENTENCE_START, "甲")
        ));
    }

    #[test]
    fn key_domains_are_separate() {
        // 甲 is only ever a context and 乙 only ever a continuation. One shared table keyed by the word would give 乙 a context total of 1 and 甲 a word count of 1.
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", 1);
        assert_eq!(model.confidence("乙"), 0.0);
        assert_eq!(model.bigram_probability("丙", "甲"), 0.0);

        // A pair (甲, 乙) and a trigram context (甲, 乙) hash the same two strings.
        let mut pairs = PersonalNgram::default();
        pairs.add_pair(SENTENCE_START, "甲", 1);
        pairs.add_pair("甲", "乙", 5);
        assert_eq!(
            pairs.context(Some(SENTENCE_START), Some("乙")).triple_total,
            0
        );
        pairs.add_triple("甲", "乙", "丙", 1);
        pairs.add_pair("乙", "丙", 1);
        assert!(near(
            pairs.bigram_probability("甲", "乙"),
            0.8 + 0.2 * 5.0 / 7.0
        ));
    }

    #[test]
    fn empty_context_knows_nothing() {
        let mut model = PersonalNgram::default();
        assert_eq!(model.context(None, None), PersonalContext::default());
        record_run(&mut model, &["我", "想"], 1);
        let unseen = model.context(None, Some("他"));
        assert_eq!(unseen.confidence, 0.0);
        assert_eq!(model.probability(&unseen, "想"), 0.0);
        let start = model.context(None, None);
        assert!(start.confidence > 0.0);
        assert_eq!(
            start.triple_total, 0,
            "the chain start has no trigram context"
        );
        assert_eq!(model.probability(&start, ""), 0.0);
    }

    #[test]
    fn remove_takes_back_add() {
        let mut model = PersonalNgram::default();
        record_run(&mut model, &["我", "想", "去"], 2);
        let entries = model.entries();
        model.add(&transition("我", "想", "区", 1));
        assert_eq!(model.context(Some("我"), Some("想")).followers, 2);
        model.remove(&transition("我", "想", "区", 1));
        assert_eq!(model.entries(), entries);
        assert_eq!(model.context(Some("我"), Some("想")).followers, 1);
        assert_eq!(model.total(), 6);

        // Removing more than is there floors at zero.
        model.remove(&transition("我", "想", "去", 5));
        assert_eq!(model.total(), 4);
        assert_eq!(model.context(Some("我"), Some("想")).triple_total, 0);
        assert_eq!(model.bigram_probability("想", "去"), 0.0);

        model.clear();
        assert!(model.is_empty());
        assert_eq!(model.entries(), 0);
    }

    #[test]
    fn counts_saturate() {
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", u32::MAX);
        model.add_pair("甲", "乙", 5);
        assert_eq!(
            model.context(None, Some("甲")).context_total,
            u64::from(u32::MAX)
        );
        assert_eq!(model.total(), u64::from(u32::MAX) + 5);
    }
}

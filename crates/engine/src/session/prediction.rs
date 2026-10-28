//! 上屏后的空闲候选；没有预编辑，不参与普通组字的自动提交。

use std::path::PathBuf;

use super::online::OnlineRequestGuard;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::text::is_all_han;
use crate::types::{CandidateSource, OnlineInputMode, OnlineQuery, SchemeType, WordItem};
use crate::user_dictionary::ngram_store::PersonalNgramStore;

const LIMIT: usize = 8;

pub(super) struct Prediction {
    pub enabled: bool,
    allowed: bool,
    pub rows: Vec<WordItem>,
    seeds: Vec<(String, String)>,
    path: PathBuf,
    database: Option<PinyinDatabase>,
    local_rows: Vec<WordItem>,
    local_seeds: Vec<(String, String)>,
    context: String,
    request: OnlineRequestGuard,
}

impl Prediction {
    pub fn new(enabled: bool, path: PathBuf) -> Self {
        Self {
            enabled,
            allowed: true,
            rows: Vec::new(),
            seeds: Vec::new(),
            path,
            database: None,
            local_rows: Vec::new(),
            local_seeds: Vec::new(),
            context: String::new(),
            request: OnlineRequestGuard::new(),
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.seeds.clear();
        self.local_rows.clear();
        self.local_seeds.clear();
        self.context.clear();
        self.request.invalidate();
    }

    pub fn set_allowed(&mut self, allowed: bool) {
        self.allowed = allowed;
        if !allowed {
            self.clear();
        }
    }

    pub fn selected(&self, index: usize) -> Option<(WordItem, String, String)> {
        let row = self.rows.get(index)?.clone();
        let (word, pinyin) = self.seeds.get(index)?.clone();
        Some((row, word, pinyin))
    }

    pub fn show(
        &mut self,
        word: &str,
        pinyin: &str,
        previous: &str,
        personal: Option<&PersonalNgramStore>,
    ) {
        self.clear();
        if !self.enabled || !self.allowed || !is_all_han(word) {
            return;
        }
        self.context = crate::text::last_characters(word, 64).to_owned();
        if let Some(personal) = personal {
            for (next, count) in personal.next_words(previous, LIMIT) {
                if !is_all_han(&next) {
                    continue;
                }
                self.push(
                    &next,
                    "",
                    count.min(i64::MAX as u64) as i64,
                    next.clone(),
                    String::new(),
                );
            }
        }
        if pinyin.is_empty() {
            self.remember_local();
            return;
        }
        let database = self
            .database
            .get_or_insert_with(|| PinyinDatabase::open(&self.path));
        let rows = database.query_next_words(word, pinyin, LIMIT);
        for row in rows {
            let seed_word = format!("{word}{}", row.value);
            let seed_pinyin = format!("{pinyin}'{}", row.key);
            self.push(&row.value, &row.key, row.weight, seed_word, seed_pinyin);
        }
        self.remember_local();
    }

    fn remember_local(&mut self) {
        self.local_rows.clone_from(&self.rows);
        self.local_seeds.clone_from(&self.seeds);
    }

    pub fn active(&self) -> bool {
        self.enabled && self.allowed && !self.context.is_empty()
    }

    pub fn query(&self, scheme: SchemeType) -> Option<OnlineQuery> {
        (self.active() && !self.request.exhausted).then(|| OnlineQuery {
            mode: OnlineInputMode::Prediction,
            scheme,
            generation: self.request.generation,
            session_id: self.request.session_id,
            identity: format!("prediction:{}", self.context),
            query_text: self.context.clone(),
            cache_key: format!("prediction:{}", self.context),
            cloud_eligible: true,
            ai_eligible: false,
            pinyin_segments: Vec::new(),
        })
    }

    pub fn clear_cloud(&mut self) {
        self.rows.clone_from(&self.local_rows);
        self.seeds.clone_from(&self.local_seeds);
        self.request.invalidate();
    }

    /// 本地首选保持在前，最多三条新云联想随后插入，其余本地行继续保留。
    pub fn apply(&mut self, query: &OnlineQuery, words: &[String]) -> bool {
        if self.query(query.scheme).as_ref() != Some(query)
            || words.is_empty()
            || words.len() > LIMIT
            || words
                .iter()
                .any(|word| word.len() > 512 || !is_all_han(word))
        {
            return false;
        }
        let mut rows = Vec::with_capacity(LIMIT);
        let mut seeds = Vec::with_capacity(LIMIT);
        let local: Vec<_> = self.local_rows.iter().zip(&self.local_seeds).collect();
        if let Some((row, seed)) = local.first() {
            rows.push((*row).clone());
            seeds.push((*seed).clone());
        }
        for word in words {
            if rows.len() >= usize::from(!local.is_empty()) + 3
                || local.iter().any(|(row, _)| row.word == word.as_str())
                || rows.iter().any(|row| row.word == word.as_str())
            {
                continue;
            }
            let row = WordItem::new("", word, 1, CandidateSource::CloudSuggestion, "");
            rows.push(row);
            seeds.push((format!("{}{}", self.context, word), String::new()));
        }
        for (row, seed) in local
            .into_iter()
            .skip(usize::from(!self.local_rows.is_empty()))
        {
            if rows.len() == LIMIT {
                break;
            }
            rows.push(row.clone());
            seeds.push(seed.clone());
        }
        self.rows = rows;
        self.seeds = seeds;
        true
    }

    fn push(
        &mut self,
        word: &str,
        pinyin: &str,
        weight: i64,
        seed_word: String,
        seed_pinyin: String,
    ) {
        if self.rows.len() == LIMIT || self.rows.iter().any(|row| row.word == word) {
            return;
        }
        let mut row = WordItem::new("", word, weight, CandidateSource::Generated, pinyin);
        row.sentence_words.push(word.to_owned());
        self.rows.push(row);
        self.seeds.push((seed_word, seed_pinyin));
    }
}

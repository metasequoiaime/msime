//! 上屏后的空闲候选；没有预编辑，不参与普通组字的自动提交。

use std::path::PathBuf;

use crate::dictionary::pinyin::PinyinDatabase;
use crate::text::is_all_han;
use crate::types::{CandidateSource, WordItem};
use crate::user_dictionary::ngram_store::PersonalNgramStore;

const LIMIT: usize = 8;

pub(super) struct Prediction {
    pub enabled: bool,
    allowed: bool,
    pub rows: Vec<WordItem>,
    seeds: Vec<(String, String)>,
    path: PathBuf,
    database: Option<PinyinDatabase>,
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
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.seeds.clear();
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

//! 上屏后的空闲候选；没有预编辑，不参与普通组字的自动提交。

use std::path::PathBuf;

use crate::dictionary::pinyin::PinyinDatabase;
use crate::lattice::offensive::is_offensive;
use crate::ordering::{apply_order, prediction_order, CandidateScorer};
use crate::text::is_all_han;
use crate::types::{CandidateSource, WordItem};
use crate::user_dictionary::ngram_store::PersonalNgramStore;

const LIMIT: usize = 8;

/// 打开候选池（[`Prediction::set_pool`]）时词库续写多取到几条，模型排过之后仍只给 [`LIMIT`] 条。与 `PinyinDatabase::query_next_words` 每张表的上限相同。
pub(super) const POOL: usize = 32;

pub(super) struct Prediction {
    pub enabled: bool,
    allowed: bool,
    /// 宿主挂了候选打分器：词库续写多取到 [`POOL`] 条，等 [`Self::order`] 按模型排过再截到 [`LIMIT`]。
    pool: bool,
    pub rows: Vec<WordItem>,
    seeds: Vec<(String, String)>,
    /// `rows` 开头有几行是个人习惯（`PersonalNgramStore::next_words`），其余是词库续写。
    personal: usize,
    /// 候选池里排在 `rows` 之外的词库续写，连同各自的种子；只在打开候选池时有。
    spare: Vec<(WordItem, (String, String))>,
    /// 这次上屏的文字，等 [`Self::order`] 拿它接在上文后面打分；排过一次、或者没有可排的就是 `None`，所以每次上屏只打一次分。
    pending: Option<String>,
    path: PathBuf,
    database: Option<PinyinDatabase>,
}

impl Prediction {
    pub fn new(enabled: bool, path: PathBuf) -> Self {
        Self {
            enabled,
            allowed: true,
            pool: false,
            rows: Vec::new(),
            seeds: Vec::new(),
            personal: 0,
            spare: Vec::new(),
            pending: None,
            path,
            database: None,
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.seeds.clear();
        self.personal = 0;
        self.spare.clear();
        self.pending = None;
    }

    pub fn set_allowed(&mut self, allowed: bool) {
        self.allowed = allowed;
        if !allowed {
            self.clear();
        }
    }

    /// 只影响之后的联想；关掉时正在显示的行留着（它们已经是按词频的前 [`LIMIT`] 条），只是不再等模型排序。
    pub fn set_pool(&mut self, enabled: bool) {
        self.pool = enabled;
        if !enabled {
            self.spare.clear();
            self.pending = None;
        }
    }

    pub fn selected(&self, index: usize) -> Option<(WordItem, String, String)> {
        let row = self.rows.get(index)?.clone();
        let (word, pinyin) = self.seeds.get(index)?.clone();
        Some((row, word, pinyin))
    }

    /// `word` 与 `pinyin` 是查词库续写用的种子（连续选联想时是上一次的种子接上选中的行），`committed` 是这一次真正上屏的文字，打分时接在上文后面。
    pub fn show(
        &mut self,
        word: &str,
        pinyin: &str,
        committed: &str,
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
        self.personal = self.rows.len();
        if pinyin.is_empty() {
            return;
        }
        let database = self
            .database
            .get_or_insert_with(|| PinyinDatabase::open(&self.path));
        let limit = if self.pool { POOL } else { LIMIT };
        let rows = database.query_next_words(word, pinyin, limit);
        for row in rows {
            let seed_word = format!("{word}{}", row.value);
            let seed_pinyin = format!("{pinyin}'{}", row.key);
            self.push(&row.value, &row.key, row.weight, seed_word, seed_pinyin);
        }
        if self.pool
            && self.personal < LIMIT
            && self.rows.len() - self.personal + self.spare.len() >= 2
        {
            self.pending = Some(committed.to_owned());
        }
    }

    /// 按模型给这次上屏后的词库续写排序（规则见 [`prediction_order`]），个人习惯行原样留在前面；`before` 是这次上屏之前的文字。排过一次就不再排。返回显示的行变了没有。
    pub fn order(&mut self, scorer: &mut dyn CandidateScorer, before: &str) -> bool {
        let Some(committed) = self.pending.take() else {
            return false;
        };
        let mut dictionary: Vec<(WordItem, (String, String))> = self
            .rows
            .drain(self.personal..)
            .zip(self.seeds.drain(self.personal..))
            .chain(self.spare.drain(..))
            .collect();
        let shown_before: Vec<_> = dictionary
            .iter()
            .take(LIMIT - self.personal)
            .map(|(row, _)| row.word.clone())
            .collect();
        let texts: Vec<&str> = dictionary
            .iter()
            .map(|(row, _)| row.word.as_str())
            .collect();
        let context = format!("{before}{committed}");
        let order = prediction_order(scorer, &context, &texts);
        if let Some(order) = &order {
            apply_order(&mut dictionary, order);
        }
        dictionary.truncate(LIMIT - self.personal);
        let changed = dictionary
            .iter()
            .map(|(row, _)| row.word.as_str())
            .ne(shown_before.iter().map(|word| word.as_str()));
        for (row, seed) in dictionary {
            self.rows.push(row);
            self.seeds.push(seed);
        }
        changed
    }

    fn push(
        &mut self,
        word: &str,
        pinyin: &str,
        weight: i64,
        seed_word: String,
        seed_pinyin: String,
    ) {
        let full = self.rows.len() == LIMIT;
        // 个人习惯行在前、最多 `LIMIT` 条，所以放不下的只会是词库续写；打开候选池时留给模型排序。
        if (full && !self.pool)
            || self.rows.iter().any(|row| row.word == word)
            || self.spare.iter().any(|(row, _)| row.word == word)
        {
            return;
        }
        // 联想是用户什么都没打时给的词，粗俗词不给，见 `lattice::offensive`。词库联想给的是词的后半截（上屏 巨 联想 乳），所以连同上屏的部分（`seed_word`）一起查。
        if is_offensive(word) || is_offensive(&seed_word) {
            return;
        }
        let mut row = WordItem::new("", word, weight, CandidateSource::Generated, pinyin);
        row.sentence_words.push(word.to_owned());
        if full {
            self.spare.push((row, (seed_word, seed_pinyin)));
            return;
        }
        self.rows.push(row);
        self.seeds.push((seed_word, seed_pinyin));
    }
}

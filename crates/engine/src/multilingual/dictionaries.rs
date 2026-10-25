use std::ops::Range;
use std::path::Path;

use super::{decode, Conversion, Language, Lexeme, MAX_INPUT_BYTES, MAX_LEXEMES};
use crate::assets;
use crate::dictionary::english::EnglishDictionary;
use crate::japanese::{JapaneseProvider, JapaneseRomajiScheme};
use crate::pinyin::segment::cut_one_piece_min_segments;
use crate::quanpin::QuanpinEngine;
use crate::types::QueryRequest;
use crate::RuntimePaths;

/// 每次输入变化最多查询的原始跨度数，每个跨度最多查询三个所选提供者。
const MAX_SPAN_QUERIES: usize = 4096;
/// 单个提供者词面的字节上限；合并后的同语言片段可包含多个词面。
const MAX_SURFACE_BYTES: usize = 256;
/// 与现有中文词网格的最长词一致，更长中文片段由多条词条连接。
const MAX_CHINESE_SYLLABLES: usize = 7;

/// 使用所选语言的现有词库提供者组成完整混合路径。构造应在会话准备阶段完成，
/// 不要在每次按键时重建。它持有独立查询状态，中文 n 元表和日文模型仍由现有缓存共享。
/// 语言集合或模型路径改变时重建；词典修订到达时调用 [`Self::clear`]。
pub struct DictionaryComposer {
    languages: Vec<Language>,
    chinese: Option<QuanpinEngine>,
    english: Option<EnglishDictionary>,
    japanese: Option<JapaneseProvider>,
    japanese_ready: bool,
    japanese_scheme: JapaneseRomajiScheme,
    japanese_request: QueryRequest,
    input: String,
    lexemes: Vec<Lexeme>,
    answer: Option<Conversion>,
}

impl DictionaryComposer {
    /// 只打开显式选择的语言，缺失资源按原有提供者语义返回没有词条，不下载或补写资源。
    pub fn new(paths: &RuntimePaths, languages: &[Language]) -> Self {
        Self::with_japanese_model(paths, languages, &paths.resource(assets::JAPANESE_MODEL))
    }

    /// 使用宿主已解析的日文资源包路径，其它词库仍取同一份运行时目录。
    pub fn with_japanese_model(paths: &RuntimePaths, languages: &[Language], model: &Path) -> Self {
        let languages: Vec<_> = [Language::Chinese, Language::English, Language::Japanese]
            .into_iter()
            .filter(|language| languages.contains(language))
            .collect();
        let active = languages.len() >= 2;
        Self {
            chinese: (active && languages.contains(&Language::Chinese))
                .then(|| QuanpinEngine::new(paths)),
            english: (active && languages.contains(&Language::English)).then(|| {
                EnglishDictionary::open(&paths.dictionary(assets::ENGLISH_DICTIONARY), None, None)
            }),
            japanese: (active && languages.contains(&Language::Japanese))
                .then(|| JapaneseProvider::new(model)),
            languages,
            japanese_ready: false,
            japanese_scheme: JapaneseRomajiScheme::new(),
            japanese_request: QueryRequest::default(),
            input: String::new(),
            lexemes: Vec::new(),
            answer: None,
        }
    }

    /// 会话准备时沿现有提供者预热；未选日文时不读模型，不等待后台完成。
    pub fn warm_up_in_background(&self) {
        if let Some(japanese) = &self.japanese {
            japanese.warm_up_in_background();
        }
    }

    /// 只返回全串覆盖的混合备选，完整中文的存在性由 `chinese_complete` 保留。
    /// 当前仅接受小写拉丁编码；非法输入或预算不足时不返回部分路径。
    pub fn convert(&mut self, input: &str) -> Option<Conversion> {
        let japanese_ready = self
            .japanese
            .as_mut()
            .is_some_and(JapaneseProvider::prepare_if_cached);
        if japanese_ready != self.japanese_ready {
            self.forget_input();
            self.japanese_ready = japanese_ready;
        }
        if input == self.input {
            return self.answer.clone();
        }
        if self.languages.len() < 2
            || input.is_empty()
            || input.len() > MAX_INPUT_BYTES
            || !input.bytes().all(|byte| byte.is_ascii_lowercase())
        {
            self.forget_input();
            return None;
        }
        let common = self
            .input
            .bytes()
            .zip(input.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        self.lexemes.retain(|lexeme| lexeme.input.end <= common);
        let mut queried = 0;
        for end in common + 1..=input.len() {
            for start in 0..end {
                queried += 1;
                if queried > MAX_SPAN_QUERIES {
                    self.forget_input();
                    return None;
                }
                if self.query_span(input, start..end).is_none() || self.lexemes.len() > MAX_LEXEMES
                {
                    self.forget_input();
                    return None;
                }
            }
        }
        let mut answer = decode(input, &self.lexemes, &self.languages);
        if let Some(conversion) = &mut answer {
            self.rerank_chinese(input, conversion);
        }
        self.input.clear();
        self.input.push_str(input);
        self.answer = answer;
        self.answer.clone()
    }

    /// 词典修订后丢弃已确认及未命中的跨度。不可变模型更换需重建本适配器。
    pub fn clear(&mut self) {
        self.forget_input();
        if let Some(chinese) = &mut self.chinese {
            chinese.reset_cache();
        }
        if let Some(japanese) = &mut self.japanese {
            japanese.reset_cache();
        }
    }

    fn forget_input(&mut self) {
        self.input.clear();
        self.lexemes.clear();
        self.answer = None;
    }

    fn push(&mut self, input: Range<usize>, language: Language, text: String) -> Option<()> {
        if text.len() > MAX_SURFACE_BYTES {
            return None;
        }
        if !text.is_empty() {
            self.lexemes.push(Lexeme {
                input,
                language,
                text,
                // 同范围、同语言的次选没有其它后续状态，首选以外的边不会胜出。
                rank: 0,
            });
        }
        Some(())
    }

    fn query_span(&mut self, input: &str, range: Range<usize>) -> Option<()> {
        let raw = &input[range.clone()];
        if let Some(chinese) = &mut self.chinese {
            let syllables = cut_one_piece_min_segments(raw, true);
            if !syllables.is_empty() && syllables.len() <= MAX_CHINESE_SYLLABLES {
                if let Some(row) = chinese.conversion_rows(&syllables, 1).into_iter().next() {
                    self.push(range.clone(), Language::Chinese, row.value)?;
                }
            }
        }
        if let Some(row) = self
            .english
            .as_ref()
            .and_then(|english| english.query_exact(raw))
        {
            self.push(range.clone(), Language::English, row.word.to_string())?;
        }
        if let Some(japanese) = self.japanese.as_mut().filter(|_| self.japanese_ready) {
            self.japanese_scheme.set_raw_input(raw, "");
            self.japanese_scheme
                .build_request_into(&mut self.japanese_request);
            let exact = japanese
                .whole_reading_lemma(&self.japanese_request)
                .is_some_and(|lemma| lemma.exact && lemma.morae >= 3);
            if exact {
                if let Some(path) = japanese
                    .whole_reading_best_path(&self.japanese_request)
                    .filter(|path| path.unknown_kana == 0)
                {
                    self.push(range, Language::Japanese, path.text)?;
                }
            }
        }
        Some(())
    }

    fn rerank_chinese(&mut self, input: &str, conversion: &mut Conversion) {
        let Some(chinese) = &mut self.chinese else {
            return;
        };
        for segment in &mut conversion.segments {
            if segment.language != Language::Chinese {
                continue;
            }
            let syllables = cut_one_piece_min_segments(&input[segment.input.clone()], true);
            let spans = chinese.convert_pinned(&syllables, &[]);
            if spans.first().is_some_and(|span| span.start == 0)
                && spans.last().is_some_and(|span| span.end == syllables.len())
                && spans.windows(2).all(|pair| pair[0].end == pair[1].start)
            {
                segment.text = spans.into_iter().map(|span| span.word).collect();
            }
        }
    }
}

#[cfg(test)]
#[path = "dictionary_tests.rs"]
mod tests;

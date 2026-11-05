use super::*;

impl InputSession {
    /// 每笔追加到未确认的整段；语言边界不会确认旧候选。
    pub(super) fn continue_bilingual_glide(
        &mut self,
        spelling: &str,
        next: &[WordItem],
    ) -> KeyResult {
        let previous_letters = self
            .raw_with_cases()
            .bytes()
            .filter(|&letter| letter != b'\'')
            .count();
        let previous: Vec<_> = self
            .candidates()
            .iter()
            .filter(|row| {
                self.glide_candidates.is_some()
                    || row.pinyin.bytes().filter(|&letter| letter != b'\'').count()
                        == previous_letters
            })
            .take(ranking::CANDIDATE_LIMIT)
            .cloned()
            .collect();
        let result = self.insert_glide_letters(spelling);
        let combined = self.raw_with_cases().to_owned();
        let mut full: Vec<_> = self
            .engine
            .query_raw_candidates(&combined, &combined)
            .into_iter()
            .filter(|row| complete_chinese_glide_candidate(row, &combined))
            .take(ranking::CANDIDATE_LIMIT)
            .collect();
        // 整段查询只对纯中文优先；混合短语由完整的各笔候选组合，不交出任何前缀。
        if previous
            .first()
            .is_some_and(|row| ranking::contains_english(&row.word))
            || next
                .first()
                .is_some_and(|row| ranking::contains_english(&row.word))
        {
            full.clear();
        }
        let mut rows = ranking::interleave(full, ranking::combinations(&previous, next, &combined));
        for row in &mut rows {
            row.source = CandidateSource::Generated;
            row.corrected_from = combined.clone().into();
        }
        self.glide_candidates = (!rows.is_empty()).then_some((combined, rows));
        self.glide_selection_pending = true;
        result
    }

    /// 滑行后的字母编辑只改拼写，不进入普通打字可能触发的顶字或边界确认路径。
    pub(in crate::session) fn edit_pending_glide(&mut self, value: u8) -> Option<KeyResult> {
        if !self.glide_selection_pending || !self.has_composition() {
            return None;
        }
        if !value.is_ascii_alphabetic() && value != b'\'' {
            return Some(KeyResult::handled());
        }
        let caret = self.caret_position();
        let mut text = self.raw_with_cases().to_owned();
        text.insert(caret, char::from(value));
        Some(self.replace_editing_text(&text, caret + 1))
    }
}

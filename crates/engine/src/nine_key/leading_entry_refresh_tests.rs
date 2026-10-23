use super::leading_entry_tests::candidate_push as push_ranked;
use super::*;

// 固定 8f011cb59c 的完整刷新，去掉注释并替换入列为单次查询实验。
impl NineKeySession {
    pub(super) fn candidate_entry_refresh(&mut self) {
        self.candidates.clear();
        self.reading.clear();
        if !self.active() {
            self.path_scratch.pages.clear();
            self.spellings.clear();
            self.sentence_seeds.clear();
            self.lock_undo.clear();
            self.initial = None;
            self.single_character = false;
            self.strokes.clear();
            return;
        }
        if self.english_only || !self.pinyin {
            self.path_scratch.pages.clear();
            self.spellings.clear();
            self.candidates = self.english_candidates(false);
            return;
        }
        self.lock_undo.resize_with(self.locked.len(), || None);
        let grid = self.grid;
        let table = grid.spelling_table();
        let locked_length = self.locked_length();
        let remaining = remaining_digits(&self.digits, locked_length);
        let unlocked_digit_count = remaining.len();
        let mut split_offsets = [0usize; DIGIT_LIMIT];
        for (index, split) in self.splits.iter().enumerate() {
            split_offsets[index] = split - locked_length;
        }
        let splits = &split_offsets[..self.splits.len()];
        let initial = self.initial.map(|initial| initial.letter);
        let starts_right =
            |piece: &str| initial.is_none_or(|letter| piece.as_bytes().first() == Some(&letter));
        let dictionary_options = self.dictionary_options();
        let mut alternatives = std::mem::take(&mut self.path_buffer);
        let alternatives = if remaining.is_empty() {
            self.spellings.clear();
            alternatives.clear();
            alternatives.push(Vec::new());
            alternatives
        } else {
            let dictionary = self
                .dictionary
                .get_or_insert_with(|| open_dictionary(&self.paths, dictionary_options.clone()));
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            table.spellings_for_into(
                remaining,
                locked_length,
                splits.first().copied(),
                prior,
                &mut self.spellings,
                &mut self.spelling_match_indices,
            );
            self.spellings.retain(|syllable| starts_right(syllable));
            table.split_paths_with_scratch(
                remaining,
                splits,
                prior,
                PATH_LIMIT,
                &mut self.path_scratch,
                &mut alternatives,
            );
            alternatives.retain(|path| path.first().is_none_or(|piece| starts_right(piece)));
            if self.locked.is_empty() && splits.is_empty() && initial.is_none() {
                if !self.sentence_seeds.is_empty() && remaining.len() >= SEEDED_TRUNCATE_DIGITS {
                    alternatives.truncate(SEEDED_PATH_LIMIT);
                }
                seeded_paths(
                    &mut alternatives,
                    &self.sentence_seeds,
                    remaining,
                    table,
                    prior,
                );
            }
            alternatives.extend(
                self.spellings
                    .iter()
                    .filter(|spelling| grid.syllable_code_len(spelling) <= remaining.len())
                    .map(|spelling| vec![spelling.clone()]),
            );
            alternatives
        };

        let mut locked_key = String::new();
        append_path_key(&mut locked_key, "", &self.locked);
        let mut dictionary = self
            .dictionary
            .get_or_insert_with(|| open_dictionary(&self.paths, dictionary_options.clone()))
            .row_cache_batch();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        let mut candidates = std::mem::take(&mut self.candidates);
        candidates.clear();
        let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
        let mut leading = LeadingMap::default();
        let prefer_exact = !self.splits.is_empty();
        let initials =
            self.locked.is_empty() && initial.is_none() && initials_apply(remaining.len(), splits);
        let initials_lead = initials && !splits.is_empty();
        let filter_dominated =
            self.fuzzy.rules == 0 && matches!(grid, KeyGrid::NineKey | KeyGrid::FourteenKey);
        for (index, path) in alternatives.iter().enumerate() {
            append_path_key(&mut key, &locked_key, path);
            if key.is_empty() || !query_key_is_new(&alternatives, index, &key, queried.as_ref()) {
                continue;
            }
            let full_len = self.locked.len() + path.len();
            let accepts_reading = |candidate: &WordItem, cached: bool| {
                let word = candidate.word.as_str();
                let pinyin = candidate.pinyin.as_str();
                let canonical = candidate.canonical_pinyin.as_str();
                let fuzzy = candidate.fuzzy;
                if self.single_character_only && is_han_phrase(word) {
                    return false;
                }
                let canonical = if canonical.is_empty() {
                    pinyin
                } else {
                    canonical
                };
                if reading_syllable_count(canonical) > full_len {
                    return false;
                }
                let matched = if fuzzy { pinyin } else { canonical };
                if !agrees_with_locked(matched, &locked_key) {
                    return false;
                }
                if let Some(letter) = initial {
                    let next = matched
                        .split('\'')
                        .nth(self.locked.len())
                        .and_then(|syllable| syllable.as_bytes().first());
                    if next != Some(&letter) {
                        return false;
                    }
                }
                if cached && filter_dominated {
                    if let Some(best) = leading_rank(&leading, &candidates, word) {
                        let Some(covered) = grid.matching_code_prefix_len(matched, &self.digits)
                        else {
                            return false;
                        };
                        if best
                            <= rank_key_for_reading(
                                candidate,
                                covered,
                                canonical,
                                prefer_exact,
                                initials_lead,
                                grid,
                            )
                        {
                            return false;
                        }
                    }
                }
                true
            };
            for mut candidate in
                dictionary.query_filtered_rows(&key, &key, self.fuzzy, accepts_reading)
            {
                let canonical = take_candidate_canonical_pinyin(&mut candidate);
                let matched = if candidate.fuzzy {
                    candidate.pinyin.as_str()
                } else {
                    canonical.as_str()
                };
                let Some(covered) = grid.matching_code_prefix_len(matched, &self.digits) else {
                    continue;
                };
                candidate.pinyin.clear();
                candidate.pinyin.push_str(&self.digits[..covered]);
                candidate.canonical_pinyin = canonical;
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                    grid,
                );
            }
            if let Some(seen) = queried.as_mut() {
                seen.insert(key.clone());
            }
        }
        if let KeyGrid::NineKeyShuangpin(table) = grid {
            let codes = (self.locked.is_empty()
                && initial.is_none()
                && splits.is_empty()
                && !self.single_character_only)
                .then(|| shuangpin_word_codes(grid, remaining))
                .flatten();
            if let Some(codes) = codes {
                for mut candidate in
                    dictionary.query_jianpin_codes_per_table(&codes, SHUANGPIN_WORD_SCAN_LIMIT)
                {
                    if !shuangpin_word_spells(table, &candidate.canonical_pinyin, remaining) {
                        continue;
                    }
                    candidate.pinyin.clear();
                    candidate.pinyin.push_str(&self.digits);
                    push_ranked(
                        &mut candidates,
                        &mut leading,
                        candidate,
                        prefer_exact,
                        initials_lead,
                        grid,
                    );
                }
            }
        }
        let mut boost = None;
        if let Some(codes) = initials
            .then(|| initials_codes(grid, remaining, INITIALS_CODE_LIMIT))
            .flatten()
        {
            let personal =
                self.learning && self.personal_context && !dictionary.personal_model().is_empty();
            let shuangpin = match grid {
                KeyGrid::NineKeyShuangpin(table) => Some(table),
                _ => None,
            };
            let mut rows = if personal {
                dictionary.query_jianpin_codes_per_table(&codes, INITIALS_SCAN_LIMIT)
            } else if shuangpin.is_some() {
                dictionary.query_jianpin_codes(&codes, INITIALS_SCAN_LIMIT)
            } else {
                dictionary.query_jianpin_codes(&codes, INITIALS_ROW_LIMIT)
            };
            if let Some(table) = shuangpin {
                rows.retain(|row| table.spells_initials(&row.canonical_pinyin, remaining));
                if !personal {
                    rows.truncate(INITIALS_ROW_LIMIT);
                }
            }
            let mut used = HashMap::new();
            if personal {
                let model = dictionary.personal_model();
                for row in &rows {
                    let count = model.word_count(&row.word);
                    if count > 0 {
                        used.insert(row.word.clone(), count);
                    }
                }
                drop(model);
                if rows.len() > INITIALS_ROW_LIMIT {
                    rows.sort_by_key(|row| !used.contains_key(&row.word));
                    rows.truncate(INITIALS_ROW_LIMIT);
                }
            }
            if self.frequency.mode != FrequencyAdjustmentMode::Disabled && !used.is_empty() {
                boost = Some(InitialsBoost {
                    used,
                    frequency: self.frequency,
                });
            }
            for mut candidate in rows {
                candidate.pinyin.clear();
                candidate.pinyin.push_str(&self.digits);
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                    grid,
                );
            }
        }
        drop(dictionary);
        let filtering = self.single_character || !self.strokes.is_empty();
        if filtering {
            let strokes = (!self.strokes.is_empty()).then_some(&self.stroke_texts);
            candidates.retain(|item| passes_filter(&item.word, self.single_character, strokes));
        }
        rank_candidates(
            &mut candidates,
            prefer_exact,
            initials_lead,
            boost.as_ref(),
            grid,
        );
        self.rerank_sentences(&mut candidates);
        let mut seeds = std::mem::take(&mut self.seed_buffer);
        next_seeds(
            grid,
            &candidates,
            &self.digits,
            &mut self.sentence_seeds,
            &mut seeds,
        );
        self.seed_buffer = std::mem::replace(&mut self.sentence_seeds, seeds);
        let (emoji, kaomoji, symbols) = if filtering {
            (Vec::new(), Vec::new(), Vec::new())
        } else {
            self.expressive_candidates(&alternatives, &candidates)
        };

        let unanswered = candidates.is_empty();
        let mut english = if initial.is_some() || filtering {
            Vec::new()
        } else {
            self.english_candidates(unanswered)
        };
        if !english.is_empty() {
            let first = english.remove(0);
            let slot = if first.weight > 0 {
                candidates.len().min(1)
            } else {
                candidates.len()
            };
            candidates.insert(slot, first);
            candidates.extend(english);
        }
        let mut candidates = insert_expressive_rows(candidates, emoji, kaomoji, symbols);
        if let Some(clock) = self
            .inline_date_time
            .as_ref()
            .filter(|_| initial.is_none() && !filtering)
        {
            let digits = self.digits.as_str();
            if let Some((_keyword, anchor, kind)) =
                inline_date_time_keyword(|keyword| keyword_spells_digits(grid, keyword, digits))
            {
                insert_inline_date_time(&mut candidates, anchor, kind, digits, || clock(), None);
            }
        }
        positions::apply_fixed_positions(
            &self.paths.user(assets::USER_JOURNAL),
            &format!("nine-key:{}:{locked_key}", self.digits),
            &mut candidates,
            false,
            None,
            false,
        );
        keep_expressive_rows_off_the_lead(&mut candidates);
        if let Some(front) = candidates.first() {
            let offset = if locked_key.is_empty() {
                0
            } else {
                locked_key.len() + 1
            };
            if let Some(rest) = front.canonical_pinyin.get(offset..) {
                let preferred = rest.split('\'').next().unwrap_or_default();
                if unlocked_digit_count < 2 || preferred.len() <= unlocked_digit_count {
                    if let Some(found) = self.spellings.iter().position(|s| s == preferred) {
                        self.spellings[..=found].rotate_right(1);
                    }
                }
            }
        }
        let mut reading = std::mem::take(&mut self.reading);
        self.write_reading(candidates.first(), &mut reading);
        self.reading = reading;
        self.candidates = candidates;
        self.path_buffer = alternatives;
        let remaining = remaining_digits(&self.digits, locked_length);
        if remaining.is_empty() {
            if let Some(Some(undo)) = self.lock_undo.last() {
                self.spellings = undo.choices.clone();
            }
        } else {
            self.append_key_choices(locked_length);
        }
    }
}

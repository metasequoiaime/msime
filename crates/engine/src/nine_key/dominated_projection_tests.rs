use super::isolated_test as isolation;
use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 固定 92058b68c2eda173906abb9ace0723836fc661eb 的完整刷新，只改名、扩大测试可见性和移除注释。
impl NineKeySession {
    pub(super) fn before_dominated_refresh(&mut self) {
        self.candidates.clear();
        self.spellings.clear();
        self.reading.clear();
        if !self.active() {
            self.sentence_seeds.clear();
            self.lock_undo.clear();
            self.initial = None;
            self.single_character = false;
            self.strokes.clear();
            return;
        }
        if self.english_only || !self.pinyin {
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
        let mut syllables = Vec::new();
        let dictionary_options = self.dictionary_options();
        let alternatives = if remaining.is_empty() {
            vec![Vec::new()]
        } else {
            let dictionary = self
                .dictionary
                .get_or_insert_with(|| open_dictionary(&self.paths, dictionary_options.clone()));
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            syllables =
                table.spellings_for(remaining, locked_length, splits.first().copied(), prior);
            syllables.retain(|syllable| starts_right(syllable));
            // 与生产刷新同一个查询上限，对照只比较支配过滤本身。
            let mut alternatives =
                table.split_paths_limited(remaining, splits, prior, QUERY_PATH_LIMIT);
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
                syllables
                    .iter()
                    .filter(|spelling| grid.syllable_code_len(spelling) <= remaining.len())
                    .map(|spelling| vec![spelling.clone()]),
            );
            alternatives
        };
        self.spellings = syllables;

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
        for (index, path) in alternatives.iter().enumerate() {
            append_path_key(&mut key, &locked_key, path);
            if key.is_empty() || !query_key_is_new(&alternatives, index, &key, queried.as_ref()) {
                continue;
            }
            let full_len = self.locked.len() + path.len();
            let accepts_reading = |word: &str, pinyin: &str, canonical: &str, fuzzy: bool| {
                if self.single_character_only && is_han_phrase(word) {
                    return false;
                }
                let canonical = if canonical.is_empty() {
                    pinyin
                } else {
                    canonical
                };
                if canonical.matches('\'').count() >= full_len {
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
                true
            };
            for mut candidate in dictionary.query_filtered(&key, &key, self.fuzzy, accepts_reading)
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
                candidate.pinyin = self.digits.as_str().into();
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
            &self.ranking_context(),
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
        self.reading = self.reading_for(candidates.first());
        self.candidates = candidates;
        let remaining = remaining_digits(&self.digits, locked_length);
        if remaining.is_empty() {
            if let Some(Some(undo)) = self.lock_undo.last() {
                self.spellings = undo.choices.clone();
            }
        } else {
            let choices = self.key_choices(remaining);
            self.spellings.extend(choices);
        }
    }
}

fn open(paths: &RuntimePaths, grid: KeyGrid, fuzzy: u32, single: bool) -> NineKeySession {
    let mut session = NineKeySession::new(
        paths,
        false,
        FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Disabled,
            ..Default::default()
        },
        FuzzyPinyinOptions { rules: fuzzy },
        EnglishInputOptions::default(),
        true,
        single,
    );
    session.grid = grid;
    session
}

fn assert_session(actual: &NineKeySession, expected: &NineKeySession) {
    assert_eq!(actual.snapshot(), expected.snapshot());
    assert_eq!(actual.candidates, expected.candidates);
    assert_eq!(actual.candidates.capacity(), expected.candidates.capacity());
    assert_eq!(actual.sentence_seeds, expected.sentence_seeds);
    assert_eq!(actual.seed_buffer, expected.seed_buffer);
    for (actual, expected) in actual.candidates.iter().zip(&expected.candidates) {
        for (actual, expected) in [
            (&actual.word, &expected.word),
            (&actual.canonical_pinyin, &expected.canonical_pinyin),
            (&actual.corrected_from, &expected.corrected_from),
        ] {
            assert_eq!(actual.capacity(), expected.capacity());
        }
        // 九键给 `pinyin` 换上一份按长度新建的 `SharedString`，不再复用旧缓冲区，容量只能比内容。
        assert_eq!(actual.pinyin, expected.pinyin);
        assert_eq!(
            actual.sentence_words.capacity(),
            expected.sentence_words.capacity()
        );
        for (actual, expected) in actual.sentence_words.iter().zip(&expected.sentence_words) {
            assert_eq!(actual.capacity(), expected.capacity());
        }
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("ni", "你", 100),
        ("mi", "米", 80),
        ("hao", "好", 100),
        ("ni'hao", "你好", 500),
        ("ni'hao", "拟号", 100),
        ("ni'men'hao", "你们好", 800),
        ("zi'you", "自由", 400),
        ("zhi'you", "只有", 500),
        ("zi", "字", 80),
        ("zhi", "只", 100),
        ("an", "安", 70),
        ("xi'an", "西安", 300),
    ] {
        fixture.insert(key, word, weight);
    }
    fixture
}

#[test]
fn dominated_rows_are_rejected_before_field_copies() {
    isolation::run(
        concat!(
            module_path!(),
            "::dominated_rows_are_rejected_before_field_copies"
        ),
        assert_dominated_rows_are_rejected_before_field_copies,
    )
    .unwrap();
}

fn assert_dominated_rows_are_rejected_before_field_copies() {
    let fixture = fixture();
    for index in 0..32 {
        fixture.insert("ni", &format!("合成前缀{index}😀"), 50 - index);
    }
    let mut before = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    let mut after = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    before.digits = "64426".to_owned();
    after.digits = before.digits.clone();
    before.before_dominated_refresh();
    after.refresh();
    let (_, old_count) = count(|| before.before_dominated_refresh());
    let (_, new_count) = count(|| after.refresh());
    assert_session(&after, &before);
    eprintln!("完整热刷新申请：{old_count} → {new_count}");
    // 新路径同时复用首候选读音行、主读音列表、拼写匹配索引、左列选项、切分路径输出和搜索页缓冲。对照路径复制行字段现在是 `SharedString` 克隆，不再分配，差额从 1123 降到 777。
    assert_eq!(
        new_count + 777,
        old_count,
        "重复前缀行和读音字符串不能先复制再丢弃"
    );
}

#[test]
fn initials_rows_reuse_their_existing_pinyin_capacity() {
    let fixture = fixture();
    fixture
        .insert("mei'tian", "每天", 900)
        .insert("ming'tian", "明天", 800)
        .insert("na'tian", "那天", 300);
    let mut before = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    let mut after = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    before.digits = "68".to_owned();
    after.digits = before.digits.clone();
    before.before_dominated_refresh();
    after.refresh();
    let (_, old_count) = count(|| before.before_dominated_refresh());
    let (_, new_count) = count(|| after.refresh());
    assert_eq!(after.snapshot(), before.snapshot());
    assert_eq!(after.candidates, before.candidates);
    let initials = after
        .candidates
        .iter()
        .find(|item| item.word == "每天")
        .expect("简拼行存在");
    assert!(
        initials.pinyin.capacity() >= "mei'tian".len(),
        "简拼行应保留完整读音的字符串容量"
    );
    assert!(
        new_count + 2 <= old_count,
        "简拼行应复用已有 pinyin 容量：旧 {old_count}，新 {new_count}"
    );
}

#[test]
fn dominated_projection_matches_frozen_refresh_across_grids_and_locks() {
    isolation::run(
        concat!(
            module_path!(),
            "::dominated_projection_matches_frozen_refresh_across_grids_and_locks"
        ),
        assert_dominated_projection_matches_frozen_refresh_across_grids_and_locks,
    )
    .unwrap();
}

fn assert_dominated_projection_matches_frozen_refresh_across_grids_and_locks() {
    let fixture = fixture();
    for (key, weight) in [
        ("ni", 100),
        ("mi", 300),
        ("ni'hao", 10),
        ("mi'gan", 400),
        ("zi", 40),
        ("zhi", 80),
    ] {
        fixture.insert(key, "合成同文😀", weight);
    }
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    for grid in [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(shuangpin),
    ] {
        for fuzzy in [0, crate::types::fuzzy_rule::Z_ZH] {
            for single in [false, true] {
                for reading in [
                    "ni",
                    "ni'hao",
                    "ni'men'hao",
                    "zi'you",
                    "xi'an",
                    "ni'badtail",
                ] {
                    for restriction in 0..4 {
                        let mut before = open(&fixture.paths, grid, fuzzy, single);
                        let mut after = open(&fixture.paths, grid, fuzzy, single);
                        for session in [&mut before, &mut after] {
                            session.digits = grid.encode(reading);
                            if restriction == 1 && reading.starts_with("ni'") {
                                session.locked.push("ni".to_owned());
                            }
                            if restriction == 2 && session.digits.len() > 1 {
                                session.splits.push(1);
                            }
                            if restriction == 3 {
                                session.initial = Some(Initial {
                                    letter: b'n',
                                    typed: session.digits.len(),
                                    splits: 0,
                                });
                            }
                        }
                        for _ in 0..2 {
                            let (_, old_count) = count(|| before.before_dominated_refresh());
                            let (_, new_count) = count(|| after.refresh());
                            assert_session(&after, &before);
                            assert!(new_count <= old_count, "{grid:?}/{reading}/{restriction}/{fuzzy}/{single}: {new_count}>{old_count}");
                        }
                    }
                }
            }
        }
    }
}

fn before_rank_key(
    item: &WordItem,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) -> RankKey {
    let completion =
        prefer_exact && grid.reading_code_len(&item.canonical_pinyin) > item.pinyin.len();
    let initials = before_is_initials_row(item);
    (
        Reverse(item.pinyin.len()),
        item.source.is_generated_or_fallback(),
        initials != initials_lead,
        completion,
        item.fuzzy,
        Reverse(before_comparable_weight(item)),
    )
}

fn before_comparable_weight(item: &WordItem) -> i64 {
    if item.source != CandidateSource::Generated || item.canonical_pinyin.is_empty() {
        return item.weight;
    }
    let syllables = item.canonical_pinyin.split('\'').count();
    item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * syllables as f64) as i64
}

fn before_is_initials_row(item: &WordItem) -> bool {
    let digits = item.pinyin.len();
    digits >= 2
        && !item.source.is_generated_or_fallback()
        && item.canonical_pinyin.split('\'').count() == digits
        && item
            .canonical_pinyin
            .bytes()
            .filter(u8::is_ascii_lowercase)
            .count()
            > digits
}

#[test]
fn projected_keys_equal_the_frozen_owned_keys() {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    for grid in [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(shuangpin),
    ] {
        for source in [
            CandidateSource::Database,
            CandidateSource::UserDatabase,
            CandidateSource::Generated,
            CandidateSource::Fallback,
            CandidateSource::NeuralKeyboard,
            CandidateSource::EnglishDictionary,
        ] {
            for canonical in ["", "ni", "ni'hao", "lü'e", "'xi''an'", "合成😀"] {
                for pinyin in ["", "ni", "ni'hao", "'xi''an'"] {
                    for covered in [0, 1, 2, 3, 8, 17] {
                        for flags in 0..8 {
                            let mut row =
                                WordItem::new(pinyin, "合成同文😀", -5000, source, canonical);
                            row.fuzzy = flags & 1 != 0;
                            let mut owned = row.clone();
                            let final_canonical = take_candidate_canonical_pinyin(&mut owned);
                            owned.pinyin = "6".repeat(covered).into();
                            owned.canonical_pinyin = final_canonical;
                            let borrowed = if canonical.is_empty() {
                                pinyin
                            } else {
                                canonical
                            };
                            assert_eq!(
                                rank_key_for_reading(
                                    &row,
                                    covered,
                                    borrowed,
                                    flags & 2 != 0,
                                    flags & 4 != 0,
                                    grid
                                ),
                                before_rank_key(&owned, flags & 2 != 0, flags & 4 != 0, grid)
                            );
                            assert_eq!(
                                rank_key(&owned, flags & 2 != 0, flags & 4 != 0, grid),
                                before_rank_key(&owned, flags & 2 != 0, flags & 4 != 0, grid)
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn invalidated_rows_and_owned_outputs_remain_independent() {
    let fixture = fixture();
    let mut before = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    let mut after = open(&fixture.paths, KeyGrid::NineKey, 0, false);
    for digits in ["64426", "94", "64426"] {
        before.digits = digits.to_owned();
        after.digits = digits.to_owned();
        before.before_dominated_refresh();
        after.refresh();
        assert_session(&after, &before);
    }
    let mut retained = after.snapshot();
    let expected = retained.clone();
    fixture.insert("ni'hao", "合成新增", 900);
    before.before_dominated_refresh();
    after.refresh();
    assert_session(&after, &before);
    assert_eq!(retained, expected);
    retained.candidates.clear();
    before.dictionary.as_mut().unwrap().reset_cache();
    after.dictionary.as_mut().unwrap().reset_cache();
    before.before_dominated_refresh();
    after.refresh();
    assert_session(&after, &before);
    before.dictionary = None;
    after.dictionary = None;
    before.before_dominated_refresh();
    after.refresh();
    assert_session(&after, &before);
}

#[test]
#[ignore = "手动运行普通系统分配器下的完整刷新配对计时"]
fn dominated_projection_paired_latency() {
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    fn batch(session: &mut NineKeySession, old: bool, iterations: usize) -> Duration {
        let started = Instant::now();
        for _ in 0..iterations {
            if old {
                black_box(&mut *session).before_dominated_refresh();
            } else {
                black_box(&mut *session).refresh();
            }
            black_box(&session.candidates);
        }
        started.elapsed()
    }
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    let mut failed = Vec::new();
    for (grid_name, grid) in [
        ("nine", KeyGrid::NineKey),
        ("fourteen", KeyGrid::FourteenKey),
        ("shuangpin", KeyGrid::NineKeyShuangpin(shuangpin)),
    ] {
        for fuzzy in [0, crate::types::fuzzy_rule::Z_ZH] {
            for width in [0, 32] {
                let fixture = fixture();
                for index in 0..width {
                    fixture.insert("ni", &format!("合成前缀{index}😀"), 50 - index);
                }
                let mut before = open(&fixture.paths, grid, fuzzy, false);
                let mut after = open(&fixture.paths, grid, fuzzy, false);
                before.digits = grid.encode("ni'hao");
                after.digits = before.digits.clone();
                before.before_dominated_refresh();
                after.refresh();
                assert_session(&after, &before);
                let mut iterations = 1;
                while batch(&mut before, true, iterations) < Duration::from_millis(50) {
                    iterations *= 2;
                }
                let mut ratios = Vec::new();
                for pair in 0..40 {
                    let (old, new) = if pair % 2 == 0 {
                        (
                            batch(&mut before, true, iterations),
                            batch(&mut after, false, iterations),
                        )
                    } else {
                        let new = batch(&mut after, false, iterations);
                        (batch(&mut before, true, iterations), new)
                    };
                    ratios.push(new.as_secs_f64() / old.as_secs_f64());
                }
                ratios.sort_by(f64::total_cmp);
                let median = (ratios[19] + ratios[20]) / 2.0;
                let threshold = if grid_name != "shuangpin" && fuzzy == 0 && width == 32 {
                    0.98
                } else {
                    1.05
                };
                println!("{grid_name} fuzzy={fuzzy} width={width} iterations={iterations} median={median:.6} p10={:.6} p90={:.6} threshold={threshold:.2}", ratios[3], ratios[35]);
                if median >= threshold {
                    failed.push((grid_name, fuzzy, width, median, threshold));
                }
            }
        }
    }
    assert!(failed.is_empty(), "完整矩阵未通过：{failed:?}");
}

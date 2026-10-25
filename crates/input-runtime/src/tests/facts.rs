//! `Runtime::facts()`、`has_composition()` 和两个 epoch 计数器的单元测试。
//!
//! 生命周期核心（host-api）只从这几样东西推导"已选择""撤回已选""上屏的 body/tail"和视图事件，所以这里逐条覆盖设计 §2.3 列出的每条选词路径、每个 outcome，以及明确不算"已选择"的路径。

use super::super::runtime::empty_result;
use super::super::*;
use super::{
    compose_stroke, compose_zhuyin, korean_runtime, open_korean_hanja, phrase_runtime,
    stroke_runtime, zhuyin_runtime,
};

/// 每个引擎调用的结果都能由字段直接控制的测试引擎。
///
/// `Fixture` 和 `PhraseEngine` 各自服务于别的测试，改它们会牵动无关用例；这里需要的是整句改字、候选来源、网格组字和"上屏与候选对不上"这些它们表达不了的状态。
#[derive(Default)]
struct FactEngine {
    scheme: u8,
    text: String,
    words: Vec<String>,
    /// 与 `words` 平行；长度不符时全部按 0。
    sources: Vec<u8>,
    /// 非空表示正在整句改字：选中候选只钉住一段，不上屏。
    conversion: String,
    spelling_symbols: String,
    /// 空表示 `"none"`。
    local_mode: String,
    candidate_list_open: bool,
    key_grid: Option<KeyGrid>,
    /// 选词后剩下的读音；`None` 表示选词结束组字。
    remaining_after_select: Option<String>,
    /// 选词时引擎实际上屏的文本，用来制造"上屏和候选对不上"的情况。
    select_commit_override: Option<String>,
    /// 设了以后，字母键在继续组字的同时上屏这段文本（韩文音节被下一键结束时就是这样）。
    character_commits: Option<String>,
    /// 设了以后，`expand_initial_candidates` 把这些候选接到列表末尾。
    withheld: Vec<String>,
    /// 五笔拼音混输；顶字关掉时字母只有混输打开才有地方容纳。
    mixed_pinyin: bool,
}

impl FactEngine {
    fn commit(text: impl Into<String>) -> EngineResult {
        EngineResult {
            handled: true,
            has_commit: true,
            commit: text.into(),
            diagnostic: String::new(),
        }
    }
}

impl InputEngine for FactEngine {
    fn wubi_mixed_pinyin(&self) -> bool {
        self.mixed_pinyin
    }
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        let composing = !self.text.is_empty() || !self.conversion.is_empty();
        let count = self.words.len();
        Ok(EngineSnapshot {
            prediction: false,
            scheme: self.scheme,
            key_grid: self.key_grid,
            grid_composing: self.key_grid.is_some() && !self.text.is_empty(),
            candidate_codes: vec![String::new(); count],
            candidate_annotations: vec![String::new(); count],
            candidate_sources: if self.sources.len() == count {
                self.sources.clone()
            } else {
                vec![0; count]
            },
            candidate_positions: vec![0; count],
            candidate_corrected: vec![false; count],
            candidate_answers_key: vec![true; count],
            candidate_list_open: self.candidate_list_open,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: self.scheme == 2 && self.text.len() == 4 && count == 1,
            local_mode: if self.local_mode.is_empty() {
                "none".into()
            } else {
                self.local_mode.clone()
            },
            spelling_symbols: self.spelling_symbols.clone(),
            preedit: self.text.clone(),
            editing_text: self.text.clone(),
            caret_position: self.text.len(),
            conversion: self.conversion.clone(),
            conversion_focus_start: 0,
            conversion_focus_end: usize::from(!self.conversion.is_empty()),
            candidates: if composing {
                self.words.clone()
            } else {
                Vec::new()
            },
            ..EngineSnapshot::default()
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        if self.spelling_symbols.as_bytes().contains(&value) || value.is_ascii_alphabetic() {
            self.text.push(char::from(value));
            if let Some(commit) = self.character_commits.take() {
                return Ok(Self::commit(commit));
            }
            return Ok(empty_result(true));
        }
        Ok(empty_result(false))
    }
    fn grid_key(&mut self, letter: u8) -> Result<EngineResult, RuntimeError> {
        if self.key_grid != Some(KeyGrid::FourteenKey) {
            return Ok(empty_result(false));
        }
        self.text.push(char::from(letter));
        Ok(empty_result(true))
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        let composing = !self.text.is_empty();
        match command {
            Command::Backspace => {
                self.text.pop();
                Ok(empty_result(composing))
            }
            // 原样上屏读音；候选为空时的 CommitCandidate 在笔画、藏文里也是上屏方案自己的文本。
            Command::CommitRaw | Command::CommitCandidate if composing => {
                let text = std::mem::take(&mut self.text);
                self.conversion.clear();
                Ok(Self::commit(text))
            }
            _ => {
                self.text.clear();
                self.conversion.clear();
                Ok(empty_result(composing))
            }
        }
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if !self.conversion.is_empty() {
            // 整句改字里选中候选：钉住这一段，句子换个写法，不上屏。
            self.conversion = format!("{}*", self.conversion);
            return Ok(empty_result(true));
        }
        let commit = self
            .select_commit_override
            .take()
            .unwrap_or_else(|| self.words[index].clone());
        self.text = self.remaining_after_select.take().unwrap_or_default();
        Ok(Self::commit(commit))
    }
    fn select_edge(
        &mut self,
        index: usize,
        edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        let word = &self.words[index];
        let character = match edge {
            CandidateEdge::FirstHan => word.chars().next(),
            CandidateEdge::LastHan => word.chars().next_back(),
        };
        self.text.clear();
        Ok(Self::commit(
            character.map(String::from).unwrap_or_default(),
        ))
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if self.text.is_empty() && self.conversion.is_empty() {
            return Ok(empty_result(false));
        }
        let commit = self
            .words
            .get(index)
            .cloned()
            .unwrap_or_else(|| self.text.clone());
        self.text.clear();
        self.conversion.clear();
        Ok(Self::commit(commit))
    }
    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        if value == b',' {
            return Ok(Self::commit("，"));
        }
        Ok(empty_result(false))
    }
    fn expand_initial_candidates(&mut self) -> Result<bool, RuntimeError> {
        if self.withheld.is_empty() {
            return Ok(false);
        }
        self.words.append(&mut self.withheld);
        Ok(true)
    }
}

fn pinyin() -> FactEngine {
    FactEngine {
        words: vec!["你好".into(), "拟好".into(), "泥".into()],
        sources: vec![0, 5, 8],
        ..FactEngine::default()
    }
}

fn focused(engine: FactEngine) -> Runtime<FactEngine> {
    let mut runtime = Runtime::new(engine, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

fn type_keys<E: InputEngine>(runtime: &mut Runtime<E>, keys: &[u8]) -> Transition {
    let mut last = None;
    for value in keys {
        last = Some(
            runtime
                .dispatch(Action::Character {
                    value: *value,
                    shift: false,
                })
                .unwrap(),
        );
    }
    last.unwrap()
}

fn fact(seat: u32, source: u8, how: SelectionHow, outcome: SelectionOutcome) -> SelectionFact {
    SelectionFact {
        seat,
        source,
        how,
        outcome,
    }
}

fn selection<E: InputEngine>(runtime: &Runtime<E>) -> Option<SelectionFact> {
    runtime.facts().selection
}

#[test]
fn finish_records_the_highlighted_seat_and_its_source() {
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let done = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(done.commit.as_deref(), Some("拟好"));
    assert_eq!(
        selection(&runtime),
        Some(fact(
            1,
            5,
            SelectionHow::Finish,
            SelectionOutcome::Committed
        ))
    );
    assert_eq!(runtime.facts().tail_chars, 0);
}

#[test]
fn punctuation_finishes_on_both_routes_with_the_mark_as_tail() {
    // `punctuation` 路由：引擎给出中文标点。
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let done = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(done.commit.as_deref(), Some("你好，"));
    let facts = runtime.facts();
    assert_eq!(
        facts.selection,
        Some(fact(
            0,
            0,
            SelectionHow::PunctuationFinish,
            SelectionOutcome::Committed
        ))
    );
    assert_eq!(facts.tail_chars, 1);
    assert_eq!(facts.split_commit("你好，"), ("你好", "，"));

    // `punctuation` 路由：没有中文形式的标点原样接在后面。
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let done = runtime.dispatch(Action::Punctuation(b'.')).unwrap();
    assert_eq!(done.commit.as_deref(), Some("你好."));
    assert_eq!(
        selection(&runtime).map(|fact| fact.how),
        Some(SelectionHow::PunctuationFinish)
    );
    assert_eq!(runtime.facts().tail_chars, 1);

    // 字符键被引擎交还后转到 `punctuation`，同样是标点结束组字。
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let done = type_keys(&mut runtime, b",");
    assert_eq!(done.commit.as_deref(), Some("你好，"));
    assert_eq!(
        selection(&runtime).map(|fact| fact.how),
        Some(SelectionHow::PunctuationFinish)
    );
    assert_eq!(runtime.facts().tail_chars, 1);

    // `punctuation_ascii` 路由：字面标点。
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let done = runtime.dispatch(Action::PunctuationAscii(b'.')).unwrap();
    assert_eq!(done.commit.as_deref(), Some("拟好."));
    let facts = runtime.facts();
    assert_eq!(
        facts.selection,
        Some(fact(
            1,
            5,
            SelectionHow::PunctuationFinish,
            SelectionOutcome::Committed
        ))
    );
    assert_eq!(facts.tail_chars, 1);
    assert!(!facts.raw);
}

#[test]
fn a_letter_after_a_complete_wubi_code_is_a_top_commit() {
    let mut runtime = focused(FactEngine {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        ..FactEngine::default()
    });
    type_keys(&mut runtime, b"wqab");
    assert_eq!(selection(&runtime), None);
    let topped = type_keys(&mut runtime, b"x");
    assert_eq!(topped.commit.as_deref(), Some("合成甲"));
    assert_eq!(topped.view.editing_text, "x");
    // 下一个组字是新的一段，不算本次选择只用掉了一部分读音。
    assert_eq!(
        selection(&runtime),
        Some(fact(
            0,
            0,
            SelectionHow::WubiTop,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn the_wubi_top_commit_decision_is_the_reference_truth_table() {
    let request = |top_commit_enabled,
                   mixed_pinyin,
                   four_code_is_complete,
                   phrase_prefix_empty,
                   pinyin_continues| {
        WubiTopCommitRequest {
            top_commit_enabled,
            mixed_pinyin,
            four_code_is_complete,
            phrase_prefix_empty,
            pinyin_continues,
        }
    };
    // 开关开：四码完整、没有暂存词段、拼音不再续输时顶字，混输开不开都一样。
    for mixed in [false, true] {
        assert!(wubi_top_commit_on_next_key(&request(
            true, mixed, true, true, false
        )));
    }
    // 开关关 + 混输开：字母并进混输串，不顶字。
    assert!(!wubi_top_commit_on_next_key(&request(
        false, true, true, true, false
    )));
    // 开关关 + 混输关：字母无处容纳，仍按顶字上屏，一个键都不丢。
    assert!(wubi_top_commit_on_next_key(&request(
        false, false, true, true, false
    )));
    // 其余每一项都是必要条件：开关关+混输关这条兜底也守不住它们。
    for request in [
        request(false, false, false, true, false),
        request(false, false, true, false, false),
        request(true, false, true, true, true),
        request(true, true, true, true, true),
        request(false, true, false, true, false),
    ] {
        assert!(!wubi_top_commit_on_next_key(&request));
    }
}

#[test]
fn the_wubi_auto_commit_decision_needs_all_five_facts() {
    let allowed = WubiAutoCommitRequest {
        auto_commit_enabled: true,
        character_action: true,
        snapshot_valid: true,
        unique_four_code: true,
        phrase_prefix_empty: true,
    };
    assert!(wubi_auto_commit_on_fourth_key(&allowed));
    for blocked in [
        WubiAutoCommitRequest {
            auto_commit_enabled: false,
            ..allowed
        },
        WubiAutoCommitRequest {
            character_action: false,
            ..allowed
        },
        WubiAutoCommitRequest {
            snapshot_valid: false,
            ..allowed
        },
        WubiAutoCommitRequest {
            unique_four_code: false,
            ..allowed
        },
        WubiAutoCommitRequest {
            phrase_prefix_empty: false,
            ..allowed
        },
    ] {
        assert!(!wubi_auto_commit_on_fourth_key(&blocked));
    }
}

#[test]
fn turning_off_the_wubi_top_commit_lets_a_mixed_spelling_grow() {
    // 混输打开、顶字关掉：第五个字母并进混输串，不上屏，也没有顶字那次选择。
    let mut mixed = focused(FactEngine {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        mixed_pinyin: true,
        ..FactEngine::default()
    });
    mixed.set_wubi_fifth_code_top_commit(false);
    type_keys(&mut mixed, b"wqab");
    assert_eq!(selection(&mixed), None);
    let grown = type_keys(&mut mixed, b"x");
    assert!(grown.commit.is_none());
    assert_eq!(grown.view.editing_text, "wqabx");
    assert_eq!(selection(&mixed), None);
}

#[test]
fn turning_off_the_wubi_top_commit_still_tops_with_mixed_input_off() {
    // 混输关掉时第五个字母无处容纳：引擎会把码裁回四位，宿主必须顶字，否则这个键就丢了。
    let mut plain = focused(FactEngine {
        scheme: 2,
        words: vec!["合成甲".into(), "合成乙".into()],
        ..FactEngine::default()
    });
    plain.set_wubi_fifth_code_top_commit(false);
    type_keys(&mut plain, b"wqab");
    let topped = type_keys(&mut plain, b"x");
    assert_eq!(topped.commit.as_deref(), Some("合成甲"));
    assert_eq!(topped.view.editing_text, "x");
    assert_eq!(
        selection(&plain),
        Some(fact(
            0,
            0,
            SelectionHow::WubiTop,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn a_unique_four_letter_wubi_code_commits_itself() {
    let mut runtime = focused(FactEngine {
        scheme: 2,
        words: vec!["合成候选".into()],
        ..FactEngine::default()
    });
    type_keys(&mut runtime, b"wqa");
    assert_eq!(selection(&runtime), None);
    let done = type_keys(&mut runtime, b"a");
    assert_eq!(done.commit.as_deref(), Some("合成候选"));
    assert_eq!(
        selection(&runtime),
        Some(fact(
            0,
            0,
            SelectionHow::WubiAuto,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn space_over_an_open_list_picks_the_highlighted_row() {
    let mut runtime = focused(FactEngine {
        candidate_list_open: true,
        ..pinyin()
    });
    type_keys(&mut runtime, b"ni");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let done = type_keys(&mut runtime, b" ");
    assert_eq!(done.commit.as_deref(), Some("拟好"));
    assert_eq!(
        selection(&runtime),
        Some(fact(
            1,
            5,
            SelectionHow::SpaceOverList,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn a_digit_records_the_seat_on_the_visible_page() {
    let mut runtime = Runtime::new(
        FactEngine {
            words: (0..8).map(|n| format!("候选{n}")).collect(),
            sources: (0..8).collect(),
            ..FactEngine::default()
        },
        3,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_keys(&mut runtime, b"ni");
    let paged = runtime.dispatch(Action::NextPage).unwrap();
    // 来源里有云和 AI，在线座位重排了列表：座位记的是用户看到的位置，不是引擎下标。
    let shown = &paged.view.candidates[1];
    assert_eq!(shown.id.index, 4);
    assert_eq!(shown.text, "候选6");
    let done = type_keys(&mut runtime, b"2");
    assert_eq!(done.commit.as_deref(), Some("候选6"));
    assert_eq!(
        selection(&runtime),
        Some(fact(
            4,
            shown.source,
            SelectionHow::Digit,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn fourteen_key_commit_raw_finishes_from_the_highlight() {
    for command in [Command::CommitRaw, Command::CommitRawWithoutLearning] {
        let mut runtime = focused(FactEngine {
            key_grid: Some(KeyGrid::FourteenKey),
            ..pinyin()
        });
        runtime.dispatch(Action::GridKey(b'n')).unwrap();
        runtime.dispatch(Action::GridKey(b'h')).unwrap();
        runtime.dispatch(Action::NextCandidate).unwrap();
        let done = runtime.dispatch(Action::Command(command)).unwrap();
        assert_eq!(done.commit.as_deref(), Some("拟好"));
        let facts = runtime.facts();
        assert_eq!(
            facts.selection,
            Some(fact(
                1,
                5,
                SelectionHow::GridRaw,
                SelectionOutcome::Committed
            ))
        );
        // 14 键的组码不是可上屏的字母，这一路是选词，不是原样上屏。
        assert!(!facts.raw);
    }
}

#[test]
fn select_and_any_candidate_record_their_own_how() {
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let id = runtime.view().candidates[2].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(
        selection(&runtime),
        Some(fact(
            2,
            8,
            SelectionHow::Select,
            SelectionOutcome::Committed
        ))
    );

    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let id = runtime.all_candidates().candidates[1].id;
    runtime.dispatch(Action::SelectAnyCandidate(id)).unwrap();
    assert_eq!(
        selection(&runtime),
        Some(fact(
            1,
            5,
            SelectionHow::AnyCandidate,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn an_edge_pick_matches_on_the_first_or_last_character() {
    for (edge, expected) in [
        (CandidateEdge::FirstHan, "你"),
        (CandidateEdge::LastHan, "好"),
    ] {
        let mut runtime = focused(pinyin());
        type_keys(&mut runtime, b"ni");
        let id = runtime.view().candidates[0].id;
        let done = runtime.dispatch(Action::SelectEdge(id, edge)).unwrap();
        assert_eq!(done.commit.as_deref(), Some(expected));
        assert_eq!(
            selection(&runtime),
            Some(fact(0, 0, SelectionHow::Edge, SelectionOutcome::Committed)),
            "{expected}"
        );
    }
}

#[test]
fn select_highlighted_with_rows_is_a_highlighted_pick() {
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    runtime.dispatch(Action::LastCandidate).unwrap();
    let done = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(done.commit.as_deref(), Some("泥"));
    assert_eq!(
        selection(&runtime),
        Some(fact(
            2,
            8,
            SelectionHow::Highlighted,
            SelectionOutcome::Committed
        ))
    );
}

#[test]
fn a_held_piece_is_held_and_a_plain_key_only_grows_the_hold() {
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"nihao");
    let id = runtime.view().candidates[2].id;
    let held = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(held.commit, None);
    assert_eq!(held.view.phrase_prefix, "泥");
    let facts = runtime.facts();
    assert_eq!(
        facts.selection,
        Some(fact(2, 8, SelectionHow::Select, SelectionOutcome::Held))
    );
    assert!(facts.held_grew);
    assert!(!facts.held_flushed);
    assert_eq!(facts.tail_chars, 0);

    // 普通字符键也会进暂存分支：只记 held_grew，不产生"已选择"。
    let mut runtime = focused(FactEngine {
        character_commits: Some("甲".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"n");
    assert_eq!(runtime.view().phrase_prefix, "甲");
    let facts = runtime.facts();
    assert_eq!(facts.selection, None);
    assert!(facts.held_grew);
}

#[test]
fn the_held_phrase_flushes_with_the_last_pick_and_on_blur() {
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"nihao");
    let id = runtime.view().candidates[2].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let id = runtime.view().candidates[0].id;
    let done = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(done.commit.as_deref(), Some("泥你好"));
    let facts = runtime.facts();
    assert_eq!(
        facts.selection,
        Some(fact(
            0,
            0,
            SelectionHow::Select,
            SelectionOutcome::Committed
        ))
    );
    assert!(facts.held_flushed);
    assert!(!facts.held_grew);

    // 失焦把暂存段送出去，不算选择；facts 在 focus 入口重置。
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"nihao");
    let id = runtime.view().candidates[2].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let blurred = runtime.focus(false).unwrap();
    assert_eq!(blurred.commit.as_deref(), Some("泥"));
    let facts = runtime.facts();
    assert_eq!(facts.selection, None);
    assert!(facts.held_flushed);

    // Esc 丢掉暂存段，不算送出。
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"nihao");
    let id = runtime.view().candidates[2].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(!runtime.facts().held_flushed);
}

#[test]
fn a_pick_inside_sentence_conversion_pins_without_committing() {
    for action in ["select", "highlighted", "digit"] {
        let mut runtime = focused(pinyin());
        type_keys(&mut runtime, b"ni");
        // 进入改字要先有组字；聚焦时的取消会清掉它，所以在打字之后再设。
        runtime.engine.conversion = "你好世界".into();
        runtime.refresh().unwrap();
        runtime.dispatch(Action::NextCandidate).unwrap();
        let transition = match action {
            "select" => {
                let id = runtime.view().candidates[1].id;
                runtime.dispatch(Action::Select(id)).unwrap()
            }
            "highlighted" => runtime.dispatch(Action::SelectHighlighted).unwrap(),
            _ => type_keys(&mut runtime, b"2"),
        };
        assert_eq!(transition.commit, None, "{action}");
        assert_eq!(transition.view.conversion, "你好世界*", "{action}");
        let how = match action {
            "select" => SelectionHow::Select,
            "highlighted" => SelectionHow::Highlighted,
            _ => SelectionHow::Digit,
        };
        assert_eq!(
            selection(&runtime),
            Some(fact(1, 5, how, SelectionOutcome::Pinned)),
            "{action}"
        );
    }
}

#[test]
fn a_pick_from_the_zhuyin_list_pins_the_span() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());
    compose_zhuyin(&mut runtime, "su3cl3");
    let opened = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(opened.view.candidate_list_open);
    // 打开列表的空格是注音的拼写符号，不是选择。
    assert_eq!(selection(&runtime), None);
    let source = opened.view.candidates[2].source;
    let picked = type_keys(&mut runtime, b"3");
    assert!(picked.handled && picked.commit.is_none());
    assert_eq!(picked.view.preedit, "你郝");
    assert_eq!(
        selection(&runtime),
        Some(fact(
            2,
            source,
            SelectionHow::Digit,
            SelectionOutcome::Pinned
        ))
    );
}

#[test]
fn a_pick_that_leaves_reading_composing_is_partial() {
    // 粤拼：覆盖开头几个音节的候选立即上屏，其余继续组字；方案从不暂存词段。
    let mut runtime = focused(FactEngine {
        scheme: 5,
        remaining_after_select: Some("hou".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"neihou");
    let id = runtime.view().candidates[0].id;
    let done = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(done.commit.as_deref(), Some("你好"));
    assert_eq!(done.view.editing_text, "hou");
    assert_eq!(
        selection(&runtime),
        Some(fact(0, 0, SelectionHow::Select, SelectionOutcome::Partial))
    );
    assert!(!runtime.facts().held_grew);

    // 宿主没打开词段暂存时，全拼的部分选词同样直接上屏。
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    type_keys(&mut runtime, b"nihao");
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(
        selection(&runtime),
        Some(fact(
            0,
            0,
            SelectionHow::Highlighted,
            SelectionOutcome::Partial
        ))
    );
}

#[test]
fn a_commit_that_does_not_carry_the_candidate_is_not_a_selection() {
    let mut runtime = focused(FactEngine {
        select_commit_override: Some("别的".into()),
        ..pinyin()
    });
    type_keys(&mut runtime, b"ni");
    let done = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(done.commit.as_deref(), Some("别的"));
    assert_eq!(selection(&runtime), None);

    // 没有候选可对时（英文、读音本身）也不算。
    let mut runtime = focused(FactEngine::default());
    type_keys(&mut runtime, b"abc");
    let done = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(done.commit.as_deref(), Some("abc"));
    assert_eq!(selection(&runtime), None);
}

#[test]
fn a_bare_mode_prefix_ends_as_punctuation_or_raw_text_without_a_selection() {
    // 高亮的第一行是上屏文本的开头，所以这几条路由一旦误记了选择，`selection_fact` 会核对通过、把它报出来：这里的 `None` 才说明路由本身没有记。
    let bare = |first_row: &str| {
        let mut runtime = focused(FactEngine {
            spelling_symbols: "/".into(),
            words: vec![first_row.into(), "泥".into()],
            ..pinyin()
        });
        type_keys(&mut runtime, b"/");
        runtime.engine.local_mode = "command".into();
        runtime.engine.spelling_symbols.clear();
        runtime.refresh().unwrap();
        assert_eq!(runtime.view().local_mode, "command");
        assert_eq!(runtime.view().editing_text, "/");
        runtime
    };

    // `punctuation` 路由：bare 的 `/` 直接按标点处理。
    let mut runtime = bare("，");
    let done = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(done.commit.as_deref(), Some("，"));
    assert_eq!(selection(&runtime), None);
    assert!(!runtime.facts().raw);

    // `punctuation_ascii` 路由：原样上屏前缀，记为 raw。
    let mut runtime = bare("/");
    let done = runtime.dispatch(Action::PunctuationAscii(b',')).unwrap();
    assert_eq!(done.commit.as_deref(), Some("/,"));
    let facts = runtime.facts();
    assert_eq!(facts.selection, None);
    assert!(facts.raw);
    assert_eq!(facts.tail_chars, 1);
    assert_eq!(facts.split_commit("/,"), ("/", ","));

    // Finish 在 bare 前缀上同样原样上屏。
    let mut runtime = bare("/");
    let done = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(done.commit.as_deref(), Some("/"));
    assert_eq!(selection(&runtime), None);
    assert!(runtime.facts().raw);
}

#[test]
fn enter_commits_the_raw_reading() {
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    let done = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(done.commit.as_deref(), Some("ni"));
    let facts = runtime.facts();
    assert_eq!(facts.selection, None);
    assert!(facts.raw);
}

#[test]
fn a_space_the_scheme_spells_with_is_not_a_pick() {
    // 列表开着：这时误记的高亮选择会被核对成"钉住"报出来，所以 `None` 才说明这条路由没有记。
    let mut runtime = focused(FactEngine {
        scheme: 6,
        spelling_symbols: " ".into(),
        candidate_list_open: true,
        ..pinyin()
    });
    type_keys(&mut runtime, b"su");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(space.commit.is_none());
    assert_eq!(space.view.editing_text, "su ");
    assert_eq!(selection(&runtime), None);

    // 真实的注音引擎：`su3cl3` 之后空格是一声。
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = zhuyin_runtime(directory.path());
    let typed = compose_zhuyin(&mut runtime, "su3cl3");
    assert!(typed.view.spelling_symbols.contains(' '));
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(selection(&runtime), None);
}

#[test]
fn space_with_no_rows_commits_the_scheme_text_without_a_pick() {
    let mut runtime = focused(FactEngine {
        scheme: 9,
        ..FactEngine::default()
    });
    type_keys(&mut runtime, b"zzz");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("zzz"));
    assert_eq!(selection(&runtime), None);
    assert!(!runtime.facts().raw);

    // 真实的笔画引擎：没有候选时空格上屏笔画字母；有候选时空格是选高亮行。
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = stroke_runtime(directory.path());
    compose_stroke(&mut runtime, "zzz");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("zzz"));
    assert_eq!(selection(&runtime), None);
    compose_stroke(&mut runtime, "hh");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("三"));
    assert_eq!(
        selection(&runtime).map(|fact| (fact.seat, fact.how, fact.outcome)),
        Some((1, SelectionHow::Highlighted, SelectionOutcome::Committed))
    );
}

#[test]
fn korean_hanja_confirmation_is_a_highlighted_pick_and_a_bare_syllable_is_not() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = korean_runtime(directory.path());
    open_korean_hanja(&mut runtime, "gks");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("漢"));
    assert_eq!(
        selection(&runtime).map(|fact| (fact.seat, fact.how, fact.outcome)),
        Some((1, SelectionHow::Highlighted, SelectionOutcome::Committed))
    );

    // 没开汉字列表时空格上屏音节本身，不是选择。
    type_keys(&mut runtime, b"gks");
    let space = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(space.commit.as_deref(), Some("한"));
    assert_eq!(selection(&runtime), None);
}

#[test]
fn taking_a_selection_back_records_restore_or_drop() {
    let mut runtime = phrase_runtime("haitanp", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(runtime.facts().retracted, None);
    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    let facts = runtime.facts();
    assert_eq!(facts.retracted, Some(SelectionRetraction::Restore));
    assert_eq!(facts.selection, None);

    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    runtime
        .dispatch(Action::Command(Command::MoveHome))
        .unwrap();
    assert_eq!(runtime.facts().retracted, None);
    runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(runtime.facts().retracted, Some(SelectionRetraction::Drop));

    // 下一键开始时重置。
    type_keys(&mut runtime, b"x");
    assert_eq!(runtime.facts(), DispatchFacts::default());
}

#[test]
fn facts_are_reset_at_every_dispatch_and_focus() {
    let mut runtime = focused(pinyin());
    type_keys(&mut runtime, b"ni");
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(selection(&runtime).is_some());
    type_keys(&mut runtime, b"n");
    assert_eq!(runtime.facts(), DispatchFacts::default());

    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(selection(&runtime).is_some());
    runtime.focus(false).unwrap();
    assert_eq!(runtime.facts(), DispatchFacts::default());

    // 失焦时的派发直接返回，也不能留下上一键的事实。
    runtime.focus(true).unwrap();
    type_keys(&mut runtime, b"n");
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    runtime.focused = false;
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(runtime.facts(), DispatchFacts::default());
}

#[test]
fn the_split_counts_characters_so_a_fullwidth_commit_splits_in_the_same_place() {
    let mut runtime = focused(FactEngine {
        words: vec!["abc".into()],
        ..FactEngine::default()
    });
    type_keys(&mut runtime, b"abc");
    let done = runtime.dispatch(Action::PunctuationAscii(b'.')).unwrap();
    let commit = done.commit.unwrap();
    assert_eq!(commit, "abc.");
    let facts = runtime.facts();
    assert_eq!(facts.tail_chars, 1);
    assert_eq!(facts.split_commit(&commit), ("abc", "."));

    // 全角转换逐字符映射，UTF-8 长度从 1 字节变成 3 字节；按字符数切仍落在同一处。
    let wide = character_width::to_fullwidth(&commit);
    assert_eq!(wide, "ａｂｃ．");
    assert_eq!(facts.split_commit(&wide), ("ａｂｃ", "．"));

    // 暂存段在前面拼上，tail 从末尾数，所以仍然只是标点。
    let mut runtime = focused(FactEngine {
        remaining_after_select: Some("hao".into()),
        ..pinyin()
    });
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"nihao");
    let id = runtime.view().candidates[2].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    let done = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    let commit = done.commit.unwrap();
    assert_eq!(commit, "泥你好，");
    let facts = runtime.facts();
    assert!(facts.held_flushed);
    assert_eq!(facts.split_commit(&commit), ("泥你好", "，"));

    // tail 比整段还长时整段都是 tail；没有 tail 时整段都是 body。
    let all_tail = DispatchFacts {
        tail_chars: 9,
        ..DispatchFacts::default()
    };
    assert_eq!(all_tail.split_commit("，"), ("", "，"));
    assert_eq!(DispatchFacts::default().split_commit("你好"), ("你好", ""));
}

#[test]
fn has_composition_ignores_snapshot_validity_and_counts_a_held_phrase() {
    let mut runtime = focused(pinyin());
    assert!(!runtime.has_composition());
    runtime.snapshot_valid = false;
    assert!(!runtime.is_idle());
    assert!(!runtime.has_composition());
    runtime.snapshot_valid = true;

    type_keys(&mut runtime, b"n");
    assert!(runtime.has_composition());

    // 读音删空、只剩暂存段时仍在组字。
    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert!(runtime.view().editing_text.is_empty());
    assert!(runtime.has_composition());
}

#[test]
fn epochs_move_with_what_the_user_sees() {
    let mut runtime = Runtime::new(
        FactEngine {
            words: (0..4).map(|n| format!("候选{n}")).collect(),
            withheld: vec!["候选4".into(), "候选5".into()],
            ..FactEngine::default()
        },
        2,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    let epochs =
        |runtime: &Runtime<FactEngine>| (runtime.composition_epoch(), runtime.candidates_epoch());

    let (composition, candidates) = epochs(&runtime);
    type_keys(&mut runtime, b"n");
    assert!(runtime.composition_epoch() > composition);
    assert!(runtime.candidates_epoch() > candidates);

    // 引擎不收的键什么都没变。
    let before = epochs(&runtime);
    let ignored = type_keys(&mut runtime, b"0");
    assert!(!ignored.handled);
    assert_eq!(epochs(&runtime), before);

    // 高亮移动和翻页在 refresh 之前就返回，只动候选的 epoch。
    let before = epochs(&runtime);
    runtime.dispatch(Action::NextCandidate).unwrap();
    assert_eq!(runtime.composition_epoch(), before.0);
    assert!(runtime.candidates_epoch() > before.1);

    let before = epochs(&runtime);
    runtime.dispatch(Action::PreviousPage).unwrap();
    assert_eq!(runtime.composition_epoch(), before.0);
    assert!(runtime.candidates_epoch() > before.1);

    // 已经在第一页第一行：再往前翻、再往上移都不算变化。
    let before = epochs(&runtime);
    runtime.dispatch(Action::PreviousPage).unwrap();
    runtime.dispatch(Action::PreviousCandidate).unwrap();
    assert_eq!(epochs(&runtime), before);

    let before = epochs(&runtime);
    let paged = runtime.dispatch(Action::NextPage).unwrap();
    assert_eq!(paged.view.page, 1);
    assert_eq!(runtime.composition_epoch(), before.0);
    assert!(runtime.candidates_epoch() > before.1);

    // 翻到最后一页时放出引擎扣着的候选，列表变长，视图跟着变。
    let before = epochs(&runtime);
    let last = runtime.dispatch(Action::NextPage).unwrap();
    assert_eq!(last.view.page_count, 3);
    assert!(runtime.candidates_epoch() > before.1);

    // 释义到达只改候选行。
    let before = epochs(&runtime);
    let generation = runtime.generation();
    assert!(runtime.apply_translations(generation, [("候选4".to_owned(), "gloss".to_owned())]));
    assert_eq!(runtime.composition_epoch(), before.0);
    assert!(runtime.candidates_epoch() > before.1);

    // 同值回包和不属于当前候选的回包都没有改变可见候选。
    let before = epochs(&runtime);
    assert!(runtime.apply_translations(generation, [("候选4".to_owned(), "gloss".to_owned())]));
    assert!(runtime.apply_translations(generation, [("无关候选".to_owned(), "unused".to_owned())]));
    assert_eq!(epochs(&runtime), before);

    // 释义配置换代会撤销旧回包；只在确实移除释义时发送候选视图事件。
    let before = epochs(&runtime);
    runtime.invalidate_translations().unwrap();
    assert_eq!(runtime.composition_epoch(), before.0);
    assert!(runtime.candidates_epoch() > before.1);
    assert!(!runtime.apply_translations(generation, [("候选4".to_owned(), "late".to_owned())]));
    let before = epochs(&runtime);
    runtime.invalidate_translations().unwrap();
    assert_eq!(epochs(&runtime), before);

    // 上屏结束组字，两个都变。
    let before = epochs(&runtime);
    runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert!(runtime.composition_epoch() > before.0);
    assert!(runtime.candidates_epoch() > before.1);
}

#[test]
fn the_held_phrase_alone_moves_the_composition_epoch() {
    let mut runtime = phrase_runtime("haitanpaobu", vec![6]);
    let id = runtime.view().candidates[0].id;
    runtime.dispatch(Action::Select(id)).unwrap();
    runtime
        .dispatch(Action::Command(Command::MoveHome))
        .unwrap();
    // 光标在读音开头时 Ctrl+Backspace 只删掉暂存段，读音和光标都不动。
    let before = (runtime.composition_epoch(), runtime.candidates_epoch());
    let dropped = runtime.dispatch(Action::SegmentBackspace).unwrap();
    assert_eq!(dropped.view.editing_text, "paobu");
    assert!(dropped.view.phrase_prefix.is_empty());
    assert!(runtime.composition_epoch() > before.0);
    assert_eq!(runtime.candidates_epoch(), before.1);

    // 只移动光标也是组字的变化。
    let before = runtime.composition_epoch();
    runtime.dispatch(Action::Command(Command::MoveEnd)).unwrap();
    assert!(runtime.composition_epoch() > before);
}

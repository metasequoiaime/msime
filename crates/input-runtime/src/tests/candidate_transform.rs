//! 脚本插件的候选变换在 runtime 里的一侧：重排和隐藏之后选第 k 行上屏的就是显示的第 k 行，每条选词路径都选不到被隐藏的行，引擎下标 0 不可隐藏，锁定的座位不动，多个插件按授权顺序合成，旁路调用只重放、不运行变换。
//!
//! 这里的"插件"是一个记录调用、按预设交回操作的 [`CandidateTransform`]；真正运行脚本的实现和它的本地模式白名单在 host-api 里测。

use super::super::runtime::empty_result;
use super::super::*;
use super::{allocations, favouring_model, LATTICE_SOURCE};
use std::cell::RefCell;
use std::rc::Rc;

/// 记下每次交给引擎的候选下标的测试引擎。
#[derive(Default)]
struct TransformEngine {
    scheme: u8,
    text: String,
    words: Vec<String>,
    /// 与 `words` 平行；长度不符时全部按 0。
    sources: Vec<u8>,
    /// 与 `words` 平行的固定位置；长度不符时全部按 0。
    positions: Vec<u8>,
    /// 非空表示正在整句改字。
    conversion: String,
    candidate_list_open: bool,
    /// 打满四码时报告五笔唯一码。
    wubi_unique: bool,
    /// `expand_initial_candidates` 接到末尾的候选。
    withheld: Vec<String>,
    /// 让编码数组比候选少一项，制造平行数组长度不一致。
    short_codes: bool,
    /// 模拟读取候选快照失败。
    snapshot_fails: bool,
    /// `select`、`select_edge`、`finish` 收到的引擎下标。
    picks: Vec<usize>,
}

impl TransformEngine {
    fn new(words: &[&str]) -> Self {
        Self {
            words: words.iter().map(|word| (*word).to_owned()).collect(),
            ..Self::default()
        }
    }

    fn commit(text: impl Into<String>) -> EngineResult {
        EngineResult {
            handled: true,
            has_commit: true,
            commit: text.into(),
            diagnostic: String::new(),
        }
    }

    fn parallel<T: Clone + Default>(&self, values: &[T]) -> Vec<T> {
        if values.len() == self.words.len() {
            values.to_vec()
        } else {
            vec![T::default(); self.words.len()]
        }
    }
}

impl InputEngine for TransformEngine {
    fn snapshot(&self) -> Result<EngineSnapshot, RuntimeError> {
        if self.snapshot_fails {
            return Err(RuntimeError::Engine("synthetic snapshot failure".into()));
        }
        let composing = !self.text.is_empty() || !self.conversion.is_empty();
        let count = if composing { self.words.len() } else { 0 };
        let codes = count - usize::from(self.short_codes && count > 0);
        Ok(EngineSnapshot {
            prediction: false,
            scheme: self.scheme,
            candidate_codes: vec!["k".into(); codes],
            candidate_annotations: (0..count).map(|index| format!("e{index}")).collect(),
            candidate_sources: self.parallel(&self.sources)[..count].to_vec(),
            candidate_positions: self.parallel(&self.positions)[..count].to_vec(),
            candidate_corrected: vec![false; count],
            candidate_answers_key: vec![true; count],
            candidate_list_open: self.candidate_list_open,
            shuangpin_profile: "xiaohe".into(),
            answered_by_pinyin_fallback: false,
            wubi_unique_four_code: self.wubi_unique && self.text.len() == 4,
            local_mode: "none".into(),
            preedit: self.text.clone(),
            editing_text: self.text.clone(),
            caret_position: self.text.len(),
            conversion: self.conversion.clone(),
            conversion_focus_start: 0,
            conversion_focus_end: usize::from(!self.conversion.is_empty()),
            candidates: self.words[..count].to_vec(),
            ..EngineSnapshot::default()
        })
    }
    fn character(&mut self, value: u8, _shift: bool) -> Result<EngineResult, RuntimeError> {
        if value.is_ascii_alphabetic() {
            self.text.push(char::from(value));
            return Ok(empty_result(true));
        }
        Ok(empty_result(false))
    }
    fn command(&mut self, command: Command) -> Result<EngineResult, RuntimeError> {
        let composing = !self.text.is_empty();
        match command {
            Command::CommitRaw if composing => Ok(Self::commit(std::mem::take(&mut self.text))),
            _ => {
                self.text.clear();
                self.conversion.clear();
                Ok(empty_result(composing))
            }
        }
    }
    fn select(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        self.picks.push(index);
        if !self.conversion.is_empty() {
            return Ok(empty_result(true));
        }
        self.text.clear();
        Ok(Self::commit(self.words[index].clone()))
    }
    fn select_edge(
        &mut self,
        index: usize,
        _edge: CandidateEdge,
    ) -> Result<EngineResult, RuntimeError> {
        self.picks.push(index);
        self.text.clear();
        let first = self.words[index].chars().next().unwrap_or_default();
        Ok(Self::commit(first))
    }
    fn finish(&mut self, index: usize) -> Result<EngineResult, RuntimeError> {
        if self.text.is_empty() {
            return Ok(empty_result(false));
        }
        self.picks.push(index);
        let commit = self
            .words
            .get(index)
            .cloned()
            .unwrap_or_else(|| self.text.clone());
        self.text.clear();
        Ok(Self::commit(commit))
    }
    fn punctuation(&mut self, _value: u8) -> Result<EngineResult, RuntimeError> {
        Ok(empty_result(false))
    }
    fn reset_cache(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn expand_initial_candidates(&mut self) -> Result<bool, RuntimeError> {
        if self.withheld.is_empty() {
            return Ok(false);
        }
        self.words.append(&mut self.withheld);
        Ok(true)
    }
}

/// 插件被调用的记录。
#[derive(Default)]
struct Calls {
    /// 每次调用时窗口里的候选文本。
    windows: Vec<Vec<String>>,
    /// 每次调用时窗口里的注释。
    annotations: Vec<Vec<String>>,
}

/// 按预设交回操作的"插件"：`ops` 里每一项代表一个插件，按授权顺序。
struct Plugins {
    ops: Rc<RefCell<Vec<CandidateOps>>>,
    calls: Rc<RefCell<Calls>>,
}

impl CandidateTransform for Plugins {
    fn transform(
        &mut self,
        window: &[TransformRow<'_>],
        _local_mode: &str,
        out: &mut Vec<CandidateOps>,
    ) {
        let mut calls = self.calls.borrow_mut();
        calls
            .windows
            .push(window.iter().map(|row| row.text.to_owned()).collect());
        calls
            .annotations
            .push(window.iter().map(|row| row.annotation.to_owned()).collect());
        out.extend(self.ops.borrow().iter().cloned());
    }
}

fn ops(order: &[u32], hidden: &[u32], annotations: &[(u32, &str)]) -> CandidateOps {
    CandidateOps {
        order: order.to_vec(),
        hidden: hidden.to_vec(),
        annotations: annotations
            .iter()
            .map(|(row, text)| (*row, (*text).to_owned()))
            .collect(),
    }
}

struct Harness {
    runtime: Runtime<TransformEngine>,
    ops: Rc<RefCell<Vec<CandidateOps>>>,
    calls: Rc<RefCell<Calls>>,
}

impl Harness {
    fn new(engine: TransformEngine, plugins: Vec<CandidateOps>) -> Self {
        let ops = Rc::new(RefCell::new(plugins));
        let calls = Rc::new(RefCell::new(Calls::default()));
        let mut runtime = Runtime::new(engine, 5).unwrap();
        runtime.set_candidate_transform(Some(Box::new(Plugins {
            ops: ops.clone(),
            calls: calls.clone(),
        })));
        runtime.focus(true).unwrap();
        Self {
            runtime,
            ops,
            calls,
        }
    }

    fn calls(&self) -> usize {
        self.calls.borrow().windows.len()
    }

    fn key(&mut self, value: u8) -> Transition {
        self.runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap()
    }

    fn shown(&self) -> Vec<String> {
        shown(&self.runtime.view())
    }
}

fn shown(view: &View) -> Vec<String> {
    view.candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

const FIVE: [&str; 5] = ["甲", "乙", "丙", "丁", "戊"];

/// 丁、乙排到前面，丙隐藏：显示 丁 乙 甲 戊。
fn reranked_and_filtered() -> Harness {
    let mut harness = Harness::new(TransformEngine::new(&FIVE), vec![ops(&[3, 1], &[2], &[])]);
    harness.key(b'a');
    harness
}

#[test]
fn rerank_and_hide_reorder_every_parallel_array_and_page_only_the_visible_rows() {
    let mut harness = reranked_and_filtered();
    let view = harness.runtime.view();
    assert_eq!(shown(&view), ["丁", "乙", "甲", "戊"]);
    // 引擎的注释跟着各自的行走。
    let annotations: Vec<_> = view
        .candidates
        .iter()
        .map(|candidate| candidate.annotation.as_str())
        .collect();
    assert_eq!(annotations, ["e3", "e1", "e0", "e4"]);
    assert_eq!(view.page_count, 1);
    assert_eq!(harness.runtime.candidate_page_len(), 4);
    assert_eq!(harness.runtime.all_candidates().candidates.len(), 4);
    // 被隐藏的行仍在列表末尾，引擎下标映射覆盖全部候选。
    assert_eq!(harness.runtime.cached.candidates[4], "丙");
    assert_eq!(harness.runtime.engine_order, [3, 1, 0, 4, 2]);
}

#[test]
fn picking_the_kth_row_commits_the_displayed_kth_row() {
    let expected = ["丁", "乙", "甲", "戊"];
    let engine_index = [3, 1, 0, 4];
    for (k, text) in expected.iter().enumerate() {
        let mut harness = reranked_and_filtered();
        let id = harness.runtime.view().candidates[k].id;
        let picked = harness.runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(picked.commit.as_deref(), Some(*text), "select {k}");
        assert_eq!(harness.runtime.engine.picks, [engine_index[k]]);

        let mut harness = reranked_and_filtered();
        let digit = b'1' + k as u8;
        let picked = harness.key(digit);
        assert_eq!(picked.commit.as_deref(), Some(*text), "digit {k}");
        assert_eq!(harness.runtime.engine.picks, [engine_index[k]]);
    }
    // 第五格是被隐藏的丙：数字选不到它，点选和整表点选的 ID 也不认。
    let mut harness = reranked_and_filtered();
    assert!(harness.key(b'5').commit.is_none());
    let mut stale = harness.runtime.view().candidates[0].id;
    stale.index = 4;
    assert!(matches!(
        harness.runtime.dispatch(Action::Select(stale)),
        Err(RuntimeError::StaleCandidate)
    ));
    assert!(matches!(
        harness.runtime.dispatch(Action::SelectAnyCandidate(stale)),
        Err(RuntimeError::StaleCandidate)
    ));
    assert!(harness.runtime.engine.picks.is_empty());
}

/// 只剩引擎下标 0 的甲可见时，每条把候选交给引擎的路径交的都是它。
#[test]
fn no_selection_path_reaches_a_hidden_row() {
    let everything_hidden = || {
        let mut harness = Harness::new(
            TransformEngine::new(&FIVE),
            vec![ops(&[], &[0, 1, 2, 3, 4], &[])],
        );
        harness.key(b'a');
        assert_eq!(harness.shown(), ["甲"]);
        harness
    };
    let paths: Vec<(&str, Vec<Action>)> = vec![
        ("finish", vec![Action::Finish]),
        ("space", vec![Action::SelectHighlighted]),
        ("punctuation", vec![Action::Punctuation(b'.')]),
        ("ascii punctuation", vec![Action::PunctuationAscii(b'.')]),
        (
            "digit",
            vec![Action::Character {
                value: b'1',
                shift: false,
            }],
        ),
        (
            "next candidate then space",
            vec![Action::NextCandidate, Action::SelectHighlighted],
        ),
        (
            "last candidate then space",
            vec![Action::LastCandidate, Action::SelectHighlighted],
        ),
        (
            "next page then finish",
            vec![Action::NextPage, Action::Finish],
        ),
    ];
    for (name, actions) in paths {
        let mut harness = everything_hidden();
        let mut commit = None;
        for action in actions {
            commit = harness.runtime.dispatch(action).unwrap().commit;
        }
        assert!(commit.unwrap().starts_with('甲'), "{name}");
        assert_eq!(harness.runtime.engine.picks, [0], "{name}");
    }
    // 以词定字和整表点选也只能落在可见的甲上。
    let mut harness = everything_hidden();
    let id = harness.runtime.view().candidates[0].id;
    let edge = harness
        .runtime
        .dispatch(Action::SelectEdge(id, CandidateEdge::FirstHan))
        .unwrap();
    assert_eq!(edge.commit.as_deref(), Some("甲"));
    assert_eq!(harness.runtime.engine.picks, [0]);
}

#[test]
fn engine_index_zero_cannot_be_hidden_but_can_move() {
    // 把戊排到最前，再把除戊以外的都隐藏：甲是引擎下标 0，留下来。
    let mut harness = Harness::new(
        TransformEngine::new(&FIVE),
        vec![ops(&[4], &[0, 1, 2, 3], &[])],
    );
    harness.key(b'a');
    assert_eq!(harness.shown(), ["戊", "甲"]);
    let finished = harness.runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(finished.commit.as_deref(), Some("戊"));
    assert_eq!(harness.runtime.engine.picks, [4]);
}

#[test]
fn fixed_positions_and_online_rows_keep_their_seats() {
    // 乙固定在第二位：不移动、不隐藏，别的行绕开它。
    let mut fixed = TransformEngine::new(&["甲", "乙", "丙"]);
    fixed.positions = vec![0, 2, 0];
    let mut harness = Harness::new(fixed, vec![ops(&[2, 1], &[1], &[])]);
    harness.key(b'a');
    assert_eq!(harness.shown(), ["丙", "乙", "甲"]);

    // 云和 AI 行先由内置排序坐好（甲 丙 乙 戊 丁：第一页排不满，AI 紧跟在其余行之后，#6888），插件既挪不动也藏不了它们；藏掉乙之后 AI 仍紧跟在可见行之后。
    let mut online = TransformEngine::new(&FIVE);
    online.sources = vec![0, 0, 2, 3, 0];
    let mut harness = Harness::new(online, vec![ops(&[4, 3, 2, 1], &[1, 2], &[])]);
    harness.key(b'a');
    assert_eq!(
        harness.calls.borrow().windows[0],
        ["甲", "丙", "乙", "戊", "丁"]
    );
    assert_eq!(harness.shown(), ["戊", "丙", "甲", "丁"]);
}

#[test]
fn the_unique_wubi_code_keeps_seat_zero_and_auto_commits_it() {
    let mut wubi = TransformEngine::new(&["工", "式", "戒"]);
    wubi.scheme = 2;
    wubi.wubi_unique = true;
    let mut harness = Harness::new(wubi, vec![ops(&[2], &[0], &[])]);
    for key in b"aaa" {
        harness.key(*key);
    }
    // 三码时没有锁：戒排到最前。
    assert_eq!(harness.shown(), ["戒", "工", "式"]);
    // 第四码是唯一码：0 号座位锁住，自动上屏的是引擎的首位，不是插件想排上来的戒。
    let fourth = harness.key(b'a');
    assert_eq!(fourth.commit.as_deref(), Some("工"));
    assert_eq!(harness.runtime.engine.picks, [0]);
}

#[test]
fn wubi_top_commit_takes_the_displayed_first_row() {
    let mut wubi = TransformEngine::new(&["工", "式", "戒"]);
    wubi.scheme = 2;
    let mut harness = Harness::new(wubi, vec![ops(&[2], &[], &[])]);
    for key in b"aaaa" {
        harness.key(*key);
    }
    assert_eq!(harness.shown(), ["戒", "工", "式"]);
    let topped = harness.key(b'a');
    assert!(topped.commit.unwrap().starts_with('戒'));
    assert_eq!(harness.runtime.engine.picks, [2]);
}

#[test]
fn plugins_compose_in_grant_order_against_the_original_window() {
    let mut harness = Harness::new(
        TransformEngine::new(&FIVE),
        vec![
            ops(&[2], &[], &[(0, "前")]),
            ops(&[1], &[3], &[(0, "后"), (4, "戊注")]),
        ],
    );
    harness.key(b'a');
    // 重排后面的插件优先：乙在丙前；隐藏取并集；同一行的注释后面的覆盖前面的。
    assert_eq!(harness.shown(), ["乙", "丙", "甲", "戊"]);
    let view = harness.runtime.view();
    let annotation = |text: &str| {
        view.candidates
            .iter()
            .find(|candidate| candidate.text == text)
            .and_then(|candidate| candidate.script_annotation.clone())
    };
    assert_eq!(annotation("甲").as_deref(), Some("后"));
    assert_eq!(annotation("戊").as_deref(), Some("戊注"));
    assert_eq!(annotation("乙"), None);
    // 引擎的注释原样留着，插件的注释另放一格。
    assert_eq!(view.candidates[2].annotation, "e0");

    // 下一键：插件看到的仍是内置排序之后、任何插件变换之前的窗口和引擎注释。
    harness.key(b'a');
    let calls = harness.calls.borrow();
    assert_eq!(calls.windows[1], FIVE);
    assert_eq!(calls.annotations[1], ["e0", "e1", "e2", "e3", "e4"]);
}

#[test]
fn duplicate_rows_keep_distinct_hide_and_annotation_targets_after_refresh() {
    let mut harness = Harness::new(
        TransformEngine::new(&["甲", "同", "同", "乙"]),
        vec![ops(&[], &[1], &[(2, "第二行")])],
    );
    harness.key(b'a');
    let view = harness.runtime.view();
    assert_eq!(shown(&view), ["甲", "同", "乙"]);
    assert_eq!(view.candidates[1].annotation, "e2");
    assert_eq!(
        view.candidates[1].script_annotation.as_deref(),
        Some("第二行")
    );
    assert_eq!(harness.runtime.engine_order, [0, 2, 3, 1]);

    // 旁路刷新在重复组前插入另一行，原来第二条“同”仍是被注释的可见行。
    harness.runtime.engine.words.insert(1, "新".into());
    harness.runtime.refresh_online_candidates().unwrap();
    let view = harness.runtime.view();
    assert_eq!(shown(&view), ["甲", "新", "同", "乙"]);
    assert_eq!(view.candidates[2].annotation, "e3");
    assert_eq!(
        view.candidates[2].script_annotation.as_deref(),
        Some("第二行")
    );
    assert_eq!(harness.runtime.engine_order, [0, 1, 3, 4, 2]);
    assert_eq!(harness.calls(), 1);
}

#[test]
fn duplicate_rows_follow_their_own_rerank_and_annotation() {
    let mut harness = Harness::new(
        TransformEngine::new(&["甲", "同", "同", "乙"]),
        vec![ops(&[2, 1], &[], &[(2, "第二行")])],
    );
    harness.key(b'a');
    let view = harness.runtime.view();
    assert_eq!(harness.runtime.engine_order, [2, 1, 0, 3]);
    assert_eq!(view.candidates[0].annotation, "e2");
    assert_eq!(
        view.candidates[0].script_annotation.as_deref(),
        Some("第二行")
    );
    assert_eq!(view.candidates[1].annotation, "e1");
    assert_eq!(view.candidates[1].script_annotation, None);
    harness.runtime.refresh_online_candidates().unwrap();
    let view = harness.runtime.view();
    assert_eq!(harness.runtime.engine_order, [2, 1, 0, 3]);
    assert_eq!(view.candidates[0].annotation, "e2");
    assert_eq!(
        view.candidates[0].script_annotation.as_deref(),
        Some("第二行")
    );
    assert_eq!(view.candidates[1].script_annotation, None);
    assert_eq!(harness.calls(), 1);
}

#[test]
fn skipped_lists_run_no_transform_and_hide_nothing() {
    // 聚焦会取消组字，所以这些状态在聚焦之后才设。
    type Setup = fn(&mut TransformEngine);
    let cases: [(&str, Setup); 3] = [
        ("conversion", |engine| engine.conversion = "甲乙".into()),
        ("open list", |engine| engine.candidate_list_open = true),
        ("arrays out of step", |engine| engine.short_codes = true),
    ];
    for (name, set) in cases {
        let mut harness = Harness::new(
            TransformEngine::new(&FIVE),
            vec![ops(&[4], &[1], &[(0, "注")])],
        );
        set(&mut harness.runtime.engine);
        harness.key(b'a');
        assert_eq!(harness.calls(), 0, "{name}");
        assert_eq!(harness.shown(), FIVE, "{name}");
        assert!(harness
            .runtime
            .view()
            .candidates
            .iter()
            .all(|candidate| candidate.script_annotation.is_none()));
    }
}

#[test]
fn skipped_refresh_discards_previous_transform_operations() {
    let mut harness = reranked_and_filtered();
    assert_eq!(harness.shown(), ["丁", "乙", "甲", "戊"]);

    // 进入整句改字后，旁路刷新不能把上一轮的排序和隐藏带进来。
    harness.runtime.engine.conversion = "甲乙".into();
    harness.runtime.refresh().unwrap();
    assert_eq!(harness.shown(), FIVE);
    assert!(!harness.runtime.has_candidate_ops());

    // 跳过状态结束后也从原始列表开始，直到下一次派发才重新运行插件。
    harness.runtime.engine.conversion.clear();
    harness.runtime.refresh().unwrap();
    assert_eq!(harness.shown(), FIVE);
}

#[test]
fn side_channel_calls_replay_by_identity_without_running_the_transform() {
    let mut engine = TransformEngine::new(&["甲", "乙", "丙"]);
    engine.sources = vec![LATTICE_SOURCE, LATTICE_SOURCE, 0];
    engine.withheld = vec!["丁".into(), "戊".into()];
    let mut harness = Harness::new(engine, vec![ops(&[], &[2], &[(0, "注")])]);
    let model = favouring_model(&['甲', '乙', '丙'], &['乙']);
    harness
        .runtime
        .set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
    harness.key(b'a');
    assert_eq!(harness.calls(), 1);
    assert_eq!(harness.shown(), ["甲", "乙"]);

    // 落定重排挪了乙，丙仍然隐藏、甲的注释仍在，插件没有再被调用。
    assert!(harness.runtime.rerank_settled());
    assert_eq!(harness.shown(), ["乙", "甲"]);
    assert_eq!(
        harness.runtime.view().candidates[1]
            .script_annotation
            .as_deref(),
        Some("注")
    );
    assert_eq!(harness.calls(), 1);

    // 在线候选到达走的刷新、宿主要整个列表时的展开、改光标、释义：都只重放。
    harness.runtime.refresh_online_candidates().unwrap();
    assert!(!harness.shown().contains(&"丙".to_owned()));
    let all: Vec<_> = harness
        .runtime
        .all_candidates()
        .candidates
        .into_iter()
        .map(|candidate| candidate.text)
        .collect();
    assert_eq!(all.len(), 4);
    assert!(!all.contains(&"丙".to_owned()));
    harness.runtime.set_caret(None).unwrap();
    let generation = harness.runtime.generation();
    harness
        .runtime
        .apply_translations(generation, [("甲".to_owned(), "first".to_owned())]);
    assert_eq!(harness.calls(), 1);
    assert!(!harness.shown().contains(&"丙".to_owned()));

    // 换了插件的输出之后，只有下一次派发才会用上它。
    *harness.ops.borrow_mut() = vec![ops(&[], &[], &[])];
    harness.runtime.refresh_online_candidates().unwrap();
    assert!(!harness.shown().contains(&"丙".to_owned()));
    harness.key(b'a');
    assert_eq!(harness.calls(), 2);
    assert!(harness.shown().contains(&"丙".to_owned()));
}

#[test]
fn settled_rerank_reuses_the_replayed_seat_order_buffer() {
    let mut harness = Harness::new(
        TransformEngine::new(&["甲", "乙", "丙", "丁"]),
        vec![ops(&[], &[3], &[])],
    );
    let model = favouring_model(&['甲', '乙', '丙', '丁'], &['丁']);
    harness
        .runtime
        .set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
    harness.key(b'a');
    let _ = harness.runtime.rerank_settled();
    let (_, allocations) = allocations::count(|| harness.runtime.rerank_settled());
    assert_eq!(
        allocations, 0,
        "settled rerank seat order allocations: {allocations}"
    );
}

#[test]
fn failed_snapshot_discards_recorded_transform_before_side_channel_recovery() {
    let mut harness = Harness::new(
        TransformEngine::new(&["甲", "乙", "丙"]),
        vec![ops(&[], &[1], &[(2, "注")])],
    );
    harness.key(b'a');
    assert_eq!(harness.shown(), ["甲", "丙"]);
    assert!(harness.runtime.has_candidate_ops());

    harness.runtime.engine.snapshot_fails = true;
    assert!(harness.runtime.refresh().is_err());
    assert!(harness.runtime.view().candidates.is_empty());
    assert!(!harness.runtime.has_candidate_ops());

    harness.runtime.engine.snapshot_fails = false;
    harness.runtime.engine.words = ["新", "乙", "后"].map(str::to_owned).to_vec();
    harness.runtime.refresh_online_candidates().unwrap();
    assert_eq!(harness.shown(), ["新", "乙", "后"]);
    assert!(harness
        .runtime
        .view()
        .candidates
        .iter()
        .all(|candidate| candidate.script_annotation.is_none()));
    assert_eq!(harness.calls(), 1);

    harness.key(b'b');
    assert_eq!(harness.shown(), ["新", "后"]);
    assert_eq!(
        harness.runtime.view().candidates[1]
            .script_annotation
            .as_deref(),
        Some("注")
    );
    assert_eq!(harness.calls(), 2);
}

/// 落定重排提上来的是一行隐藏行时，可见的行一行没动：不算挪动，高亮留在原处，旧的候选 ID 仍然有效。
#[test]
fn a_settled_rerank_that_only_reshuffles_hidden_rows_is_not_a_move() {
    let mut engine = TransformEngine::new(&["甲", "乙", "丙", "丁"]);
    engine.sources = vec![LATTICE_SOURCE; 4];
    let mut harness = Harness::new(engine, vec![ops(&[], &[2, 3], &[])]);
    let model = favouring_model(&['甲', '乙', '丙', '丁'], &['丁']);
    harness
        .runtime
        .set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
    harness.key(b'a');
    harness.runtime.dispatch(Action::NextCandidate).unwrap();
    assert_eq!(harness.shown(), ["甲", "乙"]);
    assert_eq!(harness.runtime.highlighted, 1);
    let generation = harness.runtime.generation();
    let epoch = harness.runtime.candidates_epoch();
    let order = harness.runtime.seat_order();

    assert!(!harness.runtime.rerank_settled());
    assert_eq!(harness.shown(), ["甲", "乙"]);
    assert_eq!(harness.runtime.highlighted, 1);
    assert_eq!(harness.runtime.generation(), generation);
    assert_eq!(harness.runtime.candidates_epoch(), epoch);
    assert_eq!(harness.runtime.seat_order(), order);
    assert_eq!(harness.calls(), 1);
}

#[test]
fn expanding_inside_a_dispatch_runs_the_transform_again() {
    let mut engine = TransformEngine::new(&["甲", "乙", "丙", "丁"]);
    engine.withheld = vec!["戊".into(), "己".into()];
    let mut harness = Harness::new(engine, vec![ops(&[], &[1], &[])]);
    harness.key(b'a');
    assert_eq!(harness.calls(), 1);
    // 走到最后一个候选时引擎交出留着的候选：这是派发里的展开，插件看到的是新的窗口。
    harness.runtime.dispatch(Action::LastCandidate).unwrap();
    assert_eq!(harness.calls(), 2);
    assert_eq!(harness.calls.borrow().windows[1].len(), 6);
    assert_eq!(harness.runtime.visible_len(), 5);
}

#[test]
fn an_annotation_alone_changes_the_candidate_view() {
    let mut harness = Harness::new(TransformEngine::new(&FIVE), vec![ops(&[], &[], &[])]);
    harness.key(b'a');
    let epoch = harness.runtime.candidates_epoch();
    harness.runtime.dispatch(Action::ResetCache).unwrap();
    assert_eq!(harness.runtime.candidates_epoch(), epoch);
    *harness.ops.borrow_mut() = vec![ops(&[], &[], &[(1, "注")])];
    harness.runtime.dispatch(Action::ResetCache).unwrap();
    assert_ne!(harness.runtime.candidates_epoch(), epoch);
}

#[test]
fn ending_the_composition_forgets_the_operations() {
    let mut harness = reranked_and_filtered();
    assert!(harness.runtime.has_candidate_ops());
    harness
        .runtime
        .dispatch(Action::Command(Command::Cancel))
        .unwrap();
    assert!(!harness.runtime.has_candidate_ops());
    assert_eq!(harness.runtime.visible_len(), 0);
}

/// 派发里的展开会重新运行插件，插件这次可以多藏几行：高亮不能留在被隐藏的末尾，否则空格和结束组字按高亮交给引擎的就是一行被隐藏的候选。
#[test]
fn expanding_with_more_rows_hidden_pulls_the_highlight_back_into_view() {
    let words = ["甲", "乙", "丙", "丁", "戊", "己", "庚"];
    for name in ["space", "finish", "punctuation"] {
        let action = || match name {
            "space" => Action::SelectHighlighted,
            "finish" => Action::Finish,
            _ => Action::Punctuation(b'.'),
        };
        let mut harness = Harness::new(TransformEngine::new(&words), Vec::new());
        harness.key(b'a');
        // 第二页（己 庚）是最后一页，还没满；引擎这时没有留着的候选。
        harness.runtime.dispatch(Action::NextPage).unwrap();
        assert_eq!(harness.shown(), ["己", "庚"]);
        // 再翻页时引擎交出留着的候选，插件这次把引擎下标 0 以外的行都藏起来。
        harness.runtime.engine.withheld = vec!["辛".into(), "壬".into()];
        *harness.ops.borrow_mut() = vec![ops(&[], &[1, 2, 3, 4, 5, 6, 7, 8], &[])];
        harness.runtime.dispatch(Action::NextPage).unwrap();
        assert_eq!(harness.runtime.visible_len(), 1);
        let committed = harness.runtime.dispatch(action()).unwrap();
        assert!(committed.commit.unwrap().starts_with('甲'), "{name}");
        assert_eq!(harness.runtime.engine.picks, [0], "{name}");
    }
}

/// 宿主暂停候选变换时（上屏只能观察），派发不运行插件、旁路调用不重放，选中的是内置排序里的那一行；恢复之后从下一次派发起重新运行。
#[test]
fn a_suspended_transform_neither_runs_nor_replays() {
    let mut harness = reranked_and_filtered();
    assert_eq!(harness.calls(), 1);
    assert_eq!(harness.shown(), ["丁", "乙", "甲", "戊"]);

    harness.runtime.suspend_candidate_transform(true);
    // 旁路调用不再重放之前记下的重排和隐藏。
    harness.runtime.refresh_online_candidates().unwrap();
    assert_eq!(harness.shown(), FIVE);
    assert!(!harness.runtime.has_candidate_ops());
    // 派发里也不运行插件，数字选的是内置排序里的第二行。
    harness.key(b'a');
    assert_eq!(harness.calls(), 1);
    assert_eq!(harness.shown(), FIVE);
    let picked = harness.key(b'2');
    assert_eq!(picked.commit.as_deref(), Some("乙"));
    assert_eq!(harness.runtime.engine.picks, [1]);

    harness.runtime.suspend_candidate_transform(false);
    harness.key(b'a');
    assert_eq!(harness.calls(), 2);
    assert_eq!(harness.shown(), ["丁", "乙", "甲", "戊"]);
}

#[path = "candidate_transform/allocation_tests.rs"]
mod allocation_tests;

//! 状态机的穷举测试：把分类、游标、标记、基线、记下的清空、选择、云端和本机的情况全部组合一遍，用一个模拟的驱动跑到底，检查几条不变式；再单独覆盖冲突重试、云端被清空、`use_cloud` 遇空云端和词库的几条规则。

use super::*;
use crate::account::cloud_sync::policy::{LEASE_MS, MAX_BACKOFF_MS, THROTTLE_MS};

#[derive(Debug, Default)]
struct Trace {
    steps: Vec<Step>,
    puts: Vec<Mode>,
    writes: Vec<WriteTarget>,
    commit: Option<SectionCommit>,
}

impl Trace {
    fn last(&self) -> &Step {
        self.steps.last().unwrap()
    }

    fn needs_choice(&self) -> Option<ChoiceReason> {
        match self.last() {
            Step::NeedsChoice(reason) => Some(*reason),
            _ => None,
        }
    }

    fn outcome(&self) -> Option<&SectionOutcome> {
        match self.last() {
            Step::Done(outcome) => Some(outcome),
            _ => None,
        }
    }
}

/// 模拟的驱动：第一次读云端给 `cloud`，之后（冲突重试）给 `refetch`；每次 `Put` 依次取 `puts` 里的结果，取完以后都成功。
fn drive(
    input: SectionInput,
    cloud: CloudProbe,
    local: LocalProbe,
    mut puts: Vec<Result<i64, HttpFailure>>,
    refetch: CloudProbe,
) -> Trace {
    let mut machine = SectionMachine::new(input);
    let mut trace = Trace::default();
    let mut fetched = false;
    let mut revision = cloud.revision;
    let mut step = machine.next(None);
    for _ in 0..32 {
        trace.steps.push(step.clone());
        let event = match &step {
            Step::Fetch => {
                let probe = if fetched { refetch } else { cloud };
                fetched = true;
                revision = probe.revision;
                Some(Event::Fetched(probe))
            }
            Step::ReadLocal => Some(Event::LocalRead(local)),
            Step::Put(mode) => {
                trace.puts.push(*mode);
                let result = if puts.is_empty() {
                    Ok(revision + 1)
                } else {
                    puts.remove(0)
                };
                match result {
                    Ok(saved) => {
                        revision = saved;
                        Some(Event::Written { revision: saved })
                    }
                    Err(failure) => Some(Event::HttpFailed(failure)),
                }
            }
            Step::WriteLocal(target) => {
                trace.writes.push(*target);
                Some(Event::Written { revision })
            }
            Step::Commit(commit) => {
                trace.commit = Some(*commit);
                None
            }
            Step::NeedsChoice(_) | Step::Done(_) => return trace,
        };
        step = machine.next(event);
    }
    panic!("machine did not finish: {:?}", trace.steps);
}

fn conflict() -> HttpFailure {
    HttpFailure {
        status: Some(409),
        code: Some("revision_conflict".into()),
        retry_after_seconds: None,
    }
}

fn input(section: Section, cursor: Option<i64>) -> SectionInput {
    SectionInput {
        section,
        cursor,
        dirty: false,
        base_present: cursor.is_some(),
        cleared: false,
        choice: None,
    }
}

fn cloud(revision: i64, has_data: bool) -> CloudProbe {
    CloudProbe {
        revision,
        has_data,
        reset_and_empty: false,
    }
}

fn local(has_data: bool) -> LocalProbe {
    LocalProbe {
        has_data,
        content_dirty: false,
        upload_blocked: false,
        activation: SnapshotActivation::Idle,
    }
}

#[test]
fn every_combination_finishes_and_keeps_the_invariants() {
    let sections = [Section::Settings, Section::Phrases, Section::Dictionary];
    let cursors = [None, Some(5)];
    let choices = [None, Some(Choice::Merge), Some(Choice::UseCloud)];
    let revisions = [0, 5, 7];
    let flags = [false, true];
    let activations = [
        SnapshotActivation::Idle,
        SnapshotActivation::Waiting,
        SnapshotActivation::Lost,
    ];
    let mut runs = 0;
    for section in sections {
        for cursor in cursors {
            for dirty in flags {
                for base_present in flags {
                    for cleared in flags {
                        for choice in choices {
                            for revision in revisions {
                                for has_data in flags {
                                    for reset_and_empty in flags {
                                        for local_data in flags {
                                            for content_dirty in flags {
                                                for (blocked, activation) in
                                                    flags.into_iter().flat_map(|blocked| {
                                                        activations.into_iter().map(
                                                            move |activation| (blocked, activation),
                                                        )
                                                    })
                                                {
                                                    let input = SectionInput {
                                                        section,
                                                        cursor,
                                                        dirty,
                                                        base_present,
                                                        cleared,
                                                        choice,
                                                    };
                                                    let cloud = CloudProbe {
                                                        revision,
                                                        has_data,
                                                        reset_and_empty,
                                                    };
                                                    let local = LocalProbe {
                                                        has_data: local_data,
                                                        content_dirty,
                                                        // 等激活时驱动一定也报上传受阻。
                                                        upload_blocked: blocked
                                                            || activation
                                                                == SnapshotActivation::Waiting,
                                                        activation,
                                                    };
                                                    check(input, cloud, local);
                                                    runs += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 3 * 2 * 2 * 2 * 2 * 3 * 3 * 2 * 2 * 2 * 2 * 2 * 3);
}

fn check(input: SectionInput, cloud: CloudProbe, local: LocalProbe) {
    let trace = drive(input, cloud, local, Vec::new(), cloud);
    let context = format!("{input:?} {cloud:?} {local:?} {:?}", trace.steps);
    let dictionary = input.section == Section::Dictionary;
    let changed = input.cursor != Some(cloud.revision);
    let cleared_now = if dictionary {
        input.cursor.is_some() && changed && cloud.reset_and_empty
    } else {
        input.base_present && changed && input.cursor.is_some() && !cloud.has_data
    };
    let awaiting = input.cleared || cleared_now;
    let first = input.cursor.is_none() || awaiting;

    // 有选择时从不再问；问的时候一定是首次同步而云端有数据，或者云端被清空。
    match trace.needs_choice() {
        Some(reason) => {
            assert!(input.choice.is_none(), "{context}");
            assert!(first && (cloud.has_data || awaiting), "{context}");
            assert_eq!(reason == ChoiceReason::CloudCleared, awaiting, "{context}");
            assert!(
                trace.puts.is_empty() && trace.writes.is_empty(),
                "{context}"
            );
            return;
        }
        None => assert!(
            !(first && (cloud.has_data || awaiting) && input.choice.is_none()),
            "{context}"
        ),
    }
    let commit = trace.commit;
    // 冲突重试次数有上限。
    assert!(
        trace.puts.len() <= 1 + MAX_CONFLICT_RETRIES as usize,
        "{context}"
    );
    // 词库导入队列没清空时从不上传。
    if dictionary && local.upload_blocked {
        assert!(trace.puts.is_empty(), "{context}");
    }
    // 下载的快照还在等激活：哪一边都不写。
    if dictionary && local.activation == SnapshotActivation::Waiting {
        assert!(
            trace.puts.is_empty() && trace.writes.is_empty(),
            "{context}"
        );
    }
    // 下载的快照没激活就结束了（下载过，所以一定有游标）：不上传，增量时一定把云端的词补进本机（本机没词时重新下载）。
    if dictionary && local.activation == SnapshotActivation::Lost && input.cursor.is_some() {
        assert!(trace.puts.is_empty(), "{context}");
        if !first {
            let expected = if local.has_data {
                WriteTarget::MergeIntoLocal
            } else {
                WriteTarget::Cloud
            };
            assert_eq!(trace.writes, [expected], "{context}");
        }
    }
    // 用云端替换本机时，云端为空、本机有数据只可能出在没有基线的增量下载上（旧版本导入的状态），首次和云端被清空的路径都不会。
    if !dictionary
        && trace.writes.contains(&WriteTarget::Cloud)
        && !cloud.has_data
        && local.has_data
    {
        assert!(!first && !input.base_present, "{context}");
    }
    // 「使用云端」遇到空云端或从没上传过的云端、本机又有数据时，不清空本机。
    if first
        && input.choice == Some(Choice::UseCloud)
        && local.has_data
        && (cloud.revision == 0 || !cloud.has_data)
    {
        assert!(!trace.writes.contains(&WriteTarget::Cloud), "{context}");
    }
    // 增量的模式表。
    if !first && !(dictionary && local.activation == SnapshotActivation::Lost) {
        let expected = incremental(changed, input.dirty || local.content_dirty);
        let mode = commit.map(|commit| commit.mode);
        match trace.outcome().map(|outcome| outcome.outcome) {
            Some(OutcomeKind::Skipped) => assert!(
                dictionary
                    && (expected == Mode::Upload && local.upload_blocked
                        || expected != Mode::None
                            && local.activation == SnapshotActivation::Waiting),
                "{context}"
            ),
            _ => assert_eq!(mode, Some(expected), "{context}"),
        }
    }
    // 走完的一定提交了；提交时清掉「云端被清空」。
    if let Some(outcome) = trace.outcome() {
        if matches!(
            outcome.outcome,
            OutcomeKind::Synced | OutcomeKind::Unchanged
        ) {
            let commit = commit.expect("a finished section commits");
            assert_eq!(commit.reset_cleared, awaiting, "{context}");
            if dictionary && matches!(commit.mode, Mode::Download | Mode::Merge) {
                assert!(!commit.clear_dirty, "{context}");
            } else {
                assert!(commit.clear_dirty, "{context}");
            }
            assert_eq!(
                commit.mark_dirty,
                dictionary && commit.mode == Mode::Merge,
                "{context}"
            );
            // 等激活的记录只在下载（改记新的）和还在等的时候留着。
            assert_eq!(
                commit.clear_activation,
                dictionary
                    && commit.mode != Mode::Download
                    && local.activation != SnapshotActivation::Waiting,
                "{context}"
            );
        }
    }
}

#[test]
fn a_conflicting_put_is_refetched_and_merged_up_to_two_times() {
    let mut first_run = input(Section::Phrases, Some(5));
    first_run.dirty = true;
    let trace = drive(
        first_run,
        cloud(5, true),
        local(true),
        vec![Err(conflict()), Err(conflict())],
        cloud(6, true),
    );
    assert_eq!(trace.puts, [Mode::Upload, Mode::Merge, Mode::Merge]);
    assert_eq!(trace.writes, [WriteTarget::Saved]);
    assert_eq!(trace.commit.unwrap().cursor, 7);
    assert_eq!(trace.outcome().unwrap().outcome, OutcomeKind::Synced);

    let trace = drive(
        first_run,
        cloud(5, true),
        local(true),
        vec![Err(conflict()), Err(conflict()), Err(conflict())],
        cloud(6, true),
    );
    assert_eq!(trace.puts.len(), 3);
    let outcome = trace.outcome().unwrap();
    assert_eq!(outcome.outcome, OutcomeKind::Failed);
    assert_eq!(outcome.reason.as_deref(), Some("revision_conflict"));
    assert!(trace.commit.is_none());
}

#[test]
fn other_failures_are_not_retried_and_keep_their_details() {
    let mut dirty = input(Section::Settings, Some(5));
    dirty.dirty = true;
    let mut machine = SectionMachine::new(dirty);
    assert_eq!(machine.next(None), Step::Fetch);
    assert_eq!(
        machine.next(Some(Event::Fetched(cloud(5, true)))),
        Step::ReadLocal
    );
    assert_eq!(
        machine.next(Some(Event::LocalRead(local(true)))),
        Step::Put(Mode::Upload)
    );
    let limited = HttpFailure {
        status: Some(429),
        code: Some("rate_limit_exceeded".into()),
        retry_after_seconds: Some(60),
    };
    let step = machine.next(Some(Event::HttpFailed(limited.clone())));
    assert!(matches!(
        step,
        Step::Done(SectionOutcome {
            outcome: OutcomeKind::Failed,
            ..
        })
    ));
    assert_eq!(machine.failure(), Some(&limited));
    // 结束以后再喂什么都还是那个结果。
    assert_eq!(machine.next(None), step);

    let mut network = SectionMachine::new(input(Section::Phrases, None));
    network.next(None);
    let step = network.next(Some(Event::HttpFailed(HttpFailure {
        status: None,
        code: None,
        retry_after_seconds: None,
    })));
    assert_eq!(
        step,
        Step::Done(SectionOutcome::new(OutcomeKind::Failed, Some("network")))
    );
}

#[test]
fn a_cleared_cloud_asks_and_a_choice_reuploads_instead_of_wiping() {
    // 本机有基线，云端修订号前进了、内容为空：问用户。
    let trace = drive(
        input(Section::Phrases, Some(5)),
        cloud(6, false),
        local(true),
        Vec::new(),
        cloud(6, false),
    );
    assert_eq!(trace.needs_choice(), Some(ChoiceReason::CloudCleared));
    // 状态里记着清空，下一轮选了「使用云端」：云端为空，按合并上传本机。
    for choice in [Choice::UseCloud, Choice::Merge] {
        let mut chosen = input(Section::Phrases, Some(5));
        chosen.cleared = true;
        chosen.choice = Some(choice);
        let trace = drive(
            chosen,
            cloud(6, false),
            local(true),
            Vec::new(),
            cloud(6, false),
        );
        assert_eq!(trace.puts, [Mode::Merge], "{choice:?}");
        assert!(trace.commit.unwrap().reset_cleared);
    }
    // 本机也空：什么都不写，只记下游标。
    let mut chosen = input(Section::Settings, Some(5));
    chosen.cleared = true;
    chosen.choice = Some(Choice::UseCloud);
    let trace = drive(
        chosen,
        cloud(6, false),
        local(false),
        Vec::new(),
        cloud(6, false),
    );
    assert!(trace.puts.is_empty());
    assert_eq!(trace.writes, [WriteTarget::Cloud]);

    // 词库：第一条新改动是 reset 而且云端已空。
    let mut probe = cloud(9, true);
    probe.reset_and_empty = true;
    let trace = drive(
        input(Section::Dictionary, Some(5)),
        probe,
        local(true),
        Vec::new(),
        probe,
    );
    assert_eq!(trace.needs_choice(), Some(ChoiceReason::CloudCleared));
    // 别的设备整份上传快照也写 reset，但云端不空：照常下载。
    let trace = drive(
        input(Section::Dictionary, Some(5)),
        cloud(9, true),
        local(true),
        Vec::new(),
        cloud(9, true),
    );
    assert_eq!(trace.writes, [WriteTarget::Cloud]);
}

#[test]
fn a_cloud_cleared_between_the_read_and_the_write_asks_instead_of_merging() {
    // 读云端时还有数据，写的时候 409：别的设备在这中间「删除云端数据」，重读到的是空的。
    for section in [Section::Settings, Section::Phrases] {
        let mut dirty = input(section, Some(5));
        dirty.dirty = true;
        let trace = drive(
            dirty,
            cloud(5, true),
            local(true),
            vec![Err(conflict())],
            cloud(6, false),
        );
        assert_eq!(
            trace.needs_choice(),
            Some(ChoiceReason::CloudCleared),
            "{section:?} {:?}",
            trace.steps
        );
        assert_eq!(trace.puts, [Mode::Upload], "{section:?}");
        assert!(
            trace.writes.is_empty() && trace.commit.is_none(),
            "{section:?}"
        );
    }
    // 词库：重读到第一条新改动是 reset 而且云端已空，不把云端的空快照并进本机再标待上传。
    let mut dirty = input(Section::Dictionary, Some(4));
    dirty.dirty = true;
    let mut emptied = cloud(6, false);
    emptied.reset_and_empty = true;
    let trace = drive(
        dirty,
        cloud(4, true),
        local(true),
        vec![Err(conflict())],
        emptied,
    );
    assert_eq!(trace.needs_choice(), Some(ChoiceReason::CloudCleared));
    assert!(trace.writes.is_empty() && trace.commit.is_none());
    // 这一轮本来就是清空以后带着选择来的：重读到的还是空的也不再问，照常合并。
    let mut chosen = input(Section::Phrases, Some(5));
    chosen.cleared = true;
    chosen.choice = Some(Choice::Merge);
    let trace = drive(
        chosen,
        cloud(6, false),
        local(true),
        vec![Err(conflict())],
        cloud(7, false),
    );
    assert_eq!(trace.puts, [Mode::Merge, Mode::Merge]);
    assert!(trace.commit.unwrap().reset_cleared);
}

#[test]
fn first_run_asks_when_the_cloud_has_data_and_uploads_otherwise() {
    let trace = drive(
        input(Section::Settings, None),
        cloud(3, true),
        local(true),
        Vec::new(),
        cloud(3, true),
    );
    assert_eq!(trace.needs_choice(), Some(ChoiceReason::FirstRun));
    let trace = drive(
        input(Section::Settings, None),
        cloud(0, false),
        local(true),
        Vec::new(),
        cloud(0, false),
    );
    assert_eq!(trace.puts, [Mode::Upload]);
    let trace = drive(
        input(Section::Phrases, None),
        cloud(0, false),
        local(false),
        Vec::new(),
        cloud(0, false),
    );
    assert!(trace.puts.is_empty() && trace.writes.is_empty());
    assert_eq!(trace.commit.unwrap().cursor, 0);
    let mut chosen = input(Section::Phrases, None);
    chosen.choice = Some(Choice::UseCloud);
    let trace = drive(
        chosen,
        cloud(3, true),
        local(true),
        Vec::new(),
        cloud(3, true),
    );
    assert_eq!(trace.writes, [WriteTarget::Cloud]);
    // 云端修订号是 0（从没上传过）时「使用云端」按合并处理。
    let trace = drive(
        chosen,
        cloud(0, true),
        local(true),
        Vec::new(),
        cloud(0, true),
    );
    assert_eq!(trace.puts, [Mode::Merge]);
}

#[test]
fn dictionary_rules() {
    // 首次合并而本机没有词：直接下载。
    let mut chosen = input(Section::Dictionary, None);
    chosen.choice = Some(Choice::Merge);
    let trace = drive(
        chosen,
        cloud(4, true),
        local(false),
        Vec::new(),
        cloud(4, true),
    );
    assert_eq!(trace.writes, [WriteTarget::Cloud]);
    assert!(!trace.commit.unwrap().clear_dirty);
    // 本机有词：并进本机，提交时再标一次待上传。
    let trace = drive(
        chosen,
        cloud(4, true),
        local(true),
        Vec::new(),
        cloud(4, true),
    );
    assert_eq!(trace.writes, [WriteTarget::MergeIntoLocal]);
    assert!(trace.commit.unwrap().mark_dirty);
    // 导入队列没清空时推迟上传。
    let mut dirty = input(Section::Dictionary, Some(4));
    dirty.dirty = true;
    let mut blocked = local(true);
    blocked.upload_blocked = true;
    let trace = drive(dirty, cloud(4, true), blocked, Vec::new(), cloud(4, true));
    assert_eq!(
        trace.outcome().unwrap(),
        &SectionOutcome::new(OutcomeKind::Skipped, Some("dictionary_pending"))
    );
    // 上传冲突：云端在探测之后变了，改成把云端的词并进本机。
    let trace = drive(
        dirty,
        cloud(4, true),
        local(true),
        vec![Err(conflict())],
        cloud(6, true),
    );
    assert_eq!(trace.puts, [Mode::Upload]);
    assert_eq!(trace.writes, [WriteTarget::MergeIntoLocal]);
    assert_eq!(trace.commit.unwrap().cursor, 6);
    // 上传成功时游标是服务端给的新修订号，标记清掉。
    let trace = drive(
        dirty,
        cloud(4, true),
        local(true),
        vec![Ok(5)],
        cloud(4, true),
    );
    let commit = trace.commit.unwrap();
    assert_eq!((commit.cursor, commit.clear_dirty), (5, true));
}

#[test]
fn an_empty_snapshot_never_replaces_local_words() {
    // 首次同步选了「使用云端」，云端汇总没读出来、按有词处理：下载到的快照却一个词都没有。不替换本机，改按合并处理，提交后再标一次待上传。
    let mut chosen = input(Section::Dictionary, None);
    chosen.choice = Some(Choice::UseCloud);
    let mut machine = SectionMachine::new(chosen);
    assert_eq!(machine.next(None), Step::Fetch);
    assert_eq!(
        machine.next(Some(Event::Fetched(cloud(4, true)))),
        Step::ReadLocal
    );
    assert_eq!(
        machine.next(Some(Event::LocalRead(local(true)))),
        Step::WriteLocal(WriteTarget::Cloud)
    );
    assert_eq!(
        machine.next(Some(Event::EmptySnapshot)),
        Step::WriteLocal(WriteTarget::MergeIntoLocal)
    );
    assert_eq!(machine.mode(), Mode::Merge);
    let Step::Commit(commit) = machine.next(Some(Event::Written { revision: 4 })) else {
        panic!("a merge commits");
    };
    assert_eq!(commit.mode, Mode::Merge);
    assert_eq!(commit.cursor, 4);
    assert!(commit.mark_dirty && !commit.clear_dirty);
    // 之后云端有改动、本机没改：照样不拿空快照替换本机的词。
    let mut machine = SectionMachine::new(input(Section::Dictionary, Some(4)));
    machine.next(None);
    machine.next(Some(Event::Fetched(cloud(6, true))));
    assert_eq!(
        machine.next(Some(Event::LocalRead(local(true)))),
        Step::WriteLocal(WriteTarget::Cloud)
    );
    assert_eq!(
        machine.next(Some(Event::EmptySnapshot)),
        Step::WriteLocal(WriteTarget::MergeIntoLocal)
    );
    // 本机没有词时驱动不会报这个事件；报了也不改成合并，这一类失败，什么都不写。
    let mut machine = SectionMachine::new(input(Section::Dictionary, Some(4)));
    machine.next(None);
    machine.next(Some(Event::Fetched(cloud(6, true))));
    machine.next(Some(Event::LocalRead(local(false))));
    assert_eq!(
        machine.next(Some(Event::EmptySnapshot)),
        Step::Done(SectionOutcome::new(
            OutcomeKind::Failed,
            Some("unexpected_event")
        ))
    );
}

#[test]
fn a_downloaded_snapshot_that_never_activated_is_merged_instead_of_uploaded() {
    // 下载过快照、游标已经是它的修订号；本机有改动、云端没变，本来该上传。
    let mut dirty = input(Section::Dictionary, Some(4));
    dirty.dirty = true;
    let mut lost = local(true);
    lost.activation = SnapshotActivation::Lost;
    let trace = drive(dirty, cloud(4, true), lost, Vec::new(), cloud(4, true));
    assert!(trace.puts.is_empty());
    assert_eq!(trace.writes, [WriteTarget::MergeIntoLocal]);
    let commit = trace.commit.unwrap();
    assert_eq!(commit.mode, Mode::Merge);
    assert!(commit.mark_dirty && commit.clear_activation && !commit.clear_dirty);
    // 两边都没变也要补：本机缺着下载时的那些词。本机没有词时再下载一次。
    let mut empty = local(false);
    empty.activation = SnapshotActivation::Lost;
    let trace = drive(
        input(Section::Dictionary, Some(4)),
        cloud(4, true),
        empty,
        Vec::new(),
        cloud(4, true),
    );
    assert_eq!(trace.writes, [WriteTarget::Cloud]);
    assert!(!trace.commit.unwrap().clear_activation);
    // 还在等激活：云端又变了也不下载、不合并。
    let mut waiting = local(true);
    waiting.activation = SnapshotActivation::Waiting;
    waiting.upload_blocked = true;
    let trace = drive(
        input(Section::Dictionary, Some(4)),
        cloud(6, true),
        waiting,
        Vec::new(),
        cloud(6, true),
    );
    assert!(trace.writes.is_empty() && trace.puts.is_empty());
    assert_eq!(
        trace.outcome().unwrap(),
        &SectionOutcome::new(OutcomeKind::Skipped, Some("dictionary_pending"))
    );
    // 两边都没变时照常提交游标，记着的快照留着。
    let trace = drive(
        input(Section::Dictionary, Some(4)),
        cloud(4, true),
        waiting,
        Vec::new(),
        cloud(4, true),
    );
    assert!(!trace.commit.unwrap().clear_activation);
}

#[test]
fn content_changes_count_as_local_changes() {
    let mut changed = local(true);
    changed.content_dirty = true;
    let trace = drive(
        input(Section::Phrases, Some(5)),
        cloud(5, true),
        changed,
        Vec::new(),
        cloud(5, true),
    );
    assert_eq!(trace.puts, [Mode::Upload]);
}

#[test]
fn unexpected_events_end_the_section_as_failed() {
    let mut machine = SectionMachine::new(input(Section::Phrases, None));
    machine.next(None);
    let step = machine.next(Some(Event::Written { revision: 1 }));
    assert_eq!(
        step,
        Step::Done(SectionOutcome::new(
            OutcomeKind::Failed,
            Some("unexpected_event")
        ))
    );
    let mut machine = SectionMachine::new(input(Section::Phrases, None));
    machine.next(None);
    machine.next(Some(Event::Fetched(cloud(0, false))));
    let step = machine.next(Some(Event::LocalFailed {
        reason: "phrases_storage".into(),
    }));
    assert_eq!(
        step,
        Step::Done(SectionOutcome::new(
            OutcomeKind::Failed,
            Some("phrases_storage")
        ))
    );
}

#[test]
fn the_gate_needs_an_account_respects_backoff_throttle_and_the_lease() {
    let all = Section::ALL;
    let now = 10 * THROTTLE_MS;
    let mut state = SyncState::default();
    let check = |state: &SyncState, now: u64, trigger: Trigger, choice: Option<Choice>| {
        gate(state, now, trigger, choice.is_some(), &all)
    };
    assert_eq!(
        check(&state, now, Trigger::Manual, None),
        Err(SkipReason::SignedOut)
    );
    state.reconcile(Some("u"));
    assert_eq!(
        check(&state, now, Trigger::Manual, None),
        Err(SkipReason::Disabled)
    );
    state.set_enabled(true).unwrap();
    assert_eq!(check(&state, now, Trigger::Active, None), Ok(()));
    state.last_attempt_ms = Some(now - 1_000);
    // 打开开关时有标记，但距上次不到 30 秒。
    assert_eq!(
        check(&state, now, Trigger::Active, None),
        Err(SkipReason::Throttled)
    );
    assert_eq!(check(&state, now, Trigger::Manual, None), Ok(()));
    assert_eq!(
        check(&state, now, Trigger::Active, Some(Choice::Merge)),
        Ok(())
    );
    // 只看参与的分类：标记都清掉以后，30 秒的去抖不再适用。
    for section in Section::ALL {
        let value = state.sections.get_mut(section);
        value.clean = value.generation;
    }
    state.last_attempt_ms = Some(now - 40_000);
    assert_eq!(
        check(&state, now, Trigger::Active, None),
        Err(SkipReason::Throttled)
    );
    state.sections.dictionary.generation += 1;
    assert_eq!(check(&state, now, Trigger::Active, None), Ok(()));
    assert_eq!(
        gate(&state, now, Trigger::Active, false, &[Section::Phrases]),
        Err(SkipReason::Throttled)
    );
    // 服务端要求的退避连手动同步也挡。
    state.not_before_ms = Some(now + 1);
    assert_eq!(
        check(&state, now, Trigger::Manual, None),
        Err(SkipReason::Backoff)
    );
    state.not_before_ms = Some(now + MAX_BACKOFF_MS);
    assert_eq!(
        check(&state, now, Trigger::Manual, None),
        Err(SkipReason::Backoff)
    );
    // 离现在超过一天的退避是时钟往回拨以前记下的，不再挡。
    state.not_before_ms = Some(now + MAX_BACKOFF_MS + 1);
    assert_eq!(check(&state, now, Trigger::Manual, None), Ok(()));
    state.not_before_ms = Some(now);
    assert_eq!(check(&state, now, Trigger::Manual, None), Ok(()));
    state.begin_run("u", now, None);
    assert_eq!(
        check(&state, now + 1, Trigger::Manual, None),
        Err(SkipReason::Busy)
    );
    assert_eq!(check(&state, now + LEASE_MS, Trigger::Manual, None), Ok(()));
}

#[test]
fn disabling_on_a_cleared_cloud_needs_an_account_an_open_switch_and_a_cleared_section_only() {
    let now = 10 * THROTTLE_MS;
    let mut state = SyncState::default();
    assert_eq!(disable_gate(&state), Err(SkipReason::SignedOut));
    state.reconcile(Some("u"));
    assert_eq!(disable_gate(&state), Err(SkipReason::Disabled));
    state.set_enabled(true).unwrap();
    // 首次同步的提示（没有分类记着清空）没有这个回答。
    assert_eq!(disable_gate(&state), Err(SkipReason::NotCloudCleared));
    assert_eq!(SkipReason::NotCloudCleared.as_str(), "not_cloud_cleared");
    for section in Section::ALL {
        let mut cleared = state.clone();
        cleared.sections.get_mut(section).cleared = true;
        assert_eq!(disable_gate(&cleared), Ok(()), "{section:?}");
    }
    // 不联网、不跑一轮：退避、节流和另一轮的租约都不挡，同步选择这时会被挡住。
    state.sections.phrases.cleared = true;
    state.not_before_ms = Some(now + 60_000);
    state.last_attempt_ms = Some(now - 1_000);
    state.begin_run("u", now, None);
    assert_eq!(
        gate(&state, now + 1, Trigger::Choice, true, &Section::ALL),
        Err(SkipReason::Backoff)
    );
    assert_eq!(disable_gate(&state), Ok(()));
    state.not_before_ms = None;
    assert_eq!(
        gate(&state, now + 1, Trigger::Choice, true, &Section::ALL),
        Err(SkipReason::Busy)
    );
    assert_eq!(disable_gate(&state), Ok(()));
    // 关掉以后不能再关一次，也不再跑。
    state.disable_keeping_local();
    assert_eq!(disable_gate(&state), Err(SkipReason::Disabled));
    assert_eq!(
        gate(
            &state,
            now + LEASE_MS,
            Trigger::Manual,
            false,
            &Section::ALL
        ),
        Err(SkipReason::Disabled)
    );
}

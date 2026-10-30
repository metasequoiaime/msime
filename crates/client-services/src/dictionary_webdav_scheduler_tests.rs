use super::*;
use crate::cloud::dictionary_sync::{Report, WordKind};
use crate::dictionary_webdav::SyncView;

const POLL: u64 = 5_000;
const MAX_DELAY: u64 = 60_000;
const IDLE: u64 = 300_000;
const RETRY: u64 = 30_000;

fn planner() -> Planner {
    Planner::new(Timing::default(), 0)
}

fn done(next_check_ms: u64) -> Ran {
    Ran::Done { next_check_ms }
}

#[test]
fn the_default_timing_matches_the_documented_intervals() {
    let timing = Timing::default();
    assert_eq!(millis(timing.poll), POLL);
    assert_eq!(millis(timing.max_delay), MAX_DELAY);
    assert_eq!(millis(timing.idle), IDLE);
    assert_eq!(millis(timing.retry), RETRY);
}

#[test]
fn it_runs_once_on_start_and_then_follows_next_check_ms() {
    let mut planner = planner();
    assert_eq!(planner.next(0), Step::Run(Trigger::Periodic));
    planner.finished(0, done(90_000));
    assert!(planner.active);
    // 刚可以同步时以那时的文件和词为准（后台线程在 `sync` 里做），之后按间隔看文件，到点再同步。
    planner.rebase(Some(7), Some(1));
    assert_eq!(planner.next(1), Step::Wait(POLL - 1));
    assert_eq!(planner.next(POLL), Step::Poll);
    planner.polled(POLL, Some(7));
    assert_eq!(planner.next(POLL + 1), Step::Wait(POLL - 1));
    planner.next_poll = 200_000;
    assert_eq!(planner.next(80_000), Step::Wait(10_000));
    assert_eq!(planner.next(90_000), Step::Run(Trigger::Periodic));
    // 核心给的间隔限在 1 秒到 30 分钟。
    planner.finished(90_000, done(0));
    assert_eq!(planner.due, 91_000);
    planner.finished(91_000, done(u64::MAX));
    assert_eq!(planner.due, 91_000 + schedule::PULL_INTERVAL_MS);
}

#[test]
fn a_change_is_checked_once_the_file_settles_and_only_new_words_trigger_local_change() {
    let mut planner = planner();
    planner.finished(0, done(schedule::PULL_INTERVAL_MS));
    planner.rebase(Some(1), Some(100));
    planner.polled(POLL, Some(1));
    assert_eq!(planner.next(POLL), Step::Wait(POLL));
    // 文件变了：等它安静一轮再读指纹。
    planner.polled(2 * POLL, Some(2));
    assert_eq!(planner.next(2 * POLL), Step::Wait(POLL));
    planner.polled(3 * POLL, Some(2));
    assert_eq!(planner.next(3 * POLL), Step::Fingerprint);
    // 只是权重变了：指纹一样，不调 local_change。
    assert!(!planner.fingerprinted(3 * POLL, Some(2), Some(100)));
    assert_eq!(planner.next(3 * POLL), Step::Wait(POLL));
    // 加了一个词：指纹变了，调一次，新的指纹记下来。
    planner.polled(4 * POLL, Some(3));
    planner.polled(5 * POLL, Some(3));
    assert!(planner.fingerprinted(5 * POLL, Some(3), Some(200)));
    assert_eq!(planner.baseline, Some(200));
    assert!(!planner.fingerprinted(6 * POLL, Some(3), Some(200)));
    // 读不出指纹时不算变化，也不丢掉基线。
    assert!(!planner.fingerprinted(7 * POLL, Some(4), None));
    assert_eq!(planner.baseline, Some(200));
}

#[test]
fn continuous_writes_cannot_postpone_the_check_beyond_max_delay() {
    let mut planner = planner();
    planner.finished(0, done(schedule::PULL_INTERVAL_MS));
    planner.rebase(Some(0), Some(100));
    let mut now = POLL;
    let mut stamp = 1;
    // 一直在打字，每一轮文件都变：第一次变化之后一分钟照样去读指纹。
    loop {
        planner.polled(now, Some(stamp));
        stamp += 1;
        if planner.next(now) == Step::Fingerprint {
            break;
        }
        now += POLL;
        assert!(
            now <= POLL + MAX_DELAY,
            "the check was postponed past max_delay"
        );
    }
    assert_eq!(now, POLL + MAX_DELAY);
}

#[test]
fn without_configuration_or_identity_it_neither_polls_nor_fingerprints() {
    let mut planner = planner();
    planner.finished(0, Ran::Inactive);
    assert!(!planner.active);
    assert_eq!(planner.next(1), Step::Wait(IDLE - 1));
    assert_eq!(planner.next(IDLE), Step::Run(Trigger::Periodic));
    // 可以同步之后又失去了配置：基线和看到的变化都清掉，不会拿旧的基线触发 local_change。
    planner.finished(IDLE, done(60_000));
    planner.rebase(Some(1), Some(100));
    planner.polled(IDLE + POLL, Some(2));
    planner.finished(IDLE + POLL, Ran::Inactive);
    assert_eq!(planner.baseline, None);
    assert_eq!(planner.changed_since, None);
    assert_eq!(planner.next(IDLE + POLL + 1), Step::Wait(IDLE - 1));
}

#[test]
fn a_local_failure_is_retried_without_changing_whether_it_is_active() {
    let mut planner = planner();
    planner.finished(0, Ran::Failed);
    assert!(!planner.active);
    assert_eq!(planner.due, RETRY);
    planner.finished(RETRY, done(60_000));
    planner.finished(RETRY + 1, Ran::Failed);
    assert!(planner.active);
    assert_eq!(planner.due, 2 * RETRY + 1);
}

#[test]
fn an_active_local_failure_defers_an_overdue_file_poll_until_retry() {
    let mut planner = planner();
    planner.finished(0, done(schedule::PULL_INTERVAL_MS));
    planner.rebase(Some(1), Some(100));
    assert_eq!(planner.next(POLL), Step::Poll);

    // 文件探测在更新 `next_poll` 前失败，下一步必须退避而不是马上重探。
    planner.finished(POLL, Ran::Failed);
    assert!(planner.active);
    assert_eq!(planner.next(POLL), Step::Wait(RETRY));
    assert_eq!(planner.next(POLL + RETRY - 1), Step::Wait(1));
    assert_eq!(planner.next(POLL + RETRY), Step::Run(Trigger::Periodic));
    planner.finished(POLL + RETRY, done(60_000));
    assert_eq!(planner.next(POLL + RETRY), Step::Poll);
    planner.polled(POLL + RETRY, Some(1));
    assert_eq!(planner.next(POLL + RETRY), Step::Wait(POLL));
}

#[test]
fn an_active_local_failure_defers_a_pending_fingerprint_until_retry() {
    let mut planner = planner();
    planner.finished(0, done(schedule::PULL_INTERVAL_MS));
    planner.rebase(Some(1), Some(100));
    planner.polled(POLL, Some(2));
    planner.polled(2 * POLL, Some(2));
    assert_eq!(planner.next(2 * POLL), Step::Fingerprint);

    // 指纹检查失败前还没有清掉待检查标记，不能立即重复同一个失败步骤。
    planner.finished(2 * POLL, Ran::Failed);
    assert_eq!(planner.next(2 * POLL), Step::Wait(RETRY));
    assert_eq!(planner.baseline, Some(100));
    assert_eq!(planner.changed_since, Some(POLL));
    // 重试再次失败时，从本次失败重新计时。
    planner.finished(2 * POLL + RETRY, Ran::Failed);
    assert_eq!(planner.next(2 * POLL + RETRY), Step::Wait(RETRY));
    assert_eq!(
        planner.next(2 * POLL + 2 * RETRY),
        Step::Run(Trigger::Periodic)
    );
    // 后续发现未配置时恢复原来的空闲间隔，不遗留失败阶段。
    planner.finished(2 * POLL + 2 * RETRY, Ran::Inactive);
    assert_eq!(planner.next(2 * POLL + 2 * RETRY), Step::Wait(IDLE));
}

fn view() -> SyncView {
    SyncView {
        device_id: None,
        code: None,
        approved: true,
        joined: true,
        words: 0,
        pending_upload: 0,
        pending_apply: 0,
        held_deletions: None,
        clock_skew: false,
        last_success_ms: None,
        last_error: None,
        budget_wait_until: None,
    }
}

fn result(report: Result<Report, DictionarySyncError>, next_check_ms: u64) -> RunResult {
    RunResult {
        network: false,
        report,
        next_check_ms,
        view: view(),
    }
}

#[test]
fn results_are_read_as_inactive_failed_or_done() {
    assert_eq!(Ran::of(None), Ran::Inactive);
    assert_eq!(
        Ran::of(Some(&Err(DictionarySyncError::Invalid))),
        Ran::Failed
    );
    let missing = DictionarySyncError::KeyRing("clipboard_identity_missing".into());
    assert_eq!(Ran::of(Some(&Ok(result(Err(missing), 5)))), Ran::Inactive);
    // 网盘上的失败照样按核心给的间隔再来。
    let unauthorized =
        DictionarySyncError::Sync(crate::cloud::dictionary_sync::SyncError::Unauthorized);
    assert_eq!(Ran::of(Some(&Ok(result(Err(unauthorized), 7)))), done(7));
    assert_eq!(
        Ran::of(Some(&Ok(result(Ok(Report::default()), 9)))),
        done(9)
    );
}

/// 内存里的「我的词条」。
#[derive(Default)]
struct Words(BTreeSet<WordKey>);

impl LocalDictionary for Words {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.0.clone())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        for change in changes {
            self.0.insert(change.key.clone());
        }
        Ok(vec![LocalOutcome::Applied; changes.len()])
    }
}

#[test]
fn watched_remembers_the_words_the_sync_checked_first() {
    let word = |code: &str, text: &str| WordKey::new(WordKind::Pinyin, code, text).unwrap();
    let mut words = Words(BTreeSet::from([word("shui'shan", "水杉")]));
    let checked = fingerprint(&words.0);
    let mut watched = Watched::new(&mut words);
    assert_eq!(watched.seen(), None);
    watched.words().unwrap();
    let change = LocalChange {
        key: word("ni'hao", "你好"),
        add: true,
        request_id: format!("dsync-{:0>32}", 1),
    };
    assert_eq!(
        watched.apply(&[change]).unwrap(),
        vec![LocalOutcome::Applied]
    );
    // 同步自己写进来的词不改记下的指纹：那是核对过的样子。
    assert_eq!(watched.words().unwrap().len(), 2);
    assert_eq!(watched.seen(), Some(checked));
}

#[test]
fn the_fingerprint_depends_only_on_which_words_there_are() {
    let word = |code: &str, text: &str| WordKey::new(WordKind::Pinyin, code, text).unwrap();
    let one = BTreeSet::from([word("shui'shan", "水杉"), word("ni'hao", "你好")]);
    let same = BTreeSet::from([word("ni'hao", "你好"), word("shui'shan", "水杉")]);
    let other = BTreeSet::from([word("shui'shan", "水杉")]);
    assert_eq!(fingerprint(&one), fingerprint(&same));
    assert_ne!(fingerprint(&one), fingerprint(&other));
}

// ---- 后台线程，宿主是假的 ----

/// 假宿主的状态，测试和后台线程共用。
#[derive(Default)]
struct Shared {
    configured: bool,
    identity: bool,
    stamp: u64,
    fingerprint: u64,
    next_check_ms: u64,
    /// 每次同步要花多久（测试叫停时正有一次在跑）。
    run_takes: Duration,
    runs: Vec<Trigger>,
    stamps: usize,
    fingerprints: usize,
}

#[derive(Clone, Default)]
struct FakeHost(Arc<Mutex<Shared>>);

impl FakeHost {
    fn with(&self, change: impl FnOnce(&mut Shared)) {
        change(&mut self.0.lock().unwrap());
    }

    fn read<T>(&self, read: impl FnOnce(&Shared) -> T) -> T {
        read(&self.0.lock().unwrap())
    }

    /// 等到条件成立，最多两秒。
    fn wait_until(&self, what: &str, condition: impl Fn(&Shared) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !self.read(&condition) {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl SchedulerHost for FakeHost {
    fn run(&mut self, trigger: Trigger) -> Option<Round> {
        // 核对在同步开头：记下的是那时的词，同步跑着的时候再改的词不算在里面。
        let (configured, identity, next_check_ms, takes, checked) = self.read(|shared| {
            (
                shared.configured,
                shared.identity,
                shared.next_check_ms,
                shared.run_takes,
                shared.fingerprint,
            )
        });
        std::thread::sleep(takes);
        self.with(|shared| shared.runs.push(trigger));
        if !configured {
            return None;
        }
        let report = if identity {
            Ok(Report::default())
        } else {
            Err(DictionarySyncError::KeyRing(
                crate::clipboard_relay::IDENTITY_MISSING.into(),
            ))
        };
        Some(Round {
            result: Ok(result(report, next_check_ms)),
            seen: identity.then_some(checked),
        })
    }

    fn fingerprint(&mut self) -> Option<u64> {
        let mut value = 0;
        self.with(|shared| {
            shared.fingerprints += 1;
            value = shared.fingerprint;
        });
        Some(value)
    }

    fn stamp(&mut self) -> Option<u64> {
        let mut value = 0;
        self.with(|shared| {
            shared.stamps += 1;
            value = shared.stamp;
        });
        Some(value)
    }
}

fn fast() -> Timing {
    Timing {
        poll: Duration::from_millis(5),
        max_delay: Duration::from_millis(40),
        idle: Duration::from_millis(15),
        retry: Duration::from_millis(15),
    }
}

#[test]
fn the_thread_does_nothing_but_ask_again_while_unconfigured_or_without_identity() {
    let host = FakeHost::default();
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    host.wait_until("it asked three times", |shared| shared.runs.len() >= 3);
    // 配置了、但设置应用还没生成设备身份：照样只是定时再问。
    host.with(|shared| shared.configured = true);
    let asked = host.read(|shared| shared.runs.len());
    host.wait_until("it asked three more times", |shared| {
        shared.runs.len() >= asked + 3
    });
    assert!(scheduler.stop(Some(Duration::from_secs(2))));
    host.read(|shared| {
        assert!(shared
            .runs
            .iter()
            .all(|trigger| *trigger == Trigger::Periodic));
        assert_eq!(shared.stamps, 0, "the dictionary file is not watched");
        assert_eq!(shared.fingerprints, 0, "my words are not read");
    });
}

#[test]
fn a_new_word_triggers_local_change_once_and_a_reweight_does_not() {
    let host = FakeHost::default();
    host.with(|shared| {
        shared.configured = true;
        shared.identity = true;
        shared.next_check_ms = 60_000;
        shared.stamp = 1;
        shared.fingerprint = 10;
    });
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    // 起来时的同步核对过的词就是基线；第一次看文件（还没有戳）算一次变化，读出的指纹与基线相同。
    host.wait_until("the first look at the file was checked", |shared| {
        shared.runs == [Trigger::Periodic] && shared.fingerprints >= 1
    });
    // 只是权重变了。
    host.with(|shared| shared.stamp = 2);
    host.wait_until("the change was checked", |shared| shared.fingerprints >= 2);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(host.read(|shared| shared.runs.clone()), [Trigger::Periodic]);
    // 加了一个词。
    host.with(|shared| {
        shared.stamp = 3;
        shared.fingerprint = 11;
    });
    host.wait_until("local_change ran", |shared| shared.runs.len() == 2);
    std::thread::sleep(Duration::from_millis(30));
    assert!(scheduler.stop(None));
    assert_eq!(
        host.read(|shared| shared.runs.clone()),
        [Trigger::Periodic, Trigger::LocalChange]
    );
}

#[test]
fn becoming_configured_starts_watching_after_one_run() {
    let host = FakeHost::default();
    host.with(|shared| {
        shared.next_check_ms = 60_000;
        shared.fingerprint = 10;
    });
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    host.wait_until("it asked once", |shared| !shared.runs.is_empty());
    // 设置应用里配好了网盘、生成了身份：下一次询问就开始，以那时的词为基线。
    host.with(|shared| {
        shared.configured = true;
        shared.identity = true;
    });
    host.wait_until("it started watching", |shared| shared.fingerprints >= 1);
    let runs = host.read(|shared| shared.runs.len());
    host.with(|shared| {
        shared.stamp = 5;
        shared.fingerprint = 12;
    });
    host.wait_until("local_change ran", |shared| {
        shared.runs.last() == Some(&Trigger::LocalChange)
    });
    assert!(scheduler.stop(Some(Duration::from_secs(2))));
    assert_eq!(host.read(|shared| shared.runs.len()), runs + 1);
}

#[test]
fn stopping_wakes_a_waiting_thread_and_lets_a_running_sync_finish() {
    // 在等下一次的线程：叫停马上醒来退出。
    let host = FakeHost::default();
    host.with(|shared| {
        shared.configured = true;
        shared.identity = true;
        shared.next_check_ms = 30 * 60 * 1000;
    });
    let timing = Timing {
        poll: Duration::from_secs(60),
        ..fast()
    };
    let scheduler = Scheduler::start_with(host.clone(), timing).unwrap();
    host.wait_until("the start-up run finished", |shared| shared.runs.len() == 1);
    std::thread::sleep(Duration::from_millis(20));
    let asked = Instant::now();
    assert!(scheduler.stop(Some(Duration::from_secs(2))));
    assert!(asked.elapsed() < Duration::from_secs(1));

    // 正在同步的线程：等不到就先返回，那一次跑完线程自己结束，之后不再开始新的一次。
    let host = FakeHost::default();
    host.with(|shared| {
        shared.configured = true;
        shared.run_takes = Duration::from_millis(300);
    });
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(!scheduler.stop(Some(Duration::from_millis(10))));
    host.wait_until("the running sync finished", |shared| shared.runs.len() == 1);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(host.read(|shared| shared.runs.len()), 1);

    // 一直等：返回时线程已经退出。
    let host = FakeHost::default();
    host.with(|shared| shared.run_takes = Duration::from_millis(100));
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert!(scheduler.stop(None));
    assert_eq!(host.read(|shared| shared.runs.len()), 1);
}

#[test]
fn a_word_added_while_a_sync_is_running_still_triggers_local_change() {
    let host = FakeHost::default();
    host.with(|shared| {
        shared.configured = true;
        shared.identity = true;
        shared.next_check_ms = 60_000;
        shared.stamp = 1;
        shared.fingerprint = 10;
        shared.run_takes = Duration::from_millis(100);
    });
    let scheduler = Scheduler::start_with(host.clone(), fast()).unwrap();
    // 起来时的同步核对完了、还没跑完的时候打字造了一个词。
    std::thread::sleep(Duration::from_millis(40));
    host.with(|shared| {
        shared.stamp = 2;
        shared.fingerprint = 11;
        shared.run_takes = Duration::ZERO;
    });
    host.wait_until("local_change ran", |shared| {
        shared.runs == [Trigger::Periodic, Trigger::LocalChange]
    });
    assert!(scheduler.stop(Some(Duration::from_secs(2))));
}

/// 第一次同步时 panic 的宿主。
struct Panicky {
    inner: FakeHost,
    panicked: bool,
}

impl SchedulerHost for Panicky {
    fn run(&mut self, trigger: Trigger) -> Option<Round> {
        if !self.panicked {
            self.panicked = true;
            panic!("synthetic host failure");
        }
        self.inner.run(trigger)
    }

    fn fingerprint(&mut self) -> Option<u64> {
        self.inner.fingerprint()
    }

    fn stamp(&mut self) -> Option<u64> {
        self.inner.stamp()
    }
}

#[test]
fn a_panicking_round_is_retried_instead_of_ending_the_thread() {
    let host = FakeHost::default();
    let panicky = Panicky {
        inner: host.clone(),
        panicked: false,
    };
    let scheduler = Scheduler::start_with(panicky, fast()).unwrap();
    host.wait_until("it ran again after the panic", |shared| {
        !shared.runs.is_empty()
    });
    assert!(scheduler.stop(Some(Duration::from_secs(2))));
}

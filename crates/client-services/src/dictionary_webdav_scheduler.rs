//! 输入法进程里的「我的词条」网盘自动同步：一个后台线程，替宿主按时调 `DictionarySync::run`（`docs/dictionary-webdav-sync.md` 的「宿主怎么接入」）。macOS 的 IMK 进程、Linux 的 IBus 和 Fcitx5 宿主在进程里起一次（C 入口 `msime_client_dictionary_sync_start`），退出时停下。
//!
//! 什么时候调：
//!
//! - `periodic`：起来时马上一次，之后按每次结果的 `next_check_ms`（限在 1 秒到 30 分钟）。要不要联网、额度、防抖和退避都由核心决定，这里不另算。
//! - `local_change`：「我的词条」变了。信号是个人词库文件（Engine 的日志库和它的 WAL）的戳：每 [`Timing::poll`] 看一次大小和修改时间，变了之后等它安静一轮（一直在变时最多等 [`Timing::max_delay`]），再读一遍「我的词条」算指纹（[`fingerprint`]），和上一次同步开头核对过的（[`Watched`]）不一样才调。打字学到的权重和选择次数也会改这个文件，指纹把它们挡在外面：只有真的加了或删了词才调 `local_change`，不会因为一直在打字就一直推迟发送的防抖。所有写入路径（打字自动造词、候选栏删词、输入法和设置应用的词库窗口、导入、MCP）改的都是这个文件，所以不必在每个写入点挂钩；漏看的变化也会被下一次同步开头的核对记下来，不会丢。
//!
//! 不做什么：宿主答「没配置」（没有 `dictionary_sync_webdav`，或者宿主选项读不出来）、或者本机还没有词库的设备身份（一个安装只有一个身份，只由设置应用在第一次配置时生成，宿主用 `DictionarySync::existing_identity_only`）时，不读词库、不联网，每 [`Timing::idle`] 重新问一次。从不在输入线程上运行；写本机词库时不请输入法让出词库（自动同步不打扰输入会话），维护权只在核心写一批时持有、从不跨网络请求。设置应用同时在同步时两边经同一个目录里的锁串行，宿主最多等 [`BACKGROUND_LOCK_WAIT`]，等不到就算这一轮在本机失败，[`Timing::retry`] 后再来。

use crate::cloud::dictionary_sync::{
    schedule, LocalChange, LocalDictionary, LocalError, LocalOutcome, Trigger, WordKey,
};
use crate::dictionary_webdav::{DictionarySyncError, RunResult};
use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 后台同步等另一个进程跑完一次同步最多等多久（[`DictionarySync::lock_wait`]）。设置应用的一次手动同步可能要几十秒，后台不必陪着等，下一轮再来。
///
/// [`DictionarySync::lock_wait`]: crate::dictionary_webdav::DictionarySync::lock_wait
pub const BACKGROUND_LOCK_WAIT: Duration = Duration::from_secs(5);

/// `next_check_ms` 的下限和上限：核心本来就给不出比 1 秒更短的间隔，上限是定时拉取的间隔，免得一个异常的值让后台睡上几天。
const MIN_INTERVAL_MS: u64 = 1_000;
const MAX_INTERVAL_MS: u64 = schedule::PULL_INTERVAL_MS;

/// 宿主一侧：每一轮重新读宿主选项和偏好，交出这一轮的同步。
pub trait SchedulerHost: Send + 'static {
    /// 跑一次同步。没配置词库同步、或者宿主选项读不出来时为 `None`，这一轮什么也不做。实现要用 `DictionarySync::existing_identity_only` 和 [`BACKGROUND_LOCK_WAIT`]，不请输入法让出词库，并把本机词库包在 [`Watched`] 里交给同步，交回它记下的指纹。
    fn run(&mut self, trigger: Trigger) -> Option<Round>;
    /// 「我的词条」现在的指纹（[`fingerprint`]）；读不出来时为 `None`。
    fn fingerprint(&mut self) -> Option<u64>;
    /// 个人词库文件现在的戳：由大小和修改时间合成，文件被写过它就变。读不出来时为 `None`。
    fn stamp(&mut self) -> Option<u64>;
}

/// 一次同步的结果。
#[derive(Debug)]
pub struct Round {
    pub result: Result<RunResult, DictionarySyncError>,
    /// 这一次同步开头核对本机改动时读到的「我的词条」的指纹（[`Watched::seen`]）；没读（还没加入共享词库）时为 `None`。
    pub seen: Option<u64>,
}

/// 「我的词条」的指纹：只看有哪些词条，同一个进程里同样的一组词得到同一个值。
pub fn fingerprint(words: &BTreeSet<WordKey>) -> u64 {
    let mut hasher = DefaultHasher::new();
    words.hash(&mut hasher);
    hasher.finish()
}

/// 包一层本机词库，记下同步第一次读到的「我的词条」的指纹：那就是这一次核对过的样子。之后才改的词让指纹对不上，下一次看文件时照样调 `local_change`；同步自己写进本机词库的词也会让指纹对不上，那一次 `local_change` 核对不出改动，不发任何操作。
pub struct Watched<'a> {
    inner: &'a mut dyn LocalDictionary,
    seen: Option<u64>,
}

impl<'a> Watched<'a> {
    pub fn new(inner: &'a mut dyn LocalDictionary) -> Self {
        Self { inner, seen: None }
    }

    pub fn seen(&self) -> Option<u64> {
        self.seen
    }
}

impl LocalDictionary for Watched<'_> {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        let words = self.inner.words()?;
        self.seen.get_or_insert_with(|| fingerprint(&words));
        Ok(words)
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        self.inner.apply(changes)
    }

    fn release(&mut self) {
        self.inner.release();
    }
}

/// 调度的几个间隔。默认值是产品用的；测试换成很短的。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// 多久看一次个人词库文件的戳。
    pub poll: Duration,
    /// 文件一直在变时，第一次变化之后最多等这么久就去读指纹。
    pub max_delay: Duration,
    /// 没配置、没有设备身份时，多久重新问一次。
    pub idle: Duration,
    /// 一次同步在本机就失败了（锁等不到、本机状态读写失败）时，多久再试。
    pub retry: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
            idle: Duration::from_secs(5 * 60),
            retry: Duration::from_secs(30),
        }
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// 调度下一步做什么。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Run(Trigger),
    Poll,
    Fingerprint,
    /// 等这么多毫秒。
    Wait(u64),
}

/// 一次同步的结果，调度只关心这几种。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ran {
    /// 没配置、宿主选项读不出来，或者本机还没有设备身份。
    Inactive,
    /// 在本机就失败了（锁、本机状态），没有结果可看。
    Failed,
    /// 跑了（网盘上的失败也算，退避由核心算进 `next_check_ms`）：下一次多久后再来。
    Done { next_check_ms: u64 },
}

impl Ran {
    fn of(result: Option<&Result<RunResult, DictionarySyncError>>) -> Self {
        match result {
            None => Self::Inactive,
            Some(Err(_)) => Self::Failed,
            Some(Ok(run)) => match &run.report {
                Err(error) if error.code() == crate::clipboard_relay::IDENTITY_MISSING => {
                    Self::Inactive
                }
                _ => Self::Done {
                    next_check_ms: run.next_check_ms,
                },
            },
        }
    }
}

/// 调度的状态，时间是起来以后的毫秒数。不碰宿主、不碰时钟，测试直接喂时间。
#[derive(Debug)]
struct Planner {
    poll: u64,
    max_delay: u64,
    idle: u64,
    retry: u64,
    /// 下一次 `periodic` 的时刻。
    due: u64,
    /// 配置好了、有设备身份：这时才看个人词库文件。
    active: bool,
    next_poll: u64,
    /// 上一次看到的文件戳。
    stamp: Option<u64>,
    /// 文件变了、还没读指纹：第一次看到变化的时刻。
    changed_since: Option<u64>,
    /// 上一次看文件时它还在变。
    changing: bool,
    /// 上一次记下的「我的词条」的指纹；还没有时为 `None`，这时变化不触发 `local_change`。
    baseline: Option<u64>,
}

impl Planner {
    fn new(timing: Timing, now: u64) -> Self {
        Self {
            poll: millis(timing.poll).max(1),
            max_delay: millis(timing.max_delay),
            idle: millis(timing.idle).max(1),
            retry: millis(timing.retry).max(1),
            due: now,
            active: false,
            next_poll: now,
            stamp: None,
            changed_since: None,
            changing: false,
            baseline: None,
        }
    }

    fn next(&self, now: u64) -> Step {
        if now >= self.due {
            return Step::Run(Trigger::Periodic);
        }
        if !self.active {
            return Step::Wait(self.due - now);
        }
        if self.fingerprint_due(now) {
            return Step::Fingerprint;
        }
        if now >= self.next_poll {
            return Step::Poll;
        }
        Step::Wait(self.due.min(self.next_poll) - now)
    }

    /// 文件变过，而且已经安静了一轮，或者从第一次变化起已经等够了 `max_delay`。
    fn fingerprint_due(&self, now: u64) -> bool {
        self.changed_since
            .is_some_and(|since| !self.changing || now >= since.saturating_add(self.max_delay))
    }

    fn polled(&mut self, now: u64, stamp: Option<u64>) {
        self.next_poll = now + self.poll;
        if stamp != self.stamp {
            self.stamp = stamp;
            self.changing = true;
            self.changed_since.get_or_insert(now);
        } else {
            self.changing = false;
        }
    }

    /// 读了指纹。和上次记下的不一样时返回真：该调 `local_change` 了，新的指纹已经记下。
    fn fingerprinted(&mut self, now: u64, stamp: Option<u64>, fingerprint: Option<u64>) -> bool {
        self.next_poll = now + self.poll;
        self.stamp = stamp;
        self.changed_since = None;
        self.changing = false;
        match (self.baseline, fingerprint) {
            (Some(old), Some(new)) if old != new => {
                self.baseline = Some(new);
                true
            }
            (None, Some(new)) => {
                self.baseline = Some(new);
                false
            }
            _ => false,
        }
    }

    /// 以这个戳和指纹为准，之前看到的变化不再算。
    fn rebase(&mut self, stamp: Option<u64>, fingerprint: Option<u64>) {
        self.stamp = stamp;
        self.baseline = fingerprint;
        self.changed_since = None;
        self.changing = false;
    }

    fn finished(&mut self, now: u64, ran: Ran) {
        match ran {
            Ran::Inactive => {
                self.active = false;
                self.stamp = None;
                self.baseline = None;
                self.changed_since = None;
                self.changing = false;
                self.due = now + self.idle;
            }
            Ran::Failed => self.due = now + self.retry,
            Ran::Done { next_check_ms } => {
                if !self.active {
                    self.next_poll = now + self.poll;
                }
                self.active = true;
                self.due = now + next_check_ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS);
            }
        }
    }
}

/// 叫停后台线程用的旗子。
#[derive(Default)]
struct Signal {
    stopped: Mutex<bool>,
    wake: Condvar,
}

impl Signal {
    fn stop(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.wake.notify_all();
    }

    fn stopped(&self) -> bool {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 等 `timeout`，或者等到被叫停；返回是否被叫停了。
    fn wait(&self, timeout: Duration) -> bool {
        let guard = self.stopped.lock().unwrap_or_else(PoisonError::into_inner);
        let (guard, _) = self
            .wake
            .wait_timeout_while(guard, timeout, |stopped| !*stopped)
            .unwrap_or_else(PoisonError::into_inner);
        *guard
    }
}

/// 跑一次同步，按结果安排下一次。
fn sync<H: SchedulerHost>(
    host: &mut H,
    planner: &mut Planner,
    trigger: Trigger,
    now: &dyn Fn() -> u64,
) {
    // 文件戳在同步之前取：同步开头的核对之后才写的文件，下一次看时就是变化。还不能同步时不看文件。
    let stamp = if planner.active { host.stamp() } else { None };
    let round = host.run(trigger);
    let ran = Ran::of(round.as_ref().map(|round| &round.result));
    planner.finished(now(), ran);
    // 以这一次核对过的词为准；没核对（还没加入共享词库）时保留原来的基线，一个基线都没有时下一次读指纹就记下它。
    if let (Ran::Done { .. }, Some(round)) = (ran, round) {
        let baseline = round.seen.or(planner.baseline);
        planner.rebase(stamp, baseline);
    }
}

fn work<H: SchedulerHost>(host: &mut H, timing: Timing, signal: &Signal) {
    let started = Instant::now();
    let now = move || millis(started.elapsed());
    let mut planner = Planner::new(timing, now());
    while !signal.stopped() {
        let step = planner.next(now());
        if let Step::Wait(wait) = step {
            if signal.wait(Duration::from_millis(wait)) {
                break;
            }
            continue;
        }
        // 宿主的实现或核心 panic 时这一轮算本机失败，过一会儿再来，不让后台线程就此停掉。
        let stepped = catch_unwind(AssertUnwindSafe(|| match step {
            Step::Poll => {
                let stamp = host.stamp();
                planner.polled(now(), stamp);
            }
            Step::Fingerprint => {
                let stamp = host.stamp();
                let fingerprint = host.fingerprint();
                if planner.fingerprinted(now(), stamp, fingerprint) {
                    sync(host, &mut planner, Trigger::LocalChange, &now);
                }
            }
            Step::Run(trigger) => sync(host, &mut planner, trigger, &now),
            Step::Wait(_) => {}
        }));
        if stepped.is_err() {
            planner.finished(now(), Ran::Failed);
        }
    }
}

/// 一个在跑的后台同步。丢掉它等于叫停但不等（[`Scheduler::stop`] 传 `Some(Duration::ZERO)`）。
pub struct Scheduler {
    signal: Arc<Signal>,
    thread: Option<JoinHandle<()>>,
    done: mpsc::Receiver<()>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Scheduler").finish_non_exhaustive()
    }
}

impl Scheduler {
    /// 起一个后台线程，按默认的间隔调度。线程起不来时返回错误。
    pub fn start<H: SchedulerHost>(host: H) -> std::io::Result<Self> {
        Self::start_with(host, Timing::default())
    }

    pub fn start_with<H: SchedulerHost>(mut host: H, timing: Timing) -> std::io::Result<Self> {
        let signal = Arc::new(Signal::default());
        let (finished, done) = mpsc::channel();
        let flag = Arc::clone(&signal);
        let thread = std::thread::Builder::new()
            .name("msime-dictionary-sync".into())
            .spawn(move || {
                work(&mut host, timing, &flag);
                drop(host);
                let _ = finished.send(());
            })?;
        Ok(Self {
            signal,
            thread: Some(thread),
            done,
        })
    }

    /// 叫停：不再开始新的一步；正在跑的那一次同步跑完为止（受每个请求的超时约束；核心每一步都写盘，进程中途退出也不丢不重）。最多等后台线程退出 `wait`，`None` 一直等到它退出；返回它是否已经退出。没等到时线程照样会在那一次跑完之后自己结束。
    pub fn stop(mut self, wait: Option<Duration>) -> bool {
        self.signal.stop();
        // 线程发完信号或者 panic 退出（发送端随之丢掉）都算退出了。
        let exited = match wait {
            None => {
                let _ = self.done.recv();
                true
            }
            Some(wait) => !matches!(
                self.done.recv_timeout(wait),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
        };
        if exited {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
        exited
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.signal.stop();
    }
}

#[cfg(test)]
#[path = "dictionary_webdav_scheduler_tests.rs"]
mod tests;

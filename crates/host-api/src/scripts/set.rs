//! 每个会话的脚本集合：按授权顺序的插件、各自延迟创建的实例、轮次和初始化预算，以及把一个生命周期事件变成脚本看到的 `Event`。

use super::process::{self, Health, Lookup, Reservation, Trip};
use super::CallTexts;
use crate::key_sound::{stamp, PackStamp};
use crate::lifecycle::{Context, InputClass, LifecycleEvent, LocalModeClass};
use msime_client_core::plugins::{kind_directory, script_grants, PluginKind, MANIFEST_FILE};
use msime_input_runtime::View;
use msime_script_runtime::{
    limits, CandidateRow, Candidates, CapabilitySet, CommitTexts, Composition, Event, Failure,
    Hook, InitFailure, Outcome, ScriptInstance, ScriptModule,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[cfg(test)]
thread_local! {
    /// 测试线程上脚本经 `log` 写出的 (级别, 文本)。产品里不设调试输出，`log` 是空操作。
    pub(crate) static TEST_SCRIPT_LOG: std::cell::RefCell<Vec<(i32, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// 设为 true 时下一次投递在宿主一侧 panic，测试 panic 总开关。
    pub(crate) static TEST_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 授权里启用的一个插件。
struct Entry {
    id: String,
    hash: String,
    /// 用户授予的能力。实例化时再和清单申请的取交集。
    granted: CapabilitySet,
    /// 包清单的戳：`Retry` 的模块在它变了之后才重新登记编译。
    manifest: PackStamp,
    health: Arc<Health>,
    /// 从进程缓存拿到的模块，拿到之后不再查缓存。
    module: Option<ScriptModule>,
    instance: Option<Live>,
    /// 这次聚焦里 trap 过（包括运行到一半被本轮配额截断）：实例已经丢弃，到下一次聚焦之前不再重建。否则一个每 50 次故意失败 2 次的插件永远不熔断，每次失败后都在按键路径上重跑一遍初始化。
    trapped: bool,
    /// 这个插件最近一次投递所在的轮次和那一轮还剩的 fuel。轮配额按插件算：同一轮里重建的实例（换了字段、属性变过）接着用剩下的，而不是重新给满。
    turn: Option<(u64, u64)>,
}

/// 一个活着的实例和它在内存总账里占的那份。
struct Live {
    instance: ScriptInstance,
    _reservation: Reservation,
}

/// 一个会话的脚本插件，挂在它的 `LifecycleState` 上，只在会话所在线程上使用。
#[derive(Default)]
pub(crate) struct ScriptSet {
    /// `<state root>/plugins`。宿主没给状态根目录时为 `None`，脚本不运行。
    root: Option<PathBuf>,
    /// `<user_data>/script-health.json`。
    health_file: Option<PathBuf>,
    /// 授权读过没有。第一次需要投递时、以及每次聚焦时按戳重读。
    loaded: bool,
    grants: PackStamp,
    plan: Vec<Entry>,
    /// 轮次：每个不带 PASSTHROUGH 的 `msime_client_key` 加一，fuel 按轮给。
    turn: u64,
    /// 本轮（或这次聚焦以来）还能花在初始化上的 fuel，所有插件合计。用完的插件推迟到下一轮再实例化，不算失败。
    init_left: u64,
    /// 字段属性确认过：宿主报告过属性（`msime_client_set_field_traits`），或者按过不带 PASSTHROUGH 的键（契约是每个键之前比较一次属性）。每次聚焦都重新要求确认，否则新字段在第一个键之前沿用上一个字段的属性，聚焦进密码框时插件照样会被实例化、收到 `focus_in`。
    traits_confirmed: bool,
    /// 上一个键或上一次聚焦变化之后宿主报告过属性：聚焦前报告的属性属于新字段，可以直接确认。
    traits_since_key: bool,
}

impl ScriptSet {
    /// 会话选择加入生命周期时记下插件根目录和健康文件的位置。
    pub(crate) fn bind(&mut self, root: Option<PathBuf>, user_data: &str) {
        self.root = root;
        self.health_file =
            (!user_data.is_empty()).then(|| Path::new(user_data).join(process::HEALTH_FILE));
        self.init_left = limits::FOCUS_INIT_FUEL;
    }

    /// 新的一轮：每个插件的轮配额和初始化预算都给满。
    pub(crate) fn next_turn(&mut self) {
        self.turn = self.turn.wrapping_add(1);
        self.init_left = limits::FOCUS_INIT_FUEL;
    }

    /// 宿主报告了字段属性（不论变没变）：属性确认过了。变得不能运行脚本（SECURE、PRIVATE、ELEVATED 或未知）时，正在跑的实例当场丢弃。
    pub(crate) fn traits_reported(&mut self, context: Context) {
        self.traits_confirmed = true;
        self.traits_since_key = true;
        if !context.ordinary_field() {
            self.drop_instances();
        }
    }

    /// 宿主按了一个不带 PASSTHROUGH 的键：按契约它在这之前已经比较过属性，现在的属性就是这个字段的。交给应用的键不算：Windows 的这类键走 Aux 管道，和 Main 管道上的属性帧没有先后保证。
    pub(crate) fn key_reported(&mut self) {
        self.traits_confirmed = true;
        self.traits_since_key = false;
    }

    /// `msime_client_focus(true)` 开始时，在任何事件之前：丢弃上一个字段留下的实例，清掉 trap 的记号，初始化预算给满，按戳重读授权。属性要重新确认，聚焦之前刚报告过的除外。
    pub(crate) fn focus_in(&mut self) {
        self.drop_instances();
        for entry in &mut self.plan {
            entry.trapped = false;
        }
        self.traits_confirmed = self.traits_since_key;
        self.traits_since_key = false;
        self.init_left = limits::FOCUS_INIT_FUEL;
        if self.root.is_some() && process::running() {
            self.refresh();
        }
    }

    /// 失焦（`focus_out` 送达之后）和 destroy：丢弃全部实例，有插件停用或恢复过时把健康文件交给后台线程写出。之后报告的属性属于下一个字段。
    pub(crate) fn release(&mut self) {
        self.traits_since_key = false;
        self.drop_instances();
        if let (Some(root), Some(file)) = (self.root.as_deref(), self.health_file.as_deref()) {
            process::flush_health(root, file);
        }
    }

    fn drop_instances(&mut self) {
        for entry in &mut self.plan {
            entry.instance = None;
        }
    }

    /// 授权文件的戳变了就整份重读，计划换掉、实例丢弃；没变时只给包已经变了的 `Retry` 模块重新登记编译。
    fn refresh(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let grants_stamp = stamp(Some(script_grants::document_path(&root)));
        let manifest = |id: &str| {
            stamp(Some(
                kind_directory(&root, PluginKind::Script)
                    .join(id)
                    .join(MANIFEST_FILE),
            ))
        };
        if self.loaded && grants_stamp == self.grants {
            for entry in self.plan.iter_mut().filter(|entry| entry.module.is_none()) {
                let current = manifest(&entry.id);
                if current != entry.manifest {
                    entry.manifest = current;
                    process::request(&root, &entry.id, &entry.hash);
                }
            }
            return;
        }
        let grants = script_grants::read(&root);
        process::adopt_health(&root, self.health_file.as_deref());
        // 只认签过名的 revision：没有签名的文件（或者文件不在）不能让熔断复位，否则同一用户下的程序换一次文件就能让一个被熔断的插件重来一遍。
        if let Some(revision) = grants.authenticated_revision() {
            process::observe_revision(&root, revision);
        }
        self.plan = grants
            .enabled()
            .map(|grant| Entry {
                id: grant.id.clone(),
                hash: grant.hash.clone(),
                granted: grant.granted,
                manifest: manifest(&grant.id),
                health: process::health(&root, &grant.id, &grant.hash),
                module: None,
                instance: None,
                trapped: false,
                turn: None,
            })
            .collect();
        for entry in &self.plan {
            process::request(&root, &entry.id, &entry.hash);
        }
        let hashes: Vec<&str> = self.plan.iter().map(|entry| entry.hash.as_str()).collect();
        process::retain(&root, &hashes);
        self.grants = grants_stamp;
        self.loaded = true;
    }

    /// 这次能不能投递：五项运行条件（选择加入已经由调用方保证），属性确认过，授权在第一次需要时读。
    fn prepare(&mut self, context: Context) -> bool {
        if self.root.is_none()
            || !self.traits_confirmed
            || !context.ordinary_field()
            || !process::running()
        {
            return false;
        }
        if !self.loaded {
            self.refresh();
        }
        !self.plan.is_empty()
    }

    /// 打字前，排在本次派发的 runtime 调用之前。这次打的字符（`read_composition`）随事件给出。
    pub(crate) fn input(
        &mut self,
        context: Context,
        class: InputClass,
        local_mode: LocalModeClass,
        input_char: Option<char>,
    ) {
        if !self.prepare(context) {
            return;
        }
        let mut buffer = [0; 4];
        let mut event = Event::new(Hook::Input);
        event.scalars.input_class = Some(class as u8);
        event.scalars.local_mode = Some(local_mode as u8);
        event.composition.input_char = match input_char {
            Some(c) => Some(&*c.encode_utf8(&mut buffer)),
            None => None,
        };
        self.deliver_all(&event);
    }

    /// 把生命周期核心推导出的一个事件交给各插件。不投递给脚本的事件（按键、字段属性、激活、会话结束、`before_commit`）在这里就返回。
    pub(crate) fn deliver(
        &mut self,
        event: &LifecycleEvent,
        context: Context,
        texts: &CallTexts<'_>,
    ) {
        let Some(hook) = hook(event) else {
            return;
        };
        if !self.prepare(context) {
            return;
        }
        let view_mode = texts.view.map(|view| mode(&view.local_mode));
        let commit_mode = texts.commit_local_mode.map(mode).or(view_mode);
        let mut rows = [CandidateRow::default(); limits::MAX_CANDIDATE_ROWS];
        let mut delivered = Event::new(hook);
        let scalars = &mut delivered.scalars;
        scalars.local_mode = view_mode;
        match *event {
            LifecycleEvent::SelectionRetracted(retraction) => {
                scalars.retraction = Some(retraction as u8);
            }
            LifecycleEvent::CandidateSelected(selection) => {
                scalars.how = Some(selection.how as u8);
                scalars.outcome = Some(selection.outcome as u8);
                scalars.local_mode = commit_mode;
                delivered.commit = CommitTexts {
                    selected: texts.selected,
                    seat: Some(selection.seat),
                    ..CommitTexts::default()
                };
            }
            LifecycleEvent::Commit {
                origin,
                continues,
                generated,
                ..
            } => {
                scalars.origin = Some(origin as u8);
                scalars.continues = Some(continues);
                scalars.generated = Some(generated);
                scalars.local_mode = commit_mode;
                delivered.commit = CommitTexts {
                    commit: texts.commit,
                    body: texts
                        .commit
                        .map(|commit| texts.facts.split_commit(commit).0),
                    ..CommitTexts::default()
                };
            }
            LifecycleEvent::CompositionEnd(reason) | LifecycleEvent::CandidatesHidden(reason) => {
                scalars.reason = Some(reason as u8);
            }
            LifecycleEvent::CompositionStart | LifecycleEvent::CompositionUpdate => {
                if let Some(view) = texts.view {
                    delivered.composition = composition(view);
                }
            }
            LifecycleEvent::CandidatesShown {
                page, highlighted, ..
            } => {
                if let Some(view) = texts.view {
                    let shown = view.candidates.len().min(rows.len());
                    for (row, candidate) in rows.iter_mut().zip(&view.candidates) {
                        *row = CandidateRow {
                            text: &candidate.text,
                            code: &candidate.code,
                            annotation: &candidate.annotation,
                            source: candidate.source,
                        };
                    }
                    delivered.candidates = Some(Candidates {
                        rows: &rows[..shown],
                        page: Some(u32::from(page)),
                        highlighted: highlighted.map(u32::from),
                    });
                }
            }
            _ => {}
        }
        self.deliver_all(&delivered);
    }

    /// 按授权顺序交给每个插件。一个插件的失败只算它自己的；宿主一侧任何 panic 都关掉本进程的全部脚本，调用照常返回。
    fn deliver_all(&mut self, event: &Event<'_>) {
        let turn = self.turn;
        let mut panicked = false;
        for entry in &mut self.plan {
            let init_left = &mut self.init_left;
            if catch_unwind(AssertUnwindSafe(|| {
                deliver_one(entry, turn, init_left, event);
            }))
            .is_err()
            {
                panicked = true;
                break;
            }
        }
        if panicked {
            process::panic_caught();
            self.drop_instances();
        }
    }
}

/// 一个插件收一个事件：查缓存拿模块、需要时实例化、投递、记账。
fn deliver_one(entry: &mut Entry, turn: u64, init_left: &mut u64, event: &Event<'_>) {
    #[cfg(test)]
    if TEST_PANIC.with(|panic| panic.replace(false)) {
        panic!("injected host-side panic");
    }
    if entry.health.disabled() {
        entry.instance = None;
        return;
    }
    if entry.module.is_none() {
        match process::lookup(&entry.hash) {
            Lookup::Ready(module) => entry.module = Some(module),
            Lookup::Waiting => return,
            Lookup::Invalid(reason) => {
                entry.health.disable(Trip::Invalid, reason);
                return;
            }
        }
    }
    let Some(module) = entry.module.as_ref() else {
        return;
    };
    // 只在声明过的钩子上实例化和投递。
    if !module.declaration().hooks.contains(event.hook) {
        return;
    }
    // 这个插件本轮的配额已经用完：跳过到下一轮，也不为它在按键路径上重新初始化。
    if entry.turn == Some((turn, 0)) {
        return;
    }
    if entry.instance.is_none() {
        let budget = *init_left;
        if budget == 0 || entry.trapped {
            return;
        }
        let Some(reservation) = process::reserve(module.reserved_memory_bytes()) else {
            return;
        };
        match ScriptInstance::new(module, entry.granted, budget) {
            Ok(mut instance) => {
                *init_left = budget.saturating_sub(instance.init_fuel());
                if let Some((last, left)) = entry.turn.filter(|(last, _)| *last == turn) {
                    instance.resume_turn(last, left);
                }
                #[cfg(test)]
                let instance = {
                    let mut instance = instance;
                    instance.set_log_sink(Some(Box::new(|level, text: &str| {
                        TEST_SCRIPT_LOG.with(|log| log.borrow_mut().push((level, text.to_owned())));
                    })));
                    instance
                };
                entry.instance = Some(Live {
                    instance,
                    _reservation: reservation,
                });
            }
            Err(InitFailure { failure, fuel }) => {
                *init_left = budget.saturating_sub(fuel);
                // 被本轮剩下的初始化预算卡住，而不是插件自己的上限：推迟到下一轮，不算失败。
                if failure == Failure::OutOfFuel && budget < limits::INIT_FUEL {
                    return;
                }
                entry.health.disable(Trip::Init, failure.as_str());
                return;
            }
        }
    }
    let Some(live) = entry.instance.as_mut() else {
        return;
    };
    let started = Instant::now();
    let outcome = live.instance.deliver(turn, event);
    let slow = process::slow(started);
    if !matches!(outcome, Outcome::NotSubscribed) {
        entry.turn = Some((turn, live.instance.turn_fuel_left()));
    }
    match outcome {
        Outcome::NotSubscribed => {}
        Outcome::Delivered { .. } => entry.health.record(None, slow),
        // 本轮配额用完不算失败（宿主不在每次派发前都开一轮）；运行过的照样按墙钟记一次。
        Outcome::TurnExhausted { fuel } => {
            if fuel > 0 {
                entry.health.record(None, slow);
            }
            // 运行到一半被截断：wasm 的状态可能停在一半，丢掉；这次聚焦里不再重建，否则一个每轮都花光配额的插件会在每一轮的按键路径上重跑一遍初始化。
            if !live.instance.is_usable() {
                entry.trapped = true;
                entry.instance = None;
            }
        }
        Outcome::Failed { failure, .. } => {
            entry.health.record(Some(failure), slow);
            // trap 之后 wasm 的状态可能停在一半，丢掉；这次聚焦里不再重建。
            if !live.instance.is_usable() {
                entry.trapped = true;
                entry.instance = None;
            } else if entry.health.disabled() {
                entry.instance = None;
            }
        }
    }
}

/// 生命周期事件对应的钩子；不投递给脚本的事件为 `None`。
fn hook(event: &LifecycleEvent) -> Option<Hook> {
    Some(match event {
        LifecycleEvent::FocusIn => Hook::FocusIn,
        LifecycleEvent::FocusOut => Hook::FocusOut,
        LifecycleEvent::Input { .. } => Hook::Input,
        LifecycleEvent::SelectionRetracted(_) => Hook::SelectionRetracted,
        LifecycleEvent::CandidateSelected(_) => Hook::CandidateSelected,
        LifecycleEvent::Commit { .. } => Hook::Commit,
        LifecycleEvent::CompositionEnd(_) => Hook::CompositionEnd,
        LifecycleEvent::CompositionStart => Hook::CompositionStart,
        LifecycleEvent::CompositionUpdate => Hook::CompositionUpdate,
        LifecycleEvent::CandidatesShown { .. } => Hook::CandidatesShown,
        LifecycleEvent::CandidatesHidden(_) => Hook::CandidatesHidden,
        // 修改类钩子第 4 阶段才投递；其余只给核心。
        LifecycleEvent::BeforeCommit { .. }
        | LifecycleEvent::TraitsChanged { .. }
        | LifecycleEvent::Activated
        | LifecycleEvent::Deactivated
        | LifecycleEvent::KeyDown(_)
        | LifecycleEvent::SessionEnd => return None,
    })
}

fn mode(name: &str) -> u8 {
    LocalModeClass::of(name) as u8
}

/// 组字类事件的文本：预编辑串、读音和光标（编辑串里光标前的字符数）。
fn composition(view: &View) -> Composition<'_> {
    Composition {
        preedit: Some(&view.preedit),
        reading: Some(&view.reading),
        input_char: None,
        caret: view
            .editing_text
            .get(..view.caret_position)
            .map(|typed| u32::try_from(typed.chars().count()).unwrap_or(u32::MAX)),
    }
}

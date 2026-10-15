//! `msime-pack run`：按夹具里的事件运行一个脚本插件，每个事件输出一行 JSON。
//!
//! 读包、校验和编译走的是导入和输入法进程用的同一份代码（`plugins::load_script_source`、`msime_script_runtime::compile`），执行走的是同一个 `ScriptInstance`：按授权和本地模式复制文本、授权与清单取交集、`before_commit` 和 `command` 在快照上运行、每轮和每个钩子的 fuel 配额、修改类 import 的返回码，全都不是这里重写的。这里只做输入法进程里 `scripts::ScriptSet` 对一个插件做的那几件事：第一次需要投递时实例化（受本轮初始化预算约束），每次聚焦在任何事件之前、失焦在 `focus_out` 之后丢弃实例，trap 之后这次聚焦里不再重建，本轮配额用完时跳过、运行到一半被截断时同样不再重建，这一次用掉的不到本钩子上限的一半时不算失败（达到一半时 runtime 报 fuel 耗尽，和其余失败一样），同一轮里重建的实例接着用这一轮剩下的三份配额，初始化失败之后停用。
//!
//! 和输入法进程不同的地方（夹具表达不了，或者故意更严）：字段永远是普通字段（属性已确认、不是 SECURE、PRIVATE 或 ELEVATED）；不做按次数和按墙钟的熔断，每次失败都报告；进程内存总账不检查，一个插件总放得下。

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use msime_client_services::plugins::{self, script_pack, ReservedIds};
use msime_script_runtime::abi::{scalar_values as values, status};
use msime_script_runtime::{
    limits, CandidateRow, Candidates, Capability, CapabilitySet, CommandEvent, CommitTexts,
    Composition, DebugSinks, Event, Failure, Hook, InitFailure, Outcome, Scalars, ScriptInstance,
    ScriptModule,
};
use serde::{Deserialize, Serialize};

use crate::{reason, EXIT_INVALID, EXIT_OK, EXIT_USAGE};

/// `--grant` 的取值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantSpec {
    /// 不写 `--grant`：什么都不授予，和刚导入、还没授权的包一样。
    Nothing,
    /// `--grant all`：清单申请的全部能力。
    All,
    /// `--grant read_commit,rewrite_commit`：逐项授予。
    Only(Vec<String>),
}

impl GrantSpec {
    pub fn parse(text: &str) -> Self {
        if text == "all" {
            Self::All
        } else {
            Self::Only(
                text.split(',')
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .collect(),
            )
        }
    }

    /// 按授权页的规则算出实际授予的能力：必须是清单 `permissions` 的子集，并满足依赖和互斥规则（同 `script_grants::set`）。
    pub fn resolve(&self, declared: CapabilitySet) -> Result<CapabilitySet, String> {
        let names = match self {
            Self::Nothing => return Ok(CapabilitySet::EMPTY),
            Self::All => return Ok(declared),
            Self::Only(names) => names,
        };
        let mut granted = CapabilitySet::EMPTY;
        for name in names {
            let capability =
                Capability::parse(name).ok_or_else(|| format!("--grant 里有未知的能力 {name}"))?;
            if !declared.contains(capability) {
                return Err(format!("清单没有申请 {name}"));
            }
            granted.insert(capability);
        }
        granted.check_rules().map_err(script_pack::rule_message)?;
        Ok(granted)
    }
}

/// 夹具里的一行。字段名就是 ABI 里的名字；每种事件只接受输入法进程会给它的字段，多写的字段按错误处理，免得夹具测到产品里不存在的情形。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    event: String,
    /// 这个事件之前宿主按了一个键（`msime_client_key`，不带 PASSTHROUGH）：开始新的一轮，fuel 配额和初始化预算给满。
    #[serde(default)]
    key: bool,
    /// 本地模式名，同 `commit_context.local_mode`。不写是 `none`。
    local_mode: Option<String>,
    origin: Option<String>,
    how: Option<String>,
    outcome: Option<String>,
    reason: Option<String>,
    input_class: Option<String>,
    retraction: Option<String>,
    rewritable: Option<bool>,
    continues: Option<bool>,
    generated: Option<bool>,
    preedit: Option<String>,
    reading: Option<String>,
    input_char: Option<String>,
    caret: Option<u32>,
    selected: Option<String>,
    seat: Option<u32>,
    /// 上屏文本里可改写的那一段。上屏文本是 `body` 后面接上 `tail`。
    body: Option<String>,
    /// 标点、顶字的尾巴，只读。
    tail: Option<String>,
    candidates: Option<Vec<Row>>,
    page: Option<u32>,
    highlighted: Option<u32>,
    /// `command`：用户打的触发词，必须是清单 `[[script.commands]]` 里的某一个。
    trigger: Option<String>,
    /// `command`：触发词后面的字母，不写是空串。
    argument: Option<String>,
    /// `command`：这一次的 64 字节随机数，128 位十六进制；不写是全 0。
    random: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    text: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    annotation: String,
    #[serde(default)]
    source: u8,
}

/// 夹具里的一个事件，已经核对过字段。
#[derive(Debug)]
pub struct Step {
    hook: Hook,
    key: bool,
    scalars: Scalars,
    preedit: Option<String>,
    reading: Option<String>,
    input_char: Option<String>,
    caret: Option<u32>,
    selected: Option<String>,
    seat: Option<u32>,
    /// 完整的上屏文本和其中可改写的那一段。
    commit: Option<(String, String)>,
    candidates: Option<(Vec<Row>, Option<u32>, Option<u32>)>,
    /// `command` 的触发词、参数和随机数。
    command: Option<(String, String, [u8; limits::RANDOM_BYTES_PER_EVENT])>,
}

impl Step {
    /// `command` 事件的触发词。
    pub fn trigger(&self) -> Option<&str> {
        self.command.as_ref().map(|(trigger, ..)| trigger.as_str())
    }

    /// 借用这一步的文本构造投递给插件的事件。`rows` 是调用方准备的候选行。
    fn event<'a>(&'a self, rows: &'a [CandidateRow<'a>]) -> Event<'a> {
        let mut event = Event::new(self.hook);
        event.scalars = self.scalars;
        event.composition = Composition {
            preedit: self.preedit.as_deref(),
            reading: self.reading.as_deref(),
            input_char: self.input_char.as_deref(),
            caret: self.caret,
        };
        event.commit = CommitTexts {
            selected: self.selected.as_deref(),
            commit: self.commit.as_ref().map(|(commit, _)| commit.as_str()),
            body: self.commit.as_ref().map(|(_, body)| body.as_str()),
            seat: self.seat,
            // 夹具里只有一个插件，body 就是原文。
            original_body_utf16: None,
        };
        event.candidates = self
            .candidates
            .as_ref()
            .map(|(_, page, highlighted)| Candidates {
                rows,
                page: *page,
                highlighted: *highlighted,
            });
        event.command = self
            .command
            .as_ref()
            .map(|(trigger, argument, random)| CommandEvent {
                trigger,
                argument,
                random: *random,
            });
        event
    }

    fn rows(&self) -> Vec<CandidateRow<'_>> {
        self.candidates
            .iter()
            .flat_map(|(rows, _, _)| rows)
            .map(|row| CandidateRow {
                text: &row.text,
                code: &row.code,
                annotation: &row.annotation,
                source: row.source,
            })
            .collect()
    }
}

/// 每种事件可以写的字段（`event`、`key`、`local_mode` 之外），以及哪些必须写。按输入法进程给各事件填的字段定，见 host-api 的 `scripts::set`。
fn fields(hook: Hook) -> (&'static [&'static str], &'static [&'static str]) {
    match hook {
        Hook::FocusIn | Hook::FocusOut => (&[], &[]),
        Hook::Input => (&["input_class", "input_char"], &["input_class"]),
        Hook::SelectionRetracted => (&["retraction"], &["retraction"]),
        Hook::CandidateSelected => (
            &["how", "outcome", "seat", "selected"],
            &["how", "outcome", "seat"],
        ),
        Hook::BeforeCommit => (
            &["origin", "rewritable", "body", "tail"],
            &["origin", "body"],
        ),
        Hook::Commit => (
            &["origin", "continues", "generated", "body", "tail"],
            &["origin", "body"],
        ),
        Hook::CompositionEnd | Hook::CandidatesHidden => (&["reason"], &["reason"]),
        Hook::CompositionStart | Hook::CompositionUpdate => (&["preedit", "reading", "caret"], &[]),
        Hook::CandidatesShown | Hook::Candidates => (&["candidates", "page", "highlighted"], &[]),
        Hook::Command => (&["trigger", "argument", "random"], &["trigger"]),
    }
}

/// `random` 字段：恰好 [`limits::RANDOM_BYTES_PER_EVENT`] 字节的十六进制，不写是全 0。
fn random_bytes(text: Option<&str>) -> Result<[u8; limits::RANDOM_BYTES_PER_EVENT], String> {
    let mut bytes = [0; limits::RANDOM_BYTES_PER_EVENT];
    let Some(text) = text else {
        return Ok(bytes);
    };
    let message = || {
        format!(
            "random 必须是 {} 位十六进制",
            limits::RANDOM_BYTES_PER_EVENT * 2
        )
    };
    if text.len() != bytes.len() * 2 || !text.is_ascii() {
        return Err(message());
    }
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| message())?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| message())?;
    }
    Ok(bytes)
}

fn scalar(
    table: &[(&'static str, u8)],
    field: &str,
    name: Option<&str>,
) -> Result<Option<u8>, String> {
    let Some(name) = name else {
        return Ok(None);
    };
    values::value(table, name).map(Some).ok_or_else(|| {
        let names: Vec<&str> = table.iter().map(|(name, _)| *name).collect();
        format!("{field} 不认识 {name}，只能是 {}", names.join("、"))
    })
}

/// 解析并核对夹具的一行。
fn parse_line(text: &str) -> Result<Step, String> {
    let line: Line =
        serde_json::from_str(text).map_err(|error| format!("不是有效的事件：{error}"))?;
    let hook = Hook::parse(&line.event).ok_or_else(|| format!("没有 {} 这个事件", line.event))?;
    let present = [
        ("origin", line.origin.is_some()),
        ("how", line.how.is_some()),
        ("outcome", line.outcome.is_some()),
        ("reason", line.reason.is_some()),
        ("input_class", line.input_class.is_some()),
        ("retraction", line.retraction.is_some()),
        ("rewritable", line.rewritable.is_some()),
        ("continues", line.continues.is_some()),
        ("generated", line.generated.is_some()),
        ("preedit", line.preedit.is_some()),
        ("reading", line.reading.is_some()),
        ("input_char", line.input_char.is_some()),
        ("caret", line.caret.is_some()),
        ("selected", line.selected.is_some()),
        ("seat", line.seat.is_some()),
        ("body", line.body.is_some()),
        ("tail", line.tail.is_some()),
        ("candidates", line.candidates.is_some()),
        ("page", line.page.is_some()),
        ("highlighted", line.highlighted.is_some()),
        ("trigger", line.trigger.is_some()),
        ("argument", line.argument.is_some()),
        ("random", line.random.is_some()),
    ];
    let (allowed, required) = fields(hook);
    for (field, is_present) in present {
        if is_present && !allowed.contains(&field) {
            return Err(format!("{} 事件没有 {field} 字段", hook.as_str()));
        }
        if !is_present && required.contains(&field) {
            return Err(format!("{} 事件必须写 {field}", hook.as_str()));
        }
    }
    if let Some(input_char) = &line.input_char {
        if input_char.chars().count() != 1 {
            return Err("input_char 必须正好是一个字符".into());
        }
    }

    let command_mode = values::value(values::LOCAL_MODE, "command").unwrap_or_default();
    let local_mode = scalar(values::LOCAL_MODE, "local_mode", line.local_mode.as_deref())?;
    // 输入法只在 `/` 指令里投递 `command`。
    let local_mode = if hook == Hook::Command {
        if local_mode.is_some_and(|mode| mode != command_mode) {
            return Err("command 事件的 local_mode 只能是 command".into());
        }
        command_mode
    } else {
        local_mode.unwrap_or(0)
    };
    // 输入法只投递清单里的触发词（2 到 16 个小写字母）和只含小写字母的参数，夹具不测产品里到不了的情形。
    let command = match &line.trigger {
        Some(trigger) => {
            let argument = line.argument.clone().unwrap_or_default();
            if !(2..=limits::MAX_COMMAND_TRIGGER_BYTES).contains(&trigger.len())
                || !trigger.bytes().all(|byte| byte.is_ascii_lowercase())
            {
                return Err(format!(
                    "trigger 必须是 2 到 {} 个小写字母",
                    limits::MAX_COMMAND_TRIGGER_BYTES
                ));
            }
            if argument.len() > limits::MAX_COMMAND_ARGUMENT_BYTES
                || !argument.bytes().all(|byte| byte.is_ascii_lowercase())
            {
                return Err(format!(
                    "argument 只能是至多 {} 个小写字母",
                    limits::MAX_COMMAND_ARGUMENT_BYTES
                ));
            }
            Some((
                trigger.clone(),
                argument,
                random_bytes(line.random.as_deref())?,
            ))
        }
        None => None,
    };
    let origin = scalar(values::ORIGIN, "origin", line.origin.as_deref())?;
    let mut scalars = Scalars {
        origin,
        how: scalar(values::HOW, "how", line.how.as_deref())?,
        outcome: scalar(values::OUTCOME, "outcome", line.outcome.as_deref())?,
        reason: scalar(values::REASON, "reason", line.reason.as_deref())?,
        local_mode: Some(local_mode),
        input_class: scalar(
            values::INPUT_CLASS,
            "input_class",
            line.input_class.as_deref(),
        )?,
        retraction: scalar(values::RETRACTION, "retraction", line.retraction.as_deref())?,
        rewritable: line.rewritable,
        continues: line.continues,
        generated: line.generated,
    };
    match hook {
        Hook::Commit => {
            scalars.continues = Some(line.continues.unwrap_or(false));
            scalars.generated = Some(line.generated.unwrap_or(false));
        }
        // 输入法进程按来源、本地模式、选法、字段属性和宿主的位算可改写；夹具能表达的只有前两项，其余按可改写处理，需要时显式写 `rewritable`。
        Hook::BeforeCommit if line.rewritable.is_none() => {
            let rewritable_origin = ["selection", "raw", "held_flush"]
                .iter()
                .any(|name| values::value(values::ORIGIN, name) == origin);
            scalars.rewritable = Some(rewritable_origin && local_mode == 0);
        }
        _ => {}
    }
    let composing = matches!(hook, Hook::CompositionStart | Hook::CompositionUpdate);
    let commit = line.body.map(|body| {
        let commit = format!("{body}{}", line.tail.as_deref().unwrap_or(""));
        (commit, body)
    });
    let candidates = matches!(hook, Hook::CandidatesShown | Hook::Candidates).then(|| {
        (
            line.candidates.unwrap_or_default(),
            Some(line.page.unwrap_or(0)),
            line.highlighted,
        )
    });
    Ok(Step {
        hook,
        key: line.key,
        scalars,
        // 组字类事件里预编辑串和读音总是在的，可能为空。
        preedit: line.preedit.or_else(|| composing.then(String::new)),
        reading: line.reading.or_else(|| composing.then(String::new)),
        input_char: line.input_char,
        caret: line.caret,
        selected: line.selected,
        seat: line.seat,
        commit,
        candidates,
        command,
    })
}

/// 解析整份夹具：一行一个 JSON 对象，空行跳过。错误带行号。
pub fn parse_fixture(text: &str) -> Result<Vec<Step>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            parse_line(line).map_err(|error| format!("第 {} 行：{error}", index + 1))
        })
        .collect()
}

#[derive(Serialize)]
struct LogLine {
    level: String,
    text: String,
}

/// `command` 这一次交回的一行：文本和插件注释。
type CommandRow = (String, String);

#[derive(Serialize)]
struct ActionLine {
    import: &'static str,
    result: String,
}

/// 输出的一行。
#[derive(Serialize)]
struct Report {
    seq: usize,
    event: &'static str,
    status: String,
    fuel: u64,
    /// 这个事件之前尝试实例化时初始化用掉的 fuel（`msime_abi` 加 `msime_init`）。`init:*` 和 `deferred` 行上也有，是失败或被本轮预算截断的那次尝试用掉的；没有尝试实例化时不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    init_fuel: Option<u64>,
    actions: Vec<ActionLine>,
    log: Vec<LogLine>,
    /// 只有 `command` 行有：插件交回的行，每行是 `[文本, 注释]`。没有正常返回时为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    rows: Option<Vec<CommandRow>>,
}

fn level_name(level: i32) -> String {
    match level {
        0 => "debug".into(),
        1 => "info".into(),
        2 => "warn".into(),
        3 => "error".into(),
        other => other.to_string(),
    }
}

fn status_name(code: i32) -> String {
    match code {
        status::DENIED => "denied".into(),
        status::ABSENT => "absent".into(),
        status::INVALID => "invalid".into(),
        status::LIMIT => "limit".into(),
        code if code >= 0 => "ok".into(),
        other => other.to_string(),
    }
}

/// 一个插件在一个会话里的状态，规则同 host-api 的 `scripts::set` 对一个条目做的事。
struct Session<'m> {
    module: &'m ScriptModule,
    grants: CapabilitySet,
    instance: Option<ScriptInstance>,
    /// 这次聚焦里 trap 过（包括运行到一半被本轮配额截断）：到下一次聚焦之前不再重建。
    trapped: bool,
    /// 初始化失败过：输入法进程会立即停用它。
    disabled: bool,
    turn: u64,
    init_left: u64,
    /// 插件最近一次投递所在的轮次，和那一轮其余钩子、`before_commit`、`command` 各还剩的 fuel，同一轮里重建的实例接着用。
    plugin_turn: Option<(u64, u64, u64, u64)>,
    logs: Arc<Mutex<Vec<LogLine>>>,
    actions: Arc<Mutex<Vec<ActionLine>>>,
    /// 最近一次 `command` 正常返回时交回的行。
    rows: Vec<CommandRow>,
}

impl<'m> Session<'m> {
    fn new(module: &'m ScriptModule, grants: CapabilitySet) -> Self {
        Self {
            module,
            grants,
            instance: None,
            trapped: false,
            disabled: false,
            turn: 0,
            init_left: limits::FOCUS_INIT_FUEL,
            plugin_turn: None,
            logs: Arc::default(),
            actions: Arc::default(),
            rows: Vec::new(),
        }
    }

    fn sinks(&self) -> DebugSinks {
        let logs = Arc::clone(&self.logs);
        let actions = Arc::clone(&self.actions);
        DebugSinks {
            log: Some(Box::new(move |level, text| {
                logs.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(LogLine {
                        level: level_name(level),
                        text: text.to_owned(),
                    });
            })),
            actions: Some(Box::new(move |import, code| {
                actions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(ActionLine {
                        import,
                        result: status_name(code),
                    });
            })),
        }
    }

    /// 处理一步，返回这一行的状态、用掉的 fuel、实例化用掉的 fuel，以及是否算一次失败。
    fn step(&mut self, step: &Step) -> (String, u64, Option<u64>, bool) {
        if step.key {
            self.turn = self.turn.wrapping_add(1);
            self.init_left = limits::FOCUS_INIT_FUEL;
        }
        if step.hook == Hook::FocusIn {
            self.instance = None;
            self.trapped = false;
            self.init_left = limits::FOCUS_INIT_FUEL;
        }
        let result = self.deliver(step);
        if step.hook == Hook::FocusOut {
            self.instance = None;
        }
        result
    }

    fn deliver(&mut self, step: &Step) -> (String, u64, Option<u64>, bool) {
        if self.disabled {
            return ("disabled".into(), 0, None, false);
        }
        if !self.module.declaration().hooks.contains(step.hook) {
            return ("not_subscribed".into(), 0, None, false);
        }
        // 本轮里这个钩子所用的那份配额已经用完：跳过到下一轮，也不为它重新初始化。
        if let Some((turn, left, before_commit_left, command_left)) = self.plugin_turn {
            let left = match step.hook {
                Hook::BeforeCommit => before_commit_left,
                Hook::Command => command_left,
                _ => left,
            };
            if turn == self.turn && left == 0 {
                return ("turn_exhausted".into(), 0, None, false);
            }
        }
        let mut init_fuel = None;
        if self.instance.is_none() {
            if self.trapped {
                return ("suspended".into(), 0, None, false);
            }
            let budget = self.init_left;
            if budget == 0 {
                return ("deferred".into(), 0, None, false);
            }
            match ScriptInstance::with_debug(self.module, self.grants, budget, self.sinks()) {
                Ok(mut instance) => {
                    self.init_left = budget.saturating_sub(instance.init_fuel());
                    init_fuel = Some(instance.init_fuel());
                    if let Some((turn, left, before_commit_left, command_left)) =
                        self.plugin_turn.filter(|(turn, ..)| *turn == self.turn)
                    {
                        instance.resume_turn(turn, left, before_commit_left, command_left);
                    }
                    self.instance = Some(instance);
                }
                Err(InitFailure { failure, fuel }) => {
                    self.init_left = budget.saturating_sub(fuel);
                    // 被本轮剩下的初始化预算卡住，而不是插件自己的上限：推迟到下一轮，不算失败。
                    if failure == Failure::OutOfFuel && budget < limits::INIT_FUEL {
                        return ("deferred".into(), 0, Some(fuel), false);
                    }
                    self.disabled = true;
                    return (format!("init:{}", failure.as_str()), 0, Some(fuel), true);
                }
            }
        }
        let Some(instance) = self.instance.as_mut() else {
            return ("deferred".into(), 0, init_fuel, false);
        };
        let rows = step.rows();
        let outcome = instance.deliver(self.turn, &step.event(&rows));
        if !matches!(outcome, Outcome::NotSubscribed) {
            self.plugin_turn = Some((
                self.turn,
                instance.turn_fuel_left(),
                instance.before_commit_fuel_left(),
                instance.command_fuel_left(),
            ));
        }
        match outcome {
            Outcome::NotSubscribed => ("not_subscribed".into(), 0, init_fuel, false),
            Outcome::Delivered { fuel, .. } => {
                self.rows.clone_from(&instance.actions().rows);
                ("ok".into(), fuel, init_fuel, false)
            }
            // 本轮配额用完不算失败；运行到一半被截断的实例丢弃，这次聚焦里不再重建。被截断时已经用掉本钩子上限一半的是 `Failed`，走下面的分支。
            Outcome::TurnExhausted { fuel } => {
                if !instance.is_usable() {
                    self.trapped = true;
                    self.instance = None;
                }
                ("turn_exhausted".into(), fuel, init_fuel, false)
            }
            Outcome::Failed { failure, fuel } => {
                if !instance.is_usable() {
                    self.trapped = true;
                    self.instance = None;
                }
                (failure.as_str().into(), fuel, init_fuel, true)
            }
        }
    }

    fn drain(&self) -> (Vec<ActionLine>, Vec<LogLine>) {
        let actions = std::mem::take(
            &mut *self
                .actions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let logs = std::mem::take(
            &mut *self
                .logs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        (actions, logs)
    }
}

/// 按 `steps` 运行编译好的模块，每个事件写一行 JSON 到 `out`。返回退出码：有任何失败（trap、fuel 耗尽、返回负数、初始化失败）为 1，否则为 0。
pub fn run_module(
    module: &ScriptModule,
    grants: CapabilitySet,
    steps: &[Step],
    out: &mut impl Write,
) -> i32 {
    let mut session = Session::new(module, grants);
    let mut status = EXIT_OK;
    for (index, step) in steps.iter().enumerate() {
        let (outcome, fuel, init_fuel, failed) = session.step(step);
        if failed {
            status = EXIT_INVALID;
        }
        let (actions, log) = session.drain();
        let rows = std::mem::take(&mut session.rows);
        let report = Report {
            seq: index + 1,
            event: step.hook.as_str(),
            status: outcome,
            fuel,
            init_fuel,
            actions,
            log,
            rows: (step.hook == Hook::Command).then_some(rows),
        };
        let line = serde_json::to_string(&report).unwrap_or_default();
        if writeln!(out, "{line}").is_err() {
            return EXIT_INVALID;
        }
    }
    status
}

/// `msime-pack run <pack> --events <fixture> [--grant ...] [--builtin]` 的实现。`reserved` 是 `--builtin` 给的保留 id 处理方式。
pub fn command(
    pack: &Path,
    fixture: &Path,
    grant: &GrantSpec,
    reserved: ReservedIds,
    out: &mut impl Write,
    err: &mut impl Write,
) -> i32 {
    let loaded = match plugins::load_script_source(pack, reserved) {
        Ok(loaded) => loaded,
        Err(error) => {
            let _ = writeln!(err, "error {}: {}", pack.display(), reason(&error));
            return EXIT_INVALID;
        }
    };
    let module = match msime_script_runtime::compile(&loaded.wasm, loaded.pack.declaration()) {
        Ok(module) => module,
        Err(error) => {
            let _ = writeln!(
                err,
                "error {}: plugin_invalid: {} 不是有效的脚本模块：{error}",
                pack.display(),
                loaded.pack.module
            );
            return EXIT_INVALID;
        }
    };
    let grants = match grant.resolve(loaded.pack.permissions) {
        Ok(grants) => grants,
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return EXIT_USAGE;
        }
    };
    let text = match std::fs::read_to_string(fixture) {
        Ok(text) => text,
        Err(error) => {
            let _ = writeln!(err, "error {}: 读不出夹具：{error}", fixture.display());
            return EXIT_USAGE;
        }
    };
    let steps = match parse_fixture(&text) {
        Ok(steps) => steps,
        Err(message) => {
            let _ = writeln!(err, "error {}: {message}", fixture.display());
            return EXIT_USAGE;
        }
    };
    // 输入法只把清单里声明过的触发词投递给这个插件。
    for (index, step) in steps.iter().enumerate() {
        if let Some(trigger) = step.trigger() {
            if !loaded
                .pack
                .commands
                .iter()
                .any(|command| command.trigger == trigger)
            {
                let _ = writeln!(
                    err,
                    "error {}: 第 {} 个事件的触发词 {trigger} 不在清单的 commands 里",
                    fixture.display(),
                    index + 1
                );
                return EXIT_USAGE;
            }
        }
    }
    run_module(&module, grants, &steps, out)
}

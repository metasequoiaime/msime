//! 会话句柄与会话上的函数：建与释放、设置、按键、状态与候选、选择与逐候选学习。
//!
//! 一个句柄里是一把互斥锁护着的会话。同一会话的调用由调用方串行，锁只用 `try_lock`：拿不到就是两个线程同时在调，返回 `MSIME_ENGINE_BUSY`，不等待。核心 `Session` 是 `Send`，所以会话可以在任何线程上用，只是不能并发。会话里的 panic 挡在边界内并作废会话，之后的调用都返回 `MSIME_ENGINE_PANIC`。

use std::ffi::c_char;
use std::sync::{Mutex, TryLockError};

use msime_engine::{
    CandidateEdge, Command, DictionaryAccess, FirstPageRows, FrequencyAdjustmentMode, KeyResult,
    LocalInputMode, SchemeType, Session, SessionSnapshot,
};
use serde_json::{json, Value};

use crate::abi::{
    check_output, guard, input_str, msime_engine_candidate, msime_engine_result,
    msime_engine_state, msime_engine_str, panic_message, write_owned, write_sized, Failure,
    Outcome,
};
use crate::{
    MSIME_ENGINE_BUSY, MSIME_ENGINE_CANDIDATE_ANSWERS_KEY, MSIME_ENGINE_CANDIDATE_FUZZY,
    MSIME_ENGINE_CANDIDATE_SENTENCE, MSIME_ENGINE_EDGE_FIRST_HAN, MSIME_ENGINE_EDGE_LAST_HAN,
    MSIME_ENGINE_FREQUENCY_DISABLED, MSIME_ENGINE_FREQUENCY_HALVE, MSIME_ENGINE_FREQUENCY_LINEAR,
    MSIME_ENGINE_FREQUENCY_PIN, MSIME_ENGINE_FREQUENCY_PROMOTE, MSIME_ENGINE_KEY_SHIFT_ONLY,
    MSIME_ENGINE_OPTION_CHINESE_PUNCT, MSIME_ENGINE_OPTION_DEDICATED_ENGLISH,
    MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS, MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP,
    MSIME_ENGINE_OPTION_FREQUENCY_MODE, MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT,
    MSIME_ENGINE_OPTION_HELPCODE_ENABLED, MSIME_ENGINE_OPTION_HELPCODE_SCHEMA,
    MSIME_ENGINE_OPTION_PAIRED_PUNCT, MSIME_ENGINE_OPTION_PERSONAL_CONTEXT,
    MSIME_ENGINE_OPTION_PREDICTION_ALLOWED, MSIME_ENGINE_OPTION_PUNCT_LOCK,
    MSIME_ENGINE_OPTION_RESCORING_CONTEXT, MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN,
    MSIME_ENGINE_PANIC, MSIME_ENGINE_STALE, MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN,
    MSIME_ENGINE_STATE_DEDICATED_ENGLISH, MSIME_ENGINE_STATE_PREDICTION,
    MSIME_ENGINE_STATE_TAKES_SEPARATOR, MSIME_ENGINE_UNSUPPORTED,
};

/// 不透明的会话句柄，由 `msime_engine_session_new` 建、`msime_engine_session_free` 释放。
pub struct msime_engine_session {
    state: Mutex<SessionState>,
}

// 句柄会被交给任意线程：锁里的东西必须能在线程间移动。
const _: fn() = || {
    fn shared_between_threads<T: Send + Sync>() {}
    shared_between_threads::<msime_engine_session>();
};

/// 字段按声明顺序析构：会话先结束（写下学习、交还日志连接），之后才放掉词库租约，维护在会话写完之前拿不到锁。
struct SessionState {
    session: Session,
    _access: DictionaryAccess,
    /// 之前的调用在库里 panic 过，会话状态不可信。
    poisoned: bool,
    /// 当前修订号下的快照；每次改变状态的调用都清掉，`msime_engine_state_get` 和 `msime_engine_candidate_get` 借出的字符串指向它。
    snapshot: Option<SessionSnapshot>,
    /// 上一次改变状态的调用的结果，`msime_engine_result` 借出的 `commit` 和 `diagnostic` 指向它。
    result: KeyResult,
}

impl SessionState {
    /// 改变状态之前：之前借出的字符串到此作废。
    fn begin_change(&mut self) {
        self.snapshot = None;
    }

    /// 宿主看的快照：全拼的 `preedit` 按音节分开，与 msime 自己的宿主一致。
    fn snapshot(&mut self) -> &SessionSnapshot {
        self.snapshot
            .get_or_insert_with(|| self.session.display_snapshot())
    }

    fn check_revision(&self, revision: u64) -> Outcome {
        let current = self.session.candidate_revision();
        if revision != current {
            return Err(Failure::new(
                MSIME_ENGINE_STALE,
                format!("revision {revision} is stale; the candidates are at revision {current}"),
            ));
        }
        Ok(())
    }

    /// 记下结果并写给调用方（`output` 可以是 NULL）。
    ///
    /// # Safety
    /// 同 [`write_sized`]。
    unsafe fn finish(&mut self, result: KeyResult, output: *mut msime_engine_result) -> Outcome {
        self.result = result;
        if output.is_null() {
            return Ok(());
        }
        let result = &self.result;
        let value = msime_engine_result {
            struct_size: 0,
            handled: u32::from(result.handled),
            has_commit: u32::from(result.commit.is_some()),
            diagnostic_code: result
                .diagnostic_code()
                .map_or(0, |code| u32::from(code.value())),
            revision: self.session.candidate_revision(),
            commit: msime_engine_str::borrow(result.commit.as_deref().unwrap_or_default()),
            diagnostic: msime_engine_str::borrow(result.diagnostic.as_deref().unwrap_or_default()),
        };
        // SAFETY: 调用方的前提。
        unsafe { write_sized(output, value, "result") }
    }
}

/// 在会话上执行一次调用：空句柄、并发调用、已作废的会话各自报错；`body` 里的 panic 作废会话。
fn with_session(
    handle: *const msime_engine_session,
    body: impl FnOnce(&mut SessionState) -> Outcome,
) -> i32 {
    guard(|| {
        // SAFETY: 非空的句柄来自 `msime_engine_session_new`，调用方保证它还没被释放。
        let Some(handle) = (unsafe { handle.as_ref() }) else {
            return Err(Failure::null("session"));
        };
        let mut state = match handle.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return Err(Failure::new(
                MSIME_ENGINE_BUSY,
                "another thread is calling this session; calls into one session must not overlap",
            )),
            // `body` 的 panic 在下面就被接住，锁不会因此中毒；真中毒了也只能当作废处理。
            Err(TryLockError::Poisoned(_)) => {
                return Err(Failure::new(
                    MSIME_ENGINE_PANIC,
                    "session is unusable after a panic; free it",
                ))
            }
        };
        if state.poisoned {
            return Err(Failure::new(
                MSIME_ENGINE_PANIC,
                "session is unusable after a panic; free it",
            ));
        }
        let state = &mut *state;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(state))) {
            Ok(outcome) => outcome,
            Err(payload) => {
                state.poisoned = true;
                Err(Failure::new(
                    MSIME_ENGINE_PANIC,
                    panic_message(payload.as_ref()),
                ))
            }
        }
    })
}

/// 下标转成引擎的 `usize`。
fn index(value: u32) -> usize {
    value as usize
}

fn local_mode_value(mode: LocalInputMode) -> u32 {
    match mode {
        LocalInputMode::None => crate::MSIME_ENGINE_LOCAL_MODE_NONE,
        LocalInputMode::Unicode => crate::MSIME_ENGINE_LOCAL_MODE_UNICODE,
        LocalInputMode::DateTime => crate::MSIME_ENGINE_LOCAL_MODE_DATE_TIME,
        LocalInputMode::QuickPhrase => crate::MSIME_ENGINE_LOCAL_MODE_QUICK_PHRASE,
        LocalInputMode::Emoji => crate::MSIME_ENGINE_LOCAL_MODE_EMOJI,
        LocalInputMode::Kaomoji => crate::MSIME_ENGINE_LOCAL_MODE_KAOMOJI,
        LocalInputMode::SuperJianpin => crate::MSIME_ENGINE_LOCAL_MODE_SUPER_JIANPIN,
        LocalInputMode::TemporaryEnglish => crate::MSIME_ENGINE_LOCAL_MODE_TEMPORARY_ENGLISH,
        LocalInputMode::TemporaryJapanese => crate::MSIME_ENGINE_LOCAL_MODE_TEMPORARY_JAPANESE,
        LocalInputMode::Expression => crate::MSIME_ENGINE_LOCAL_MODE_EXPRESSION,
        LocalInputMode::Command => crate::MSIME_ENGINE_LOCAL_MODE_COMMAND,
        LocalInputMode::Mention => crate::MSIME_ENGINE_LOCAL_MODE_MENTION,
        LocalInputMode::Url => crate::MSIME_ENGINE_LOCAL_MODE_URL,
        LocalInputMode::Chaizi => crate::MSIME_ENGINE_LOCAL_MODE_CHAIZI,
        LocalInputMode::Symbol => crate::MSIME_ENGINE_LOCAL_MODE_SYMBOL,
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// `MSIME_ENGINE_FREQUENCY_*` 到引擎的调频方式；编号冻结，与选项 JSON 的 `frequency.mode` 名字一一对应。
fn frequency_mode(value: i64) -> Outcome<FrequencyAdjustmentMode> {
    let mode = u32::try_from(value).ok().and_then(|value| match value {
        MSIME_ENGINE_FREQUENCY_DISABLED => Some(FrequencyAdjustmentMode::Disabled),
        MSIME_ENGINE_FREQUENCY_PIN => Some(FrequencyAdjustmentMode::Pin),
        MSIME_ENGINE_FREQUENCY_HALVE => Some(FrequencyAdjustmentMode::Halve),
        MSIME_ENGINE_FREQUENCY_LINEAR => Some(FrequencyAdjustmentMode::Linear),
        MSIME_ENGINE_FREQUENCY_PROMOTE => Some(FrequencyAdjustmentMode::Promote),
        _ => None,
    });
    mode.ok_or_else(|| Failure::invalid(format!("unknown frequency mode {value}")))
}

/// 调频的次数与步长：1 到 10，与建会话时的检查相同。
fn frequency_count(value: i64, what: &'static str) -> Outcome<i32> {
    if !(1..=10).contains(&value) {
        return Err(Failure::invalid(format!("{what} must be 1..=10")));
    }
    Ok(value as i32)
}

fn bool_option(value: i64) -> Outcome<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Failure::invalid("a switch option takes 0 or 1")),
    }
}

// ---- 会话 ----

/// 按选项 JSON（版本 1，键表见 README）建一个会话，成功时把句柄写进 `*session`，失败时写 NULL。会话持有这两个目录上的共享词库租约，直到释放：租约在维护（`msime_engine_user_dict_edit`、`msime_engine_reset_learning`）进行时拿不到，这时返回 `MSIME_ENGINE_BUSY`。
///
/// # Safety
/// `options_json` 指向 `options_len` 个可读字节；`session` 可写一个指针。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_session_new(
    options_json: *const c_char,
    options_len: usize,
    session: *mut *mut msime_engine_session,
) -> i32 {
    guard(|| {
        if session.is_null() {
            return Err(Failure::null("session"));
        }
        // SAFETY: 上面检查过非空，调用方保证可写。
        unsafe { session.write(std::ptr::null_mut()) };
        // SAFETY: 调用方保证 `options_json` 的长度。
        let json = unsafe { input_str(options_json, options_len, "options_json") }?;
        let options = crate::options::parse(json)?;
        options.paths.validate()?;
        let access =
            DictionaryAccess::try_session(&options.paths.user_data, &options.paths.dictionaries)
                .map_err(|error| {
                    Failure::new(
                        crate::MSIME_ENGINE_STORAGE,
                        format!("dictionary lease: {error}"),
                    )
                })?
                .ok_or_else(|| {
                    Failure::new(
                        MSIME_ENGINE_BUSY,
                        msime_engine::diagnostics::DICTIONARY_MAINTENANCE_BUSY,
                    )
                })?;
        let engine = Session::new(options)?;
        let handle = Box::new(msime_engine_session {
            state: Mutex::new(SessionState {
                session: engine,
                _access: access,
                poisoned: false,
                snapshot: None,
                result: KeyResult::default(),
            }),
        });
        // SAFETY: 同上。
        unsafe { session.write(Box::into_raw(handle)) };
        Ok(())
    })
}

/// 结束并释放会话：写下最近的学习，交还日志连接和词库租约。NULL 什么也不做。另一个线程正在调这个会话时返回 `MSIME_ENGINE_BUSY`、不释放；已作废的会话照样可以释放。释放之后句柄不能再用。
///
/// # Safety
/// `session` 是 NULL 或 `msime_engine_session_new` 交出、还没释放过的句柄。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_session_free(session: *mut msime_engine_session) -> i32 {
    guard(|| {
        if session.is_null() {
            return Ok(());
        }
        // SAFETY: 调用方保证句柄有效。
        let busy = match unsafe { &*session }.state.try_lock() {
            Ok(_) | Err(TryLockError::Poisoned(_)) => false,
            Err(TryLockError::WouldBlock) => true,
        };
        if busy {
            return Err(Failure::new(
                MSIME_ENGINE_BUSY,
                "another thread is calling this session; free it after that call returns",
            ));
        }
        // SAFETY: 句柄来自 `Box::into_raw`，调用方保证只释放一次。析构里的 panic 由 `guard` 接住。
        drop(unsafe { Box::from_raw(session) });
        Ok(())
    })
}

/// 换方案（`MSIME_ENGINE_SCHEME_*`），丢弃组字。方案必须在选项的 `enabled_schemes` 里；新方案的词库打不开时失败，留在原方案、组字不动。
///
/// # Safety
/// `session` 是有效的句柄。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_switch_scheme(
    session: *mut msime_engine_session,
    scheme: u32,
) -> i32 {
    with_session(session, |state| {
        let scheme = u8::try_from(scheme)
            .ok()
            .and_then(SchemeType::from_u8)
            .ok_or_else(|| Failure::invalid(format!("unknown scheme {scheme}")))?;
        state.begin_change();
        state.session.switch_scheme(scheme)?;
        Ok(())
    })
}

/// 设一个整数选项（`MSIME_ENGINE_OPTION_*` 里标着「整数」的）。不认识的编号是 `MSIME_ENGINE_UNSUPPORTED`，字符串选项或越界的值是 `MSIME_ENGINE_INVALID_ARGUMENT`。
///
/// # Safety
/// `session` 是有效的句柄。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_set_int(
    session: *mut msime_engine_session,
    option: u32,
    value: i64,
) -> i32 {
    with_session(session, |state| {
        match option {
            MSIME_ENGINE_OPTION_CHINESE_PUNCT => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_chinese_punctuation_enabled(enabled);
            }
            MSIME_ENGINE_OPTION_PAIRED_PUNCT => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_paired_punctuation_enabled(enabled);
            }
            MSIME_ENGINE_OPTION_PUNCT_LOCK => {
                let lock = i32::try_from(value)
                    .map_err(|_| Failure::invalid("punctuation lock must be 0..=2"))?;
                state.begin_change();
                state.session.set_punctuation_lock(lock)?;
            }
            MSIME_ENGINE_OPTION_DEDICATED_ENGLISH => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_dedicated_english(enabled);
            }
            MSIME_ENGINE_OPTION_HELPCODE_ENABLED => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_helpcode_enabled(enabled);
            }
            MSIME_ENGINE_OPTION_PERSONAL_CONTEXT => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_personal_context_enabled(enabled);
            }
            MSIME_ENGINE_OPTION_PREDICTION_ALLOWED => {
                let allowed = bool_option(value)?;
                state.begin_change();
                state.session.set_prediction_allowed(allowed);
            }
            MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS => {
                if !(1..=255).contains(&value) {
                    return Err(Failure::invalid("first page rows must be 1..=255"));
                }
                state.begin_change();
                state
                    .session
                    .set_first_page_rows(FirstPageRows::new(value as usize));
            }
            MSIME_ENGINE_OPTION_FREQUENCY_MODE
            | MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT
            | MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP => {
                let mut frequency = state.session.frequency_adjustment();
                match option {
                    MSIME_ENGINE_OPTION_FREQUENCY_MODE => frequency.mode = frequency_mode(value)?,
                    MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT => {
                        frequency.trigger_count = frequency_count(value, "trigger count")?;
                    }
                    _ => frequency.linear_step = frequency_count(value, "linear step")?,
                }
                state.begin_change();
                state.session.set_frequency_adjustment(frequency)?;
            }
            MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN => {
                let enabled = bool_option(value)?;
                state.begin_change();
                state.session.set_wubi_mixed_pinyin(enabled);
            }
            MSIME_ENGINE_OPTION_HELPCODE_SCHEMA | MSIME_ENGINE_OPTION_RESCORING_CONTEXT => {
                return Err(Failure::invalid(format!("option {option} takes a string")));
            }
            _ => {
                return Err(Failure::new(
                    MSIME_ENGINE_UNSUPPORTED,
                    format!("unknown option {option}"),
                ))
            }
        }
        Ok(())
    })
}

/// 设一个字符串选项（`MSIME_ENGINE_OPTION_*` 里标着「字符串」的），`value` 是 `value_len` 个字节的 UTF-8。不认识的编号是 `MSIME_ENGINE_UNSUPPORTED`，整数选项是 `MSIME_ENGINE_INVALID_ARGUMENT`。
///
/// # Safety
/// `session` 是有效的句柄；`value` 指向 `value_len` 个可读字节。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_set_string(
    session: *mut msime_engine_session,
    option: u32,
    value: *const c_char,
    value_len: usize,
) -> i32 {
    with_session(session, |state| {
        // SAFETY: 调用方保证长度。
        let value = unsafe { input_str(value, value_len, "value") }?;
        match option {
            MSIME_ENGINE_OPTION_HELPCODE_SCHEMA => {
                if !Session::is_supported_helpcode_schema(value) {
                    return Err(Failure::invalid(format!(
                        "unknown helpcode schema {value:?}"
                    )));
                }
                state.begin_change();
                if !state.session.set_helpcode_schema(value) {
                    return Err(Failure::new(
                        crate::MSIME_ENGINE_FAILED,
                        format!("helpcode table {value:?} could not be loaded"),
                    ));
                }
            }
            MSIME_ENGINE_OPTION_RESCORING_CONTEXT => {
                state.begin_change();
                state.session.set_rescoring_context(value);
            }
            MSIME_ENGINE_OPTION_CHINESE_PUNCT
            | MSIME_ENGINE_OPTION_PAIRED_PUNCT
            | MSIME_ENGINE_OPTION_PUNCT_LOCK
            | MSIME_ENGINE_OPTION_DEDICATED_ENGLISH
            | MSIME_ENGINE_OPTION_HELPCODE_ENABLED
            | MSIME_ENGINE_OPTION_PERSONAL_CONTEXT
            | MSIME_ENGINE_OPTION_PREDICTION_ALLOWED
            | MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS
            | MSIME_ENGINE_OPTION_FREQUENCY_MODE
            | MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT
            | MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP
            | MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN => {
                return Err(Failure::invalid(format!(
                    "option {option} takes an integer"
                )));
            }
            _ => {
                return Err(Failure::new(
                    MSIME_ENGINE_UNSUPPORTED,
                    format!("unknown option {option}"),
                ))
            }
        }
        Ok(())
    })
}

/// 忘掉上屏的上下文（焦点或应用变了、宿主自己插入了文字），清掉上屏后联想。
///
/// # Safety
/// `session` 是有效的句柄。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_reset_context(session: *mut msime_engine_session) -> i32 {
    with_session(session, |state| {
        state.begin_change();
        state.session.reset_context();
        Ok(())
    })
}

// ---- 输入 ----

/// 一个 ASCII 字符（0x20 到 0x7E，另收 Tab 0x09）：字母、拼写用的数字，以及 `spelling_symbols` 里列出的符号。`flags` 是 `MSIME_ENGINE_KEY_*`。结果写进 `result`（可以是 NULL）。按键路由见 README。
///
/// # Safety
/// `session` 是有效的句柄；`result` 是 NULL 或按 `struct_size` 约定可写。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_key_char(
    session: *mut msime_engine_session,
    character: u8,
    flags: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_session(session, |state| {
        if !(character == b'\t' || (0x20..=0x7e).contains(&character)) {
            return Err(Failure::invalid(
                "key_char takes one printable ASCII character or Tab",
            ));
        }
        if flags & !MSIME_ENGINE_KEY_SHIFT_ONLY != 0 {
            return Err(Failure::new(
                MSIME_ENGINE_UNSUPPORTED,
                format!("unknown key flags {flags:#x}"),
            ));
        }
        // SAFETY: 调用方的前提。
        unsafe { check_output(result, "result") }?;
        state.begin_change();
        let outcome = state
            .session
            .character(character, flags & MSIME_ENGINE_KEY_SHIFT_ONLY != 0);
        // SAFETY: 调用方的前提。
        unsafe { state.finish(outcome, result) }
    })
}

/// ABI 1.1 起。数字键当选择键，引擎自己的数字选词入口：有组字时 `1`–`9` 上屏整张候选表的第 `digit - 1` 行（也就是第一页，`msime_engine_select` 的那套学习照常）；越南文 VNI 和注音此刻拼写用的数字交给拼写；没有组字时不处理（`handled` 为 0），并且忘掉上屏的上下文，因为这个数字由宿主自己交给应用；上屏后联想的候选不接数字，按下时被清掉。`digit` 只收 ASCII 数字 `0`–`9`，其余是 `MSIME_ENGINE_INVALID_ARGUMENT`。不带修订号：它选的是调用时的候选。Unicode、日期、计算这些用数字拼写的本地模式里它照样选词，所以按键路由仍是先 `msime_engine_key_char`，没被处理时再用它（只显示第一页的前端）或 `msime_engine_select`（翻过页的前端），见 README 按键路由第 4 条。
///
/// # Safety
/// 同 `msime_engine_key_char`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_candidate_key(
    session: *mut msime_engine_session,
    digit: u8,
    result: *mut msime_engine_result,
) -> i32 {
    with_session(session, |state| {
        if !digit.is_ascii_digit() {
            return Err(Failure::invalid("candidate_key takes one ASCII digit 0-9"));
        }
        // SAFETY: 调用方的前提。
        unsafe { check_output(result, "result") }?;
        state.begin_change();
        let outcome = state.session.candidate_key(digit);
        // SAFETY: 调用方的前提。
        unsafe { state.finish(outcome, result) }
    })
}

/// 一个 ASCII 标点：有组字时以首选结束组字，再接上中文标点；中文标点关着、锁成 ASCII 或这个键没有中文标点时不处理（`handled` 为 0），宿主自己输出这个键。宿主高亮了首选以外的行时，先用 `msime_engine_finish` 以那一行结束，再调这个函数。
///
/// # Safety
/// 同 `msime_engine_key_char`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_key_punct(
    session: *mut msime_engine_session,
    character: u8,
    result: *mut msime_engine_result,
) -> i32 {
    with_session(session, |state| {
        if !character.is_ascii_punctuation() {
            return Err(Failure::invalid(
                "key_punct takes one ASCII punctuation character",
            ));
        }
        // SAFETY: 调用方的前提。
        unsafe { check_output(result, "result") }?;
        state.begin_change();
        let outcome = state.session.punctuation(character);
        // SAFETY: 调用方的前提。
        unsafe { state.finish(outcome, result) }
    })
}

/// 一个编辑命令（`MSIME_ENGINE_COMMAND_*`）。
///
/// # Safety
/// 同 `msime_engine_key_char`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_command(
    session: *mut msime_engine_session,
    command: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_session(session, |state| {
        let command = u8::try_from(command)
            .ok()
            .and_then(Command::from_u8)
            .ok_or_else(|| {
                Failure::new(
                    MSIME_ENGINE_UNSUPPORTED,
                    format!("unknown command {command}"),
                )
            })?;
        // SAFETY: 调用方的前提。
        unsafe { check_output(result, "result") }?;
        state.begin_change();
        let outcome = state.session.command(command);
        // SAFETY: 调用方的前提。
        unsafe { state.finish(outcome, result) }
    })
}

/// 把组字光标放到 `editing_text` 的第 `caret_byte` 个字节前；负数（`MSIME_ENGINE_CARET_END`）回到末尾。位置限制在可编辑的范围内，九键组码时什么也不做。
///
/// # Safety
/// `session` 是有效的句柄。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_set_caret(
    session: *mut msime_engine_session,
    caret_byte: i64,
) -> i32 {
    with_session(session, |state| {
        let caret = usize::try_from(caret_byte).ok();
        state.begin_change();
        state.session.set_caret(caret);
        Ok(())
    })
}

// ---- 状态 ----

/// 读组字状态。只读：不改变会话，之前借出的字符串照样有效。
///
/// # Safety
/// `session` 是有效的句柄；`state` 按 `struct_size` 约定可写。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_state_get(
    session: *mut msime_engine_session,
    state: *mut msime_engine_state,
) -> i32 {
    with_session(session, |inner| {
        if state.is_null() {
            return Err(Failure::null("state"));
        }
        let revision = inner.session.candidate_revision();
        let takes_separator = inner.session.takes_local_separator();
        let snapshot = inner.snapshot();
        let mut flags = 0;
        if snapshot.prediction {
            flags |= MSIME_ENGINE_STATE_PREDICTION;
        }
        if snapshot.dedicated_english {
            flags |= MSIME_ENGINE_STATE_DEDICATED_ENGLISH;
        }
        if snapshot.candidate_list_open {
            flags |= MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN;
        }
        if takes_separator {
            flags |= MSIME_ENGINE_STATE_TAKES_SEPARATOR;
        }
        let value = msime_engine_state {
            struct_size: 0,
            scheme: u32::from(snapshot.scheme as u8),
            revision,
            candidate_count: saturating_u32(snapshot.candidates.len()),
            flags,
            local_mode: local_mode_value(snapshot.local_mode),
            caret_byte: saturating_u32(snapshot.caret_position),
            conversion_focus_start_scalar: saturating_u32(snapshot.conversion_focus.0),
            conversion_focus_end_scalar: saturating_u32(snapshot.conversion_focus.1),
            preedit: msime_engine_str::borrow(&snapshot.preedit),
            editing_text: msime_engine_str::borrow(&snapshot.editing_text),
            conversion: msime_engine_str::borrow(&snapshot.conversion),
            spelling_symbols: msime_engine_str::borrow(&snapshot.spelling_symbols),
        };
        // SAFETY: 调用方的前提。
        unsafe { write_sized(state, value, "state") }
    })
}

/// 读第 `index` 行候选。`revision` 必须是当前的候选修订号，否则 `MSIME_ENGINE_STALE`；越界是 `MSIME_ENGINE_INVALID_ARGUMENT`。只读，可以先取状态再逐个取一页候选。
///
/// # Safety
/// `session` 是有效的句柄；`candidate` 按 `struct_size` 约定可写。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_candidate_get(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    candidate: *mut msime_engine_candidate,
) -> i32 {
    with_session(session, |state| {
        if candidate.is_null() {
            return Err(Failure::null("candidate"));
        }
        state.check_revision(revision)?;
        let snapshot = state.snapshot();
        let row = index as usize;
        let Some(item) = snapshot.candidates.get(row) else {
            return Err(Failure::invalid(format!(
                "candidate {index} is out of range ({} candidates)",
                snapshot.candidates.len()
            )));
        };
        let mut flags = 0;
        if item.fuzzy {
            flags |= MSIME_ENGINE_CANDIDATE_FUZZY;
        }
        if snapshot
            .candidate_answers_key
            .get(row)
            .copied()
            .unwrap_or(false)
        {
            flags |= MSIME_ENGINE_CANDIDATE_ANSWERS_KEY;
        }
        if item.sentence_association {
            flags |= MSIME_ENGINE_CANDIDATE_SENTENCE;
        }
        let value = msime_engine_candidate {
            struct_size: 0,
            index,
            source: u32::from(item.source as u8),
            scheme: u32::from(item.scheme as u8),
            flags,
            fixed_position: item.fixed_position,
            word: msime_engine_str::borrow(&item.word),
            code: msime_engine_str::borrow(&item.pinyin),
            annotation: msime_engine_str::borrow(
                snapshot
                    .candidate_annotations
                    .get(row)
                    .map_or("", String::as_str),
            ),
            corrected_from: msime_engine_str::borrow(&item.corrected_from),
        };
        // SAFETY: 调用方的前提。
        unsafe { write_sized(candidate, value, "candidate") }
    })
}

/// 单字母全拼只先给出一部分候选；宿主翻过第一页时调它要回其余的。`*grew` 写 1 表示列表变长了（修订号随之改变），否则写 0。会改变状态。
///
/// # Safety
/// `session` 是有效的句柄；`grew` 是 NULL 或可写。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_expand_candidates(
    session: *mut msime_engine_session,
    grew: *mut u32,
) -> i32 {
    with_session(session, |state| {
        state.begin_change();
        let expanded = state.session.expand_initial_candidates();
        if !grew.is_null() {
            // SAFETY: 调用方保证可写。
            unsafe { grew.write(u32::from(expanded)) };
        }
        Ok(())
    })
}

/// 整个状态和全部候选的 JSON，给调试和日志用；格式不属于 ABI，随版本变化。返回的字符串用 `msime_engine_string_free` 释放。只读。
///
/// # Safety
/// `session` 是有效的句柄；`json` 可写一个指针。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_state_json(
    session: *mut msime_engine_session,
    json: *mut *mut c_char,
) -> i32 {
    with_session(session, |state| {
        if json.is_null() {
            return Err(Failure::null("json"));
        }
        let revision = state.session.candidate_revision();
        let text = debug_json(revision, state.snapshot()).to_string();
        // SAFETY: 上面检查过非空。
        unsafe { write_owned(json, text) };
        Ok(())
    })
}

fn debug_json(revision: u64, snapshot: &SessionSnapshot) -> Value {
    let candidates: Vec<Value> = snapshot
        .candidates
        .iter()
        .enumerate()
        .map(|(index, item)| {
            json!({
                "word": item.word.as_str(),
                "code": item.pinyin.as_str(),
                "canonical_code": item.canonical_pinyin.as_str(),
                "source": item.source as u8,
                "scheme": item.scheme.name(),
                "weight": item.weight,
                "fixed_position": item.fixed_position,
                "fuzzy": item.fuzzy,
                "corrected_from": item.corrected_from.as_str(),
                "sentence": item.sentence_association,
                "annotation": snapshot.candidate_annotations.get(index).map_or("", String::as_str),
                "answers_key": snapshot.candidate_answers_key.get(index).copied().unwrap_or(false),
            })
        })
        .collect();
    json!({
        "revision": revision,
        "scheme": snapshot.scheme.name(),
        "local_mode": snapshot.local_mode.name(),
        "prediction": snapshot.prediction,
        "dedicated_english": snapshot.dedicated_english,
        "preedit": snapshot.preedit,
        "editing_text": snapshot.editing_text,
        "caret_byte": snapshot.caret_position,
        "raw_segmentation": snapshot.raw_segmentation,
        "normalized_segmentation": snapshot.normalized_segmentation,
        "spelling_symbols": snapshot.spelling_symbols,
        "shuangpin_profile": snapshot.shuangpin_profile,
        "conversion": snapshot.conversion,
        "conversion_focus_scalar": [snapshot.conversion_focus.0, snapshot.conversion_focus.1],
        "candidate_list_open": snapshot.candidate_list_open,
        "nine_key_spellings": snapshot.nine_key_spellings,
        "candidates": candidates,
    })
}

// ---- 选择与逐候选学习 ----
//
// 都带 `revision`：不是当前修订号时 `MSIME_ENGINE_STALE`，什么也不做。下标是引擎顺序里的行号，与 `msime_engine_candidate_get` 相同。

/// 在核对修订号和输出结构体之后执行一次选择类调用；被拒绝的调用不碰会话。
fn with_candidate(
    session: *mut msime_engine_session,
    revision: u64,
    result: *mut msime_engine_result,
    action: impl FnOnce(&mut Session) -> KeyResult,
) -> i32 {
    with_session(session, |state| {
        state.check_revision(revision)?;
        // SAFETY: 调用方的前提（每个公开函数的 Safety 段）。
        unsafe { check_output(result, "result") }?;
        state.begin_change();
        let outcome = action(&mut state.session);
        // SAFETY: 调用方的前提（每个公开函数的 Safety 段）。
        unsafe { state.finish(outcome, result) }
    })
}

/// 选第 `index` 行上屏（或消耗它覆盖的那段编码，剩下的接着组字），并照常学习。
///
/// # Safety
/// `session` 是有效的句柄；`result` 是 NULL 或按 `struct_size` 约定可写。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_select(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.select(self::index(index))
    })
}

/// 以词定字：上屏第 `index` 行的第一个或最后一个汉字（`MSIME_ENGINE_EDGE_*`），丢掉其余组字。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_select_edge(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    edge: u32,
    result: *mut msime_engine_result,
) -> i32 {
    let edge = match edge {
        MSIME_ENGINE_EDGE_FIRST_HAN => CandidateEdge::FirstHan,
        MSIME_ENGINE_EDGE_LAST_HAN => CandidateEdge::LastHan,
        _ => {
            return guard(|| Err(Failure::invalid(format!("unknown edge {edge}"))));
        }
    };
    with_candidate(session, revision, result, |engine| {
        engine.select_edge(self::index(index), edge)
    })
}

/// 结束组字：从第 `first_index` 行（宿主高亮的那一行）起上屏，用于失焦、标点前或宿主自己的结束键。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_finish(
    session: *mut msime_engine_session,
    revision: u64,
    first_index: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.finish(self::index(first_index))
    })
}

/// 把第 `index` 行（词库里的词）提到前面，不上屏。不支持的行不处理（`handled` 为 0）。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_pin(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.pin(self::index(index))
    })
}

/// 删掉第 `index` 行：用户自己的词从个人词典删除，随包的词不再出现。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_remove(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.remove(self::index(index))
    })
}

/// 把第 `index` 行固定在第 `position` 位（1..=5）。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_fix_position(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    position: i32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.fix_position(self::index(index), position)
    })
}

/// 取消第 `index` 行的固定位置。
///
/// # Safety
/// 同 `msime_engine_select`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_clear_position(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
    result: *mut msime_engine_result,
) -> i32 {
    with_candidate(session, revision, result, |engine| {
        engine.clear_position(self::index(index))
    })
}

#[cfg(test)]
pub(crate) fn lock_for_test(
    session: *mut msime_engine_session,
) -> std::sync::MutexGuard<'static, impl Sized> {
    // SAFETY: 测试里句柄一直有效，直到测试释放它。
    unsafe { &*session }.state.lock().expect("session lock")
}

#[cfg(test)]
pub(crate) fn run_for_test(
    session: *mut msime_engine_session,
    body: impl FnOnce() -> Outcome,
) -> i32 {
    with_session(session, |_| body())
}

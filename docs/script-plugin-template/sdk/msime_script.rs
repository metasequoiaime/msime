//! 水杉输入法桌面脚本插件 ABI v1 的 Rust 绑定：单文件，只用 `core`，可以直接放进 `#![no_std]` 的 crate（`#[path = "…/msime_script.rs"] mod msime_script;`）。
//!
//! 模块按能力域分组，和清单 `permissions` 一一对应：`event`、`log`、`random` 不需要能力；`composition` 要 `read_composition`；`commit` 要 `read_commit`；`candidates` 要 `read_candidates`；`rewrite`、`rerank`、`filter`、`annotate` 分别要 `rewrite_commit`、`rerank_candidates`、`filter_candidates`、`annotate_candidates`；`command` 要 `provide_commands`。只有真正调用到的函数才会成为模块的 import，校验要求导入了的函数都有对应的能力、申请了的能力都至少导入一个函数，所以用到哪一组就申请哪一项。
//!
//! 插件自己要导出 `msime_abi`（返回 [`ABI_VERSION`]）和 `msime_on_event(kind: i32) -> i32`（`kind` 是 [`hook`] 里的编号，返回负数算这一次失败），可选导出 `msime_init`。完整说明见仓库的 docs/plugins.md「脚本插件」一节。编号只会追加，不会改值。

#![allow(dead_code)]

/// `msime_abi()` 必须返回的版本号。
pub const ABI_VERSION: i32 = 1;

/// import 的返回码。成功时是非负数。
pub mod status {
    /// 没有授予这个函数所属的能力。
    pub const DENIED: i32 = -1;
    /// 这个事件没有这个字段，或者它被隐私规则屏蔽，两者故意不可区分。
    pub const ABSENT: i32 = -2;
    /// 字段编号不认识，或者参数不合法。
    pub const INVALID: i32 = -3;
    /// 目标缓冲区放不下，什么都没有复制。
    pub const LIMIT: i32 = -4;
}

/// 钩子：`msime_on_event` 的参数，也是 `event::KIND` 的值。清单 `hooks` 里写小写的名字。
pub mod hook {
    pub const FOCUS_IN: i32 = 0;
    pub const FOCUS_OUT: i32 = 1;
    pub const INPUT: i32 = 6;
    pub const SELECTION_RETRACTED: i32 = 7;
    pub const CANDIDATE_SELECTED: i32 = 8;
    pub const BEFORE_COMMIT: i32 = 9;
    pub const COMMIT: i32 = 10;
    pub const COMPOSITION_END: i32 = 11;
    pub const COMPOSITION_START: i32 = 12;
    pub const COMPOSITION_UPDATE: i32 = 13;
    pub const CANDIDATES_SHOWN: i32 = 14;
    pub const CANDIDATES_HIDDEN: i32 = 15;
    pub const CANDIDATES: i32 = 17;
    /// 用户在 `/` 指令里打了这个插件声明的触发词。只投递给触发词的主人，跑在 `msime_init` 之后的快照上。
    pub const COMMAND: i32 = 18;
}

/// `event::ORIGIN`：上屏文本从哪来（`commit`、`before_commit`）。
pub mod origin {
    pub const SELECTION: u64 = 0;
    pub const RAW: u64 = 1;
    pub const HELD_FLUSH: u64 = 2;
    pub const SCHEME_TEXT: u64 = 3;
    pub const FOCUS_LOSS: u64 = 4;
}

/// `event::HOW`：怎么选中的（`candidate_selected`）。
pub mod how {
    pub const FINISH: u64 = 0;
    pub const PUNCTUATION_FINISH: u64 = 1;
    pub const WUBI_TOP: u64 = 2;
    pub const SPACE_OVER_LIST: u64 = 3;
    pub const DIGIT: u64 = 4;
    pub const GRID_RAW: u64 = 5;
    pub const SELECT: u64 = 6;
    pub const ANY_CANDIDATE: u64 = 7;
    pub const EDGE: u64 = 8;
    pub const HIGHLIGHTED: u64 = 9;
    pub const WUBI_AUTO: u64 = 10;
}

/// `event::OUTCOME`：选中之后的结果（`candidate_selected`）。
pub mod outcome {
    pub const COMMITTED: u64 = 0;
    pub const HELD: u64 = 1;
    pub const PINNED: u64 = 2;
    pub const PARTIAL: u64 = 3;
}

/// `event::REASON`：组字结束或候选隐藏的原因（`composition_end`、`candidates_hidden`）。
pub mod reason {
    pub const COMMIT: u64 = 0;
    pub const CANCEL: u64 = 1;
    pub const DELETED: u64 = 2;
    pub const FOCUS: u64 = 3;
    pub const DESTROYED: u64 = 4;
    pub const HOST_COMMIT: u64 = 5;
    pub const OTHER: u64 = 6;
}

/// `event::LOCAL_MODE`：本地模式。只有 0 到 5 能读到文本，其余模式下文本类函数一律返回 `Absent`。
pub mod local_mode {
    pub const NONE: u64 = 0;
    pub const TEMPORARY_ENGLISH: u64 = 1;
    pub const TEMPORARY_JAPANESE: u64 = 2;
    pub const SUPER_JIANPIN: u64 = 3;
    pub const EMOJI: u64 = 4;
    pub const KAOMOJI: u64 = 5;
    pub const MENTION: u64 = 6;
    pub const URL: u64 = 7;
    pub const COMMAND: u64 = 8;
    pub const EXPRESSION: u64 = 9;
    pub const UNICODE: u64 = 10;
    pub const DATE_TIME: u64 = 11;
    pub const QUICK_PHRASE: u64 = 12;
    pub const CHAIZI: u64 = 13;
    pub const SYMBOL: u64 = 14;
    pub const UNKNOWN: u64 = 255;
}

/// `event::INPUT_CLASS`：打字前那次操作的类别（`input`）。
pub mod input_class {
    pub const CHARACTER: u64 = 0;
    pub const PUNCTUATION: u64 = 1;
    pub const SELECTION: u64 = 2;
    pub const NAVIGATION: u64 = 3;
    pub const EDIT: u64 = 4;
    pub const COMMIT: u64 = 5;
    pub const CANCEL: u64 = 6;
    pub const HOST_COMMIT: u64 = 7;
    pub const OTHER: u64 = 8;
    pub const FOCUS_LOSS: u64 = 9;
}

/// `event::RETRACTION`：撤回暂存选择的方式（`selection_retracted`）。
pub mod retraction {
    pub const RESTORE: u64 = 0;
    pub const DROP: u64 = 1;
}

/// import 返回的错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Denied,
    Absent,
    Invalid,
    Limit,
    /// 这个版本的绑定不认识的负数。
    Other(i64),
}

impl Error {
    fn of(code: i64) -> Self {
        match code {
            -1 => Self::Denied,
            -2 => Self::Absent,
            -3 => Self::Invalid,
            -4 => Self::Limit,
            other => Self::Other(other),
        }
    }
}

fn check(code: i64) -> Result<u64, Error> {
    if code < 0 {
        Err(Error::of(code))
    } else {
        Ok(code as u64)
    }
}

/// 复制文本到 `buffer`，返回其中的那一段。宿主给的总是 UTF-8。
fn text(code: i32, buffer: &[u8]) -> Result<&str, Error> {
    let len = check(i64::from(code))? as usize;
    core::str::from_utf8(&buffer[..len]).map_err(|_| Error::Invalid)
}

/// 复制文本到 `buffer`，不做 UTF-8 校验，直接交回其中的那一段字节。
fn raw(code: i32, buffer: &[u8]) -> Result<&[u8], Error> {
    let len = check(i64::from(code))? as usize;
    Ok(&buffer[..len])
}

fn cap(buffer: &[u8]) -> i32 {
    i32::try_from(buffer.len()).unwrap_or(i32::MAX)
}

/// 宿主函数。签名就是 ABI 的一部分；wasm32 上指针和 `i32` 一样宽。
mod sys {
    #[link(wasm_import_module = "msime")]
    unsafe extern "C" {
        pub fn event_scalar(field: i32) -> i64;
        pub fn log(level: i32, text: *const u8, len: i32);
        pub fn composition_text(field: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn composition_scalar(field: i32) -> i64;
        pub fn commit_text(field: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn commit_scalar(field: i32) -> i64;
        pub fn candidate_count() -> i32;
        pub fn candidate_text(index: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn candidate_code(index: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn candidate_annotation(index: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn candidate_meta(index: i32, field: i32) -> i64;
        pub fn rewrite_commit(text: *const u8, len: i32) -> i32;
        pub fn candidates_order(rows: *const u32, count: i32) -> i32;
        pub fn candidates_hide(rows: *const u32, count: i32) -> i32;
        pub fn candidates_annotate(index: i32, text: *const u8, len: i32) -> i32;
        pub fn command_text(field: i32, dst: *mut u8, cap: i32) -> i32;
        pub fn command_row(text: *const u8, text_len: i32, note: *const u8, note_len: i32) -> i32;
        pub fn random_fill(dst: *mut u8, len: i32) -> i32;
    }
}

/// 不需要能力：事件的枚举字段。
pub mod event {
    use super::{check, sys, Error};

    pub const KIND: i32 = 0;
    pub const ORIGIN: i32 = 1;
    pub const HOW: i32 = 2;
    pub const OUTCOME: i32 = 3;
    pub const REASON: i32 = 4;
    pub const LOCAL_MODE: i32 = 5;
    pub const INPUT_CLASS: i32 = 6;
    pub const RETRACTION: i32 = 7;
    /// 0 或 1：这次上屏能不能改写（`before_commit`）。
    pub const REWRITABLE: i32 = 8;
    /// 0 或 1：同一次操作里接着有新的组字（`commit`）。
    pub const CONTINUES: i32 = 9;
    /// 0 或 1：引擎生成的文本，例如计算器、命令（`commit`）。
    pub const GENERATED: i32 = 10;

    /// 字段的值；这个事件没有该字段时是 `Err(Error::Absent)`。
    pub fn scalar(field: i32) -> Result<u64, Error> {
        check(unsafe { sys::event_scalar(field) })
    }
}

/// 不需要能力：日志。输入法里是空操作，只有 `msime-pack run` 会把它打出来。
pub mod log {
    use super::sys;

    pub const DEBUG: i32 = 0;
    pub const INFO: i32 = 1;
    pub const WARN: i32 = 2;
    pub const ERROR: i32 = 3;

    pub fn write(level: i32, text: &str) {
        let len = i32::try_from(text.len()).unwrap_or(i32::MAX);
        unsafe { sys::log(level, text.as_ptr(), len) }
    }

    pub fn debug(text: &str) {
        write(DEBUG, text);
    }

    pub fn info(text: &str) {
        write(INFO, text);
    }

    pub fn warn(text: &str) {
        write(WARN, text);
    }

    pub fn error(text: &str) {
        write(ERROR, text);
    }
}

/// `read_composition`：预编辑串、读音、这次打的字符。
pub mod composition {
    use super::{cap, check, sys, text as decoded, Error};

    /// 文本：预编辑串（`composition_start`、`composition_update`）。
    pub const PREEDIT: i32 = 0;
    /// 文本：读音（同上）。
    pub const READING: i32 = 1;
    /// 文本：这次打的字符（`input`）。
    pub const INPUT_CHAR: i32 = 2;
    /// 只有标量：光标前已经打了几个编码字符（按打下的字母算，不是按预编辑串）。
    pub const CARET: i32 = 3;

    /// 复制文本到 `buffer`。放不下是 `Err(Error::Limit)`，先用 [`scalar`] 读长度。
    pub fn text(field: i32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code = unsafe { sys::composition_text(field, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    /// 文本字段的字节数，或者 `CARET`。
    pub fn scalar(field: i32) -> Result<u64, Error> {
        check(unsafe { sys::composition_scalar(field) })
    }
}

/// `read_commit`：选中的候选和上屏文本。
pub mod commit {
    use super::{cap, check, raw, sys, text as decoded, Error};

    /// 文本：选中的候选（`candidate_selected`）。
    pub const SELECTED: i32 = 0;
    /// 文本：完整的上屏文本（`before_commit`、`commit`）。
    pub const COMMIT: i32 = 1;
    /// 文本：上屏文本里可改写的那一段，不含标点、顶字的尾巴（同上）。
    pub const COMMIT_BODY: i32 = 2;
    /// 只有标量：选中候选在呈现顺序里的座位（`candidate_selected`）。
    pub const SEAT: i32 = 3;

    /// 复制文本到 `buffer`。放不下是 `Err(Error::Limit)`，先用 [`scalar`] 读长度。
    pub fn text(field: i32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code = unsafe { sys::commit_text(field, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    /// 和 [`text`] 一样，但不做 UTF-8 校验，交回字节。宿主给的总是合法 UTF-8，而校验在解释器里每个汉字要上百 fuel，逐字扫描文本的插件用它更省。
    pub fn bytes(field: i32, buffer: &mut [u8]) -> Result<&[u8], Error> {
        let code = unsafe { sys::commit_text(field, buffer.as_mut_ptr(), cap(buffer)) };
        raw(code, buffer)
    }

    /// 文本字段的字节数，或者 `SEAT`。
    pub fn scalar(field: i32) -> Result<u64, Error> {
        check(unsafe { sys::commit_scalar(field) })
    }
}

/// `read_candidates`：候选窗口里的前 20 行，插件变换之前的顺序和注释（`candidates_shown`、`candidates`）。
pub mod candidates {
    use super::{cap, check, raw, sys, text as decoded, Error};

    pub const SOURCE: i32 = 0;
    pub const TEXT_LEN: i32 = 1;
    pub const CODE_LEN: i32 = 2;
    pub const ANNOTATION_LEN: i32 = 3;
    /// 当前页码，不看 `index`。
    pub const PAGE: i32 = 16;
    /// 高亮行在页内的下标，不看 `index`。
    pub const HIGHLIGHTED: i32 = 17;

    /// 行数。
    pub fn count() -> Result<u64, Error> {
        check(i64::from(unsafe { sys::candidate_count() }))
    }

    pub fn text(index: u32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code = unsafe { sys::candidate_text(index as i32, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    /// 第 `index` 行的文本，不做 UTF-8 校验，交回字节；理由同 [`super::commit::bytes`]。
    pub fn bytes(index: u32, buffer: &mut [u8]) -> Result<&[u8], Error> {
        let code = unsafe { sys::candidate_text(index as i32, buffer.as_mut_ptr(), cap(buffer)) };
        raw(code, buffer)
    }

    pub fn code(index: u32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code = unsafe { sys::candidate_code(index as i32, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    pub fn annotation(index: u32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code =
            unsafe { sys::candidate_annotation(index as i32, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    /// 第 `index` 行的 `SOURCE`、各文本的字节数，或者页级的 `PAGE`、`HIGHLIGHTED`。
    pub fn meta(index: u32, field: i32) -> Result<u64, Error> {
        check(unsafe { sys::candidate_meta(index as i32, field) })
    }
}

/// `rewrite_commit`（依赖 `read_commit`，不能和 `read_composition` 同时申请）：只在 `before_commit` 里，事件的「能否改写」不为真时返回 `Err(Error::Denied)`。
pub mod rewrite {
    use super::{check, sys, Error};

    /// 把上屏文本的可改写段换成 `text`；尾巴原样接在后面。
    pub fn commit(text: &str) -> Result<(), Error> {
        let len = i32::try_from(text.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::rewrite_commit(text.as_ptr(), len)
        }))
        .map(drop)
    }
}

/// `rerank_candidates`（依赖 `read_candidates`）：只在 `candidates` 里，别的钩子里返回 `Err(Error::Denied)`。
pub mod rerank {
    use super::{check, sys, Error};

    /// 按 `rows` 里的行号重排。
    pub fn order(rows: &[u32]) -> Result<(), Error> {
        let count = i32::try_from(rows.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::candidates_order(rows.as_ptr(), count)
        }))
        .map(drop)
    }
}

/// `filter_candidates`（依赖 `read_candidates`）：只在 `candidates` 里，别的钩子里返回 `Err(Error::Denied)`。
pub mod filter {
    use super::{check, sys, Error};

    /// 把 `rows` 里的行降到末尾并隐藏。
    pub fn hide(rows: &[u32]) -> Result<(), Error> {
        let count = i32::try_from(rows.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::candidates_hide(rows.as_ptr(), count)
        }))
        .map(drop)
    }
}

/// `annotate_candidates`（依赖 `read_candidates`）：只在 `candidates` 里，别的钩子里返回 `Err(Error::Denied)`。
pub mod annotate {
    use super::{check, sys, Error};

    /// 给第 `index` 行加注释。
    pub fn set(index: u32, text: &str) -> Result<(), Error> {
        let len = i32::try_from(text.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::candidates_annotate(index as i32, text.as_ptr(), len)
        }))
        .map(drop)
    }
}

/// `provide_commands`：在自己声明的 `/` 指令里给出候选。清单 `[[script.commands]]` 声明触发词，`hooks` 里要有 `command`；只在 `command` 里有效。
pub mod command {
    use super::{cap, check, sys, text as decoded, Error};

    /// 用户打的触发词，即清单里的某个 `trigger`。
    pub const TRIGGER: i32 = 0;
    /// 触发词后面用户打的小写字母，可以为空（`/uuid`），最多 64 字节。
    pub const ARGUMENT: i32 = 1;

    /// 复制 `TRIGGER` 或 `ARGUMENT` 到 `buffer`。别的钩子里是 `Err(Error::Absent)`。
    pub fn text(field: i32, buffer: &mut [u8]) -> Result<&str, Error> {
        let code = unsafe { sys::command_text(field, buffer.as_mut_ptr(), cap(buffer)) };
        decoded(code, buffer)
    }

    /// 给出一行候选，返回它的行号。`text` 是选中时上屏的文本：1 到 64 个 UTF-16 码元，首尾不能是空白，不能含控制、格式和不可见字符，也不能含 `://`、`www.`、`mailto:`。`note` 是画在插件注释格里的说明，最多 32 个 UTF-16 码元，可以为空。第 10 行或太长是 `Err(Error::Limit)`，不合规是 `Err(Error::Invalid)`。钩子正常返回之后这些行才会显示。
    pub fn row(text: &str, note: &str) -> Result<u64, Error> {
        let text_len = i32::try_from(text.len()).map_err(|_| Error::Limit)?;
        let note_len = i32::try_from(note.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::command_row(text.as_ptr(), text_len, note.as_ptr(), note_len)
        }))
    }
}

/// 不需要能力：随机数。每次 `command` 事件有 64 字节，按调用顺序取；别的钩子里没有。
pub mod random {
    use super::{check, sys, Error};

    /// 用接下来的 `buffer.len()` 个随机字节填满 `buffer`；剩下的不够时是 `Err(Error::Limit)`，什么都不写。
    pub fn fill(buffer: &mut [u8]) -> Result<(), Error> {
        let len = i32::try_from(buffer.len()).map_err(|_| Error::Limit)?;
        check(i64::from(unsafe {
            sys::random_fill(buffer.as_mut_ptr(), len)
        }))
        .map(drop)
    }
}

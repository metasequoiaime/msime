// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! 水杉输入法引擎的 C ABI：把核心层 `msime_engine::Session` 包成带版本的 C 函数，符号一律以 `msime_engine_` 开头，头文件是 cbindgen 生成并提交的 `include/msime_engine.h`。
//!
//! 设计与取舍见 `.agents/notes/implemented/architecture/2026-10-29-engine-capi.md`，给 C 采用方的用法（生命周期、按键路由、线程、内存、要带的数据文件、许可证）见 crate 的 README。这里只包核心层，不包 `host` 门面，也不扩展 host-api。

// 工作区禁止 unsafe；这个 crate 就是 C 采用方链接的边界，收进来的每个指针、长度和句柄都是编译器查不到的约定。豁免写在这里而不是退出工作区的 lint 表，和其他 FFI crate 一样，免得以后工作区加的 lint 悄悄漏掉这里。
#![allow(unsafe_code)]
// 类型名与 C 头文件里的一致（`msime_engine_str`），cbindgen 原样导出。
#![allow(non_camel_case_types)]

mod abi;
mod library;
mod options;
mod session;
#[cfg(test)]
mod tests;

pub use abi::{
    msime_engine_candidate, msime_engine_paths, msime_engine_result, msime_engine_state,
    msime_engine_str,
};
pub use library::*;
pub use session::*;

// ---- 版本 ----

/// ABI 主版本。同一个主版本内只追加函数、常量和结构体尾部的字段，不改已有的含义；Linux 的 SONAME 带的就是它（`libmsime_engine.so.1`）。
pub const MSIME_ENGINE_ABI_MAJOR: u32 = 1;
/// ABI 次版本，每次追加加一。调用方可以按 `msime_engine_abi_version()` 判断运行时的库有没有某个后加的函数。
pub const MSIME_ENGINE_ABI_MINOR: u32 = 1;

// ---- 返回码 ----
//
// 除了 `msime_engine_abi_version`、`msime_engine_version`、`msime_engine_last_error` 和 `msime_engine_string_free`，每个函数都返回 `int32_t` 状态码。非零时 `msime_engine_last_error()` 给出说明文字。编号冻结，只追加；1、2、3、6 与引擎的 `ErrorCode` 共用一个空间。

/// 成功。
pub const MSIME_ENGINE_OK: i32 = 0;
/// 参数不合法：选项 JSON 有未知键或取值不对、索引越界、不是 UTF-8、`struct_size` 比 1.0 的布局还小等。
pub const MSIME_ENGINE_INVALID_ARGUMENT: i32 = 1;
/// 引擎没能完成操作（例如换到一个没有词库的方案）。
pub const MSIME_ENGINE_FAILED: i32 = 2;
/// SQLite 或文件系统出错。
pub const MSIME_ENGINE_STORAGE: i32 = 3;
/// 这个库不认识的选项编号、模型种类或标志位。
pub const MSIME_ENGINE_UNSUPPORTED: i32 = 4;
/// 传入的 `revision` 不是会话当前的候选修订号：候选已经变了，先重新 `msime_engine_state_get`。
pub const MSIME_ENGINE_STALE: i32 = 5;
/// 会话正被另一个线程调用（同一会话的调用不能并发），或者词库正被别的会话、别的进程占着（维护要等所有会话关掉）。什么也没做，稍后重试。
pub const MSIME_ENGINE_BUSY: i32 = 6;
/// 之前的某次调用在库里 panic 了，会话已作废，只能 `msime_engine_session_free`。进程级函数 panic 时也返回它，但不作废任何东西。
pub const MSIME_ENGINE_PANIC: i32 = 7;
/// 必需的指针参数是 NULL。
pub const MSIME_ENGINE_NULL: i32 = 8;

const _: () = {
    use msime_engine::ErrorCode;
    assert!(ErrorCode::InvalidArgument.value() == MSIME_ENGINE_INVALID_ARGUMENT as u32);
    assert!(ErrorCode::Failed.value() == MSIME_ENGINE_FAILED as u32);
    assert!(ErrorCode::Storage.value() == MSIME_ENGINE_STORAGE as u32);
    assert!(ErrorCode::Busy.value() == MSIME_ENGINE_BUSY as u32);
};

// ---- 方案（`SchemeType` 的序号，冻结） ----

pub const MSIME_ENGINE_SCHEME_QUANPIN: u32 = 0;
pub const MSIME_ENGINE_SCHEME_SHUANGPIN: u32 = 1;
pub const MSIME_ENGINE_SCHEME_WUBI: u32 = 2;
pub const MSIME_ENGINE_SCHEME_JAPANESE: u32 = 3;
pub const MSIME_ENGINE_SCHEME_KOREAN: u32 = 4;
pub const MSIME_ENGINE_SCHEME_CANTONESE: u32 = 5;
pub const MSIME_ENGINE_SCHEME_ZHUYIN: u32 = 6;
pub const MSIME_ENGINE_SCHEME_VIETNAMESE: u32 = 7;
pub const MSIME_ENGINE_SCHEME_TIBETAN: u32 = 8;
pub const MSIME_ENGINE_SCHEME_STROKE: u32 = 9;

// ---- 命令（核心层 `Command` 的序号，冻结；与 host-api 的命令编号不同） ----

pub const MSIME_ENGINE_COMMAND_BACKSPACE: u32 = 0;
/// 上屏首选（引擎顺序的第 0 行）。宿主自己高亮了别的行时用 `msime_engine_select`。
pub const MSIME_ENGINE_COMMAND_COMMIT_CANDIDATE: u32 = 1;
/// 上屏按键原文（回车）。专用英文里打的词学进英文词库；选项 `english.learn_typed_spellings_on_enter` 打开时，临时英文的字母和拼不成音节的拼音也学。
pub const MSIME_ENGINE_COMMAND_COMMIT_RAW: u32 = 2;
/// 丢弃组字（Esc）。越南文和藏文的第一次只把显示切回按键原文，再发一次才丢弃；开着可打开的候选列表（韩文汉字）时第一次只关列表。
pub const MSIME_ENGINE_COMMAND_CANCEL: u32 = 3;
pub const MSIME_ENGINE_COMMAND_MOVE_LEFT: u32 = 4;
pub const MSIME_ENGINE_COMMAND_MOVE_RIGHT: u32 = 5;
pub const MSIME_ENGINE_COMMAND_MOVE_HOME: u32 = 6;
pub const MSIME_ENGINE_COMMAND_MOVE_END: u32 = 7;
pub const MSIME_ENGINE_COMMAND_DELETE_FORWARD: u32 = 8;
/// 只用于日文：切换假名写法。
pub const MSIME_ENGINE_COMMAND_CYCLE_KANA_VARIANT: u32 = 9;
/// 只用于日文：上屏假名读音。
pub const MSIME_ENGINE_COMMAND_COMMIT_READING: u32 = 10;
/// 打开或关闭当前方案可打开的候选列表：韩文当前音节的汉字、注音整句的其他转换。
pub const MSIME_ENGINE_COMMAND_OPEN_CANDIDATE_LIST: u32 = 11;
/// 整句改字的光标左移一个字；进不了改字时与 `MOVE_LEFT` 相同。
pub const MSIME_ENGINE_COMMAND_CONVERSION_LEFT: u32 = 12;
/// 整句改字的光标右移一个字；不在改字里时与 `MOVE_RIGHT` 相同。
pub const MSIME_ENGINE_COMMAND_CONVERSION_RIGHT: u32 = 13;
/// ABI 1.1 起：与 `COMMIT_RAW` 上屏同样的原文，但不学英文词（例如宿主的 Shift+回车）。1.0 的库对它返回 `MSIME_ENGINE_UNSUPPORTED`。
pub const MSIME_ENGINE_COMMAND_COMMIT_RAW_WITHOUT_LEARNING: u32 = 14;

// ---- 候选来源（`CandidateSource` 的序号，冻结） ----
//
// 不进 ABI 的能力（云与 AI 候选、插件、联系人、多语言）的来源也保留编号，这个 ABI 不会产生它们；10 从未发布过。

pub const MSIME_ENGINE_SOURCE_DATABASE: u32 = 0;
pub const MSIME_ENGINE_SOURCE_USER_DATABASE: u32 = 1;
pub const MSIME_ENGINE_SOURCE_CLOUD_SUGGESTION: u32 = 2;
pub const MSIME_ENGINE_SOURCE_AI_SUGGESTION: u32 = 3;
pub const MSIME_ENGINE_SOURCE_ENGLISH_DICTIONARY: u32 = 4;
pub const MSIME_ENGINE_SOURCE_QUICK_PHRASE: u32 = 5;
/// emoji 行，也用于打拼音出符号的符号行：选中时整段上屏，不学习。
pub const MSIME_ENGINE_SOURCE_EMOJI: u32 = 6;
pub const MSIME_ENGINE_SOURCE_KAOMOJI: u32 = 7;
/// 词格解出的整句。
pub const MSIME_ENGINE_SOURCE_GENERATED: u32 = 8;
/// 解码器给不出词时的原文或兜底行。
pub const MSIME_ENGINE_SOURCE_FALLBACK: u32 = 9;
pub const MSIME_ENGINE_SOURCE_NEURAL_KEYBOARD: u32 = 11;
pub const MSIME_ENGINE_SOURCE_AI_PENDING: u32 = 12;
pub const MSIME_ENGINE_SOURCE_MIXED_JAPANESE: u32 = 13;
pub const MSIME_ENGINE_SOURCE_PLUGIN: u32 = 14;
pub const MSIME_ENGINE_SOURCE_CLOUD_PENDING: u32 = 15;
pub const MSIME_ENGINE_SOURCE_CONTACT: u32 = 16;
pub const MSIME_ENGINE_SOURCE_COLLOCATION: u32 = 17;
pub const MSIME_ENGINE_SOURCE_MULTILINGUAL: u32 = 18;

// ---- 本地模式（`msime_engine_state::local_mode`，冻结） ----

pub const MSIME_ENGINE_LOCAL_MODE_NONE: u32 = 0;
pub const MSIME_ENGINE_LOCAL_MODE_UNICODE: u32 = 1;
pub const MSIME_ENGINE_LOCAL_MODE_DATE_TIME: u32 = 2;
pub const MSIME_ENGINE_LOCAL_MODE_QUICK_PHRASE: u32 = 3;
pub const MSIME_ENGINE_LOCAL_MODE_EMOJI: u32 = 4;
pub const MSIME_ENGINE_LOCAL_MODE_KAOMOJI: u32 = 5;
pub const MSIME_ENGINE_LOCAL_MODE_SUPER_JIANPIN: u32 = 6;
pub const MSIME_ENGINE_LOCAL_MODE_TEMPORARY_ENGLISH: u32 = 7;
pub const MSIME_ENGINE_LOCAL_MODE_TEMPORARY_JAPANESE: u32 = 8;
pub const MSIME_ENGINE_LOCAL_MODE_EXPRESSION: u32 = 9;
pub const MSIME_ENGINE_LOCAL_MODE_COMMAND: u32 = 10;
pub const MSIME_ENGINE_LOCAL_MODE_MENTION: u32 = 11;
pub const MSIME_ENGINE_LOCAL_MODE_URL: u32 = 12;
pub const MSIME_ENGINE_LOCAL_MODE_CHAIZI: u32 = 13;
pub const MSIME_ENGINE_LOCAL_MODE_SYMBOL: u32 = 14;

// ---- 标志位 ----

/// `msime_engine_key_char` 的 `flags`：这是单独按下的 Shift 加字母（大写字母开本地模式的入口）。
pub const MSIME_ENGINE_KEY_SHIFT_ONLY: u32 = 1 << 0;

/// `msime_engine_state::flags`：候选是上屏后的联想，只接受显式选择，空格、回车不上屏它们。
pub const MSIME_ENGINE_STATE_PREDICTION: u32 = 1 << 0;
/// `msime_engine_state::flags`：专用英文模式开着。
pub const MSIME_ENGINE_STATE_DEDICATED_ENGLISH: u32 = 1 << 1;
/// `msime_engine_state::flags`：方案可打开的候选列表（韩文汉字）正开着。
pub const MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN: u32 = 1 << 2;
/// `msime_engine_state::flags`：此刻的 `'` 是分隔符（单位与目标之间、`/fy` 的词之间），要按字符交给 `msime_engine_key_char`，不能当标点结束组字。
pub const MSIME_ENGINE_STATE_TAKES_SEPARATOR: u32 = 1 << 3;

/// `msime_engine_candidate::flags`：按模糊音读出的行。
pub const MSIME_ENGINE_CANDIDATE_FUZZY: u32 = 1 << 0;
/// `msime_engine_candidate::flags`：这一行回答的是整段输入，不是它的前缀或更长的补全。
pub const MSIME_ENGINE_CANDIDATE_ANSWERS_KEY: u32 = 1 << 1;
/// `msime_engine_candidate::flags`：整句联想解出的整句。
pub const MSIME_ENGINE_CANDIDATE_SENTENCE: u32 = 1 << 2;

/// `msime_engine_select_edge` 的 `edge`：上屏这一行的第一个汉字。
pub const MSIME_ENGINE_EDGE_FIRST_HAN: u32 = 0;
/// `msime_engine_select_edge` 的 `edge`：上屏这一行的最后一个汉字。
pub const MSIME_ENGINE_EDGE_LAST_HAN: u32 = 1;

/// `msime_engine_flush_learning` 的 `flags`：写完之后停掉后台写线程并等它结束。卸载库或退出进程前用；之后会话照常可用，下一次学习重新起线程。
pub const MSIME_ENGINE_FLUSH_STOP_THREAD: u32 = 1 << 0;

/// `msime_engine_user_dict_list` 的 `flags`：同时列出随包多字词被学习或编辑过的拼音权重。
pub const MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED: u32 = 1 << 0;

/// `msime_engine_preload_model` 的 `kind`：`msime-japanese.dat`。
pub const MSIME_ENGINE_MODEL_JAPANESE: u32 = 1;

/// `msime_engine_set_caret` 的 `caret_byte` 取这个值（或任何负数）时光标回到末尾。
pub const MSIME_ENGINE_CARET_END: i64 = -1;

// ---- 会话选项编号（`msime_engine_set_int` / `msime_engine_set_string`） ----

/// 整数：中文标点开关，0 或 1。
pub const MSIME_ENGINE_OPTION_CHINESE_PUNCT: u32 = 1;
/// 整数：成对标点开关，0 或 1。
pub const MSIME_ENGINE_OPTION_PAIRED_PUNCT: u32 = 2;
/// 整数：标点锁，0 跟随模式、1 强制中文、2 强制 ASCII。
pub const MSIME_ENGINE_OPTION_PUNCT_LOCK: u32 = 3;
/// 整数：专用英文模式，0 或 1；只在模式真的改变时丢弃组字。
pub const MSIME_ENGINE_OPTION_DEDICATED_ENGLISH: u32 = 4;
/// 整数：辅助码开关（全拼和双拼一起），0 或 1。
pub const MSIME_ENGINE_OPTION_HELPCODE_ENABLED: u32 = 5;
/// 字符串：辅助码表名，例如 `wubi86`；读不到表时 `MSIME_ENGINE_FAILED`。
pub const MSIME_ENGINE_OPTION_HELPCODE_SCHEMA: u32 = 6;
/// 整数：个人上下文学习（词序与搭配），0 或 1；关掉时也结束当前上下文。
pub const MSIME_ENGINE_OPTION_PERSONAL_CONTEXT: u32 = 7;
/// 整数：宿主的即时隐私门禁，0 时清掉上屏后联想、之后不再给出，选项 `next_word_prediction` 本身不变。
pub const MSIME_ENGINE_OPTION_PREDICTION_ALLOWED: u32 = 8;
/// 整数：宿主每页的候选数（1 到 255）；决定全长组的同音词够多少行就不再并入更长的补全词。
pub const MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS: u32 = 9;
/// 字符串：整句模型参照的已上屏文字（只看最后 64 个字），宿主在光标前文字变化时交来。
pub const MSIME_ENGINE_OPTION_RESCORING_CONTEXT: u32 = 10;
/// 整数，ABI 1.1 起：调频方式，`MSIME_ENGINE_FREQUENCY_*`，与选项 `frequency.mode` 相同。从下一次选择起按新方式学，已经学下的排序照旧生效，眼前的候选不重排。
pub const MSIME_ENGINE_OPTION_FREQUENCY_MODE: u32 = 11;
/// 整数，ABI 1.1 起：同一个词要被选中几次才挪位（1 到 10），与选项 `frequency.trigger_count` 相同。
pub const MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT: u32 = 12;
/// 整数，ABI 1.1 起：`MSIME_ENGINE_FREQUENCY_LINEAR` 每次前移几位（1 到 10），与选项 `frequency.linear_step` 相同。
pub const MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP: u32 = 13;
/// 整数，ABI 1.1 起：五笔拼音混输（码表没有时按全拼出字），0 或 1，与选项 `wubi.mixed_pinyin` 相同；五笔组字中切换时立即按新设置重查候选。
pub const MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN: u32 = 14;

// ---- 调频方式（`MSIME_ENGINE_OPTION_FREQUENCY_MODE` 的取值，ABI 1.1 起，冻结） ----

/// 不调频：选词不改变以后的排序（缺省）。
pub const MSIME_ENGINE_FREQUENCY_DISABLED: u32 = 0;
/// 选中的词直接排到首位。
pub const MSIME_ENGINE_FREQUENCY_PIN: u32 = 1;
/// 选中的词挪到原位置的一半处：下标 n 挪到 n/2（下标从 0 数）。
pub const MSIME_ENGINE_FREQUENCY_HALVE: u32 = 2;
/// 选中的词前移 `linear_step` 位。
pub const MSIME_ENGINE_FREQUENCY_LINEAR: u32 = 3;
/// 选中的词前移一位；下标大于 4 的直接挪到下标 4（第 5 位）。
pub const MSIME_ENGINE_FREQUENCY_PROMOTE: u32 = 4;

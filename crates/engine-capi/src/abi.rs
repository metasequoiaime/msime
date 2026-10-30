//! 跨边界的值类型和几条共用的规则：借用字符串、按 `struct_size` 收发结构体、线程局部的错误说明、把 panic 挡在边界内。

use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::mem::{size_of, MaybeUninit};

use msime_engine::EngineError;

use crate::{
    MSIME_ENGINE_INVALID_ARGUMENT, MSIME_ENGINE_NULL, MSIME_ENGINE_OK, MSIME_ENGINE_PANIC,
};

/// 库借给调用方的一段 UTF-8 文本：`len` 个字节，不以 NUL 结尾，按长度读。空串时 `ptr` 也不是 NULL。指向会话里缓存的数据，有效期见各个函数：热路径上的都到同一会话下一次改变状态的调用为止。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct msime_engine_str {
    pub ptr: *const c_char,
    pub len: usize,
}

impl msime_engine_str {
    pub(crate) const EMPTY: Self = Self {
        ptr: c"".as_ptr(),
        len: 0,
    };

    pub(crate) fn borrow(text: &str) -> Self {
        if text.is_empty() {
            return Self::EMPTY;
        }
        Self {
            ptr: text.as_ptr().cast(),
            len: text.len(),
        }
    }
}

/// 一次按键、命令或选择的结果（ABI 1.0）。调用前把 `struct_size` 设为 `sizeof(msime_engine_result)`；库写入 min(调用方大小, 库内大小) 个字节，把 `struct_size` 改成写入的大小，多出的部分清零。`commit` 和 `diagnostic` 借自会话，到同一会话下一次改变状态的调用为止有效。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct msime_engine_result {
    pub struct_size: u32,
    /// 引擎接管了这一键（非零）。为零时宿主按自己的规则处理，例如把字符原样交给应用。
    pub handled: u32,
    /// 有上屏文字（非零），文字在 `commit`；上屏的也可能是空串。
    pub has_commit: u32,
    /// 诊断的稳定数字码（引擎的 `DiagnosticCode`）；0 是没有诊断，0xFFFF 是表外的文字。诊断不是失败：键照样处理了，只是学习没能存下之类。
    pub diagnostic_code: u32,
    /// 调用之后的候选修订号，与 `msime_engine_state::revision` 相同。
    pub revision: u64,
    pub commit: msime_engine_str,
    /// 诊断的说明文字（英文），没有时为空串。
    pub diagnostic: msime_engine_str,
}

/// 组字状态（ABI 1.0），`msime_engine_state_get` 填写，收发规则同 `msime_engine_result`。字符串借自会话按修订号缓存的快照，`msime_engine_state_get`、`msime_engine_candidate_get` 这类只读调用不让它们失效。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct msime_engine_state {
    pub struct_size: u32,
    /// 当前方案，`MSIME_ENGINE_SCHEME_*`。
    pub scheme: u32,
    /// 候选修订号：每次可能改变候选的调用都让它变，从 0 起只增不减。按下标选择和取候选时原样交回。
    pub revision: u64,
    pub candidate_count: u32,
    /// `MSIME_ENGINE_STATE_*` 的组合。
    pub flags: u32,
    /// `MSIME_ENGINE_LOCAL_MODE_*`。
    pub local_mode: u32,
    /// 组字光标在 `editing_text` 里的字节偏移。
    pub caret_byte: u32,
    /// 整句改字的光标，按 `conversion` 的字（Unicode 标量）计：光标在第 `conversion_focus_start_scalar` 个字之前，`[start, end)` 是候选替换的那一段；不在改字时两者都是 0。
    pub conversion_focus_start_scalar: u32,
    pub conversion_focus_end_scalar: u32,
    /// 给用户看的组字：全拼按音节分开（`ni'hao`），双拼默认是按键原文，本地模式与专用英文是按键原文。
    pub preedit: msime_engine_str,
    /// 按键原文（ASCII），`caret_byte` 指向它；为空就是没有在组字。
    pub editing_text: msime_engine_str,
    /// 整句改字时改好的整句，宿主在它非空时把它画在行内代替拼音。
    pub conversion: msime_engine_str,
    /// 此刻要按字符（`msime_engine_key_char`）而不是按标点交来的非字母键，见 README 的按键路由。
    pub spelling_symbols: msime_engine_str,
}

/// 一行候选（ABI 1.0），`msime_engine_candidate_get` 填写，收发规则同 `msime_engine_result`，字符串的有效期同 `msime_engine_state`。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct msime_engine_candidate {
    pub struct_size: u32,
    /// 这一行的下标（就是传进来的那个）。
    pub index: u32,
    /// `MSIME_ENGINE_SOURCE_*`。
    pub source: u32,
    /// 给出这一行的方案，`MSIME_ENGINE_SCHEME_*`（五笔混拼里的拼音行是全拼）。
    pub scheme: u32,
    /// `MSIME_ENGINE_CANDIDATE_*` 的组合。
    pub flags: u32,
    /// 用户固定的位置 1..=5，没固定时 0。
    pub fixed_position: i32,
    pub word: msime_engine_str,
    /// 这一行匹配的编码，按输入的写法（全拼是带 `'` 的音节）。
    pub code: msime_engine_str,
    /// 注释：辅助码或纠错提示，没有时为空串。
    pub annotation: msime_engine_str,
    /// 纠错读出的行：用户实际打的字母；其余为空串。
    pub corrected_from: msime_engine_str,
}

/// 运行时的四个根目录（ABI 1.0），都要是绝对路径，UTF-8。库只读 min(调用方大小, 库内大小) 个字节，读不到的字段按全零处理。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct msime_engine_paths {
    pub struct_size: u32,
    /// 只读的随包资源：词库、辅助码表、模型。
    pub resources: msime_engine_str,
    /// 用户数据：学习日志 `msime_user.db` 和代次目录 `dictionaries/`。
    pub user_data: msime_engine_str,
    /// 可以随时清空的缓存。
    pub cache: msime_engine_str,
    /// 代次目录，`msime_engine_prepare_runtime` 准备并交回；`msime_engine_prepare_runtime` 不读这一项，调用方可以不填。
    pub dictionaries: msime_engine_str,
}

/// 按 `struct_size` 收发的结构体。
pub(crate) trait Sized1: Copy {
    /// ABI 1.0 布局的大小；调用方给的 `struct_size` 不能比它小。在 1.0 之后追加字段时把它固定成 1.0 最后一个字段的结束偏移，不要跟着 `size_of` 变。
    const V1_SIZE: usize;
}

macro_rules! sized1 {
    ($($name:ident),*) => {$(
        impl Sized1 for $name {
            const V1_SIZE: usize = size_of::<$name>();
        }
    )*};
}
sized1!(
    msime_engine_result,
    msime_engine_state,
    msime_engine_candidate,
    msime_engine_paths
);

// 输出结构体按字节整块复制，所以不能有填充：填充字节的值是未定义的。每个断言都是字段大小之和。
const _: () = {
    let word = size_of::<msime_engine_str>();
    assert!(size_of::<msime_engine_result>() == 4 * 4 + 8 + 2 * word);
    assert!(size_of::<msime_engine_state>() == 4 * 2 + 8 + 4 * 6 + 4 * word);
    assert!(size_of::<msime_engine_candidate>() == 4 * 6 + 4 * word);
};

/// 失败的状态码和说明文字。
#[derive(Debug)]
pub(crate) struct Failure {
    pub code: i32,
    pub message: Cow<'static, str>,
}

impl Failure {
    pub(crate) fn new(code: i32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn invalid(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(MSIME_ENGINE_INVALID_ARGUMENT, message)
    }

    pub(crate) fn null(what: &'static str) -> Self {
        Self::new(MSIME_ENGINE_NULL, what)
    }
}

impl From<EngineError> for Failure {
    fn from(error: EngineError) -> Self {
        Self::new(error.code().value() as i32, error.to_string())
    }
}

pub(crate) type Outcome<T = ()> = Result<T, Failure>;

thread_local! {
    /// 本线程上一次失败的说明；成功的调用把它清掉。`None` 时 `msime_engine_last_error` 给空串，按键路径上因此不分配。
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// 本线程的说明文字；有效到本线程下一次调用这个库（`msime_engine_last_error` 本身除外）。
pub(crate) fn last_error() -> *const c_char {
    LAST_ERROR.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(c"".as_ptr(), |message| message.as_ptr())
    })
}

fn set_last_error(message: Option<&str>) {
    let message = message.map(|text| {
        // 说明文字里不会有 NUL；万一有，换掉而不是丢掉整条。
        CString::new(text.replace('\0', "\u{fffd}")).unwrap_or_default()
    });
    LAST_ERROR.with(|slot| *slot.borrow_mut() = message);
}

/// 进入库的每个入口都经这里：清掉上一条说明，把 panic 挡在边界内，失败时记下说明并交回状态码。会话级的 panic 另由 `session::with_session` 作废会话。
pub(crate) fn guard(body: impl FnOnce() -> Outcome) -> i32 {
    set_last_error(None);
    let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(outcome) => outcome,
        Err(payload) => Err(Failure::new(
            MSIME_ENGINE_PANIC,
            panic_message(payload.as_ref()),
        )),
    };
    match outcome {
        Ok(()) => MSIME_ENGINE_OK,
        Err(failure) => {
            set_last_error(Some(&failure.message));
            failure.code
        }
    }
}

pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string payload");
    format!("internal panic: {detail}")
}

/// 借用的输入文本：`(ptr, len)`，UTF-8，不要求 NUL 结尾。`ptr` 为 NULL 时只接受 `len == 0`，当作空串。
///
/// # Safety
/// `ptr` 非空时指向至少 `len` 个可读字节，调用期间不变。
pub(crate) unsafe fn input_str<'a>(
    ptr: *const c_char,
    len: usize,
    what: &'static str,
) -> Outcome<&'a str> {
    if ptr.is_null() {
        return if len == 0 {
            Ok("")
        } else {
            Err(Failure::null(what))
        };
    }
    // SAFETY: 调用方保证 `ptr` 指向 `len` 个可读字节。
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    std::str::from_utf8(bytes).map_err(|_| Failure::invalid(format!("{what} is not UTF-8")))
}

/// 结构体里的一个输入字符串字段。
///
/// # Safety
/// 同 [`input_str`]。
pub(crate) unsafe fn field_str<'a>(
    value: msime_engine_str,
    what: &'static str,
) -> Outcome<&'a str> {
    // SAFETY: 转交给 `input_str`，前提相同。
    unsafe { input_str(value.ptr, value.len, what) }
}

/// 读调用方交来的输入结构体：只读它声称的大小与库内大小中较小的那部分，其余按全零。
///
/// # Safety
/// `input` 非空时按 `T` 对齐，至少可读它开头的 `struct_size` 指出的字节数。
pub(crate) unsafe fn read_sized<T: Sized1>(input: *const T, what: &'static str) -> Outcome<T> {
    if input.is_null() {
        return Err(Failure::null(what));
    }
    // SAFETY: 非空且对齐，开头四个字节是 `struct_size`。
    let caller = unsafe { input.cast::<u32>().read() } as usize;
    if caller < T::V1_SIZE {
        return Err(Failure::invalid(format!(
            "{what}: struct_size is smaller than the ABI 1.0 layout"
        )));
    }
    let mut value = MaybeUninit::<T>::zeroed();
    // SAFETY: 两边都至少有 `min(caller, size_of::<T>())` 个字节；全零对这些 `repr(C)` 类型（整数和裸指针）是合法值。
    unsafe {
        std::ptr::copy_nonoverlapping(
            input.cast::<u8>(),
            value.as_mut_ptr().cast::<u8>(),
            caller.min(size_of::<T>()),
        );
        Ok(value.assume_init())
    }
}

/// 调用方给的输出结构体声明的大小；比 1.0 的布局还小时失败。
///
/// # Safety
/// `output` 非空，按 `T` 对齐，开头四个字节可读。
unsafe fn output_size<T: Sized1>(output: *const T, what: &'static str) -> Outcome<usize> {
    // SAFETY: 调用方的前提，开头四个字节是调用方填的 `struct_size`。
    let caller = unsafe { output.cast::<u32>().read() } as usize;
    if caller < T::V1_SIZE {
        return Err(Failure::invalid(format!(
            "{what}: struct_size is smaller than the ABI 1.0 layout"
        )));
    }
    Ok(caller)
}

/// 改变会话之前核对可选的输出结构体（NULL 表示调用方不要结果）：`struct_size` 太小的调用在这里就被拒绝，会话原样不动，不会出现键已经处理、上屏文字却交不回去的情况。
///
/// # Safety
/// `output` 是 NULL，或按 `T` 对齐、开头四个字节可读。
pub(crate) unsafe fn check_output<T: Sized1>(output: *const T, what: &'static str) -> Outcome {
    if output.is_null() {
        return Ok(());
    }
    // SAFETY: 调用方的前提。
    unsafe { output_size(output, what) }.map(|_| ())
}

/// 把输出结构体写给调用方：写 min(调用方大小, 库内大小) 个字节，`struct_size` 改成写入的大小，调用方多出的部分清零。
///
/// # Safety
/// `output` 非空时按 `T` 对齐，开头的 `struct_size` 是调用方这块内存的真实大小，整块可写。
pub(crate) unsafe fn write_sized<T: Sized1>(
    output: *mut T,
    mut value: T,
    what: &'static str,
) -> Outcome {
    if output.is_null() {
        return Err(Failure::null(what));
    }
    // SAFETY: 非空且对齐，调用方的前提。
    let caller = unsafe { output_size(output, what) }?;
    let written = caller.min(size_of::<T>());
    // SAFETY: 每个输出类型都以 `u32 struct_size` 开头（`repr(C)`），这里改写的就是它。
    unsafe {
        std::ptr::addr_of_mut!(value)
            .cast::<u32>()
            .write(written as u32)
    };
    // SAFETY: 输出类型没有填充（见上面的断言），按字节读 `value` 不会读到未初始化的内存；调用方保证 `caller` 个字节可写。
    unsafe {
        std::ptr::copy_nonoverlapping(
            std::ptr::addr_of!(value).cast::<u8>(),
            output.cast::<u8>(),
            written,
        );
        if caller > written {
            std::ptr::write_bytes(output.cast::<u8>().add(written), 0, caller - written);
        }
    }
    Ok(())
}

/// 交给调用方、由 `msime_engine_string_free` 释放的字符串。
pub(crate) fn owned_string(text: String) -> *mut c_char {
    CString::new(text.replace('\0', "\u{fffd}"))
        .unwrap_or_default()
        .into_raw()
}

/// 写给调用方的 `char**` 输出参数。
///
/// # Safety
/// `output` 非空时可写一个指针。
pub(crate) unsafe fn write_owned(output: *mut *mut c_char, text: String) {
    if !output.is_null() {
        // SAFETY: 调用方保证可写。
        unsafe { output.write(owned_string(text)) };
    }
}

//! 资源限制的初值（设计 §4.4），第 3 阶段在真机上标定后再调。
//!
//! fuel 和内存都是每个插件固定配额：一个插件能拿到多少，与其他插件做了什么无关，所以不设先到先得的总池。fuel 是唯一的硬限制，wasmi 没有 epoch，"硬超时"指的就是 fuel。

use crate::abi::Hook;

/// 每个插件的初始化：`msime_abi` 加 `msime_init`。
pub const INIT_FUEL: u64 = 5_000_000;
/// 每次 focus 之后所有插件初始化的合计。超出的插件推迟到下一轮再实例化，不算失败；由宿主一侧记账。
pub const FOCUS_INIT_FUEL: u64 = 5_000_000;
/// 每个插件每一轮（宿主的一次 `msime_client_key` 到下一次之间，跨同一轮内的多次 FFI），`before_commit` 和 `command` 之外的钩子合计。
pub const TURN_FUEL: u64 = 125_000;
/// 单次 `candidates` 的上限。
pub const CANDIDATES_FUEL: u64 = 125_000;
/// 单次 `before_commit` 的上限，也是每个插件每一轮 `before_commit` 的合计。它和 [`TURN_FUEL`] 分开记：快照上的改写能用多少 fuel 不能取决于持久实例这一轮用掉了多少，所以一个插件一轮最多用两者之和。
pub const BEFORE_COMMIT_FUEL: u64 = 100_000;
/// 单次 `command` 的上限，也是每个插件每一轮 `command` 的合计。和 [`BEFORE_COMMIT_FUEL`] 同一个理由单独记：`command` 跑在快照上，同一轮里 `input` 等观察类钩子先花掉多少 fuel，不该决定用户主动打开的指令里有没有候选；它也不扣其余钩子的配额。一个插件一轮最多用三份之和。
pub const COMMAND_FUEL: u64 = 100_000;
/// 其余观察类钩子单次的上限。
pub const OBSERVE_FUEL: u64 = 25_000;

/// import 每次调用先扣的 fuel。
pub const IMPORT_CALL_FUEL: u64 = 100;
/// import 每复制这么多字节再扣 1。读写数据文件和存储也按它计。
pub const IMPORT_BYTES_PER_FUEL: u64 = 4;

/// 单个钩子一次最多能用多少 fuel。
pub fn hook_fuel(hook: Hook) -> u64 {
    match hook {
        Hook::Candidates => CANDIDATES_FUEL,
        Hook::BeforeCommit => BEFORE_COMMIT_FUEL,
        Hook::Command => COMMAND_FUEL,
        _ => OBSERVE_FUEL,
    }
}

/// wasm 的页大小。
pub const PAGE_BYTES: usize = 65_536;
/// 清单没写 `memory_pages` 时的线性内存页数。
pub const DEFAULT_MEMORY_PAGES: u32 = 16;
/// `memory_pages` 的上限：64 页，4 MiB。
pub const MAX_MEMORY_PAGES: u32 = 64;
/// 每张表的元素上限。wasmi 的 `StoreLimits::table_elements` 按张分别限制，不是全部表的合计：一个实例最多 [`MAX_TABLES`] 张，合计至多 40 000 个元素。wasmi 每个元素 4 字节，最坏约 160 KiB，不计入内存总账。
pub const MAX_TABLE_ELEMENTS: usize = 10_000;
/// 每个实例最多几张表（reference-types 允许多张）。
pub const MAX_TABLES: usize = 4;

/// 解释器值栈（字节）。每个模块有自己的 Engine，各留一份。
pub const MAX_STACK_HEIGHT: usize = 65_536;
/// 最大递归深度。
pub const MAX_RECURSION_DEPTH: usize = 256;
/// 留着复用的栈数。
pub const MAX_CACHED_STACKS: usize = 1;

/// 模块文件的上限。按专门构造的模块实测的翻译膨胀定（见 [`TRANSLATED_BYTES_PER_MODULE_BYTE`]）：最大的模块带 64 页内存，在总账里约 36 MiB，单独启用放得下。
pub const MAX_MODULE_BYTES: usize = 256 * 1024;
/// 同时启用的脚本数。
pub const MAX_ENABLED_SCRIPTS: usize = 8;
/// 候选类事件里交给插件的行数上限（窗口内前 20 行）。
pub const MAX_CANDIDATE_ROWS: usize = 20;

/// 插件给一个候选加的注释最多几个 UTF-16 码元。`command_row` 的注释同一个上限。
pub const MAX_ANNOTATION_UTF16: usize = 32;

/// 一次 `command` 最多暂存几行：候选窗的一页。
pub const MAX_COMMAND_ROWS: usize = 9;
/// `command_row` 一行文本最多几个 UTF-16 码元。比 Windows 候选管道的 199 严得多，GUID 写法是 38 个：行越短，宿主在行尾截断、让「显示的」和「上屏的」不一样的机会越小。
pub const MAX_COMMAND_ROW_UTF16: usize = 64;
/// 每次 `command` 事件给插件的随机字节数。UUID v4 要 16 个。
pub const RANDOM_BYTES_PER_EVENT: usize = 64;
/// `command` 事件的参数（触发词后面的字母）最多几个字节，超过的不投递（[`crate::ScriptInstance::deliver`] 返回 `NotSubscribed`）。引擎的本地模式输入最多 64 字节，参数本来到不了这个长度，这里是第二道。
pub const MAX_COMMAND_ARGUMENT_BYTES: usize = 64;
/// `command` 事件的触发词最多几个字节，同清单的触发词规则（2 到 16 个小写字母）。
pub const MAX_COMMAND_TRIGGER_BYTES: usize = 16;
/// 一个插件每次 `candidates` 最多用 `candidates_insert` 插几行。
pub const MAX_INSERTED_ROWS_PER_PLUGIN: usize = 3;
/// 所有插件合计每次最多插几行，宿主按执行顺序截断。
pub const MAX_INSERTED_ROWS: usize = 6;

/// 清单 `[[script.settings]]` 最多几项。
pub const MAX_SETTINGS: usize = 16;
/// 清单 `[script].data` 最多几个文件。
pub const MAX_DATA_FILES: usize = 4;
/// 单个数据文件的上限，和单词本包的单文件上限一致。
pub const MAX_DATA_FILE_BYTES: usize = 4 * 1024 * 1024;
/// 一个包的数据文件合计的上限。
pub const MAX_DATA_TOTAL_BYTES: usize = 8 * 1024 * 1024;
/// `persist_data` 的存储上限。
pub const MAX_STORAGE_BYTES: usize = 64 * 1024;

/// 改写后的 body 最多几个 UTF-8 字节。
pub const MAX_REWRITE_BYTES: usize = 4096;
/// 改写后的 body 最多是原文 UTF-16 码元数的几倍。
pub const REWRITE_GROWTH: usize = 4;
/// 原文很短时改写后 body 的 UTF-16 码元上限的下限，让一个字也能改写成一小段话。
pub const MIN_REWRITE_UTF16: usize = 256;

/// 原文 body 有 `original_utf16` 个 UTF-16 码元时，改写结果最多几个：原文的 [`REWRITE_GROWTH`] 倍，至少 [`MIN_REWRITE_UTF16`]。
pub fn rewrite_utf16_limit(original_utf16: usize) -> usize {
    original_utf16
        .saturating_mul(REWRITE_GROWTH)
        .max(MIN_REWRITE_UTF16)
}

/// 进程里全部脚本合计的内存总账：线性内存预留、翻译后的字节码和解释器值栈。授权和启用时按它做准入检查，超出就拒绝启用，而不是让脚本在运行时争抢。
pub const PROCESS_MEMORY_BYTES: usize = 64 * 1024 * 1024;
/// 编译结果（翻译后的代码、模块里的各种表，以及 Engine 留着复用的翻译缓冲）相对模块大小的倍数的上界。普通代码约 3.3 倍；专门构造的模块实测到约 110 倍（`block` 嵌套每层 3 字节，翻译缓冲却要约 330 字节），取 128。产品里量不到真实的编译结果（库不能替宿主进程装全局分配器），准入检查和内存总账都按它估算，`tests::expansion` 守着它确实是上界。
pub const TRANSLATED_BYTES_PER_MODULE_BYTE: usize = 128;
/// 每个模块与大小无关的固定开销：自己的 Engine、链接表和模块头约 10.6 KiB，一个声明了 3 万个局部变量的函数让翻译缓冲再留约 370 KiB，扣掉按倍数已经算进去的部分，实测最多差约 122 KiB，取 256 KiB。
pub const MODULE_OVERHEAD_BYTES: usize = 256 * 1024;

/// 一个编译好的模块在总账里占的字节，与有几个实例无关：翻译结果的上界、固定开销和它的 Engine 留着复用的一份值栈。
pub fn module_memory_bytes(module_bytes: usize) -> usize {
    module_bytes
        .saturating_mul(TRANSLATED_BYTES_PER_MODULE_BYTE)
        .saturating_add(MODULE_OVERHEAD_BYTES)
        .saturating_add(MAX_STACK_HEIGHT)
}

/// 一个脚本在总账里占的字节：一次预留的线性内存，加 [`module_memory_bytes`]。
pub fn script_memory_bytes(module_bytes: usize, memory_pages: u32) -> usize {
    (memory_pages as usize)
        .saturating_mul(PAGE_BYTES)
        .saturating_add(module_memory_bytes(module_bytes))
}

/// 一组脚本（每项是模块字节数和 `memory_pages`）合计放不放得进总账。
pub fn fits_memory_budget(scripts: impl IntoIterator<Item = (usize, u32)>) -> bool {
    fits_memory_budget_with_resident(
        scripts
            .into_iter()
            .map(|(module_bytes, memory_pages)| (module_bytes, memory_pages, 0)),
    )
}

/// 和模块无关、原样常驻内存的字节：随包数据文件按 1 倍记（不走模块的 [`TRANSLATED_BYTES_PER_MODULE_BYTE`] 倍，数据不会被翻译），申请了 `persist_data` 的再加一整块 [`MAX_STORAGE_BYTES`] 的存储缓冲。
pub fn resident_bytes(data_bytes: usize, persists: bool) -> usize {
    data_bytes.saturating_add(if persists { MAX_STORAGE_BYTES } else { 0 })
}

/// 同 [`fits_memory_budget`]，每项另带 [`resident_bytes`]。
pub fn fits_memory_budget_with_resident(
    scripts: impl IntoIterator<Item = (usize, u32, usize)>,
) -> bool {
    scripts
        .into_iter()
        .map(|(module_bytes, memory_pages, resident)| {
            script_memory_bytes(module_bytes, memory_pages).saturating_add(resident)
        })
        .fold(0, usize::saturating_add)
        <= PROCESS_MEMORY_BYTES
}

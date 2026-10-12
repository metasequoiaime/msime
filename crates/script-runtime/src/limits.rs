//! 资源限制的初值（设计 §4.4），第 3 阶段在真机上标定后再调。
//!
//! fuel 和内存都是每个插件固定配额：一个插件能拿到多少，与其他插件做了什么无关，所以不设先到先得的总池。fuel 是唯一的硬限制，wasmi 没有 epoch，"硬超时"指的就是 fuel。

use crate::abi::Hook;

/// 每个插件的初始化：`msime_abi` 加 `msime_init`。
pub const INIT_FUEL: u64 = 5_000_000;
/// 每次 focus 之后所有插件初始化的合计。超出的插件推迟到下一轮再实例化，不算失败；由宿主一侧记账。
pub const FOCUS_INIT_FUEL: u64 = 5_000_000;
/// 每个插件每一轮（宿主的一次 `msime_client_key` 到下一次之间，跨同一轮内的多次 FFI），`before_commit` 之外的钩子合计。
pub const TURN_FUEL: u64 = 125_000;
/// 单次 `candidates` 的上限。
pub const CANDIDATES_FUEL: u64 = 125_000;
/// 单次 `before_commit` 的上限，也是每个插件每一轮 `before_commit` 的合计。它和 [`TURN_FUEL`] 分开记：快照上的改写能用多少 fuel 不能取决于持久实例这一轮用掉了多少，所以一个插件一轮最多用两者之和。
pub const BEFORE_COMMIT_FUEL: u64 = 100_000;
/// 其余观察类钩子单次的上限。
pub const OBSERVE_FUEL: u64 = 25_000;

/// import 每次调用先扣的 fuel。
pub const IMPORT_CALL_FUEL: u64 = 100;
/// import 每复制这么多字节再扣 1。
pub const IMPORT_BYTES_PER_FUEL: u64 = 4;

/// 单个钩子一次最多能用多少 fuel。
pub fn hook_fuel(hook: Hook) -> u64 {
    match hook {
        Hook::Candidates => CANDIDATES_FUEL,
        Hook::BeforeCommit => BEFORE_COMMIT_FUEL,
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

/// 插件给一个候选加的注释最多几个 UTF-16 码元。
pub const MAX_ANNOTATION_UTF16: usize = 32;
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
    scripts
        .into_iter()
        .map(|(module_bytes, memory_pages)| script_memory_bytes(module_bytes, memory_pages))
        .fold(0, usize::saturating_add)
        <= PROCESS_MEMORY_BYTES
}

//! 写死的 wasmi `Config`、模块校验和编译。导入时（client-services、pack-tool）和加载时（输入法进程）走同一份校验。

use crate::abi::{self, Capability, CapabilityRuleError, CapabilitySet, HookSet, ImportSpec, Ty};
use crate::instance::{linker, HostState};
use crate::limits;
use std::sync::Arc;
use wasmi::{CompilationMode, Config, Engine, ExternType, FuncType, Linker, Module, ValType};

/// ABI v1 的 `Config`，不对外开放任何开关。
///
/// - `Eager`：默认的 `LazyTranslation` 把首次调用时的翻译成本计进当次 fuel，会误判失败、造成延迟尖峰；编译在后台线程一次做完。
/// - 显式关掉默认开着的 multi-memory、tail-call、extended-const。memory64 和 SIMD 由 wasmi 的同名 cargo 特性控制，本 crate 不开，它们在 `Config` 里随之关闭（没有开关可调）；测试里各有一个被拒的模块守着，依赖图里哪天有人打开了这两个特性就会变红。
/// - GC_TYPES 保持默认开启：reference-types 需要它，GC 提案本身不开。
/// - 不允许 start 函数：初始化只走 `msime_init`，它受 fuel 约束、返回值可检查；`before_commit` 的快照复位要重新实例化，start 函数会在每次复位时再跑一遍。
fn config() -> Config {
    let mut config = Config::default();
    config
        .compilation_mode(CompilationMode::Eager)
        .wasm_multi_memory(false)
        .wasm_tail_call(false)
        .wasm_extended_const(false)
        .wasm_wide_arithmetic(false)
        .wasm_custom_page_sizes(false)
        .allow_start_fn(false)
        .consume_fuel(true)
        .set_max_stack_height(limits::MAX_STACK_HEIGHT)
        .set_max_recursion_depth(limits::MAX_RECURSION_DEPTH)
        .set_max_cached_stacks(limits::MAX_CACHED_STACKS);
    config
}

/// 清单里和模块有关的声明，由解析清单的一方（client-services 的 `script_pack`）填好交进来。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// 清单 `permissions`。
    pub capabilities: CapabilitySet,
    /// 清单 `[script].hooks`。
    pub hooks: HookSet,
    /// 清单 `[script].memory_pages`，默认 [`limits::DEFAULT_MEMORY_PAGES`]。
    pub memory_pages: u32,
}

/// 模块不符合 ABI v1。`reason()` 是封闭的错误码，可以写进诊断；`Display` 给作者看，可能带 import 名。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleError {
    /// 超过 [`limits::MAX_MODULE_BYTES`]。
    TooLarge,
    /// 不是 wasm 二进制（文本格式也不接受）。
    NotBinary,
    /// `memory_pages` 不在 1..=64。
    MemoryPages,
    /// 能力组合违反依赖或互斥规则。
    Capabilities(CapabilityRuleError),
    /// 清单没声明任何钩子。
    NoHooks,
    /// wasmi 校验或翻译失败：格式错误、用了被拒的提案、带 start 函数、超出实现限制。
    Invalid(String),
    /// 没有导出名为 `memory` 的线性内存。
    MissingMemory,
    /// 线性内存的声明不合规：64 位，或者最小页数超过 `memory_pages`。
    BadMemory,
    /// 缺少必需的导出函数。
    MissingExport(&'static str),
    /// 导出函数的签名不对。
    ExportSignature(&'static str),
    /// import 不在 `msime` 白名单里（包括导入内存、表和 global）。
    ImportNotAllowed { module: String, name: String },
    /// 白名单里的 import 签名不对。
    ImportSignature(&'static str),
    /// 导入了某个函数却没申请它所属的能力。
    ImportWithoutCapability {
        import: &'static str,
        capability: Capability,
    },
    /// 申请了某个能力却没导入任何属于它的函数。
    UnusedCapability(Capability),
}

impl ModuleError {
    /// 封闭的错误码，不带模块里的任何字符串。
    pub fn reason(&self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::NotBinary => "not_binary",
            Self::MemoryPages => "memory_pages",
            Self::Capabilities(_) => "capabilities",
            Self::NoHooks => "no_hooks",
            Self::Invalid(_) => "invalid",
            Self::MissingMemory => "missing_memory",
            Self::BadMemory => "bad_memory",
            Self::MissingExport(_) => "missing_export",
            Self::ExportSignature(_) => "export_signature",
            Self::ImportNotAllowed { .. } => "import_not_allowed",
            Self::ImportSignature(_) => "import_signature",
            Self::ImportWithoutCapability { .. } => "import_without_capability",
            Self::UnusedCapability(_) => "unused_capability",
        }
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(
                f,
                "module is larger than {} bytes",
                limits::MAX_MODULE_BYTES
            ),
            Self::NotBinary => f.write_str("module is not a WebAssembly binary"),
            Self::MemoryPages => write!(
                f,
                "memory_pages must be between 1 and {}",
                limits::MAX_MEMORY_PAGES
            ),
            Self::Capabilities(error) => error.fmt(f),
            Self::NoHooks => f.write_str("script declares no hooks"),
            Self::Invalid(message) => write!(f, "module is not valid for ABI v1: {message}"),
            Self::MissingMemory => f.write_str("module does not export a memory named `memory`"),
            Self::BadMemory => {
                f.write_str("memory must be 32-bit and its minimum must fit memory_pages")
            }
            Self::MissingExport(name) => write!(f, "module does not export `{name}`"),
            Self::ExportSignature(name) => write!(f, "export `{name}` has the wrong signature"),
            Self::ImportNotAllowed { module, name } => {
                write!(f, "import `{module}.{name}` is not part of ABI v1")
            }
            Self::ImportSignature(name) => {
                write!(f, "import `msime.{name}` has the wrong signature")
            }
            Self::ImportWithoutCapability { import, capability } => write!(
                f,
                "import `msime.{import}` needs permission `{}`",
                capability.as_str()
            ),
            Self::UnusedCapability(capability) => write!(
                f,
                "permission `{}` is requested but no import that uses it is imported",
                capability.as_str()
            ),
        }
    }
}

impl std::error::Error for ModuleError {}

/// 编译好、校验过的模块。内部是 `Arc`，克隆便宜，可以跨线程共享（进程级缓存由后台线程填充）。
///
/// 每个模块有自己的 Engine，链接表也绑在它上面。wasmi 的 Engine 只追加不回收，翻译后的字节码要等 Engine 本身释放才还回去：所有模块共用一个进程级 Engine 时，淘汰掉的旧版本和导入时校验过的模块会一直占着内存，内存总账里减掉的数并没有真的释放。各用各的 Engine，最后一个克隆释放时字节码随 Engine 一起还回去。
#[derive(Clone)]
pub struct ScriptModule {
    pub(crate) module: Module,
    /// 绑在这个模块的 Engine 上的链接表，实例化（包括 `before_commit` 的每次复位）直接用它。
    pub(crate) linker: Arc<Linker<HostState>>,
    pub(crate) declaration: Declaration,
    pub(crate) has_init: bool,
    /// 实例化时一次预留到的页数：`memory_pages`，模块自己声明的最大值更小时取它。
    pub(crate) reserved_pages: u32,
}

impl std::fmt::Debug for ScriptModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 链接表里只有宿主函数，不打印。
        f.debug_struct("ScriptModule")
            .field("module", &self.module)
            .field("declaration", &self.declaration)
            .field("has_init", &self.has_init)
            .field("reserved_pages", &self.reserved_pages)
            .finish_non_exhaustive()
    }
}

impl ScriptModule {
    pub fn declaration(&self) -> Declaration {
        self.declaration
    }

    /// 模块是否导出了 `msime_init`。
    pub fn has_init(&self) -> bool {
        self.has_init
    }

    /// 每个实例的线性内存预留（字节）。进程内存总账在授权和启用时按它做准入检查。
    pub fn reserved_memory_bytes(&self) -> usize {
        self.reserved_pages as usize * limits::PAGE_BYTES
    }

    /// 是否还有别的持有者在使用这个编译好的模块。宿主整理缓存时用它判断能否
    /// 淘汰，避免模块仍存活却把进程内存总账扣掉。
    pub fn is_shared(&self) -> bool {
        Arc::strong_count(&self.linker) > 1
    }
}

/// 按 ABI v1 校验并 Eager 编译。耗时与模块大小成正比，不要放在按键或 focus 路径上，进程里由后台编译线程调用。
pub fn compile(wasm: &[u8], declaration: Declaration) -> Result<ScriptModule, ModuleError> {
    if wasm.len() > limits::MAX_MODULE_BYTES {
        return Err(ModuleError::TooLarge);
    }
    if !wasm.starts_with(b"\0asm") {
        return Err(ModuleError::NotBinary);
    }
    compile_unchecked_format(wasm, declaration)
}

/// 只校验，不留下编译结果。导入时用：和加载时是同一份检查。编译用的 Engine 随结果一起释放，设置应用每次刷新目录都校验一遍，不会越攒越多。
pub fn validate(wasm: &[u8], declaration: Declaration) -> Result<(), ModuleError> {
    if wasm.len() > limits::MAX_MODULE_BYTES {
        return Err(ModuleError::TooLarge);
    }
    if !wasm.starts_with(b"\0asm") {
        return Err(ModuleError::NotBinary);
    }
    check(wasm, declaration).map(drop)
}

/// 编译 WAT 文本，校验与 [`compile`] 相同，只是不要求二进制魔数。只给依赖本 crate 的测试用（`test-support` 特性），产品构建里没有它。
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub fn compile_text(wat: &str, declaration: Declaration) -> Result<ScriptModule, ModuleError> {
    compile_unchecked_format(wat.as_bytes(), declaration)
}

/// 跳过二进制魔数检查的编译。测试打开了 wasmi 的 `wat` 特性，`Module::new` 直接吃 WAT 文本。
pub(crate) fn compile_unchecked_format(
    wasm: &[u8],
    declaration: Declaration,
) -> Result<ScriptModule, ModuleError> {
    let checked = check(wasm, declaration)?;
    let linker = Arc::new(linker(checked.module.engine()));
    Ok(ScriptModule {
        module: checked.module,
        linker,
        declaration,
        has_init: checked.has_init,
        reserved_pages: checked.reserved_pages,
    })
}

/// 校验通过、还没有链接表的模块。
struct Checked {
    module: Module,
    has_init: bool,
    reserved_pages: u32,
}

/// 在一个新的 Engine 上按 ABI v1 校验并 Eager 编译。
fn check(wasm: &[u8], declaration: Declaration) -> Result<Checked, ModuleError> {
    if !(1..=limits::MAX_MEMORY_PAGES).contains(&declaration.memory_pages) {
        return Err(ModuleError::MemoryPages);
    }
    declaration
        .capabilities
        .check_rules()
        .map_err(ModuleError::Capabilities)?;
    if declaration.hooks.is_empty() {
        return Err(ModuleError::NoHooks);
    }
    let engine = Engine::new(&config());
    let module =
        Module::new(&engine, wasm).map_err(|error| ModuleError::Invalid(error.to_string()))?;

    let reserved_pages = check_memory(&module, declaration.memory_pages)?;
    check_func_export(&module, abi::EXPORT_ABI, &[], &[ValType::I32])?;
    check_func_export(
        &module,
        abi::EXPORT_ON_EVENT,
        &[ValType::I32],
        &[ValType::I32],
    )?;
    let has_init = match module.get_export(abi::EXPORT_INIT) {
        None => false,
        Some(_) => {
            check_func_export(&module, abi::EXPORT_INIT, &[], &[ValType::I32])?;
            true
        }
    };
    check_imports(&module, declaration.capabilities)?;

    Ok(Checked {
        module,
        has_init,
        reserved_pages,
    })
}

/// 恰好一块内存、不导入、导出名为 `memory`、32 位。导入的内存由 `check_imports` 拒绝，multi-memory 已关，所以导出了 `memory` 就是唯一的那一块。
fn check_memory(module: &Module, memory_pages: u32) -> Result<u32, ModuleError> {
    let Some(ExternType::Memory(memory)) = module.get_export(abi::EXPORT_MEMORY) else {
        return Err(ModuleError::MissingMemory);
    };
    if memory.is_64() || memory.minimum() > u64::from(memory_pages) {
        return Err(ModuleError::BadMemory);
    }
    let reserved = match memory.maximum() {
        Some(maximum) if maximum < u64::from(memory_pages) => maximum as u32,
        _ => memory_pages,
    };
    Ok(reserved)
}

fn check_func_export(
    module: &Module,
    name: &'static str,
    params: &[ValType],
    results: &[ValType],
) -> Result<(), ModuleError> {
    match module.get_export(name) {
        Some(ExternType::Func(func)) if signature_is(&func, params, results) => Ok(()),
        Some(_) => Err(ModuleError::ExportSignature(name)),
        None => Err(ModuleError::MissingExport(name)),
    }
}

fn signature_is(func: &FuncType, params: &[ValType], results: &[ValType]) -> bool {
    func.params() == params && func.results() == results
}

fn val_types(types: &[Ty]) -> impl Iterator<Item = ValType> + '_ {
    types.iter().map(|ty| match ty {
        Ty::I32 => ValType::I32,
        Ty::I64 => ValType::I64,
    })
}

fn spec_matches(spec: &ImportSpec, func: &FuncType) -> bool {
    func.params().iter().copied().eq(val_types(spec.params))
        && func.results().iter().copied().eq(val_types(spec.results))
}

/// import 白名单和双向的静态最小权限：导入了某个函数就必须申请它所属的能力；申请了某个能力就必须导入至少一个属于它的函数。
fn check_imports(module: &Module, declared: CapabilitySet) -> Result<(), ModuleError> {
    let mut used = CapabilitySet::EMPTY;
    for import in module.imports() {
        let not_allowed = || ModuleError::ImportNotAllowed {
            module: import.module().to_owned(),
            name: import.name().to_owned(),
        };
        if import.module() != abi::IMPORT_MODULE {
            return Err(not_allowed());
        }
        let spec = abi::import_spec(import.name()).ok_or_else(not_allowed)?;
        let ExternType::Func(func) = import.ty() else {
            return Err(not_allowed());
        };
        if !spec_matches(spec, func) {
            return Err(ModuleError::ImportSignature(spec.name));
        }
        if let Some(capability) = spec.capability {
            if !declared.contains(capability) {
                return Err(ModuleError::ImportWithoutCapability {
                    import: spec.name,
                    capability,
                });
            }
            used.insert(capability);
        }
    }
    if let Some(unused) = declared
        .iter()
        .find(|capability| !used.contains(*capability))
    {
        return Err(ModuleError::UnusedCapability(unused));
    }
    Ok(())
}

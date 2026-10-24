# Agent Note: 桌面脚本插件的沙箱运行时：wasmi 解释执行、写死的 ABI v1 配置、按插件固定配额与快照复位

Status: implemented

## Problem

脚本插件（`kind = "script"`）要在接收全部击键的进程里运行第三方代码：macOS 的 IMK 进程、Windows Server、IBus 引擎进程，以及和整个输入框架同进程的 Fcitx5 addon。约束来自几处：macOS 发布包签了 `--options runtime`，放开可执行内存等于让输入法进程接受安全降级；Fcitx5 卡住或 abort 就是整个守护进程出事，线性内存、值栈和字节码都要有硬上限；Windows TIP 等 Server 回复的上限是 50 ms，脚本的耗时必须远小于它；一个插件的行为不能影响另一个插件拿到的资源；`before_commit` 的改写结果只能是本次上屏文本的函数，不能夹带之前的输入。移动端（iOS、Android、HarmonyOS）和网页引擎一行都不能多编译。

导入时（client-core、pack-tool）和加载时（输入法进程）还必须用同一份规则判断一个模块合不合规，否则导入通过的包会在输入法里加载失败，或者反过来。

## Decision

新建 `crates/script-runtime`（`msime-script-runtime`），只在桌面 cfg 下有内容：`cfg(any(target_os = "macos", target_os = "windows", all(target_os = "linux", not(target_env = "ohos"))))`。依赖 `wasmi 2.0.0` 写在同一个 target 段里，`default-features = false`，只开 `std`、`stable`、`validate`、`auto-dispatch`、`extra-checks`；测试另开 `wat`，模块全用 WAT 文本写，仓库不提交 `.wasm`。crate 根上同样挂这个 cfg，所以即使有人在移动端目标上 `cargo build --workspace`，这里也是空 crate、wasmi 不进依赖图。根 `Cargo.toml` 的 members 和 default-members 加上它，并给 `msime-script-runtime`、`wasmi`、`wasmi_core`、`wasmi_ir`、`wasmi_collections`、`wasmparser`、`libm` 各加 `[profile.dev.package.*] opt-level = 3`，理由见下面的实测；`wasmi` 另外关掉 `debug-assertions`，否则 dev 构建里解释器的原生栈随执行的指令数增长，实测见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)。

第一个提交只放一条编译期断言 `wasmi::Module: Send + Sync`，它通过了，所以进程级模块缓存由后台编译线程填充、会话线程读取，见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md)。

**接口不带泛型，wasmi 不出现在调用方的签名里。** 对外只有这些：

- `abi`：`ABI_VERSION = 1`；返回码 `DENIED -1`、`ABSENT -2`、`INVALID -3`、`LIMIT -4`；`Capability`（8 项，第 8 项 `provide_commands` 见 [插件指令的行](2026-10-13-script-plugin-command-rows.md)；`parse`/`as_str`/`requires`/`reads_text`）和按位存放的 `CapabilitySet::check_rules()`（依赖缺失、`rewrite_commit` 与 `read_composition` 互斥）；`Hook`（14 个，编号沿用 host-api `LifecycleEvent` 的判别值，`candidates` 取 17，不是生命周期事件的 `command` 取 18）和 `HookSet`；各 import 的字段编号；`local_mode_allows_text`（U6 白名单）。
- `compile(wasm, Declaration) -> Result<ScriptModule, ModuleError>` 与 `validate`。`Declaration` 是清单里和模块有关的三项（能力、钩子、`memory_pages`），由解析清单的一方填好交进来。`ModuleError::reason()` 是封闭错误码，`Display` 给作者看。
- `ScriptInstance::new(&ScriptModule, grants, init_fuel)`、`deliver(turn, &Event) -> Outcome`、`is_usable`、`turn_fuel_left`、`before_commit_fuel_left`、`command_fuel_left`、`resume_turn`（宿主在同一轮里重建实例时交还这一轮剩下的三份配额）、`init_fuel`、`grants`、`set_log_sink`、`actions`（修改类钩子这一次暂存、已按 ABI 校验过的动作）；`with_debug` 在实例化之前接上 `DebugSinks`（`log` 和修改类 import 的 `ActionSink`），只给作者工具用。`Event` 借用宿主的文本，按授权复制进插件自己的上下文。
- `abi::scalar_values`（`event_scalar` 各枚举字段的名字和数值）和 `abi::imports()`（import 白名单的名字和能力），给作者工具的夹具和 SDK 的一致性测试用。

**每个模块一个 Engine。** wasmi 的 Engine 只追加不回收：翻译后的代码只在 Engine 本身释放时才还回去，Engine 还留着复用的翻译缓冲（大小取决于见过的最大函数）。所有模块共用一个进程级 Engine 时，插件更新后淘汰的旧版本、设置应用每次刷新目录时校验过的模块，都会一直留在里面。所以 `compile` 给每个模块新建一个 Engine，链接表绑在它上面、随模块建一次（`before_commit` 的每次复位直接用它）；`validate` 的 Engine 随结果释放。最后一个 `ScriptModule` 克隆释放时，代码和缓冲随 Engine 一起还回去。代价是每个模块多约 10.6 KiB 的固定开销，每个 Engine 各留一份值栈。

**写死的 `Config`：** `CompilationMode::Eager`（翻译成本不落在首次调用的 fuel 上）；关掉默认开着的 multi-memory、tail-call、extended-const，以及 wide-arithmetic、custom-page-sizes；`consume_fuel(true)`；`set_max_stack_height(65_536)`、`set_max_recursion_depth(256)`、`set_max_cached_stacks(1)`；`allow_start_fn(false)`。memory64 和 SIMD 在 wasmi 里是 cargo 特性，没有 `Config` 开关可调，本 crate 不开它们就随之关闭，测试各有一个被拒的模块守着。GC_TYPES 保持开启，reference-types 需要它。

**模块校验（导入和加载同一份）：** 二进制魔数（文本格式不接受）、≤ 256 KiB、`memory_pages` 在 1..=64、能力规则、至少一个钩子；恰好一块导出名为 `memory` 的 32 位内存，最小页数不超过 `memory_pages`；导出 `msime_abi() -> i32`、`msime_on_event(i32) -> i32`，可选 `msime_init() -> i32`；import 只能是 `msime` 命名空间白名单里的函数且签名一致。静态最小权限双向检查：导入了某个函数就必须申请它所属的能力，申请了某个能力就必须导入至少一个属于它的函数。

**执行：** 每个实例一个 `Store`，`StoreLimits` 把线性内存卡死在预留值（`memory_pages`，模块声明的最大值更小时取它），实例化后一次长到预留值，`trap_on_grow_failure` 让之后的 `memory.grow` 直接 trap；表最多 4 张、每张至多 10 000 个元素（wasmi 的 `table_elements` 按张限制，合计至多 40 000 个，每个 4 字节，最坏约 160 KiB，不计入内存总账）。fuel 是每插件固定配额：`before_commit`、`command` 之外的钩子每轮合计 125 000，单次 `candidates` 125 000、其余观察类 25 000，本次可用的是本轮剩余和本钩子上限中较小的那个；`before_commit` 另有一份每轮 100 000 的配额，单次上限也是它，`command` 同样另有一份每轮 100 000（[插件指令的行](2026-10-13-script-plugin-command-rows.md)），三份互不扣减（理由见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)）；轮次由宿主传入的计数决定，换了轮次三份都给满。这个钩子所用的那一份为 0 时不运行，报 `Outcome::TurnExhausted`，不是失败：宿主不在每次派发前都开一轮。运行到一半被那一份的剩余截断（而不是被本钩子的上限截断）时把那一份记为用完，再看这一次自己用掉多少：不到本钩子上限（`limits::hook_fuel`）的一半是 `Outcome::TurnExhausted`，不是失败，这是没经过 `msime_client_key` 的派发只拿到上一次按键剩下的零头；达到一半就和超出本钩子上限一样是 `Failed { OutOfFuel }`，计入熔断。否则 `candidates` 的单次上限和轮配额相等，同一轮里 `input` 只要先花过一点 fuel，`candidates` 里的死循环就总是先撞上轮配额，被归为不算失败，熔断永远落不到它身上。持久实例上被截断时实例不可用（两种结果都一样），`before_commit` 和 `command` 跑在快照实例上，持久实例照用。初始化（`msime_abi` 加 `msime_init`）不超过 5 000 000，失败时报告用掉的额度，供宿主计入 focus 的初始化合计。轮配额记在实例上，宿主在同一轮里重建实例时用 `resume_turn` 交还三份各剩下的，配额按插件算。import 也计 fuel：每次 100，每复制 4 字节再加 1。trap 映射成封闭的 `Failure`：`OutOfFuel`、`StackOverflow`、`MemoryLimit`、`MemoryOutOfBounds`、`Unreachable`、`Trap`，外加 `NegativeReturn`、`AbiMismatch`、`InitFailed`、`Instantiate`、`Unusable`。持久实例上出过 trap 就标为不可用（wasm 的影子栈指针可能停在一半），宿主丢弃它，这次聚焦里不再重建；返回负数只算一次失败，实例照用。

**快照复位靠重新实例化。** 声明了 `before_commit` 或 `command`（`Hook::runs_on_snapshot`）的插件在 `msime_init` 之后取一份快照：线性内存里最后一个非零字节之前的部分，加上导出的数值型可变 global。每次 `before_commit`、`command` 都新建一个 Store 和实例（start 函数已被拒绝，实例化不跑任何 wasm 代码），盖上快照，把快照之外、数据段曾写过的区域清零，跑完即弃。持久实例不受影响，观察类钩子攒下的状态照样留着，但进不了这两个钩子；它们自己的改动也留不下来。代价写进作者文档：表和未导出的 global 回到模块声明的初值，`msime_init` 对它们的改动不保留（Rust 编译出的 `__stack_pointer` 在函数返回时已经复原，不受影响）。

**观察类 import：** `event_scalar` 只给枚举字段（事件类型、origin、how、outcome、reason、本地模式、输入类别、撤回方式）和三个 0/1 标志（rewritable、continues、generated），不需要能力。`composition_*`、`commit_*`、`candidate_*` 先查授权（没有就 `DENIED`），再查字段（没有或被屏蔽都是 `ABSENT`）。上下文在调用前按授权复制、调用后清空：没有授权的文本根本不进插件的上下文；本地模式不在 U6 白名单里（或者宿主没填）时，整个能力域都按屏蔽处理，连光标位置、座位和候选页码这类标量也一样。候选的数量、页码和高亮位置归 `read_candidates`，不放进 `event_scalar`。文本复制先检查整个目标区间 `[dst, dst + cap)`，越界直接 trap；放不下返回 `LIMIT`，什么都不复制。修改类 import（`rewrite_commit`、`candidates_order`、`candidates_hide`、`candidates_annotate`）只暂存动作，钩子正常返回后才交给宿主，约定、校验和宿主怎么用它们见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)。`log` 在产品里是空操作，只有设了调试输出（`msime-pack run`）才打印。

**实测（Mac Studio，同时跑着 CI runner，数字有抖动）。** release 和带覆盖的 dev 列是到目前为止 4 次运行的范围（建 crate 时 2 次，接进输入法进程后 1 次，改成每个模块一个 Engine 后 1 次）；去掉覆盖的那一列只量过一次。512 KiB 的测量绕过了现在 256 KiB 的模块上限，量的是设计要求的那个大小。

| 项 | release | dev（带 opt-level 覆盖） | dev（去掉覆盖） |
|---|---|---|---|
| 最小钩子一次投递 | 78–190 ns | 60–220 ns | 2.5 µs |
| 512 KiB 模块校验 + Eager 编译 | 5–28 ms | 14–24 ms | 322 ms |
| 512 KiB 模块实例化 | 0.47–1.32 ms | 0.60–1.6 ms | 22 ms |
| 快照复位 16 页（内存全脏）每次 `before_commit` | 53–76 µs | 35–92 µs | 11 ms |
| 快照复位 64 页（内存全脏）每次 `before_commit` | 148–343 µs | 233–676 µs | 46 ms |

去掉 opt-level 覆盖后最小钩子慢约 12 倍，64 页复位从亚毫秒变成 46 ms，所以覆盖是必要的。测量用例在 `crates/script-runtime/src/tests.rs` 的 `measure` 模块里，默认忽略，用 `cargo test -p msime-script-runtime -- --ignored --nocapture --test-threads=1 measure` 运行。

**编译膨胀和内存总账的估算。** 产品里量不到一个模块编译后真正占了多少（库不能替宿主进程装全局分配器），总账只能按模块大小估算，所以估算必须是上界。用只数当前线程的计数分配器量编译结果留下的字节（模块自己的 Engine 也算在内），在 2 KiB 到 256 KiB 之间按 1.07 倍取样：

| 形状 | 最大倍数 |
|---|---|
| 普通代码（524 KB 访存密集的合成模块） | 3.36 倍 |
| 重复的类型、导出、全局变量 | 8–11 倍 |
| 每个函数压 1 万层操作数栈 | 26 倍 |
| 空的被动元素段和数据段、元素段条目、最小的函数（函数表按 2 的幂分桶） | 23–43 倍 |
| `br_table` 每个目标 1 字节（Engine 留着的翻译缓冲） | 92 倍 |
| `block` 嵌套每层 3 字节（同上） | 110 倍 |
| 声明 3 万个局部变量的函数 | 与大小无关，约 370 KiB |

估算取每字节 128 倍加 256 KiB 固定开销（`limits::module_memory_bytes`，再加每个 Engine 一份值栈）。`tests::expansion::the_ledger_estimate_bounds_every_adversarial_shape` 在每次 `cargo test` 里按 1.5 倍取样守住它；`measure_expansion_of_adversarial_modules` 打出每种形状的最大倍数。按设计 §4.4 "量出膨胀比再定上限"，模块上限从 1 MiB 降到 256 KiB：最大的模块带 64 页内存在总账里约 36 MiB，单独启用放得下，两个就放不下；普通的几十 KiB 的插件每个约 5 MiB。

## Testing

- 本 crate 的普通单测由 CI 的 Script plugins 作业和 `scripts/verify-local.sh` 完整验证显式执行；完整验证也运行本 crate 的全目标严格 Clippy。插件夹具只构建运行时依赖，不执行这里的 `cfg(test)`；4 项人工性能测量保持忽略，门禁接线与取舍见[插件合集](2026-10-12-script-plugin-collection.md)。

- `cargo test -p msime-script-runtime` 和 `cargo clippy -p msime-script-runtime --all-targets -- -D warnings` 通过，覆盖：死循环按钩子上限耗尽 fuel、`unreachable`、越界访问、内存越过预留、深递归撞上栈限制、被拒的提案（multi-memory、tail-call、extended-const、SIMD、memory64、threads）和 start 函数、允许的提案能编译、双向的静态最小权限、import 白名单和签名、能力的依赖与互斥、每轮配额跨多次投递并在新一轮补满、本轮用完时跳过或截断而不算失败（`before_commit` 那一份同样，持久实例照用）、被截断时这一次用掉达到本钩子上限一半就算 fuel 耗尽的失败（`input` 花过 fuel 之后 `candidates` 死循环，三个钩子在一半减一和正好一半处的边界）、一个插件烧满配额不影响另一个、import 计 fuel、`before_commit` 读不到观察类钩子和上一次 `before_commit` 留下的内存与 global、快照上的 trap 不影响持久实例、授权与本地模式门控、文本复制的 `LIMIT` 和越界 trap、初始化看不到任何事件、`log` 没有调试输出时是空操作。修改类 import 的测试见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)。
- 移动端不编译解释器：`cargo tree -p msime-script-runtime -e normal` 在 `aarch64-apple-ios`、`aarch64-linux-android`、`aarch64-unknown-linux-ohos` 和 `wasm32-wasip2` 上只有本 crate 自己，在 `x86_64-unknown-linux-gnu` 上是 wasmi 及其 6 个依赖；前三个目标上 `cargo check -p msime-script-runtime` 通过（已跑过）。
- client-core 一侧（`script_pack`、`script_grants`、`PluginKind::Script`）见 [脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md)，那篇就是取代 `plugins.rs:3` 原则的笔记；host-api 一侧（投递观察类钩子、`msime_client_scripts_enable`、后台编译线程、熔断）见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md)。pack-tool 的 `msime-pack run` 直接依赖本 crate，用 `ScriptInstance::with_debug` 接上 `log` 和修改类 import 的调试输出，见 [脚本插件的作者工具](../feature/2026-10-12-script-plugin-author-tools.md)。
- `test-support` 特性只给依赖本 crate 的测试用：它打开 wasmi 的 `wat` 特性并导出 `compile_text`，让 host-api 和 pack-tool 的测试用 WAT 写模块。dev-dependencies 打开的特性不并进普通构建，产品里没有它。依赖批准时写明 `wat` 只在本 crate 的 dev-dependencies 里；这个特性是批准范围之外的另一条路线（任何 crate 都能在普通构建里打开它），需要单独确认，不批准就把 host-api 的测试模块改成手写二进制。

## Alternatives considered

- **wasmtime、wasmer、extism。** 最强的理由是快一到两个数量级，生态成熟，wasmtime 还有 epoch 中断，能做真正的时间上限。不用：它们靠 JIT，macOS 的 hardened runtime 要为此放开可执行内存；wasmtime 在 Windows 上注册进程级 VEH、在 Unix 上装 `sigaction`，会进入 Fcitx5 守护进程这类我们不拥有的进程；wasmtime 的 MSRV 每月都在抬高。脚本的钩子是微秒级的小函数，解释器的速度足够。
- **wasmi 1.1.0。** 最强的理由是发布更久、用的人更多，2.0 发布才一个多月、由一人维护。不用但保留为退路：1.1.0 没有 `stable`、`validate`、`auto-dispatch` 特性，退回时要改成 `["std", "extra-checks"]`，fuel 单位也会漂移，配额要重新标定。
- **原地复位：在同一个实例里复写线性内存和导出的 global。** 最强的理由是省掉每次 `before_commit` 的实例化。不用：wasmi 不给宿主访问未导出的 global 和表，而它们同样能把上一次事件的内容带进下一次 `before_commit`，原地复位守不住"改写只是本次文本的函数"这条规则。重新实例化在 16 页时只多几十微秒。
- **允许 start 函数。** 最强的理由是有的工具链会生成它，拒绝它会挡住一部分模块。不用：快照复位要重新实例化，start 函数会在每次复位时再跑一遍；初始化有 `msime_init`，它受 fuel 约束、返回值可检查。
- **所有模块共用一个进程级 Engine，链接表每个线程一份。** 最强的理由是没有每个模块的固定开销和多出来的值栈，链接表也只建一次。不用：Engine 不回收翻译后的代码，淘汰的模块和导入时校验过的模块一直留着，总账减掉的数不是真的释放；见过的最大函数的翻译缓冲也一直留着。
- **总账按普通代码的膨胀比（3.3 倍，取 4 倍）估算，保留 1 MiB 的模块上限。** 最强的理由是正常插件的总账更宽松，8 个大插件也放得下。不用：专门构造的模块能放大到 110 倍，1 MiB 的模块实际能留下上百 MiB，总账形同虚设；产品里又量不到真实值，只能按上界记。
- **被本轮配额截断一律不算失败。** 最强的理由是宿主不在每次派发前都开一轮，截断可能是之前几次正常投递花光了配额。不用：`candidates` 的单次上限等于轮配额，同一轮里之前的钩子只要花过 fuel，它的死循环就永远归为截断而不是失败，熔断管不到。
- **把 `candidates` 的单次上限压到轮配额以下，截断仍一律不算失败。** 最强的理由是不引入按比例的判定。不用：差额多大都只是把问题挪到"之前的钩子花掉多少"上，`input` 花得比差额多时同样永远不熔断；按这一次自己用掉多少判定，和之前花掉多少无关。
- **把 wasmi 的类型或泛型 `Store<T>` 暴露给调用方。** 最强的理由是调用方可以按需扩展宿主数据。不用：client-core、host-api 和 pack-tool 的签名里一旦出现 wasmi，升级或回退 wasmi（上面的 1.1.0 退路）就要改三个 crate；ABI 的规则也会散到调用方去。

## Consequences
- 收益：解释器、ABI 规则和配额集中在一个不带泛型的 crate 里，导入和加载走同一份校验；fuel、内存和栈都是硬上限，一个插件的行为不改变另一个插件拿到的资源。
- 代价：解释执行比 JIT 慢一到两个数量级，dev 构建靠 7 段 opt-level 覆盖才够快；每次 `before_commit` 都要重新实例化。总账按上界记，正常插件也按 128 倍估算，同时启用的大插件个数受它限制。
- 截断按一半判定也会误伤：一个每次正常要用掉自己上限一半以上的插件，在同一轮里收到足够多次投递、余额落在"不到它一次的用量、但不少于上限一半"时被截断，这一次就算失败。例如每次 `composition_update` 用 18 019，六次之后余额 16 886，第七次被截断就是 `out_of_fuel`（pack-tool 的 `failures_are_reported_with_their_fuel_and_fail_the_run` 第 12 行）。要一轮里同一类钩子收到好几次才会这样，3 次失败进同一个 50 次投递的窗口才停用。
- 编译时的峰值内存同样能到模块大小的约 110 倍（翻译缓冲），256 KiB 的模块约 28 MiB，发生在后台编译线程和设置应用的校验里，编译完随 Engine 释放；没有记进总账。
- wasmi 2.0 的执行器没有经过审计，发布至今约 40 天。`extra-checks` 已开，`Cargo.lock` 锁定版本；与 1.1.0 的差分 fuzz 还没做，是已知缺口。
- 64 页、内存全脏时，dev 构建里一次复位最慢量到 0.68 ms，贴着 `before_commit` p99 ≤ 1 ms 的目标。实际插件的快照只到最后一个非零字节，通常远小于预留值；第 4 阶段接通 `before_commit` 时在真机上复测，仍然超预算就按设计 §4.3 的退路把输出限制成对 body 的编辑。
- memory64 和 SIMD 由 wasmi 的 cargo 特性控制。依赖图里哪天有别的 crate 打开了这两个特性，`Config` 会随之放开；被拒模块的测试只在本 crate 的测试构建里守着。
- `Cargo.lock` 里新增的第三方 crate 不只设计说的 7 个，共 16 个。只进锁文件、从不编译的 3 个：`wasmi_collections` 的 `std` 特性写的是弱依赖 `string-interner?/std`，Cargo 把这个可选依赖记进锁文件，连同它的 `hashbrown 0.15.5` 和 `foldhash 0.1.5`，`cargo tree --workspace --target all -e all -i string-interner` 为空。只在测试里编译的 6 个：`wat` 特性带来的 `wat`、`wast`、`wasm-encoder`、`wasmparser 0.261.0`、`leb128fmt`、`unicode-width`，`cargo tree -p msime-host-api -e normal -i wat` 为空。编译进产物的仍是那 7 个，第三方声明脚本只需覆盖它们。
- 体积增量见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md) 的实测。

相关：[桌面端生命周期核心](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md) 产生投递给脚本的事件，`Hook` 的编号沿用它的 `LifecycleEvent` 判别值。

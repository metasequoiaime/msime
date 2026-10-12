# Agent Note: 脚本插件在输入法进程里：进程开关、观察类事件的投递、后台编译与进程级熔断

Status: implemented

## Problem

沙箱运行时（`msime-script-runtime`）和插件包一侧（`PluginKind::Script`、`script_pack`、`script_grants`）都有了，但还没有任何进程真的运行脚本。接进输入法进程要回答几件事：哪些进程运行、哪些永远不运行（Windows TIP 和 Server 共用同一份宿主 DLL，TIP 被注入每个应用进程）；什么时候运行（密码框、隐私字段、提权窗口、属性未知）；生命周期核心推导出的事件里哪些交给脚本，哪些会泄露击键节奏、停顿和网络时序；编译一个几百 KiB 的模块不能落在按键或 focus 路径上；一个坏插件的失败不能拖垮上屏，也不能在 IBus 频繁重建会话时每个会话重来一次；授权文件在用户可写的目录下，发布构建在加固之前不能运行任何脚本；移动端一行都不能多编译。

## Decision

**进程开关。** `msime_client_scripts_enable(bool) -> bool` 是进程级的 `AtomicBool`，返回开关的实际状态，在 `msime_client.h` 声明。在桌面上生效，移动端恒返回 false，`on` 被忽略。在授权页落地之前它还要求 `cfg(debug_assertions)`，release 构建恒为 false；随 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md) 落地，macOS 和 Linux 上去掉了这个条件（授权不签名，残余风险写在授权页上）；Windows 的原生设置应用还没有授权页，那里仍只在开发构建里生效。4 个桌面入口在第一个会话之前各调用一次 `msime_client_scripts_enable(true)`：

| 宿主 | 进程 | 调用位置 |
|---|---|---|
| macOS | IMK 输入法进程 | `input_method_main.mm`，独立设置窗口的分支返回之后、`IMKServer` 之前 |
| Windows | `MetasequoiaImeServer` | `server_main.cpp`，登记诊断出口之后 |
| Linux IBus | `msime-linux-ibus` | `ibus_main.cpp`，`ibus_init()` 之前 |
| Linux Fcitx5 | fcitx5 守护进程（addon 是它加载的共享库） | `FcitxEngine` 构造函数开头 |

Windows TIP、设置应用（Tauri）和 MCP 从不调用。`scripts/test-script-plugin-hosts.py` 检查这 4 处各有一次调用，`platforms/windows/tsf` 和其余任何源码（host-api 自己的定义和测试除外）都不调用它、不按名字取它的地址。

**运行条件，五项同时满足：** 进程开关开着；会话经 `msime_client_set_lifecycle` 选择加入（没选择加入的会话没有 `LifecycleState`，脚本集合就挂在它上面，所以什么都没有）；字段属性带 `KNOWN`；不带 `SECURE` 或 `PRIVATE`；不带 `ELEVATED`。属性未知按不满足处理。`set_field_traits` 让属性变得不满足时，正在跑的实例当场丢弃。

**每次聚焦之后属性要重新确认。** 宿主按契约在每个键之前比较一次属性、变了才报告，聚焦本身不带属性；不重新确认的话，聚焦进密码框、隐私字段或提权窗口时，插件会按上一个字段的普通属性被实例化并收到 `focus_in`，直到第一个键报告了 SECURE 才丢弃。所以 `focus(true)` 把属性记为未确认，确认之前不实例化、不投递。确认有两种：聚焦之前（上一个键或上一次失焦之后）调过 `set_field_traits`，属性没变也算；或者聚焦之后的第一个 `msime_client_key`。头文件写明想让脚本收到 `focus_in` 的宿主要在 `msime_client_focus(true)` 之前报告属性；不这样做的宿主，脚本从第一个键开始运行。

**投递哪些事件。** `focus_in`、`focus_out`、`input`、`selection_retracted`、`candidate_selected`、`commit`、`composition_start`、`composition_update`、`composition_end`、`candidates_shown`、`candidates_hidden`，只来自派发类调用、focus 和 destroy（包括报错时的收尾推导）。不投递：`msime_client_key` 的按键（泄露击键节奏）；字段属性、激活和会话结束（只给核心）；旁路调用和设置类调用（`apply_online_*`、`rerank_settled`、`set_english_mode`、`update_preferences` 等）的视图事件（泄露停顿和网络时序）。`before_commit` 和 `candidates` 是修改类钩子，什么时候运行、怎么改写和合成见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)。

**文本。** 生命周期事件不带文本。`LifecycleState::after_texts` 借这次调用的 transition 和 runtime 构造 `CallTexts`（调用后的视图、全角转换之后的上屏文本、`commit_context.local_mode`、派发事实、选中候选的文本），每个插件由 `ScriptInstance` 按自己的授权和 U6 本地模式白名单各复制一份；没有授权的文本根本不进它的上下文。各事件给的字段：

- `input`：输入类别、本地模式，`read_composition` 下这次打的字符（字符键、两条标点路由、14 键）。
- `candidate_selected`：how、outcome、本地模式（取上屏时的模式），`read_commit` 下选中候选的文本（`SELECTED`）和座位。选中的文本由 input-runtime 在记下 `SelectionFact` 的同时，从引擎调用之前的快照复制进一个复用的缓冲区（`Runtime::selected_text`），`DispatchFacts` 仍是 `Copy`。
- `commit`：origin、continues、generated、本地模式，`read_commit` 下完整上屏文本和 body（按 `tail_chars` 从末尾切掉标点、顶字尾巴）。
- `composition_start`/`composition_update`：`read_composition` 下预编辑串、读音、光标。光标是编辑串（`editing_text`，打下的字母本身）里光标前的字符数，ABI 文档随之写明，因为笔画、韩文、九键下预编辑串和编辑串不是同一个串。
- `candidates_shown`：`read_candidates` 下当前页（最多 20 行，在栈上的数组里借用视图，不分配）、页码和高亮。
- `composition_end`/`candidates_hidden`：原因。

**实例。** 每个选择加入的会话一组（`scripts::ScriptSet`），按授权顺序，第一次需要投递一个它声明过的钩子时才实例化。初始化预算是所有插件合计 `FOCUS_INIT_FUEL`，每次 focus 和每一轮都给满；被本轮剩下的预算卡住（而不是被插件自己的 `INIT_FUEL` 上限卡住）的插件推迟到下一轮，不算失败。轮次是每个不带 `PASSTHROUGH` 的 `msime_client_key` 加一，fuel 按轮给，轮配额记在插件上而不是实例上：同一轮里重建的实例（换了字段、属性变过）接着用这一轮剩下的，不重新给满 125 000。三个桌面宿主都有不先调 `msime_client_key` 的派发（Linux、Windows 的修饰键组合，macOS 的 Cmd/Ctrl 快捷键，失焦命令），它们和上一次按键共用一轮，把这一轮花光的可能是之前几次完全正常的投递，所以**本轮配额用完不算插件的失败**：剩余为 0 时跳过这个插件（`Outcome::TurnExhausted`），也不为它在按键路径上重新初始化；运行到一半被本轮剩下的配额截断时，实例和 trap 一样丢弃、这次聚焦里不再重建；这一次用掉的不到本钩子上限的一半时不计入熔断，达到一半就和超出单个钩子自己的上限一样算一次 fuel 耗尽的失败。一轮刚开始时剩余不少于任何观察类钩子的上限，观察类钩子的死循环在开头总是先撞上钩子上限；`candidates` 的单次上限却和轮配额相等，同一轮里 `input` 先花过一点 fuel，它的死循环就先撞上轮配额，要是截断一律不算失败，这样的插件永远不熔断。**一个插件 trap 之后，这次聚焦里不再重建它的实例**，下一次聚焦才重新实例化；否则一个每 50 次故意失败 2 次的插件永远不熔断，每次失败后都在按键路径上重跑一遍至多 5 000 000 fuel 的初始化。返回负数不算 trap，实例照用。**每次 `msime_client_focus` 和 destroy 都把这个会话的全部实例丢弃**：`focus(false)` 在 `focus_out` 送达之后丢，`focus(true)` 在任何事件之前丢（只有 `focus(true)`、没有失焦的换字段也算）。import 只读写本次调用的上下文，从不回调宿主，会话正借给这次调用。

**授权的重读。** `focus(true)` 紧挨着 `refresh_plugin_tables` 给 `script-grants.json` 打戳（`key_sound::stamp`，带 inode）；戳变了就整份重读，计划换掉。会话第一次需要投递时如果还没读过（宿主在聚焦之后才选择加入）也读一次。Windows 没有 inode，靠原子替换带来的新修改时间；授权的 `revision` 另外驱动熔断的恢复：要比本进程以前看到过的大。

**进程级模块缓存与后台编译线程。** 缓存按内容哈希存放编译好的 `ScriptModule`。会话只登记请求、查结果；名为 `msime-script-compile` 的后台线程在第一次登记时才起（从不运行脚本的进程不起任何线程），用 `script_pack::load` 读包、核对哈希、按 ABI v1 校验并 Eager 编译。还没编译好的插件这一次跳过，不算失败。已安装的包和授权的哈希对不上（授权之后包被换过）或者包读不出时记为待重试，包清单的戳变了再登记；模块不合规时按哈希永久记下，插件随即停用。重读授权时淘汰本插件根目录下不再启用的模块。

**内存总账的兜底。** 授权时已经按单个会话做过准入检查（`limits::fits_memory_budget`）。实例化时再按进程记一次账：活着的实例的线性内存预留，加缓存里每个模块的开销（`limits::module_memory_bytes`：编译结果按每字节 128 倍加 256 KiB 估算的上界，再加它自己的 Engine 留着的一份值栈），合计不超过 `PROCESS_MEMORY_BYTES`（64 MiB），放不下就这一次不实例化，不算失败。估算为什么是上界、为什么每个模块有自己的 Engine，见 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)；因为各用各的 Engine，淘汰一个模块时总账减掉的那份是真的释放了。实例在每次 focus 都丢弃，所以只有聚焦着的会话占着线性内存。

**熔断是进程级的**，键为 (插件根目录, id, 哈希)：初始化失败、ABI 不符和模块不合规立即停用；否则最近 50 次投递里失败 3 次，或者最近 20 次里有 3 次超过 4 ms（只做事后熔断，fuel 才是硬限制；测试里墙钟可注入），就停用。一次失败只丢弃这个插件这一次的结果，后面的插件照常执行。停用持续到授权文件的 `revision` 比以前看到过的都大或进程重启。停用时报告一行 `script plugin disabled: <id> <reason>`，reason 是封闭的原因码（`unreachable`、`out_of_fuel`、`init_failed`、`slow` 等），每次停用只报一次。

**健康文件只记停用。** 有插件停用或恢复过时，失焦和 destroy 把 `<user_data>/script-health.json` 交给后台线程原子替换：

```json
{"format_version":1,"plugins":[{"id":"…","hash":"…","disabled":"failed"}]}
```

`disabled` 只有 4 个类别：`invalid`（模块不合规或编译时 panic）、`init`（实例化或初始化失败）、`failed`（投递失败太多）、`slow`。不记失败次数，不按原因细分，没停用的插件不出现，没有变化时不写。进程重启也是恢复：本进程第一次读某个插件根目录的授权时，健康文件在就标记为待写，下一次失焦按本进程的状态重写它（`adopt_health`），设置页不再显示上一个进程留下的停用；只在开关打开的进程里发生，不运行脚本的进程不改写它。哪种 trap、失败几次都由脚本自己选，窗口里不到 3 次又永远不熔断：按次数或原因写出去，同一用户下伪造了授权的进程每次失焦后对比文件，就有了一条能反复使用、按位传出已读文本的通道。只记停用之后，熔断后要等 `revision` 变大或进程重启才能再触发，剩下的就是设计 U5 接受的一次性信道。具体原因码仍照设计 §4.5 进诊断，每次停用一次。

**panic。** 宿主一侧投递每个插件都包一层 `catch_unwind`；捕获到 panic 就关掉本进程的全部脚本直到重启，报告一次，这次调用照常返回上屏结果。后台编译线程的每个任务也包一层：编译时 panic 同样关掉全部脚本，那个模块记为不合规（`compile_panicked`），不会永远停在待编译、既不运行也不停用。

**移动端。** `msime-script-runtime` 只在桌面 cfg 下作为 host-api 的依赖；`scripts` 模块在移动端是一个什么都不做的同名空壳，生命周期核心的调用处不用写 cfg。`cargo tree -p msime-host-api -e normal -i wasmi` 在 `aarch64-apple-ios`、`aarch64-linux-android`、`aarch64-unknown-linux-ohos` 上为空，三个目标上 `cargo check -p msime-host-api -p msime-input-runtime -p msime-client-core --all-targets` 通过。

**第三方声明。** Windows 和 Linux 的 `collect-notices.py cargo` 按 `cargo tree -p msime-host-api -e normal,no-proc-macro` 收集，host-api 直接依赖本 crate 之后，wasmi、wasmi_core、wasmi_ir、wasmi_collections、wasmparser、spin、libm 这 7 个都在其中，脚本不用改。macOS 的 `THIRD_PARTY_NOTICES.txt` 按惯例只单列 MPL 等需要指明源码的组件，这 7 个都是 MIT 或 Apache-2.0。

**测试用 WAT。** host-api 的测试经 `msime-script-runtime` 的 `test-support` 特性（打开 wasmi 的 `wat` 特性，导出 `compile_text`）用 WAT 写模块，编译好直接放进进程缓存；只有后台编译线程那一例从磁盘上的包走完整路径，模块按字节手写。仓库不提交 `.wasm`。依赖批准时写明的是 `wat` 只出现在 script-runtime 自己的 dev-dependencies 里；经一个普通特性交给另一个 crate 的测试，是批准范围之外的另一条路线，锁文件里没有多出 crate，但这条路线本身还要单独确认。

## Measurements

Mac Studio（arm64，同时跑着 CI runner，数字有抖动，同一份测量在不同运行里能差一倍）。运行时内的数字（最小钩子、512 KiB 模块的编译和实例化、编译膨胀、快照复位）只记在 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)，这里只记宿主一侧。

| 项（各量一次） | dev（带 opt-level 覆盖） | release |
|---|---|---|
| 宿主一侧投递 `composition_update` 给一个空钩子（运行条件、构造事件、执行、记账） | 557 ns | 174 ns |
| 宿主一侧投递 `candidates_shown` 给一个授予了 `read_candidates` 的空钩子，复制一整页 20 行 | 900 ns | 300 ns |

- 由 `measure_delivery_cost`（`crates/host-api/src/tests/script_plugins.rs`，默认忽略）直接对着会话的脚本集合量，十万次取平均；一次按键的派发本身是几十到几百微秒，抖动远大于脚本这点开销，所以不从整键时间里减。夹具的词库里 `ni` 只有一个候选，测量照着它补满一页。这是测试构建：墙钟熔断的计时换成了测试开关，进程开关和 panic 总开关读的是线程局部变量，产品里每次投递还多一次 `Instant::elapsed`。dev 比 release 慢约 3 倍，是因为 host-api 自己没有 opt-level 覆盖。
- 体积只做了归属估算，不是增量：release 的 `libmsime_host_api.dylib` 是 33.66 MB，`__text` 17.73 MB，按符号名归到解释器的代码约 0.95 MB（wasmparser 622 KB、wasmi 284 KB、msime-script-runtime 9 KB、host-api 的 `scripts` 模块 36 KB；libm 和 spin 的符号都被内联或裁掉了）。不含只读数据；泛型实例化按名字里出现 `wasmi`、`wasmparser` 计入；没有和改动之前的构建对比。
- 这一测试现名 `the_real_process_switch_gates_delivery_in_debug_and_release_builds_without_any_key`，macOS 和 Linux 的 release 构建里打开开关也投递，Windows 的 release 构建里开关仍打不开（见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md)）。加固之前的版本 `cargo test --release -p msime-host-api the_real_process_switch_gates_delivery_and_stays_off_in_release_builds` 通过：release 构建里 `msime_client_scripts_enable(true)` 返回 false，打开之后也一个事件都不投递。
- 没量：x64 `msime_host_api.dll` 的体积增量（Studio 没装 Windows 目标，要等 Windows 交叉构建或 CI 产物）；macOS 输入法可执行文件的体积增量（它静态链接 `libmsime_host_api.a`，要分别在改动前后构建输入法本身再比，上面的 dylib 归属代替不了）。

## Testing

`cargo test -p msime-host-api`（`tests/script_plugins.rs` 22 例，授权页落地后加上带早期 `mac` 字段的授权照常运行、放回旧授权文件不复位熔断、重启后接管健康文件的几例是 26 例，外加默认忽略的测量）：普通字段收到全部观察类事件，按键不投递；SECURE、PRIVATE、ELEVATED、没有 KNOWN 位时一个事件都没有，换回普通字段照常运行；聚焦进新字段后属性确认之前什么都不运行（聚焦前报告过属性时 `focus_in` 照常投递，否则从第一个键开始）；字段变成 SECURE 时实例当场丢弃，回来后重建；没有授权时文本类 import 返回 `DENIED`，授权后读到上屏文本；`SELECTED` 只在 `candidate_selected` 里有，body 不带结束组字的标点；`read_composition` 下 `input` 带打的字符、`composition_update` 带预编辑串；每次 focus（含只有 `focus(true)` 的换字段）都重建实例；一个插件烧满 fuel 不影响下一个；同一轮里连发会用完轮配额，每个新键补满；同一轮里换字段重建的实例接着用这一轮剩下的配额，trap 过的插件这次聚焦里不再重建；三次聚焦各失败一次就停用，没停用之前失焦什么都不写，停用后 `script-health.json` 只有 id、哈希和类别，`revision` 变大后恢复，换成 `revision` 相同但没有条目的文件、更旧的文件、删掉文件再放回原来那份都不恢复；本进程第一次读授权时健康文件在就按本进程的状态重写，不在就不写；初始化失败立即停用；慢投递按墙钟熔断；没选择加入的会话什么都不运行；产品里的进程开关（不经测试覆盖）关着不投递，打开就投递（release 构建也一样，见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md)）；旁路调用的视图事件不投递；宿主一侧 panic 关掉全部脚本而上屏照常；后台编译时 panic 的模块被停用而不是一直等；磁盘上的包经后台编译线程编译后运行；哈希对不上的授权从不运行。`lifecycle_alloc.rs` 的零分配断言不变（没有脚本时投递路径在第一项检查就返回）。`scripts/test-script-plugin-hosts.py` 守住入口。

## Alternatives considered

- **只丢弃授予了 `read_*` 能力的实例（设计 §4.7 第 4 条的字面要求），不读文本的实例跨 focus 保留。** 最强的理由是不读文本的插件（例如只数事件的统计）能把状态留在实例里，换字段也不必重新初始化。不用：macOS 每个客户端应用一个会话，不读文本的实例会在每个曾经聚焦过的会话里各留一份线性内存，进程总账很快被不在前台的会话占满，前台会话的插件就因为放不下而一直跳过；全部丢弃让只有聚焦着的会话占内存，也少一条要推理的分支。代价是这类插件的状态只活在一个字段里。
- **在会话线程的 focus 路径上编译（设计 §3.4 的退路）。** 最强的理由是不需要线程和跨线程缓存，模块到手就能用。不用：编译 512 KiB 的模块在 dev 构建里要十几到二十几毫秒，IBus 改一个设置就重建会话，Fcitx5 卡住就是卡住整个守护进程的事件循环；`wasmi::Module` 是 Send + Sync 的断言已经通过，后台编译没有障碍。
- **熔断按会话计。** 最强的理由是状态随会话自然释放，不需要进程级的表和锁。不用：IBus 频繁重建会话，坏插件会在每个新会话里再失败一轮；按 (根目录, id, 哈希) 计，换一版内容（哈希变了）或改授权（`revision` 变大）才恢复。剩下约 1 bit 的跨字段信道（设计 U5）是已接受的风险。
- **本阶段就把 `before_commit` 当观察类事件投递。** 最强的理由是 pangu 这类只声明了 `before_commit` 的插件在开发构建里就能看到自己被调用。不用：`before_commit` 每次都在重新实例化的快照上运行，它的意义在改写；第 4 阶段把改写、可改写条件和它的观察语义一起接通，免得作者先按"只能看"写了插件、再被改写的规则改掉行为。
- **进程开关放进 HostOptions。** 最强的理由是不用新 FFI，宿主已经在写这份文档。不用：`HostOptions` 带 `deny_unknown_fields`，往一个仍被旧版输入法读取的文件里加键会让它再也开不了会话；而且开关属于进程，不属于某个会话。
- **host-api 的测试也按字节手写 wasm（client-core 的做法）。** 最强的理由是不需要 `test-support` 特性，测试依赖图和产品完全一样。不用：这里的模块要读文本、写日志、循环、trap、维护全局计数，手写指令编码容易错、难审；`wat` 已经作为 wasmi 的测试特性批准过，只在测试构建里出现。只有后台编译线程那一例需要真的 `.wasm` 文件，那一个仍然手写。
- **健康文件照设计 §4.5 字面写失败计数，按原因细分。** 最强的理由是设置页（第 5 阶段）能告诉用户一个插件怎么坏的、坏了几次，作者也能据此排查。不用：计数和原因都由脚本自己选，窗口里不到 3 次又不熔断，写出去就是一条能反复用的输出通道，远超 U5 接受的一次性约 1 bit。设置页需要的是"停用了、哪一类"，作者排查用诊断行和第 6 阶段的 `msime-pack run`。
- **trap 之后下一个事件就重建实例。** 最强的理由是偶发的一次 trap 不让插件在这个字段里剩下的时间都不工作。不用：重建要在按键路径上重跑初始化（至多 5 000 000 fuel，墙钟熔断量不到），初始化预算每轮又给满，一个每 50 次故意失败 2 次的插件就能永远隔几十个事件卡一次按键。等到下一次聚焦，代价只落在出过错的插件自己身上。
- **新字段的属性只等第一个键确认，不认聚焦前的报告。** 最强的理由是规则更简单，不用记"上一个键之后有没有报告过"。不用：那样任何宿主都收不到 `focus_in`；聚焦前报告属性是宿主能做到的，头文件写明了顺序。
- **选中候选的文本在 host-api 里派发前抄一份。** 最强的理由是不碰 input-runtime。不用：派发前不知道会选中哪个座位（数字、空格、标点结束组字、五笔顶字各走各的路），只能把整个候选列表抄一遍；runtime 在记下 `SelectionFact` 的同一处已经拿着那个文本。

## Consequences

- 收益：脚本只在 4 个桌面进程、普通字段、开发构建里运行，Windows TIP 从不打开开关并有契约检查守着；编译不在按键和 focus 路径上；坏插件只影响它自己，失败不改变上屏；健康文件和诊断行都不带文本。
- 代价：每个事件对每个插件多一次进程级 `Mutex` 的加锁（熔断窗口）和一次 `catch_unwind`；宿主一侧投递一个事件在 release 里约 170 ns。不读文本的插件也在每次 focus 重建；trap 过的插件在这个字段里剩下的时间不运行；没在聚焦前报告属性的宿主，脚本收不到 `focus_in`。缓存里的模块按根目录淘汰，同一进程里哈希不断变化的包在重读授权之前会多占一份。健康文件说不出插件失败过几次、怎么失败的。
- 在第 2 阶段各宿主接入生命周期（`msime_client_set_lifecycle`、`set_field_traits`、`key`）之前，没有任何会话满足运行条件，脚本实际上不会在产品里运行；开发构建里自测要等那些分支合入。
- release 构建打开开关是第 5 阶段的事。[脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md) 原本要求三条对策（macOS 钥匙串 MAC、Windows 提权窗口不跑脚本、Linux 与 Windows 把残余风险写进授权页）都已落地；钥匙串 MAC 实现之后被用户否决，macOS 改为和 Linux 一样把残余风险写进授权页，macOS、Linux 的开关随 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md) 打开；Windows 的残余风险要写在它原生设置应用的授权页上，那一页还没有，所以 Windows 的 release 构建仍不打开。
- 同一用户下伪造了授权的进程（U11）也能改 `revision` 让熔断复位，每复位一次就能再触发一次停用、读一次健康文件。熔断只随比以前看到过的都大的 `revision` 复位，删文件、放回旧文件都不复位；直接写一份 `revision` 更大的文件仍能让它复位，三个平台都是 U11 已接受的残余风险的一部分（[授权页](2026-10-12-script-grants-settings-page-without-keychain.md)）。同一用户下的进程还能直接结束输入法进程让它重启，重启同样清空熔断，这条在三个平台上都挡不住，每用一次都要让输入法重启一次。
- 已知缺口：x64 DLL 和 macOS 输入法可执行文件的体积增量没量；wasmi 2.0 与 1.1.0 的差分 fuzz 没做（见 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)）；pack-tool 还没有直接依赖运行时，`msime-pack run` 在第 6 阶段。

相关：[桌面端生命周期核心](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md) 推导出这里投递的事件；[沙箱运行时](2026-10-12-script-runtime-sandbox.md) 提供 `ScriptInstance`、ABI 和配额；数据类插件的门控与诊断见 [插件门控如实显示](../bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md)。

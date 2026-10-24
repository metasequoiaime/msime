# Agent Note: 脚本插件的作者工具：msime-pack run 复用产品的运行时、夹具按事件写字段、SDK 与模板

Status: implemented

## Problem

脚本插件（`kind = "script"`）的作者在第 3 阶段之后只有 `msime-pack validate`：它说得出模块合不合规，说不出插件在输入法里会怎样运行。输入法这一侧又几乎不给反馈：`log` 是空操作，健康文件只记停用类别，诊断只有一行停用原因，脚本只在开发构建、桌面、接入了生命周期的宿主里运行，修改类 import 还没接通。作者需要一个离线的办法回答：这个插件收到某串事件时用了多少 fuel、在哪一步失败、按授权和本地模式读到了什么、想怎样改写上屏。

这个办法必须和输入法用同一份代码，否则作者在工具里看到的配额、门控和快照复位与产品不一致，工具就成了误导。作者还需要不靠读源码就能写出模块的 SDK、一个能编译的例子和一份完整的规则说明；SDK 是给作者复制走的独立文件，编号一旦和运行时漂移，插件会在输入法里静默地读错字段。

## Decision

**`msime-pack run <pack> --events <fixture.jsonl> [--grant all|cap,...]`**，只在桌面编译（`crates/pack-tool/src/run.rs`，cfg 同运行时；移动端上 `run` 报用法错误）。

- 读包走 client-core 新增的 `plugins::load_script_source`：和 `validate` 同样暂存（文件夹或 `.zip`）、同样检查，再从同一份暂存里按 `DataFiles::Module` 读出模块字节。不是脚本包时报 `plugin_invalid`。编译走 `msime_script_runtime::compile`，执行走 `ScriptInstance`：授权与清单取交集、按授权和 U6 本地模式白名单复制文本、`before_commit` 在 `msime_init` 之后的快照上运行、每轮和每个钩子的 fuel 配额、import 计 fuel、修改类 import 的返回码，全部是产品代码，`run` 不重写任何一条（`command` 同样在快照上运行，见 [插件指令行](../architecture/2026-10-13-script-plugin-command-rows.md)）。
- `run` 只复刻 host-api `scripts::set` 对一个条目做的几件事：第一次需要投递一个声明过的钩子时实例化（受本轮初始化预算约束，被预算卡住时是 `deferred`）；`focus_in` 之前、`focus_out` 之后丢弃实例；trap 之后这次聚焦里不再重建（`suspended`）；本轮配额用完时跳过、运行到一半被本轮配额截断时同样不再重建，都是 `turn_exhausted`，不算失败，只有被截断时这一次用掉的达到本钩子上限的一半是 `out_of_fuel`（runtime 判定，见 [沙箱运行时](../architecture/2026-10-12-script-runtime-sandbox.md)）；同一轮里重建的实例经 `resume_turn` 接着用这一轮剩下的配额（其余钩子、`before_commit`、`command` 各一份）；初始化失败之后停用（`disabled`）。
- 与输入法有意不同的三处，写进作者文档：字段永远是普通字段；不做按次数和按墙钟的熔断，每次失败都报告；不检查进程内存总账，单个插件总放得下。（第 4 阶段接通之前还有一处：`run` 照样投递 `before_commit` 和 `candidates`，输入法当时还不投递；合并第 4 阶段后两边一致，这条差异删掉了。）
- `--grant` 按授权页的规则（同 `script_grants::set`）：必须是清单 `permissions` 的子集，满足依赖和互斥，错误文案共用 client-core 的 `script_pack::rule_message`（由 `pub(crate)` 改为 `pub`）。不写 `--grant` 什么都不授予，和刚导入、还没授权的包一样。
- 退出码：没有失败为 0；有任何失败（trap、fuel 耗尽、栈溢出、内存越界、返回负数、初始化失败）或包不合规为 1；命令行、`--grant`、夹具写错或读不出为 2。包和夹具的错误写到标准错误，标准输出只有 JSONL。

**夹具一行一个事件，字段名就是 ABI 里的名字。** `event` 是钩子名；`key: true` 表示这个事件之前宿主交来一个按键（新的一轮，配额和初始化预算给满），对应产品里不带 PASSTHROUGH 的 `msime_client_key`；`local_mode` 是 runtime 的模式名，不写是 `none`。每种事件只接受输入法会给它的字段（按 `scripts::set` 填字段的方式定），多写的字段和缺了的必填字段都是错误，`deny_unknown_fields` 挡住拼错的键。上屏文本写成 `body` 加 `tail`，和产品按 `tail_chars` 切分出的两段一一对应。`before_commit` 不写 `rewritable` 时按夹具能表达的两项（来源是 selection、raw、held_flush，本地模式是 none）推出来。`command` 写 `trigger`（必填）、`argument`、`random`（128 位十六进制，不写是全 0），`local_mode` 固定是 `command`；触发词必须是清单 `[[script.commands]]` 声明过的，`run` 在投递之前就拒绝别的触发词（用法错误），和输入法只把自己的触发词投递给主人一致。

**输出一行一个事件：** `seq`、`event`、`status`、`fuel`、`actions`、`log`，实例化时多一个 `init_fuel`，`command` 行多一个 `rows`（插件给出的 `[文本, 注释]`，没有正常返回时为空）。`status` 是 `ok`、`not_subscribed`、`Failure::as_str()` 的失败码、`init:<失败码>`、`disabled`、`suspended`、`deferred`、`turn_exhausted`。`actions` 是修改类 import 的每次调用和宿主交还的返回码（`ok`、`denied` 等），`log` 是 `log` 写出的级别和文本，`msime_init` 里写的跟在实例化的那一行上。

**运行时为此加了只旁观的调试输出。** `ScriptInstance::with_debug(module, grants, init_fuel, DebugSinks)` 在实例化之前接上 `log` 和新的 `ActionSink`（`FnMut(&'static str, i32)`），`new` 就是不带输出的 `with_debug`。修改类 import 的返回码仍由 import 自己决定（第 3 阶段一律 `DENIED`），设了 `ActionSink` 时只是把 (import 名, 返回码) 交给它；`before_commit` 的快照实例上的调用也交给同一个输出。产品里不设，行为不变。修改类 import 尚未接通时，`run` 就靠它把"请求了、被拒了"列进 `actions`，和产品给插件的返回值一致。第 4 阶段接通之后，四个修改类 import 各包一层，按自己算出的返回码（成功是 0）照样喂给它；动作本身经 `ScriptInstance::actions` 交给宿主，`ActionSink` 仍只旁观。

**枚举字段的取值进入 ABI。** `abi::scalar_values` 列出 `event_scalar` 各枚举字段的名字和数值（来源、选法、结果、原因、本地模式、输入类别、撤回方式），`abi::imports()` 交出 import 白名单的名字和能力。数值本来就是 host-api、input-runtime 各枚举的判别值，此前只写在注释里。host-api 的 `the_abi_scalar_tables_are_the_lifecycle_discriminants` 用穷举 match 逐项核对：枚举加了变体就编译不过，提醒同时在 ABI 里追加。

**SDK 和模板在 `docs/script-plugin-template/`。** `sdk/msime_script.h`（C 头文件，`import_module`/`import_name` 属性声明全部 18 个 import，宏给出全部编号）和 `sdk/msime_script.rs`（单文件，只用 `core`，按能力域分模块：`event`、`log`、`random`、`composition`、`commit`、`candidates`、`rewrite`、`rerank`、`filter`、`annotate`、`command`，外加 `status`、`hook` 和各枚举取值）。模板是一个自带 `[workspace]` 的独立 crate，`src/lib.rs` 是 pangu-spacing（`before_commit` 读 body，在中日韩文字和 ASCII 字母数字之间加空格，请求改写），`.cargo/config.toml` 默认目标 `wasm32-unknown-unknown` 并把栈缩到 64 KiB（Rust 默认 1 MiB 的栈会让线性内存最小 17 页，超过 `memory_pages` 的上限），`pack/plugin.toml` 是清单，`fixture.jsonl` 是给 `run` 的事件。`.gitignore` 排除 `*.wasm`、`target/` 和 `Cargo.lock`。`rust-toolchain.toml` 不动，README 写明要给固定的 1.97.1 加 `wasm32-unknown-unknown` 目标。`pack-tool/tests/sdk.rs` 解析头文件的整数宏和 Rust 绑定的 `pub const`，要求两边和运行时的常量不多不少，import 声明恰好是白名单。

2026-10-23：九键到达组码上限时新增 `SelectionHow::GridLimit = 11`，运行时 ABI 已追加 `grid_limit`，两份作者 SDK 也必须分别导出 `MSIME_HOW_GRID_LIMIT` 和 `how::GRID_LIMIT`，值保持为 11。作者直接写数值可以绕过缺少符号的问题，但会把 ABI 编号的知识散落在插件里；把事件归并成旧原因可以沿用旧绑定，却会丢失到达上限的选词语义，因此只补齐绑定。已有常量和 ABI 版本沿用原值，继续由现有完整性测试逐项核对 C、Rust 两份 SDK；缺项会使该测试失败。

**作者文档。** `docs/plugins.md` 新增「脚本插件」一节：在哪里运行（4 个桌面进程、只在开发构建、授权页之前要手写授权）、清单 v2、能力表、钩子表、ABI v1 的模块要求、限制（第 3 阶段的数值：模块 256 KiB、每轮 125 000、单次钩子 125 000/100 000/25 000、初始化 5 000 000、内存总账按每字节 128 倍估算）、隐私规则（含快照复位对未导出 global 和表的影响）、失败与停用、社区不收脚本包，以及 `msime-pack run` 的夹具和输出。开头"插件不能带可执行内容"一段写明唯一的例外。

**持续验证执行工具自身的全部集成测试。** CI 的既有 Script plugins 作业运行 `cargo test --locked -p msime-pack-tool`，本地完整 `verify-local.sh` 同时枚举该包的测试与全目标严格 Clippy。`run`、`sdk`、`validate` 是三个独立集成测试目标；原 CI 的 `--test sdk` 只执行 `sdk` 目标的两项测试，构建工具再运行插件夹具也不执行其余目标。按 [Cargo 的目标选择规则](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection)，不指定目标时运行整包测试，从而将夹具解析、固定 fuel 金样、包校验和命令行退出码纳入同一既有作业。沿用作业路径范围与时限，`--quick` 仍只承担原有编译门禁。

## Alternatives considered

- **只补本地完整测试，CI 继续只运行 SDK。** 最强理由是编译工具已经覆盖普通代码，完整测试可留给本地，CI 省去其余用例。不采用：现有作业已经构建同一工具与运行时，遗漏的目标直接验证作者会使用的校验、事件和退出码契约；复用现有作业运行整包不新增作业或触发范围。

- **pack-tool 依赖 host-api，直接驱动 `ScriptSet`（甚至经 C ABI 开一个会话）。** 最强的理由是实例生命周期、属性门控和熔断也是产品代码，`run` 一条规则都不用复刻。不用：`ScriptSet` 挂在会话的 `LifecycleState` 上，事件由生命周期核心从真实的派发推导出来，要驱动它就得带上引擎、词库和整个 input-runtime，夹具也只能写按键而写不了"一次上屏、body 是什么"；进程级的模块缓存、后台编译线程和熔断表在一个命令行工具里只会添乱。复刻的只有 `set.rs` 里关于单个条目的五条规则，代价是两处要保持一致，写在 Consequences 里。
- **修改类 import 的结果放进 `Outcome`（第 4 阶段大概率会给 `before_commit` 带回改写结果）。** 最强的理由是动作本来就是投递结果的一部分，不需要旁路输出。不用：第 3 阶段没有任何动作会被接受，`Outcome` 的形状应该由第 4 阶段按改写和候选变换的真实需要定；现在改它会和并行的第 4 阶段冲突。`ActionSink` 只旁观返回码，第 4 阶段可以让接通后的 import 继续喂它，或者改从 `Outcome` 取。
- **夹具里的枚举字段写数值。** 最强的理由是不需要名字表，`run` 原样交给运行时。不用：作者要对着头文件查 `origin = 3` 是什么，写错了也没人报错；名字表进了 ABI 之后，SDK 和 host-api 的一致性测试正好也需要它。
- **模板放在设计写的 `docs/plugin-template/script/`。** 最强的理由是所有模板在一个目录下。不用：`docs/plugin-template` 本身就是一个能通过校验的音效包，包里不能有子文件夹，它的 README 也写明"其他类型的模板不放在这里"；放进去会让 `the_documented_template_is_a_valid_sound_pack` 失败。改放在相邻的 `docs/script-plugin-template/`，音效模板的 README 指过去。
- **模板用 WAT 写，测试里直接编译运行，金样覆盖模板本身。** 最强的理由是模板永远在 CI 里跑过，不会烂掉。不用：作者实际会用 Rust 或 C，WAT 写的 pangu 要手写 UTF-8 解码，起不到模板的作用；设计也要求说明怎样为 `wasm32-unknown-unknown` 编译。模板的夹具由测试解析，SDK 由一致性测试守着，金样用测试里专门的 WAT 模块。
- **不写 `--grant` 时授予全部。** 最强的理由是作者最常想看的是插件拿到全部能力时的样子。不用：产品里导入之后什么都没授予，默认值和产品一致更不容易误读；`--grant all` 只多打几个字。
- **测试模块全部按字节手写，不经 `test-support`。** 最强的理由是不扩大 `test-support` 这条还待确认的路线。不用：金样用的模块要读文本、写日志、改全局计数、死循环、按计数转圈，手写指令编码难审；走命令行的几例仍然用手写的二进制，覆盖从磁盘读包的完整路径。

## Consequences

- 持续验证增加 `run`、`validate` 目标的编译与执行，以及本地完整验证中的工具 Clippy；复用现有工具依赖，CI 仍受原作业时限约束。这里只补已有测试的执行入口，不扩大插件运行时的验证结论。
- 收益：作者在本机就能看到每个事件的 fuel、失败原因、授权和本地模式下读到的东西、想做的改写；这些都来自产品的运行时，配额、快照和门控不会和输入法不一致。SDK 的每个编号和 import 名都有测试对着运行时核对，host-api 的枚举判别值也有测试对着 ABI 核对。
- 代价：`run` 复刻了 `scripts::set` 的五条实例规则（延迟实例化与初始化预算、聚焦丢弃、trap 后不重建、同轮续用配额、初始化失败停用）。改了 `set.rs` 里这些规则，要同步改 `run.rs`，没有测试替两边对拍。
- 金样里的 fuel 数值取决于 wasmi 的计量。升级或回退 wasmi（沙箱运行时那篇写的 1.1.0 退路）会让 `crates/pack-tool/tests/run.rs` 的金样全部变化，这正是要人看一眼的信号，不是噪音。
- 第 4 阶段接通修改类 import 后（合并时已做）：接通后的 import 继续把 (名字, 返回码) 交给 `ActionSink`；SDK 里 `rerank::order`、`filter::hide`、`annotate::set` 的参数含义（小端 u32 行号数组、行号加注释）和第 4 阶段的定义一致，头文件、绑定和文档里"宿主一律返回 DENIED"的说法已删；`run` 的"`before_commit` 照样投递"差异说明已删，模板 README 的输出按 `wasm32-unknown-unknown` 重新跑过。
- `test-support` 特性现在有两个使用方（host-api 和 pack-tool 的测试）。沙箱运行时那篇写明这条路线在依赖批准范围之外、需要单独确认；不批准时两处的 WAT 模块都要改成手写二进制。
- pack-tool 新增的 `serde`、`serde_json` 是工作区里已有的依赖，`Cargo.lock` 只多了 `msime-pack-tool` 自己的三条依赖边，没有新的第三方 crate。
- 模板在 Mac Studio 上没有 `wasm32-unknown-unknown` 目标（`rust-toolchain.toml` 只装了 rustfmt 和 clippy），验证时改用已经装好的 `wasm32-wasip2`、链接器换成 `rust-lld` 编出 core 模块（2158 字节，版本头 `01 00 00 00`），`msime-pack validate` 输出 `ok pangu-spacing script 1.0.0`，`run --grant all` 的输出就是模板 README 里的那四行；按 README 的 `wasm32-unknown-unknown` 命令本身没有跑过。C 头文件只在本机用 clang 的 wasm32 后端编译检查过（`-fsyntax-only` 和 `-c`），本机没有 `wasm-ld`，没有链接过。
- 模板的 pangu-spacing 和内置的[中英文之间加空格](2026-10-12-chinese-english-spacing.md)做的是同一件事。内置开关排在全角转换之后、`before_commit` 之前，第 4 阶段接通改写后，脚本读到的 body 已经加过空格；pangu 只在相邻处插入，不会重复加。

## Verification

- `cargo test --locked -p msime-pack-tool --no-fail-fast`：`tests/run.rs` 17 项、`tests/sdk.rs` 2 项、`tests/validate.rs` 7 项，共 26 项通过、0 忽略；lib、bin 和 doc-tests 各 0 项。覆盖固定 fuel 金样、夹具和 `--grant` 错误、真实包与命令行退出码。
- 完整包选择的真实负对照：临时反转现有模板校验的退出码断言，旧 `--test sdk` 仍两项通过、退出 0，完整包命令捕获该断言失败、退出 101；精确恢复源码并核对摘要后，完整 26 项通过。
- `cargo clippy --locked -p msime-pack-tool --all-targets -- -D warnings` 通过。另一次性提取真实本地测试和 Clippy 循环，以实际负对照输出驱动命令替身：旧清单均退出 0，新清单均退出 1。该检查只证明清单选择与失败传播，不代替整套本地验证。
- `cargo test -p msime-script-runtime`：新增 `debug_sinks_observe_denied_modifications_and_init_logs`、`the_scalar_value_tables_are_one_to_one`。
- `cargo test -p msime-client-core script`：新增 `load_script_source_hands_over_what_validate_accepted`。
- `cargo test -p msime-host-api the_abi_scalar_tables`：枚举判别值与 ABI 名字表一致。
- `cargo clippy -p msime-client-core -p msime-host-api -p msime-script-runtime -p msime-pack-tool --all-targets -- -D warnings` 干净。

相关：[沙箱运行时](../architecture/2026-10-12-script-runtime-sandbox.md) 提供 `ScriptInstance`、配额与快照；[脚本插件进入插件包体系](../architecture/2026-10-12-script-plugin-kind-and-grants.md) 定义清单 v2、`validate` 与授权规则；[脚本插件在输入法进程里](../architecture/2026-10-12-script-plugins-in-the-input-method-process.md) 是 `run` 复刻的实例规则的出处。

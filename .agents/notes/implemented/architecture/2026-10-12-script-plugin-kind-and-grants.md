# Agent Note: 桌面脚本插件进入插件包体系：只对 script 放开"包里没有会运行的东西"，清单 v2、内容哈希与 script-grants.json

Status: implemented

## Problem

插件包体系建立时立过一条原则，写在 `crates/client-services/src/plugins.rs` 的模块文档里：包里没有任何会运行的东西，类型是封闭集合，申请任何权限的包一律拒绝。音效、音乐、指令表、特效、短语表、辅助码表、单词本、符号集都是被完整解析的数据，宿主只读它们。

脚本插件（`kind = "script"`）要打破这条原则：包里带一个 core wasm 模块，在接收全部击键的桌面输入法进程里运行，并按用户授予的能力读取组字、上屏文本和候选。打破它需要回答几件事：旧客户端和移动端怎样保证仍然拒绝这种包；导入时和输入法加载时怎样用同一份规则判断模块合不合规；用户的授权记在哪里、怎样绑定到用户看过的那一份内容、怎样在删包和重新导入时失效；设置页在授权页落地之前不该看到什么。

## Decision

**只对 `script` 推翻原则，只在桌面。** `PluginKind::Script` 变体及其在 `ALL`、`as_str`、`is_builtin`、`PluginSummary::kind`、`PluginContent`、清单解析各处的分支，都挂 `cfg(any(target_os = "macos", target_os = "windows", all(target_os = "linux", not(target_env = "ohos"))))`。`ALL` 按 cfg 写两份：桌面是 `[Self; 9]`（`Script` 排最后），其他目标保持原来的 `[Self; 8]`。所以在 iOS、Android、HarmonyOS 和 wasm32 上，`parse("script")` 仍返回 `None`，`scan` 和遗留目录清扫不遍历 `script/`，报错原文一字不变；`plugins.rs` 里一对编译期断言（非桌面 `ALL.len() == 8`、桌面 `== 9`）在任何目标只要编译 client-core 就检查一次，不依赖哪台 CI 机器跑了测试。`msime-script-runtime` 只在同一个 cfg 下作为 client-core 的依赖，移动端的 `cargo tree` 里没有 wasmi（已在 `aarch64-linux-android`、`aarch64-apple-ios`、`aarch64-unknown-linux-ohos` 上核对）。模块文档里的原则改写为"数据类插件里没有会运行的东西"，并写明唯一的例外。

**清单规则按类型。** 桌面上先看版本：既不是 1 也不是 2 时照旧报"不支持这个 schema_version"（报错顺序与移动端一致）；再认类型；然后 `script` 必须写 2，其余 8 种仍只认 1。旧客户端和移动端在 `schema_version != 1` 处就拒绝 v2 包，服务端的 `community_plugin_archive.go` 同样只收 1 且拒绝非空 `permissions`。`permissions` 只对 `script` 放行：每项必须在 `Capability` 白名单里、不能重复，依赖必须一起申请，`rewrite_commit` 与 `read_composition` 不能同时申请；数据类插件照旧"插件不能申请权限，permissions 必须为空"。

**`script_pack.rs`。** `[script]` 只有 5 个键：`abi`（只能是 1）、`module`（平铺的 `.wasm` 文件名）、`hooks`（非空、不重复的钩子名）、`memory_pages`（可选，1 到 64，默认 16）、`commands`（可选，`[[script.commands]]`，插件在 `/` 指令里声明的触发词和标题，和能力 `provide_commands`、钩子 `command` 三者缺一不可，规则见 [插件指令行](2026-10-13-script-plugin-command-rows.md)）。能力白名单现在是 8 项，第 8 项是 `provide_commands`。模块按 `DataFile{max_bytes: 256 KiB, extension: "wasm"}` 登记（上限由运行时的 `limits::MAX_MODULE_BYTES` 定，理由见沙箱运行时那篇），走现有的 `check_files`，文件数上限仍是 `MAX_PACK_FILES`。完整载入（导入、`validate`、`scan`）时读出模块，用 `msime_script_runtime::validate` 按 ABI v1 校验（只编译，不执行），所以 `msime-pack validate` 和导入天然是同一份校验。`load(root, id)` 给输入法进程用：清单和文件照常检查，模块读出来但不在这里编译，字节和内容哈希出自同一次读取，交给调用方的 `compile`（那就是同一份校验，不必先校验一遍再编译一遍）。

**内容哈希。** `sha256("msime-script\0" ‖ len ‖ plugin.toml ‖ len ‖ 模块)`，`len` 是后一段的字节数、8 字节小端，64 位小写十六进制；说明文件不计入。前缀做域分隔，长度做段分隔，字节在两段之间挪动不会得到同一个哈希。哈希用的清单字节就是刚解析的那一份。

**`script_grants.rs`：`<plugins root>/script-grants.json` 的唯一读写方。**

- 格式 `{"format_version":1,"revision":N,"plugins":[{"id","hash","enabled","granted":[...]}]}`，数组顺序就是执行顺序。和 `mentions.json` 同级，不进偏好文档、不同步（偏好文档会被宿主复制、被账号同步读取，`PluginPreferences` 还带 `deny_unknown_fields`）。
- 读取方不加锁、容忍未知字段；`format_version` 不认识、文件读不出、不是合法 JSON、是符号链接时，按"没有任何脚本启用"处理。单个条目不合规（id 或哈希的形状不对、授予的能力违反规则）只跳过这一条；同一个 id 只认第一条；不认识的能力名丢掉。启用的条目按顺序最多 8 个。
- 写入只在这里，持有 `import::lock_plugin_root`（与导入、删包同一把锁），整份原子替换，每次写入 `revision` 都变大（取上一份加 1 和当前时间微秒数中较大的一个，上限 2^53，见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md)）。`set(root, id, hash, enabled, granted)` 在锁里重新读包、重新算哈希，与调用方给出的不一致时返回 `plugin_script_changed`；`granted` 必须是清单 `permissions` 的子集并满足规则；模块必须通过校验。启用时做准入：同时启用不超过 8 个，进程内存总账（`limits::fits_memory_budget`：每个脚本的线性内存预留加按 4 倍模块大小估算的字节码，再加一份值栈）不超过 64 MiB，否则返回 `plugin_script_limit` 或 `plugin_script_memory`。`format_version` 不认识的文件不覆盖（`plugin_script_grants_format`），读不出的文件整份重写。
- 删包时在同一把锁里、移走包目录之前去掉条目；导入脚本包时在替换包目录之前更新条目：内容哈希没变、并且是原地覆盖一个已安装的包，授权保留；否则停用、清空授予项、保留旧哈希（授权页据此显示"已更新，需要重新授权"）。先改授权再动包：写授权失败时包没变，换包失败时授权已经收紧。包目录被手工删掉后再导入同样的内容，留下的条目也不会复活。
- 输入法进程只认 `hash` 等于已安装包当前内容哈希的条目，实际能力是 `granted` 与清单 `permissions` 的交集。

**威胁模型与发布构建。** 授权文件和插件目录都在用户可写的宿主状态根下，同一用户下任何没有特权的进程都能放进一个 script 包、再手写一条带正确哈希的授权。在加固落地之前，发布构建里 `msime_client_scripts_enable` 恒返回 false（见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md)），这个文件只在开发构建里起作用。授权不签名：设计原本要在 macOS 上用钥匙串密钥给条目签 MAC，实现之后又去掉了，用户决定授权不使用钥匙串；读取方容忍未知字段，早期开发构建写过的 `mac` 字段照常忽略。三个桌面平台的残余风险相同，macOS 和 Linux 写在授权页上，发布构建打开开关见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md)。输入法进程在聚焦时按带 inode 的戳重读它，后台编译线程用 `script_pack::load` 读包并核对哈希。

**设置页在第 5 阶段之前看不到脚本插件。** `PluginCatalog::without_scripts()` 去掉脚本包和脚本文件夹的问题行。授权页落地之后只有 host-api 的 `msime_client_plugins` 不带 `script_page` 的目录命令（Android、HarmonyOS 用）还经过它，桌面外壳的 `plugin_catalog` 和 Windows 原生设置应用（带 `script_page`）列出脚本插件，`host_surface.script_plugins` 在三个桌面平台为真，见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md) 和 [Windows 授权页](2026-10-12-windows-script-grants-page.md)。导入命令不拦脚本包，开发构建里自测时还要靠它（或手工拷进 `script/<id>/`）装包，再手写授权。`PUBLISHABLE_KINDS` 和 `KINDS_DECLARATION` 不变，`script` 不能发布到社区。

## Testing

- `cargo test -p msime-client-core` 的插件测试通过，覆盖：清单版本 × 类型矩阵（script 只认 2、其他只认 1、其余版本先报版本）；数据类插件申请白名单里的权限照旧被拒；权限白名单、重复、依赖、`rewrite_commit` + `read_composition` 被拒；`[script]` 的每条规则；模块不是 wasm、导入了没申请的能力、申请了没用到、超过模块上限被拒，恰好上限可以；摘要里的声明和内容哈希；哈希覆盖清单和模块、不覆盖说明文件；`load` 交出的字节就是哈希的那一份；`scan` 列出脚本包而 `without_scripts` 去掉它们；脚本不能发布；授权的哈希绑定、原样重新导入保留、换内容重新导入作废、删包清除、手工删目录后重新导入不复活；授予项必须是清单的子集；未知格式、坏 JSON、符号链接什么都不启用，未知字段和单个坏条目不影响别的；同时启用的个数和内存总账；授权失败的错误码。移动端的既有测试原样（`script` 仍不是已知类型），桌面上对应的三处断言按 cfg 分开。
- host-api 的 `plugins_boundary_lists_imports_removes_and_keeps_the_name_list` 断言目录命令不报脚本文件夹。
- `cargo check -p msime-client-core -p msime-host-api --all-targets` 在 `aarch64-apple-ios`、`aarch64-linux-android`、`aarch64-unknown-linux-ohos` 上通过（编译期断言 `ALL.len() == 8` 随之成立），`cargo check -p msime-desktop --target aarch64-linux-android --lib` 通过；桌面上 `cargo clippy --all-targets -- -D warnings` 干净。
- host-api 一侧的测试（授权的哈希绑定在输入法进程里同样生效：哈希对不上的条目从不运行；从磁盘上的包经后台编译线程走完整路径）见 [脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md)。

## Alternatives considered

- **`Script` 变体不加 cfg，移动端在 `scan` 里跳过它，或另设一个 `DESKTOP_ALL`。** 最强的理由是少写一堆 cfg，类型在所有目标上形状一致，序列化也一致。不用：移动端的 `PluginKind::ALL`、`parse("script")` 的结果和报错原文都会变，`scan` 和遗留目录清扫要各自记得跳过，任何新的 `ALL` 使用方忘了跳过就在移动端碰到脚本包；而且 client-core 在移动端要么也依赖解释器去校验模块，要么对同一种类型给出两套校验。变体本身挂 cfg，编译器替我们找全每一处穷举。
- **授权写进偏好文档的 `PluginPreferences`。** 最强的理由是设置页已经有完整的偏好读写和变更通知，不必为一个文件另写读写方和刷新。不用：偏好文档会被宿主复制、被账号同步读取，授权不该跟着换机器；`PluginPreferences` 带 `deny_unknown_fields`，往里加键会让还在读它的旧版输入法打不开会话。
- **授权只按 id 记，不绑定哈希。** 最强的理由是包升级后用户不必重新授权。不用：导入一个同 id 的新版本就能继承旧版本的读写上屏能力，用户授权的是一份代码、运行的是另一份。
- **导入或重新导入时直接删掉条目，而不是停用并保留旧哈希。** 最强的理由是状态更少，读取方不必处理"有条目但哈希对不上"。不用：授权页需要区分"从没授权过"和"授权过、但包已更新"，后者要提示重新授权。读取方本来就要核对哈希，留着旧哈希不增加任何判断。
- **`load` 在读出模块时也校验一遍。** 最强的理由是任何拿到字节的调用方都确定这是合规模块。不用：输入法进程拿到字节后马上就要 `compile`，而 `compile` 就是那份校验；在后台编译线程上把同一个模块编译两遍，只是多花一倍时间。
- **准入检查放到输入法进程加载时，授权写入不管。** 最强的理由是写入方拿不到真实的字节码大小，只能估算。不用：设计要求在授权和启用时拒绝，而不是让脚本在运行时争抢；用户在授权页上就该知道为什么开不了。估算按专门构造的模块实测到的膨胀上界记（每字节 128 倍加固定开销，见 [沙箱运行时](2026-10-12-script-runtime-sandbox.md)），输入法进程实例化时按同一个函数再记一次账。

## Consequences
- 收益：移动端的类型集合、报错原文和依赖图逐字节不变；导入、`msime-pack validate` 和输入法加载走同一份模块校验；授权绑定在用户看过的那份内容上，换内容就作废。
- 代价：`plugins.rs` 里多了一批按 cfg 分开的分支；授权文件有了第二种写入时机（导入和删包时在同一把锁里改条目）。
- 授权文件可以被同一用户下的任何进程伪造。第 5 阶段之前只有发布构建里 `scripts_enable` 恒为 false 挡着；设计原本要求打开发布构建之前落地三条对策（macOS 钥匙串 MAC、Windows 提权窗口不跑脚本、Linux 与 Windows 把残余风险写进授权页）。钥匙串 MAC 实现之后被用户否决，现在 macOS 和 Linux 一样把残余风险写进授权页、发布构建打开开关，见 [授权页](2026-10-12-script-grants-settings-page-without-keychain.md)；Windows 的原生设置应用有了同样的授权页之后发布构建也打开，见 [Windows 授权页](2026-10-12-windows-script-grants-page.md)。
- 准入检查按编译膨胀的上界估算（每字节 128 倍加 256 KiB 固定开销，再加一份值栈），不是按普通代码的约 3.3 倍：产品里量不到真实值，没有"加载时按实际编译结果再查一次"的那一层。所以普通插件在总账里也记得偏多：几十 KiB 的插件每个约 5 MiB；256 KiB、64 页的模块约 36 MiB，同时只能启用一个。
- `set` 每次启用都要读出其他已启用的包算总账，最多 8 个、每个至多 256 KiB，只在设置页操作时发生。
- `scan` 现在会完整编译每个已安装的脚本包（dev 构建里 512 KiB 约 20 ms），设置页的目录命令在后台线程上跑，输入法进程的按键和 focus 路径不调用 `scan`。每次校验用一个随结果释放的 Engine，反复刷新目录不会越攒越多。
- 导入命令不拦脚本包。页面的类型表随授权页加上了 `script`（「脚本插件」），导入脚本包后的提示写得出类型名。

相关：[桌面端生命周期核心](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md) 产生投递给脚本的事件；[沙箱运行时](2026-10-12-script-runtime-sandbox.md) 提供本篇用到的 `Capability`、`Hook`、模块校验和 `limits`；数据类插件的门控与诊断见 [插件门控如实显示](../../implemented/bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md)；社区可发布类型见 [特效包可分享](../../implemented/feature/2026-10-10-community-effect-packs-shareable.md)。

后续：官方脚本插件（`BUILTIN_SCRIPT_PACKS`）随桌面安装包分发，从只读的内置目录读，授权照样写在本篇的 `script-grants.json`、绑定内置那一份的哈希，导入同 id 的包按本篇的保留 id 规则拒绝，见 [官方插件包随三个桌面安装包分发](2026-10-13-official-plugin-packs-in-desktop-installers.md)。

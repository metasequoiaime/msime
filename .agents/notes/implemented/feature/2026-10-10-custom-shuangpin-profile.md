# Agent Note: 自定义双拼方案（引擎与共享层）

Status: implemented

## Problem

#6572 要两样东西：九键双拼（米旮旯 2/3、辜氏 2 和它的低重码版），以及能配置声母、韵母键位并保存、导入、导出的自定义双拼方案；issue 里说内置方案做不全时，先给自定义配置也行。

双拼方案原来是 Engine 里四张编译期的静态表（`shuangpin/profile.rs` 的小鹤、自然码、手到、微软），会话从头到尾拿 `ShuangpinProfileKind` 这个枚举值到处调 `profile(kind)` 取表，宿主经 `EngineOptions::shuangpin_profile` 的 0–3 选其中一张；共享偏好 `shuangpin_profile` 也只认这四个名字。微软双拼把 ing 放在 `;` 上，这一点在会话、编辑、宿主快照三处都写成了 `profile == Microsoft`。没有任何地方能让用户给出自己的表。

## Decision

这次只做自定义 26 键双拼在 Engine 和共享层的部分；九键双拼另见 [九键双拼](../../proposed/feature/2026-10-10-nine-key-shuangpin.md)。

- **表的形状与内置方案相同**：`ShuangpinCustomTable` 三部分，`initials` 只给 `zh` `ch` `sh`（单字母声母固定在自己的字母键上），`finals` 给齐 33 个韵母，`zero_initials` 给 12 个零声母音节各一个两键编码。键是字母或 `;`，大写当小写。`custom::FINALS`、`ZERO_INITIALS` 就是内置方案的单位集合，`builtin_profiles_use_the_same_units` 核对四张内置表与它们一致。
- **合不合法只由 Engine 判定**（`shuangpin::custom`）：单位要认识、不重复、不缺；声母和韵母一个键，零声母两个键；`;` 只能做韵母或零声母编码的第二个键（方案只在奇数长度的片段后接受它）；多字母声母不能放在单字母声母的键上，也不能两个共用一个键；任何一个两键编码不能解出两个不同的音节，零声母编码不能占掉一个声母加韵母的音节（零声母表先查，占掉的那个音节就打不出来），零声母编码之间不能重复。错误是带位置的枚举 `CustomProfileError`，`Display` 是给日志和代理看的英文。四张内置表写成自定义表都能通过，而且 26×27 个编码逐个解出来与内置方案相同（`builtin_tables_pass_and_decode_as_the_builtin_profile`）。
- **合法的表变成同样的 `&'static ShuangpinProfile`**，按内容只留一份：`custom_profile` 先把表规范化（单位换成常量、键小写、按常量顺序排），在已留下的方案里找内容相同的那份，找不到才逐个检查编码、把表 leak 成静态数据存起来。所以切分、转换、辅助码、`;` 键全部走内置方案那一套，没有第二份解码；进程里只为用户实际用过的每一张不同的表各留几 KB，不合法的表什么也不留。`validate_custom_profile` 也走这条路，宿主先校验再建会话时第二次是现成的。
- **会话持有解析好的方案，不再持有枚举**：`ImeSession`、`ProviderRegistry`、`Scheme::new`、`CandidateQueries`、`InputSession` 改为拿 `&'static ShuangpinProfile`，原来每处 `profile(self.profile)` 变成直接读字段。`SessionOptions` 和 `EngineOptions` 各加一个 `shuangpin_custom_profile: Option<ShuangpinCustomTable>`，`ShuangpinProfileKind::Custom = 4`（名字 `custom`）是 ABI 值；`session_profile` 在建会话时解析：内置取内置表，`Custom` 校验用户的表，缺表或不合法时，会话里有双拼就报 `INVALID_CUSTOM_SHUANGPIN_PROFILE: <原因>`，没有双拼就按小鹤，与不合法的 ABI 值同样处理。`profile(kind)` 改为返回 `Option`，`Custom` 没有内置表。
- **`;` 键看表，不看方案名**：`ShuangpinProfile::uses_semicolon_key()` 取代三处 `== Microsoft`。它看两处：有韵母放在 `;` 上，或者有零声母编码的第二个键是 `;`。校验允许后一种排法，只看韵母的话这样的零声母音节能存进去却打不出来。宿主快照的 `microsoft_shuangpin` 字段名不变，含义写成「当前双拼方案用得到 `;` 键」。Linux 和鸿蒙按这个字段决定把 `;` 送进 Engine，自定义方案在这两处直接能用。Windows 的 TIP 不读快照，它在组字时只凭 Server 推来的 `MicrosoftShuangpinChanged` 开关把 `;` 当输入键，这个开关原来只看方案名是不是 `microsoft`。现在 Server 用 `EditPolicy.h` 的 `shuangpin_uses_semicolon_key` 从偏好里按同一条规则算（自定义方案读偏好里的表），否则自定义方案放在 `;` 上的音节在 Windows 上会被当成标点上屏。偏好里的表不合法时，Engine 按小鹤运行，这个开关仍按表打开。这种表 MCP 存不进来，只可能来自手改的偏好文件，那时 Server 照旧按快照判断，和延迟应用期间新旧方案不一致的情形一样。
- **只有名字、拿不到表的地方把 `custom` 当作没有表**：`shuangpin_key_hints("custom")` 和 `shuangpin_zero_initials("custom")` 返回空（标错键面比不标更糟，与不认识的名字一样）；后端 JSON 请求的 `profile: "custom"` 按小鹤，与不认识的名字一样。
- **共享偏好**：`ShuangpinProfile::Custom`（`"custom"`）和 `Preferences::shuangpin_custom_profile`（三个单位到键的 JSON 对象，空表不写进文件）。`client-core` 不依赖 Engine，只挡结构上明显不对的东西（每部分最多 64 项、单位是小写字母且不超过 8 字节、键是可见 ASCII 且不超过 4 字节）。选别的方案时表留着，切回 `custom` 还能用。导入导出直接用这个 JSON 形状。
- **host-api 先校验再交给 Engine**：`apply_shuangpin_profile` 在建 Engine 选项时调 `validate_custom_shuangpin_profile`，表不能用就按小鹤运行，偏好变更的响应里带诊断 `Custom shuangpin profile unusable because <原因>; using Xiaohe.`（只在实际跑双拼时报告）；建会话时没有地方带原因，同样按小鹤。一张写坏的表不会让 Engine 建不起来、把输入法卡在旧会话上。
- **账号同步不带它**：账号 schema 没有 `custom`，也装不下表。选着自定义方案时不上传 `input.shuangpin_schema`，账号保留上次的值；反过来应用账号文档时，本机选着自定义方案就不拿账号里的内置方案覆盖它，否则 Android 合并上传后再应用回来会把本机的自定义方案改回账号里那个旧值。账号里出现 `custom` 时作为本机不认识的值跳过。与 `account_input_schema` 对待账号还没收录的方案一致。
- **MCP**：`get_preferences` 显示 `shuangpin_profile: custom` 和保存着的表，`update_preferences` 可以整张替换表、选 `custom`；给了表或要选 `custom` 时先用 host-api 的同一个校验，不能用就拒绝并说明原因，不会存下一张输入法只能悄悄退回小鹤的表。这是目前唯一能在产品里设置自定义方案的入口（桌面端经代理）。

## Alternatives considered

- **不引入 leak，把 `ShuangpinProfile` 改成持有 `Box<[(Box<str>, Box<str>)]>`、会话持有 `Arc<ShuangpinProfile>`**：最「干净」，没有任何常驻内存。但表的 `&'static str` 一路传到 `source_by_code`、`shuangpin_zero_initials` 等返回值，切分和转换函数也都按 `&'static` 写，改成拥有型要动整条双拼热路径的签名，还要在 `ShuangpinScheme`、`ShuangpinEngine`、`CandidateQueries` 之间克隆 `Arc`。按内容只留一份之后，泄漏的上限是「用户在这个进程里用过多少张不同的合法表」，每张几 KB；换来热路径一行不改。
- **全局「当前自定义表」，`profile(Custom)` 去读它**：改动最小，会话里继续存枚举。但同一进程里可以有多个会话（host-api 每个宿主会话一个，测试并行），一个会话换表会改掉另一个会话正在用的方案，正是 ARCHITECTURE.md 说的「靠共享内存里的一个全局变量」。
- **`profile(Custom)` 返回小鹤，保持返回值不是 `Option`**：少改二十几处测试里的调用。但这是一个静默回退的陷阱，以后谁在拿不到表的地方调它都会不知不觉按小鹤跑；改成 `Option` 之后，后端和键面提示两个只有名字的地方各自写明怎么处理 `custom`。
- **在 `client-core` 里做完整校验**：偏好一写进去就知道合不合法。但 `client-core` 不能依赖 Engine（四条边界之一），合法性又取决于 Engine 接受的音节集合（`accepted_syllables`，含历史排除项），在 `client-core` 再写一份规则就是两份实现开始漂移。`client-core` 只限大小，合法性由 Engine 回答，写偏好的一方（host-api、MCP，以后的设置页）在保存前问它。
- **自定义表允许重新安排单字母声母**：有的输入法支持。但现有方案没有一个这样做，单键片段也一直按「多字母声母的键，否则字母本身」读（`initial_for_key`）；放开它要改切分规则、键面提示和简拼。issue 点名的方案也不需要。
- **把 `microsoft_shuangpin` 改名成 `semicolon_final`**：名字更准。但 Windows（TSF 与 Server 的 IPC 回复）、Linux、鸿蒙都按这个字段名读，改名要动跨进程协议；只改文档说明含义，字段名留着。

## Consequences

- **收益**：Engine 能跑任意一张合法的 26 键双拼表，与内置方案共用同一套切分、整句、辅助码和 `;` 处理；表不合法时有带位置的原因，宿主按小鹤运行而不是建不起会话。默认行为不变：没有人选 `custom` 时，偏好文件、Engine 选项和账号同步都与原来相同。
- **代价**：进程会为用过的每一张不同的合法表常驻几 KB，不能回收。`EngineOptions` 多一个字段，每个构造处都要写。
- **还不能用的地方**：六个平台的设置页和触屏方案选择器都还只列四种双拼。macOS 设置保存其他选项、Android 切换方案时原来会把 `custom` 规范成小鹤写回，#6657、#6660 已改为保留它，但两处都还没有选择或编辑它的入口；iOS 和鸿蒙按触屏方案写偏好，选不到它。自定义方案的键面提示还没有（FFI 只收方案名），React 设置页的 `ShuangpinProfile` 类型也没有 `custom`。这些是各平台接入的后续工作。

## Verification

- `rbuild cargo test -p msime-engine`：库 1592 项、golden 31 项全过。新增 `shuangpin::custom` 的 10 项（内置表等价、去重、各类非法表、非法表不留下）、`host::tests::custom_profile_runs_the_users_table`（`ahx;oa` 在 zh→a、ing→`;`、零声母 o 引导的表上切成 zhang'xing'a，`;` 只在奇数位置收）和 `custom_profile_needs_a_valid_table_only_when_shuangpin_is_enabled`。
- `rbuild cargo test -p msime-client-core`（1183 项）、`-p msime-host-api`（395 项，新增 `custom_shuangpin_profile_reaches_engine_options_or_falls_back_to_xiaohe`）、`-p msime-input-runtime`、`-p msime-mcp-server`（新增 `a_custom_shuangpin_profile_needs_a_table_the_engine_accepts`）全过；`cargo clippy` 上述五个 crate `--all-targets -D warnings` 无警告。
- 热路径：在 Mac Studio（负载 77–95，数字有噪声）上用出货词库、从评测句子集转出的 299 句小鹤键序（3752 键）逐键计 `character` + `snapshot`，三轮共 11256 个样本。分支上内置小鹤 p50 0.40–0.49 ms、自定义的等价表 p50 0.39–0.45 ms，基线提交上内置小鹤 p50 0.46–0.49 ms；两种方案逐键首选完全相同。校验一张新表约 50 µs（进程里第一次还要建音节集合，约 120 µs），已经留下的表约 3–5 µs。基准是临时程序，没有提交。
- 审查后的补充：`cargo test -p msime-engine --lib shuangpin::`（69 项）和 `custom_profile`（新增 `custom_profile_accepts_semicolon_used_only_by_a_zero_initial_code`：ing 不在 `;` 上、零声母 a 编码为 `o;` 时 `xko;` 解成 xing'a，快照的 `microsoft_shuangpin` 为真）、`cargo test -p msime-client-core --lib settings_sync`（16 项，本机自定义方案不被账号里的小鹤覆盖）全过。Windows 的 `windows-edit-policy`（`shuangpin_uses_semicolon_key` 的各种偏好）在 macOS 上用 clang++ `-Wall -Wextra -Werror` 编过并通过，Server 和 TIP 的改动靠 CI 的 Windows 构建检查，没有在 Windows 上手动试。

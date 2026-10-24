# Agent Note: 混输总开关与首页英文候选数

Status: implemented

## Problem

Windows 差距任务书 A6 与 A9 是同一混输域里的两个缺口：

- A6：`general.cn_en_mixed_input_first_page_count`（中文状态下首页最多展示 1–3 个英文候选，默认 1）。新架构 `EnglishInputOptions` 只有 `mixed_candidates` 与 `minimum_prefix`，第一行英文进优先位置、其余一律接列表末尾，没有「让第二、三行英文跟进首页」的能力。
- A9：`utility.mixed_candidates` 混输总开关。Windows 关掉后英文、emoji、颜文字、符号、快捷短语、行内日期时间都不再混进候选，各分开关原值保留。新架构只有分键（`mixed_input.{english,emoji,kaomoji,symbols,japanese}`、`local_modes.*`），没有一键全关的入口。
- 附带判定：`mixed_input.minimum_prefix` 桌面宿主不读（`host_english_minimum_prefix` 固定默认值），要判定是缺口还是有意收敛。

## Decision

A6 归 A，落成共享偏好 `mixed_input.first_page_count`（`u8`，1–3，默认 1；为 1 时不写入文档，旧文档缺省按 1 处理）。装配链：`Preferences::validate` 限制 1–3 → `EngineOptions.english_first_page_count` → `EnglishInputOptions.first_page_count` → `queries.mixed` 连同会话请求里的 `first_page_rows` 传给 `insert_mixed_rows_with_japanese`。混排时记录第一行英文的落位 `english_seat`（日文行插在它前面时座标后移），其余英文取 `min(first_page_count - 1, first_page_rows - seat - 1)` 行紧跟其后，放不下的照旧接列表末尾；`first_page_rows` 为 0 表示不展开。在线座位整理（`normalize_online_slots`）在云候选把混输组重新归拢到头段时，英文组头段按同一个 `english_first_page_count` 留位（`Runtime::set_english_first_page_count`，宿主状态，改偏好不必等会话重建）。设置页「中英混输」组给「首页英文候选数」1–3 选择器，中英混输关掉时禁用。

A9 归 A，落成真实的引擎级门而不是 UI 批量写：共享偏好 `mixed_input.enabled`（默认开，开着时不写入文档）→ `EngineOptions.mixed_candidates` → `SessionOptions.mixed_candidates` → `InputSession.mixed_candidates_enabled` 与 `NineKeySession.mixed_candidates_enabled`。门点与 Windows `GetConfiguredMixedCandidatesEnabled()` 一一对应：26 键 `queries.mixed` 的 `anything_enabled`（英文、emoji、颜文字、符号、日文行一并拦下）、普通候选里的快捷短语追加、`insert_inline_date_time`；九键的英文混输判定、`expressive_candidates`、行内日期时间插入。各分成员原值不碰，重开即恢复；本地模式本身（T、K、M）不受影响，与 Windows 一致——总开关只管「混进普通候选」，不管模式入口。设置页「中英混输」组顶部给「候选混输」总开关，关掉时全部子行禁用但保留原值。

`minimum_prefix` 桌面不读判定为**有意收敛**，不接。证据：`HostPlatform::reads_mixed_input_minimum_prefix` 只对 Android、iOS、HarmonyOS 返回真，注释写明桌面固定 5 的理由（调低会让常见简拼被英文词抢首选，不喜欢英文的人有「中英混输」开关）；`HostCapabilities::mixed_input_minimum_prefix` 按同一能力位在桌面设置页藏掉该行；`host_english_minimum_prefix` 对桌面强制 `DEFAULT_MIXED_INPUT_MINIMUM_PREFIX`；`english_minimum_prefix_is_fixed_where_the_host_does_not_read_it` 已把这个事实钉成测试。这不是漏接，是已有的平台能力设计，本笔记钉住这个判定。

## Existing notes

[英文领先不可拆输入](../feature/2026-10-12-mixed-english-leads-unsegmentable-input.md) 同属混输域，记录 `minimum_prefix` 默认 5 的取舍，本次扩展其同一结构但不改变归属。

## Alternatives considered

- A9 用「UI 层批量写、不动引擎」：看起来改动最小，但「各开关原值保留」做不到——批量写会覆盖子开关，恢复必须再藏一份影子偏好，影子状态本身仍是一个新偏好字段，却比真门多一层语义；而且 Android、iOS 的原生设置页不经共享 React 页，批量写的糖只在 Tauri 页上存在。真门是一个 `bool` 加五处 `&&`，语义与 Windows 完全对齐，更省。
- A9 归 C 只记笔记：分键确实能表达同一个终态，但关掉要依次点五六个开关、重开要靠记忆恢复，Windows 迁移用户预期一个总开关；门本身代价小，不值得为此牺牲交互。
- A6 只在 `normalize_online_slots` 里做首页预算：云候选在场时英文组确实在那里归拢，但无在线候选时英文组由 `insert_mixed_rows` 直接落位，runtime 不动它，功能在无云时失效；两层各管一段是必要分工，不是重复。
- 固定位置英文计入首页预算（Windows 的 `fixed_on_first_page` 会从可动预算里扣）：没接。`apply_candidate_positions` 在混排之后才读 journal 定置顶位置，要精确扣预算得在 `insert_mixed_rows` 之前复制一份置顶查询；置顶英文本身是用户显式钉的，扣它会让「钉一个英文反而挤掉混输英文」变得反直觉。此差异记入笔记，需要时再补。

## Consequences

- `mixed_input.enabled` 与 `mixed_input.first_page_count` 都是后加成员且带 serde 缺省，旧文档、只写前四项的宿主、旧版本引擎都能继续读同一份文档；开着/为 1 时文档逐字节不变。
- `Session::new` 与 `NineKeySession` 的选项只在建会话时应用，改偏好走既有的会话重建路径，与 emoji 混输开关一致；runtime 侧的首页英文数是宿主状态，即时生效。
- 列表比页短时「接在末尾」的英文仍在首页上——放置只控制次序，不控制可见行数，Windows 把多余英文移到 `items.end()` 后同样如此。
- 验证：引擎单测 `first_page_count_limits_english_rows_on_the_first_page`（1/2/3 三种数量与首页放不下的裁剪各一条断言）、`the_mixed_candidates_master_switch_gates_every_member`（总开关关掉时全分成员开也不混入）；先失败证据：把 extra 上限恒为 0 时 `first_page_count=2` 断言从期望 `["何咯","hello","hellos","和","helmet"]` 收到 `["何咯","hello","和","hellos","helmet"]`。`minimum_prefix` 桌面固定值由既有测试 `english_minimum_prefix_is_fixed_where_the_host_does_not_read_it` 钉住。
- 宿主 FFI 的合成词库回归覆盖云候选实际到达后的三个英文优先位；固定位置英文不计入预算的差异如上节所记。


## Integration verification

与 `develop` 的快捷短语调频合并时，候选入口保留 `insert_quick_phrase_candidates` 和已学习槽位，只在外层增加混输总开关条件；行内日期时间继续读取独立的 `date_time_inline`，不退回旧的 T 模式开关。

共享设置页新加入的快捷短语混输与调频同样受总开关禁用，原有值保留。实际控件回归先因总开关关闭后两项仍可操作而失败，补齐门控后 15 项混输测试全部通过；禁用点击使用 DOM 标准 `click()`，避免人工事件绕过浏览器禁用行为。前端类型检查和生产构建通过。Android「表达」、iOS「候选栏」与 Windows「候选与联想」增加同一总开关与 1–3 数量控件；总开关只禁用子控件，不覆盖子偏好，英文数量还受英文分开关禁用。原生层继续走各平台已有的共享偏好保存接口，不复制校验或引擎逻辑。Swift 语法解析通过；Android 宿主检查通过 257 个 JVM 冒烟、220 个设备测试源码编译及 JNI/manifest 契约。Windows WinUI 仍需 CI 编译。

九键沿用首条英文的优先位置，其余最多两条只占首页余位；带合成中英文词库的数量、裁剪与上屏回归先失败再通过。大写字面输入也读取混输总开关，关闭时保留原文字面候选且不追加英文词库补全。

在线归组为云、本地优先段、日文及 AI 预留原有座位，再安排额外英文；英文领先时同样先扣除后续这些组的预算。第一页放不下的额外英文回到本地候选之后，原有第一行英文的优先规则保持。宿主初次创建运行时与重载偏好都传入首页英文数；缺少初始化传参的真实 FFI 回归已复现为云候选到达后只保留一个英文优先位。在线预算所在的运行时全套 363 项测试通过，含英文领先与非领先、AI 预留位和小页数裁剪；引擎 61 项混输回归与初次会话的 FFI 回归均通过。四项修复的只读复审未发现新的阻塞问题。最终 Clippy 通过。

集成本地完整 quick 首轮仅因语音语言目录的冷编译超时失败；同一标准产物目录预编译完成后，完整 warm quick 重跑通过（退出码 0），没有仅拼接分阶段结果。原生 macOS 编译、Linux 容器宿主和共享设置、Android Java、跨目标编译及 Apple 桥均经过该轮门禁。各阶段因平台工具缺失而跳过的项仍不计设备验收；Windows WinUI 与实际输入法中的多语言候选流程继续等待对应 CI 和产品验收。

提交钩子检出九个随上游整合进入的 TypeScript 文件不符合当前 oxfmt；按钩子列出的路径完成两遍机械格式化并重新检查通过，不改变行为，不关闭钩子。

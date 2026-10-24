# Agent Note: 连续多语言候选在共享运行时逐段输出

Status: implemented

## Problem

#8921 的[会话候选](2026-10-24-multilingual-session-candidates.md)已保留语言分段，但宿主只知道整行字符串。传统的整行 OpenCC 会把日文汉字也当中文转换；候选重排后按旧下标取分段还会错配词面。完整产品仍归[统一输入提案](../../proposed/feature/2026-10-24-unified-language-composition.md)。

## Decision

`Preferences::multilingual_input` 使用独立的 `MultilingualInputPreferences`，语言枚举归 client-core，默认关闭、预选中英日。整项为默认值时不写入文档，旧配置缺项仍读为关闭；关闭时保留所选语言，开启后按配置顺序去重，不足两种时返回空集合。未知语言和错误字段类型由反序列化拒绝。现有 `MixedInputPreferences` 继续保持 `Copy`，不引入对 Engine 的依赖。

`EngineOptions` 显式传递语言集合，现有 Host API 构造继续空集合关闭。`EngineSnapshot::multilingual_conversion` 转述实际来源 18 行的原始编码及 `Conversion`；不扩展随座位排列的平行数组。

Runtime 只在来源、编码、当前输入、完整词面和连续范围全部匹配时消费分段。输入须为非空小写 ASCII、在既有输入预算内，范围完整覆盖且至少涉及两种语言。候选展示只转换 `Chinese` 段，保留引擎缓存和选择身份；简繁设置变更、有效分段变更均使候选视图失效。

选择在刷新前保留转换、暂存中文前缀及明确的边缘方向。最终上屏必须以已保存的前缀和原始选中文字开头，才逐段转换：前缀按中文处理，混合行仅转中文，标点或完成操作的尾部保留。转换在 AI 上文和宿主字符宽度处理之前完成。来源 18 的成功提交禁止宿主整行简繁，即使元数据失效也保留原文，避免误改日文。下一键、失焦和每次新选择都清理本次状态。

以词定字复用 `CandidateEdge::character`，由引擎自身的首末汉字规则判定。运行时按明确方向找到所属语言段，同一个汉字分别出现在中文和日文段时也不能只凭字符猜语言。来源 18 不参与整行中文释义。

`Command::CommitCandidate` 保留引擎选择原始首行的语义，运行时经引擎顺序映射逆查其呈现座位，并在提交词面确实匹配后记录选择。`SelectionHow::FirstCandidateCommand = 12` 只追加枚举值，同步脚本 ABI 的 `first_candidate_command` 表和契约穷举，不复用高亮或点选的含义。

## Existing note audit

- [统一输入提案](../../proposed/feature/2026-10-24-unified-language-composition.md)：部分重叠，产品偏好、资源门控、四套设置与六宿主仍待接入。
- [会话候选](2026-10-24-multilingual-session-candidates.md)：部分重叠，沿用默认关闭、唯一最佳路径和来源 18，本篇补充 Host 元数据与共享展示输出。
- [日文整段混输](../feature/2026-10-12-mixed-japanese-candidates.md)：部分重叠，旧来源 13 的整段语义不变，新路径不复用它。

## Alternatives considered

- 把语言列表并入 `MixedInputPreferences` 能让所有混输配置集中，但它是多个宿主直接复制的 `Copy` 值；增加 `Vec` 会迫使无关消费点改契约，因此新增独立的共享偏好项。
- 各宿主各自调用 OpenCC 并拼片段，能贴近现有候选展示代码，但六套实现会重复身份校验、边缘语言判定和尾部保护，因此集中在已有依赖 client-core 的 Runtime，不给纯引擎引入 client-base。
- 全句禁用简繁最容易保护日文，但中文片段和暂存中文前缀会漏转，因此只对可证明的中文片段转换，元数据失效才保留原样。
- 按字符定位边缘语言可以少传方向，但中文和日文可包含相同汉字，因此保留明确的首末方向，复用引擎的汉字规则。

## Verification

共享偏好先以缺字段和缺类型的编译错误确认测试覆盖新契约，补齐类型后默认关闭与语言去重专项通过。随后真实 `PreferencesStore` 保存、重载、关闭时保留子集、非法 JSON 字段和未知语言回归通过；独立审查补充的非默认 section 未知字段保留回归也通过，client-core 全部 538 项单测通过。损坏语言列表仍按既有恢复规则处理：正常读取和自动恢复不改写未知语言，用户显式恢复时逐字段救援可保留 `enabled`，并将损坏的列表恢复为默认三语。

Host 语言映射先得到 30 通过、2 失败，补映射后元数据仍缺失，得到 31 通过、1 失败。Runtime 六项测试先得到 1 通过、5 失败；逐段修复后全通过。新增边缘选字和释义回归得到 6 通过、3 失败，修复后全通过。独立复审再发现命令提交和候选 epoch 两处遗漏，真实回归得到 9 通过、2 失败；修复后引擎及 Host 32 项、Runtime 11 项专项全部通过，37 份源文件与笔记散列一致。

真实合成词库的 Host Session 与 Runtime 覆盖点选、结束组字、首末汉字、原始首行提交命令及重排后的命令；重排路径明确让展示首行不等于引擎首行，断言实际选择事实的座位、来源及路径。元数据回归只改变语言分段、不改变词面及编码，验证候选 epoch 递增。独立复审关闭两项阻断。

同源 Windows Runtime 全套 353 项通过。正规 `target/android-cargo` 的两个测试程序在指定 API35 `emulator-5590` 执行，引擎与 Host 专项 32 项、Runtime 专项 11 项全部通过，部署前后程序 SHA-256 及源码散列一致。这是 `adb shell` UID 测试，未安装 APK，也不代表应用沙箱、设置或系统 IME 验收。

Windows 的引擎、Runtime、client-core 和 script-runtime 全部 target 严格 clippy、工作区格式、Host API ABI 枚举契约、Host API 构建及正规 CMake 配置通过；既有链接器 stdout 诊断保留。首轮完整 quick 唯一失败是早期 proposed 笔记缺少必需章节，本篇转为 implemented 并通过 1819 篇笔记及 4 份归档封印检查后，使用原门禁重跑，不改测试基线或脚本。笔记转为 implemented 后，同一 `scripts/verify-local.sh --quick` 在 Windows runner 重跑退出 0；既有 131 项通过、2 项按宿主差异排除、116 项交 Windows 原生构建，Linux/Docker、Apple、Harmony 等缺失环境按脚本跳过。

## Consequences

共享偏好可以持久化，但 Host API 尚未注入、没有资源门控、设置入口及六宿主消费。HostAPI 现有构造继续传空语言集合，不能把内部逐段回归通过写成 #8921 已完成，也不能声称 APK、系统 IME 或真实编辑器已经验收。首次日文资源加载锁竞争仍需后续解决。

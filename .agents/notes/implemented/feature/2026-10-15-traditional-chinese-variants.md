# Agent Note: 繁体输出的 OpenCC 臺灣变体

Status: implemented

## Problem

繁体输出原来只有一套配置：`crates/client-base/src/chinese_conversion.rs` 固定按 OpenCC `s2t.json` 转换，宿主经 `msime_client_simplified_to_traditional` 调用，设置页是一个「繁体输出」开关。`s2t` 出的是 OpenCC 标准字形（「裏面」「着急」「乾麪」），臺灣用户要的是臺灣正体字形（「裡面」「著急」「乾麵」），往往还要臺灣惯用词（「軟體」「滑鼠」「程式碼」）。维护者要求加上 `s2tw` 与 `s2twp`，设置页把开关改成单选。

## Decision

**偏好**：`Preferences::traditional_chinese_variant`，类型 `chinese_conversion::TraditionalVariant`，序列化为配置名 `"s2t"`、`"s2tw"`、`"s2twp"`，默认 `s2t`，默认时不写进文档（`Preferences` 带 `deny_unknown_fields`，旧版本读不了这个键）。`traditional_chinese_output` 保持布尔开关不变：工具栏、托盘菜单、Ctrl+Shift+F 和账号同步的 `input.character_set` 都只翻转它，变体在开关之外单独存，切回繁体时还是原来的变体。

**转换**：`convert_to_traditional(text, variant)`，`simplified_to_traditional` 是它的 `s2t` 特例。臺灣两种配置照搬 OpenCC 的 `s2tw.json`、`s2twp.json`：规范化之后先按 `STPhrases` 与地区词组的并集做最大正向匹配分段（命中的词组各成一段，连续未命中的字并成一段），每段先做 `s2t`，再在同一段内做第二遍：`s2tw` 查 `TWVariantsPhrases` → `TWVariants`，`s2twp` 在前面加 `TWPhrases`，组内先命中的词典优先（`short_circuit`）。第二遍 never 跨段——这是 OpenCC 的行为，跨段实现会在少数词组边界上与 OpenCC 不同。三份臺灣词典取自已固定的同一个 OpenCC 提交，原样放在 msime-dictionary 的 `sources/opencc/`，由本仓 `resources/opencc.lock.json` 固定并在构建期获取（见[跨仓决策](../architecture/2026-10-16-opencc-dictionary-repository.md)），第一次用到臺灣变体时才解析。整次转换只申请输出和逐段暂存两块缓冲，与分段数无关。

**C 接口**：`msime_client_convert_to_traditional(text, length, variant)`，`variant` 0/1/2 对应三个配置名，未知编号返回 NULL（调用方保留原文）。旧的 `msime_client_simplified_to_traditional` 保留，等于 variant 0。宿主读偏好时把配置名换成编号存进会话状态，`platforms/common/ChineseTextConversion.h` 的 `traditional_variant_code` 是这条换算的唯一 C++ 实现，认不出的名字按 `s2t`。

**宿主能力**：`HostCapabilities::traditional_chinese_variants`，目前只有 Linux（IBus 的 `ClientEngine.cpp` 与 Fcitx5 的 `FcitxEngine.cpp`）为真。设置页「中文输出」（`packages/ui/src/settings/traditional-chinese-output-section.tsx`）在能力为真时列简体、繁体、台湾正体、台湾用词四项，否则只列简体、繁体；能力为假的宿主上存着的臺灣变体显示成「繁体」，选择时也不改写它——那个宿主本来就按 `s2t` 转换。其它五个平台接上变体时，把能力打开即可，设置页不用再改。

**共享偏好转换**：`packages/ui/src/settings/chinese-output-preferences.ts` 持有三个偏好类型、`chineseOutputForm` 和 `chineseOutputPatch`，总偏好类型直接依赖这个无 React 模块。组件只负责选项文案与输入方案提示，并重导出原来的公开类型。缺省变体仍是 `s2t`；选择简体不写变体，选择当前变体只写开关，不支持变体的宿主显示标准繁体并保留已存值。鸿蒙手机的二元「中文字符集」控件保持其原有布局和布尔补丁。

**账号同步**：单独的键 `input.traditional_variant`，取值同配置名；`input.character_set` 的取值不变，旧客户端照常读。服务端字段表收录这个键之前，上传前的过滤会去掉它，与 `input.quote_style` 的处理相同。

## Alternatives considered

- **把 `traditional_chinese_output` 改成枚举（`simplified` / `s2t` / `s2tw` / `s2twp`）** — 一个键表达一个选择，最贴近单选界面。但六个宿主的快捷切换、托盘菜单、Windows Server 自己解析的偏好 JSON、`corner_when_traditional` 的解析和账号同步都按布尔读它，换类型要同时改全部读者，旧版本读到新文档还会整份解析失败。另起一个默认不落盘的键，旧读者完全不受影响。
- **进程内全局变量记住当前变体，`msime_client_simplified_to_traditional` 隐式读它** — 宿主一行不改就六个平台全生效。但转换结果取决于这个进程有没有、什么时候加载过偏好（Windows Server 自己解析偏好 JSON，不经过 host-api 的偏好加载），架构文档也明确不靠全局变量传设置。显式参数让每个调用点看得见用的是哪套配置。
- **偏好模型继续从 React 组件导入类型** — 不新增模块，所有单选逻辑也能在组件测试中覆盖。但总偏好模型因此依赖展示层，后续复用还要再次从 JSX 中取出转换规则；纯模块集中已有规则，组件保留兼容导出，调用方无需迁移公开 API。
- **六个平台一次接完** — 用户可见的行为一次到位。macOS、iOS、HarmonyOS 在这次的开发机上无法编译验证，维护者选了核心加设置页加 Linux，其余平台另开 PR；能力位保证在此之前设置页不在这些平台上露出不起作用的选项。

## Consequences

- **收益**：Linux 用户可以选臺灣正体或臺灣惯用词；转换与用同一份词典的 OpenCC 逐字一致；新增配置只需在 `TraditionalVariant` 加一项、补词典和编号。
- **代价**：多带三份词典（约 21 KB）；臺灣变体比 `s2t` 多一次分段和一遍扫描。Windows、macOS、iOS、Android、HarmonyOS 在接上之前一律按 `s2t` 转换；它们各自的原生设置面（Android 的 `InputFeatureToggle`、鸿蒙手机的「中文字符集」、Windows 托盘）仍是开关。
- **重访信号**：OpenCC 固定提交升级时，臺灣词典要和 `ST*` 词典一起换；OpenCC 若改了 `s2tw.json`/`s2twp.json` 的分段或词典组，`convert_to_traditional` 要跟着改。

## Verification

`apps/desktop/tests/settings/chinese-output-preferences.test.ts` 覆盖缺省值、宿主能力降级、最小补丁及切回繁体保留变体；组件回归验证选项顺序、文案与回调，真实设置页回归验证输出标签与保存链路。纯模块与组件旧路径的类型兼容另以 TypeScript 探针验证。

`crates/client-base/src/chinese_conversion.rs` 的 `taiwan_variants_follow_opencc_configurations` 钉住三种配置的例子；开发时另用系统 OpenCC 1.3.2 加载由本目录词典编译的 `.ocd2` 和固定提交的三份配置，对约 2300 行语料逐行比对，三种配置都是 0 差异。分配预算在 `crates/host-api/tests/chinese_conversion_alloc.rs`，C 接口在 `crates/host-api/src/tests.rs` 的 `traditional_variant_conversion_boundary_selects_the_configuration`，Linux 两个宿主在 `platforms/linux/tests/input/traditional_output.cpp` 与 `platforms/linux/fcitx5/tests/native.cpp`（`--rows`）。

相关：转换扫描的分配约束见[繁简转换未变化的扫描借用原文](../simplification/2026-10-14-chinese-conversion-borrow-unchanged.md)和[繁简词典用实际首字节排除不可能的前缀](../simplification/2026-10-14-chinese-prefix-byte-filter.md)。

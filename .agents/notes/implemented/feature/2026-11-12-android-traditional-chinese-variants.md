# Agent Note: Android 繁体输出宿主边界与组字安全

Status: implemented

## Problem

[共享 OpenCC 变体](2026-10-15-traditional-chinese-variants.md)和 [macOS 宿主接入](2026-11-03-macos-traditional-chinese-variants.md)已经提供 `s2t`、`s2tw`、`s2twp`。Android 的四选设置、能力与 JNI 转换已由 #11421 落地，见[Android 臺灣变体接入](2026-11-14-android-traditional-chinese-variants.md)。剩余缺口在宿主前缀、注解和组字中输出切换：候选重画不等于编辑器文本同步；编辑器自己改写但保留标记时，结束路径还可能覆盖那份文字。

## Decision

Android 沿用 #11421 的字符串会话状态：`traditional_chinese_variant` 在完整偏好每次应用时读取 `s2t`、`s2tw` 或 `s2twp`，只在 `AndroidChineseTextConversion` 的 JNI 调用边界经 `ChineseOutputPolicy.traditionalVariantCode` 映射为 0/1/2。缺键、非字符串和未知值按 `s2t` 转换，不保留本分支原来的平行整数编码。转换失败保留原文，来源 18 与日语方案沿用原边界；输出变化判断使用字符串内容相等，菜单、工具栏和快捷键只翻转布尔开关。

宿主组装编辑器组字和候选条标题之前，先把 `phrase_prefix` 送进与正常上屏相同的 `chineseOutput(text, context)`，再交给无状态的 `KoreanInputPolicy` / `PhrasePreeditPolicy` 拼接。只转换已选前缀，不转换拼写或读音；`script_conversion`、方案与本地模式的边界仍由原有转换入口决定。候选标题的光标位置和编辑器点击换算均使用转换后前缀的长度，避免词级变换改变长度后定位错位。

工具栏乐观切换、完整偏好快照应用、重读与保存失败回滚都经 `setChineseOutputChoice` 更新全局简繁、变体和粤语简体开关；实际变化时按当前宿主组装路径重算组字，`ChineseOutputCompositionSync` 尝试读编辑器带样式的快照。只有能确认非空组字标记、内容仍等于桥最后写入的组字且光标折叠在组字末尾时，才经 `EditorBridge.refreshComposition` / `apply` 重写；没有标记或用户已经移走选区时跳过即时写入，不能冒险在新光标处插字。完整快照在新 `view` 就位后才同步，粤语开关变化也进入重画判断。

`EditorBridge` 记录最近一次成功 `compose` 的文本，提交、清空、丢弃和结束时清除记录。`finishCompositionInPlace` 经 `ChineseOutputCompositionSync.readMarked` 读带样式的完整快照，按 `startOffset` 换算回报的组字区，并确认当前 composing spans 的范围一致，再把标记文字交给 `finishInPlace`。读不到、范围不匹配或标记文字不等于桥最后写入时，一律 `abandon`，只结束、不替换、不移动选区。匹配时，只有 drawn / settled 与最后写入都相同才直接结束；否则 `settle` 替换成新确定文本并按原有 `resettled` 算法恢复选区。无标记或空区间不能在新光标处插字。可核对的日语 `n` → `ん` 保持转换，读不回的日语标记同样不盲写；笔画丢弃和纯拼接 policy 不改。

候选条和展开候选的普通引擎 `annotation` 在进入 `CandidateGlossPolicy` 之前走 `chineseOutput`，读屏 suffix 同样处理。注解不是候选正文，不套用正文来源 18 的跳过规则；中文提示即使候选正文已经逐段转换也应转换。五笔剩余编码仍由原有编码策略生成，拉丁编码不变；韩文汉字训音保留独立的原文分支，日语与专用英文沿用原有方案边界。释义、音标以及插件 `script_annotation` 不并入这次转换，遵循 macOS 的插件注释原样显示约定，不增加 Android 插件展示路径。

`PreferencesStore::decode_document` 在严格解码之前调用独立的 `traditional_variant_read` 模块：仅 `target_os = "android"` 的读取删除这个字段的未知或非字符串值，让默认 `s2t` 生效。只在 Java 里做回退不够，因为原始文档会先被共享加载器拒绝，热更新根本到不了 Java，会粘住旧变体。其他平台读法、提交校验、其他字段的错误都不变；普通加载不写文件，下次正常保存会省略被回退的无效字段（与已有五笔版本读取规范化同类），不保留未来版本的未知变体名称。

Android 的 `CloudSync.applySettings` 经 `NativeClient.accountSettingsApply` 使用 `client-services::apply_android_settings` 合并账号文档：旧云快照没有 `input.traditional_variant` 时，`Applier::set_string` 不触碰本机共享偏好里的变体。新快照的合法值照常应用；这不同于 **完整共享文档** 缺键时宿主缓存必须回退 `s2t`。两种缺键不可混淆，不新增 `platform.android.traditional_chinese_variant`。

能力、原生 `TypingPage` 四选设置、JNI 和构建契约均沿用 develop，不再次实现已落地功能。移除重复且不再被设置页使用的 `ChineseOutputChoice` 及其专属测试；保留设置搜索两种臺灣标签、真实 JNI 字形和账号旧快照测试。不改变任何平台的出厂默认值，也不改 Windows、iOS 或 HarmonyOS 产品行为。

## Verification

此前未整合 #11421 的分支上，`ChineseOutputPolicySmoke` 验编号、输出和来源 18 边界；当时的 `ChineseOutputChoiceSmoke` 验四个标签与最小写入规则；`settings_sync` 的 Android 账号快照用例验旧快照保持两种臺灣变体。Linux 上通过 38 个宿主能力相关 Rust 测试、48 个设置同步测试、8 个繁体相关 Host API 测试。真实共享库的 C ABI 冒烟得到 `s2t`「喫軟件裏面」、`s2tw`「吃軟件裡面」、`s2twp`「吃軟體裡面」，并调用 Android 账号设置应用接口验证旧快照只关闭简繁开关，分别保留 `s2tw` 与 `s2twp`。这些是历史结果，不代替 rebase 后验证。

`ChineseOutputPolicySmoke` 的正向测试用注入转换器核对整词、普通候选和注解的原文与变体编号完整传递；`ChineseOutputPresentationSmoke` 用明确的测试转换器覆盖三变体顺序切换、恢复简体、前缀显示、`EditorBridge.abandon` / `settle` 两条 cursor-move 结束方式、提示与读屏一致性以及非中文边界。这些 JVM 用例验证组装和路由，不把测试转换器当真实 OpenCC，也不声称实际调用了 Android 服务或设备热重载；真实字形仍由共享 Host API 测试与设备验收证明。

`ChineseOutputCompositionSmoke` 直接使用服务调用的 `EditorBridge.finishInPlace` 与 `refreshComposition`：先让带前后文的编辑器持有旧组字，再令新设置的 drawn == settled，未再输入就移出光标，断言最终文档替换成新字形。覆盖简转繁、变体切换、还原简体、粤语输出、未转换的拼写后缀、长度变化与反向选区，以及即时同步后结束、相同设置不重复写入、选区已移动时延迟至结束修正、丢失或空标记不写入、日语与笔画边界。这仍是 JVM 行为测试，不实例化完整 `InputMethodService`，不宣称本轮跑过模拟器或设备热重载。2026-11-14 完整 `check-host.sh` 通过 351 个 JVM 冒烟和 281 个设备源文件编译；把结束判断还原为只比较新 drawn / settled 的变异测试在首条旧字形用例上按预期失败，证明新测试能抓住本轮缺口，而非只验证 `apply` 写入。

`traditional_variant_falls_back_on_read_only_on_android` 在桌面测试环境显式开启 Android 分支，覆盖三种合法值、缺键、未知和非字符串值，关闭分支仍严格拒绝未知值；其他字段损坏仍拒绝整份文档。`traditional_variant_store_reload_does_not_keep_the_previous_document_variant` 经真实 store 和 `load_reusing` 验证换文档不沿用旧变体、读取不写盘；非 Android 上未知值仍失败。Android target 的 `cargo check -p msime-client-core --target aarch64-linux-android` 通过。临时 Rust 运行探针直接调用同一规范化模块并解码真实 `PreferencesSnapshot`，六种输入得到 Standard / Standard / Taiwan / TaiwanPhrases / Standard / Standard，关闭 Android 分支仍拒绝 `s2hk`；这不替代设备运行。

Android `check-host.sh` 通过 350 个 JVM 冒烟、281 个设备套件源文件编译、NDK aarch64 JNI 编译与桌面 JVM 的 JNI 回调检查。Gradle `:app:compileFullDebugJavaWithJavac` 也通过，包含原生 `TypingPage`；这些不等于 APK 安装或设备验收。API 35 的既有设备验收在 `bbe564eb494c2570fa083b3ed1f3a54caea7aaaf` 上实际观察四选设置、三种变体的候选和常规上屏，十张合成输入截图上传 sugi.moe 后均下载回验，PNG 字节和 SHA-256 与本地一致；没有改前截图。PR 正文保留该轮两次首次失败及重跑记录，不把旧设备结果当这次前缀与注解修复的设备验收。本次没有重装 APK、没有新增前缀或注解设备截图，没有执行设备 `NativeSmoke` 或真实组字中偏好热重载。笔记校验通过；本次 quick 门禁仅 Linux 桌面壳阶段因缺少 `gobject-2.0.pc` 等 GTK 开发依赖失败，其余实际执行阶段通过，不能宣称全绿。

活跃笔记检索结果：共享 OpenCC 变体与 macOS 接入两篇部分重叠，保留并互链、同步 Android 接入状态；其他命中涉及字母长按或 Emoji 变体，与本次中文输出无关。

2026-11-14 整合时，#11421 的接入笔记与本篇部分重叠：前者保留基础功能理由，本篇只记录剩余宿主边界和安全约束，双方互链。`ChineseOutputPolicySmoke` 与 `ChineseOutputPresentationSmoke` 按 develop 的字符串会话与单参数转换器重新表达；前缀、注解、可访问性和组字结束的独有断言保留。新增编辑器先把 `海滩` 改成 `别字`、即时刷新拒写、再移出光标的回归：修复前退出 1，失败在被换成 `海灘`；修复后要求文档仍为 `前别字后文`，写入次数和反向选区不变。未再输入就移出光标、合法旧字形应变为新字形的原有回归仍保留。读不回、无样式、过期范围和丢失标记另有行为断言。本轮不新增设备验收，完整日志和退出码记录在交付证据中。

## Alternatives considered

- **保留本分支的整数会话编码和设置实现** — 能少改既有测试；但 #11421 已使用字符串会话和四选设置，平行实现会重复功能并制造 JNI / 设置类型漂移。复用 develop，只适配独有宿主修复。
- **把账号快照缺键当完整偏好缺键** — 可以少写一条读取路径；但旧云账号没有变体字段，应用它会清除用户本机已经选择的臺灣变体。
- **在拼接 policy 里保存变体或调用 JNI** — 所有调用者都能自动转换前缀；但会让纯拼接依赖宿主状态，混淆拼音与汉字边界，而且 cursor-move 与普通上屏容易分别维护不同规则。转换留在宿主组装点，现有 policy API 和拼接断言保持不变。
- **只在简繁变化时无条件 `bridge.apply`** — 能立即让编辑器和候选栏一致；但编辑器可能已经移除组字标记或用户先移动了选区，写入会覆盖新选区。即时同步和结束替换都采用读回校验，不以重画代替编辑器同步，也不以新设置下 drawn == settled 推断旧文字已经正确。
- **结束时只信有效组字区和缓存的旧写入** — 对不支持样式读回的编辑器也能补新字形；但标记并不意味着其内容仍属于输入法，编辑器改成 `别字` 后会被覆盖。因此读不回时只结束，接受该编辑器保留旧字形的限制。

## Consequences

- **收益**：基于已落地的臺灣变体，前缀、候选提示和读屏与普通上屏一致；输出切换后的合法旧组字会同步或在结束时补正，编辑器自行改写的文字不会被覆盖。
- **代价**：结束替换多一次带样式读回；无法核对的编辑器可能保留旧字形或日语末尾 `n`，以不覆盖用户文字为优先。共享偏好未知值回退仅限 Android，其他平台保持严格读取与原有行为。

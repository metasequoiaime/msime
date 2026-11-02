# Agent Note: iOS 独立选择粤语双拼

Status: implemented

## Problem

#9962 希望在 iOS 的「添加方案」里选择并测试粤语双拼。[粤语双拼键位规则](../../implemented/feature/2026-10-26-cantonese-shuangpin.md)已经进入引擎，但目前只有 `cantonese` 一个触屏方案入口。引擎靠首个两键码自动判断：若两键同时是合法粤拼音节，仍按普通粤拼解释。只给 iOS 列表加名称，用户选中后也无法稳定测试这种同形码。

## Decision

- 给共享触屏方案枚举追加 `cantonese_shuangpin`，保留现有 `cantonese` 的标识、顺序和默认启用状态；新入口仅在 iOS 的「添加方案」中展示，仍用同一份粤拼词典和 26 键键面。
- 方案选择经共享偏好的 `touch_keyboard_schemes.selected` 传给宿主，再传到引擎会话的粤拼模式。选中双拼时从第一键起按两键码解释，包括与合法粤拼同形的码；选中普通粤拼时保留现有自动识别行为，避免升级后改变老用户输入。
- 会话重建、切换到别的方案再切回、光标编辑和退格都保留所选模式。候选查询仍只读现有 `msime-cantonese.db`，不复制词表。
- Android、HarmonyOS 等宿主若读到新标识应能保留偏好且不崩溃，但这次不在它们的方案列表增加新入口。

## 分号键

- 引擎的粤语双拼把 ing、ik 两个韵母只放在 `;` 上（`crates/engine/src/cantonese/shuangpin.rs`），会话在双拼组字的奇数位置把 `;` 当第二键（`session/input.rs` 的 `cantonese_final`），空闲时 `;` 仍是标点。入口刚上线时 iOS 只在微软双拼下显示第二排 L 右边的 `;` 键，粤语双拼在触屏上打不出 sing、ging、ming、sik 这类音节（#10099）。
- 现在 `ChineseInputScheme.showsSemicolonFinalKey` 对微软双拼和粤语双拼为真：选了粤语双拼，26 键中文字母层就一直显示这个 `;` 键，位置、宽度和第二排不缩进的规则与微软双拼相同；英文、本地模式下照旧隐藏。没有设置开关。
- 读屏标签按方案给出（`semicolonFinalKeyAccessibilityLabel`）：「分号，微软双拼 ing」「分号，粤语双拼 ing 和 ik」，不再被字母键的「字母 ;」覆盖，也不重复读键位提示。
- 粤语双拼不在 `typesShuangpinLetters` 里，不走 #10018 的触点纠错；`;` 键进了键区布局，但这份几何只在 26 键双拼下注册给引擎。
- 其他方案不加可选的分号键：全拼、小鹤、自然码等方案的 `;` 不是编码键，多一个键只会打出标点，还会把第二排的键挤窄。普通粤拼不同，引擎在一个声母后也把 `;` 当双拼韵母（`CantoneseScheme::accepts_semicolon`，`s;` 出 sing），但普通粤拼用户把 ing、ik 完整拼出来，用不上这个键，所以同样不显示。

## Alternatives considered

- **只在 iOS 增加列表项并复用 `cantonese` 标识**：改动少，但共享文档无法区分两个入口，重建会话后丢失选择，同形码也仍走普通粤拼。
- **粤语双拼的分号键做成默认关闭的开关**（#10099 报告者的原始建议）：最强的理由是不改变已经习惯九键第二排的用户的手感，想要的人自己打开。不采用，因为 ing、ik 只在 `;` 上，开关关着时这个方案缺两个韵母，默认状态下就是不完整的方案；微软双拼的 `;` 也是选了方案就显示，没有开关。
- **给所有方案加一个「显示分号键」的通用开关**：能满足想在全拼下直接打分号的用户。不采用，全拼等方案的 `;` 不参与编码，普通粤拼完整拼写也用不到它，键盘多一个基本只出标点的键，第二排每个键都变窄，而符号层已经有分号。
- **新增全局引擎方案编号**：每个宿主都能把双拼当独立方案，但需要改引擎方案集合、FFI 和所有平台的方案映射；这里两种输入共用粤拼词典及候选语义，独立的触屏入口加模式更贴合现有结构。

## Consequences

- 入口只增加 iOS 的触屏方案；共享偏好与引擎字段可被其他宿主读取但不会自动显示新入口。桌面宿主不读触屏方案，另有粤拼方案下的选项，见 [桌面粤拼方案下的粤语双拼选项](2026-11-02-desktop-cantonese-shuangpin-option.md)。主应用的粤拼语言分组同时列出普通粤拼与粤语双拼；语言词典下载完成后同时提供两个入口，键盘里的「添加语言」跳转到此处启用。

## Verification

- #10099 分号键：`LanguageSchemesTests.testOnlyMicrosoftAndCantoneseShuangpinShowTheSemicolonFinalKey` 核对只有微软双拼和粤语双拼显示 `;` 以及两种读屏标签；`testCantoneseShuangpinKeyboardShowsTheSemicolonKeyRightOfL` 在暂存了 `msime-cantonese.db` 的测试宿主里核对键在 L 右边、与微软双拼横向几何一致、M 加 `;` 进入组字、出 ming 或 mik 的候选、普通粤拼不显示，词库缺席时跳过。未在真机或模拟器的系统键盘里看过实际画面。

- 入口审查回归使用真实 `InputLanguage.swift` 和 `InputSchemePreference.swift` 编译 Swift harness，仅替代应用容器与外部资源服务。修复前准确报出添加入口、语言归属和下载词典三个失败，修复后 7 项检查通过；旧特性测试排除集合同步纳入粤语双拼，避免把它误当作不需要词典的方案。

- `cargo test -p msime-engine --locked --lib cantonese::`：39 项通过；显式模式同形码和会话切换回归通过。
- `cargo test -p msime-client-core --locked --lib cantonese_zhuyin_and_vietnamese_touch_schemes_are_appended_and_opt_in`：通过。
- `cargo check -p msime-host-api --locked --all-targets`、`cargo fmt --all -- --check`、`CI=true pnpm run verify-notes`：通过。
- `xcodegen generate`、`bash platforms/ios/build-native.sh simulator` 和 `xcodebuild ... -scheme MSIMEKeyboardTestHost`：通过；`MSIMEClientTests` 的 `LanguageSchemesTests` 在 Xcode 27 测试会话保存阶段卡住，未宣称 Swift 测试套件通过。
- 已在副盘 iOS 模拟器工作树中暂存锁定的粤拼词典；没有完成真实键盘扩展交互验收，需后续在模拟器测试会话稳定后补跑。
- 入口补齐后实际运行 `scripts/verify-local.sh --quick`：真实 Swift 入口回归 7 项、本机 Windows 策略 173 项、Harmony 设置打包、工作流/笔记、Rust workspace、Android 目标、wasm 检查与 clippy、Android 宿主检查通过。公共 `windows package managers` 契约仍因 workflow runner 配置失败；系统盘仅余 20 GiB，在创建 Linux 构建树前退出 75，完整 quick 未完成，不能记为全绿。

## Risks

- 新触屏方案标识会被旧版本偏好解析器拒绝；与现有新增方案一样，必须确认共享文档的升级和回退路径。
- 仓库主卷仅余约 33 GiB，因此构建和验证使用副盘工作树。

系统扩展部分上屏的文档写入与独立编辑器回归见 [iOS 部分上屏直接替换标记区](../../implemented/bug-fix/2026-10-31-ios-cantonese-partial-commit.md)。

# Agent Note: iOS 独立选择粤语双拼

Status: implemented

## Problem

#9962 希望在 iOS 的「添加方案」里选择并测试粤语双拼。[粤语双拼键位规则](../../implemented/feature/2026-10-26-cantonese-shuangpin.md)已经进入引擎，但目前只有 `cantonese` 一个触屏方案入口。引擎靠首个两键码自动判断：若两键同时是合法粤拼音节，仍按普通粤拼解释。只给 iOS 列表加名称，用户选中后也无法稳定测试这种同形码。

## Decision

- 给共享触屏方案枚举追加 `cantonese_shuangpin`，保留现有 `cantonese` 的标识、顺序和默认启用状态；新入口仅在 iOS 的「添加方案」中展示，仍用同一份粤拼词典和 26 键键面。
- 方案选择经共享偏好的 `touch_keyboard_schemes.selected` 传给宿主，再传到引擎会话的粤拼模式。选中双拼时从第一键起按两键码解释，包括与合法粤拼同形的码；选中普通粤拼时保留现有自动识别行为，避免升级后改变老用户输入。
- 会话重建、切换到别的方案再切回、光标编辑和退格都保留所选模式。候选查询仍只读现有 `msime-cantonese.db`，不复制词表。
- Android、HarmonyOS 等宿主若读到新标识应能保留偏好且不崩溃，但这次不在它们的方案列表增加新入口。

## Alternatives considered

- **只在 iOS 增加列表项并复用 `cantonese` 标识**：改动少，但共享文档无法区分两个入口，重建会话后丢失选择，同形码也仍走普通粤拼。
- **新增全局引擎方案编号**：每个宿主都能把双拼当独立方案，但需要改引擎方案集合、FFI 和所有平台的方案映射；这里两种输入共用粤拼词典及候选语义，独立的触屏入口加模式更贴合现有结构。

## Consequences

- 入口只增加 iOS 的触屏方案；共享偏好与引擎字段可被其他宿主读取但不会自动显示新入口。主应用的粤拼语言分组同时列出普通粤拼与粤语双拼；语言词典下载完成后同时提供两个入口，键盘里的「添加语言」跳转到此处启用。

## Verification

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

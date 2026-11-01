# Agent Note: 三端触屏键盘开放「候选栏大小」，工具栏跟随候选栏一起缩放

Status: implemented

## Problem

#10137 的报告者觉得 Android 键盘顶部空闲工具栏那一行上下空白太多，希望只把这一行调矮，键盘按键不变。

空闲工具栏和组字时的候选栏共用同一行。Android 的「工具栏尺寸」（[独立工具栏尺寸](2026-10-26-android-toolbar-size.md)）选得比候选栏矮时，会在上方补一块不可点的空白，保证组字开始、结束时键区不跳，所以单独调小工具栏并不能让这一行变矮。真正能让整行变矮的是共享偏好 `candidate_scale_percent`（[Android 候选栏整体大小](2026-10-26-android-candidate-bar-size.md)），但 Android 发布包的原生设置页没有它的入口，iOS 和鸿蒙手机键盘也不读它。另外，Android 跟随候选栏时工具栏按钮固定 44 dp，候选栏缩到 80% 以下、行高不足 44 dp 时按钮会伸出这一行被裁掉。

## Decision

三端触屏键盘统一开放「候选栏大小」，写共享偏好 `candidate_scale_percent`，范围 50–200%、步长 5，100% 时从文档里删掉这一项（与共享设置页的 `candidateScalePatch` 相同）。说法三端一致：标题「候选栏大小」，说明「候选字和工具栏一起缩放，按键大小不变」。出厂默认值仍是 100%，没有调过的用户看不到任何变化。

- **Android**：入口在「键盘 → 工具栏 → 键盘工具栏」，紧跟预览和显示方式（`KeyboardOptionsPage`）。跟随候选栏时，这一行矮于基准行时按钮高度取行高，图标、选中底块和品牌标志按行高相对基准行的比例缩小（`ToolbarSizePolicy.followedButtonHeight`、`followedPercent`），按钮宽度不变。基准行是 44 dp 和同一状态下 100% 时的行高中较矮的一个：读音行显示（直接输入、准备中这类模式提示）时这一行只有 42 dp 的候选行，44 dp 的按钮本来就略高于它，按 44 dp 比较会让默认 100% 的图标在提示出现时缩到 95%、消失时跳回来。26 键长空格样式的「符」和品牌键、收起键一起缩放。工具栏预览按 `ToolbarSizePolicy.followedHeightDp` 跟着变化。单独设了「工具栏尺寸」时行为不变，说明改写成跟随与单独设定两种情况。
- **iOS**：入口在「候选栏」页的「候选栏」组，紧跟候选字号（`CandidateOptionsSettingsView`）。键盘把候选和编码字号、候选行、读音行、释义行和顶栏下限 `topRowMinimumHeight` 都乘以这个倍数（`KeyboardViewController.topRowHeight(..., barScale:)`）。工具栏按钮和品牌标志按行高相对 42pt 的比例缩小，行更高时保持设计尺寸（`KeyboardToolbarButton.contentScale`）。候选词块 34pt 的最小高度也乘以这个倍数：它和候选内容贴住面板底边的约束同为 999 优先级，不缩放时矮于 34pt 的候选栏里两者冲突，Auto Layout 可能撑高候选内容、压到按键上。「键盘工具栏」页的预览跟着变化（`KeyboardSkinPreview.stripScale`）。
- **鸿蒙**：设置是共享 React 页。手机和平板的设置桥按设备形态声明 `candidate_window_scale`（`SettingsFormFactorCapabilities.candidateWindowScale`），「候选栏」组在候选字号后面给出触屏版的 `CandidateScaleRow`。键盘把 50vp 工具条按倍数缩放（`KeyboardMetrics.stripHeightVp`），候选栏的字号、读音行、候选通道和注释行一起缩放，工具栏按钮、图标和标志只随它缩小（`KeyboardMetrics.toolbarScale`）。面板总高经 `totalHeightVp` 的 `stripScalePercent` 同步，扩展能力定第一帧高度时也带上它。2in1 不声明这项能力，候选卡片照旧只随字号调整。展开的「更多候选」网格的字号和格高也跟随候选字号和这个倍数，规则与 Android 相同，见 [Android 候选栏整体大小](2026-10-26-android-candidate-bar-size.md)（#10400）。

`HostCapabilities.candidate_window_scale` 在平台层面加上 iOS。鸿蒙在平台层面仍不声明，由设置桥按设备形态打开，原因同上。

## 不变量

- 空闲工具栏行和组字候选栏始终同高，切换时键区位置不变。
- 不提供让工具栏比候选栏矮的模式（维护者的决定，见 Alternatives considered 第一条）。「工具栏尺寸」选得较矮时仍在上方留白。
- 工具栏按钮和图标只随行高缩小，不随它放大：放大时 Android 仍是 44 dp，iOS 是 40pt 底板，鸿蒙是 40vp。

## Alternatives considered

- **允许工具栏单独比候选栏矮，组字时键区下移**：最直接满足「只调矮工具栏」的字面要求，代码改动也最少，只要去掉补空白的逻辑。但每次开始打字键盘都会跳一下，正是 [独立工具栏尺寸](2026-10-26-android-toolbar-size.md) 当初加空白要避免的，维护者否决。
- **把「工具栏尺寸」下限从 90% 调低**：只要改一个常量。但补空白的逻辑还在，调低只会让图标变小，这一行的高度不变。
- **只在 Android 做**：#9940 的先例是只做 Android，范围最小。但键盘 UI 的改动要 iOS、Android、鸿蒙三端一致，维护者也要求三端统一。
- **共享设置页沿用桌面的 75–150%**：不用改滑块常量。但手机上要调矮的人正需要 75% 以下的那一段，Android 的 `CandidateScalePolicy` 本来就接受 50–200，所以触屏用整个核心范围。

## Consequences

收益：三端都能把键盘顶部这一行调矮或调高，空闲和组字同高，键区不跳。工具栏图标在矮行里不再被裁。候选字号和这一行一起缩放，`candidate_scale_percent` 在三端触屏键盘上的含义一致。

代价：候选字跟着变小，想要大字号、矮工具栏的用户做不到，只能在两者之间取舍。这个偏好不随账号同步：Android、鸿蒙、iOS 和桌面的设置同步映射（`settings_sync`、`harmony_settings`、`ios_settings` 和 macOS 的 `cloudSettingsSnapshot`）都不含 `candidate_scale_percent`，`settings_sync_export_is_exactly_the_shared_android_keys` 锁住了 Android 导出的键集，所以手机上调到 60% 不会让同账号的桌面候选窗跟着缩小，桌面上的「整体大小」也不会改变手机的顶栏。只有用户手动导出、在另一台设备导入的设置文件会把它带过去（它不在 `settings_document::LOCAL_SECTIONS` 里），这是用户自己的操作。顶栏调到 50% 时，调整键盘高度用的内联高度条（iOS、鸿蒙的 `HeightBar`）仍按原尺寸画，在矮行里会被裁掉一部分；Android 已换成盖在整块键盘上的「键盘调整」遮罩，不占这一行。Android 候选行右端的展开键仍是 40 dp，50% 时略高于 21 dp 的候选行，箭头本身仍能完整显示。iOS 工具栏上 SF Symbols 画的几个可选按钮（AI 润色、简繁、全半角、标点）没有跟着缩小，18pt 的图标在 21pt 的行里仍放得下。

验证：Android JVM smoke（`ToolbarSizePolicySmoke`、`CandidateScalePolicySmoke`、`SettingsSearchIndexSmoke`）和 Gradle Java 编译；共享设置页的 vitest 与类型检查；鸿蒙 `tests/run.sh`；client-core 的 host_surface 测试。iOS 的 Swift 单测在模拟器上跑，本次没有运行。三端的真机与模拟器手感（各档位下工具栏和候选栏是否同高、图标是否被裁、键区是否跳动）都还要人工验收。

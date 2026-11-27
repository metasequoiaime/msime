# Agent Note: Android 云候选与 AI 候选的来源标记

Status: implemented

## Problem

Android 的普通候选条、竖排候选和展开面板对云候选（Engine 来源 2）和 AI 候选（来源 3）不加任何标记，读屏也不说来源，用户分不出哪一个是联网得来的、哪一个是本机词库的。其他五个宿主都标出来源：Windows、macOS、HarmonyOS 与 Linux 在候选后面接 ` ☁️`、` 🤖`，iOS 画着皮肤色的 SF Symbol 并在读屏里补「云候选」「AI 候选」（`platforms/ios/SharedUI/candidate/CandidateMarker.swift`）。Android 上唯一的 ☁️/🤖 是自定义皮肤横幅里的装饰文字，且不读共享偏好。

共享偏好里已有两个字段：`candidate_cloud_badge`（缺值按开，关掉只隐藏云标记，见[Windows 云候选来源角标开关](2026-11-13-windows-cloud-candidate-badge.md)）和 `candidate_ai_badge_style`（`robot` 或 `whale`，见[macOS 候选显示补齐](2026-10-21-candidate-display-toggles.md)）。此前没有一个宿主两个都读，Android 一个都不读。

## Decision

Android 在候选字（含纠错星号）后面、释义行之前画单色矢量标记，着这一格释义的颜色（`CandidateCorrectionMarkPolicy.annotationColour` 的未纠错分支），跟随皮肤和选中状态：

- 云候选画云；AI 候选按 `candidate_ai_badge_style` 画机器人头或小鲸鱼，未知值按机器人。`candidate_cloud_badge` 关掉时只去掉云图形。
- 读屏在候选描述后补「，云候选」「，AI 候选」，与 iOS 同词；图形开关不影响这一句，开关只管看得见的标记。
- AI 占位行（来源 12）与云占位行（来源 15）不是候选，不加标记，读屏照旧念 `AiPendingCandidatePolicy` 的状态。
- 自定义皮肤横幅只能写文字，仍用 ☁️、🤖，并跟随同一组偏好（🐳、关掉云标记）；皮肤的 `stripBadge` 照旧优先。

规则在 `candidate/CandidateSourceMark.java`（纯 Java，由 `tests/candidate/CandidateSourceMarkSmoke.java` 覆盖），绘制在 `candidate/CandidateSourceMarkSpan.java`：标签里补一个占位字符 `U+FFFC`，挂 `ReplacementSpan`，宽度是 0.12 倍字号的间隔加 0.8 倍字号的图形，行高沿用候选字体的度量。三个图形是本仓自绘的 24×24 `VectorDrawable`（`res/drawable/ic_candidate_source_{cloud,robot,whale}.xml`）。键盘这一侧的类不引用 `R`，按资源名查找，与 `CandidateSymbolLabel` 相同。

设备上由 `StaleCandidateDeviceSmoke -e source-marks true`（`CandidateSourceMarkDeviceChecks`）在真实按钮上核对标记、读屏、两个偏好和行高。同一处也接手 `check-host.sh` 里 #10018 守卫中钉住 `updateCandidateButton`、`expandedCandidateButton` 源码文字的那部分：这两处改了，守卫换成看结果的检查（坐上首选的纠错行有 *、候选字盖标记色、读屏补「，已纠正」），去掉展开面板的「，已纠正」时它会失败；`candidateLabel` 内部的守卫不变。

两个偏好经 `ImeCandidateDisplayPreferences` 实时读取，并记进皮肤提示（`SKIN_HINT_KEYS`），键盘冷启动的第一帧就按用户的选择画。设置应用加两行原生设置：「输入」页「云候选」下面的「显示云候选标记」开关（位置与 Windows 相同），「AI」页「服务」组的「AI 候选标记」选项（机器人 / 小鲸鱼）。说明收进「?」。

## 尺寸与布局

- **行高不变。** `CandidateStripSizing` 按样例文字「Ag汉あ한」量候选栏高度，候选按钮开着回退字体行距。彩色 emoji 走回退字体时上升、下降都比中文字体高，整格会比量出来的候选栏高；`ReplacementSpan` 报告候选字体自己的度量，所以带标记和不带标记的候选一样高，候选栏高度的计算不用改。
- **宽度变宽。** 横向候选条的按钮是 `WRAP_CONTENT`，带标记的那一格多出约 0.9 倍字号；展开面板按自然宽度分列，短的云候选可能占更宽的一列。联系人候选的角标（`ContactCandidateButton`）本来就给按钮加 13 dp，布局已经能处理按格不同的额外宽度。
- **晚到的行。** 云候选回来时替换隐藏的来源 15 席位，整格本来就是新出现的；AI 回答落在第一页最后一格。标记本身不引起已显示候选的移动。

## Alternatives considered

- **照其他四个宿主接 emoji 文字（` ☁️`、` 🤖`）。** 最强的理由是字形与 Windows、macOS、HarmonyOS、Linux 和皮肤横幅完全一致，偏好里的 robot/whale 就是这两个 emoji 的名字，代码最少。不用它：彩色 emoji 不随皮肤着色，在深色或配色强烈的皮肤上显得突兀；各厂商的 emoji 字体不同，同一个标记在不同手机上样子不一；emoji 的回退字体行距会撑高候选格，还得另做度量处理。Android 和 iOS 一样是贴着皮肤画的触屏候选条，跟随皮肤更合适。
- **像联系人候选那样在按钮右上角画小角标。** 最强的理由是与 Android 现有的联系人标记一致，完全不碰行高，额外宽度固定。不用它：其他宿主都把来源标记放在候选字后面；9 dp 的角标里云和 AI 两个形状不好分辨；与联系人共用同一个位置，含义容易混。
- **用 Material Symbols 的 `cloud` 与 `smart_toy`。** 最强的理由是仓库已经随包带着一批 Material Symbols 图标（`res/drawable/ic_ms_*`，Apache 2.0）。不用它：Material Symbols 没有鲸鱼，`whale` 只能另画，一套标记里两种画风；三个图形都自绘，比例和笔画粗细一致。

## 旧笔记审计

- [Windows 云候选来源角标开关](2026-11-13-windows-cloud-candidate-badge.md)：部分重叠。它新增 `candidate_cloud_badge` 并只接入 Windows；本篇让 Android 也读它，同步改了那篇里「Android 尚未读取」的事实。
- [macOS 候选显示补齐词性、单条释义与两类徽标](2026-10-21-candidate-display-toggles.md)：部分重叠。它新增 `candidate_ai_badge_style`，共享设置页的徽标行只在 macOS 显示；本篇在 Android 原生设置里提供同一个字段，不改共享设置页。
- [AI 候选固定在第一页最后一格](2026-10-16-ai-candidate-last-slot.md)：无冲突。本篇沿用它对来源 12 的占位处理。
- [整句候选来源徽章](../../proposed/feature/2026-10-21-sentence-source-badge.md)：无关。那篇只谈整句模型来源，明确不含云和 AI。

## Consequences

- Android 用户默认能看到云候选和 AI 候选的来源；两个偏好的出厂值和缺值读法都不变（`candidate_cloud_badge` 开、`candidate_ai_badge_style` 为 robot），Android 只是开始读它们。
- 在 macOS 选的 AI 标记样式经设置同步到 Android（`candidate_ai_badge_style` 在同步白名单里）；`candidate_cloud_badge` 不同步，各设备自己设。
- 图形是 Android 自己的一份；以后要改样子，只改三个 `VectorDrawable`。联系人角标如果要改成同一套画法，另开改动。
- 读屏多念一句来源，候选描述变长。

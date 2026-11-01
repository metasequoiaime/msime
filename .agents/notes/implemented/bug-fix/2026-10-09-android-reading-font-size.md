# Agent Note: Android 读音字号按「预编辑字号」缩放，读音与候选之间留出间距

Status: implemented

## Problem

#6107：Android 键盘组字时，候选栏上方的拼音（读音行）太小，而且和下面的候选字贴在一起。

字号小是 bug。共享设置页在 Android 上给出了「候选栏 › 字体与大小 › 预编辑字号」（`candidate_preedit_font_size`，12–32，`host_surface.rs` 的 `candidate_preedit_font` 对 Android 打开），宿主也读了这个值（`MSIMEInputService.applyCandidateAppearance` 和偏好快照路径），但读音字号有三处在写：`render()` 里按设置写一次，`ImeStyler.applySkin` 按设置再写一次，`ImeToolbar.styleTopRow` 又写死成 12sp。`render()` 的三个出口都是先 `applySkin` 再 `styleTopRow`，所以 12sp 总是最后生效，设置怎么改读音都是 12sp × min(系统字体缩放, 1.15)。这个 12sp 来自 #3782 键盘重新设计，当时同一个提交里也加了 `ImeStyler` 按设置写字号的那一行，两者从一开始就互相覆盖。

贴得近来自布局：读音行最小 14 dp、上下内边距都是 0，下面紧接着 42 dp 的候选行，中间没有专门的间距。

## Decision

- 读音字号只由 `ImeStyler.applySkin` 设置，值是 `ReadingRowPolicy.textSizeSp(candidatePreeditFontSize)`：设计字号 12sp 乘以「预编辑字号」÷ 共享默认值 15，与 iOS `CandidateFontPreference.preeditScale` 同一种换算。`ImeToolbar.styleTopRow` 删掉写死 12sp 的那一行，`render()` 里提前设一次字号的那一行也删掉；`render()` 的每个出口都先跑 `applySkin`，再由 `styleTopRow` 调 `updateCandidateViewportHeight` 按最终字号量读音行高度。面板切换等只调 `applySkin` 的路径设的也是同一个字号，读音行高度不会和字号对不上。
- 读音行底部加固定间距 `ImeToolbar.READING_GAP_DP = 3`，做成读音行容器 `candidateHeader` 的下内边距。`readingRowHeight()` 把 `candidateHeader` 的上下内边距和读音视图自己的内边距一起交给 `ReadingRowPolicy.heightPx`，读音行高度取 14 dp 与读音及同行文字控件的完整字体高度加内边距的最大值。字号再大，这 3 dp 也不会被读音挤掉；空闲工具栏和「最近复制」那一行的同高逻辑按读音行加候选行算，跟着一起变，打字时键盘仍不跳。
- #10057 的英文「清除」按钮是 14sp，且保留 Android 默认的字体留白，比默认 12sp 读音更高。`readingLabelHeight` 按每个控件的 `getIncludeFontPadding()` 选择字体的 `top/bottom` 或 `ascent/descent`，再加控件的复合内边距和读音行容器的上下内边距（含候选间距）；读音行取这些高度的最大值。收起的同行控件也预留高度，按钮显示与隐藏时键盘不跳动。字体留白语义按 [Android TextView 文档](https://developer.android.com/reference/android/widget/TextView#getIncludeFontPadding())，英文清除行为仍归[英文当前词清除](../feature/2026-10-22-android-english-word-clear.md)。
- Android 默认读音字号不改（产品决定）：没改过设置的用户「预编辑字号」是共享默认 15，换算后仍是 12sp，和修之前看到的一样；设置 12–32 对应 9.6–25.6sp，每一档都会变。共享默认值不动，`ReadingRowPolicy.DEFAULT_PREEDIT_FONT_SIZE` 由 `ReadingRowPolicySmoke` 对着 `crates/client-core/src/preferences.rs` 的 `default_candidate_preedit_font_size` 锁住，那边改了这边的换算基准必须跟着改。读音行最低高度同时受读音与同行按钮约束，键盘总高度随行高增加；按钮显隐不额外改变高度。
- 设置界面不变：保留现有的「字号 / 预编辑字号」两个 12–32 下拉框，不改成小、中、大三档。
- 读音的颜色、背景和内边距仍是 `ImeStyler.applySkin` 先设、`styleTopRow` 再改（颜色改成皮肤的 `hint`，背景和内边距清空），这次只理清字号。`ImeStyler` 里 `brandPillVisible` 那一支在当前代码里永远不成立（`render()` 只把它赋成 `false`），没有动它。

后续的「编辑拼写」图标接入让 `applySkin()` 也可能在 `styleTopRow()` 之外由绘制前兜底监听单独调用；因此 `brandPillVisible` 为假时必须在同一调用中清掉旧药丸的背景、内边距和文字色，不能依赖下一次完整 render。

## Alternatives considered

- **把「清除」按钮缩成 12sp** — 改动最少，也接近默认读音高度。不采用：用户调小读音、系统放大字体或换字体时仍可能放不下，而且缩小了独立按钮的可读性。行高直接按实际控件需要的空间计算。

- **在 `styleTopRow` 里照抄 `ImeStyler` 的 `brandPillVisible ? 12 : candidatePreeditFontSize`** — 改动最小，一行就修好；但字号仍然有三处在写，下次有人在其中一处改字号，另两处就会悄悄盖掉它，这次的 bug 就是这样来的。
- **把读音的颜色、背景、内边距也全部收归 `ImeStyler`** — 读音样式彻底只有一个归属方；但 `styleTopRow` 的颜色是皮肤的 `hint`，`ImeStyler` 的是候选的序号色，合并就得改掉当前读音的颜色，这不在 #6107 的范围里，要单独决定。
- **读音直接画成「预编辑字号」的 sp 值** — 设置页上的数字和 sp 一一对应；但共享默认是 15，所有没动过设置的用户读音会从 12sp 变成 15sp、键盘再高 3–4 dp，违背「Android 默认读音字号不改」。第一版就是这样做的，review 时改掉。
- **只在用户改过设置后才按设置走，否则保持 12sp** — 偏好文档里 `candidate_preedit_font_size` 总有值（共享默认 15），宿主分不出用户是没动过还是特意选了 15；按「等于 15 就画 12sp」处理会让 14 比 15 画得还大。
- **在共享偏好里给 Android 换一个默认值（12）** — 设置页数字和 sp 一致；但改动跨到 `crates/client-core` 和设置页，老用户文档里已经写着 15，还得做迁移，不在这次范围里。
- **间距从 42 dp 的候选行里挤出来，键盘总高度不变** — 打字时键盘高度和修改前一样；但候选行的 42 dp 本来就是为选中 chip 和释义实测出来的最小值（34、36 dp 时会压到下面的键），再挤 3 dp 会重新出现裁切。
- **设置页改成小、中、大三档（或像鸿蒙手机那样受限范围的滑块）** — 用户原话要三档，也能避免字号过大；但现有下拉框在其他平台共用，修好之后「能调」的需求已经满足，改设置页是另一项产品决定。

## Consequences

- **收益**：「预编辑字号」在 Android 上第一次真正生效；读音和候选之间有可见的间距，字号调大时读音行跟着加高，y、g、p 的下伸部不会被切。
- **代价**：Android 上「预编辑字号」的数字不等于读音的 sp（15 画成 12sp，20 画成 16sp），和 iOS 一样是相对默认值的比例；除了读音与候选之间的 3 dp 间距，行高还必须容纳同行按钮的字号与字体留白。小读音字号时也不低于按钮所需高度，按钮收起时继续预留。
- **未覆盖**：候选字号（「字号」）只作用于候选条，展开面板里的候选仍写死 17sp（`ImeCandidates.expandedCandidateButton`），面板格子固定 44 dp 高，要让它跟设置走得先改格子高度，这次不做。候选行固定 42 dp，「字号」调到接近 32 时候选可能被裁切（读代码推出，未在设备上确认），同样不在这次范围内。读音的颜色和内边距仍有两处在写，见上文。
- **验证缺口**：本地 arm64 API 35 模拟器上用 adb 截图看过第一版；#10057 在 API 28 安装后的原生宿主隔离视图中直接设置换算后的最小、默认、最大读音字号（9.6/12/25.6sp）进行补查，每档覆盖系统字体 1 倍和 2 倍，并检查实际布局与合成截图。未在报告者的 vivo Android 11 真机验收，也未遍历全部中间档位。

## Verification

- #10057 的 API 28 隔离产品布局探针在原实现复现：默认读音下按钮可用 37 px、布局需要 49 px；系统字体 2 倍时可用 43 px、需要 57 px。修复后六组布局均不裁剪、按钮显隐行高一致；普通字体下默认行高 57 px、按钮可用与所需高度均为 49 px，系统字体 2 倍时行高 65 px、可用与所需高度均为 57 px。六张合成截图中的读音与「清除」均完整显示。原生 Java 宿主 APK 经 Gradle 构建、签名、对齐校验并覆盖安装；未改的 Rust 原生库及资源从先前设备验证包复用，151 个文件与旧包逐字节一致，不代表本批重新构建 Rust。首次探针因本机高负载导致 Android 进程启动超时而未执行，清理其等待状态并预编译被测包后，重跑通过。完整 `NineKeyEnglishDeviceSmoke` 首轮在九键完整英文候选 `commit` 的前置用例停在拼写编辑面板，清除用例尚未执行；未改测试或产品代码，用同一安装包重跑后完整通过，包含英文当前词清除与编辑器边界。quick 首轮的编译阶段全部通过，但两个未改动的 PowerShell 契约检查分别触发 20/40 秒超时；两项单独复验通过，完整 quick 限制本会话检查与构建并发为 2 后重跑通过（退出 0），未改超时阈值或失败基线。Rust 工作区、Android 目标、Java 宿主及共享 Apple 桥均通过，其余平台按改动范围跳过。`pnpm run verify-notes` 与 `git diff --check` 通过。

- `ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`：`tests/candidate/ReadingRowPolicySmoke.java` 新增大字号加间距、默认字号刚好填满设计高度时仍加上间距两种情况，以及 `textSizeSp` 的换算（默认 15 → 12sp、12 → 9.6sp、32 → 25.6sp）和换算基准与 client-core 共享默认值一致。
- Gradle `compileFullReleaseJavaWithJavac` 编译 `ImeToolbar`、`MSIMEInputService` 的改动。

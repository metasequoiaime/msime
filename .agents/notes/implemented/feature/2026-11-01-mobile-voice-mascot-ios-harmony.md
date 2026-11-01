# Agent Note: iOS 与 HarmonyOS 点按聆听面板改画水杉娘

Status: implemented

## Problem

#10313 要求从工具栏开始语音时，聆听面板不再画「强调色圆盘 + 麦克风 + 光圈」，改成水杉娘侧耳倾听：耳边画红色弧线，有声音时弧线随信号动，没声音时不画；标题「正在聆听…」、提示行、「说完了」「取消」不变。Android 的实现在 PR #10366，复用外部听写入口的 `VoiceMascotDrawing`。按手机 UI 三端一致的规则，iOS 和 HarmonyOS 要跟进，但两端都没有工具栏语音按钮：iOS 的入口是长按空格和功能菜单「语音输入」磁贴，HarmonyOS 手机键盘工具栏也已去掉语音按钮，长按空格松手后的识别面板、语音快捷键按住时的面板和功能菜单入口共用同一个 `voiceFace()`。所以「工具栏入口」在这两端没有对应物，要找的是同一类会话：点按开始、由「说完了」结束的聆听。

## Decision

两端都按会话是否「按住」来选指示区，不按入口来源：

- iOS：`KeyboardVoicePanelPolicy.showsMascot(held:)` 为 `!held`。`KeyboardVoicePanelView(dictationHeld: false)` 覆盖功能菜单磁贴、读屏操作等全部 `startVoiceFromSpace(held: false)` 的调用；`held: true` 的长按空格保留圆球和光晕。
- HarmonyOS：`VoiceListeningPolicy.showsMascot(spaceHeld, heldByKey)` 为两者都假。空格长按开始的会话以 `fromHotkey = true` 开始，`voiceHeldByKey` 在松手收尾时不清，所以松手后的识别面板仍画圆盘；快捷键「锁定录音」的 LOCK 分支清掉 `voiceHeldByKey`，会话改按点按对待，指示区随之换成人物，和 Android `hold()` 关掉人物的语义一致。两个按住标志不是状态变量，视图用 `@State voiceMascot` 保证面板重画：`startVoice` 在任何提前返回（语音已关闭、没有麦克风权限、会话不可用）之前就按这次尝试的 `fromHotkey` 算出它，开不了录音时显示原因的面板也用这一次的指示区，不沿用上一次会话的；LOCK 清掉 `voiceHeldByKey` 之后调 `refreshVoiceMascot` 重算。功能菜单打开时若还有上一次会话没上屏的结果，面板显示的是那次会话，指示区沿用它的，不重算。

画法与 Android 的 `VoiceMascotDrawing` 是同一套几何，数值分别放进 iOS 的 `KeyboardVoicePanelPolicy` 和 HarmonyOS 的 `VoiceListeningPolicy.ts`：320×250 视口，人物方框 (20, 12) 边长 228，放大的耳部轮廓与耳内一笔的贝塞尔路径，红线圆心 (218, 111)，从水平向右上下各张开 48°。第 i 道红线在电平达到 0.3 × i 时出现，半径 33 + 15i + 12 × 电平，不透明度 (90 + 165 × 电平) / 255，线宽 3 + 2 × 电平；耳部与红线颜色固定，不随皮肤。

- 安静时不画线：电平不到 0.01（两端的平滑到位阈值）时一道也没有，不沿用圆盘光圈 0.3 的静息外扩。
- 结束录音后立即收起：iOS 的 `showRecognizing`（识别和润色共用）把圆球按钮标成 `isRecognizing`，人物按电平 0 画，之后的电平本就被面板挡掉；HarmonyOS 给面板传 `recording = voiceBusy && !voiceFinishing`，为假时按 0 画。圆盘模式的光圈仍按原来的平滑慢慢回落，不改。
- 系统「减弱动态效果」打开时 iOS 的显示电平固定为 0，人物同样没有线，与 Android 关闭动画时按静音绘制一致；HarmonyOS 的 API 21 边界见[手机聆听面板](2026-10-28-mobile-voice-listening-panel.md)。
- 排布不变：人物占用的指示区高度仍是圆盘的 1.5 倍，宽度取高度的 320/250 倍且不超过面板宽度，标题、提示、按钮和平滑规则继续按手机聆听面板那篇。

素材是随包表情包里的 `01.webp`（[移动人物贴纸](2026-10-20-mobile-mascot-stickers.md)，Android 用同一张），不新增素材或依赖：

- iOS 按清单找到这张，用 `KeyboardStickerCatalog.thumbnail` 在后台队列按指示区最大时的人物边长（96 × 228 / 250 ≈ 87.6 pt）乘屏幕倍率下采样解码一次，进程内缓存，回主线程交给发起解码的那个按钮（弱引用，面板关掉就丢弃）。
- HarmonyOS 从 `MascotStickerStore` 取同一张的沙箱 URI，`Image.sourceSize` 固定为同一个最大边长，排布变小时只缩放显示；耳部和红线画在叠在图片上的 `Canvas` 里，显示电平每推进一帧重画一次。`Image` 异步加载，`onComplete` 在读到数据时（`loadingStatus` 为 0）和解码成功后（为 1）各回调一次，只有解码成功才换人物，此前图片透明、画布只清空、下面照旧画圆盘，不会出现只有耳朵没有人物的一帧。
- 素材还没到时两端都先画圆盘、到了再换人物，与 Android #10366 位图就绪前保留麦克风圆盘一致；清单读不到或图片解码失败时一直画圆盘，识别、结束和取消照常可用。人物图片和画布不进读屏（iOS 的指示按钮本来就不是无障碍元素，HarmonyOS 两处 `accessibilityLevel('no')`）。

## Existing notes audit

- [手机聆听面板](2026-10-28-mobile-voice-listening-panel.md)：部分重叠。指示区在非按住会话下换成人物，排布、文字、平滑和按钮规则不变；在那篇的 Decision 加了指向本篇的一句，互链。
- [移动人物贴纸](2026-10-20-mobile-mascot-stickers.md)：复用同一份素材与打包路径，不改表情面板行为。
- [外部本地听写](2026-10-23-android-external-offline-dictation.md)：几何的出处，本篇只复制数值，不改 Android 外部入口。
- [iOS 按住空格说话](2026-10-21-ios-hold-to-talk-voice-session.md)、[移动端语音入口](../bug-fix/2026-10-24-mobile-voice-entry.md)、[HarmonyOS 语音直接上屏](../bug-fix/2026-10-24-harmony-voice-direct-insert.md)：无冲突，入口路由、会话协议和上屏流程不变。
- [Android 工具栏语音水杉娘](2026-10-31-android-toolbar-voice-mascot.md)：部分重叠。同属 #10313，本篇把 Android 工具栏的人物、耳部几何与红线规则搬到 iOS 和 HarmonyOS，三端几何要一起改，双方互链。

## Alternatives considered

- **HarmonyOS 按入口来源传 mascot（只有功能菜单入口为真）**：最贴近「工具栏入口」的字面意思。但 `voiceFace()` 只有一处，入口来源要额外记一个标志；而用户在意的区别是会话要不要按住，快捷键锁定后从按住变成点按，按来源判断会让锁定后的面板仍画圆盘，与 Android `hold()` 的语义相反。
- **只用 `spaceVoiceHeld && voiceHeldByKey` 现读，不加状态变量**：少一个字段。但两个标志都是普通成员，LOCK 改了标志不保证触发重画，用户不出声、电平不变时人物可能迟迟不出现；加一个 `@State` 由同一条策略算出，接线由 `tests/run.sh` 钉住。
- **人物模式下沿用 0.3 的静息光晕**：安静时也看得出在听。但报告者明确要求没有声音时没有线条，Android #10366 也是电平为 0 时不画。
- **为人物放宽排布、把它画得更大**：草图里人物占面板上半部更多。但指示区 1.5 倍圆盘、宽度按视口比例放宽已经与报告者截图的量级相当，放宽会挤掉标题行和提示行，三端也要另立一套排布。
- **iOS 在主线程同步解码**：少一次回调。但第一次打开面板会卡键盘扩展的主线程；后台解码一次、缓存复用，代价是素材到达前那一帧画圆盘。
- **iOS 在键盘视图加载时预先解码，或素材到达前让指示区留空**：能去掉每个扩展进程第一次点按面板时圆盘换人物的那一下。但预先解码让从不用语音的用户每次拉起键盘扩展都多做一次解码、多占一张位图；留空则在素材慢或失败时先看到一块空白再出圆盘。Android #10366 明确选择位图就绪前保留麦克风圆盘，三端保持同一种过渡。

## Consequences

- 三端非按住的聆听会话都画同一个人物和同一套红线，长按空格（iOS、HarmonyOS 松手前后）与按住快捷键保留原来的圆盘和光晕。
- iOS 键盘扩展常驻一张约 263 px 见方（3 倍屏）的位图；HarmonyOS 的解码交给 ArkUI，按固定的 `sourceSize` 只解一次。
- 人物模式每次电平推进都重画一次画布（HarmonyOS）或三条弧线路径（iOS），追上电平即停，没有常驻动画。
- 几何在三端各存一份，改耳部或红线时三处要一起改；两端的策略测试固定了红线的数值，Android 一侧由 #10366 的设备像素对照守住。

## Verification

- iOS：Studio 上 iOS 27 模拟器（临时创建，跑完删除）用 `xcodegen generate` 重新生成工程后跑 `SpaceVoicePanelTests` 和 `VoiceSessionTests`，32 项全部通过，其中新增人物模式选择、红线数值、几何与素材选择、点按面板画线与识别时收线四项（识别阶段另把电平直接推给指示区，确认是指示区自己按 0 画线，而不只是面板挡掉了电平），原有的点按面板几何断言改为人物指示区的宽高，功能菜单磁贴的用例断言该面板画人物。另用一个不提交的临时用例按真实 `01.webp` 渲染面板截图：满电平时耳边三道红线、静音时没有线、长按面板仍是圆盘，标题、提示和按钮位置不变。截图由 XCTest 渲染面板得到，不是真键盘界面。
- HarmonyOS：`tests/run.sh` 通过（聆听面板策略 89 项检查，含新增的人物选择、红线、几何与解码边长），新增的接线检查钉住 `startVoice` 在提前返回之前按 `fromHotkey` 算出指示区、LOCK 之后调 `refreshVoiceMascot`、面板传 `mascot`、图片地址与 `recording`、图片解码前画圆盘且画布不画耳部、两处 `accessibilityLevel('no')`；`test-harmony-arkts-subset.py`、`test-harmony-unwired-policies.py`、`test-harmony-unwired-symbols.py`、`test-harmony-string-code-points.py` 通过；Studio 上 `hvigorw assembleHap --no-daemon` 编译 ArkTS 成功（不带原生库，只验编译）。没有上设备或模拟器。
- 两端都没有在真机上用真实麦克风验收红线随音量起落，HarmonyOS 也没有看过 `Canvas` 叠在图片上的实际画面。

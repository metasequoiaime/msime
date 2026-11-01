# Agent Note: 手机聆听面板的光圈排布、音量驱动和中间文字

Status: implemented

## Problem

维护者在 Pixel 3 XL 上用「中英整句」本机模型按住说话时，聆听面板「正在聆听…」的光圈被裁掉：外圈的顶边贴着工具栏，脉冲放大到一半就被切成平的。同一面板还有三个问题：光圈是固定节奏的脉冲，看不出麦克风有没有收到声音；取消按钮的底边几乎贴着下面的键行，标题和提示挤在一起；边说边识别的引擎（本机流式、豆包流式）把中间文字塞进提示行，盖掉了「松开结束，上滑取消」，而且只有一行、放不下就看不到最新的话。

三端各自的根因：

- Android：`VoiceListeningView` 只按圆盘把内容竖直居中，脉冲环（18 dp）和音量光圈（12 dp）画在圆盘外面，不在排布里。视图高度是键区高度减去按钮行，Pixel 3 XL 竖屏约 142 dp，而圆盘、间距和两行文字已经要 132 dp，于是圆盘顶到视图上沿，光圈伸出视图被画布裁掉。音量只有系统识别服务的 `onRmsChanged` 接了，本机模型、豆包和整句上传的采集循环都没报。按钮行没有边距，用的是默认按钮底色，不随皮肤。圆盘取的是未按应用主题推导的 `s.skin`，颜色是基础绿，而键帽用的是推导后的强调色。
- iOS：旧 `KeyboardVoicePanelView` 的脉冲层按圆球中心放大到 (72+36)/72，竖排只约束了 `centerY` 和 `top >= 面板.top`，外扩的 18 pt 从没留位置。`installPanel` 把面板加在顶栏之后、没有任何 `clipsToBounds`，所以光晕直接盖到候选栏和工具栏上（顶栏隐藏时才被扩展窗口裁掉）。固定部分已要 158 pt，横屏约 161 pt、键盘调矮约 170 pt 时标签被压扁、提示行消失，更矮时约束冲突。多行 `UILabel` 的 `.byTruncatingHead` 只作用在最后一行，中间文字第一行显示最早的字、第二行显示省略号加最新的字，中间整段丢掉。
- HarmonyOS：`VoiceListening.ets` 用 `.scale` 把脉冲圆放大到 1.5 倍，缩放不占排布空间，满幅时向上伸出 18 vp；整列内容约 208 vp，而 `surfaceHeight()` 默认约 200 vp，祖先节点都不裁剪，光圈就画进了上面的工具条，按钮贴着面板底边。

## Decision

Android 长按入口的无按钮排布、距离取消与上屏后停留由[长按锁定编辑](2026-10-31-android-voice-hold-lock.md)取代；本篇的原有取舍保留，点按入口、其他平台与通用平滑规则继续适用。

iOS 与 HarmonyOS 非按住的会话（点按开始、快捷键锁定之后）的指示区按[点按聆听面板改画水杉娘](2026-11-01-mobile-voice-mascot-ios-harmony.md)换成水杉娘与耳边红线，红线安静时不画、不沿用下文的静息光圈；本篇的排布、文字、平滑和按钮规则继续适用，按住说话仍画圆盘和光圈。

三端用同一套排布、平滑和裁剪规则，数值相同（Android 的 `VoiceListeningPolicy`、iOS 的 `KeyboardVoicePanelPolicy`、HarmonyOS 的 `VoiceListeningPolicy.ts`）：

- 圆盘连同光圈的最大外扩算进一个方框，边长是直径的 1.5 倍（光圈外扩 = 直径 × 0.25）。按钮行上面的区域依次试「标题两行加提示」「标题一行加提示」「标题一行不带提示」，取第一个圆盘不小于 40 的，两行时圆盘还要不小于 52，圆盘最大 64；都放不下时圆盘停在 40、光圈收窄，再不够才缩小圆盘。上下内边距 4 和 8，方框到标题 6，标题到提示 4。标题位总按预留行数排，中间文字从一行换到两行时圆盘不变；内容整体竖直居中。点按开始时按钮行（「说完了」「取消」）与面板底边隔 10、左右各 16，排布区域是面板减去按钮行和这 10；按住说话时由松手结束、上滑取消，按钮一个也不显示，整个面板都是排布区域。维护者 2026-10-28 在 PR #10126 上拍板：按住说话去掉「取消」，让出的高度把圆盘放回 60–64，点按开始保留取消按钮（停止只能靠它），圆盘按剩下的高度。
- 光圈两圈：外圈不透明度 0.16，内圈 0.26、外扩取外圈的一半。外扩 = 光圈宽度 ×（0.3 + 0.7 × 显示音量），安静时也露出 30%，看得出在听。显示音量按经过的时间指数逼近目标，变大的时间常数 60 ms、变小 320 ms，一步最多按 100 ms 推进，离目标不到 0.01 就到位并停止逐帧重绘，不跑常驻动画。Android 和 iOS 系统关掉动画时显示音量固定为 0，光圈停在安静时的大小；HarmonyOS 的 API 21 编译约束见下文。结束录音后音量归零，晚到的采集音量不再让光圈涨回去。
- 音量来源：Android 本机模型用已有的 `LocalAsrRecognizer.Listener.onVolume`，豆包与整句上传的采集循环新增 `levels(...)` 回调，按 `VoiceAudioLevel.fromPcm16le` 换算（−60 dBFS 为静音），整句上传每次读 100 ms，系统识别服务仍用 `onRmsChanged`。iOS 键盘扩展不能录音，主 App 的后台会话在 tap 里算电平，经 `VoiceSessionState.level` 节流后传给键盘。HarmonyOS 四种识别方式的采集数据都经 `VoiceListeningPolicy.pcmLevel` 换算，`VoiceLevelThrottle` 限到 10 Hz。
- 文字：录音中有中间文字时标题位显示它，放不下预留的行数就从开头省略，留住最新说的那段（按码点或字形切，不拆表情）；提示行始终是怎么结束录音。上滑进取消区时标题是「松开取消」。结束录音后标题回到「正在识别…」（或「正在润色…」），中间文字挪到提示行，只占一行、从开头省略，没有中间文字时提示「识别完成后自动上屏」。上屏后「识别完成」加「已识别：…」，裁剪时保留「已识别：」。
- 颜色只取皮肤：圆盘与光圈用强调色（取消区换次要色），麦克风用 onAccent，标题用键前景色，提示用次要色；Android 改取 `ImeStyler.themed` 推导后的皮肤，按钮直接按皮肤上色（「说完了」强调色、「取消」普通键面）。

按住说话与点按开始的区别、上滑取消的判定、各端文案都不变。HarmonyOS 按住空格期间仍不弹面板，只在空格上显示「松开结束」「松开取消」；松手后面板出现时这次会话仍在识别，同样不显示按钮，空闲（提示、出错、识别完成）时照旧显示「返回键盘」。鸿蒙的语音快捷键按住时会直接弹面板，同样没有按钮，结束和取消走快捷键本身；快捷键「锁定录音」（松开按键不再结束）之后会话改按点按开始对待：面板恢复「说完了」「取消」和按钮行排布，提示换成点按说明，流式识别停顿后自动结束；实现上是 LOCK 分支清掉 `voiceHeldByKey`，快捷键的 STOP、CANCEL 不读这个标志，照常生效。

Android 工具栏入口的指示图按 [人物聆听界面](2026-10-31-android-toolbar-voice-mascot.md) 使用水杉娘与耳边红线；本篇的布局、文字、音量平滑、结束与取消规则继续适用，长按空格仍使用麦克风光圈。

## HarmonyOS 编译 SDK 边界

宿主锁定的编译 SDK 是 6.0.1 / API 21。系统「减弱动画」查询 `accessibility.isAnimationReduceEnabledSync` 从 [API 23](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/reference/apis-accessibility-kit/js-apis-accessibility.md#accessibilityisanimationreduceenabledsync23) 才公开；运行时判断版本不能让 API 21 的 ArkTS 编译器接受这个尚未声明的属性。聆听面板不引用该接口，保留 API 21 原本的音量反馈与追上即停止的排程。当前鸿蒙构建不承诺跟随系统减弱动画开关，待编译 SDK 升级后再接入；iOS 和 Android 的对应行为保留。

## iOS

键盘扩展不能录音，电平从主 App 传过来：`VoiceSessionService` 在音频线程按每块 PCM16 算电平存进 `VoiceSessionTap`，听写期间主线程每 100 ms 看一次，只在 `listening` 且变化不小于 0.02 时连同当前文字发布（`VoiceSessionChannel.levelInterval`、`levelStep`）。`VoiceSessionState.level` 是可选字段，为 nil 时 JSON 不写这个键，旧状态照常解码；`KeyboardVoiceDictation` 新增 `Event.level`，只在仍在聆听时发出。

面板改为手动 frame 布局，指示区就是圆球按钮自己的 bounds，光晕由 CADisplayLink（30–60 fps）在电平没追上时推进，「减弱动态效果」打开时不启动并监听开关变化。标题 16 pt、提示 13 pt 跟随 Dynamic Type，上限 24 / 19 pt：不设上限时 AX5 下横屏圆球会缩到 0。按钮宽高都不小于 44 pt，因为 iOS 按钮没有底色，「取消」两个字不到 44 pt 宽。手机横屏面板约 161 pt：按住说话没有按钮行，能排「两行标题加提示」、圆盘约 57 pt，「松开结束，上滑取消」看得见；点按开始还要减去按钮行，只能选「一行标题不带提示」。

## Existing notes audit

- [移动端语音入口直接开始听写](../bug-fix/2026-10-24-mobile-voice-entry.md)：部分重叠，入口路由、结束与取消按钮的语义不变，本篇只改面板的排布、动画和文字位置。
- [Android 自动上屏](../bug-fix/2026-10-21-android-voice-auto-insert.md)：无冲突，结果投递和「已识别：…」停留不变。
- [iOS 按住空格说话](2026-10-21-ios-hold-to-talk-voice-session.md)：部分重叠，后台会话通道新增一个可选的电平字段，其余协议不变。
- [Android 键区内系统识别服务](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)：部分重叠，`onRmsChanged` 的音量光圈改由新的平滑规则驱动，说完后收回光圈的守卫照旧。

## Alternatives considered

- **不动排布，只把光圈改小或去掉脉冲**：改动最小，圆盘保持原来的 72 dp。但 Android 竖屏的内容本来就比视图高，光圈只要外扩一点就会被切；去掉光圈又丢了「在听」的反馈，和第二个问题正相反。
- **允许光圈画出视图（父容器 `clipChildren=false`、ArkUI 不裁剪）**：圆盘不用缩小。但光圈会盖到工具栏和键行上，正是维护者报告的样子，而且三端的祖先容器各不相同，靠不裁剪保证不了在面板里。
- **中间文字仍放提示行，只改成两行**：不动标题。但提示行就是「松开结束，上滑取消」，被中间文字盖掉后用户在按住时看不到怎么取消；提示行字号小，最新的话也不醒目。
- **iOS 每块音频都把电平写进共享状态**：最灵敏。但每次写都是一次 App Group 文件原子写加一条 Darwin 通知，tap 约 12 Hz 还会叠上中间文字的写入；节流到 10 Hz、变化不到 0.02 不写，肉眼看不出差别。

- **通过可选接口投影保留新系统能力**：可以按版本与运行时存在性保留 API 23 行为，但实际 HAP 编译报 `arkts-no-ns-as-obj`，ArkTS 不允许把 `accessibility` 命名空间当作对象转换。没有用动态类型绕过编译检查。
- **直接升级编译 SDK**：能静态调用新接口，但会同时改变本机、CI 与发布工具链，超出这次修复 API 21 构建失败的范围。

## Consequences

- Android 约 180 dp 的键区里，按住说话（最常用的入口）圆盘约 61 dp、中间文字两行；点按开始要留按钮行，圆盘缩到约 44 dp、中间文字一行。iOS 和 HarmonyOS 的面板更高，按住说话时圆盘通常到 64 的上限。
- 光圈只在音量变化时逐帧重绘，安静时没有任何动画开销。
- Android 圆盘颜色改为跟随应用主题推导后的强调色，和回车键一致，不再是基础绿。

## Verification

- 真机（Pixel 3 XL，Android 12，0.6.0 原装包覆盖安装、保留数据，维护者批准用发布密钥签了这一次构建）：识别服务回落到设备自带的系统识别，从 Mac 播放一段中文语音。按住说话时没有按钮、圆盘约 61 dp，安静时只露一圈淡光圈，出声后光圈明显外扩、说完回落；中间文字在标题位，长句换成两行时圆盘位置不变；深色同样正常；光圈在任何一帧都没有被切。点按开始（功能菜单「语音输入」）有「说完了」「取消」，停顿后自动出「识别完成」「已识别：…」。系统识别在松手后几乎立刻给出结果，「正在识别…」一闪而过，截不到。
- HarmonyOS 锁定录音：按钮文字移到 `VoiceListeningPolicy.primaryLabel`、`cancelLabel`，逻辑测试覆盖按住、锁定后、点按、空闲和识别完成，`tests/run.sh` 钉住 LOCK 分支清掉 `voiceHeldByKey` 的接线；没有接实体键盘的鸿蒙手机，未上设备。
- API 21 SDK 下实际复现 `VoiceListening.ets` 引用 API 23 属性导致 ArkTS 编译失败；移除该不可编译的引用后，整合版本的 `hvigorw assembleHap --no-daemon` 完整通过，保留音量平滑与按需帧回调。

- Android：`check-host.sh` 通过（292 个 JVM 冒烟，含新增的 `VoiceListeningPolicySmoke` 和扩充的 `VoiceAudioLevelSmoke`，252 个设备套件源文件编译）。在 Mac Studio 上用同一份源码分别编出 develop 基线和本分支的 APK，装到专用 API 35 模拟器（Pixel 6，1080×2400），用一个只发回调、不录音的合成识别服务持续报音量和逐字的中间文字，按住空格截图：基线竖屏和横屏的光圈都被切成平顶、取消按钮贴着键行、中间文字挤掉了提示；本分支竖屏、横屏、深色、字体 130%、关闭动画下光圈都在面板内，中间文字在标题位从开头省略，提示行保留，取消区、识别中、识别完成三态正确。合成服务只在截图时临时编进测试包，没有提交。
- 维护者改为按住说话不显示按钮后，同一模拟器重新截图：按住说话竖屏圆盘约 61 dp、中间文字两行、没有按钮，横屏和深色下提示行都在、光圈在面板内；点按开始（功能菜单「语音输入」）仍有「说完了」「取消」，圆盘约 44 dp。
- HarmonyOS：`tests/run.sh` 通过（聆听面板 52 项检查）；`hvigorw assembleHap` 编译 ArkTS 成功；`test-harmony-unwired-policies.py`、`test-harmony-unwired-symbols.py`、`test-harmony-arkts-subset.py` 通过。没有上设备或模拟器：HAP 不带原生库时键盘起不来。
- iOS：Studio 上 iOS 27 模拟器跑 `SpaceVoicePanelTests`、`VoiceSessionTests` 和一个临时截图用例共 28 项全部通过，MSIMEApp 模拟器构建成功。完整 `MSIMEClientTests` 在 Studio 过载时停在 928 项通过，唯一失败是无关的计时用例 `IdleMaintenancePollTests`，所以完整套件结果没有拿到。截图由 XCTest 直接渲染面板得到，不是真键盘界面。
- 设备测试的文本编辑顺序检查通过正式 JNI 补齐磁盘中被省略的默认值，与同期其余 PR 的设备测试补修一致；未改变产品默认排列。
- 审查补修：HarmonyOS 的尾部截取复用 `TextPolicy.characters`，避免 ArkTS 运行时的 `Array.from` 按 UTF-16 码元拆开 emoji；由 `test-harmony-string-code-points.py` 和已有 emoji 尾部截取用例共同验证。
- 按住说话时隐藏按钮后，HarmonyOS 录音中的提示同步为「松开结束录音」，不再引导用户点击已经隐藏的「说完了」。
- 三端都没有在真机上用真实麦克风验收：电平的实际范围、光圈的流畅度、App 到键盘的电平链路（iOS）和 `measureTextSize` 与实际 `Text` 的行高是否一致（HarmonyOS）都还要真机确认。

Android 自动插入设备回归手工构造输入法服务，必须和真实服务一样装配 `ImeStyler`；面板改为即时解析主题后，缺少该组件会在绘制前空指针失败。夹具补齐真实组件，保留按钮几何、停止/取消及结果插入的全部断言。

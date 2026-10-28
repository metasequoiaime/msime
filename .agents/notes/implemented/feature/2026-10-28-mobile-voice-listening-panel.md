# Agent Note: 手机聆听面板的光圈排布、音量驱动和中间文字

Status: implemented

## Problem

维护者在 Pixel 3 XL 上用「中英整句」本机模型按住说话时，聆听面板「正在聆听…」的光圈被裁掉：外圈的顶边贴着工具栏，脉冲放大到一半就被切成平的。同一面板还有三个问题：光圈是固定节奏的脉冲，看不出麦克风有没有收到声音；取消按钮的底边几乎贴着下面的键行，标题和提示挤在一起；边说边识别的引擎（本机流式、豆包流式）把中间文字塞进提示行，盖掉了「松开结束，上滑取消」，而且只有一行、放不下就看不到最新的话。

三端各自的根因：

- Android：`VoiceListeningView` 只按圆盘把内容竖直居中，脉冲环（18 dp）和音量光圈（12 dp）画在圆盘外面，不在排布里。视图高度是键区高度减去按钮行，Pixel 3 XL 竖屏约 142 dp，而圆盘、间距和两行文字已经要 132 dp，于是圆盘顶到视图上沿，光圈伸出视图被画布裁掉。音量只有系统识别服务的 `onRmsChanged` 接了，本机模型、豆包和整句上传的采集循环都没报。按钮行没有边距，用的是默认按钮底色，不随皮肤。圆盘取的是未按应用主题推导的 `s.skin`，颜色是基础绿，而键帽用的是推导后的强调色。
- iOS：旧 `KeyboardVoicePanelView` 的脉冲层按圆球中心放大到 (72+36)/72，竖排只约束了 `centerY` 和 `top >= 面板.top`，外扩的 18 pt 从没留位置。`installPanel` 把面板加在顶栏之后、没有任何 `clipsToBounds`，所以光晕直接盖到候选栏和工具栏上（顶栏隐藏时才被扩展窗口裁掉）。固定部分已要 158 pt，横屏约 161 pt、键盘调矮约 170 pt 时标签被压扁、提示行消失，更矮时约束冲突。多行 `UILabel` 的 `.byTruncatingHead` 只作用在最后一行，中间文字第一行显示最早的字、第二行显示省略号加最新的字，中间整段丢掉。
- HarmonyOS：`VoiceListening.ets` 用 `.scale` 把脉冲圆放大到 1.5 倍，缩放不占排布空间，满幅时向上伸出 18 vp；整列内容约 208 vp，而 `surfaceHeight()` 默认约 200 vp，祖先节点都不裁剪，光圈就画进了上面的工具条，按钮贴着面板底边。

## Decision

三端用同一套排布、平滑和裁剪规则，数值相同（Android 的 `VoiceListeningPolicy`、iOS 的 `KeyboardVoicePanelPolicy`、HarmonyOS 的 `VoiceListeningPolicy.ts`）：

- 圆盘连同光圈的最大外扩算进一个方框，边长是直径的 1.5 倍（光圈外扩 = 直径 × 0.25）。按钮行上面的区域依次试「标题两行加提示」「标题一行加提示」「标题一行不带提示」，取第一个圆盘不小于 40 的，两行时圆盘还要不小于 52，圆盘最大 72；都放不下时圆盘停在 40、光圈收窄，再不够才缩小圆盘。上下内边距 4 和 8，方框到标题 6，标题到提示 4。标题位总按预留行数排，中间文字从一行换到两行时圆盘不变；内容整体竖直居中。按钮行与面板底边隔 10，左右各 16。
- 光圈两圈：外圈不透明度 0.16，内圈 0.26、外扩取外圈的一半。外扩 = 光圈宽度 ×（0.3 + 0.7 × 显示音量），安静时也露出 30%，看得出在听。显示音量按经过的时间指数逼近目标，变大的时间常数 60 ms、变小 320 ms，一步最多按 100 ms 推进，离目标不到 0.01 就到位并停止逐帧重绘，不跑常驻动画。系统关掉动画时显示音量固定为 0，光圈停在安静时的大小。结束录音后音量归零，晚到的采集音量不再让光圈涨回去。
- 音量来源：Android 本机模型用已有的 `LocalAsrRecognizer.Listener.onVolume`，豆包与整句上传的采集循环新增 `levels(...)` 回调，按 `VoiceAudioLevel.fromPcm16le` 换算（−60 dBFS 为静音），整句上传每次读 100 ms，系统识别服务仍用 `onRmsChanged`。iOS 键盘扩展不能录音，主 App 的后台会话在 tap 里算电平，经 `VoiceSessionState.level` 节流后传给键盘。HarmonyOS 四种识别方式的采集数据都经 `VoiceListeningPolicy.pcmLevel` 换算，`VoiceLevelThrottle` 限到 10 Hz。
- 文字：录音中有中间文字时标题位显示它，放不下预留的行数就从开头省略，留住最新说的那段（按码点或字形切，不拆表情）；提示行始终是怎么结束录音。上滑进取消区时标题是「松开取消」。结束录音后标题回到「正在识别…」（或「正在润色…」），中间文字挪到提示行，只占一行、从开头省略，没有中间文字时提示「识别完成后自动上屏」。上屏后「识别完成」加「已识别：…」，裁剪时保留「已识别：」。
- 颜色只取皮肤：圆盘与光圈用强调色（取消区换次要色），麦克风用 onAccent，标题用键前景色，提示用次要色；Android 改取 `ImeStyler.themed` 推导后的皮肤，按钮直接按皮肤上色（「说完了」强调色、「取消」普通键面）。

按住说话与点按开始的区别、上滑取消的判定、各端文案都不变。HarmonyOS 按住空格期间仍不弹面板，只在空格上显示「松开结束」「松开取消」。

## iOS

键盘扩展不能录音，电平从主 App 传过来：`VoiceSessionService` 在音频线程按每块 PCM16 算电平存进 `VoiceSessionTap`，听写期间主线程每 100 ms 看一次，只在 `listening` 且变化不小于 0.02 时连同当前文字发布（`VoiceSessionChannel.levelInterval`、`levelStep`）。`VoiceSessionState.level` 是可选字段，为 nil 时 JSON 不写这个键，旧状态照常解码；`KeyboardVoiceDictation` 新增 `Event.level`，只在仍在聆听时发出。

面板改为手动 frame 布局，指示区就是圆球按钮自己的 bounds，光晕由 CADisplayLink（30–60 fps）在电平没追上时推进，「减弱动态效果」打开时不启动并监听开关变化。标题 16 pt、提示 13 pt 跟随 Dynamic Type，上限 24 / 19 pt：不设上限时 AX5 下横屏圆球会缩到 0。按钮宽高都不小于 44 pt，因为 iOS 按钮没有底色，「取消」两个字不到 44 pt 宽。手机横屏面板约 161 pt，按规则只能选「一行标题不带提示」，横屏时看不到「松开结束，上滑取消」。

## Existing notes audit

- [移动端语音入口直接开始听写](../bug-fix/2026-10-24-mobile-voice-entry.md)：部分重叠，入口路由、结束与取消按钮的语义不变，本篇只改面板的排布、动画和文字位置。
- [Android 自动上屏](../bug-fix/2026-10-21-android-voice-auto-insert.md)：无冲突，结果投递和「已识别：…」停留不变。
- [iOS 按住空格说话](2026-10-21-ios-hold-to-talk-voice-session.md)：部分重叠，后台会话通道新增一个可选的电平字段，其余协议不变。
- [Android 键区内系统识别服务](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)：部分重叠，`onRmsChanged` 的音量光圈改由新的平滑规则驱动，说完后收回光圈的守卫照旧。

## Alternatives considered

- **不动排布，只把光圈改小或去掉脉冲**：改动最小，圆盘保持 72 dp。但 Android 竖屏的内容本来就比视图高，光圈只要外扩一点就会被切；去掉光圈又丢了「在听」的反馈，和第二个问题正相反。
- **允许光圈画出视图（父容器 `clipChildren=false`、ArkUI 不裁剪）**：圆盘不用缩小。但光圈会盖到工具栏和键行上，正是维护者报告的样子，而且三端的祖先容器各不相同，靠不裁剪保证不了在面板里。
- **中间文字仍放提示行，只改成两行**：不动标题。但提示行就是「松开结束，上滑取消」，被中间文字盖掉后用户在按住时看不到怎么取消；提示行字号小，最新的话也不醒目。
- **iOS 每块音频都把电平写进共享状态**：最灵敏。但每次写都是一次 App Group 文件原子写加一条 Darwin 通知，tap 约 12 Hz 还会叠上中间文字的写入；节流到 10 Hz、变化不到 0.02 不写，肉眼看不出差别。

## Consequences

- Android 竖屏 180 dp 的键区里，光圈要留在面板内，圆盘从 72 dp 缩到约 44 dp，中间文字只有一行；键区更高的设备、iOS 和 HarmonyOS 的面板通常放得下两行和更大的圆盘。想要更大的圆盘，只能缩小光圈比例或去掉按住说话时的提示行与取消按钮，这属于产品取舍，这次没动。
- 光圈只在音量变化时逐帧重绘，安静时没有任何动画开销。
- Android 圆盘颜色改为跟随应用主题推导后的强调色，和回车键一致，不再是基础绿。

## Verification

- Android：`check-host.sh` 通过（292 个 JVM 冒烟，含新增的 `VoiceListeningPolicySmoke` 和扩充的 `VoiceAudioLevelSmoke`，252 个设备套件源文件编译）。在 Mac Studio 上用同一份源码分别编出 develop 基线和本分支的 APK，装到专用 API 35 模拟器（Pixel 6，1080×2400），用一个只发回调、不录音的合成识别服务持续报音量和逐字的中间文字，按住空格截图：基线竖屏和横屏的光圈都被切成平顶、取消按钮贴着键行、中间文字挤掉了提示；本分支竖屏、横屏、深色、字体 130%、关闭动画下光圈都在面板内，中间文字在标题位从开头省略，提示行保留，取消区、识别中、识别完成三态正确。合成服务只在截图时临时编进测试包，没有提交。
- HarmonyOS：`tests/run.sh` 通过（新增 48 项检查）；`hvigorw assembleHap` 编译 ArkTS 成功；`test-harmony-unwired-policies.py`、`test-harmony-unwired-symbols.py`、`test-harmony-arkts-subset.py` 通过。没有上设备或模拟器：HAP 不带原生库时键盘起不来。
- iOS：Studio 上 iOS 27 模拟器跑 `SpaceVoicePanelTests`、`VoiceSessionTests` 和一个临时截图用例共 27 项全部通过，MSIMEApp 模拟器构建成功。完整 `MSIMEClientTests` 在 Studio 过载时停在 928 项通过，唯一失败是无关的计时用例 `IdleMaintenancePollTests`，所以完整套件结果没有拿到。截图由 XCTest 直接渲染面板得到，不是真键盘界面。
- 设备测试的文本编辑顺序检查通过正式 JNI 补齐磁盘中被省略的默认值，与同期其余 PR 的设备测试补修一致；未改变产品默认排列。
- 审查补修：HarmonyOS 的尾部截取复用 `TextPolicy.characters`，避免 ArkTS 运行时的 `Array.from` 按 UTF-16 码元拆开 emoji；由 `test-harmony-string-code-points.py` 和已有 emoji 尾部截取用例共同验证。
- 三端都没有在真机上用真实麦克风验收：电平的实际范围、光圈的流畅度、App 到键盘的电平链路（iOS）和 `measureTextSize` 与实际 `Text` 的行高是否一致（HarmonyOS）都还要真机确认。

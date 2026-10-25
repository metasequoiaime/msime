# Agent Note: Android 语音识别结果自动填入原输入框

Status: implemented

## Problem

Android 本地模型、豆包流式和系统识别已经在键盘内识别并自动上屏，但 HTTP 整句上传仍打开 `VoiceRecognitionActivity`。窗口只将转写保存到 `VoiceResultStore`，键盘不知道哪一次识别结束，因此用户说完、点完成后还要到工具栏手动插入。识别入口的交互由服务商协议决定，容易让用户把语音识别误解成只做录音。

## Decision

将 HTTP 整句上传纳入 `ImeVoiceEntry`，沿用长按空格松手结束、取消、润色、代次和原输入位置校验；点按开始时由聆听面板内的「完成」按钮结束采集。整句协议没有中间转写，不能照搬流式文字停顿计时；保持显式结束后转写及六十秒上限。

HTTP 采集在键区被麦克风权限拒绝时与系统识别一样回退到前台窗口；旧代次和主动结束后到达的拒绝不能重新发起录音。权限申请与系统服务需要前台恢复时保留独立窗口。键盘启动窗口时传入独立的 `ResultReceiver`，窗口成功保存后只回传结果 UUID。键盘保留独立于面板的原编辑连接、修订及上下文快照，原位置仍有效时按 UUID 一次性领取并上屏。短暂断开的连接最多等待三秒；窗口造成新输入会话、原位置变化或不可读时保留手动交接，不能仅凭文字相同重绑定另一个编辑器。新识别、重复回调和服务销毁使旧代次失效。独立设置入口不传接收器，仍保留原结果面板流程。

点按开始 HTTP 识别后，聆听面板会遮住原空格键，工具栏也没有常驻的语音开始键；仅提示「再点语音键」会让用户只能取消或等满六十秒。因此整句上传的面板自带「完成／取消」按钮，完成只停止采集并禁用自身，保留后台转写；退出时移除整个面板并恢复键行。

工具栏入口及所有识别器的显式结束、取消操作由[移动端语音入口](2026-10-24-mobile-voice-entry.md)补充；面板结束按钮统一显示「说完了」。

## Existing notes audit

- [系统识别留在键区](../../implemented/bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)：部分重叠；扩展同一键盘内识别契约，取代其中 HTTP 上传必须打开窗口的限制。
- [前台恢复](../../implemented/bug-fix/2026-10-15-android-speech-permission-foreground.md)：保留权限失败回退和恢复后才开始采集的规则，补充其成功结果的键盘回传。
- [百炼请求格式](../../implemented/feature/2026-10-09-bailian-asr-request-format.md)：协议与解析继续由既有识别器持有，只增加键盘调用入口。
- [转写完整性](../../implemented/bug-fix/2026-10-19-voice-asr-incomplete-reply.md)：保留拒绝明确失败和截断回复的规则，不绕开识别器读取原始文本。
- [语音结果视图](../../implemented/simplification/2026-10-19-voice-results.md)：共享 React 视图，与本次原生 Android 交接无关。

## Alternatives considered

- **全部保留窗口，只监听结果文件变化**：不用把上传接入键盘，改动入口较少；但文件只保留最新一条，不能可靠区分设置页或另一次识别生成的内容，也继续引发焦点往返。因此常规上传留在键盘，窗口回退显式回传已保存的 UUID。
- **回传转写后直接调用当前连接**：不需要额外读取文件，链路更短；但既可能重复上屏，也可能把识别文字写入用户后来切换的输入框。因此继续复用原子领取及目标快照。

## Verification

远端 `bash platforms/android/check-host.sh` 通过；原生宿主 `assembleFullRelease` 构建通过，复用既有 JNI 库和词库，不声称重编了 Rust 原生库。JVM 路由覆盖上传服务不依赖系统识别、离线回退优先；生命周期覆盖正常结束保留上传、取消释放识别器并中断上传。

API 28 模拟器 `emulator-5566` 的 `VoicePolishReplyDeviceSmoke` 通过新增交接回归：调用已安装宿主的真实投递入口，把合成转写写入 Android `EditText`，旧代次不能写入；经 Parcel/Binder 回传已保存结果的 UUID，关闭面板不清掉独立目标，结果自动插入且从存储清除；重复回调不重复写入，光标变化保留待插入结果。为遵守 Android 每个清单最多一百个 instrumentation 的限制，复用既有语音设备入口，不新增常驻测试入口。

同设备的真实按钮回归验证「完成」停止采集但不取消转写、等待期间禁用完成按钮、「取消」丢弃识别，以及整个面板退出后恢复原键行。注入旧代次与主动结束后的权限失败回调，均不会重新启动录音。

同设备的 `VoiceRecognitionLifecycleDeviceSmoke` 四项 `result`、`ready-error`、`stop-before-resume`、`refused-start` 通过。新增 `result` 验证合成系统服务可用时 HTTP 权限失败会生成前台窗口启动意图、使旧回调失效且不会重复启动，并直接验证真实 Activity 保存并回传的 UUID 可以领取到对应合成转写；其余三项保留恢复前不启动、权限失败与同步拒绝的回归。设备运行复用远端编译并同证书签名的测试 APK，恢复原识别服务、麦克风授权和合成服务启停状态。

`pnpm run verify-notes` 与 `git diff --check` 通过。远端 `bash scripts/verify-local.sh --quick` 通过，包含 Rust workspace、Android target 和 Android 主机门禁；该次远端没有 Node，笔记门禁由本机单独通过。

## Consequences

常规整句上传不再因打开独立窗口丢失原编辑位置，语音识别完成即自动上屏；必要的窗口回退可以显式通知原键盘领取本次结果。没有额外持久化格式、广播接收器或真实转写日志。

本次不实现 HTTP 整句识别的静音检测。没有使用真实在线服务凭据，不声称真实服务端识别质量已经验收。原编辑连接重建、原输入位置变化或不可读取上下文时采取保守交接，仍需用户在结果面板选择插入位置。设备回归只覆盖 API 28 的合成输入与真实 Android 组件，未声称 API 35、真机麦克风或完整系统输入法端到端验收。

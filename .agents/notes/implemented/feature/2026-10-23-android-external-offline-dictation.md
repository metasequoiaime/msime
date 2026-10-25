# Agent Note: Android 向外部调用方提供本地语音输入

Status: implemented

## Problem

#9159 希望 MacroDroid 的「语音输入」直接调用水杉的离线模型，并显示水杉娘的放大聆听耳部和随音量变化的红色信号。现有 `VoiceRecognitionActivity` 不导出，接收内部解析后的服务地址、凭据与文件路径，且把结果写入输入法待插入队列，不能直接作为外部接口。

## Decision

新增独立导出的 `OfflineVoiceRecognitionActivity`，声明 `android.speech.action.RECOGNIZE_SPEECH` 与 `DEFAULT`。仅从标准参数读取有界语言标签；本地配置、模型及运行库仍由应用自己读取并经既有共享规则解析。外部参数不能指定端点、凭据、文件路径或热词。复用 `LocalAsrRecognizer`，不调用系统识别、网络识别或云端润色。

当前共享移动配置按用户选择的识别服务解析；沿用该入口，明确要求在语音设置中选用「本地模型」。没有配置或准备不完整时显示原因及设置入口，不替用户改写服务配置。若后续需要独立于键盘服务的模型选择，须由共享 Rust 层新增本地配置解析接口，不能在 Java 再复制一套规则。

结果经 `RecognizerIntent.EXTRA_RESULTS` 返回调用者；提供标准 `EXTRA_RESULTS_PENDINGINTENT` 时按 Android 契约转交。取消、失败与空结果不含结果列表，每次调用最多投递一次。外部结果不进入 `VoiceResultStore`、剪贴板、日志或统计。录音仅在用户可见且授权后开始；返回、Home、销毁或重建时取消并释放，旧工作线程不得向新窗口投递结果。确认完成后停止采集并等待本机解码，取消则丢弃结果。权限弹窗仅暂停窗口时保留尚未开始的请求，`onStop` 在窗口真正不可见时取消未授权请求。识别器在模型及运行库准备的边界检查取消；`openRecorder`、`startRecording` 与 `stop`/`cancel` 共用启动锁，`cancel` 返回后不能再调用录音启动。句柄创建与原生取消共用另一把锁，两锁不交叉嵌套；模型冷加载不持有这两把锁。

聆听界面复用已打包的水杉娘透明贴纸，增加耳部放大表现和红色弧线。识别器在读取每个 PCM 块后报告有界的 RMS 音量，不保留额外录音、不写原始采样日志；旧监听器使用默认回调保持兼容。音量动画须由真实输入幅度驱动，不能用固定循环动画代替。

原生识别器与共享安装器接受相同的 Android 系统别名：仅允许 `/data/user/0` 指向 `/data/data`。Android 同时定义 `__linux__`，但 `/data/user` 属于 `system`，不能套用要求父目录属于 root 的 Linux 发行版链接规则。其余目录链接、模型末级链接、清单链接与多硬链接保护继续生效。

## Historical audit

- [Android 本地模型](../../implemented/feature/2026-10-12-android-local-voice-models.md)、[离线导入](../../implemented/feature/2026-10-12-android-offline-voice-import.md)：部分重叠，复用安装和选用规则，不新增模型安装器。
- [本地准备状态](../../implemented/bug-fix/2026-10-21-android-local-voice-readiness.md)：部分重叠，外部入口沿用模型与运行库均需准备的边界。
- [语音自动填入](../../implemented/bug-fix/2026-10-21-android-voice-auto-insert.md)：不同结果归属。键盘请求仍使用原队列，外部请求只回调用方，不修改原决定。
- [水杉娘贴纸](../../implemented/feature/2026-10-20-mobile-mascot-stickers.md)：复用单一素材源和现有打包路径，不另存一套位图。该笔记未单列素材授权，实施时继续核对原始引入记录。
- 检索活跃笔记中的 `RecognizerIntent`、`MacroDroid`、`VoiceRecognitionActivity`、本地语音与人物素材；其余平台的模型、文件句柄、设置和发布笔记不定义外部 Android 调用契约，没有需要废弃的旧提案。

## Alternatives considered

- 导出现有语音窗口：录音与界面复用最多。但其参数包括内部服务配置，结果进入输入法队列；外部调用与键盘请求的权限和结果归属不同，因此保留非导出边界。
- 实现 `RecognitionService`：能供 `SpeechRecognizer` 服务客户端使用。但报告者截图是应用选择器，且需求包含可见人物界面；先实现标准 Activity，并以真实 MacroDroid 实测确认，不能仅凭截图宣布互通。
- 新写一套录音和识别流程：外部生命周期可完全独立。但模型、热词、取消与运行库规则会重复；保留独立 Activity，复用已有识别器。
- 只提供接口并使用既有麦克风动画：实现更少。但遗漏原 issue 的人物、耳部和实时红色信号要求，不作为完整交付。
- 重建测试用返回键清理系统权限窗口：不改变用户选择的权限标记。但 API 28 的残留权限窗口不响应返回，清理必须点击系统拒绝按钮；用专用模拟器承受拒绝产生的标记，完成后移除专用设备，不在清理时授予录音权限。
- 旋转结束后只恢复自动旋转开关：设备较快返回原来的显示方向，但会把用户保存的固定方向留在测试值。连续调用固定方向与解冻又会受到系统设置异步监听的影响：解冻仍可能回写旧缓存。夹具先恢复模式，再单独恢复保存的 `USER_ROTATION`，复读两个系统设置并等待窗口管理器缓存消费原值；仅复读设置库仍不足以防止自动化断开时回写旧缓存。缓存检查依赖 API 28/35 实际提供的 `dumpsys window` 固定字段，不输出转储内容；字段不可识别时测试失败，不默认为已恢复。

## Device verification coverage

- API 28 与 API 35 已通过标准隐式 Intent 发现、调用、一次性 PendingIntent 成功与取消回传、独立 UID 应用的 Activity 成功与取消回传、采集启动边界取消以及授权阶段 Home；原内部 Activity 仍不导出。
- API 28 与 API 35 通过真实系统权限拒绝、未选模型和连续三次独立请求。未选模型用例只临时修改共享配置并恢复原选择，不删除模型或运行库；不把它等同于损坏资源或缺运行库覆盖。两版均已通过权限阶段的系统重建、实际显示旋转与后继实例取消；仍待设备验证：缺运行库、固定横屏初始模式的退出后设置恢复；API 35 真实模型识别尚未通过。
- 权限阶段重建夹具在主体断言结束后，通过系统权限窗口的拒绝按钮关闭残留弹窗，不用返回键：API 28 的真实权限窗口在产品实例已结束后仍可见且不响应返回，旧夹具因此失败。清理只拒绝权限；未获授权、新实例不启动录音、旧实例迟到结果不重复回传等主体断言保持不变。主体和清理同时失败时保留主体异常，将清理异常附为 suppressed；仅清理失败也使整项失败，不将测试环境清理误计为产品识别通过。
- 实际旋转专项 `offline-rotation` 先打开可旋转的合成编辑器作为浮动语音窗口的底层，避免依赖启动器的方向策略；再通过 `UiAutomation.setRotation` 改变系统显示方向，必须观测到显示旋转、旧实例销毁、后继实例创建且配置方向改变；它不调用 `Activity.recreate()`。同样保留单次取消、无转写、新实例不录音及残留权限窗清理断言。旋转期间辅助功能节点可能暂时为空，清理必须等系统权限窗消失且合成调用方重新可见，不能把空根当作已关闭；旧清理在 API 35 实际留下了孤立授权窗口。结束时恢复原来的自动旋转或固定方向并关闭合成调用方，主体与清理失败时仍独立尝试恢复；恢复失败本身也会使测试失败。仅下述实际执行的初始模式计为通过。
- 固定方向验收在建立 `UiAutomation` 之前，先让合成底层实际呈现保存的方向；Android 的自动化连接保存的是连接时的显示方向，断开时还会再次恢复。启动器可能保持竖屏而保存的用户方向为横屏，因此只在夹具内部复读设置还不够：设备驱动须同时检查仪器化正常退出后的两个设置值。早期固定模式日志虽然有仪器化通过码，但退出后的值不相等，不计入完整恢复验收；两版首轮未旋转以及 API 35 的 SystemUI 无响应现场也不计通过。
- 新增 `offline-voice-rotation.sh` 设备 runner：固定横屏后执行 instrumentation，待 `am instrument` 返回（即 `UiAutomation` 已完成断开恢复）才核对两个方向设置，失败时仍由 `trap` 恢复服务、授权和原始方向。当前工作树构建的产品与测试 APK 在 API 28 `emulator-5592`、API 35 `emulator-5590` 各通过一次该 runner；两台设备结束后均恢复原值。此 runner 的核对不把 instrumentation 内部的方向读值当作设备验收。
- 最终测试包 SHA-256 为 `cf84e6342587679ee6f51338a2ba6d6028ed394de6597712f0a49a5af61430ee`，产品沿用下述 `ad64c215b7` 原生 APK。远端专用 `msime-issue-9159-rotation-api28` / `api35` 的自动旋转、固定竖屏初始模式共四项实际旋转通过，两版 `offline-recreate` 回归另两项通过，原始结果均为 `INSTRUMENTATION_CODE: -1`；每项前后 APK 哈希一致，录音权限始终未授予，旋转项额外检查退出后系统设置精确相等且无孤立权限 Activity。API 28 日志为 `9159-dedicated-api28-rotation-cache.log`；API 35 自动模式通过记录在 `9159-dedicated-api35-rotation-cache.log` 前半段，固定竖屏及重建回归在 `9159-dedicated-api35-rotation-cache-portrait.log`。固定横屏 `USER_ROTATION=3` 的退出后值仍会变成 0，两版均不计该初始模式通过；不能用主体旋转成功掩盖恢复失败，也不推断已经支持任意固定方向。
- 本次补验使用 `ad64c215b7` 的原生产品（APK SHA-256 `2296954e13e09861c1261f0ae5641cbf328d8988adfc534615326e5c1e624e11`，语音源码来自已合并实现），仅更新测试 APK。旧夹具在 API 28 实际失败于残留权限窗口清理；本机随后出现其他会话替换 APK，初轮通过不作为最终验收。改用远端外盘中新建的 `msime-issue-9159-recreate-api28` / `api35` 专用设备（端口 5794 / 5796），两版 `offline-recreate` 均返回 `INSTRUMENTATION_CODE: -1`，测试前后产品与测试 APK 哈希分别精确一致，测试 APK SHA-256 为 `52ec03e9395cea66ee8edc96fd9788678ba836ab35838027e7d8c0b5d1c0b169`。两版均保持录音权限未授予，主体断言、权限窗口清理和正常终止均通过；原始日志为 `9159-dedicated-api28-recreate.log`、`9159-dedicated-api35-recreate.log`。完整远端 `scripts/verify-local.sh --quick`、最终笔记及 diff 检查通过。
- JVM 已验证：合成 PCM 的静音、幅度增长、符号对称、最大负数、部分缓冲与上限；API 28 真实合成音频已驱动完整识别与音量回调。
- API 28 已在飞行模式下用真实本地模型完成识别并返回请求者；API 35 仍待通过。API 28 与 API 35 的跨应用成功和取消调用均已确认语音待插入队列与系统剪贴板保持调用前的合成值。
- 待互通验证：安装官方 MacroDroid，执行其真实语音输入宏，确认返回字符串变量；标准测试调用器只能证明 Android 契约，不能替代此项；用户没有 APK，按其要求继续源码核对，不再把取得 APK 当作代码工作的前置条件。
- API 28 原生 APK 截图已确认人物、放大耳部和红色信号可见，设备用例确认信号由合成录音驱动；API 35 尚待完成。资源、生命周期与互通验证通过后才关闭 issue。

## Consequences

新增标准外部调用入口与人物实时音量界面，复用本地模型、凭据解析和运行库管理；外部调用增加独立的结果通道与生命周期，不改变键盘原有语音结果队列。宿主编译、JVM 与 API 28 的结果及取消边界已完成验证，API 28 的完整识别也已通过，API 35 与互通要求尚未完成；此提交不能作为关闭 issue 的依据。现阶段尚未取得并安装官方 MacroDroid，Activity 方案仍须互通确认。冷模型加载期间必须允许取消且不阻塞主线程；错误信息使用固定文案，不暴露路径、凭据或用户转写。现有录音有六十秒上限，外部参数不能扩大它。当前并行构建占用磁盘，新增 APK 构建前先确认剩余空间，不另建 Cargo 产物目录。

## References

- [Android RecognizerIntent](https://developer.android.com/reference/android/speech/RecognizerIntent)：标准 Activity、结果列表与 PendingIntent 契约。
- [报告者截图](https://github.com/metasequoiaime/msime/issues/9159#issuecomment-6314999955)：应用选择器证据，不能单独证明具体 action。
- [FUTO 固定提交参考](https://github.com/futo-org/voice-input/blob/d6e1eb2d139dc1a6342a4681c283686cca4bfceb/app/src/main/java/org/futo/voiceinput/RecognizeActivity.kt)：仅研究结果回传，不复制实现。
- [Android 15 的旋转设置](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-15.0.0_r1/services/core/java/com/android/server/wm/DisplayRotation.java)：`setUserRotation` 由设置监听更新缓存，`thawRotation` 使用缓存的用户方向。
- [Android 15 的自动化连接](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-15.0.0_r1/core/java/android/app/UiAutomationConnection.java)：连接时保存实际显示方向，断开时再次恢复，用于界定设备夹具结束后的验收边界。

## Verification progress

- 已通过独立审查并修复授权阶段 Home 遗漏与取消后仍可能启动录音的竞态；修复复审未发现新阻塞。
- `LocalAsrCancellationSmoke` 初次实际执行以「创建原生会话之前取消不应调用 JNI」断言失败；修复后通过。补充用线程屏障覆盖资源准备期间取消；设备用例另在实际采集入口前暂停工作线程，取消后验证没有一次 `startRecording` 调用。
- PCM 与一次性会话 JVM 用例通过；Android 入口与结果归属契约脚本通过。先前完整 host 门禁通过 260 个 JVM 冒烟及 222 个设备源码编译；最终 host 门禁通过 261 个 JVM 冒烟及 223 个设备源码编译（日志 `issue-9159-host-final.log`）。设备用例另断言未授权、`started=false` 以及权限弹窗存在，避免已授权状态误通过。
- 设备测试 APK 与真实原生产品 APK 已成功构建并安装，完整本地 quick 已通过。API 28 已通过标准入口的 PendingIntent 成功、取消、采集启动前取消和授权阶段 Home；权限用例先因系统 Launcher/SystemUI ANR 受阻，专用模拟器重启后通过。API 35 与 MacroDroid 互通仍未通过；官方论坛索引的 MacroDroid beta 下载地址返回 404，未获得安装包。

- 模型验收使用固定目录中的中英流式归档，已在主机按锁定大小和 SHA-256 校验。`offline-prepare` 在设备内经共享模型导入及偏好保存接口准备环境；`offline-asr` 等待实际麦克风、音量视图和本地模型给出合成问候后点击产品完成按钮，检查标准结果。`offline-prepare` 已在 API 28 与 API 35 经真实共享安装器完成。模拟器 gRPC 只监听本机并使用令牌，实际宿主麦克风已关闭；测试音频由系统语音合成生成。

- API 28 的真实 JNI 返回固定错误 `Not an installed local speech model`，测试确认配置使用系统别名而非规范化路径；设备目录属主与 AOSP 初始化规则一致。`LocalSpeechHandleDeviceSmoke` 新增合成清单的私有目录回归，修复前实际失败，修复后通过，同时断言应用自建链接仍被拒绝。修复后真实模型已启动，合成麦克风音频驱动音量界面；随后发现并修复下面记录的中间结果空值语义问题。

- JNI 用 `null` 表示没有更新，原 Java 文本校验却将其变为空串，使中间结果刚显示又被下一块音频清空。`LocalAsrPolicySmoke` 的新回归实际失败后，文本校验保留 `null`；其余合法性、长度与控制字符检查继续执行。修复后全套 261 个 JVM 冒烟与 223 个设备源码编译通过；独立复审确认最终结果、热词纠错及数字转换的调用链仍保证非空引用，不引入空指针。
- 修复后的 API 28 `offline-asr` 通过真实麦克风、固定模型、音量视图、合成英文问候及一次性 PendingIntent 完整链路（`issue-9159-api28-asr-null-fixed.log`，`INSTRUMENTATION_CODE: -1`）。原生产品截图已确认人物、放大耳部和红色弧线可见。仅使用合成音频，宿主真实麦克风禁用；流按实际采集速度持续供给，在测试完成后结束。该证据不代替 API 35、实际 MacroDroid 或真实设备验收。
- 最终补丁的完整 quick 检出旧源码正则仍要求无锁创建录音器与句柄；更新该契约检查，要求取消锁内创建、启动失败释放录音器，以及句柄锁内销毁。定向检查通过，完整 quick 必须重新通过。API 35 飞行模式识别首轮未注入音频，直接注入重试时模拟器宿主进程以 139 退出；均未计为识别通过，重启专用模拟器后继续验收。
- Android 系统别名的原生回归已分别在 API 28 和 API 35 通过。API 35 的权限窗口实际存在，但原观察只查询活动辅助功能根；设备用例启用交互窗口检索并检查各系统权限窗口，仍保留未授权、`started=false`、弹窗可见及 Home 取消断言。更新后的用例分别在 API 28 与 API 35 通过（`issue-9159-api28-permission-windows.log`、`issue-9159-api35-permission-windows.log`），独立复审无新增阻塞。
- API 35 的音频注入使模拟器宿主崩溃，故障栈位于 `audio_forwarder_enable`；去掉 `-no-audio` 后仍发生，不能认定禁音参数是唯一原因。AOSP 的转发实现直接访问当前输入 voice，后续须先确认设备实际录音已启动再注入；尚未获得 API 35 真实识别通过的证据。仅记录固定符号与结果，不提交系统崩溃报告或模拟器令牌。
- 更新生命周期契约后的整轮 quick 通过（`issue-9159-quick-current.log`），Linux 原生 111 项通过；本分支 API 28 四组共享库设备单测合计 2034 项通过、0 失败。API 35 确认实际录音已启动后，默认后端注入仍使模拟器宿主崩溃；显式空音频后端避免崩溃，但尚未驱动产品音量信号，不能算识别通过。用户没有官方 MacroDroid APK，后续继续按代码与标准调用器核对契约，保留真实互通的验证缺口。

- 本分支 API 35 四组共享库设备单测也通过，合计 2034 项、0 失败（`issue-9159-api35-core-current.log`）；不把隔离子进程输出重复计数。
- 标准 Activity 结果增加独立 UID 的测试调用器：它自己发出 `startActivityForResult`，产品请求不带结果 PendingIntent；只有调用器的 `onActivityResult` 将实际结果码、原始结果字段和回调次数转发给本次测试专用的报告桥。测试检查成功列表精确相等、取消时完全没有 `EXTRA_RESULTS`，并尝试第二次产品回传以覆盖迟到结果覆盖。测试 APK 不访问模型和输入法存储。初版辅助功能观察在 SystemUI 无响应弹窗及空活动根处失败，临时诊断确认调用器实际已收到一次标准成功结果；改为读取调用器的实际回调报告，避免系统窗口状态混入结果协议断言，设备重跑结果另行记录。

- 独立 UID 调用器的 `offline-activity-result` 和 `offline-activity-cancel` 已分别在 API 28 与 API 35 通过，四项均返回 `INSTRUMENTATION_CODE: -1`（`issue-9159-emulator-5566-activity-relay.log`、`issue-9159-emulator-5580-activity-relay.log`）。本次测试专用桥经过独立复审：产品请求不含报告通道，桥只转发调用器实际收到的 Activity 结果，不合成成功数据。

- API 35 在显式空音频后端下另做无显式限速、`MODE_UNSPECIFIED` 注入对照，与 API 28 已通过的注入方式对齐；已确认实际 AudioFlinger 采集启动，注入进程正常退出，但测试仍因音量信号未响应而失败（`issue-9159-api35-asr-unpaced-orchestration.log`）。没有放宽识别时限，也未将命令退出码 0 当作通过；API 35 的真实识别缺口保留。

- API 28 当前原生产品 `smoke.sh emulator-5566 --core` 以 1 退出（`issue-9159-api28-core-smoke-current.log`）：短语上屏与删除、延迟批量组字、数字层三组通过，随后在组字放大镜的长拼写准备断言失败。没有精确本分支原始基线的同场对照，故不将该失败归为基线，完整装包验收仍未完成。
- 按用户要求继续代码核对：外部 Activity 只读取共享解析的本地模型，结果仅经 `setResult` 或请求方指定的 PendingIntent 返回；取消与迟到回调共用一次性终止状态。该入口没有 `VoiceResultStore` 或剪贴板写入调用，静态核对不能证明实际 MacroDroid 互通；设备前后状态断言的执行结果见下段。

- 跨应用 Activity 成功与取消两条用例增加实际存储边界断言：以目标 UID 的前台原生首页设置合成剪贴板及语音待插入值，调用后重新取得前台焦点，再检查队列 ID、文本与时间戳完全一致，系统剪贴板也未改变。随后恢复原剪贴板、消费本测试自己的合成队列并关闭观察窗口；遇到其它队列内容不覆盖。中断遗留的同一合成值可继续观察和清理。首轮 API 35 因 SystemUI/Launcher ANR 无焦点而失败，API 28 的 `startActivitySync` 因复用已有首页等待超时；改用独立任务的新首页实例并重启自有 API 35 后，最终源码在 API 28、API 35 上的成功与取消四项均通过、均返回 `INSTRUMENTATION_CODE: -1`（`issue-9159-emulator-5566-storage-boundary-raw.log`、`issue-9159-emulator-5580-storage-boundary-raw.log`）。没有把普通流输出的通过文案代替原始仪器化退出码；这只验证返回与存储边界，不新增 API 35 的真实 ASR 通过证据。

- 存储边界夹具以 `try-with-resources` 保留主体断言，清理失败作为 suppressed 异常；剪贴板恢复、按测试自身 UUID 消费队列及每个观察窗口结束分别尝试，外层仍强制停止独立调用方。独立复审确认此前清理顺序问题已解决，最终测试包重编通过，并在 API 28、API 35 重新运行成功与取消四项，原始仪器化结果全部为 `INSTRUMENTATION_CODE: -1`。

- API 28（`emulator-5566`）与 API 35（`emulator-5590`）新增三项设备边界各自通过，六项原始结果均为 `INSTRUMENTATION_CODE: -1`（`extended-model-emulator-5566.log`、`extended-model-emulator-5590.log`）。`offline-permission-deny` 点击真实系统拒绝按钮，再确认产品失败、仍未授权、未开始准备、完成禁用、设置和权限说明可见；用户取消后只回一次取消结果，清掉调用方旧结果并保留合成标记。`offline-repeat` 用唯一通道连续三次发起请求并检查一次性回传；合成回传不等于三次真实 ASR。`offline-no-model` 经共享 JNI 保存本地服务与空模型选择，确认固定说明、完成禁用、设置入口和取消结果，识别器诊断不得出现任何阶段；按最新共享快照和 CAS 恢复原来的两个选择字段，保留其它设置的新改动并复读确认。
- 权限夹具的旧 `await` 在条件成功后再次求值，系统拒绝按钮已被第一次求值点击关闭，第二次查找使测试误报。现在成功立即返回，仍检查系统点击动作回执和真实产品状态，没有放宽权限或结果断言。API 28 权限命令不支持修改标记，本轮原有已授予状态已恢复；API 35 也恢复原权限及支持操作的标记，不承诺任意 API 28 权限标记都可恢复。最终产品 APK SHA-256 为 `a6e5a705a87ae254e247710c15484dac23e0809f879088b1e0e782aceff8d439`，测试 APK 为 `091af5fa1d9f5b7fc701b7370cea5158f067cc307cb10ab4a347ffaff1e3c40d`；纳入分支的夹具与这些设备运行的源码同字节。
- 提交 `46d73c4d7d` 的最新正常 pre-push 未通过：Linux 的徽章、启动器退避、启动注册、缺依赖语音服务和剪贴板五项测试失败；后续真实主题测试长时间处于文件同步等待，终止本任务容器后整轮 quick 退出 1，远端未更新。保留日志 `resumed-push.log` 与 `linux-failures-current.txt`，按原测试复验，不增失败基线或放宽时限；此前通过的 quick 不能代替这轮终态。

- API 35 的 `offline-recreate` 在真实系统权限窗口出现且尚未开始识别时调用 [`Activity.recreate()`](https://developer.android.com/reference/android/app/Activity#recreate())，显式等待旧实例销毁、后继实例创建并结束，在主线程核对后继没有启动识别，再向旧实例注入合成迟到结果，最终仍只回传一次取消。初版遗漏强制观察后继的断言，独立复审发现后已补齐，旧通过结果不作为后继覆盖依据。
- 重建可遗留已结束请求的系统权限窗口，下一轮最初因此未能启动；夹具只在该场景的权限窗口仍可见时按返回键并等待消失，清理失败仍执行结果通道注销且整轮失败。补齐后的完整场景通过（`recreate-emulator-5590.log`，`INSTRUMENTATION_CODE: -1`）；产品 `a6e5a705a87ae254e247710c15484dac23e0809f879088b1e0e782aceff8d439` 与夹具 `3ea9ef7193f5019cf4da9a2ab3d46f0f4b20288222e3e166d82dcb43abcf179a` 前后哈希一致，结束恢复原夹具、麦克风权限及权限标志，并确认系统回到 Launcher。本场景不等同于物理旋转、API 28 重建或 API 35 真实 ASR。

- 后续完整 quick（`extended-quick-tmpfs.log`）已退出 1：Linux 原生 111 项中 13 项失败、1 项按既有环境跳过，其余平台阶段结束。失败包括 IBus 等待、设置注册和剪贴板子进程超时，以及浮动面板断言；未把这些结果追加到失败基线。此轮启动早于最后的 `offline-recreate` 增量，不作为当前完整源码门禁通过的证据。

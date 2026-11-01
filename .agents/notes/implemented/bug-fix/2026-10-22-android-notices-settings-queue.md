# Agent Note: Android 公告请求不阻塞设置内容

Status: implemented

## Problem

#9184 的录屏中，设置首页和详情标题正常，皮肤、语音页的实际内容暂时为空。两页首次显示依赖 `HostTask.run` 的串行存储队列；首页 `NoticeBanner.load` 也把可能联网的 `NativeClient.notices` 放在同一队列，公告过期后的慢请求会挡住后续页面读写。公告的 `NoticeStore.current` 在请求期间持有独立的 `notices.lock`；关闭公告同样需要这个锁，不能让关闭操作再次挡住设置队列。

诊断包没有导航耗时或崩溃记录，不能断言报告设备上的每次延迟都来自公告。源码能够确认这条阻塞路径，设备回归使用公告锁可控地延迟真实 JNI 调用，不依赖外部网络的速度。

## Decision

公告获取和关闭都使用现有 `HostTask.runNetwork`。共享偏好的读写仍串行执行，结果仍受原视图身份及生命周期检查约束。不缓存语音凭据，不在主线程读文件，不用额外 loading 覆盖空内容。

## Historical audit

- [皮肤顶部分类](../../implemented/feature/2026-10-13-android-skins-top-tabs.md)和[设置顶部分类](../../implemented/feature/2026-10-13-android-settings-top-tabs.md)持有内容与导航的布局，部分重叠，保留并互链。
- AI 皮肤、备份、资料和社区安装视图生命周期笔记持有各页面操作状态的清理，与本次执行队列无关；保留原视图守卫。
- [带链接 Markdown](../../implemented/simplification/2026-10-15-linked-markdown.md)持有共享 React 公告渲染，与 Android 公告线程选择无关；不修改。

## Alternatives considered

- **保留最近一次皮肤和语音模型，重建时先画缓存。** 能改善重复进入，但首次进入仍在慢公告后排队，设置保存也会被阻塞，还延长凭据对象的存活时间，因此先消除错误排队。
- **延长全局加载遮罩。** 可以遮住空内容，但用户依旧要等公告请求，不能满足内容及时出现的要求。
- **只迁公告获取。** 改动最小，但关闭操作会等待正在获取公告所持有的锁，再次挡住设置队列，因此两项一起迁移。

## Verification

原生 full APK 在 API 28、35 上复现旧代码的问题：真实 `noticesRaw` 等待公告锁时，皮肤页只显示标题和分类，没有任何皮肤卡；相同内容断言失败并保留截图。仅迁移公告获取的中间版本能显示皮肤和语音的实际内容，并通过返回前台及 Activity 重建检查；随后点击「知道了」会使设置队列再次等待公告锁，皮肤内容断言再次失败。最终版本在两版模拟器均通过获取、关闭、皮肤预览、语音实际行、前后台切换、Activity 重建和关闭状态持久化验收。

设备用例复用 `SettingsDeviceSmoke -e notice-queue true`，不增加已达系统上限的 instrumentation 入口。合成公告的备份、写入都在真实公告锁内完成，防止首页先前发出的请求覆盖夹具；取得锁后才生成缓存时间。释放锁后经真实 JNI 验证合成缓存，再操作实际「知道了」按钮并核对持久化关闭状态。回归由 `smoke.sh` 在 `--core` 分支之前运行。

测试的 shell 用标准输出描述符持锁，另存握手输出；Android shell 的高位重定向描述符带 `CLOEXEC`，不能直接交给子进程的 `flock`。更新测试 APK 后，设备运行器先重启产品进程并打开 `HomeActivity`，避免向已结束的旧 instrumentation 附着。API 35 首次启动出现过 System UI 无响应弹窗，处理后基础输入验收通过；这次启动异常不计作产品回归。

原生 full APK 最终构建通过，`scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与 `git diff --check` 通过；quick 包含 Android 宿主 Java/JVM 冒烟、设备测试源码编译、Rust 工作区、Android 目标和共享 Apple bridge 检查。五项生产与设备源文件的 SHA-256 和远端实际构建工作区一致。

独立验收脚本首次在重新安装、强停产品后没有重选输入法，基础输入检查失败时两机的 `default_input_method` 都是系统 `LatinIME`。重新启用并选中水杉后，同一 APK 在 API 28 通过词组上屏与删除，在 API 35 通过词组、繁体显示与上屏、删除和密码框直输；原失败日志保留。

2026-10-31 develop 推送 CI（run 41226129397，5e33ca8407）的 API 35 在「fetch: activity recreation loads voice content while notice JNI is blocked」失败，IME showed []；此前 29 次 API 35 运行这一项都通过，期间 develop 没有改 Android 设置或公告代码。logcat 显示测试注入 `input keyevent 3` 之后，`am start -W` 在 18:31:00.386 先到达，任务仍在前台，结果码 3（直接交给栈顶）；随后 `recreate` 在 .419 排期，系统由 Home 键拉起桌面的 START 直到 .438 才出现，水杉任务被推到后台，重建出的 Activity 停在后台 15 秒，任何窗口里都没有产品节点。`input keyevent` 注入后即返回，桌面启动由系统异步分发，所以测试原先没有保证「先回桌面、再回前台」的顺序。现在在 Home 后先等产品的应用窗口从无障碍窗口列表消失，再 `am start` 回到前台。

## Consequences

公告获取和关闭可并发，共享层现有公告文件锁继续保护缓存与关闭状态。慢公告不再占用设置读写队列；普通存储调用仍可能有自身的磁盘或锁等待，本改动不承诺所有页面在任意设备上零耗时。

不增加跨视图模型或凭据缓存，旧视图的结果交付守卫保持。报告人的 vivo 真机没有可控复现条件，设备回归证明的是已确认的排队缺陷。

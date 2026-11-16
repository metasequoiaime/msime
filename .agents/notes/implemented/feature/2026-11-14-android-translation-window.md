# Agent Note: Android 独立翻译窗口

Status: implemented

## Problem

#11347 需要在聊天时独立编辑原文、预览译文并手动上屏，同时接受手写与语音。现有候选翻译固定以中文为源语言，不提供完整文本编辑入口。

## Decision

工具栏功能菜单打开原生 Activity，原文使用普通 EditText，继续使用当前输入法及其手写能力；语音按钮复用已有识别配置与一次性语音交付。选择源语言和目标语言后显式请求翻译，复用共享 Rust 请求描述器、签名、响应解析及既有服务选路。候选显示总开关不限制这次手动翻译，不修改任何已有偏好默认值。

窗口不自动读取聊天正文或剪贴板，不保存草稿或译文。编辑原文或切换语言立即撤销旧译文，代次挡住迟到的回复。返回原编辑器后须同时核对包、字段、输入类型、选区和前后文；无法确认时只保留内存结果，等待用户重新打开窗口自行复制。重复回调不得重复插入。

「插入」要等输入法确认接管才领取译文并关闭窗口：窗口把一个回复接收器随译文送出，输入法核对代次后回 ACCEPTED 并把译文记进 retained，接收器过期（键盘中途换成别的输入法、服务已销毁）时回 INVALID；2 秒内收不到回复（输入法进程已退出，send 静默丢弃）按失败处理。失败后窗口保留译文、只能复制。原编辑器要等窗口关闭才重新连上输入法，所以确认只能表示「已接管」，真正写入仍在窗口关闭后按前述核对进行，写不进去时译文留在 retained。在窗口自己的原文框里再点「翻译」直接返回，不作废窗口持有的接收器；从别的编辑器打开时用 `FLAG_ACTIVITY_CLEAR_TASK` 换成新窗口，旧窗口的接收器已经作废，不能被带回前台。上次没交付的译文只作复制预览，同一次打开里新翻译的结果仍按新取的回插目标插入。原编辑器禁止个性化学习时，原文框同样带上 `IME_FLAG_NO_PERSONALIZED_LEARNING`。窗口声明与拍照识字窗口相同的 `configChanges`，旋转、深色模式切换和分屏不会重建窗口、丢掉原文与预览。

窗口用 `WindowInsetsCompat` 同时读取系统栏和输入法占用，复用 `WindowInsetsPolicy.bottomContentInset`，让底部复制与插入按钮留在键盘上方。只避让系统栏会在 Android 15 上遮住操作；设备测试在真实输入法出现后检查两个按钮的边界。

## Historical audit

- [Android 用户翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)：部分重叠，复用其共享请求与响应处理，候选仍默认以中文为源语言。
- [阿里云机器翻译](../../implemented/feature/2026-11-14-android-aliyun-translation.md)：部分重叠，窗口使用相同路由与标准版、专业版配置。
- [设置页归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)：部分重叠，采用 Android 原生界面。

## Alternatives considered

- 在输入法窗口里放可编辑原文框：可紧贴键盘，但输入法窗口不能作为普通编辑器建立自己的输入连接，容易将组字和译文写入聊天框。独立 Activity 获得真实的编辑连接。
- 识别完成后直接翻译并上屏：步骤少，但不能检查识别和翻译结果，且异步回调可能遇到编辑器切换；保留预览和手动插入。
- 窗口等输入法 `commitText` 成功后再关闭：原编辑器在窗口关闭前拿不到输入连接，这样会互相等待；改为确认接管加 retained 兜底。
- 用 `FLAG_ACTIVITY_MULTIPLE_TASK` 每次另开窗口：旧窗口仍留着、插入照样无效；改为清栈换新。

## Verification

新增行为测试覆盖原文或语言变化作废译文、迟到回复、重复交付和编辑器位置保护。Android 宿主 353 项 JVM 冒烟通过，产品 APK 和设备测试 APK 构建通过；基础版本 quick（运行 `20261114-185121`）通过。设备回环服务运行 `20261114-190401` 在 API 28/35 验证显式翻译请求、中英双向描述器、预览不提前上屏与手动插入，API 35 再验证从功能菜单到原编辑器的完整返回路径。截图随后发现按钮遮挡；修复后的产品 APK 与测试 APK 重建，API 28/35 的 `HardwareTranslationDeviceSmoke -e translation-window-only true` 再次通过，包含键盘上方按钮边界断言和 API 35 完整回插。最终截图还实点 `n`、`i`、空格，将“你”输入原文框，原聊天框未改变。最终 `20261114-192452` 的宿主检查、产品 APK 和 quick 门禁全部通过，API 35 对最终 APK 再次验收成功。AndroidX Activity 与只依赖 SDK 的 IME 入口保持编译边界，协议常量由入口持有，入口以组件名打开 Activity；带 AndroidX 的界面由产品 Gradle 与设备测试验证。结果与前后截图写入独立 PR。

后续在 Pixel 3 XL（Android 12/API 31）复验时，发现 `UiAutomation` 对原文 `EditText` 的 `ACTION_CLICK` 只改变焦点、不拉起输入法，导致“更多”节点和输入法窗口根本不存在；代码中品牌键的稳定无障碍描述是“更多”，不能按可见文本匹配。设备测试改为等待 500 ms 后用 shell 触摸注入真实点击，并按描述匹配键盘边界；远端重建测试 APK、安装到 `8AFY0K0SC` 后，`HardwareTranslationDeviceSmoke -e translation-window-only true` 通过。

## Consequences

不同服务支持的语言组合仍由该服务与共享描述器决定；失败显示固定提示，不展示凭据或原始服务回复。Activity 结束或进程重启后未插入结果不持久化。

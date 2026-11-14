# Agent Note: Android 独立翻译窗口

Status: implemented

## Problem

#11347 需要在聊天时独立编辑原文、预览译文并手动上屏，同时接受手写与语音。现有候选翻译固定以中文为源语言，不提供完整文本编辑入口。

## Decision

工具栏功能菜单打开原生 Activity，原文使用普通 EditText，继续使用当前输入法及其手写能力；语音按钮复用已有识别配置与一次性语音交付。选择源语言和目标语言后显式请求翻译，复用共享 Rust 请求描述器、签名、响应解析及既有服务选路。候选显示总开关不限制这次手动翻译，不修改任何已有偏好默认值。

窗口不自动读取聊天正文或剪贴板，不保存草稿或译文。编辑原文或切换语言立即撤销旧译文，代次挡住迟到的回复。返回原编辑器后须同时核对包、字段、输入类型、选区和前后文；无法确认时只保留内存结果，等待用户重新打开窗口自行复制。重复回调不得重复插入。

窗口用 `WindowInsetsCompat` 同时读取系统栏和输入法占用，复用 `WindowInsetsPolicy.bottomContentInset`，让底部复制与插入按钮留在键盘上方。只避让系统栏会在 Android 15 上遮住操作；设备测试在真实输入法出现后检查两个按钮的边界。

## Historical audit

- [Android 用户翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)：部分重叠，复用其共享请求与响应处理，候选仍默认以中文为源语言。
- [阿里云机器翻译](../../implemented/feature/2026-11-14-android-aliyun-translation.md)：部分重叠，窗口使用相同路由与标准版、专业版配置。
- [设置页归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)：部分重叠，采用 Android 原生界面。

## Alternatives considered

- 在输入法窗口里放可编辑原文框：可紧贴键盘，但输入法窗口不能作为普通编辑器建立自己的输入连接，容易将组字和译文写入聊天框。独立 Activity 获得真实的编辑连接。
- 识别完成后直接翻译并上屏：步骤少，但不能检查识别和翻译结果，且异步回调可能遇到编辑器切换；保留预览和手动插入。

## Verification

新增行为测试覆盖原文或语言变化作废译文、迟到回复、重复交付和编辑器位置保护。Android 宿主 353 项 JVM 冒烟通过，产品 APK 和设备测试 APK 构建通过；基础版本 quick（运行 `20261114-185121`）通过。设备回环服务运行 `20261114-190401` 在 API 28/35 验证显式翻译请求、中英双向描述器、预览不提前上屏与手动插入，API 35 再验证从功能菜单到原编辑器的完整返回路径。截图随后发现按钮遮挡；修复后的产品 APK 与测试 APK 重建，API 28/35 的 `HardwareTranslationDeviceSmoke -e translation-window-only true` 再次通过，包含键盘上方按钮边界断言和 API 35 完整回插。最终截图还实点 `n`、`i`、空格，将“你”输入原文框，原聊天框未改变。最终 `20261114-192452` 的宿主检查、产品 APK 和 quick 门禁全部通过，API 35 对最终 APK 再次验收成功。AndroidX Activity 与只依赖 SDK 的 IME 入口保持编译边界，协议常量由入口持有，入口以组件名打开 Activity；带 AndroidX 的界面由产品 Gradle 与设备测试验证。结果与前后截图写入独立 PR。

## Consequences

不同服务支持的语言组合仍由该服务与共享描述器决定；失败显示固定提示，不展示凭据或原始服务回复。Activity 结束或进程重启后未插入结果不持久化。

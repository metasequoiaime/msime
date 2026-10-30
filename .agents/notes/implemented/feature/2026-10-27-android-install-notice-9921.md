# Agent Note: Android 首次安装与升级临时说明

Status: implemented

## Problem

用户在首次安装或升级后可能不知道词库准备、输入方案切换等已知使用条件，重复反馈同一类问题。当前宿主没有按安装包版本显示一次性说明的机制。

## Decision

Android 宿主在主界面回到前台且首次引导完成后，读取当前安装包的 `lastUpdateTime` 作为安装标记。缺少已显示标记时视为首次安装，标记变化时视为升级；每个安装标记只显示一次临时说明。说明使用现有 Material 对话框，并按文案长度计算 4 至 30 秒的自动关闭时间，用户仍可立即点“知道了”。延迟回调仅在 Activity 仍有效且窗口仍附着时关闭；关闭时取消未执行的回调，页面已结束时不再调用页面的后续动作，避免系统先移除窗口后再关闭 Dialog 引发崩溃。开屏和首次引导期间不打断用户，首次安装的说明在引导结束返回主界面后出现。

## Alternatives considered

- **在每次启动都显示**：最直接，但会把临时说明变成持续打扰，也无法满足“升级后首次运行”这一边界，因此不采用。
- **只按版本名记录**：版本名可能不变而安装包已替换，不能可靠识别升级；采用系统提供的 `lastUpdateTime`，并在查询失败时静默跳过。

## Consequences

说明内容集中在 `InstallNotice`，显示判定和倒计时边界集中在无 Android 依赖的 `InstallNoticePolicy`，可由 JVM 冒烟测试覆盖。SharedPreferences 只保存当前安装标记，不保存用户输入或账号信息。安装包查询失败时不弹窗；临时说明仍需后续根据实际产品稳定情况移除。

验证包括 `bash platforms/android/check-host.sh` 和 `CI=true pnpm run verify-notes`；完整 APK 与模拟器验收受当前机器磁盘余量及固定 vcpkg 依赖限制，需在具备构建环境后补做首次安装、升级和自动关闭验收。

设备复现发现设置页销毁后 `InstallNotice` 的延迟回调仍调用 `dismiss()`，触发 `View not attached to window manager`。`OnboardingDeviceSmoke -e install-notice true` 在真实主界面显示说明后销毁 Activity，经过完整 30 秒期限后检查进程与回调边界；此专项复用引导验收入口。

审查补验：Android 宿主检查 296 项 JVM 冒烟、设备测试源严格编译和 APK 打包通过；API 35 专用模拟器中 `OnboardingDeviceSmoke -e install-notice true` 实际通过，确认主界面销毁后经过完整自动关闭期限不崩溃，也不调用旧页面回调。其余键盘专项的失败单独保留，不算整套通过。

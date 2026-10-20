# Agent Note: Android 设备冒烟的预览时序与语言资源契约

Status: implemented

## Problem

CI run `39349166940` 的两个 Android job 都在设备冒烟阶段失败。API 28 的 `NumberPadDeviceSmoke` 在没有语言词库的 APK 上仍强制切换笔画和注音方案，产品按既定规则回退到全拼，测试却把回退键面报成产品失败。API 35 的 `QuickDeletePreviewDeviceSmoke` 只在长按 600 ms 后取一次截图；输入法已经连删，但 Android 15 模拟器有时尚未把覆盖层绘制事务提交给截图，九键预览因此间歇性报不可见。

## Decision

设备 workflow 的两个 API 级别都下载并校验锁定的语言词库。`NumberPadDeviceSmoke` 因而可以在 API 28 和 API 35 使用相同的笔画、注音键面覆盖，测试前提与它实际验证的方案一致；产品缺少词库时的正常回退仍由其他资源缺失场景覆盖。

`QuickDeletePreviewDeviceSmoke` 在首次 600 ms 等待后，于手指保持按住期间最多再等待 1.2 秒，每 100 ms 重新截图同一段键盘区域，直到达到原有的变化像素阈值。它仍检查长按没有清空文本、释放或取消后连删停止，只把单个截图时刻改成有界的屏幕提交等待。

## Alternatives considered

- **只在 API 35 下载语言词库**：改动最少，但 API 28 的数字页测试仍会在固定矩阵中选择不可用方案，失败信息继续误导；让 workflow 的资源契约覆盖实际测试矩阵更直接。
- **从 API 28 的 `NumberPadDeviceSmoke` 删除笔画和注音用例**：可以保留较快的 API 28 job，但会让最低支持版本失去真实方案键面的覆盖；保留用例并补齐它需要的锁定资源更能发现旧系统上的布局问题。
- **把预览断言改成只查无障碍节点或延长一个固定睡眠**：覆盖层刻意不进入无障碍树，固定睡眠仍依赖模拟器调度；有界重复截图直接等待可观察的像素提交。

## Consequences

两个 Android job 各多下载一组已锁定的语言词库，构建时间和网络用量略增；API 28 的设备门禁现在覆盖与 API 35 相同的语言键面。快速删除设备测试在模拟器绘制延迟下更稳定，但最长等待增加到约 1.8 秒，真正没有显示预览仍会在有界期限内失败。

## Verification

- 原始 CI 日志确认 API 28 在构建阶段明确打印 `no language dictionaries`，随后失败于 `stroke letter face`；API 35 打印三个语言词库已打包，数字页通过。
- 本机 arm64 API 35 模拟器在未修改测试时重跑九键预览，连续运行中复现同一像素断言失败；应用和输入法没有崩溃。
- 应用 APK 使用仓库脚本构建，修订后的测试 APK 安装到专用 API 35 模拟器；修改后的 `QuickDeletePreviewDeviceSmoke` 连续 5 次通过，包含 26 键与 9 键静止长按、取消和上滑清空路径。

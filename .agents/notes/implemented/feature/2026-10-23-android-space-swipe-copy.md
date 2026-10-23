# Agent Note: Android 上滑空格复制当前输入框全文

Status: implemented

## Problem

#9332 希望复制 GitHub 表单、MacroDroid 等当前编辑框全文时，不必打开文本编辑面板再依次全选、复制，而是在空格键上滑一次，复制后看到已有最近复制预览。

## Decision

空格键短按和横滑保持原有语义；未进入语音或水平光标移动时，明显以上移为主、超过 24 dp 的上滑锁定复制，取消长按计时。键面显示「松开复制全文」，松手执行一次复制，系统取消不复制；已经开始按住说话后的上滑仍属于语音取消区，不转换成复制。上下文存在组字或不适合复制时不进入新手势，避免把选候选的空格意外变成编辑器操作。

复制由编辑器依次执行 `performContextMenuAction(selectAll)` 与 `performContextMenuAction(copy)`；不由输入法读取文本再写剪贴板。允许先读取选区位置、在全选后验证选区，复制完成后，在仍为本次全选或复制菜单将其正常折叠到文末时尽力恢复原有选区，避免下一次输入替换整篇文字；位置未知或选区无法可靠验证时放弃。调用链绑定同一输入连接，处理读取异常、菜单拒绝、连接变化和选区变化。密码框不发起全选或复制。

复制后的预览复用系统剪贴板监听与已有「最近复制」行，不自动启用剪贴板历史、不绕过隐藏工具栏、无痕、禁止学习和敏感内容规则。手势自身给一次按键反馈并恢复键面，状态不持久化，不新增设置。

## Related notes audit

- [文本编辑面板](../../implemented/feature/2026-10-08-android-text-edit-panel.md)：部分重叠；沿用编辑器拥有复制权限的决定，新增独立手势入口与原选区恢复。
- [最近复制工具栏](../../implemented/feature/2026-10-08-android-recent-clip-toolbar.md)、[文本编辑中最近复制](../../implemented/feature/2026-10-15-android-recent-copy-in-text-edit.md)：部分重叠；复用显示位置、监听及隐私开关，不改变预览决定。
- 其他空格命中是底栏、各语言空格上屏、iOS 语音及数字层布局，没有 Android 上滑复制实现；PR 和活跃工作区未发现 #9332 重复任务。

## Alternatives considered

- 直接提取全文并写剪贴板：可以绕过选区切换，也可覆盖不支持上下文菜单的编辑器；但会绕过应用拒绝复制的规则，因此不采用。
- 上滑刚越过阈值立即复制：反馈更快；但系统随后取消也无法收回已经覆盖的剪贴板，选择松手执行。
- 无条件显示新复制内容：更符合 issue 对实时展示的字面预期；但用户可能明确关闭剪贴板读取和预览，继续遵守原有偏好，不为手势自动开启历史。

## Verification

- `check-host.sh` 通过 257 个 JVM smoke、217 个设备源码编译与 JNI/Java 契约。完整当前原生库与发布宿主 APK 构建通过，使用开发签名并校验 16 KB 对齐。
- 旧 APK 在 API 28 的新专项中确定失败：上滑后没有「松开复制全文」键面。
- API 28/35 的完整 `TextEditRecentClipboardDeviceSmoke` 均通过，覆盖原生 EditText 和 WebView 的全文复制与光标/选区恢复、松手前不复制、超过语音时限仍保持已锁定的复制手势、滑回/系统取消、多输入框、拒绝全选/复制、未知位置、密码、预览开关、真实短按/横滑及原有编辑面板回归。
- 最终 quick `20261023-044100` 通过，笔记和差异检查通过。

设备夹具直接在 `EditText` 创建时设置合成光标或选区，不通过 `ACTION_SET_SELECTION` 初始化：Android 9 的该无障碍动作会启动智能选择模式，迟到的 `SmartSelectSprite` 动画会在复制结束后重新全选，干扰对产品恢复位置的判断。调查时从合成编辑器的选区回调定位到系统 `SelectionActionModeHelper`，已移除临时诊断字段。旧 WebView 没有把 textarea 的 `aria-label` 暴露为 `contentDescription`，专项按夹具中唯一可编辑节点定位，复制结果仍核对完整合成文本。

## Consequences

收益是无需离开键区即可复制全文，并保留原选区；代价是需要编辑器提供完整选区元数据与上下文菜单支持。

InputConnection 的跨进程 true 可能只表示已投递，编辑器拒绝复制或不提供可验证选区时不能保证成功；不承诺覆盖任意自定义输入框。系统剪贴板监听是异步的，预览仍由现有开关与生命周期决定。选区恢复使用同一连接并核对已选中的位置，应用并发修改仍没有通用文档版本号可排除。

Android TextView 复制会结束文本操作菜单，Editor 在菜单销毁时可能将选区折叠到末尾，原位置恢复需要接受这一落点。实现依据是 [Android 9 TextView](https://android.googlesource.com/platform/frameworks/base/+/android-9.0.0_r1/core/java/android/widget/TextView.java) 与 [Editor](https://android.googlesource.com/platform/frameworks/base/+/android-9.0.0_r1/core/java/android/widget/Editor.java)；WebView 的提取和选区接口由 Chromium 的 [ThreadedInputConnection](https://chromium.googlesource.com/chromium/src/+/38c3eb61c737a8d3313ca8cd31b0c514c9d35b05/content/public/android/java/src/org/chromium/content/browser/input/ThreadedInputConnection.java) 提供，仍以设备验收为准。

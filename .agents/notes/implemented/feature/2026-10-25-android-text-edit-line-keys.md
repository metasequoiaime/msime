# Agent Note: Android 文本编辑行边界与换行长按符号

Status: implemented

## Problem

#9601 希望在文本编辑面板直接移动到当前行的首尾，并通过换行键长按直达符号面板。现有首尾键发送 Ctrl+Home／End，只覆盖整篇文档；进入符号还需先退出编辑面板。

## Decision

在既有四行网格的上排两个空位加入固定的行首、行尾，不加入七个可排序侧边动作，不改变保存格式。新键向 InputConnection 发送无 Ctrl 的 Home／End；选择模式沿用既有 Shift 按键通道，由宿主决定自动折行、双向文字和选区边界，不读取文本后自行寻找换行符。底部原有文档首尾继续保留 Ctrl。

换行键短按只提交一次换行，长按关闭文本编辑并进入现有符号面板，消费长按以避免松手又插入换行。键面上半部显示「符号」、下半部显示「换行」，中间绘制分隔线，字号按实际键宽高收缩；无障碍描述同时说明长按动作。手势取消和隐藏继续由 Android 按钮生命周期处理。

触控板模式保留上排两个行边界键，触控区域下移一行、覆盖中央两行；侧边和底部操作继续同屏可用。

## Related notes audit

- [固定网格与七键排序](../../implemented/feature/2026-10-24-android-text-edit-layout.md)：部分重叠，本篇只替代上排两个空位和触控板覆盖前三行的决定，保留侧边顺序、保存校验和其余布局。
- [原文本编辑面板](../../implemented/feature/2026-10-08-android-text-edit-panel.md)：部分重叠，沿用编辑器事件、Shift 选择与文档边界行为。
- [触控板与放大镜](../../implemented/feature/2026-10-23-android-text-edit-trackpad.md)：部分重叠，保留移动、预览和取消生命周期，只调整固定网格中的可用区域。
- 最近复制预览及工具栏排序与本次新增动作无关；没有完全吸收的旧笔记或过时提案。

## Alternatives considered

- 读取周围文本并寻找换行符：能直接得到逻辑行边界，但不能识别自动折行，部分终端也不给文本；因此交给宿主的 Home／End 实现。
- 把行边界加入侧边排序：可以复用排序页，但会改变既有七键保存契约并占掉原操作；草图明确放在上排空位，因此采用固定键。
- 保留触控板原来的完整矩形并覆盖新键：保持原触控高度，但行边界在触控模式无法点击；本篇缩小为中央两行，保留同一位置的显式按钮。
- 为长按另造计时与触摸状态机：可以完全定制时间，但系统按钮已提供长按消费、滑出取消和无障碍操作，因此复用原通道。

## Verification

- JVM 模型回归先在旧模型上报「行首和行尾必须加入固定网格」，新增后通过；十七键几何、七键排序、行与文档边界修饰键、Shift 选择均有断言。
- 完整 `platforms/android/check-host.sh` 通过：266 项 JVM 冒烟、241 份设备源码、Java/API、JNI 与资源清单检查。
- API 28 原生基线包通过逻辑行、自动折行、Shift 选择、文档首尾、触控板共存、终端事件、短按换行和滑出取消，最后在「长按打开符号」阶段预期失败。修复后的 远端 macOS 专用 API 28 已通过基础套件、行边界、自动折行、Shift 选择、短按换行、滑出取消、长按符号、五种布局截图和完整触控板套件；API 35 基础 `DeviceSmoke` 已通过，但布局套件在启动设置页后等待超过三分钟，未计为通过，设备已停止。
- 完整 `bash scripts/verify-local.sh --quick` 已退出 0；Apple、MinGW 等缺环境阶段按脚本明确跳过。笔记落盘后重新运行 `pnpm run verify-notes` 与 `git diff --check`。

## Consequences

十七个操作同屏可见，行边界不改变七键保存格式。换行保持短按一次，并提供长按符号；上半部「符号」、下半部「换行」和分隔线共用皮肤文字颜色，字体按实际键面收缩。系统按钮负责长按消费、滑出取消与松手，不增加计时状态机。

触控板可用高度下降，需要在浮动与横屏检查真实手势和几何；几何与点击通过不等于主观触控舒适度。宿主可能不处理 Home／End，不用截断的周围文本伪装支持。长按在输入连接、引擎或符号面板未就绪时消费，不额外插入换行。

本机 Bash 3 在空相机选项数组下报 `camera_options[@]: unbound variable`；沿用仓库的条件数组展开写法修复，API 28／35 均已正常启动，不改变默认相机设置。

本机在修复后复验时失去整个 Workspace 卷的读取权限。最终代码已同步至远端自有 worktree；后续用同源远端 APK 与夹具，在系统盘独立产物目录重新签名，经 adb 执行原 instrumentation。保留基线与中断证据，不把安装成功计为行为通过。

参考：[Android KeyEvent](https://developer.android.com/reference/android/view/KeyEvent)、[AOSP ArrowKeyMovementMethod](https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/master/core/java/android/text/method/ArrowKeyMovementMethod.java)。

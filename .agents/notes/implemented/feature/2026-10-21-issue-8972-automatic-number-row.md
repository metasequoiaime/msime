# Agent Note: 密码框与 SSH 工具临时展开 Android 数字行

Status: implemented

## Problem

#8972 希望在密码框以及 ConnectBot、JuiceSSH 中自动显示数字行，并保留到切换输入法。现有 Android 数字行只取本地布尔偏好，输入上下文不影响它。直接把本地设置改成开启会永久改变用户选择，按输入框退出立即复位又不满足保留到切换输入法的要求。

## Decision

Android 宿主为数字行维护仅在进程内存中的自动开启状态：进入文字或数字密码框、ConnectBot 或 JuiceSSH 时开启，普通输入框和隐藏键盘不清除；系统选中的输入法发生变化时清除。设置页保存的数字行开关不变，实际布局使用「手动开启或自动开启」。服务销毁时状态自然结束。

使用 `EditorPolicy.password` 识别密码，不读取输入内容；SSH 工具按 `EditorInfo.packageName` 精确匹配 `org.connectbot` 和 `com.sonelli.juicessh`，不把所有关闭建议的输入框当作终端。数字行仍遵循现有键面与高度约束，数字密码使用已有数字键盘，九键、手写与过矮窗口不强塞一行。符号页的补充行与字母页使用同一有效值。

监听 `Settings.Secure.DEFAULT_INPUT_METHOD`，只读取当前选中的输入法，绝不写入系统设置。观察者随服务注册与注销。切回本输入法且当前仍为目标输入框时按该输入框重新开启，输入视图开始显示时再次核对，避免异步通知先于窗口显示而抹掉新编辑器的状态。

## 历史决策审计

- [Android 数字行](../../implemented/feature/2026-10-09-android-number-row-and-split-duplicates.md) 部分重叠：本地开关、布局、高度与数字输入语义保留，本篇补充自动开启的会话状态。
- [数字符号页第五排](../../implemented/bug-fix/2026-10-16-number-symbol-extra-row.md) 部分重叠：沿用同一有效值，避免切换符号页突然缩高。
- 其余数字层标点、实体键盘数字选词和移动端数字九宫格笔记不持有这个上下文决定，保持不变。

## Alternatives considered

- 只按当前输入框实时开关。无须维护会话状态，也不需要观察系统输入法；但离开密码框立即消失，不满足 issue 明确要求的保留期。
- 自动保存本地数字行设置。能跨进程保留；但用户并未主动打开该开关，切换输入法后仍会永久开启，因此不采用。
- 将所有 `TYPE_TEXT_FLAG_NO_SUGGESTIONS` 输入框视作 SSH 工具。能覆盖未列出的终端；但普通聊天和搜索控件也使用这个标志，会误开数字行，因此不采用。

## Verification

`bash scripts/verify-local.sh --quick` 最终通过，覆盖宿主契约、Rust workspace、Android target、Android 宿主 Java 编译与 JVM 冒烟、Apple bridge。`pnpm run verify-notes` 和 `git diff --check` 通过。原生宿主 Gradle `assembleFullRelease` 与发布 lint 成功，复用已有原生库与锁定资源后签成开发测试 APK，未更改发布签名。

JVM 用例覆盖四种密码、两个具名 SSH 工具、普通输入框不触发、输入框切换保留、切换输入法清除、手动开启不被清除和高度限制。API 28、35 专用 AVD 均运行最终版 `NumberRowDeviceSmoke` 通过：真实密码框触发、字面数字输入、离开保留、系统输入法往返复位、切回密码框重新开启、设置字节不变，以及原有数字/符号页、关闭设置和横屏检查。没有把专项回归表述为整套设备冒烟通过。

设备回归并入现有测试入口，保持 Android 35 每包最多 100 项 instrumentation。测试先固定全拼和 26 键，再恢复偏好；首轮 API 28 在自动状态检查之后的旧全拼断言中，因继承设备上一次的双拼设置失败，隔离偏好后两版最终回归均通过。首轮 quick 还暴露迁移缓存中的旧 Tauri 权限路径和 CMake 路径，清理失效缓存后重跑；系统临时盘空间不足造成的首次失败不算通过。

尚未实测 ConnectBot、JuiceSSH 真应用及真机；具名终端包名的触发由 JVM 测试覆盖，不能当作这两个应用的实际验收。

## Consequences

收益是常用上下文无需手动开关，并且退出临时状态不会改变保存的设置；代价是服务多一个随生命周期注册与注销的系统设置观察者。终端识别只覆盖具名包和正确声明密码类型的控件，不能宣称任意 SSH 工具都会触发。系统输入法变更通知异步到达，回调需依据当前输入法与当前编辑器重建状态；状态不得落盘或记录应用包名到日志。

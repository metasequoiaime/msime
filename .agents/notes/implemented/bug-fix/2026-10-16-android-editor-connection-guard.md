# Agent Note: Android 编辑器连接断开时保护输入法进程

Status: implemented

## Problem

`MSIMEInputService` 通过 `EditorBridge.Sink` 调用 Android `InputConnection`。编辑器在两次跨进程调用之间失去焦点或销毁时，`begin`、`compose`、`finish`、`end` 等操作可能抛出 `RuntimeException`。原来的 `EditorBridge` 没有捕获这些异常，编辑器拒绝本次写入后，清理组字或结束批量编辑还可能把异常带出输入法主线程，导致输入法进程崩溃。已有的 [输入连接失败后自动重建](2026-10-15-android-input-connection-recovery.md) 只能处理已经进入 `fail` 的错误，不能覆盖这个更早的异常边界。

## Decision

`EditorBridge` 把 `Sink` 的运行时异常视为编辑失败：`apply` 返回 `false`，清理路径 `abandon`、`discard` 和 `settle` 尽力结束但不再向外抛出，批量编辑的 `begin` 与 `end` 也采用同一保护。这样宿主仍可按现有流程把失败归类、销毁失效会话并尝试一次恢复，不把断开的编辑器升级为输入法进程崩溃。

`EditorBridge` 之外的辅助路径通过 Android 专用的 `InputConnectionGuard` 调用编辑器，包括删除、邮箱补全、文本编辑面板、候选栏光标报告、图片发送、计算器、日语连点与手写/语音上文。写入断开返回 `false`，读取断开返回 `null`，批量结束尽力而为。批量开始的返回值代表调用没有抛异常：编辑器原生返回 `false` 可能仅表示不支持批量编辑，不能因此禁止正常写入。发键辅助方法保留系统的软键盘标记、触摸模式与虚拟设备标识，并分别保护按下与松开：空框退格、成对标点左移、空格滑动和日语九键移动不再调用系统内未经保护的 `sendDownUpKeyEvents`。批量结束绑定开始时的同一连接；不捕获业务处理或整个 UI 回调的异常。

## Alternatives considered

- 只在 `MSIMEInputService.sink` 的每个 Android 调用点分别捕获异常：可以局部止血，但 `EditorBridge` 的清理和其他宿主仍会重复实现同一边界，容易漏掉 `end` 或新增的编辑操作。
- 在 `fail` 中包住 `stop(false)`：能遮住一次清理异常，但正常的编辑操作仍会把断开连接的异常带出 `apply`，而且 `discard`、`settle` 和失焦清理仍各有未保护路径。
- 让异常继续向上传播并依赖系统重启输入法：会丢失当前输入会话，且把编辑器的瞬态断开扩大成用户可见的输入法故障。

- 只在每个按钮的回调外捕获异常：改动较少，但选区变化触发的自动读取、配置变化和 `finally` 清理仍会漏掉；守卫只包实际编辑器调用，避免把产品内部错误也吞掉。

## Verification

- `EditorSmoke` 新增合成断开 `Sink`；修复前 `EditorBridge.apply` 直接抛出 `IllegalStateException`，修复后返回失败并完成 Android host contract 的全部 231 项 JVM 冒烟。
- 该保护只捕获 `RuntimeException`，不吞掉 `Error`；现有成功、拒写、组字清理、日语定稿和笔画清理顺序测试保持通过。

- 辅助路径回归先在未修改的 `a5bf43722d` Android 宿主上运行：API 28 的 `InputFailureCategoryDeviceSmoke` 在 `deleteBeforeCursor` 阶段报告 `InvocationTargetException`，证明异常穿过产品服务。新增 JVM 冒烟覆盖每项守卫的断开返回值，以及批量开始返回 `false` 时仍继续提交。
- 空框退格回归使用读取空串成功、发键抛异常的合成连接，分别断言按下与松开都尝试一次；它在遗漏系统发键辅助方法的第一版 APK 上失败，补齐保护后在 API 28、35 的安装产品中通过。
- 同步 `develop` 后重新构建原生 Android 宿主和 `client-core` 设备测试程序：API 28、35 各 1420 项通过、0 项失败（13 项忽略、7 项过滤）；API 28 的 `smoke.sh --core` 安装验收通过。API 35 安装产品的断开回归也通过。验证使用锁定的语言词库，缺少注音词库的包会回退拼音，不能拿来验收注音布局。

## Consequences

编辑器连接在操作中断开时，本次编辑会失败，调用方可以沿用既有的恢复和“仅直接输入”降级；失焦清理可能无法完成，但不会杀死输入法进程。清理代码多了一层统一的尽力而为语义，后续新增 `Sink` 操作必须继续在 `EditorBridge` 边界内执行。

# Agent Note: Android 编辑器无法读取光标时仍执行快速清除

Status: implemented

## Problem

Nightly Android API 35 的 `QuickDeleteUndoDeviceSmoke` 在合成编辑器拒绝提供光标位置时失败：快速清除只完成了按下删除键的一次普通删除，光标前剩余文字没有清除。该场景对应 #11313 跟踪的 nightly 红灯，也覆盖真实编辑器可能只支持删除范围、暂不支持上下文读取的能力差异。

## Decision

快速清除逐段读取光标前文时，如果编辑器完全没有返回可读前文，改用 `deleteSurroundingText(Integer.MAX_VALUE, 0)` 请求编辑器删除光标前全部内容。某些编辑器在已经删空后最后一次读取也会返回 `null`，因此只有本轮尚未成功删除任何文字时才走回退，避免丢掉正常路径的撤回备份。这个回退不读取或猜测被删文字，因此不创建撤回备份；光标后的内容仍由 `afterLength = 0` 保护。

## Alternatives considered

- 继续在读取失败时停下：保留了数据安全，但快速清除的明确操作只完成一次普通退格，和用户看到的“删掉全部前文”不符，也重现了 nightly 失败。
- 发送一串物理退格键：依赖编辑器处理按键事件，可能触发输入法自身组字、统计或快捷键，且无法保证一次操作的边界。

## Verification

- 远端 `:app:compileFullDebugJavaWithJavac`、`platforms/android/check-host.sh` 通过：354 条 Android JVM 冒烟、288 个设备套件源码编译。
- API 28 `QuickDeleteUndoDeviceSmoke` 通过，包含快速清除、下滑撤回、滑回取消、短按和编辑器边界。
- API 35 同一套件已走完快速清除、不可读位置和撤回阶段；随后在既有普通输入回归阶段因合成控件未出现而失败，失败不在本次改动路径，需 nightly/设备 job 继续确认。
- 本机磁盘不足以启动新的 Android 构建树，构建均在远端 Mac Studio 完成。

## Consequences

编辑器支持删除范围但不提供上下文时，快速清除会完成用户请求，但这次操作没有撤回入口，因为产品无法安全保存被删正文。正常能读到前文的编辑器继续保留既有撤回；清空后的终止性 `null` 读取不会误伤该备份。API 35 普通输入阶段的既有设备失败仍需单独追踪，不由本修复掩盖。

## Follow-ups

- 合并前保留 #11313 的 nightly 跟踪，确认 API 28/35 均通过后再关闭红灯。

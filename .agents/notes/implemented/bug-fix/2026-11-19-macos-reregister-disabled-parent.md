# Agent Note: macOS 重登记补开已存在但关闭的顶层输入源

Status: implemented

## Problem

macOS 输入法的重装和「修复输入源」会先用 `TISCreateInputSourceList` 判断 bundle 是否已经登记。旧实现只要查到任意可启用输入源就直接返回成功，没有读取顶层 bundle 输入源的 `kTISPropertyInputSourceIsEnabled`。重复安装、安装多个副本或系统状态恢复后，顶层输入源可能仍在注册表里却处于关闭状态，命令报告成功而输入法不会出现在可切换输入源中。

## Decision

`MSIMERegisterAndEnableInputSources` 在已登记路径继续保留所有模式的用户选择，不自动恢复被移除的模式。它额外查找输入源 ID 等于 bundle identifier 的顶层输入源，只有该源存在且明确关闭时调用 `TISEnableInputSource`。启用失败继续向调用方返回系统错误，顶层源已经开启或不存在时保持原有重登记结果。

回归测试把重登记夹具中的顶层源状态显式设为已开启，覆盖关闭顶层源时补开以及补开失败两条路径。英文系统的模式名称已经使用 `Metasequoia …`，本修复不改动已有的本地化命名决定。

## Alternatives considered

- **重登记时重新启用顶层源和主模式**：最强的理由是能一次恢复完整输入法。放弃它，因为用户可能在系统设置中移除主模式或其他模式，重登记不应悄悄改回用户选择。
- **无论状态如何都调用 `TISEnableInputSource`**：最强的理由是实现最短，也能覆盖系统返回状态不可靠的情况。放弃它，因为每次设置应用启动都会触发系统调用，可能重新加入用户主动关闭的源，也无法区分顶层源和可选模式。

## Consequences

- 重复安装后顶层输入源被关闭时，修复命令会真正恢复可用状态并保留可选模式的启用选择。
- macOS 27 对键盘输入模式本身的系统限制仍然存在，模式名称和用户手动添加流程不变。
- 没有可用的 HarmonyOS SDK 或签名设备，因此本次只验证 macOS 注册逻辑与仓库契约。

# Agent Note: Android 设备验收共用命令与输入法重启

Status: implemented

## Problem

18 个 Android 设备验收类重复实现不读取命令结果的 shell 调用，另有 12 个类分别以 `restartIme` 或 `rebindInputMethod` 重复执行相同的输入法禁用、启用、选中和等待步骤。改动设备准备顺序时，这些副本容易分叉。

## Decision

`DeviceSmoke.shellDiscard` 统一执行命令并排空输出管道；`DeviceSmoke.restartIme` 用它完成固定的三条命令和一秒等待。两个方法均为 `protected final`。原来返回命令输出的 `SettingsLifecycleSmoke.shell` 保持独立，因为它的返回值是测试断言所需数据。

## Alternatives considered

让共同方法既支持丢弃输出又支持返回字符串，可以把所有 shell 调用收进一个入口。但丢弃和收集的用途不同，返回值接口会迫使大多数调用者处理无用文本，也容易误用系统输出；这里保留两个明确的语义。

## Consequences

设备验收类删除重复实现并继承共同方法，命令内容、顺序、输出读取和等待时长保持不变。共同基类增加两个测试辅助方法；今后需要检查命令输出的测试仍应使用返回字符串的独立方法。Android Java 编译覆盖所有继承调用点，本次不改产品宿主代码。

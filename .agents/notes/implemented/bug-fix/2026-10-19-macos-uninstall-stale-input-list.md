# Agent Note: macOS 卸载优先读取当前第三方输入源列表

Status: implemented

## Problem

macOS 当前版本把第三方输入法记录在 `com.apple.inputsources` 的 `AppleEnabledThirdPartyInputSources`，旧版本则使用 `com.apple.HIToolbox` 的 `AppleEnabledInputSources`。卸载命令原来把两份列表中的“输入法总条目”用 OR 合并。用户在系统设置中删除输入法后，旧 HIToolbox 列表仍可能留下旧条目；卸载检查因此持续返回 `input_source_listed`，每次确认都重新打开系统设置，无法把 bundle 移到废纸篓（#8710）。

## Decision

`input_method_listed` 按列表的新旧顺序读取，优先采用现代第三方列表的第一个可读结果；只有现代列表缺失或不可读时才回退到旧 HIToolbox 列表。现代列表明确返回 `false` 时不再被旧列表中的残留 `true` 覆盖。安装启动检查使用的 `input_source_enabled` 仍保留跨版本的 OR 语义，因为它需要兼容旧版 macOS 的输入源记录方式。

回归测试固定“当前列表为 `false`、旧列表为 `true`”的情形，并覆盖现代列表不可读时的回退。

## Alternatives considered

- **继续把两份列表 OR 合并**：最强的理由是同时兼容新旧 macOS，且已有实现很短。没有采用：旧列表是历史残留时会把已经从系统设置移除的输入法误判为仍在列表，正是 #8710 的循环原因。
- **只读取现代第三方列表**：最强的理由是当前 macOS 的真实来源明确，能彻底排除旧列表残留。没有采用：旧版 macOS 仍可能只维护 HIToolbox 列表，直接删除回退会让旧系统无法卸载。
- **删除 bundle 后再让系统设置清理列表**：最强的理由是用户不必先手动移除输入源。没有采用：macOS 27 不允许输入法进程修改启用列表，bundle 消失后残留条目反而无法在系统设置中显示和删除。

## Consequences

在当前 macOS 上，用户从系统设置移除输入法后，旧列表的残留不会再阻塞卸载；在旧版系统上，现代列表不可读时仍沿用 HIToolbox 回退。新增一个很小的“优先第一个可读列表”判定函数，安装状态探测的兼容语义保持不变。

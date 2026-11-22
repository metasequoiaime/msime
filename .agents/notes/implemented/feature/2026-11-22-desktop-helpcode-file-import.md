# Agent Note: 桌面插件文件导入支持单个辅助码表

Status: implemented

## Problem

[#11566](https://github.com/metasequoiaime/msime/issues/11566) 要求能在 macOS 和 Windows 导入用户自己的辅助码表。桌面已有辅助码插件和方案选择，设置页的文件选择器只收 ZIP，单个文本要先手工打包或复制到状态目录。

## Decision

桌面「插件」页现有的文件导入接受 `.txt` 辅助码表。从选择器取得普通文件路径后，`plugins::import_from_settings` 限制读取至多 1 MiB 原始字节，复用移动端的 UTF-8、重复字、码长和行号校验，再把表安装为一个辅助码插件。导入后从插件卡片或对应方案的辅助码列表选择使用。全拼和双拼仍是两个独立的选择，导入成功本身不写偏好。

macOS 和 Linux 由现有系统文件选择器显示 TXT，Windows 的 WinUI 与 Win7 原生插件页也使用同一入口。ZIP、目录、脚本授权和手工安装的 `custom/<stem>` 仍按原契约处理。

## Alternatives considered

- **给每个桌面方案组单独加导入按钮。** 能少点一步，并能直接把码表启用到正确的方案，但需要在各个宿主复制选择文件的流程。当前已有插件安装与方案选择，复用插件文件选择器，也无需给共享 React 设置添加入口。
- **按手工 `custom/` 格式复制。** 能继续读桌面的宽松表格，但会让相同表格在桌面和手机上有不同校验结果。插件导入还提供显示名、删除入口、失败时保留上一版和原子换入。

## Consequences

- 同名文本按原有的 `custom-` id 规则整包替换。坏表只返回错误，已安装的表仍可用。
- [移动端单文件导入](2026-10-12-mobile-helpcode-table-import.md) 拥有码表解析和安装规则。[Windows 原生设置选择辅助码插件](2026-10-11-windows-settings-helpcode-packs.md) 拥有方案选择。[状态目录的 `custom/` 表](../bug-fix/2026-11-03-custom-helpcode-tables-in-state-root.md) 继续覆盖既有手工安装。
- #11566 的快捷短语优先级是候选排序改动，仍待实现。

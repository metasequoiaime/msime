# Agent Note: Windows 输入习惯页补齐共享候选偏好

Status: implemented

## Problem

Windows 的 WinUI「输入习惯」页已经从 host-api 读取 `mixed_input.japanese`、`single_character_only` 和 `sentence_association.word_lattice`，但页面没有绘制这三项。用户因此只能通过共享设置或其他平台改动它们，Windows 上既看不到当前状态，也无法恢复默认值。缺少的三项原本被精简设置页的跟进记录为已知缺口。

## Decision

在 Windows 原生「候选与联想」组的「更多选项」中补上三行，并沿用共享设置的标题、说明和默认值：

- 带临时日语能力的版本显示「日文混输（实验）」，绑定 `mixed_input.japanese`，缺省为关；不带该能力的版本不绘制此行。
- 「只出单字」绑定 `single_character_only`，缺省为关。
- 「本地整句联想」绑定 `sentence_association.word_lattice`，缺省为开。

搜索命中这三行时与已有的符号、颜文字混输一样自动展开「更多选项」。新增的源码契约检查锁定标题和偏好键，防止原生页面再次漏掉共享候选偏好。

## Alternatives considered

- **把三项放回可见的一级行**：这样更容易发现，但它们与符号、颜文字混输一样是少有人改的候选来源开关；放入现有折叠区能保持 Windows 与共享页的分组层级一致，也不改变默认布局。
- **无条件显示日文混输**：可以避免按版本分支，但没有临时日语词典的版本由 host-api 固定关闭该偏好，展示一个永远不可用的开关会误导用户。因此沿用临时日语快捷模式的版本能力位守门。
- **在 Windows 设置层复制偏好校验或业务逻辑**：原生页只负责读写字段，默认值和能力门控仍由共享 Rust/host-api 决定，避免两套规则漂移。

## Consequences

Windows 用户现在可以在「输入 › 候选与联想 › 更多选项」查看和修改三项已有能力（这一组原来在「输入习惯」，后来归回「输入」页的「候选与联想」标签，见[打字三页照 macOS 分标签](../feature/2026-10-20-windows-settings-typing-mac-parity.md)）；搜索这些标题会定位到并展开对应折叠区。带临时日语的版本可直接开启日文混输，其他版本不会出现无效开关。新增检查只读取源码，不要求 Windows 运行时；WinUI 的实际编译和 UI Automation 仍由 Windows CI 验证。

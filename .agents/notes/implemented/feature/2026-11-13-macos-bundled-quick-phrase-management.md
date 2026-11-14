# Agent Note: macOS 设置管理内置快捷短语

Status: implemented

## Problem

`quick_parases` 随词库发布的示例短语会出现在 macOS 设置的快捷短语列表中，但共享列表把所有 `source: bundled` 的行都隐藏了编辑和删除操作。Engine 与 host-api 已经支持内置行只改权重或删除，并把操作写入 journal，词库更新后仍能重放，因此缺口只在设置界面。

## Decision

macOS 设置允许内置快捷短语显示「调权重」和「删除」。编辑表单继续把编码和短语设为只读，只提交新的权重；删除沿用现有确认框和 `dictionary.edit(previous, null, request_id)`。其他内置词库以及 Windows、Linux 的共享设置界面继续保持只读，避免把已有的平台约定一起改变。

`DictionaryEntries` 通过显式的 `allowBundledActions` 能力接收这个平台差异，页面只在 `client.host.platform === "macos"` 时打开它，并且列表再次限定 `kind === "quick_phrase"`。宿主仍在 Rust 层校验来源、身份和权重范围，界面不能绕过这些限制。这里局部取代了[桌面词库对齐笔记](2026-10-20-windows-settings-lexicon-mac-parity.md)中对内置行一律隐藏操作的约定，Windows 侧原规则保持不变。

## Alternatives considered

- **修改共享列表，让所有平台的所有内置词条都能操作**：实现最小，但会推翻 Windows 对齐笔记中“内置词条在界面上不能改权重或删除”的明确决定，也会让拼音、五笔的大型基础词库暴露破坏性操作，因此不采用。
- **只新增用户短语覆盖内置短语**：可以保留发布词库，但需要新的覆盖数据结构和候选合并规则；现有 journal 已能安全删除或调权，覆盖机制超出本 issue 的必要范围，因此不采用。

## Consequences

- macOS 用户可以隐藏无意义的随包示例，或调低其权重使自定义短语优先；操作会随词库代次更新保留。
- 编码和短语本身仍不能改写，避免把发布行伪装成个人词条；要使用不同内容仍应新增个人短语。
- 共享组件新增一个显式平台开关，未来若其他宿主要开放同一能力，必须单独确认产品范围并补充对应验收。

## Verification

- 定向 Vitest 覆盖默认宿主仍隐藏内置操作，以及 macOS 显示「调权重」「删除」并将编码、短语输入框设为只读。
- 运行 `pnpm run verify-notes`、相关设置测试、TypeScript 类型检查和格式检查。

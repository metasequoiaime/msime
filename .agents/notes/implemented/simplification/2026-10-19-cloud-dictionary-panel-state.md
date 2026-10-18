# Agent Note: 云词库面板状态与交互集中到公共 hook

Status: implemented

## Problem

`CloudDictionaryPanel` 把列表查询、表单、修改后刷新和异步确认写在 JSX 同一函数中，其他共享呈现无法单独复用状态。确认等待期间更换客户端后，旧删除和单词条下载仍会调用旧客户端；真实面板的两个回归用例已复现。

## Decision

提供 `useCloudDictionaryPanel(client, embedded)` 和派生的 `CloudDictionaryPanelState`。保留现有状态、查询参数、同步互斥、错误文案及初始化时机：独立面板延迟 0，嵌入模式延迟 280 ms，cleanup 清 timer 并作废代次。面板继续拥有 DOM、主题和宿主可选能力展示。

删除和下载确认在弹窗前捕获 `usePanelAction` 的代次，回答后检查 `isCurrent`，过期确认不得启动宿主调用。已有请求的结果仍由原代次守卫处理。

## Existing notes audit

按 `CloudDictionaryPanel`、云词库面板及确认生命周期检索活跃笔记。

- [桌面客户端装配](../../implemented/simplification/2026-10-18-desktop-cloud-dictionary-client.md)部分重叠，继续持有平台能力装配，本项只持有共享呈现状态。
- [背单词确认归属](../../implemented/bug-fix/2026-10-15-vocabulary-action-owner.md)部分重叠，借鉴确认前捕获代次原则，组件与请求槽位独立。
- [双拼编辑器代次](../../implemented/simplification/2026-10-14-shuangpin-custom-editor-generation.md)部分重叠，生命周期原则相同，草稿校验不属于本项。
- 图标客户端 pending 修复、SCEL 导入、词库顶部标签、Windows 云账号、macOS 云账号回退与 Rust 自动云同步提案分别持有其他状态或宿主协议，不改变它们。

## Alternatives considered

- 保留所有逻辑在组件内，改动最少且不增加模块，但无法单独复用和验证面板状态。
- 新增客户端 generation hook 可专门识别客户端更换，但现有操作代次已经在客户端更换、查询失效及卸载时作废，复用它可以同时拒绝被新操作取代的旧确认。

## Verification

抽取前相关基线 78 项通过；真实面板的旧客户端删除与下载两个回归在修复前均失败。新 hook 首次运行 6 项均因缺少公开 hook 失败。抽取后相关套件 75 项、设置页云词库消费者 11 项通过，补充正常删除确认与取消后最终聚焦 10 项通过，合计覆盖 88 项。独立只读审查核对 AST 与 JSX 等价，并独立运行 8 项通过；审查发现的旧类型转导出误删已恢复。

桌面类型检查与生产构建、HarmonyOS 类型检查、改动文件 lint 和格式检查、笔记校验、差异检查通过。

`bash scripts/verify-local.sh --quick` 输出 `quick check passed`，覆盖仓库契约、Rust workspace、Android 目标、Linux 设置外壳及共享 Apple bridge 编译；HarmonyOS 原生构建缺少 staged 依赖和 hvigorw、Windows 缺少 MinGW，其余原生宿主按范围跳过。

## Consequences

呈现与状态分离，列表及交互可供其他共享视图直接复用；代价是增加一个公共 hook 与派生类型。确认代次守卫有意拒绝被其他操作或嵌入查询作废的旧确认。已发出的宿主操作不会被取消，只丢弃失效结果。没有修改宿主协议或文件读写，jsdom 验证不代表原生设备验收。

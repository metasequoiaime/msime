# Agent Note: Windows 粤拼简体输出

Status: implemented

## Problem

`#10435` 的共享 Engine 已支持 `cantonese_simplified_output`，但 Windows 仍把简繁按钮固定绑定到 `traditional_chinese_output`。Windows 的粤拼词库是繁体，按钮在粤拼下没有可达的简体输出选项；设置页还会误报该开关不影响粤拼。此前的总决策记录[粤拼和粤语双拼输出简体](../feature/2026-11-02-cantonese-simplified-output.md)明确把 Windows 留作后续独立 PR。

## Decision

Windows 在粤拼（包括粤语双拼）下让已有的「中文输出」分段控件和工具栏简繁按钮操作 `cantonese_simplified_output`。这个字段为真表示简体，因此工具栏状态和设置页显示值都取反；其他方案继续读写 `traditional_chinese_output`。切换方案时，托盘、悬浮工具栏和设置页按当前实际运行方案重新计算显示状态，缺字段按既有默认值处理：粤拼默认繁体，其他方案默认简体。

宿主只负责读写偏好并把按钮状态映射到共享字段，粤拼繁转简仍由 host-api 注入 Engine 的转换完成；Windows 不在 `ReplyComposer` 里再做一次转换。

## Alternatives considered

- **让粤拼继续跟随 `traditional_chinese_output`**：改动最小，但默认值是简体，会把已有粤拼用户升级后的出厂行为改成简体，也无法和移动端已经落地的独立字段保持一致。
- **在 Windows `ReplyComposer` 里单独做繁转简**：可绕过偏好重建，但会与 Engine 的候选去重和其他宿主结果分叉；现有 host-api 注入已经提供单一转换入口，因此不采用。
- **只改新 WinUI 设置页**：旧版 Win7 设置仍可写错字段，Windows 的两个设置入口会给出不同结果；本次同时接入两套设置页和运行时按钮。

## Consequences

- Windows 粤拼用户可以从现有「中文输出」控件、悬浮工具栏和托盘按钮切换简体/繁体，默认仍为繁体。
- 全拼、双拼、五笔等方案的 `traditional_chinese_output` 行为不变；注音和笔画仍保持原有原生字形语义。
- 新字段只在用户选择粤拼简体时写入，旧版本读取未知字段时保持兼容；真实 Windows 桌面和安装包验收仍需在 Windows 环境完成。

## Verification

- 新增 `windows-character-set-output` 行为测试，覆盖粤拼与普通方案的字段选择、存储值和显示状态映射。
- 更新 Windows 新设置页与 Win7 设置页的现有模型断言，覆盖粤拼和注音说明。
- 待远端 Windows 原生 CI 编译、Windows 双设置页测试及真实 Windows 宿主验收后，才可把 `#10435` 的 Windows 部分写成已验证。

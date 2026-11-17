# Agent Note: Windows 发布默认只签署完整版

Status: implemented

## Problem

Windows 0.5.0 六版共用签名缓存后仍新增 86 个文件签名，其中 full 为 26 个，其余五版各新增 12 个、复用 14 个。维护者先要求不能共用时只发 full，又在看到实际消耗后明确反对每次发布消耗 86 次；部分文件可共用不足以证明六版的剩余成本可接受。这些数字来自该次构建的文件签名计数，不代表 Certum 账户本月账单或合同额度。

## Decision

手动 Release Windows 的 `editions` 默认 `full`，只有显式选择 `all` 才构建和签署全部独立版本。可复用工作流没有指定范围时，发布构建默认 full，非发布构建保持 all。`edition_windows.py editions --selection` 从版本表生成 CSV 和 JSON；四个构建、签名及安装矩阵共用 source 输出，发布附件、下载指引和符号包也使用同一范围。非法范围在 source 阶段失败。

full 保留全部包内签名、外层安装器签名以及 x64、ARM64 安装卸载门禁；共存门禁只在选择多个版本时执行。不减少文件签名覆盖来降低计数，不重新签署已经完成的 0.5.0 产物。未来签名数随二进制内容及文件数量变化，26 只是本次 full 的实测基线。

## 既有笔记归属

- [SimplySign 自动发布与签名复用](2026-11-17-windows-simplysign-release.md)：部分取代默认六版的范围决定，保留缓存、凭据边界及完整验证。
- [SignPath CI 签名](2026-10-08-windows-signpath-ci-signing.md)：签名来源和审核约束不变，选择范围也适用于该分支。
- [Windows 发布页与符号包](2026-10-20-windows-release-page-symbols.md)：部分重叠，附件仍严格对应本次所选版本，符号包仍在独立滚动发布中。
- 检索到的其他平台签名、六平台构建和输入行为笔记不改变本次 Windows 发布范围，无决定重叠。

## Alternatives considered

- 默认六版并只做内容缓存：保留全部独立版本的自动分发，且本次已减少 70 次重复签名；但六版仍消耗 86 次，违反维护者降低常规发布成本的要求，因此改为显式选择。
- full 省去第三方 DLL 签名：还能少签部分文件，但会改变现有安装包的签名覆盖和部署信任条件；本次保持覆盖，通过减少默认版本数降低成本。

## Verification

`scripts/test-windows-editions.py` 执行范围选择 CLI，覆盖默认全部、显式 all、full、CSV、JSON 与非法输入。`scripts/test-windows-release-page.py` 执行工作流中的实际发布脚本，覆盖全量与 full 附件、下载说明、摘要一致性，并确认 full 范围出现额外版本产物时拒绝发布。`actionlint` 校验动态矩阵和作业依赖。

## Consequences

按本次基线，常规发布由 86 降为 26 个新增文件签名；独立版本不再随默认操作更新，需要维护者显式选 all。签名缓存仍用于同一运行的重试和全版本发布。安装门禁失败仍阻止发布，账户月累计及实际计费必须另向供应商账户核实，不能从本工作流推算。

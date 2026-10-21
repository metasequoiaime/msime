# Agent Note: Windows 设置验收等待页面名称就绪

Status: implemented

## Problem

PR #9111 的 Windows 原生设置验收在深色模式账号页找到 `settings-page-account` 后，立即读到空的 `Current.Name` 并失败；同一提交重跑通过。`Open-Page` 只等节点存在，页首断言又重新查节点，不能保证名称已经可读。随后合入的 PR #9118 给主循环补了节点重查，但仍只等存在、不保证名称可读；统一由名称等待覆盖它。用真实 `Assert-PageIntro` 和空、空白、正确文案的查询序列可稳定复现过早失败。

## Decision

页面导航和页首断言共用名称等待：每次重新查询正文和页首节点，等非空白名称后返回当次字符串快照，最多 15 秒。节点在查询过程中失效时重新查询。读取到非空名称后仍按原断言核对句末、账号文案、当前标签及关于页产品名，不把错误文案当成就绪失败重试。发现设置加载错误时立即失败。

## Alternatives considered

- 增加固定延时最容易修改，但 runner 负载波动时仍可能太早，正常运行也会白等。
- 只在 `Open-Page` 等名称即可修最早的导航窗口，但页首断言重新查节点时仍可能赶上页面重建，因此所有读取页首文案的位置复用等待并只使用一次字符串快照。

## Consequences

正常运行只等到名称可读为止，错误文案继续由原来的精确断言拒绝；永久缺少名称最多等 15 秒。合成查询只能验证等待与断言流程，不能代替真实 WinUI / UIA；真实界面仍由 PR 的 Windows 原生设置 CI 验收。UIA 节点失效是平台边界，[微软文档](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.elementnotavailableexception)明确该异常表示对应界面已不可用，等待只捕获该异常。

## Verification

`python3 scripts/test-windows-settings-page-readiness.py` 从真实验收脚本的 PowerShell AST 加载等待和断言，只替换 UIA 查询边界，不执行脚本顶层的 WinUI 启动。旧实现遇到第一个空名称就失败；修复后节点暂缺、空名称、空白名称再到正确文案时通过，节点失效后重新查询也通过。错误非空文案及设置加载错误仍立即失败，永久为空会在 15 秒超时。无 `pwsh` 的环境明确跳过，不把合成回归视为完整原生界面验收。

## Related notes

检索活跃笔记的页首、UIA、设置验收和等待：已有[页首引导](../../implemented/feature/2026-10-20-windows-settings-mascot-intro.md)持有文案及自动化 ID 契约，保持不变；[画布像素就绪](../../implemented/testing/2026-10-20-windows-canvas-pixel-readiness.md)处理屏幕绘制，部分重叠但完成条件不同，保留。各设置页迁移笔记只定义页面内容，不持有此次等待规则；没有同类过时提案。

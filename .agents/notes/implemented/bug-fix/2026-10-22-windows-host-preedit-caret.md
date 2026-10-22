# Agent Note: Windows 预编辑文字与光标共用宿主快照

Status: implemented

## Problem

#9158 报告回车上屏英文后立即续打拼音时，光标停在上一段文字后。当前 `_HandleCompositionInputWorker` 从宿主视图绘制文字，却从没有随宿主字符入口更新的旧键击缓冲计算光标；即使宿主光标在末尾，空旧缓冲也会把 TSF 选区放到组字开头。这个状态来源缺口可以独立验证，但还不能据此认定它解释了 Grok Bot 的快慢输入差异。

## Decision

已有按键快照的视图转换由 `KeyCompositionStateFromView` 持有，绘制时从同一次 `host->view()` 的结果取得文字、原文与光标。没有宿主会话时保留旧缓冲；有效宿主视图读取失败则返回失败，不绘制另一份旧组字。分词符映射、已选词前缀、隐藏预编辑、锁定光标方案和整句改字的优先级保持现有规则。

## Historical audit

检索 `MapRawCaretToPreedit`、`GetRenderedCaretPosition`、宿主光标及快照。已有[邮箱与网址补全](../../implemented/feature/2026-10-15-engine-email-url-completions.md)持有按键分类快照，本次复用它的转换规则，绘制入口单独归本笔记；[整句改字](../../implemented/feature/2026-10-10-sentence-conversion-edit.md)的转换文字与焦点偏移继续优先；[组字光标](../../implemented/feature/2026-10-08-composition-caret-tap.md)持有引擎编辑语义，不修改。macOS 标记文字的笔记只涉及另一宿主，无需迁移。

## Alternatives considered

- 直接调用 `GetKeyCompositionState()` 最少改动并复用现有接口；但它会再次查询宿主，文字与光标无法由同一次读取保证一致，也增加按键路径上的查询。
- 每个宿主按键后同步旧缓冲能让原调用继续工作；但引入第二份组字镜像，选词、取消与部分提交都必须同步，与既有状态归属相冲突。

## Verification

`scripts/test-windows-preedit-caret.py` 从生产源码提取并编译完整 `_HandleCompositionInputWorker`，通过合成宿主视图与 TSF 文本范围记录文字写入及 `SetSelection` 的位置；旧源码在空旧缓冲的第一个续打字母上失败，修复后通过。覆盖残留旧缓冲、组字中间和越界光标、分词符、中文与扩展字词段前缀、仅含前缀的组字、日语读音与词中字母、隐藏样式、锁定光标方案、整句改字、旧入口、视图读取或解析失败及写入失败。每次绘制只查询一次宿主。测试模型不替代真实 TSF、Windows 编辑器或 Grok Bot 快速续打验收。

Windows 11 的 MSVC 2022 原生编译通过，包含 Rust Host API、实际 TSF DLL 与三项相关测试目标；`msime-tsf-preedit-caret`、`msime-tsf-host-composition` 和 `msime-tsf-key-composition-state` 的 CTest 全部通过。完整应用按键回放 183 项通过；远端 `bash scripts/verify-local.sh --quick` 通过，其中当前宿主可运行的 Windows 测试为 164 项通过、2 项因宿主差异排除、76 项需要 Windows 构建。笔记校验与差异空白检查通过。原生构建使用已有 vcpkg 工具链，未修改依赖或构建配置。

## Consequences

预编辑绘制不再依赖另一条输入入口的旧缓冲，也不引入需要同步的第二份组字状态；代价是每次绘制保存当前原文的 UTF-16 快照。生产方法提取测试依赖方法边界，源码结构变化时必须同步测试。Windows 测试机只有服务会话，缺少可交互桌面；原生构建与合成模型分别报告，不将模型执行当成现场复现。

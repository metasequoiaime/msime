# Agent Note: Windows 无法切分的全拼回车后恢复组字

Status: implemented

## Problem

Windows 输入 `wi` 这类无法切分成音节的全拼后，旧版本曾出现输入法不再响应后续按键、必须切换输入法才能恢复的报告（#11035）。引擎对无候选的全拼会保留原文字母；Windows 宿主的回车提交路径必须使用同一段原文完成提交并清空会话。

## Decision

在 Engine 会话回归中固定以下契约：无效全拼仍显示为可编辑原文；`CommitRaw` 提交原文并清空编辑状态；下一次输入可以重新组字。Windows `session_smoke` 通过真实 `ServerSession` 和 `ReplyComposer` 补同一条宿主路径，使用合成的 `wi`、回车和 `ni` 数据验证原文提交、会话清空及后续恢复。产品逻辑没有为单个字母串增加特殊分支，避免把无效拼音变成不可编辑状态。

#11544 暴露了此前只测 Engine/Server 的覆盖缺口：`CCompositionProcessorEngine::IsVirtualKeyNeed` 在 `CANDIDATE_NONE` 且仍组字时，把回车分派到 `CATEGORY_COMPOSING / FUNCTION_FINALIZE_CANDIDATELIST`，处理器只尝试选中候选再取消组字；空列表没有文字可写。这个入口现在与 `CANDIDATE_INCREMENTAL` 一样，分派到 `CATEGORY_CANDIDATE / FUNCTION_FINALIZE_CANDIDATELISTForVKReturn`，由现有宿主原文提交处理器提交并清空会话。显式转换列表在此前已将 `fComposing` 置为假，不经过本分支，其回车选词保持原规则；无组字回车、空格转换及其他功能键不改。

#11520 暴露了同一状态在候选窗口上的另一处缺口：宿主会话有效时，`GetCandidateList` 在候选数组为空后直接返回，旧的本地路径原本会放入原文回退项。现在普通候选方案在有可编辑原文且没有候选时继续生成一条无 Engine 身份的原文回退项；韩文、注音、越南文和藏文仍只画行内组字，不凭空打开候选窗。这样 `win` 仍可原样提交，同时候选窗口不会因空列表消失，后续按键也能持续刷新同一窗口。

## Existing notes audit

按 `wi`、无效全拼、`CommitRaw`、原文回车和宿主会话检索活跃笔记。已有全拼候选与原文提交笔记分别持有 Engine 命令和 Windows Enter 观测的通用契约；本篇只补充无法切分全拼在真实 Windows 会话中的端到端回归，不改变其它 scheme 的提交规则。

## Alternatives considered

- **在共用的 `_HandleCompositionFinalize` 中为缺候选增加原文回退**：能让多个调用方得到容错；但这个处理器还服务继续输入、结束文本存储等路径，扩大它的提交语义会影响其他键的取消或转换边界。本次只修回车的分派，复用已有原文处理器。
- **在 Windows 宿主里为 `wi` 增加专门恢复分支**：可以针对报告字符串绕过现象，但会让宿主和 Engine 的编辑状态分叉，也无法覆盖其他无法切分的字母串，因此不采用。
- **只增加 Engine 单测**：能证明核心命令可恢复，却不能验证 Windows `ReplyComposer` 的原文提交和确认送达路径，因此同时保留宿主回归。
- **让空候选直接结束组字并把字母交给应用**：可以让下一次输入重新出现候选窗，但会丢失微软拼音保留原文、回车再提交的行为；因此保留组字，只补回候选窗口里的原文回退项。

## Verification

- `rbuild cargo test -p msime-engine session::tests::invalid_pinyin::invalid_quanpin_input_stays_editable_and_commits_raw -- --exact --nocapture` 通过。
- `rbuild bash scripts/verify-local.sh --quick` 通过；其中 Windows 本机可移植测试 191 项通过，Rust workspace、Android host Java、共享 Apple bridge、Windows contracts 和 notes 校验通过。Windows MinGW 原生编译在远端环境未配置，交给 PR 的 Windows 构建 job 编译并运行 `windows-session-smoke`。
- 未改变界面、资源或文件读写，不需要截图或设备验收。

- #11544 的 `scripts/test-windows-empty-candidate-enter.py` 编译生产功能键分类分支和真实枚举：修前 20 项检查只有空候选回车失败，修后全部通过。覆盖空/非空候选的回车、只探测不接结果、无组字、退格、Delete、Esc、空格、Tab/Shift+Tab 和左右键原规则。这是生产分派代码回归，不等同完整 TSF/QQ/报告者编辑器的运行证据；既有 Engine/Server 回归继续保留。
- `g++ -std=c++17 -Wall -Wextra -Werror -DMSIME_EDITION_FULL .../host_candidate_fallback.cpp` 编译并运行通过，覆盖无候选原文回退、有候选时不回退、行内方案和本地模式边界。

## Consequences

当前实现继续由 Engine 的通用原文提交规则处理无效全拼；Windows TSF 候选镜像也为普通方案保留原文回退项，新增回归会在宿主路径被改坏时阻止合入。无法在本机远端环境执行 Windows 真机/原生宿主安装验收，缺少的真实编辑器验证写进 PR 验证段，不把它当作未实现，也不改变已覆盖实现的 `Fixes` 关系。

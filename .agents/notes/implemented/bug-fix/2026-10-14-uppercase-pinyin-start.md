# Agent Note: Shift 大写字母可以开始拼音组字

Status: implemented

## Problem

#7653：中文输入法状态下输入 `Observable` 时，首个大写 `O` 被宿主直接交给应用，后续 `bservable` 才进入拼音组字，导致原始大小写被拆开，英文候选无法按完整单词联想。

#8776：组字原文已有完整大小写，但候选直接使用词库的显示拼写，词库里是 `observable` 时仍显示和提交小写。旧回归预先把词库存成 `Observable`，没有覆盖从输入恢复大小写的必要步骤。

## Decision

全拼和双拼会话在没有组字时也接收 Shift 产生的大写字母，并把它作为保留大小写的拼音原文起点。快捷模式入口仍先于拼音接受过滤运行，因此启用的 `Shift+U/T/K/E/M/J/Y/R/V/C` 不会被这一改动抢走；非 Shift 产生的空闲大写字母继续交还宿主。后续输入按原有大小写原文与小写查询键分离逻辑处理，英文混输查询使用规范化小写键，候选文字保留用户输入的大小写。

英文混输行仅在快照和上屏文本的副本中恢复实际输入的前缀拼写；两个及以上全大写字母把整个补全词转为大写，单个大写字母只恢复前缀，后续可继续输入首字母大写的词。小写输入保留词库自身的专名拼写。辅助码激活时大写尾字母仍属于辅助码，不参与英文词形恢复。内部候选保留词库原始的 `word` 和规范化的 `pinyin`，供调频、置顶和删除定位原词条；快照显示和选词上屏调用同一套词形恢复逻辑。

#8774 中没有启用的快捷入口也遵循同一接受规则：入口查找未命中后，所有 Shift 大写字母都能开始全拼或双拼组字，不再按入口字母表再次排除。模式开关、方案允许列表或资源能力拒绝了 `R/C/V` 时，按键不应落入既不能进模式又不能组字的空档。已启用的入口仍在接受过滤之前运行，总开关关闭和非 Shift 大写继续保持透传。

## Alternatives considered

- **只在 macOS 宿主拼接首个大写字母**：可以针对报告平台做补丁，但会让 Android、Windows、Linux 和共享引擎的实体键盘行为不同，也无法复用 Engine 已有的大小写原文与查询键分离逻辑。
- **切换到专用英文模式**：能得到英文联想，却丢失中文方案的拼音候选和用户原本的输入状态，且需要额外的模式切换，不符合“中文状态下直接输入”的复现步骤。
- **直接改英文词库或查询键为大写**：词库预存大写能让既有测试通过，但同一个词需要随每次输入变化大小写，且词库索引只接受规范化的小写查询键，因此在快照与上屏副本中恢复显示词形。
- **直接改内部候选的 `word`**：显示和提交可共用一份文字，但手动调频按规范化键和原始显示拼写定位词条，修改后无法持久化。手动调频回归在这一方案下复现诊断错误，因此保留内部词条身份。

## Consequences

Shift+大写字母现在可以成为拼音组字的首键，后续英文字符会在同一组字里参与英文混输，原始大小写可在预编辑和原样提交中保留。仅 Caps Lock 或宿主未标记 Shift 的空闲大写字符仍保持透传语义；快捷模式入口及其他方案不变。

接收大写与恢复英文词形的决定继续成立。普通大写仍参与中文查询、完整拼音 `Ni` 仍按「你」结束组字的旧边界由[大写字面片段](2026-10-19-uppercase-literal-pinyin-boundaries.md)取代：大写保留字面含义，小写片段独立转换，结束组字与明确选词使用同一套候选及原文消费范围。

## Verification

`cargo test -p msime-engine shifted_uppercase_starts_pinyin_and_mixes_the_cased_english_word` 覆盖 `Observable` 的首个大写、完整预编辑和英文候选；`cargo test -p msime-engine punctuation_after_` 覆盖不完整大写原文保留与完整拼音选词；macOS 原生 `shortcut` 测试经真实 `MSIMEInputController`、`MSIMEClientSession` 和文本客户端路径发送 Shift+`O` + `bservable`，确认组字区保持完整 `Observable` 且回车一次性提交；`cargo fmt --all` 已运行。

`mixed_english_completion_and_commit_keep_typed_case_without_changing_lookup_key` 覆盖首字母大写、全大写、混合前缀和词库专名拼写；`mixed_english_case_preserves_dictionary_identity_for_frequency_learning` 覆盖大写显示下的手动调频、词库原始显示字段与最终上屏，防止显示恢复破坏维护接口。

#8776 最终验证：引擎单元测试 2349 通过、49 忽略；golden 31 通过，包含 300 个合成场景。严格 all-targets clippy、格式、笔记校验和 quick 通过。quick 在本工作树的远程 macOS 镜像运行，覆盖 workspace、Android、Wasm、Java 宿主、Linux 共享壳与原生宿主（90 项测试）、macOS 原生与共享 Apple 桥接编译；远程缺 Node、HarmonyOS 暂存依赖及 MinGW 的阶段明确跳过。笔记在本机单独验证；指定旧真实词库未就绪，不把真实词库 golden 的跳过或宿主编译当作输入法 UI 验收。

`disabled_shift_shortcuts_leave_every_capital_available_for_pinyin` 遍历全拼、双拼的 26 个大写字母；旧实现首次在 `C` 失败，修复后各键的预编辑与原文提交保持大小写。拆字默认关闭的既有回归改为断言普通拼音组字，开启拆字与关闭总开关的用例继续约束原入口语义。[空入口拒绝大写的衔接](2026-10-19-bare-shortcut-capital-continuation.md)同时修复 `UV` 提前上屏，两部分共同覆盖 #8774；大写是否参与中文候选仍由 #8773 另行处理。

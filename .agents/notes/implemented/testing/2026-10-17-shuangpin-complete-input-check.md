# Agent Note: 双拼完整输入判定直接检查按键

Status: implemented

## Problem

`shuangpin::query::is_complete_input` 只需要回答输入能否按当前双拼表切成完整音节，却先为每个撇号分段构造 `pinyin_segmentation` 字符串，再交给 `is_all_complete_pinyin` 检查。ASCII 双拼输入的这条热路径因此会为只读判断申请临时字符串。双辅助码检测也会调用它，分配预算测试已经暴露了这项重复开销。

## Decision

对 ASCII 分段直接按两个字节一组调用已有的 `takes_two_keys`：长度必须为偶数，且每一对键都必须在当前 `ShuangpinProfile` 中形成有效双拼音节。入口继续先拒绝空输入、首尾撇号和连续撇号，再对每个非空分段独立检查。非 ASCII 分段保留原来的 `pinyin_segmentation` 与 `is_all_complete_pinyin` 路径，因为物化实现对 UTF-8 输入的行为是现有契约，当前不把 ASCII 快速路径的假设扩展到它。

`detect_active_double_helpcode_length` 的既有分配预算随完整性判定减少一次临时存储，从 2 次更新为 1 次；`helpcode_segmentation_reuses_the_case_output_buffer` 的预期从 8 次更新为 7 次。新增零分配回归测试，并用原物化实现对五种内置双拼表及合成输入逐项对照，保留大小写、分段和非法撇号边界的行为检查。

## Historical audit

按 `is_complete_input`、`pinyin_segmentation`、双拼完整输入和完整性判定检索活跃笔记，没有发现持有这项决定的既有笔记；命中的九键、双拼键面和其它输入性能笔记只涉及候选生成或宿主显示，均与本函数的切分判定独立。

## Alternatives considered

- **所有输入继续物化分段**：实现最简单，也能让所有字符共用一个检查器；但 ASCII 热路径必然保留临时字符串，无法满足零分配目标。
- **所有输入都按 UTF-8 字符直接配对**：可以统一代码；但会改变非 ASCII 输入与旧物化实现的边界行为，尤其是多字节字符和既有错误输入，不应在本切片顺带改契约。
- **把完整性判断改成调用完整查询流程**：可复用更多现有逻辑；但会引入查询状态和额外分配，且完整性判断只需要键位合法性，扩大了行为和性能风险。

## Verification

旧实现的回归测试确认 `is_complete_input("nihc")` 会申请 1 次临时分配；修改后同一调用为 0 次。双辅助码检测的预算从 2 次降为 1 次。新旧物化实现对五种内置双拼表和空串、短输入、大小写、撇号边界、非法键及非 ASCII 合成输入逐项相等。

`cargo test -p msime-engine --lib` 通过（2286 passed，40 ignored）；`cargo fmt --all -- --check`、`git diff --check` 和 `cargo clippy -p msime-engine --all-targets -- -D warnings` 通过。

## Consequences

ASCII 双拼完整性检查不再创建分段文本，调用它的双辅助码路径少一次临时分配；完整性结果、撇号边界和有效双拼键的接受规则保持不变。非 ASCII 输入仍走旧路径，后续若要统一它，必须先补齐与物化实现的行为矩阵和独立分配预算。新增对照测试会在键位表或分段规则变化时提示快速路径与旧语义分离。

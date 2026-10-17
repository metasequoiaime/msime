# Agent Note: 词网格插入位置借用拼音键

Status: implemented

## Problem

词网格合并在计算整句块插入位置时，`whole_sentence_insert_position` 每次都把输入音节用撇号拼成新的 `String`，随后只与词库行的 `canonical_pinyin` 比较。这个临时键不跨调用保存，也不参与输出；多音节候选刷新会为一次只读判定承担额外堆分配。

动手前检索了 `whole_sentence_insert_position`、整句插入位置、规范读音键和词网格合并笔记。现有笔记覆盖首位规则、候选去重、图边所有权和缓存行借用，没有覆盖插入位置比较键的存储方式；本片不改变这些排序与所有权决定。

## Decision

`whole_sentence_insert_position` 直接把 `canonical_pinyin.split('\'')` 与输入音节的借用迭代器按顺序比较。迭代器相等同时检查分段内容和分段数量，因此保留原先 `syllables.join("'")` 的完整键语义；空 canonical 仍走原有按字数或音节数覆盖的分支。比较期间不创建拼音键字符串。

## Alternatives considered

- **继续 `join` 后比较**：实现最简单，但每次整句合并都为一次只读判定申请临时字符串。
- **预先把连接键放进调用方状态**：可以跨比较复用，但会扩大 `merge_lattice_candidates` 的输入契约并引入键失效边界；当前判定只需借用现有音节切片。
- **按字节手写扫描比较**：可以避免迭代器适配，但会重复实现分隔符和尾段计数规则；标准 `split` 与 `Iterator::eq` 已提供无分配且完整的分段相等判断。

## Consequences

多音节整句插入位置的热判定不再分配临时键，候选顺序、整句块位置、空 canonical 回退和公开接口保持不变。比较会遍历 canonical 的分段并按输入音节比较；它仍然需要读取相同的字符串字节，不改变 SQL、图解码或候选行所有权。

## Verification

先只加入分配回归测试，旧实现按预期失败并报告一次临时键分配；加入借用分段比较后专项测试通过。`lattice::merge` 29 项、Engine 2275 项与 golden 31 项、Runtime 318 项通过；Engine Clippy `-D warnings`、fmt、diff check、笔记校验和 `bash scripts/verify-local.sh --quick` 通过。quick 中 Windows 宿主测试 157 项通过、2 项宿主差异排除，Linux Docker、MinGW、HarmonyOS 完整工程和 pipe-only 因环境缺失跳过。

# Agent Note: 双拼分段转换复用解码结果

Status: implemented

## Problem

`convert_seg_shuangpin_to_seg_complete_pinyin` 逐个双拼音节调用 `cvt_single_sp_to_pinyin`。每次调用都先构造一个拥有型 `String`，随后立即把内容复制进整句输出；长分段会重复申请和释放这些短字符串。

## Decision

双拼解码拆成两步：`decode_single_sp_to_pinyin` 返回借用输入初声与静态韵母的 `DecodedSyllable`，`append_decoded_syllable` 直接把这两个部分写进最终输出。零声母仍返回静态完整读音；双键读音的合法性仍用同一份 `accepted_syllables` 检查，并使用固定六字节栈缓冲组成校验视图。公开的 `cvt_single_sp_to_pinyin` 保留原有拥有型返回值，并按解码长度一次预留容量，因此单音节容量契约不变。

## Historical audit

已检索双拼分段转换、`cvt_single_sp_to_pinyin`、双拼解码和分配预算相关的活跃笔记。相邻的[直接辅助码字符借用](2026-10-17-shuangpin-direct-helpcode-character-borrow.md)与[直接辅助码路径编码](2026-10-17-shuangpin-direct-helpcode-path-code.md)分别管理候选字符和路径编码，不持有分段解码结果；没有发现重复决定或需要吸收的提案。

## Alternatives considered

- **继续逐音节调用 `cvt_single_sp_to_pinyin`**：改动最小，但每个音节都要申请临时字符串，再复制到整句结果。
- **让转换函数为每个音节复用一个临时 `String`**：可以减少申请次数，但仍需要把临时内容复制到最终输出，并且要处理零声母和不同长度音节的清空与容量边界。
- **预先构造整张双拼到全拼的字符串表**：能避免运行时解码，但会为每个方案常驻大量短字符串，且自定义方案会增加缓存生命周期和失效边界；借用解码结果已经覆盖热路径成本。

## Consequences

分段转换不再为每个已解码音节申请堆字符串，输出分隔符、大小写处理、零声母、`v` 在 `j/q/x/y` 后规范为 `u` 的规则及不可接受音节的空位行为保持不变。单独调用 `cvt_single_sp_to_pinyin` 仍返回独立字符串，并保留精确容量。依赖该路径的请求刷新预算随之下降：代表性双拼输入从 `5/6` 次降为 `3/4` 次，带辅助码的刷新从 `10` 次降为 `8` 次；这些断言继续锁住优化后的边界。

## Verification

TDD 分配回归在旧实现上测得长分段 `7` 次申请；直接写入最终输出后为 `1` 次，剩余一次是最终返回字符串本身。新增测试覆盖 `ni'hc'vs` 到 `ni'hao'zhong` 的语义和分配预算；同时保留单音节精确容量、所有内置方案解码和编码往返测试。合并前运行双拼定向测试、全引擎测试、Clippy、Rustfmt、差异检查、笔记校验和 quick 门禁。

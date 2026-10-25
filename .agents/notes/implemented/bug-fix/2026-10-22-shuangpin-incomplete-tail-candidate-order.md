# Agent Note: 双拼未完成尾音节保持较长候选优先

Status: implemented

## Problem

#9264：小鹤双拼输入完整词组后继续输入下一个音节的首键时，末尾直接辅助码路径会把这个裸字母当成单字辅助码。若该字母不匹配前缀候选中的任何汉字，短的整段回退行会排到完整前缀词组之前，候选看起来会随输入变长而变短。

同样的表象还有一条不依赖辅助码的路径：完整前缀若来自词网格组词而非单条词典记录，追加未完成音节后，整句解码因含不完整音节而跳过，而前缀循环只查询词典行，丢掉了刚才的组词结果。较短前缀的末音节补全于是成为首选。合成例子 `nihckejiq` 会从 `你好科技` 退到 `你好可`，继续打到下一个未完成音节也一样。

## Decision

在双拼单字辅助码查询中，若尾部辅助码没有匹配任何基础候选，把前缀候选和把尾字母按普通拼音解释的整段回退行合并后按实际按键覆盖长度排序，完整前缀行自然排在较短回退行之前。仍有辅助码匹配时保留原有顺序，避免改变真正的辅助码选词语义。

双拼系列查询在且仅在末音节不完整、前面音节全部完整时，对最长完整前缀复用 `generate_series`，把该前缀的组词行和词典行一起放进原有的最长优先前缀组。完整前缀不再满足这个条件，因此不会继续递归解码所有短前缀。整段匹配词条仍优先；词格仍只解完整音节，不猜尚未打完的尾音节。前缀使用原始输入中对应的按键范围与手动分隔符，选中后不消耗尾部未完成字母；缓存键、上下文和个人模型失效规则沿用已有系列查询。

## Alternatives considered

- **关闭末尾直接辅助码**：会改变用户现有辅助码设置，且不能修复仅在未匹配尾字母时发生的候选排序问题。
- **删除整段回退行**：会丢失用户把尾字母当作普通拼音继续输入时的候选，破坏既有单辅助码兼容行为。
- **所有情况下都把基础候选放到整段回退前**：会改变实际命中辅助码时的首选顺序，因此只在没有任何辅助码命中时调整。
- **让共享词格直接解不完整音节**：能提供整个输入的预测句，但要重新定义未输入韵母的搜索、读音与选词消耗契约，超出保留已有完整前缀的范围。
- **对每个短前缀都重新组词**：能让所有前缀拥有整句候选，但会增加正常完整输入的解码工作；这里只补未完成尾音节之前的最长完整前缀。

## Related decisions

[双拼末尾声母参与整句补全](../feature/2026-10-24-shuangpin-trailing-initial-sentence.md) 扩展本决定原先不做的末尾预测范围，仅在双拼末尾一个声母处开放完整词边；本篇的完整前缀回退、缓存和选词范围仍保留。共享普通整句入口仍要求完整音节。

## Consequences

输入 `fgkluvu` 这类“完整的 `疯狂水` 加下一个未完成音节首键”的序列时，`疯狂水` 保持在 `疯狂` 前面；真正命中辅助码的输入继续按辅助码匹配优先级排列。

不在词典中独立存储的组合短句同样保留，不依赖曾经输入过完整前缀，也不修改任何双拼、辅助码或排序默认值。关闭整句能力时仍只有词典前缀。代价是未完成尾音节的冷查询至多多一次完整前缀系列查询；逐键输入通常命中前一步已有的缓存。

## Verification

- `cargo test --locked -p msime-engine incomplete_tail -- --nocapture`：2 项通过。合成用例在修复前明确失败；覆盖辅助码关闭与未命中、手动分隔符、冷/热缓存、关闭词格、完整覆盖词条优先，以及四种双拼方案选中后保留尾字母。
- `cargo test --locked -p msime-engine`：默认完整套件通过，2575 项通过、56 项忽略。
- `cargo clippy --locked -p msime-engine --all-targets -- -D warnings`：通过；`cargo fmt --all`：通过。
- `MSIME_EVAL_RESOURCES=/Applications/MSIME.app/Contents/Resources/EngineResources cargo test --locked -p msime-engine shuangpin -- --nocapture`：通过。
- 使用已安装的发布词库与隔离用户/缓存目录逐键复核：辅助码关闭时，追加未完成音节保留前一步的完整前缀首选，连续三个更长的未完成音节同样成立；不读取个人词库，不记录反馈原文。
- 当前机器缺少 Developer ID 签名身份，因此不替换系统中的安装；以上证据属于真实词库上的共享引擎/会话验证，不是 InputMethodKit 或真实编辑器验收。
- 全套件另设 `MSIME_EVAL_RESOURCES` 指向当前安装资源时，历史固定日文模型词条数断言与 4 项旧参考词库 golden 不符；未改动的 `develop` 上同样复现这 5 项失败，默认套件通过。它们不属于此次双拼修复，不能把该资源组合报告为全绿。
- `pnpm run verify-notes`、`git diff --check`：通过。
- `bash scripts/verify-local.sh --quick`：失败于现有 `client-core` 测试的 3 项编译错误、Android 文档注释的 `-Werror`，以及当前 CommandLineTools 路径下找不到 Swift `XCTest`；`client-core` 错误在未改动的 `develop` 上复现，另外两项涉及的源码与基线相同。本次不修改这些无关代码。
- `cargo test -p msime-engine shuangpin_unmatched_tail -- --nocapture`（远端 `rbuild`，2 项通过）
- `cargo test -p msime-engine shuangpin -- --nocapture`（远端 `rbuild`，191 项通过，1 项忽略）
- `bash scripts/verify-local.sh --quick`（远端 `rbuild`；Rust、Linux、Android、Wasm、macOS 等阶段通过，Windows 本机既有 provider-list 断言失败）
- `pnpm run verify-notes`、`git diff --check`

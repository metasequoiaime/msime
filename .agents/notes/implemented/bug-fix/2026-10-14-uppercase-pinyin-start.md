# Agent Note: Shift 大写字母可以开始拼音组字

Status: implemented

## Problem

#7653：中文输入法状态下输入 `Observable` 时，首个大写 `O` 被宿主直接交给应用，后续 `bservable` 才进入拼音组字，导致原始大小写被拆开，英文候选无法按完整单词联想。

## Decision

全拼和双拼会话在没有组字时也接收 Shift 产生的大写字母，并把它作为保留大小写的拼音原文起点。快捷模式入口仍先于拼音接受过滤运行，因此启用的 `Shift+U/T/K/E/M/J/Y/R/V/C` 不会被这一改动抢走；非 Shift 产生的空闲大写字母继续交还宿主。后续输入按原有大小写原文与小写查询键分离逻辑处理，英文混输查询使用规范化小写键，候选文字保留用户输入的大小写。

## Alternatives considered

- **只在 macOS 宿主拼接首个大写字母**：可以针对报告平台做补丁，但会让 Android、Windows、Linux 和共享引擎的实体键盘行为不同，也无法复用 Engine 已有的大小写原文与查询键分离逻辑。
- **切换到专用英文模式**：能得到英文联想，却丢失中文方案的拼音候选和用户原本的输入状态，且需要额外的模式切换，不符合“中文状态下直接输入”的复现步骤。

## Consequences

Shift+大写字母现在可以成为拼音组字的首键，后续英文字符会在同一组字里参与英文混输，原始大小写可在预编辑和原样提交中保留。仅 Caps Lock 或宿主未标记 Shift 的空闲大写字符仍保持透传语义；快捷模式入口及其他方案不变。

当这段原文无法完整切成拼音时，结束组字也保留用户输入的英文原文。这样 `Observable<` 不会因为纠错候选存在而把中文首候选误上屏；完整拼音（例如 `Ni`）仍按首候选结束组字。

## Verification

`cargo test -p msime-engine shifted_uppercase_starts_pinyin_and_mixes_the_cased_english_word` 覆盖 `Observable` 的首个大写、完整预编辑和英文候选；`cargo test -p msime-engine punctuation_after_` 覆盖不完整大写原文保留与完整拼音选词；`cargo fmt --all` 已运行。

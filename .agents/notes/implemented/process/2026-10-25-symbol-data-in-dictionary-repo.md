# Agent Note: 符号数据统一由词库仓保管

Status: implemented

## Problem

符号相关的数据散在三处：打拼音出符号的名称表和 LaTeX 命令名表写在 msime 仓库里，符号模式的分组表由 `scripts/build_symbol_groups.py` 直接从上游 rime-wanxiang 下载，表情面板符号页的 `piliapp_symbols.txt` 放在 msime 的 `resources/dictionary-sources/symbols/`。而词库仓 msime-dictionary 的 `sources/symbols/` 早就收了万象符号表，MSIME-Windows 的构建器也从那里读。数据不在词库仓，改符号要碰引擎代码仓，同一份上游数据还会从两个地方取。`piliapp_symbols.txt` 采集自网站，许可从来没有记录，`docs/third-party.md` 和随词库分发的 NOTICE 都没有它。

## Decision

- 名称表与 LaTeX 命令名表迁到 `sources/symbols/symbol-names.tsv`、`latex-symbols.tsv`，msime 留锁定的同步副本，见 #9638 带来的 `feature/2026-10-25-math-greek-symbol-names.md`（本篇合入时它还在那个 PR 里，所以不写相对链接）。
- 符号模式分组表的锁 `resources/symbol-groups.lock.json` 改从词库仓固定提交（`dictionary_commit`）下载 `sources/symbols/wanxiang_symbols.yaml` 与 `LICENSE`，它们与上游同一提交逐字节一致，生成的表不变；上游提交仍记在锁里供署名，`test-symbol-groups.py` 核对地址指向词库仓。
- 表情面板符号页的 `piliapp_symbols.txt` 原样移到词库仓 `sources/symbols/piliapp-symbols.txt`，dict-builder 的 Symbols 阶段按 `others::PILIAPP_SYMBOLS` 经 `--dictionary` 读取，面板内容、12 个父分类 ID 与小标题都不变。许可如实记为「未声明」：随词库分发的 `resources/licenses/msime-engine-dictionary-NOTICE.md` 加了这一行，词库仓 `NOTICE.md` 的「待解决」写明它照旧进入 `msime-others.db`、不在 `licensing.rs` 的排除名单里，以及可替换的来源能覆盖多少。

## Alternatives considered

- **符号页换成万象分组加 codex 名称。** 最强的理由是许可干净（CC BY 4.0，已经在 12 个渠道署名），MSIME-Windows 已经这样做。不采用的原因：两者只覆盖 PiliApp 2323 个符号中的 1138 个，动物（260）、人员与活动（113）、盲文（65）、心形（43）、分数（20）等分类整类没有；面板的 12 个父分类 ID 是跨平台契约（Android `SymbolPanelModel` 写死标签页、`packages/ui` 的翻译表、插件 `symbol_set` 的 `parent`），MSIME-Windows 是连分类一起换的。六个平台的符号页会少一半，由维护者决定先不换。
- **按 Unicode 区块与字符名自己生成面板目录。** 最强的理由是事实数据、没有许可问题，也能保住大部分分类。不采用的原因：内容会变，要重新设计每个分类取哪些区块，是单独的一件事；搬家不该夹带内容变化。
- **把 PiliApp 表放进 `sources/unlicensed/` 并在 `licensing.rs` 排除。** 最强的理由是符合词库仓「没有明确许可就不进发布」的规则。不采用的原因：发布版的符号页会整页变空，等于在没有替代来源时删掉一个功能；这份数据在 msime 里时就一直随发布分发，搬家不改变它的许可状态，只是把它写明。

## Consequences

- 改符号数据只改词库仓。符号页的 PiliApp 表在许可确认或换来源之前不再加内容，词库仓 `AGENTS.md` 与 `NOTICE.md` 写明了这一点。
- 合并顺序：先合词库仓加入 `piliapp-symbols.txt` 的 PR，再合 msime 的 dict-builder 改动；反过来，下一次 `dict-v` 发布会因为 `--dictionary` 检出里没有这个文件而在 Symbols 阶段失败。
- 验证：`cargo test -p msime-dict-builder` 通过；用词库仓 PR 分支的检出跑 `msime-dict-build --only symbols --dictionary <检出>`，构建出 2323 个符号，和移动前的源文件逐字节一致。没有重跑整套 `dict-v` 发布。

# Agent Note: Linux 候选拼音行的组字光标按引擎位置放

Status: implemented

## Problem

IBus 候选窗辅助行和 Fcitx5 的辅助行、原生画布拼音行都画组字光标，位置由 `CandidatePalette.h` 的 `candidate_preedit_with_caret` 决定：只在显示的 `View.preedit` 等于按键原文 `editing_text` 时按 `caret_position` 放，否则不画。全拼的 `preedit` 是分词后的 `ni'hao`，双拼选「拼音分词」时是展开的全拼，都不等于原文，所以多音节的全拼和这类双拼在 Linux 上从来不画光标，Ctrl+左右按音节移光标时拼音行看不出光标在哪。IBus 默认分支的 `Candidate auxiliary text did not expose the preedit caret` 断言抓到的就是它；这条断言写于 #2116，那时宿主也是同样的规则，默认分支一直没进 CI，所以没人发现。

## Decision

新增 `candidate_preedit_caret_bytes(preedit, editing, caret, preedit_caret)`，两个宿主的辅助行和 Fcitx5 原生画布都用它：

- 显示的就是按键原文：照旧按 `caret_position`（字节）放，含末尾。
- 显示的是分词或展开过的拼写：按 [引擎给的 `View.preedit_caret`](2026-11-02-engine-preedit-caret.md)（Unicode 标量）换成字节放，音节边界上落在隔音符前面（`ni|'hao`）。
- 这时光标在末尾或越界一律不画。引擎在两段文字没有可靠对应（假名、韩文、笔画字形等）时把 `preedit_caret` 放在末尾，那里画一个光标不带信息，这些方案的拼音行也就和以前一样不画。视图里没有这一项时不画。

IBus 辅助行把光标画成纯文本 `|`；Fcitx5 辅助行同样，原生画布拿这个字节位置画光标。

## Alternatives considered

- **一律按 `preedit_caret` 画，末尾也画。** 和 macOS、Windows 候选窗的几何光标一致，规则更简单。但 IBus 辅助行只能插一个 `|` 字符，日文、韩文的拼音行会一直拖着一个没有信息的 `|`，等于给这些方案加了一个以前没有的显示变化，这次没有在这些方案上验收。
- **只改用例，接受不画光标。** 改动最小，但断言守的正是「Ctrl+左右移光标时拼音行看得出光标」，Windows 候选窗和 TSF 已经按引擎位置画，Linux 照旧看不出来就是缺陷。

## Consequences

全拼多音节和双拼拼音分词在 Linux 候选拼音行里能看到光标随 Ctrl/Shift+左右按音节移动；光标在末尾时多音节全拼仍不画，和以前一样。代价是两个宿主多读一个视图字段，规则多一个分支。IBus 辅助行的文字在光标停在中间时多一个 `|`，读辅助行文字的地方（测试、无障碍）会看到它。这是界面变化，需要在真机 GNOME/IBus 和 KDE/Fcitx5 上看一眼。

## Verification

`linux-candidate-palette` 覆盖原文相同、分词全拼、双拼展开、假名的标量换字节、末尾和越界不画。IBus 默认分支 `ibus-native-context` 用 Ctrl+左把光标移到 `ni` 后面，断言辅助行出现 `ni|'hao`。两个都在 Linux 资源容器里跑过。

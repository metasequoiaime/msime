# Agent Note: 连续大写只有第一枚进快捷模式，本地模式回车原样上屏入口字母

Status: implemented

## Problem

#7616：快捷模式的入口键和输入大写字母是同一个组合（Shift+字母）。中文模式下按住 Shift 连打 `MRI` 时，`R`（临时日文）会打开快捷模式并吃掉这个字母，回车只上屏 `I`。判定入口的是共享引擎，所以同一冲突在 Linux、Windows、Android、iOS、HarmonyOS 上一模一样；Linux 那边 `platforms/linux/fcitx5/tests/native.cpp` 还断言入口键必须被消费。

两个临时模式（`Y` 临时英文、`R` 临时日文）另有一处与其它七个本地模式不一致：回车（`CommitRaw`）把入口字母当模式标记从原样上屏的文本里去掉，所以只按了入口键就回车时上屏的是空串，按了 `R` 再打 `ka` 回车只上屏 `ka`；日期、表情、颜文字等七个模式回车都原样上屏含入口字母的预编辑。

[#7723](../../archived/bug-fix/2026-10-14-consecutive-uppercase-shortcut.md) 曾在 macOS 宿主里用 `_uppercasePassthroughPending` 单独处理这一件事，只覆盖「上一枚大写已经交给应用」这一种情形，其它平台的宿主各写一套。

## Decision

判定留在共享引擎：会话记住「上一枚按键是本会话交还给宿主的大写字母」，这一串大写里只有第一枚字母能进快捷模式，后面的入口字母照常交给宿主当大写字母。任何不是「交给宿主的大写字母」的按键都结束这一串——小写字母、数字、标点、任意命令（回车、空格、退格、光标）、宿主结束组字（`Action::Finish`、失焦结束、macOS 的 `Cmd+A`，它们经过 `InputSession::finish` 而不经过 `handle_command`；runtime 的字面标点路由同样调它）、失焦或换应用的重置——下一枚字母重新算第一枚。没有组字时 Shift+字母只可能被交给宿主或进快捷模式，所以各宿主不需要各自的实现，macOS 那边 #7723 的特判随之删除。

回车保持「原样上屏」的语义，并把入口字母算作文本：本地模式回车时上屏所见预编辑，空输入时就是入口字母本身。进词库的一侧仍把入口字母当标记——`LocalInputMode::without_entry_marker` 是这两侧的唯一判据，只有入口字母时去掉标记后没有正文，就直接跳过学习（旧实现上屏空串，同样不学；不跳的话临时英文会报「学不了」，临时日文还会把补出来的 `r` 写进词库），临时日文照旧补 `r` 前缀，所以学进词库的英文词和打字统计的口径不变。

## Alternatives considered

- **只在 macOS 宿主记录大写串（#7723 的做法）**：修得了报告平台，但 Android、Windows、Linux 与共享引擎行为各不相同，而且和 Linux 那条「入口键必须被消费」的断言方向相反。
- **只做「仅第一枚字母」不回车**：`MRI` 这类连打能修好，但按下的第一枚入口键仍然拿不回来（回车什么都不上屏），报告里「回车希望把 RI 一起打出来」的诉求没解决。
- **让回车继续去掉入口字母**：与其它七个本地模式相反，而且用户按了哪个键、屏幕上显示的就是哪个字母，上屏文本里少一个字符更难解释。
- **关掉「Shift+字母进入快捷模式」总开关**：总开关是现成的逃生口，但关掉会同时失去日期、表情、临时语言等全部入口，不能当修法。

## Consequences

按住 Shift 连打大写时只有第一枚字母可能进模式，第二枚起一律是大写字母；单个 Shift+入口键（这一串的第一枚）照旧进模式，总开关的行为不变。两个临时模式回车上屏的文本多了入口字母，与其它七个本地模式一致；词库、学习与打字统计不变。macOS 宿主不再自己判断连续大写，`TestConsecutiveUppercaseDoesNotOpenShortcut` 改成只验证宿主把键交回应用。

## Verification

- `crates/engine/src/session/tests.rs`：`only_the_first_letter_of_an_uppercase_run_opens_a_shortcut`（连打 `M`、`R`、`E`，后两枚交给宿主；重置上下文或一条命令之后入口键照旧进模式）、`a_host_finish_ends_the_uppercase_run`（宿主结束键与标点路由同样结束这一串，来自 PR #8035 的评审）与 `enter_commits_the_local_preedit_with_its_entry_letter`（上屏 `R`、`Rka`、`Y`、`Yhe`、`Trq`、`Cab`）。
- `crates/engine/src/host/tests.rs`：临时英文回车上屏 `Yrustacean`，词库学的仍是 `rustacean`；临时日文回车上屏 `Rkk`，词库学的仍是带 `r` 前缀的 `rkk`；`a_bare_entry_letter_raw_commit_learns_nothing`（只按入口键回车：上屏 `Y`/`R`，无诊断，英文词库不多出 `y`/`r`，来自 PR #8035 的评审）。
- 黄金场景 `temp_japanese_enter`、`temp_english_enter_and_learning` 只在上屏文本一处与参考实现不同，学词那一半（`dump_journal`）没有变化。
- `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-client-core`；`bash scripts/verify-local.sh --quick`（macOS、Windows 阶段在本机跳过）。
- macOS `ShortcutTest` 的相应用例同步改到新契约；本机编不了 macOS，未运行。

# Agent Note: 五笔开放本地表模式入口

Status: implemented

## Problem

五笔编码只使用小写字母，但引擎把所有本地模式入口统一绑在 `opens_local_modes` 上，因此五笔不会发布或处理 Shift+K、`/`、`@`。启用短语表、指令表或名单后，五笔用户仍无法从键盘进入这些模式；同时运行时把字面 `/` 交给引擎时会错误地打开模式。

## Decision

新增 `SchemeType::opens_table_modes`，让全拼、双拼和五笔提供只查本地表的 K、`/`、`@` 入口；其它依赖拼音字母的 Shift 模式仍只由 `opens_local_modes` 控制。引擎在空闲时按两组能力分别判断入口，运行时的 ASCII 标点路径也按 `opens_table_modes` 排除 Wubi 的本地模式，使宿主明确要求的字面 `/` 不会打开指令模式。

## Alternatives considered

- 只在 Linux 或桌面宿主里为五笔转发这些按键：不同宿主会得到不同结果，移动端和共享运行时仍无法使用同一插件能力。
- 让五笔复用所有 Shift 模式：表达式、临时英文等模式依赖拼音输入，不应改变五笔的编码键语义。
- 只改变引擎而不改运行时的 ASCII 标点分流：宿主要求字面 `/` 时运行时仍会把它送回引擎，刚发布的模式入口会抢走字面字符。

## Consequences

五笔在空闲状态可进入 K、`/`、`@`，并继续把 Shift+V 等拼音专用模式交还宿主；有组字时 `/` 和 `@` 仍按普通字符结束或延续编码。运行时和引擎共享同一能力划分。在把按键交给 Engine 之前自行判断的宿主各有一份副本（Linux `SpellingSymbols.h`、Windows Server 下发给 TSF 的触发帧），它们随后一起改为同一份方案集合，见 [插件门控如实显示、宿主侧五笔表模式与插件失败的诊断出口](2026-10-09-plugin-gates-and-wubi-table-modes.md)。

## Verification

先让引擎回归测试和运行时字面标点测试分别在旧实现上失败，再完成实现。`cargo test -p msime-engine --lib --locked`：1576 passed、13 ignored；`cargo test -p msime-input-runtime --lib --locked`：159 passed；`cargo fmt --all -- --check`、`cargo clippy -p msime-engine -p msime-input-runtime --all-targets --locked -- -D warnings`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均通过。

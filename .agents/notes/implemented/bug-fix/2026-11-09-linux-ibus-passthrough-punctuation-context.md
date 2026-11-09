# Agent Note: IBus 直通数字后的智能标点前文

Status: implemented

## Problem

#11166 报告 Ubuntu Wayland 上 IBus 的终端和 Firefox 输入框标点行为异常。现有 Linux 智能标点影子只记录输入法自己上屏的文本；Alacritty 一类客户端不发布 surrounding text 时，用户直接输入的数字交给应用后不会留下可供下一次 `.`、`,`、`:` 判断的前文，于是版本号和时间等序列走中文标点。浏览器输入框还可能在没有可用 surrounding 时沿用客户端自己的输入处理，必须保持宿主上下文不可用时的保守边界。

## Decision

IBus 在没有 surrounding text 时，额外记录最近一次交给应用的 ASCII 字母或数字，但只保留到下一次按键，并且只用于下一次智能标点的数字后/字母后判断。输入法自己的提交、光标移动、删除、其他直通按键以及非智能标点都会清掉这个临时前文；它不并入最多 1024 字节的长期影子，也不改变有效 surrounding text 的优先级。这样 `7.` 在不支持 surrounding text 的终端里保持 `7.`，而未知文档内容仍不会被猜成 ASCII 前文。

## Alternatives considered

- **继续只记录输入法自己的提交。** 这能避免误判，但无法覆盖 #11166 的终端场景；报告者直接输入的数字本来就由应用接收，输入法没有任何前文可读。
- **把每个直通字符追加进长期影子。** 最强的理由是实现简单。没有采用：应用可能拦截、改写或不插入按键，长期影子会跨越后续编辑把未知内容误当成 caret 前文。
- **在无 surrounding 时一律输出中文标点。** 最保守，但会让终端无法使用数字后的英文标点；单按键临时记录能覆盖已知的相邻按键而不扩大信任范围。

## Consequences

IBus 的 Alacritty 等客户端在输入法无法读取 surrounding text 时，数字或字母后紧接的 `.`、`,`、`:` 会按智能标点设置保留 ASCII。有效 surrounding、中文前文、光标移动和其他按键的语义不变。该临时状态只在 IBus 宿主内存中存在，不写入偏好、日志或输入统计。

## Verification

新增 `platforms/linux/tests/runtime/engine_smoke.cpp` 回归：清空 surrounding text 后让 ASCII 数字直通应用，再按句号，断言输入法消费句号并上屏 ASCII `.`；原有输入法自己上屏数字、有效 surrounding 和重复标点用例保留。已运行 `rbuild bash scripts/verify-local.sh --quick`；尝试运行 Linux 原生容器测试时因本机 Docker daemon 不可用而跳过。未在报告者的 Alacritty/Firefox 实机上验收的部分写入 PR 验证段，请报告者复验。

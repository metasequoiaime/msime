# Agent Note: 增加 un 与 ong 模糊音

Status: implemented

## Problem

模糊音设置已有五组韵母规则，缺少部分方言用户需要的 `un ⇄ ong`。用户在全拼中输入 `lun`、`chun` 等读音时无法通过模糊音匹配对应的 `long`、`chong` 候选。

## Decision

新增稳定规则标识 `un-ong`，分配 Engine 位 11，并接入共享偏好枚举、首次启用种子规则、macOS 原生模糊音列表和 Engine 双向扩展。缺字段仍按既有偏好默认规则处理，已有用户的显式规则集合不被静默改写。

## Alternatives considered

- **只在 macOS 设置页做本地映射**：改动小，但共享设置、账号同步和其它宿主会出现规则不一致，放弃。
- **把规则合并到相近的 `uan-uang`**：会让 `un` 和 `uan` 的匹配范围混在一起，无法精确表达用户选择，放弃。

## Consequences

共享 Rust、Engine 和 macOS 设置页使用同一个规则标识。开启全部规则的首次种子集合从 11 项变为 12 项，旧文档中没有新规则时保持原有选择，新增规则只有用户明确选择或首次启用时才生效。

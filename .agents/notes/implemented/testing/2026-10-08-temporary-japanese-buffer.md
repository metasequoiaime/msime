# Agent Note: 临时日文候选复用会话缓冲

Status: implemented

## Problem

临时日文模式每次编辑都用 `engine.candidates().to_vec()` 替换 `local_candidates`，即使会话已有足够容量也会重新申请候选行缓冲。

## Decision

刷新时清空现有 `local_candidates`，再把引擎候选克隆回同一缓冲；保留原有预编辑更新和空结果兜底行逻辑。已有容量足够时刷新不会更换底层存储。

## Alternatives considered

继续直接赋值会让每次临时日文按键都分配新 `Vec`；把引擎候选改成借用切片会越过会话和引擎的所有权边界。清空并扩展现有缓冲只改变存储复用方式，不改变候选所有权。

## Verification

新增真实会话回归测试，旧实现会更换候选缓冲指针；改造后候选缓冲指针保持不变。临时日文相关测试 4 项、引擎完整单元测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖实现。

## Consequences

临时日文连续编辑且已有容量足够时不再为候选行列表分配新缓冲；候选行仍按引擎顺序逐行克隆，容量不足时仍按标准 `Vec` 规则增长。

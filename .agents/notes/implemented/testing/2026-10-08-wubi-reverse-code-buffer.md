# Agent Note: 五笔反查编码缓冲复用

Status: implemented

## Problem

五笔候选刷新会逐词反查完整编码。反查缓存命中时仍复制新的 `String`，展示列表还会每次重新创建编码向量，重复刷新会为已有编码反复分配。

## Decision

反查 provider 增加写入已有字符串的路径，缓存命中时借用编码文本再写回目标缓冲；registry 复用编码向量和其中的字符串，按候选数量截断或补齐。原有缓存、排序和空编码行为保持不变。

## Alternatives considered

继续返回拥有的 `String` 会在缓存命中时重复复制；只复用外层编码向量仍会为每个编码重新分配。改变候选结构以借用缓存文本会跨越 provider 和会话的所有权边界。

## Verification

新增缓存命中写入已有字符串的零分配测试；五笔 provider 测试、完整引擎测试、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

五笔方案连续刷新候选时可以保留反查编码的字符串存储，减少反查展示路径上的堆分配。

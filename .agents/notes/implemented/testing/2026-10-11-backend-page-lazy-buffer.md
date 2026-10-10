# Agent Note: 后端分页查询延后结果缓冲

Status: implemented

## Problem

后端 `catalog` 和 `dictionary` 分页查询在执行 SQLite 行读取前，分别按 `limit + 1` 创建 JSON 结果向量。空搜索、偏移超出结果或首行读取失败时，结果页没有任何可写项目，却仍申请整页容量；分页接口是账户服务和设置页的正常读取路径。

## Decision

两个读取循环都从空 `Vec<Value>` 开始。首条行已经成功转换为 JSON 后，再按原 `limit + 1` 一次 `reserve_exact`；随后继续追加、截断、`has_more` 判断和类别查询。有效页继续保留旧容量提示，空页和首行错误不会申请结果页存储。

## Alternatives considered

- 查询前继续预留整页：命中路径少一个分支，但每次空分页都承担固定容量成本。
- 按每行自然增长：空页成本最低，但密集页会失去原上限提示并增加扩容次数。
- 先收集再按结果数分配：需要同时保留行迭代和中间存储，增加峰值并改变读取失败时的所有权边界。

## Verification

新增预热后的空 catalog 和空 dictionary 分页回归；旧实现分配计数分别为 51、43，修复后为 50、42，精确去除各自的结果页预留。提交前继续运行引擎全量单测、golden、Clippy、Rustfmt、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

空后端分页查询不再为 `limit + 1` 条 JSON 项申请结果存储；首条有效行仍按原分页上限准备容量。分配计数只反映 Rust 结果容器与请求包装，不代表 SQLite 内部或系统 RSS，也不把一次分配减少直接等同于端到端延迟下降。

# Agent Note: 单读音字典查询移动唯一键

Status: implemented

## Problem

`LanguageDictionary::lookup_readings` 在所有位置只有一个读音时，为每条结果克隆同一个连接键。注音重算的 `limit` 通常为 1，因此单结果查询会产生一次没有必要的字符串分配。

## Decision

查询结果为空或只有一条时，直接移动已构造的键进入结果；只有多条结果时才保留逐条克隆键的行为。新增 SQLite 回归测试，预热语句缓存后确认单结果路径少一次分配，并验证结果内容不变。

## Alternatives considered

- 改变返回类型，让多条结果共享键的 `Arc<str>`：会扩大 API 影响面，并给候选构造增加共享指针成本。
- 为单结果场景新增独立查询 API：会重复 SQL 查询逻辑；当前分支判断已足够局部。

## Consequences

单结果单读音查询少一次字符串分配；多结果查询的排序和所有权行为保持不变。空结果也会直接返回空向量。

# Agent Note: 空崩溃记录扫描延后缓冲

Status: implemented

## Problem

`crash_record_paths` 在扫描目录前固定预留 `MAX_CRASH_RECORDS_PER_START` 个路径槽位。没有崩溃记录的正常启动也会分配不会使用的结果缓冲。

## Decision

路径向量从空容器开始，首个常规崩溃记录文件加入前再按原批次容量执行 `reserve_exact`。有效扫描仍一次预留原批次容量，文件类型筛选、错误传播、记录排序和清理语义不变。

## Verification

空目录容量断言先在基线因固定预留而失败，修复后通过；原有非空目录容量断言保持通过。相关遥测测试、client-core 全量测试、Clippy、Rustfmt、差异检查和 quick 门禁覆盖提交。

## Alternatives considered

- 继续按原批次预留：实现简单，但正常启动会保留未使用容量。
- 让向量自然增长：空扫描不分配，但首批有效记录可能多次扩容。

## Consequences

没有崩溃记录的启动不再申请路径缓冲；首个有效记录出现时仍一次准备原批次容量，扫描结果和后续处理保持不变。

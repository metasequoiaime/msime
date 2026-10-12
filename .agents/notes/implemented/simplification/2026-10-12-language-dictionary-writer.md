# Agent Note: 语言词典共用写库流程

Status: implemented

## Problem

笔画、粤拼、注音词典各自复制建库、写入格式与来源元数据、写入音节和词条、ANALYZE、完整性检查及 freeze 流程。格式升级时三处必须同步修改，否则同一引擎结构会产出不同的数据库。

## Decision

`crates/dict-builder/src/language_writer.rs` 接收有序音节迭代器、词条映射、来源提交和许可证，保留原有 SQL、写入顺序与收尾顺序。三种词典的公开 `write` 方法只提供各自的音节与许可证；笔画仍先构造排序后的五笔画集合。

## Alternatives considered

- 将 `Dictionary` 类型合并：三种构建器的来源和统计不同，统一数据模型会扩大本次改动。
- 在 `languages::build` 里统一写库：现有模块测试和调用方直接使用各自的 `write`，迁移会改变调用边界。

## Consequences

语言词典格式的写入顺序只有一个维护点。三种词典现有的引擎读取、元数据和重复写入字节稳定性测试继续覆盖各自入口。

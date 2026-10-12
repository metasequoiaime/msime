# Agent Note: provider Unix socket 请求写入共用入口

Status: implemented

## Problem

在线候选、翻译、语音流、语音取消和语音停止分别把请求追加换行后写入 Unix socket。行协议的终止符约束和写入行为散落在五处，修改帧格式时容易只改部分调用。

## Decision

`crates/input-runtime/src/providers.rs` 增加 `write_request`，统一追加单个换行并执行 `write_all`。各调用方仍自行设置写超时、请求大小上限和响应截止时间；面板请求已有自己的带截止时间分片写入逻辑，保持不变。

## Alternatives considered

- 让 `exchange_panel_request` 覆盖所有请求：它会重新计算并管理写入截止时间，与流式语音和在线查询已有的超时语义不同。
- 只保留 `with_terminator`：调用方仍重复写入调用和错误处理，无法集中行协议写入行为。

## Consequences

五类简单 provider 请求共享同一行写入入口，超时和响应读取策略仍由各协议保留。现有 provider 测试覆盖请求帧、超时和响应处理。

# Agent Note: macOS 云词库本机应用状态返回词库版本

Status: implemented

## Problem

共享“应用到本机”页面先请求 `snapshot_status`，只有响应包含字符串 `localVersion` 才允许下载并预览云端快照。macOS 的 `BackendDesktopSnapshots` 只返回 `nativeFiles` 和任务状态，因此该页面始终提示尚未获取本机词库版本，无法开始本机应用流程。

## Decision

macOS `snapshot_status` 使用与 `snapshot_preview` 相同的 `DesktopSnapshotTarget` 捕获当前原生宿主词库版本，并在响应中返回 `localVersion`。宿主会话不可用时保留捕获错误，让页面显示无法读取本机状态；预览和入列时现有的版本重查继续负责拒绝过期快照。

## Alternatives considered

- 让页面跳过 `localVersion` 检查：可以减少一次本机查询，但会在宿主不可用时仍允许下载与准备快照，直到较晚阶段才报错，也与移动平台的页面契约不一致。
- 从上一笔任务的 `expectedLocalVersion` 推导当前版本：首次进入页面没有任务，而且历史任务保存的是当时版本，不能代表当前词库。

## Consequences

macOS 共享页面可以在原生宿主可用时开始云词库下载与预览；每次状态轮询会计算一次当前词库版本。版本只作为页面可用性和展示信息，应用前仍须通过原有的本机与云端版本校验。

## Verification

`desktop-dictionary-provider` 的注入宿主测试先在旧实现上因缺少 `localVersion` 失败，修复后通过；测试也确认状态请求不依赖账户凭据。

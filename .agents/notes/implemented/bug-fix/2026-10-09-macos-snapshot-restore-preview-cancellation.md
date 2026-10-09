# Agent Note: macOS 云词库恢复预览取消与原生选文件契约

Status: implemented

## Problem

共享词库文件页在 macOS 上把“放弃恢复”和页面卸载发送为 `snapshot_restore_cancel`，原生 `BackendDesktopSnapshots` 只接受带令牌的 `snapshot_discard`。取消请求因此返回错误，已校验的恢复令牌继续有效，私有快照副本也一直保留到下一次预览或提供者销毁。文件页还先打开网页文件选择器，随后原生提供者忽略所选文件并再次打开系统选择器；用户取消系统选择器时，原生返回 `cancelled`，页面却只识别 `saved: false`。

## Decision

macOS 提供者接受共享页面使用的 `snapshot_restore_cancel`，只清除云端恢复预览，不影响本机应用预览和正在运行的应用任务。取消系统文件选择时返回页面识别的 `saved: false`。macOS 客户端提供 `chooseSnapshotRestore`，通过 Rust 命令层新增的 `snapshot_choose_restore` 请求直接打开系统选择器，使恢复流程只选一次文件；移动平台明确拒绝该原生操作。恢复令牌被取消后，后续 `snapshot_restore_native` 无法触发网络恢复。

## Alternatives considered

- 把共享页面改为发送带令牌的 `snapshot_discard`：可以复用原生现有操作，但页面卸载时未必还持有令牌，并会把平台内部的令牌清理细节扩散到共享页面。
- 保留网页选择器并让原生读取它选中的文件：网页 `File` 不携带可安全传给原生层的持久路径，传完整快照文本会破坏目前只在原生层保留大文件内容的边界。

## Consequences

取消恢复会立即释放原生预览，旧令牌无法再恢复云端词库；macOS 用户只需选择一次文件。新入口只在 macOS 暴露，Android 和 iOS 仍走各自的现有恢复流程。本机应用预览的取消仍由独立的 `snapshot_cancel` 管理。

## Verification

原生回归测试先在旧实现上以 `BackendAccountClient.Failure(status: 400)` 失败，修复后通过；测试覆盖预览取消后旧令牌失效、选择器取消响应。前端能力测试覆盖 macOS 路由到 `snapshot_choose_restore` 和 Android 不暴露该入口；Rust 请求校验测试覆盖新操作。

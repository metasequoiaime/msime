# Agent Note: IBus 切换在线服务时撤销旧候选

Status: implemented

## Problem

IBus 在组字中热更新 `online_provider_socket` 时，只调用 `invalidate_providers()` 取消待发和迟到请求。旧服务已经送达的云候选或 AI 候选仍保留在 Engine 缓存与候选栏；新 socket 无服务端时，旧行会一直留到组字结束。Fcitx5 同一路径已清除两个来源。关闭云候选和修改 AI 偏好由 [Host API 偏好更新](2026-10-13-host-api-cloud-disable-invalidates-candidates.md) 清理，不覆盖 socket 切换。

## Decision

IBus 发现在线 socket 变化时，先使旧请求失效，再通过 Host API 清除来源 0 和 1 的在线候选，并在配置定时刷新中重绘返回的新视图。翻译 socket 单独变化仍只清理翻译，不撤销在线候选。

## Alternatives considered

- 仅等待新服务的候选覆盖旧行：新 socket 可能没有服务、返回空列表或拒绝请求，旧行仍会误导用户。
- 重建整个会话：会丢失正在输入的组字；Host API 已有按来源清除候选的入口。

## Verification

用锁定词库和本地合成 provider 运行 `ibus-engine-smoke --online-socket-switch`。回归先收到「云端测试」，随后把在线 socket 改到不存在的端点：修复前五秒后仍显示旧行，测试失败；修复后旧行消失，拼音组字保留。该独立用例避开完整 smoke 中与本问题无关的语音断言。

`scripts/verify-local.sh --quick` 的合约、笔记、Rust workspace 和 Linux 桌面组件检查通过；Linux 原生容器编译阶段因本机剩余空间接近 2 GiB 而主动停止，未取得完整门禁结果。定向 IBus 宿主与 Host API 均已在容器中编译并运行上述回归。

## Consequences

socket 切换会推进候选代次并立即撤销旧在线行；本地候选和当前组字继续可用。新服务成功返回时，再按当前查询插入新在线候选。

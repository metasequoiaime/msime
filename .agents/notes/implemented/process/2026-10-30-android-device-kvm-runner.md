# Agent Note: Android 设备作业按实际 KVM 支持选择 runner

Status: implemented

## Problem

[Android 设备验收迁移到 Depot](../../implemented/process/2026-10-29-android-device-on-depot.md)依据单台 runner 探测结果选择 `depot-ubuntu-24.04-8`，但[后续 PR 的新 runner](https://github.com/metasequoiaime/msime/actions/runs/40902873434/job/123549853877)在 `/dev/kvm` 字符设备检查处直接失败，没有启动模拟器，也没有运行任何设备回归。改权限不能补出没有透传的硬件。

[Depot 官方故障说明](https://depot.dev/docs/github-actions/troubleshooting)明确：普通 GitHub Actions runner 不提供 `/dev/kvm`，支持嵌套虚拟化的是另一套 Depot CI sandbox。此前某台 runner 开机成功的证据仍然有效，但不能成为该标签持续提供 KVM 的保证。

## Decision

只把 `.github/workflows/android-device.yml` 的设备 job 恢复到原先使用过的 GitHub `ubuntu-24.04`。保留已有按需开放 KVM 权限、设备启动、四个 client crate 设备单测、整套装包验收，以及有界日志采集和退出清理。缺少字符设备时给出明确错误，不降级为无加速模拟器，不跳过设备门禁。

### 既有笔记审计

- [迁移到 Depot](../../implemented/process/2026-10-29-android-device-on-depot.md)：部分取代，只翻转 runner 选择；单台探测记录、按权限初始化和日志采集边界继续保留，双方互链。
- [默认 Depot](../../implemented/process/2026-10-27-depot-runners-by-default.md)：部分重叠，Android 设备作业重新属于缺少硬件能力的 GitHub 例外，其他 job 归属保持。
- [Android 设备门禁](../../implemented/testing/2026-10-08-android-device-gates.md)：部分重叠，只同步执行环境；触发范围、API 级别和验收要求保持。
- 活跃 proposed/rejected 笔记没有同一 runner 选择提案，不合并其他功能计划。

## Alternatives considered

- **继续使用 Depot，修改 udev 或权限。** 保留更快构建和 GitHub 并发额度；但当前失败发生在设备存在性检查，权限命令不能制造 KVM 能力，官方也不承诺这个产品提供该设备。
- **迁移到 Depot CI sandbox。** 官方明确支持嵌套虚拟化，是未来继续节省 GitHub 托管额度的路线；但它属于不同执行系统，需要单独验证事件、可复用 workflow 与检查回报的接入，不在修复当前 GitHub workflow 的最小范围里。

## Verification

原 PR 的设备作业在字符设备检查处失败，未运行模拟器和本次产品回归。`actionlint`、笔记门禁、本地 quick 与 `git diff --check` 通过；quick 包含仓库契约、Rust workspace 和共享 Apple bridge，其余平台按 workflow 改动范围跳过。提取真实 KVM 初始化脚本，在没有该字符设备的本机执行，确认明确报错并以 1 退出。本 PR 实际设备运行结果见 PR，不把其他 runner 的开机记录当作本次整套验收。

## Consequences

设备 job 重新占用组织的 GitHub 托管并发额度，构建可能变慢；这是恢复可运行设备验收的代价。触发范围、API 级别、测试断言、发版依赖和失败退出保持，硬件缺失时增加明确的错误说明，不降级为无加速模拟器或跳过门禁。Depot CI sandbox 可作为未来独立评估的执行系统。

托管镜像的磁盘余量不稳定：装完 NDK、SDK 和系统镜像后，模拟器开机要为 userdata 分区留出 7.2 GB，2026-11-01 develop 推送的 API 28 两次只剩 6.7 GB，模拟器报 Not enough space 退出，开机步骤空等 10 分钟超时（#10516）。设备 job 因此在装工具之前先删掉用不到的预装工具链（dotnet、Swift、GHC、Boost、CodeQL），并在日志里留下清理前后的 `df`。

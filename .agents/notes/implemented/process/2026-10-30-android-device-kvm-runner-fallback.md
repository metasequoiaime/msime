# Agent Note: Android 设备门禁回到有实证的 KVM runner

Status: implemented

## Problem

设备门禁从 GitHub 托管 runner 挪到 Depot 后，新的 PR 实际作业在第一步就因 `/dev/kvm` 不存在而失败。此前 Depot 的探测只证明某一次 runner 有 KVM，不能证明每次分配到的实例都提供设备节点；设备测试因此还没有开始。

这篇笔记 supersede 了[Android 设备验收挪到 Depot](2026-10-29-android-device-on-depot.md)中关于当前 runner 的决定；原笔记保留 Depot 探测和权限夹具的历史证据。

## Decision

`android-device.yml` 使用 `ubuntu-24.04`。该 runner 的同源 `6bea262fd0` API 35 整套设备作业已经通过，当前门禁继续检查字符设备、读写权限，并在安装模拟器后执行 `emulator -accel-check`。不把 Depot 的单次探测结果当作持续能力承诺，也不把缺少 KVM 时跳过设备测试算作通过。

## Evidence

当前分支的 Depot 作业 `40906511115` / job `123563320499` 在 `Enable KVM for the emulator` 步骤失败，随后所有设备测试均未执行。旧同源 GitHub 作业 `40875035104` 的 API 35 设备作业通过。两者都使用相同的设备测试入口和 x86_64 AVD。

## Alternatives considered

- **继续使用 Depot 并在缺少 `/dev/kvm` 时跳过**：可以减少 GitHub 并发占用，但会把未运行的设备验收伪装成通过，不能接受。
- **继续使用 Depot 并创建或挂载 KVM 节点**：workflow 没有宿主权限，且不能把主机设备能力当作仓库代码可以修复的条件；失败时仍无法证明模拟器真正使用硬件加速。
- **恢复旧的 udev 触发流程**：Depot 的节点不存在时 udev 不能创建它；在 GitHub runner 上也没有必要放弃当前直接权限检查。

## Consequences

设备 job 会重新占用 GitHub Linux 并发，启动时间可能比 Depot 长，但 API 35 的完整输入和崩溃验收有实际通过路径。若以后重新选择 Depot，必须先用完整 workflow 在多个实际分配实例上验证 `/dev/kvm` 和 `emulator -accel-check`，不能只依赖探测 job。

完整输入冒烟的总时限同时从 90 分钟调整为 120 分钟。`40907715096` 的 GitHub runner 已完成 KVM、设备单测、APK、崩溃专项以及 96 项输入套件，最后在英文长按和剪贴板生命周期之前被总时限取消；日志没有套件失败或 instrumentation 卡死。单项仍由 `MSIME_DEVICE_INSTRUMENTATION_TIMEOUT_SECONDS`（默认 300 秒）有界，新增总时限只覆盖整套套件的累计时间。


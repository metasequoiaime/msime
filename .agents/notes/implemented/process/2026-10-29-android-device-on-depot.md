# Agent Note: Android 设备验收挪到 Depot

Status: implemented

## Problem

当前 runner 决定已由[Android 设备门禁回到有实证的 KVM runner](2026-10-30-android-device-kvm-runner-fallback.md)取代；本篇仅保留 Depot 探测、权限夹具和迁移过程的历史证据。

`android-device.yml` 是 GitHub 托管 Linux runner 上最大的占用。[改键盘或设置界面的 PR 跑一遍设备验收](2026-10-28-android-device-on-ui-prs.md)之后，每个改到 Android 界面的 PR 都起一台 API 35 模拟器，整套最长 90 分钟。2026-10-29 上午同时有 13 个这样的 job 在 GitHub 托管的 `ubuntu-24.04` 上跑（11 个 PR 的 API 35，加 develop 推送的 API 28 和 35），另有 8 个 `windows-2025`。组织共用的 20 个托管并发名额（macOS 的 5 个也算在里面）被占满，其余托管 job 只能排队。

它留在 GitHub 上的理由是「模拟器要 KVM，Depot 的 runner 不保证有」（[Android 设备门禁](../testing/2026-10-08-android-device-gates.md) 的 Alternatives，[不碰长期凭据的 job 默认跑在 Depot 上](2026-10-27-depot-runners-by-default.md) 沿用了它）。这一条从来没在 Depot 上验证过。macOS 挪到 Depot 之前有 `depot-macos-probe.yml` 实测，KVM 没有。

## Decision

`android-device.yml` 的 job 跑在 `depot-ubuntu-24.04-8`。权限准备先确认 `/dev/kvm` 是字符设备；当前用户已经可读写时直接使用，否则仅对这一现有节点执行 `sudo chmod a+rw`，随后重新检查实际访问权限。安装模拟器后执行 `emulator -accel-check`，不把节点存在等同于加速可用。权限不依赖 udev：Depot 的静态节点不保证能重新触发设备事件，GitHub 托管 runner 的 0660 节点也可以按同一流程开放当前作业所需权限。

日志采集与停止模拟器只在 `SERIAL` 已设定后运行；初始化之前失败时还没有模拟器，不能让 adb 等待不存在的设备。日志采集先创建输出目录，每条 adb 命令最多等待 30 秒，停止命令最多等待 15 秒，开机失败也不会无限等待设备重新出现。

依据是 #10190 里的一次性探测（`depot-android-kvm-probe.yml`，在两种 runner 上做同一件事，结论记下后在同一个 PR 里删掉），2026-10-29 的结果：

| | `ubuntu-24.04` | `depot-ubuntu-24.04-8` |
| --- | --- | --- |
| CPU、内存 | 4 核、15 GB | 8 核、31 GB |
| CPU 虚拟化标志 | 4/4 | 8/8 |
| `/dev/kvm` | 有，打开权限前 0660 | 有，0666 |
| `emulator -accel-check` | KVM (version 12) is installed and usable | 同左 |
| API 35 AVD 开机 | 45 秒 | 21 秒 |

两边镜像里都有 `sdkmanager` 和 `adb`，都没有预装 emulator，由 workflow 自己装，和原来一样。

探测不保留：`android-device.yml` 自己在每次 PR、推送和 nightly 上起模拟器，Depot 哪天没了 KVM，它会在开机这一步超时报红，比手动触发的探测更早发现。

## Alternatives considered

- **保留 udev 规则并强制触发**：可在节点后来重建时继续维持权限，也是原 GitHub runner 的流程。但 Depot 的实际完整作业在 `udevadm trigger --name-match=kvm` 返回 `Failed to open the device 'kvm': Invalid argument`，发生在安装工具和设备测试之前；先前只验证模拟器加速的探测没有覆盖这一步。本作业只使用 runner 已存在的设备节点，不依赖热插拔，直接检查和修正该节点即可，不忽略 udev 错误后继续冒充准备成功。
- **留在 GitHub，限制 `android-device-pr` 的触发**（比如打标签才跑）：不占 Depot 额度。不采用：Depot 上跑得更快，限制触发会把 10-28 那篇要解决的「合并后才看见界面验收失败」重新带回来。
- **用 `depot-ubuntu-24.04-4`**：更便宜。不采用：这个 job 要在同一台机器上交叉编译四个 client crate 的单测和 APK，同时跑模拟器，8 核 31 GB 开机快一倍，构建也是 8 核的规格；按 [Depot 默认](2026-10-27-depot-runners-by-default.md) 的规格划分，编 Rust 用 `-8`。
- **无条件执行原有 udev 命令**：最容易沿用 GitHub runner 的初始化流程。不采用：Depot 的实际设备访问可用，但 udev 无法枚举该设备；已有权限时仍执行命令会把足够的运行条件变成启动失败。

## Consequences

GitHub 托管的 Linux 名额只剩发版、发布和 Pages 这类持有凭据或用 OIDC 的 job。代价是设备验收也依赖 Depot 的可用性，Depot 出故障时 PR 上的 `android-device-pr`（非必需检查）和合并后的 push、nightly、发版前门禁都会卡住；`release-android.yml` 发版前调用的就是这个 workflow。

## Verification

`actionlint` 通过。这个 PR 改到了 `android-device.yml`，`ci-platforms.yml` 的 `android_device` 会点亮，PR 上的 API 35 整套验收就在 Depot 上实跑一遍。API 28 只在合并后的 develop 推送上跑，合并后看第一次的结果。

崩溃完善分支的固定 `d955936174`、Native Platform CI `40891776963`、设备 job `123510423275` 实际在旧 udev 准备步骤失败；Rust 设备单测、APK 和故障专项均未开始，不能记为设备验收失败或通过。上述权限准备修补需单独核对 Linux 实际设备与拒绝分支，并由下一轮 Depot 作业验证模拟器加速和完整套件；旧 `6bea262fd0` 的 GitHub runner 整作业成功不替代本轮。

修补后的完整 Bash 步骤在 PVE Linux 非 root 用户上直接执行，真实可访问的 KVM 节点保持原身份与权限；私有挂载命名空间内另外验证可访问字符节点、受限字符节点权限修正、普通文件拒绝和缺失节点拒绝，五项均符合预期。所有替换与 chmod 仅作用于命名空间绑定的合成节点，原设备未修改；`actionlint`、Bash 语法和显式 Bash 模式的 ShellCheck 均通过。证据为 `target/kvm-permission-fixture-attempt1.log` 及远端 `target/kvm-permission-verification-d955-attempt1/verification.json`。这不替代 Depot 上的实际加速与完整设备作业，仍待下一轮。

权限修补候选树 `d5c3c96217f4d8cf1603ff7f411ea4dc561f8109` 的整轮 `scripts/verify-local.sh --quick` 退出 0，12,612 个源码及模式条目在开始和结束后均逐项一致。可移植 Windows、契约与笔记、Rust/Android/wasm、Android Java、macOS、Apple bridge 和 x64/x86 pipe-only 通过；工具缺失的目标阶段仍按原规则跳过。证据为 `target/kvm-candidate-quick-evidence/`；随后只补充本篇验证事实，设备加速与整套输入仍待新 CI。

[原失败作业](https://github.com/metasequoiaime/msime/actions/runs/40891799388/job/123510811361)在 KVM 初始化阶段报 `Failed to open the device 'kvm': Invalid argument`；[完整设备路径的复验](https://github.com/metasequoiaime/msime/actions/runs/40894655655/job/123520558785)仍报 `Failed to open the device '/dev/kvm': No such device`，且采集阶段会等待尚不存在的模拟器。原探测只验证 KVM 可用和开机时间，没有覆盖 udev 命令。新的 runner 验收以权限检查和模拟器实际启动为准，不能把原探测当成整个 workflow 已通过。

整合固定 develop `de5d51623a` 时，保留上述已验证的直接字符设备权限修正与加速检查，同时接入上游有界 adb 清理，避免初始化失败后等待未启动的模拟器。整合候选树 `56b8841f3765b02e3994d841eb49fd7d51fb9058` 的整轮 quick 退出 0，12,631 个源码及模式条目前后相同；此后只更新笔记，实际 Depot 加速和完整 API 35 设备作业仍需新 CI 独立验证。

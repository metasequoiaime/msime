# Agent Note: Android 设备冒烟按变更风险选择矩阵

Status: implemented

## Problem

`Native Platform CI` 在 develop 的每次相关推送上都启动 API 28 和 API 35 两台模拟器。最近一次成功运行（`42350776881`）中，API 28 的 `--core` 设备验收耗时约 55 分钟，API 35 的完整验收耗时约 108 分钟。引擎、脚本或其他共享改动也会因为共享路径判定而触发这两台设备，等待时间与改动是否能触及 Android 9 没有关系。

API 28 的核心套件主要守住最低系统版本、沙箱文件权限、资源准备和旧系统行为；API 35 的完整套件覆盖高版本的完整输入、设置和面板交互。两者同时跑对 Android 宿主、资源、JNI 会话和输入运行时改动有价值，但对普通共享改动重复启动 API 28 的收益很低。

## Decision

`ci-platforms.yml` 的 `Decide platform scope` 额外输出 `android_device_matrix`，develop、main 的推送调用可复用的 `android-device.yml` 时把它作为矩阵传入。设备脚本另提供 `--risk`，给合并后的推送用短的资源/存储验收路径：

- Android 资源、清单、版本资源、原生库、宿主存储和设备工具，或 `client-base`、`client-core`、`client-providers`、`client-services`、`input-runtime`、`host-api`、`shared/contracts`、`resources` 的改动，跑 API 28 和 API 35 的 `--risk`：安装、真实主输入、辅助码/词库导入、内置词书、会话存储和剪贴板存储。
- Android Java 和设备测试改动只跑 API 35 的 `--pr` 短验收；其他共享 crate、共享脚本、工作区清单和工具链改动也只跑 API 35 的 `--pr` 短验收。两者都保留一台真实设备的启动、真实输入、键盘绘制和设置入口门禁。
- 不属于 Android 设备运行时的改动不启动 `android-device`。PR 上已有的 Android 界面设备作业使用 API 35 的 `--pr` 短验收；Nightly 继续使用默认完整双 API 矩阵；发版使用[发布冒烟分层](2026-11-10-android-release-smoke.md)。

`android-device.yml` 的默认输入不变，因此 nightly 和未指定矩阵的显式调用仍然是 API 28 `--core` 加 API 35 完整套件；只有 `ci-platforms.yml` 的 develop/main 推送按风险收窄矩阵。`--core` 和无参数完整套件仍负责长 UI 回归；`--risk` 不运行 14/18 键、候选面板和设置长套件。

## Alternatives considered

- **所有推送继续跑双 API**：覆盖最全，但最近一次成功运行表明 API 28 单独占用约 55 分钟、API 35 完整套件还要约 85 分钟；普通 Java 或测试改动没有最低系统或资源变化，重复启动两台设备的边际收益低。没有继续采用。
- **所有推送只跑 API 35 的 `--pr`**：等待时间最短，但会把最低 API 的文件权限、资源准备和 Android 9 回归推迟到 nightly 或发版。没有采用，资源、原生库和宿主存储仍保留 API 28 核心套件。
- **高风险推送继续跑 API 35 完整套件**：覆盖最全，但完整套件把与改动无关的长回归混进 develop 门禁；nightly 已经有完整矩阵。没有继续采用，develop 只保留 `--risk`。
- **高风险推送继续跑 `--core`**：仍会重复运行几十个长 UI 套件，其中已知的 14 键和候选面板失败会遮住资源与存储回归。没有继续采用，改为只运行 `--risk` 的必要路径。
- **完全取消 develop 推送上的设备验收，只留 nightly**：最省 runner 时间，但合并后不能及时发现真实设备上的回归。没有采用，至少保留一台 API 35 的端到端门禁。

## Consequences

- 普通 Java、设备测试和共享改动只启动一台 API 35 模拟器并跑 `--pr`；按最近一次成功运行估算，设备墙钟时间从约 108 分钟降到短验收的单台时长。资源、原生库和宿主存储改动仍需两台设备的 `--risk`，避免把 API 28 的文件权限和资源启动问题推迟到发版，同时不在 develop 重复运行几十个长 UI 套件。
- 资源、JNI 会话、输入运行时和文件权限相关改动的双 API 覆盖保持不变；Java 和设备测试改动的 API 28 覆盖改由 nightly、Release 和本机验证承担。
- 设备矩阵判定成为新的 CI 契约；新增会影响 Android 资源、原生库、宿主存储或输入运行时的共享路径时，必须加入高风险列表，否则只会跑 API 35 的 `--pr`。Nightly、Release 和本机验证仍可覆盖完整双 API。
- 当前 develop 仍有 `ci-red` 跟踪单 `#11169`，本改动不能绕过红灯；合并前要在 issue 中说明上一轮 API 35 失败与本次 CI 范围调整无关，并等待新的完整设备运行恢复。

## Verification

- `actionlint .github/workflows/ci-platforms.yml` 通过。
- `git diff --check` 通过。
- 未在本机启动 Android 模拟器：当前磁盘剩余低于仓库规定的 50 GB 构建门槛；设备矩阵的最终展开需由 PR 上的 CI job 验证。

2026-11-10：完整 API 35 冒烟在冷构建后还要运行约 85 分钟，且 `--core` 仍包含几十个长 UI 套件。Java 和设备测试改动的 develop 矩阵继续用 API 35 单台 `--pr`；资源、原生库和宿主存储改动的两台设备改用 `--risk`，只保留安装、主输入、资源导入、会话存储和剪贴板存储回归。Nightly 继续使用完整双 API 矩阵；发版分层见[发布冒烟](2026-11-10-android-release-smoke.md)。

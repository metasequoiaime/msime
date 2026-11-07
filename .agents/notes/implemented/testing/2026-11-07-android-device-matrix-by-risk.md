# Agent Note: Android 设备冒烟按变更风险选择矩阵

Status: implemented

## Problem

`Native Platform CI` 在 develop 的每次相关推送上都启动 API 28 和 API 35 两台模拟器。最近一次成功运行（`42350776881`）中，API 28 的 `--core` 设备验收耗时约 55 分钟，API 35 的完整验收耗时约 108 分钟。引擎、脚本或其他共享改动也会因为共享路径判定而触发这两台设备，等待时间与改动是否能触及 Android 9 没有关系。

API 28 的核心套件主要守住最低系统版本、沙箱文件权限、资源准备和旧系统行为；API 35 的完整套件覆盖高版本的完整输入、设置和面板交互。两者同时跑对 Android 宿主、资源、JNI 会话和输入运行时改动有价值，但对普通共享改动重复启动 API 28 的收益很低。

## Decision

`ci-platforms.yml` 的 `Decide platform scope` 额外输出 `android_device_matrix`，develop、main 的推送调用可复用的 `android-device.yml` 时把它作为矩阵传入：

- Android 宿主 Java、资源、清单、版本资源、设备套件，或 `client-base`、`client-core`、`client-providers`、`client-services`、`input-runtime`、`host-api`、`shared/contracts`、`resources` 的改动，继续跑 API 28 `--core` 加 API 35 完整套件。
- 其他共享 crate、共享脚本、工作区清单和工具链改动只跑 API 35 的 `--core` 套件，保留一台真实设备的核心端到端门禁，省掉 API 28 模拟器和 API 35 的非核心长套件。
- 不属于 Android 设备运行时的改动不启动 `android-device`。PR 上已有的 Android 界面设备作业仍使用原来的 API 28 崩溃专项加 API 35 完整矩阵；Release Android 继续使用可复用 workflow 的默认双 API 矩阵。

`android-device.yml` 的默认输入不变，因此发版和显式调用仍然是双 API；只有 `ci-platforms.yml` 的 develop/main 推送按风险收窄矩阵。设备验收项目、断言、单项超时和失败收集均不变。

## Alternatives considered

- **所有推送继续跑双 API**：覆盖最全，但最近一次成功运行表明 API 28 单独占用约 55 分钟；普通共享改动没有最低系统或 Android 资源变化，重复启动它的边际收益低。没有继续采用。
- **推送只跑 API 35**：等待时间最短，但会把最低 API 的文件权限、资源准备和 Android 9 回归推迟到 nightly 或发版。没有采用，保留高风险路径的 API 28 核心套件。
- **完全取消 develop 推送上的设备验收，只留 nightly**：最省 runner 时间，但合并后不能及时发现真实设备上的回归。没有采用，至少保留一台 API 35 的端到端门禁。

## Consequences

- 普通共享改动只启动一台 API 35 模拟器并跑核心套件；按最近一次成功运行估算，设备墙钟时间可从约 108 分钟降到核心套件的约 55 分钟，节省约 50 分钟。
- Android 宿主、资源、JNI 会话、输入运行时和文件权限相关改动的双 API 覆盖保持不变。
- 设备矩阵判定成为新的 CI 契约；新增会影响 Android 运行时的共享路径时，必须加入高风险列表，否则只会跑 API 35。Release 和本机验证仍可覆盖完整双 API。
- 当前 develop 仍有 `ci-red` 跟踪单 `#11079`，本改动不能绕过红灯；合并前要等对应设备作业在新的 develop 基础上恢复并通过。

## Verification

- `actionlint .github/workflows/ci-platforms.yml` 通过。
- `git diff --check` 通过。
- 未在本机启动 Android 模拟器：当前磁盘剩余约 22 GB，低于仓库规定的 50 GB 构建门槛；设备矩阵的最终展开需由 PR 上的 CI job 验证。

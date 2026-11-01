# Agent Note: Android 崩溃与 ANR 采集

Status: implemented

## Problem

Android 的 Java/JNI、系统退出和 ANR 路径原先没有统一的持续授权代次；系统延迟诊断可能落入新授权，现场 Java 致命信号也可能在进程退出后丢失。发布 APK 的原生模块还需要保留精确构建身份，才能用相对偏移恢复源码行。

## Decision

Android 通过 Host API 读取共享授权代次和偏好目录，Java 层在 App 与键盘进程分别准备捕获身份，JNI 层导出 consent、现场记录和解除武装入口。系统 `ApplicationExitInfo` 只接受本进程曾保存的短身份；真实故障与现场记录使用同一诊断身份，ANR 和低内存终止单独分组，撤回后不再收取旧诊断。系统退出摘要只保存固定类型、状态码和时间，不读取 tombstone、日志或输入文本。

构建脚本为每个 ABI 保留 Rust/JNI 的调试符号并归档构建清单；API 35 设备工作流在完整输入冒烟前独立运行 crash-only 门禁，避免既有 UI/设备故障掩盖崩溃链路。发布工作流上传并长期保存原生符号，测试使用合成身份和故障数据。

## Alternatives considered

- **只依赖 `ApplicationExitInfo`**：它无法覆盖现场致命信号，也不能保证扩展会话归属；因此保留 Java/JNI 现场记录并用系统退出做延迟补录。
- **用当前偏好覆盖历史诊断**：会把撤回前的故障归入新授权；因此保存 epoch、起始时间、进程组件和构建身份并逐条校验。
- **让完整输入冒烟代表崩溃门禁**：UI 套件的设备故障会掩盖崩溃专项结果；因此在 API 35 上先跑独立 `--crash-only`。
- **发布后重新从剥离的 ELF 生成符号**：会丢失 Rust/JNI 的精确行表；因此在剥离前归档各 ABI 的原始符号和清单。

## Verification

- Android shell 脚本、Python 设备夹具和构建身份工具通过语法/差异检查；设备工作流新增 API 35 crash-only 门禁与符号 artifact。
- `smoke.sh` 显式解析 `--crash-only`，让 API 35 的独立崩溃门禁确实进入专用构建分支，而不是在参数校验阶段退出。
- `--crash-only` 在安装与选择输入法后只运行 `telemetry-watch.py`、`telemetry-consent.py` 和 `telemetry-anr.py`，随后结束设备步骤；系统退出脚本按实际首条事件 ID 去重，避免未定义变量让专项在重启断言前失败。
- 本机没有 Android SDK/模拟器，且磁盘剩余不足以新建构建树，未把静态检查当作 Android 编译或设备通过；由 Android CI 在 API 28/35 运行 Java/JNI、系统退出、ANR、授权撤回和安装验收。
- develop 已有 Android API 28 `ci-red` 跟踪单；本切片保留独立 crash-only 结果，不把既有完整输入冒烟失败算作采集通过或失败。

## Consequences

Android 记录携带的身份足以在对应 ABI 符号包下离线还原，撤回授权后旧进程不会污染新队列。设备工作流时间略有增加，但崩溃专项结果不再被完整 UI 冒烟覆盖；真机和不同 ABI 仍需 CI/维护者设备复验。

共享语义见[共享崩溃记录基础](../../implemented/architecture/2026-11-01-shared-crash-reporting-core.md)。

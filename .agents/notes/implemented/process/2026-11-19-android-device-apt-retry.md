# Agent Note: Android 设备 job 的 Ubuntu 包源重试与回退

Status: implemented

## Problem

`#11679` 的 Native Platform CI 在 Android API 28 和 API 35 job 的设备步骤开始前失败。两个 job 都停在 `sudo apt-get` 安装 `libxml2-utils`，日志显示 GitHub runner 访问 `azure.archive.ubuntu.com` 超时，后续的 NDK、模拟器和设备验收没有执行。单纯重跑一次仍可能把同一个临时镜像故障传播到两个矩阵项。

## Decision

Android 设备 workflow 在安装工具时先对原 Ubuntu 镜像执行四轮带退避的 `apt-get update` 与安装，每次 apt 请求同时启用三次 URI 级重试。四轮仍失败后，只在当前 job 的 runner 上把 APT 源中的 `azure.archive.ubuntu.com` 替换成 Ubuntu 公共镜像 `archive.ubuntu.com`，再执行同样的重试流程。只有包安装最终成功才继续 NDK 和模拟器准备；失败仍让 job 失败并保留原始 apt 输出。

## Alternatives considered

- **只增加一次 workflow 重跑**：最强理由是改动最小，但无法改变镜像持续不可达时的结果，也不能保证两个 API 矩阵采用相同恢复路径，因此不采用。
- **永久改用 `archive.ubuntu.com`**：最强理由是绕开当前区域镜像，但会丢掉 GitHub runner 默认的就近镜像选择，扩大所有正常运行的网络路径变化，因此只在原镜像重试耗尽后回退。
- **跳过 `libxml2-utils` 安装**：最强理由是可避免 APT 网络依赖，但后续 Android 构建和设备脚本把 XML 工具作为前置条件，跳过会把确定的缺依赖变成更晚、更难诊断的失败，因此不采用。

## Consequences

短暂的 Ubuntu 镜像抖动会在 job 内自动恢复，区域镜像持续故障时仍有公共镜像兜底，API 28/35 不会因为同一环境瞬时故障一起在设备验收前退出。代价是最坏情况下安装阶段增加退避等待，并在 runner 的 APT 源文件上做一次临时修改；修改只存在于该 job，不改变仓库或发布环境。

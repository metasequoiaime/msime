# Agent Note: Linux 词库升级刷新保留运行时配置文件名

Status: implemented

## Problem

`msime-linux-setup --update --download` 会把新资源目录写进状态目录旁的隐藏临时文件，再调用随包提供的 `msime-linux-prepare --refresh`。临时文件名是 `.runtime-options-update-*`，而刷新入口的参数契约和各平台状态布局都以 `runtime-options.json` 为配置文件。已有 setup 测试的桩没有约束文件名，因此没有覆盖真实安装包入口对这个布局的依赖；失败时用户只能看到“Dictionary refresh failed”。

## Decision

升级刷新在状态目录下建立一个随机的临时子目录，并在其中写入名为 `runtime-options.json` 的副本。刷新程序成功后，把这个文件通过同一文件系统的原子 `rename` 替换正式配置；失败或异常时递归删除临时目录。原有词库目录和正式配置在失败路径都保持不变。

## Alternatives considered

- **继续把隐藏临时文件直接放在状态目录中**：改动最小，但把安装器内部的临时名字暴露给独立发布的刷新入口，现有桩也无法发现这项布局不一致，因此不采用。
- **让 `msime-linux-prepare --refresh` 接受任意文件名**：可以放宽入口，但会扩展一个本来围绕 `runtime-options.json` 设计的状态文件契约，且不能帮助旧版随包程序，因此不采用。

## Consequences

刷新入口始终看到标准的 `runtime-options.json` 文件名，同时临时副本仍与正式配置位于同一文件系统，成功替换保持原子性。临时目录在成功、失败和异常路径都会清理；如果进程在清理前被强制终止，下一次刷新不会读取它，目录仍可能由用户手动清理。测试桩现在拒绝非标准文件名，并验证成功后没有中间目录残留。

## Verification

`python3 platforms/linux/tests/core/setup_update.py` 通过；它覆盖过期词库下载、会话锁、刷新失败保留旧配置、临时文件名和残留清理。Host API 的现有 `refresh_*` 单测继续通过；Linux 原生容器门禁需在具备 Docker 的 Linux 环境运行。

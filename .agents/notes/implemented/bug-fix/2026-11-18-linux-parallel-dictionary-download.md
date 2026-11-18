# Agent Note: Linux 首次配置并行下载词库

Status: implemented

## Problem

`msime-linux-setup --download` 会按词库锁逐个下载 12 个独立文件。当前锁包含约 303 MiB 资源，镜像或 GitHub 的单连接带宽会把首次配置时间拉长，网络较慢时用户会误以为安装器卡住。

## Decision

安装器在同一个暂存目录中最多并行下载三项词库。每一项仍按镜像优先、锁定原地址回退和 SHA-256 校验执行，所有任务完成且全部通过后才把暂存文件改名发布到目标目录。任一任务失败时暂存目录由上下文清理，目标目录保持原状。

## Alternatives considered

- **继续逐个下载**：实现最简单，也保持了现有网络并发；但所有文件共用一条连接，首次配置时间完全受最慢单项限制。
- **把词库重新打成一个归档**：可以只建立一次连接；但需要改变发布资产、锁文件和手动下载清单的契约，现有安装包无法立即使用。

## Consequences

独立资源可以同时利用镜像或 GitHub 的可用带宽，首次配置等待时间缩短。暂存和原子发布边界保持不变，单个下载失败不会留下半份词库。并发连接数固定为三，避免把安装器变成无限制的请求发起方。

## Verification

- `python3 platforms/linux/tests/core/setup_resolution.py`
- `python3 scripts/test-linux-setup-download-links.py`
- `python3 -m py_compile platforms/linux/scripts/msime-linux-setup`

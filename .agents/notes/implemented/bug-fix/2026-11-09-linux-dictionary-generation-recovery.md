# Agent Note: Linux 词库代次失配时恢复到安装包资源

Status: implemented

## Problem

Linux 覆盖升级后，首次配置或词库更新可能仍拿旧版本在线下载的代次作为显式资源目录。更新记录里的 `bundled_resources` 也可能是用户首次下载词库的目录，不是当前安装包目录。当前词库锁已经变化时，Host API 会拒绝这份目录；`msime-linux-prepare` 因而连空状态目录都无法建立，输入法刷新也只能保留旧代次。

## Decision

Linux 宿主从 `/proc/self/exe` 解析当前安装包的资源目录，并通过 Host API 的可选 `bundled_resources` 兜底入口传递它。准备流程先保留显式资源目录；只有它不符合当前锁时，才验证并使用安装包目录。显式传入的安装包目录优先于旧 `installed.json` 记录，避免把用户下载目录当成随包资源。

Linux 的刷新入口使用新增的 `msime_client_refresh_host_from`，仍沿用宿主刷新时的语言词库同步规则。旧的 `msime_client_refresh_host` 保持原有行为，空兜底参数时与以前相同。首次准备和刷新共用同一份安装包路径解析，状态目录与运行时配置仍通过原有原子发布流程写入。

## Alternatives considered

让 `msime-linux-setup` 在 `--refresh` 失败后直接改写 `runtime-options.json`，可以避免新增 C ABI，但会把资源切换和用户词库回放拆成两条流程，设置脚本也无法复用输入法宿主的原子刷新逻辑，因此不采用。

只继续读取 `installed.json` 的 `bundled_resources`，改动更小，但首次在线下载记录的目录可能不是安装包目录，正是本问题的触发条件，因此不采用。

## Consequences

旧在线代次失配时，Linux 首次配置会回到当前安装包资源，覆盖升级后的 IBus、Fcitx5 和 `msime-linux-setup --update --download` 都能走同一条恢复路径。新增的 bootstrap 字段和刷新函数保持 ABI 版本不变；其他平台不传兜底路径时行为不变。

验证覆盖了合成资源目录的失配回退、Host API 编译、Fcitx5 合约、Linux 更新脚本和 `git diff --check`。本机没有可用的 Linux Docker daemon，Linux 原生 C++ 容器构建未运行；全量 `msime-host-api` 单测另有 develop 基线失败：资源包列表期望值缺少 `local-translation`，与本改动无关。

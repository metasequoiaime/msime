# Agent Note: Linux 剪贴板自启动不阻塞桌面登录

Status: implemented

## Problem

首次配置下载词库后会启用 `msime-linux-clipboard.service`。在没有 `graphical-session.target` 的桌面会话里，XDG 自启动项负责把本次会话的显示环境交给用户级 systemd，并重启这个服务。原自启动命令同步执行 `systemctl --user restart`，而服务单元声明了 `After=graphical-session.target`。`systemctl` 默认等待排队的启动任务完成；如果登录阶段还没有完成该 target，自启动进程会一直等服务，桌面登录也可能被它拖住（#10780）。

## Decision

XDG 自启动项使用 `systemctl --user --no-block restart msime-linux-clipboard.service`。重启任务仍由用户级 systemd 排队并继续执行，但登录进程不再等待服务跨过 `graphical-session.target`。已经启动该 target 的会话仍在前置检查处直接退出，未启用服务的用户也不受影响。契约测试同时钉住 `--no-block` 和调用顺序。

## Alternatives considered

- **只删除 XDG 自启动项**：可以避开登录阶段的等待，但 Sway、i3 和未用 uwsm 的 Hyprland 没有 `graphical-session.target`，这些会话会失去剪贴板监视器的登录启动能力，因此不接受。
- **删除服务的 `After=graphical-session.target`**：会改变服务在标准图形会话中的启动顺序，仍不能保证自启动时显示环境已经导入；问题只需要解除调用方的同步等待，不需要放宽服务依赖。

## Consequences

- 登录阶段只负责把服务任务交给用户级 systemd，不再把服务启动时间算进桌面登录时间。
- 在服务真正启动前退出登录时，systemd 会按正常的用户服务规则处理排队任务；剪贴板服务本身的重试和显示环境导入逻辑保持不变。
- 本机没有可用的 Linux 桌面或 Docker daemon，未完成 Ubuntu 26.04.1 实机登录复验；已用 systemd 官方文档确认 `systemctl` 默认等待、`--no-block` 只入队不等待，并用自启动契约测试覆盖命令。

# Agent Note: Linux 语音服务测试等待套接字就绪

Status: implemented

## Problem

`provider_degraded_start.py` 启动语音服务后，只等待 Unix socket 路径出现就开始连接。服务先 `bind` 创建路径，再 `listen` 接受连接；构建负载较高时，测试可能在这两个动作之间连接，出现 `ConnectionRefusedError`。退出登录相关 Pull Request 的本地 quick 门禁曾在该测试失败，同一构建产物的定向复跑通过。

## Decision

启动等待改为在十秒期限内尝试连接 Unix socket，只有连接成功才认为服务就绪。尚未监听导致的拒绝连接或路径消失会继续轮询；进程提前退出仍立即报告实际错误。测试连接失败时关闭 socket，避免泄漏文件描述符。

## Alternatives considered

- 增加固定等待时间：机器负载不同，无法保证不会再次撞到 `bind` 与 `listen` 之间的窗口。
- 只增加后续连接的重试次数：启动等待仍会过早返回，其他测试调用也会遇到同一竞态。

## Consequences

测试以服务可连接为就绪条件。启动失败仍受原有十秒期限限制。

## Verification

在 Linux 构建容器内连续运行该测试 20 次，80 个测试方法全部通过；本地 quick 门禁见对应 Pull Request。

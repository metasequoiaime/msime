# Agent Note: CI 路径路由契约

Status: implemented

## Problem

`ci-docs.yml` 与三个必需检查的路径表依靠人工保持一致。
路径表失配会让必需检查停在 `Expected`。
平台内部的 Bash 分类也缺少自动回归测试。

## Decision

`scripts/test-ci-path-filters.py` 核对两种事件的四份路径表。
它还确认许可证、资源锁和构建脚本不会进入文档入口。

`scripts/test-ci-platform-routing.py` 从 workflow 读取实际 Bash 分类代码。
它用合成路径运行分类代码，再检查各平台的输出。
目录与预期路由共用一张表。新增平台目录时，测试要求增加路由断言。
每个样例核对全部十个原生 CI 输出，包括未触发的作业。

两项检查都使用 Python 标准库和 Bash。
`scripts/run-checks.sh` 自动发现并运行它们。

## Alternatives considered

- 把 Bash 分类规则全部改写成 Python。此方案需要同时改变四份 workflow 的运行逻辑。当前测试先固定现有行为。
- 只扫描 Bash 的路径模式。此方案不能检查顺序、共享标志或最终输出。

## Consequences

改动入口路径表或平台路由时，维护者要更新相应测试。
测试在 Linux 上运行，不需要平台 SDK。
测试只覆盖列出的合成路径；新构建输入仍需添加对应断言。

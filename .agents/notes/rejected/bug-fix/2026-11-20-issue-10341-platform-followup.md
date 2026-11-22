# Agent Note: 跟踪平台 issue #10341

Status: rejected — issue #10341 已由维护者以 completed 关闭，隔离对照未复现缺陷，只由 #10380 补上模型加载诊断日志，无需单独跟踪。

## Problem

issue #10341 报告 macOS 0.55.0 打开「桌面神经联想」并装好 `settled-model` 后模型从不加载，依据是模型文件 atime 不变，而 CLI 显式给路径时可以加载。维护者核对了 `macos-v0.55.0` 与 develop 的 `preferences_directory` 回退路径，并经静态链接的 Host API C ABI 做了隔离对照：自动查找、显式路径、启动后安装三种布局都加载了模型并改变首选，没有复现差异；同时确认 APFS 上普通读取不一定更新 atime，不能据此判断是否加载。#10380（`fix(host-api): 诊断日志记录落定重排模型是否接上`，正文写 `Refs #10341`）于 2026-10-31 合入，在诊断日志里记录 `host_api: settled model attached / not loaded / missing / missing without state directory`，随 macOS 0.56.0 发布。维护者请报告者升级后贴日志，报告者没有回复，issue 于 2026-11-02 以 completed 关闭，没有关联的修复 PR。本笔记原本是为该 issue 建立的独立跟踪提案，现记录为不采用。

## Proposal

不再为 #10341 保留独立跟踪 PR。原因是没有可复现的缺陷可修，可观测性缺口已由 #10380 补上；若之后有报告者贴出含 `host_api: settled model` 行的日志并显示真实问题，另开新 issue 处理。

## Alternatives considered

- **继续保留 Draft 跟踪 PR**：最强理由是报告者的原生 IMK 进程始终没有被直接验证，但 issue 已关闭且没有新证据，空跟踪只会成为过期提案，因此不采用。
- **按报告者猜测修改 `preferences_directory` 路径处理**：最强理由是与报告症状直接相关，但隔离对照显示自动路径与显式路径行为一致，猜测性修复没有依据，因此不采用。

## Acceptance criteria

- 本笔记位于 `rejected/`，状态行写明 #10341 的真实处理结果。
- 引用的 #10380 与 issue 关闭方式与 GitHub 记录一致。

## Risks

若报告者的环境确有未复现的问题，关闭跟踪后只能依赖新的 issue 和 #10380 的诊断日志重新发现，不会自动追回。

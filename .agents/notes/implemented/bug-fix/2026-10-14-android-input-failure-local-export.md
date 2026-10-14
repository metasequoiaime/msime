# Agent Note: 输入失败类别进入本地诊断包

Status: implemented

## Problem

#7837 的输入失败分类只写 logcat，用户导出的诊断 ZIP 没有这些原因。#7806/#7809 的两份附件各保留 5,000 条输入事件和性能记录，崩溃记录都是零；只有按键、选词、退格、结束及候选耗时，不能把输入连接失败归因到编辑器，也不能凭空崩溃记录排除系统终止。事件已截断，不能当作完整故障历史。

## Decision

在既有日志与隐私开关允许时，停止输入会话之前写入四种固定本地事件：`input_failure_runtime_rejected`、`input_failure_invalid_response`、`input_failure_editor_rejected`、`input_failure_jni_link`。复用时间戳与枚举，不增加自由文本、异常说明、堆栈、输入、路径或应用身份。共享诊断包保留这些事件；Android 上传读取和最终编码都只允许原有十种云端事件。该实现已在 `MSIMEInputService.fail` 中接入，旧失败停止与直接输入回退保持不变。

服务端实际默认分支 `main` 的固定提交 `87b3300e6f5e35b25098a9f7dd60c4d364ae117e` 中，`internal/account/diagnostics.go` 仍只认可十种事件，未知 kind 会拒绝整份请求。因此本次明确支持本地 ZIP，上传不携带新事件，不假设未发布的服务端变更已上线。失败恢复行为与用户日志默认开关保持原样。

## Historical audit

- [输入失败类别](../../implemented/bug-fix/2026-10-14-android-input-failure-categories.md)：部分取代其只写 logcat 的范围；保留分类、隐私与禁止重放的理由，双方互链。
- [原生诊断包上传](../../implemented/bug-fix/2026-10-14-android-diagnostics-native-bundle-upload.md)：部分重叠，保留运输、读取上限与云端类别契约。
- [反馈页连续输入](../../implemented/testing/2026-10-14-android-feedback-input-stability.md)：无关，不改变反馈提交和输入验收。

## Alternatives considered

- 同时扩展服务端事件枚举：上传可直接定位，但需要另一仓库的发布和客户端兼容顺序，未部署前会导致上传失败，本片先交付手动 ZIP 证据。
- 把恢复性故障写成 crash：现有运输已支持异常类型，但会把可恢复输入故障误报为崩溃，丢失事件时间线语义，不采用。
- 只保留 logcat：无需调整格式，但用户已提供的标准诊断附件仍无法携带原因，不能补齐这里的缺口。

## Verification

安装产品的固定分类经真实本地事件写入、JNI 诊断 ZIP 导出可保留四种类别；同一 ZIP 的上传编码仍只含原十种事件，直接构造上传事件也不能绕过。合成私人标记与未知类别被拒绝。Rust 诊断测试 12 项通过；API 28 安装产品的失败分类、诊断包、反馈页连续输入、过期候选与核心剪贴板冒烟通过；client-core API 28 设备全套重跑 1,371 项通过；`check-host.sh`、`git diff --check` 与 `pnpm run verify-notes` 通过。一次先跑的 1,370/1,371 失败是既有的秒级备份碰撞随机性，重跑消失。

## Consequences

日志未开启或隐私模式禁止记录时仍不会记录；本次不能恢复旧附件的缺失原因，也不能证明原设备根因。云端仍没有这些失败类别，本地诊断包的事件白名单需要与上传白名单明确区分。

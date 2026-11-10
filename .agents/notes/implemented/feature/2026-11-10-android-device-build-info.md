# Agent Note: Android 设备信息包含版本与构建时间

Status: implemented

## Problem

#11109 反馈 Android 键盘功能菜单的「设备信息」只能复制版本号，nightly 或本地构建难以从反馈文本定位具体构建；英文入口也需要提供同样的信息。

## Decision

把 Gradle 构建时间以 ISO-8601 写进 `BuildConfig.BUILD_TIME`：优先采用 `MSIME_BUILD_TIME`，否则使用当前 Git 提交时间。设备信息的「应用版本」字段加入 `构建时间`/`Build time` 换行内容，并加入可单独插入的字段列表；「全部」和中英文版本自动复用同一格式。

## Alternatives considered

- 使用设备安装时间：安装时间不是构建时间，更新、恢复备份和重装会误导定位，因此不采用。
- 直接使用构建机器当前时钟：同一提交在不同 CI 重试中会产生不同文本，降低可追溯性，因此默认使用提交时间并保留显式构建环境覆盖。

## Consequences

旧包或直接 javac 测试没有 `BuildConfig.BUILD_TIME` 时仍显示原有版本信息；正式 Gradle 包和 nightly 包会在反馈信息中带 ISO-8601 构建时间。设备信息面板多一个「应用版本」入口，全部字段仍保持 12 格以内。

## Verification

运行 Android host 检查、笔记校验、差异格式检查和 quick 门禁；API 28/35 设备 smoke 需在设备 job 中确认中英文「全部」与单项插入内容。

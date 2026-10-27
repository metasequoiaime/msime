# Agent Note: Android 开发者选项入口与日志总开关

Status: implemented

## Problem

Android 已有开发者选项页，但关于页没有入口；调试信息、输入日志和系统日志级别彼此独立，无法一次关闭相关调试功能，且默认日志级别为 `warn`，不符合 issue 对开发调试的要求。

## Decision

在本机设置中增加 `DEVELOPER_LOGGING_ENABLED`，默认开启。开发者页新增日志总开关，并在关闭时禁用调试信息与输入日志控件；键盘运行时也用总开关门控这两项行为。系统日志默认级别改为 `debug`。关于页新增开发者选项导航，页面搜索关键词同步登记。

## Alternatives considered

- 只隐藏开发者页的控件：运行中的键盘仍会继续显示调试信息或记录输入日志，不能满足总开关语义。
- 让总开关覆盖错误日志：错误日志仍用于发现崩溃和故障，保留现有 `ImeLog` 的错误输出，仅关闭相关调试功能。

## Verification

`git diff --check` 通过；更新了 `AndroidLocalSettingsSmoke` 的默认值和本机设置不参与同步断言。受本机剩余磁盘 45 GiB 限制，未启动新的完整 Android 构建或模拟器验收。

## Consequences

用户可以从关于页直达开发者选项，并用一个开关关闭调试覆盖层与输入日志；关闭后系统日志仍保留错误级别输出。现有用户的日志级别默认值会按设置规范在新安装或未显式保存时变为 `debug`，已保存的用户值不被覆盖。

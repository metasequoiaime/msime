# Agent Note: Android 文本最小尺寸策略别名

Status: implemented

## Problem

`home/Ui.java` 在最近的 Android 策略复用中改为调用 `ViewPolicy.setTextMinHeight` 和 `setTextMinWidth`，但共享策略只提供了等价的 `setMinimumHeight` 和 `setMinimumWidth`。`check-host.sh` 不编译 `home/`，所以这项错误直到 Gradle 全宿主编译才暴露。

## Decision

在 `ViewPolicy` 增加文本最小高度和宽度的命名别名，统一转发到已有的双约束实现，保持 `Ui` 的调用契约和文本控件的 `setMin*`、`setMinimum*` 同步行为。

## Alternatives considered

- **修改 `Ui` 改用已有名称**：也能修复编译，但会让已经由策略检查脚本约束的文本专用调用契约失去统一命名。
- **复制一套最小尺寸实现**：会重复双约束逻辑，增加后续漂移风险。

## Consequences

Android 各 edition 的 Gradle Java 编译能够解析 `Ui` 的策略调用；既有最小尺寸行为不变。

## Verification

CI 首次失败日志确认缺失方法；补充别名后重新运行本地 Android 主机检查、quick 门禁和笔记校验。远端 PR 需要重新执行 Android Gradle 编译和设备任务。

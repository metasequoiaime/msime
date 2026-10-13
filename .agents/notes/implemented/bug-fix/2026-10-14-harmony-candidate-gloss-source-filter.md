# Agent Note: HarmonyOS 候选释义按来源过滤

Status: implemented

## Problem

HarmonyOS 的候选翻译查询只保留候选文字，生成离线释义请求时又把所有来源改成 `source: 0`。脚本插件、表情、颜文字和日文混输等来源的行因此可能被当成普通候选查询或显示释义。

## Decision

保留 HarmonyOS 查询和离线释义请求中的候选来源，统一按共享来源集合（6、7、13、14）判定禁止释义。特殊来源在候选栏和展开面板中隐藏释义，即使同文字的普通候选获得了共享的译文；离线 Host API 对这些来源保留空槽以维持位置对齐。

## Alternatives considered

- **只过滤在线请求**：离线词典仍会为特殊行生成结果，且同文字普通行的结果会通过按文字应用扩散到特殊行。
- **按文字把特殊行从查询中删除**：回复无法再和候选位置对齐，且同文字行仍需要消费端的来源门控。

## Consequences

插件、表情、颜文字和日文混输候选不会送往候选释义服务，也不会在 HarmonyOS 候选显示中出现释义。普通候选仍可正常查询；来源数组继续作为候选协议的一部分传递。

## Verification

新增 HarmonyOS 策略冒烟断言和 Rust Host API 的来源空槽处理。`git diff --check` 通过；本机缺少 `apps/desktop/node_modules/.bin/tsc`，`platforms/harmony/tests/run.sh` 因此跳过编译。

# Agent Note: macOS 按键恢复焦点时恢复共享偏好监视

Status: implemented

## Problem

#7829 的最新反馈是设置只对新输入会话生效，旧微信和 Excel 会话仍用旧配置。源码中 `deactivateServer:` 停止偏好轮询但保留会话；`handleKeyEvent:client:` 可以独立恢复客户端焦点，却没有恢复轮询。中文旧会话无需再次准备，之后没有重新建立轮询的入口。报告中的实际 IMK 回调时序尚未证实。

## Decision

按键入口接管客户端时调用 `startPreferencesMonitoring`，恢复现有偏好监视，沿用后台读取、客户端代次保护和成功修订号去重。中文与英文旧会话都通过同一入口恢复；不需要重建会话。读取代次测试手动控制后台读取，不启动定时器；另一个回归专门覆盖监视生命周期。

## Alternatives considered

- 每个按键同步读取配置：可以让第一个键使用最新设置，但会把文件读取放进输入热路径；现有首次同步与后台轮询已有明确职责，只补齐焦点入口即可。
- 改写 `runtime-options.json`：能让下次新建会话拿到最新初始配置，但已有会话不会重新读它，无法修复缺失的轮询生命周期。

## Verification

`TestKeyFocusRestoresPreferenceMonitoring` 先建立监视，再调用真实失焦入口，保留原会话；随后只发送修饰键释放事件恢复客户端。修前在定时器有效性断言失败，修后中文、英文两种状态都能立即应用第二个修订号，并由自动轮询应用第三个修订号。没有手动 reload 或触发 timer，也没有重新准备会话。测试只替换未初始化 IMK 基类的失焦实现，生产控制器的失焦与按键入口完整执行。

原生输入法 bundle 构建成功，`shortcut` 与 `shortcut-translations` 全量通过；读取代次、失败重试、迟到失焦保护仍通过。未安装替换用户正在使用的输入法，未在微信或 Excel 实际会话中验证。

## Consequences

恢复入口多一次异步读取，保留的会话可以继续收到新偏好；不能据此断言微信或 Excel 的现场回调时序已复现。保持默认 timer mode，不混入未经证实的 run-loop 改动。

偏好读取成功去重与失败重试继续遵守[修订号重试](2026-10-13-macos-preference-revision-retry.md)；旧客户端迟到失焦不能停止新客户端监视的约束见[迟到失焦](2026-10-10-macos-stale-deactivate-paired-closing.md)。

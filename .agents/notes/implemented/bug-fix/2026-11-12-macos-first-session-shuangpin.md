# Agent Note: macOS 首个会话采用最新双拼方案

Status: implemented

## Problem

macOS 输入法进程启动后，设置应用或账号同步可能已经把共享偏好文档改成双拼，但 `runtime-options.json` 仍保存进程启动时的全拼副本。激活时外观对象会先同步到共享文档，因此菜单显示双拼；随后 `prepareSession` 若直接把旧运行时副本交给 Engine，首个会话仍按全拼处理按键，直到后台偏好轮询完成。这个窗口会让用户看到方案已经是双拼，却在首次输入时得到全拼行为。

## Decision

`prepareSession` 首次创建会话时，在构造 `MSIMESessionOptions` 前根据运行时选项中的绝对 `preferences_directory` 同步读取当前共享偏好文档。成功读到合法 `preferences` 对象时，只替换本次会话选项里的 `preferences`，保留运行时选项中的资源目录、版本和宿主能力字段；读取失败时继续使用原有 `runtime-options.json` 副本。这样共享偏好文档仍是方案的权威来源，且不改变运行时选项文件格式。

## Alternatives considered

- **等待已有的每秒偏好轮询**：实现最小，但首个按键仍可能在轮询前进入旧方案，不能满足输入法激活后立即可用的时序要求。
- **只把共享文档的方案写回 `runtime-options.json`**：能让后续会话读到新值，但增加发布文件的写入竞态，并把输入法首次激活的正确性依赖到另一条持久化流程；本次只在会话创建边界读取当前文档。
- **只更新 `MSIMEAppearancePreferences`**：菜单和状态项会显示双拼，但 Engine 的会话选项仍是旧全拼，正是 issue 的根因，因此不足以修复。

## Consequences

首次建会话多一次受限的同步偏好读取；读取仍经 `MSIMEClientSession` 的绝对目录校验和共享偏好解析，不访问输入内容或网络。共享文档不可读时保持旧的回退行为，后台轮询仍负责后续偏好变化。回归测试用合成共享文档和故意过时的运行时副本创建真实 `MSIMEClientSession`，断言首个会话的方案编号为双拼。

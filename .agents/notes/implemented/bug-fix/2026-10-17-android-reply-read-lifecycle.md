# Agent Note: Android 回复模板生成读取生命周期

Status: implemented

## Problem

社区回复模板异步读取由 `generateReply` 发起时，只比较读取代次。回复面板收起或切换到其他面板后，迟到结果仍可能启动 AI 请求；面板快速重开时，旧读取还可能被当前面板误认为有效。

## Decision

模板读取的成功和失败回调统一检查读取代次、回复面板开关和实际可见性。关闭或收起回复面板时推进读取代次，使旧请求即使在面板重新打开后也失效。代次推进集中在 `ReplyTemplateLifecyclePolicy.advance`，并覆盖整数回绕。

## Alternatives considered

- **只在回调检查 `replyOpen`**：不能阻止关闭后快速重开时旧结果写回，因此还需要在面板生命周期变化时推进代次。
- **只在关闭时推进代次**：异步读取入口仍可能在隐藏的回复面板上写回，成功和失败回调都需要可见性检查。
- **取消后台文件读取**：当前读取接口没有可取消契约，且后台读取本身无副作用；在主线程丢弃失效结果即可避免界面和 AI 请求被污染。

## Consequences

回复面板已关闭、隐藏或换代后，社区模板读取结果会被安全丢弃；当前可见面板的读取和生成流程不变。后台文件读取仍可能完成，但不再触发状态更新或 AI 请求。

## Verification

新增 smoke 断言覆盖读取代次推进；先在缺少 `advance` 实现时确认编译失败，再加入实现后 `platforms/android/check-host.sh` 通过，包含 244 项 Android JVM smoke、150 项设备套件源码检查和服务 Java/API 资源检查。

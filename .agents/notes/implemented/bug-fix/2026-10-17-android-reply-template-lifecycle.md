# Agent Note: Android 回复模板读取结果绑定可见面板

Status: implemented

## Problem

Android 回复键盘从社区读取回复模板时，回调只检查读取代次。用户在读取期间收起回复面板、切换输入框或打开其他工具面板后，迟到的成功或失败结果仍可能更新回复状态，成功结果还会弹出属于旧输入会话的 `PopupMenu`。

## Decision

新增 `ReplyTemplateLifecyclePolicy`，回复模板读取结果必须同时满足读取代次未变化、回复键盘仍处于打开状态且回复视图仍可见。`ImePanels.showReplyTemplates()` 的成功与失败回调统一使用该策略后再更新状态或弹出模板菜单。

## Alternatives considered

- **只在关闭面板时递增读取代次**：可以使多数旧结果失效，但所有切换路径都必须记得递增，且无法表达视图被暂时隐藏的状态；可见性检查更直接覆盖现有面板切换路径。
- **只检查 `replyOpen`**：输入法服务在布局切换期间可能保留逻辑开关而暂时隐藏视图，旧回调仍会修改不在屏幕上的面板，因此还需要检查实际可见性。
- **让 `showReplyTemplatesLoaded` 自己再次判断**：能挡住成功菜单，但失败状态仍会写回，且把生命周期判断拆到两个调用层，容易漏掉新回调；统一在异步边界判断。

## Consequences

旧输入会话或已收起的回复面板不再接收迟到模板结果；当前可见回复面板的读取、错误提示和菜单行为保持不变。后台文件读取仍会完成，但结果在主线程被安全丢弃。

## Verification

新增 `ReplyTemplateLifecyclePolicySmoke`，先在缺少策略实现时按预期编译失败，再加入策略后与 Android host 检查一起通过；`check-host.sh` 共运行 244 项 Android JVM 冒烟及 150 项设备套件源码检查，无失败。

# Agent Note: Android 回复模板菜单读取代次

Status: implemented

## Problem

回复模板菜单有独立的 `replyMenuReadGeneration`。回复面板关闭时此前只推进了生成请求的读取代次，快速收起再打开期间，旧的菜单读取回调可能在新面板中再次弹出模板菜单。

## Decision

面板关闭和收起统一推进回复读取代次与菜单读取代次。新增 `ReplyTemplateLifecyclePolicy.Generations`，保证两个代次在同一生命周期边界同时失效，并继续由回调检查面板开关和可见性。

## Alternatives considered

- **只依赖面板可见性检查**：无法阻止面板快速重开后旧菜单回调被重新接受。
- **删除独立菜单代次**：菜单读取和 AI 请求的并发关系不同，合并代次会让一次新 AI 请求无意义地取消菜单读取，扩大状态耦合。
- **在回调里比较按钮实例**：按钮实例在面板重建中可能复用，不能表达一次面板生命周期已经结束。

## Consequences

关闭或收起回复面板会同时使模板列表读取和菜单读取失效；当前面板的新读取行为不变。

## Verification

新增 smoke 覆盖两个代次同时推进；先确认缺少实现时编译失败，再加入实现后 `platforms/android/check-host.sh` 通过（245 项 Android JVM smoke、150 项设备套件源码检查）。

# Agent Note: 鸿蒙键盘冷启动首个输入客户端

Status: implemented

## Problem

鸿蒙输入法扩展被强制停止后，第一次点进编辑框时键盘面板可以出现，但按键没有效果；收起键盘再打开后恢复。`KeyboardExtensionAbility.onCreate` 在共享目录解析、资源暂存、引擎会话创建和面板创建之间包含多个异步步骤，系统可能在这些步骤期间发出 `inputStart`。原实现直到会话创建完成后才订阅 `inputStart`，事件因此没有回调，面板就绪后也没有编辑器客户端可以接入。

## Decision

`KeyboardExtensionAbility.onCreate` 在执行第一个异步步骤前就订阅 `inputStart` 和 `inputStop`。已有的 `pending` 客户端槽位继续负责保存面板创建期间到达的编辑框；面板完成初始化后再调用 `KeyboardSession.attach`，而提前到达的 `inputStop` 会清掉这个槽位并解除会话焦点。其余依赖会话和面板的监听仍在会话创建成功后安装。

`platforms/harmony/tests/run.sh` 增加静态顺序门禁，要求 `inputStart` 订阅出现在 `onCreate` 的首个 `await` 之前，防止后续初始化代码回归到丢失首个焦点的顺序。

## Alternatives considered

- **只在面板创建失败或首次按键无响应时重建会话**：可以覆盖部分设备时序，但已经丢失的 `inputStart` 没有客户端可恢复，且会把生命周期竞态变成依赖用户再次聚焦的补救逻辑，因此不采用。
- **把 `inputStart` 订阅继续放在 `KeyboardSession.start` 之后**：会话已创建后能避免早期回调访问未准备好的状态，但仍可能错过资源暂存和会话创建期间的系统事件；现有 `pending` 设计已经提供了早订阅后的安全缓冲，因此不采用。

## Consequences

扩展能力在冷启动的整个异步准备阶段都能接住编辑器客户端，首次显示的面板可以正常输入；输入停止事件也能在面板尚未创建时清理挂起客户端。早期订阅的回调只触碰 `pending` 或可安全空操作的会话生命周期，面板和其他委托仍不会在会话成功前使用。真实设备复现需要完整鸿蒙 HAP 构建和设备验收；本次因开发机磁盘空间不足未启动新的完整构建，PR 验证段会明确这一缺口。


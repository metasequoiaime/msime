# Agent Note: 桌面面板客户端装配脱离启动入口

Status: implemented

## Problem

`main.tsx` 内联六类面板的客户端。语音、云剪贴板和表情重复关闭、目标应用记录、发送与复制动作；剪贴板启用还隐式依赖入口中的设置客户端。启动入口因此同时持有界面选择与 Tauri 命令装配，订阅失败清理和偏好保存也无法单独验证。

## Decision

`apps/desktop/src/input/desktop-panel-clients.ts` 提供 `createDesktopPanelClients`，显式接收偏好读写与已有云词库客户端。入口仍在模块级创建一次，独立窗口、设置页内面板和手写资源包装配继续共用原客户端对象。

语音、云剪贴板和表情复用私有的文本面板动作工厂。键盘与手写保留各自动作；语音继续复用 `createVoiceRecognitionClient` 的请求标识与过滤，事件桥只取 `payload`；云词库保持传入对象的身份。剪贴板数据、启用时的修订号与偏好合并、两条顺序订阅和第二条失败时释放第一条的顺序不变。

## Existing notes audit

检索 `panelClients`、面板客户端、`clipboard-history-changed` 与语音识别客户端。

- [面板客户端契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)部分重叠，继续持有共享接口；本项只调整桌面具体实现的归属。
- [桌面表情适配器](../../implemented/simplification/2026-10-18-desktop-emoji-panel.md)部分重叠，继续持有粘贴能力查询与窗口标签覆盖，本工厂提供它消费的基础客户端。
- [云词库客户端装配](../../implemented/simplification/2026-10-18-desktop-cloud-dictionary-client.md)部分重叠，继续持有平台快照策略与移动端下载，本工厂不重复它的能力组合。
- [表情剪贴板订阅](../../implemented/simplification/2026-10-14-emoji-clipboard-subscription.md)部分重叠，继续持有共享面板的读取、轮询与过期结果清理；本项保留宿主两条事件订阅的原有原子清理。
- 宿主能力类型、窗口聚焦订阅与原生语音取消笔记持有各自的类型或宿主生命周期，与本项无关。

## Alternatives considered

- **按六类面板分别建模块**：各面板独立演进最直接。但这些实现主要是少量命令适配，三个文本面板仍需共用基础动作；先保留一处集中装配，已有语音请求模块继续独立。
- **只搬移原对象**：入口最短且差异最少。但重复文本动作仍需多处修改，因此统一这四个确实相同的动作，保留其他专属能力。

## Verification

直接测试命令与窗口标签、语音事件有效期、剪贴板映射与偏好保存、正常订阅释放和注册失败清理。另执行真实入口路由与面板相关回归、类型检查、构建、lint、笔记、quick 和 CI，实际结果记录在 PR。

## Consequences

启动入口减少命令装配实现，三个文本面板只维护一份共用动作。工厂如果在渲染中调用，会改变客户端身份并重启面板任务；入口继续在模块级调用。订阅顺序和清理顺序若变化会留下监听，保存偏好若只写一个字段会丢失其他设置，均保留原逻辑并直接测试。新增一个桌面装配模块是维护成本；本项不代表原生宿主装机验收。

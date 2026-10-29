# Agent Note: 云剪贴板面板状态独立复用

Status: implemented

## Problem

`CloudClipboardPanel` 已是公共组件，但列表、上传、输入目标、设备批准与破坏性操作确认仍与 JSX 混在 `panels.tsx`。其他入口只能复用整页，无法单独复用这套状态和动作。刷新、动作后重载与草稿修订有共同代次，拆成多份容易改变迟到结果的归属。

## Decision

公开 `useCloudClipboardPanel` 和返回状态类型，接收原 `CloudClipboardPanelClient`。原状态、effect、动作和派生值按原顺序迁入 hook；确认类型和文案生成函数随状态保留为私有实现。草稿输入的修订递增与赋值原样合为 `updateDraft`，历史列表由受控的 [CloudClipboardHistory](2026-10-19-cloud-clipboard-history.md) 消费条目、忙状态和动作；上传表单由受控的 [CloudClipboardComposer](2026-10-19-cloud-clipboard-composer.md) 消费草稿及动作，并与 hook 共用已有长度上限常量；面板保留其他 JSX、文案、客户端能力判断及装配。

继续复用 `usePanelAction` 的同步互斥和刷新代次、`useAsyncGeneration` 的客户端及草稿代次、`useLatestRef` 的搜索与设备列表镜像。宿主契约、加密协议、原生存储和网络实现不变，不新增依赖。

## Historical audit

按 `CloudClipboardPanel`、`useCloudClipboardPanel`、云剪贴板面板及状态独立复用检索活跃笔记：

- [面板客户端契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)继续持有类型边界，本片只消费原接口。
- [云剪贴板加密](../../implemented/feature/2026-10-11-cloud-clipboard-e2e.md)继续持有设备批准、重置及移除协议，本片只迁移现有前端会话。
- [桌面客户端装配](../../implemented/simplification/2026-10-18-desktop-panel-clients.md)继续持有具体宿主对象与共享实例，本片不改变对象身份。
- [渲染值引用](../../implemented/simplification/2026-10-14-render-value-ref-reuse.md)继续持有搜索等镜像的 render 更新时间，本片不把赋值移到 effect。
- [语音面板状态](../../implemented/simplification/2026-10-19-voice-panel-state.md)是同层抽取参考，领域不同，没有完全吸收的决定或失效提案。其他平台剪贴板、设置页中继和功能菜单笔记与本片无关。

## Alternatives considered

- 仅把整个面板搬到独立模块：最直接减少大文件长度，但调用方仍只能使用完整界面，因此抽取可独立消费的状态与动作。
- 分别建立列表、上传和设备 hook：各模块更短，但操作后重载、同步互斥、搜索镜像与错误恢复都共用刷新代次，分拆需要额外协调，先保持完整会话边界。

## Verification

- 抽取前 7 个文件、19 项基线测试通过；新增 5 项 hook 测试先因公共 hook 缺失失败，抽取后 8 个文件、24 项通过。覆盖客户端切换的迟到列表与能力、StrictMode 重放、上传防重与新草稿保留、最新搜索刷新、设备状态及确认、能力回退和刷新失败清理。
- 实际设置页中云剪贴板相关的 6 项测试通过，其余 389 项按名称筛选跳过；上述验证合计 30 项。
- 桌面构建、HarmonyOS 类型检查与 lint 通过，lint 保留原有 4 条未使用声明警告。独立审查另运行 5 项 hook 测试，全部通过。本地 quick 通过，包含 Rust workspace、Android 交叉编译、Linux 桌面容器和 Apple bridge 检查；PR CI 是合并门禁。
- 固定基线 `0d1b5ecb1d918147a09e775197cf751420cedbd6` 的语法树对照确认原状态、effect、动作和派生值及其顺序保持，确认类型和文案 helper 保持，展开 `updateDraft` 后的完整 JSX 一致，返回字段与消费逐项相同，9 个传递模块没有运行时包入口循环。

## Consequences

面板的会话逻辑可独立复用，代价是新增公开 hook 和状态返回契约。

动作后重载仍使用同一刷新代次，失败时的列表清理和错误说明顺序必须保持。客户端更换仍不重置草稿和搜索；确认状态的更换边界由[云剪贴板确认归属修复](../../implemented/bug-fix/2026-10-30-cloud-clipboard-confirmation-owner.md)补充。直接调用 hook 的消费者负责沿用界面的操作许可与确认流程。前端验证不代表原生宿主装机或真实账号联调。

# Agent Note: 回复模板预览区独立复用

Status: implemented

## Problem

AI 辅助页的私有 `ReplyTemplatesGroup` 把社区列表、五条预览、添加状态和 UI 绑在设置上下文中，其他共享入口无法独立复用。添加操作只读取 React state 互斥，没有同步请求锁，完成与异常也没有检查社区客户端或卸载代次，可能重复添加或弹出迟到提示。

## Decision

公开 `useCommunityReplyTemplates` 与 `CommunityReplyTemplates` 及其输入、返回类型。设置页只判断显示条件并注入客户端、提示模式和浏览回调；独立组件持有原有预览布局，hook 组合现有 `addCommunityReply` 与 `useAsyncActionRunner`，列表直接复用 runner 的所有者代次，不再单独维护第二份代次。

读取仍取社区回复列表的前五条，每次挂载或客户端变化时读取。客户端变化清除上一个客户端的预览、错误、完成提示与已添加集合。添加保留收藏、读取最新版本、可选写入回复键盘的顺序；runner 阻止同批重复操作，并让过期成功、失败及 finally 不写回新界面或弹 toast。底层已发出的操作不取消。

## 旧笔记归属

- [回复模板请求代次](../../implemented/simplification/2026-10-12-ai-reply-generation.md)部分重叠，继续持有异步归属规则，补充添加操作与新 owner。
- [社区资源操作](../../implemented/simplification/2026-10-16-community-resource-actions.md)保留收藏和写入键盘的业务顺序，本片只提取其调用方状态与展示。
- AI assistant 配置代次、MCP 客户端重新加载的笔记仅提到同一个公共工具，不持有回复模板预览决定，属于无关记录。

未发现相同提案或完全吸收的决定，无需归档。

## Alternatives considered

- 只导出原私有组件：改动最少，但状态仍与展示绑定，调用方无法复用预览逻辑，既有添加竞态也继续存在。
- 为回复模板再写 busy ref 和代次守卫：局部代码直观，但会重复已有 `useAsyncActionRunner` 的同步互斥、卸载与所有者隔离，应直接组合公共工具。

## Verification

原有设置页上两条合成回归先稳定失败，分别出现迟到 toast 与两次收藏请求；其余 12 项原有导航测试通过。新 hook 与公共 UI 的 9 项测试先因入口缺失失败，覆盖加载上限、提示模式、客户端切换、旧结果隔离、添加失败与浏览回调。相关 5 个文件共 45 项测试通过；桌面生产构建、HarmonyOS 类型检查、lint 与笔记门禁通过。提交前另运行完整 quick 门禁。

## Consequences

公共预览逻辑与展示可以独立组合，代价是增加公开 hook、组件及类型契约；设置页仍决定可见性和图库导航。客户端切换时若未清空已添加集合，同标识模板会错误继承上一客户端结果；若旧 finally 无条件清理，会解除当前添加状态。测试覆盖两个客户端同标识交错完成。不改宿主文件读写、权限或网络协议，不声明原生设备验收。

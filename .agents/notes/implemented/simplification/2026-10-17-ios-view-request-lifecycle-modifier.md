# Agent Note: iOS 页面请求生命周期修饰符

Status: implemented

## Problem

云词库文件、云词库应用和 AI 皮肤抽卡页都用 `ViewRequestGeneration` 防止离页后的异步结果写回，但页面仍分别手写 `.onAppear { request.appear() }`、`ViewRequestTask.disappear` 和 `pending` 取消。社区资源与皮肤的列表、详情、发布和编辑页也重复同一组生命周期胶水。请求代次的规则已有共享实现，重复的是 SwiftUI 生命周期胶水；每页的业务清理仍不同，不能把它们整页合并。

## Decision

在 `ViewRequestTask.swift` 增加 `ViewRequestLifecycle` 和 `trackViewRequest`。修饰符只负责：页面出现时调用 `request.appear()`，随后执行可选的 `onAppear` 闭包；页面离开时先调用 `ViewRequestTask.disappear`，再调用调用方提供的 `onDisappear` 清理闭包。调用方继续持有 `@State` 的请求代次和任务句柄，并负责忙碌状态、预览、草稿和其他领域状态的清理。

迁移 `CloudDictionaryFilesView`、`CloudDictionaryApplyView` 和 `AISkinGenerationView`，随后迁移 `CommunityResourceDetail`、`CommunityResourceEditor`、`CommunitySkinDetail`、`CommunityPublishView`、`CommunityResourcesView`、`CommunityReplyTemplatesSection`、`SkinCommunityView` 和 `SettingsSyncView`，再迁移 `CloudDictionaryView`，最后迁移账号资料页、昵称编辑页和 `CloudClipboardView`。页面保持原有清理顺序，公共失效和取消先发生，页面字段随后清空；列表与发布页通过可选的 `onAppear` 闭包继续刷新本地状态。社区资源列表的登录刷新、内嵌回复模板的逐项添加代次，以及皮肤列表的账号和逐项获取代次仍由各自页面保留，暂不把这些独立代次塞进修饰符。云词库主页的类型切换仍由页面先取消 pending、再使请求代次失效并重新加载；账号资料页的资料加载任务仍由页面单独取消，修饰符只接管账号操作任务；云剪贴板的请求代次仍由 `run` 和 `execute` 持有，离开页面时的忙碌状态、列表、设备状态和编辑文本清理仍由调用方闭包负责。

## Alternatives considered

- **继续在每页手写两个生命周期钩子。** 行为容易逐页看懂，但已经重复了同一组代次失效和任务取消调用；新增页面时还会继续复制这段样板。
- **让修饰符接管页面所有离开状态。** 可以进一步缩短调用方，但会把业务字段、忙碌状态和独立任务的所有权塞进公共 API，破坏不同页面原有的清理顺序。
- **只给 `ViewRequestTask` 增加一个组合静态函数。** 能减少一次调用，却不能统一 SwiftUI 的出现和离开注册，调用方仍要重复写钩子；因此使用受限的 `ViewModifier` 作为边界。

## Consequences

- 十五个页面共享同一套出现、离开失效和 pending 取消注册；旧请求结果的代次语义不变。
- 页面自有清理在公共清理之后执行，保留原有的忙碌状态、预览、草稿和进度重置顺序。
- `ViewRequestLifecycle` 不处理轮询、独立操作代次、会话校验或业务错误；这些仍由页面和 `ViewRequestTask` 原有入口管理。
- 契约测试同时检查修饰符归属和十五个调用方不再复制生命周期钩子；原有社区列表、回复模板区、详情、资源编辑、皮肤、设置同步、云词库主页、账号资料页和云剪贴板生命周期检查随调用路径更新，并覆盖列表与发布页的自定义出现清理。

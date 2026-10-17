# Agent Note: iOS 页面请求生命周期修饰符

Status: implemented

## Problem

云词库文件、云词库应用和 AI 皮肤抽卡页都用 `ViewRequestGeneration` 防止离页后的异步结果写回，但三页仍分别手写 `.onAppear { request.appear() }`、`ViewRequestTask.disappear` 和 `pending` 取消。请求代次的规则已有共享实现，重复的是 SwiftUI 生命周期胶水；每页的业务清理仍不同，不能把它们整页合并。

## Decision

在 `ViewRequestTask.swift` 增加 `ViewRequestLifecycle` 和 `trackViewRequest`。修饰符只负责：页面出现时调用 `request.appear()`；页面离开时先调用 `ViewRequestTask.disappear`，再调用调用方提供的 `onDisappear` 清理闭包。调用方继续持有 `@State` 的请求代次和任务句柄，并负责忙碌状态、预览、草稿和其他领域状态的清理。

迁移 `CloudDictionaryFilesView`、`CloudDictionaryApplyView` 和 `AISkinGenerationView`。三页保持原有清理顺序，公共失效和取消先发生，页面字段随后清空；其他页面的离开钩子包含额外的生命周期、轮询或独立代次时暂不迁移。

## Alternatives considered

- **继续在每页手写两个生命周期钩子。** 行为容易逐页看懂，但已经重复了同一组代次失效和任务取消调用；新增页面时还会继续复制这段样板。
- **让修饰符接管页面所有离开状态。** 可以进一步缩短调用方，但会把业务字段、忙碌状态和独立任务的所有权塞进公共 API，破坏不同页面原有的清理顺序。
- **只给 `ViewRequestTask` 增加一个组合静态函数。** 能减少一次调用，却不能统一 SwiftUI 的出现和离开注册，调用方仍要重复写钩子；因此使用受限的 `ViewModifier` 作为边界。

## Consequences

- 三个页面共享同一套出现、离开失效和 pending 取消注册；旧请求结果的代次语义不变。
- 页面自有清理在公共清理之后执行，保留原有的忙碌状态、预览、草稿和进度重置顺序。
- `ViewRequestLifecycle` 不处理轮询、独立操作代次、会话校验或业务错误；这些仍由页面和 `ViewRequestTask` 原有入口管理。
- 契约测试同时检查修饰符归属和三个调用方不再复制生命周期钩子；原有三份页面生命周期检查随调用路径更新。

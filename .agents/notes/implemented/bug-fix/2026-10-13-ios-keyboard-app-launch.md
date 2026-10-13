# Agent Note: 键盘添加语言的宿主跳转

Status: implemented

## Problem

键盘输入方式面板的「添加语言」关闭面板后没有进入应用的输入设置。原 `KeyboardAppLauncher` 沿响应链调用废弃的 `openURL:`，只要找到应答对象就报告请求成功，并未获得系统真正打开的结果。

## Decision

响应链入口改用公开的 `openURL:options:completionHandler:`。响应链先遇到 `UIScene` 时，直接调用 `scene.open`，options 为 nil；其他应答对象用 Objective-C 协议声明 UIApplication 的字典参数签名，再通过 `AnyObject` 动态派发，保留系统参数与 block 桥接。UIScene 与 UIApplication 使用相同选择器，但前者 options 为 `UISceneOpenExternalURLOptions`，不能给它传字典；真机错误堆栈确认错误字典在 `UIScene.openURL` 内触发 `doesNotRecognizeSelector:`。请求是否发出与系统是否打开分开：返回值表示找到入口，回调表示打开结果；找不到入口也回调 false。控制器收到失败后在键盘中给出手动打开应用的提示，避免继续无反馈。目的地仍为本版本 scheme 的 `settings/input`，应用的既有路由负责进入输入设置。

## Alternatives considered

- 保留 `openURL:`：代码少且假响应链单测原本通过，但废弃接口不提供当前系统的异步结果，也无法解释用户看见的无响应，不继续使用。
- 直接调用 `UIApplication.shared`：普通宿主中最简单，但键盘扩展不可用，不引入这一依赖。
- 对相同选择器统一传字典 options：可以复用一套动态调用且假 UIApplication 响应链单测通过，但 iPhone 17 的链条先遇到 UIScene，参数类型不符导致崩溃。函数指针和 Objective-C 动态派发都会重现，按接收者类型选择正确签名。
- 使用 `NSExtensionContext.open`：它是扩展 API，但官方支持范围不包含键盘扩展，不把它当键盘跳转方案。

## Consequences

词库、设置、常用语、反馈、关于等既有键盘入口共享同一现代调用；各 URL 不变。测试使用合成响应链检查目标与成功/拒绝回调，并在 iPhone 17 中点「添加语言」，要求应用输入设置实际出现。

验证：九键、设备布局与打开入口合计 117 项单测通过。iPhone 17 点按添加语言后实际出现应用输入设置，单独入口用例通过。回归编辑器直接进入对话标签，避免上一轮深链接留下的设置导航栈；添加卡片可翻页，验收按输入页面导航标题判断跳转，避免所有语言已启用时没有添加按钮造成误报。

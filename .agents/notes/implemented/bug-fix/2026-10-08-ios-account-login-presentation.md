# Agent Note: iOS 账号登录的弹窗与 task 挂在 List/Form 上

Status: implemented

## Problem

在「我的」页点「邮箱登录」，填好邮箱、获取验证码，然后切到邮箱 App 看验证码。切回来时验证码弹窗里的邮箱被清空，已经拿到的验证码挑战也没了，只能重新获取。

`AppleAccountSection` 的 body 是一个 `Section`，验证码弹窗的 `.sheet`、加载登录方式的 `.task` 和错误提示 `.alert` 都挂在这个 `Section` 上。List 和 Form 会把 `Section` 上的修饰符分给它的每一行，于是同一个弹窗有好几份呈现者：点一次按钮，日志里出现 5 条 `Attempt to present … while a presentation is in progress`，`.task` 也按行数重复执行。App 进后台时系统会重配列表行，弹窗随之被另一份呈现者重建，`CodeLoginView` 的 `@State`（邮箱、验证码、挑战、倒计时）全部重置。`AccountLoginSheet` 和 `SavedSkinPublishFlow` 把同一个 Section 放在 Form 里，有同样的问题。

## Decision

修复时，登录状态放在 `AppleAccountModel`（`platforms/ios/App/Sources/account/AccountSettingsView.swift`）里，由包住 `AppleAccountSection` 的视图以 `@StateObject` 持有。`AppleAccountSection` 只渲染行，不挂任何 task、弹窗或提示。外层 List/Form 调用 `.appleAccountPresentation(account)`，由它统一挂这三样，每个页面只有一份。

[iOS 与鸿蒙宿主采用全平台设计稿](../feature/2026-10-07-ios-harmony-all-platform-design.md) 之后，「我的」页和 `AccountLoginSheet` 不再用 List/Form：登录面板是 ScrollView 里的 `AccountLoginOptions`，邮箱表单在面板里原地展开，手机号验证码弹窗由它在自己身上挂一份，`AppleAccountModel` 和 `appleAccountPresentation` 随之移除。仍放在 Form 里的只有 `SavedSkinPublishFlow` 用的 `AppleAccountSection`，它的 `.task` 挂在 `AccountLoginOptions` 那一行上，Section 上不挂修饰符。以后在任何会被放进 List/Form 的 `Section` 上，都不得挂 `.sheet`、`.alert`、`.task` 这类修饰符。

## Alternatives considered

- **把修饰符挂到 Section 里的某一行（如资料卡）** — 改动最小，状态也能留在原视图里。但资料卡在登录前后是 if/else 两个分支，登录状态一变 `.task` 就重跑；惰性列表的行在滚出屏幕时也会被回收。修好的只是眼前这一种重建。
- **只把邮箱和验证码状态提到 `CodeLoginModel` 里** — 弹窗被重建后输入还在。但多份呈现者、重复的 `.task` 和重复的 `.alert` 都还在，症状只是被掩盖了。

## Consequences

- **收益**：弹窗只有一份呈现者，切后台回来后弹窗和输入都保留；登录方式的加载和 Apple 挑战的获取每次出现只执行一次，不再按行数重复。
- **代价**：修复时 `AppleAccountSection` 不再自带完整功能，调用方忘记挂 `appleAccountPresentation` 就不会加载登录方式。改版后登录方式由 `AccountLoginOptions` 自己加载，这一代价不再存在。

## Verification

`platforms/ios/UITests/OnboardingUITests.swift` 的 `testCodeLoginKeepsTargetAcrossBackgrounding` 打开邮箱登录并填写邮箱（改版后是点未登录的资料卡打开登录面板，再展开邮箱表单），按 Home 键回到桌面再切回 App，然后断言弹窗还在、邮箱未变。修复前，这个用例在弹窗出现这一步就失败；修复后通过，同次运行日志里不再有 `while a presentation is in progress`。该用例依赖账号服务提供邮箱登录，服务不提供时跳过。

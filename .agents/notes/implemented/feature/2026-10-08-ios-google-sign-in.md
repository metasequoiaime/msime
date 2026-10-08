# Agent Note: iOS 通过 Google 登录走 GoogleSignIn SDK 的 ID token 流程

Status: implemented

## Problem

后端 `/v1/auth/providers` 早就返回 `google: true`，Android 和桌面端也都能用 Google 登录，iOS 的登录面板却只画 Apple、邮箱和手机号三个按钮（`AccountLoginOptions.offersAny` 只认这三项）。在别的平台用 Google 注册的用户，到 iPhone 上只能换一种方式另起一个账号，或者先在别的平台给账号绑上邮箱。[iOS 与鸿蒙宿主采用全平台设计稿](2026-10-07-ios-harmony-all-platform-design.md) 当时把它记在「缺后端或依赖」里：缺的是 iOS 端的 OAuth 客户端和 SDK，不是后端。

## Decision

iOS 用 Google 官方的 `GoogleSignIn` 10.0.0，经 CocoaPods 只链接进 `MSIMEApp`，走和 Android 相同的 ID token 流程：

1. `CodeLoginModel.signInWithGoogle()` 向后端申请 `provider: "google"`、target 为空的 challenge，拿到 `challenge_id` 和 `nonce`。
2. 用 `GIDSignIn.signIn(withPresenting:hint:additionalScopes:nonce:)` 拉起 Google 的账号选择，ID token 的 `nonce` 就是后端给的这个值。
3. 拿到 token 后立刻调用 `GIDSignIn.signOut()`，清掉 SDK 自己存在钥匙串里的 Google 会话，再用 `BackendAccountSession.signIn(challenge:credential:)` 换后端会话。登录状态只由后端会话决定。

Info.plist 里同时写 `GIDClientID`（iOS 类型的 OAuth client）和 `GIDServerClientID`（Android 用的同一个 web client）。设了服务端 client ID 后，SDK 会在授权请求里带上 `audience`，ID token 的 `aud` 就是这个 web client，后端 `msime-cloud` 的 Google 校验器本来就接受它，所以后端配置不用改。回调 URL scheme 是倒序的 iOS client ID，由 `onOpenURL` 先交给 `GIDSignIn.handle(_:)`。

按钮只在两个条件都满足时出现：后端列出了 `google`，并且 `GoogleSignInSupport.available` 为真。后者要求这个构建链接了 GoogleSignIn，Info.plist 里两个 client ID 也都不为空。模拟器走的 `build-app.sh simulator` 不装 pod，所以没有这颗按钮，和手写识别的处理方式一样。

为了让空 target 的 Google challenge 能发出去，共享 Swift 客户端的 `validProviderTarget` 对 `google` 放行了空 target；带 target 时仍然只接受桌面端回环监听的回调地址。

## Wire contract

- `POST /v1/auth/challenges` `{provider: "google", target: "", purpose: "login"}` → `{challenge_id, expires_in, nonce}`
- `POST /v1/auth/login` `{challenge_id, credential: <Google ID token>}`
- 后端按 `Google.ClientIDs` 里的每个 audience 依次校验 ID token，并按原文比对 `nonce`。

## Alternatives considered

- **复用桌面端的回环流程（`crates/client-core/src/account/google.rs`）** — 不用引入任何 SDK，PKCE 和 client secret 都已经放在后端。但 iOS 切到 Safari 后，App 里的回环监听很快会被挂起；后端也只接受 `http://127.0.0.1:<port>/callback` 形式的回调地址。要让它在 iOS 上可靠工作，得在后端加一条 https 或自定义 scheme 的回调路径，改动比接一个 SDK 大。
- **自己用 `ASWebAuthenticationSession` 拼 OAuth 授权码加 PKCE 请求** — 可以少一组依赖（AppAuth、GTMAppAuth、AppCheckCore 等）。但这等于手写一份 OAuth 客户端：iOS 客户端的回调、授权码换 token、ID token 的提取都要自己维护，而 GoogleSignIn 正是 Google 为此维护的那一份。仓库约定优先用成熟的 SDK。
- **用 Swift Package Manager 引入 GoogleSignIn** — 模拟器构建也能带上这颗按钮，代码里不需要 `#if canImport`。但 iOS 现有的第三方依赖全部走 CocoaPods，`Podfile.lock` 把整棵依赖树都锁住了，发布构建也一直用 workspace。XcodeGen 生成的工程里 SPM 的传递依赖只按版本范围解析，CI 生成到 `build/ios` 的工程也拿不到提交进仓库的 `Package.resolved`。同一个 App 里两套包管理器并存，也会让 `GTMSessionFetcher` 这类共同依赖出现两份来源。

## Consequences

- **收益**：iOS 用户可以用 Google 账号登录，和 Android、桌面端是同一个后端身份（同一个 `google` subject）。后端不用改，Android 和 iOS 共用同一套 ID token 加 nonce 的契约。
- **代价**：`MSIMEApp` 多了 GoogleSignIn 及其依赖（AppAuth、GTMAppAuth、GTMSessionFetcher、AppCheckCore、GoogleUtilities、Promises、RecaptchaInterop），它们的许可证随 App 打包为 `GoogleSignIn-Dependencies.txt`。Podfile 里键盘扩展原本嵌套在 App 下面，现在改成独立的顶层 target：嵌套时会把 GoogleSignIn 继承进键盘扩展，嵌套加 `inherit! :none` 又会让 CocoaPods 把 ML Kit 的预编译框架从扩展的链接参数里漏掉，真机构建报 `Unable to resolve module dependency: 'MLKitDigitalInkRecognition'`。`ProjectConfigurationTests` 守着这一条。
- **代价**：iOS 类型的 OAuth client 建在 Google Cloud 项目里，仓库之外。以后 iOS 的 bundle ID 变了（比如出了别的版本），每个 bundle ID 都要另建一个 client，否则 Google 会拒绝回调。
- **代价**：不装 pod 的模拟器构建里看不到 Google 按钮，界面测试只能在 workspace 构建里覆盖这条路径。

## Verification

- `shared/backend` 的 `BackendAccountClientTests.testProvidersAndServerNonce` 覆盖空 target 的 Google challenge。
- `ProjectConfigurationTests.test_google_sign_in_is_linked_into_the_app_only` 检查 Podfile 的 target 结构、回调 scheme 与 `GIDClientID` 是否对应、`GIDServerClientID` 是否与 Android 一致。
- 真机发布构建（完整 Podfile，`CODE_SIGNING_ALLOWED=NO`）编过：App 二进制里有 `GIDSignIn`，键盘扩展里没有，ML Kit 仍在扩展里。
- `OnboardingUITests.testGoogleSignInOpensGoogleAndCancelReturnsToSheet` 在只装 GoogleSignIn pod 的 arm64 模拟器构建上通过：打开 Google 的登录页（没有 `invalid_request` 一类的错误），取消后回到登录面板，也不显示错误。不装 pod 的构建里这个用例会跳过。
- 用真实 Google 账号完成登录、换到后端会话这一步，还没有在设备上验证过。

# Agent Note: Android 的 Google 登录按钮走按钮流程，不走一键登录底部弹窗

Status: implemented

## Problem

#5979（Android 11，vivo X60，客户端 0.2.2）：用 Google 登录过一次、退出登录后再点「通过 Google 登录」，屏幕上提示「这台设备上没有可用的 Google 账号」，而手机里明明有 Google 账号。

`GoogleSignInFlow.start` 给 Credential Manager 的是 `GetGoogleIdOption`（`setFilterByAuthorizedAccounts(false)`）。这是一键登录底部弹窗用的选项：Play services 会从里面去掉需要重新验证的账号，用户关掉弹窗几次后还会让这个 app 进入冷却期（状态码 `28436`）。发布包实际解析到的 credentials 1.6.0 把这类不可重试的 `ApiException` 都映射成 `NoCredentialException`，消息里带着 Play services 的状态码。而 `explain()` 只认 `28444`（签名证书没登记），其余一律说成「没有可用的 Google 账号」，既不写日志也不带状态码，真实原因就此丢失。

我们的入口是一个明确的「通过 Google 登录」按钮（`LoginSheet` → `SignIn.startGoogle` → `GoogleSignInFlow.start`），首次引导和「我的」页都经过这一条路径，没有别的地方构造 Google 凭据请求。

## Decision

- `GoogleSignInFlow.start` 的请求里只放一个 `GetSignInWithGoogleOption.Builder(serverClientId).setNonce(nonce).build()`。这是 Google 给按钮准备的流程：弹出完整的账号选择，需要重新验证的账号也列在里面，用户可以当场添加账号，不受底部弹窗冷却期的限制。返回的仍是 `TYPE_GOOGLE_ID_TOKEN_CREDENTIAL` 类型的 `CustomCredential`（credentials-play-services-auth 的 `CredentialProviderGetSignInIntentController` 用 `GoogleIdTokenCredential` 包装），`onResult` 的解析不用改。按钮选项不能和其他选项放在同一个请求里，否则 Credential Manager 直接报 `GetCredentialUnsupportedException`。
- `NoCredentialException` 的文案由不依赖 AndroidX 的 `GoogleSignInFailure.noCredential` 生成：保留 `28444` / `Developer console` 那一支，通用的那句后面加上状态码，例如「这台设备上没有可用的 Google 账号（28436）」。状态码优先取方括号里的详细码，没有时再取 `ApiException` 消息开头的通用码（如 `10: `）；屏幕上只出现数字，不搬 Play services 的英文原文。
- 取消以外的失败，`explain()` 用 `Log.w`（tag `MSIMESignIn`）记一行异常类型和消息。这条消息是 Play services 的状态说明，里面不含账号、令牌或 nonce。
- `SignIn.signOut` 删掉本机会话之后调用 `GoogleSignInFlow.clearCredentialState`：用应用 Context 的 `CredentialManager.clearCredentialStateAsync`，回调直接在 Play services 回调所在的线程上执行，失败只记日志。同步抛出的 `RuntimeException | LinkageError` 也只记日志。`signOut` 原有的契约不变：仍然阻塞，清除本机会话失败时照旧抛出，并且在那种情况下不会去清凭据状态。
- `platforms/android/gradle-app/app/build.gradle.kts` 把 credentials 和 credentials-play-services-auth 的声明版本改成实际解析出的 1.6.0（googleid 1.2.1 依赖 1.6.0，`androidx.credentials` 是原子组），注释改正为 googleid 提供的是按钮用的 `GetSignInWithGoogleOption`。

Tauri 合包（`apps/desktop/src-tauri/gen/android`）编译的是同一份 `platforms/android/java`，依赖是 googleid 1.1.1 和 credentials 1.3.0。修复时用 javap 确认过：googleid 1.1.1 里有 `GetSignInWithGoogleOption.Builder(String)` 和 `setNonce`，credentials 1.3.0 里有 `clearCredentialStateAsync` 和 `ClearCredentialStateRequest()`，所以合包不需要升级依赖。

## Alternatives considered

- **保留 `GetGoogleIdOption`，只把状态码带到屏幕上** — 改动最小，也能让下一次反馈说清楚原因。但冷却期和需要重新验证的账号仍然会挡住一个已经明确按了登录按钮的人，问题只是从「看不懂」变成「看得懂但登不上」。
- **先发 `GetGoogleIdOption`，报 `NoCredentialException` 时再退回 `GetSignInWithGoogleOption`** — 这样设备上有已授权账号时还能保留底部弹窗的轻量体验。但用户按的是按钮，本来就期待一个账号选择；多一轮请求就多一种失败组合要解释，冷却期内还会先白白失败一次。
- **只在退出登录时调用 `clearCredentialState`** — 调查时最先怀疑的就是它。但在 Play services 上它对应的 `SignInClient.signOut` 只关掉自动选择账号，我们没有开自动选择，所以它解释不了 #5979。现在仍然调用它，是按 Google 的建议做的加固，不当作修复本身。

## Consequences

- **收益**：设备上有 Google 账号时，按钮总能弹出账号选择，退出后再登录、需要重新验证的账号和刚添加的账号都能用；真的失败时，屏幕上带着状态码，logcat 里有完整消息，下一次反馈能直接对上原因。
- **代价**：每次登录都要在完整的账号选择里点一下，不再有底部弹窗那种「一点就登录」的体验；按钮流程的界面由 Play services 决定，样式比底部弹窗重一些。
- **未覆盖**：没有在真机上复现 #5979，也没有验证按钮流程。验证到的只有这些：`check-host.sh` 里新加的 JVM 冒烟 `tests/account/GoogleSignInExplainSmoke.java`（覆盖 `28444`、方括号状态码、通用状态码、空消息），以及发布用的 `gradle-app` 在 Studio 上跑通 `compileFullReleaseJavaWithJavac` / `compileFullDebugJavaWithJavac`。

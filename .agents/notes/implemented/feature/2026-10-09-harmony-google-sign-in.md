# Agent Note: 鸿蒙通过系统浏览器加本机回环提供 Google 登录

Status: implemented

## Problem

后端 `/v1/auth/providers` 一直返回 `google: true`，桌面端和 iOS 都能用 Google 登录，鸿蒙的登录面板却只有邮箱和手机号。在别的平台用 Google 注册的用户，到鸿蒙上只能换一种方式另起一个账号。

iOS 的做法用不了：它用 GoogleSignIn SDK 拿 ID token（见 [iOS 通过 Google 登录](2026-10-08-ios-google-sign-in.md)），鸿蒙没有 Google 服务，也没有给鸿蒙的 OAuth 客户端类型。在应用内的 WebView 里打开 Google 登录页也不行，Google 会拒绝嵌入式浏览器（`disallowed_useragent`）。

## Decision

走桌面端的服务端交换流程（RFC 8252，系统浏览器加本机回环），后端不改：

- 设置应用用 `TCPSocketServer` 在 `127.0.0.1` 的随机动态端口上监听，被占用就换一个，最多试 8 次。
- 经 `/v1/auth/challenges` 申请 challenge，`target` 是回环地址；用 `openLink` 把后端生成的授权链接交给系统浏览器。
- 浏览器跳回本机端口后，把授权码经 `/v1/auth/login` 交给后端，由后端用它保管的 PKCE 和 client secret 换 token。

判定只有一份：把 `crates/client-core/src/account/google.rs` 里与 socket 无关的部分拆成公开函数，桌面端改调它们，行为不变：
- `google_loopback_target`：生成回跳地址；
- `google_loopback_plan`：校验回跳地址和授权链接，算出等待时长；
- `google_loopback_reply`：判定一次回环请求是不是回跳，并给出写回浏览器的完整 HTTP 回应。

这几个函数经新的 C ABI `msime_client_google_loopback`（`operation` 为 `target`、`plan`、`reply`）和 NAPI `googleLoopback` 交给 ArkTS。`account/HarmonyGoogleSignIn.ets` 只做 socket、浏览器和后台保活；请求头的拼接与截断放在可测的 `GoogleLoopbackRequestHead.ts`。

`AccountCloudBridge` 的 `request_code` 放行 `google`，但只接受回环形状的 `target`；新增 `google_login`，授权码按 client-core `validate_google_login` 同样的规则校验，与验证码登录共用后半段的会话保存。

用户在浏览器里登录时设置应用在后台，所以等待期间申请延迟挂起（`requestSuspendDelay`），结束时撤销。页面那一侧，鸿蒙登录面板（`login-sheet.tsx`）在宿主提供 `googleLogin` 时显示「使用 Google 登录」、等待状态和「取消 Google 登录」，关闭面板也会取消。Google 登录的请求超时放宽到 6 分钟，默认的 30 秒会在用户还在浏览器里时就报超时。

## Alternatives considered

- **后端提供 https 回调，换完 token 后用 App Linking 拉起鸿蒙应用。** 最强的理由是不依赖本机监听，没有后台冻结的风险。不用它，是因为要改后端、加一种回调流程，还要在 AGC 配置 App Linking；而本机回环在模拟器上实测可行，后端也已经支持这条流程。
- **在 ArkTS 里重写回跳解析和授权链接校验。** 最强的理由是不用改 C ABI、不用重编原生库。不用它，是因为这几项都和安全相关（校验链接是不是指向这个监听的 Google 授权地址、`state` 用常量时间比较、授权码格式），写两份就会漂移；拆出纯函数后桌面端和鸿蒙共用一份。
- **在 WebView 里打开 Google 登录页。** 实现最简单，不用离开应用。不用它，是因为 Google 明确拒绝嵌入式浏览器的 OAuth 请求。

## Consequences

- **收益**：鸿蒙用户可以用 Google 登录，与其他平台的同一个账号互通；回环判定只有一份，桌面端行为不变。
- **代价**：真机上的后台冻结可能比模拟器严格，延迟挂起是否足够要在真机上确认。中国大陆的网络访问不了 Google，这个按钮对那里的用户没有用，是否按地区隐藏没有在这次决定。

## Verification

- Rust：`google_loopback_plan` 和 `google_loopback_reply` 的单测；桌面端原有的 Google 登录测试，包括完整的回环登录、拒绝、超时和取消，全部通过；host-api 的 ABI 测试，以及头文件与导出符号的一致性检查。
- 鸿蒙：`tests/google-sign-in.test.ts` 覆盖请求头累积与截断、`google` challenge 只接受回环目标、`google_login` 的授权码校验与提交。
- 共享 UI：`account-page.test.tsx` 新增三条鸿蒙面板用例：没有宿主客户端时不显示按钮、登录成功后关闭面板、取消和关闭面板都会结束等待。
- 设备（HarmonyOS 6.0.2 模拟器）：
  - 可行性试验：浏览器 120 秒后回跳仍被立即收到；
  - 真实登录：浏览器打开 Google 授权页，完成后设置应用记录 `google sign-in: signed in`，前后成功两次。
- 第一次真实登录时出现过一次被取消的登录，紧接着第二次成功；没有复现，从代码看连点不会触发两次（`useAsyncActionRunner` 的同步 `running` 标记会挡住），更可能是那次等待中被关面板或切页面取消了。早先用户报告过一次「500」，在加了分步日志之后没有再出现，原因未查明。

# Agent Note: 鸿蒙账号请求落在失效的复用连接上时，幂等请求换新连接重发一次

Status: implemented

#8201 第 2 步删掉了这里说的 ArkTS 账号传输和桥接，账号请求改由 Rust 发出、错误码由 Rust 给，见[鸿蒙账号与云端业务整体切到 Rust 目录模式](../architecture/2026-10-17-harmony-account-rust-directory-mode.md)。下面是当时的记录。

## Problem

已登录的「我的」页顶上出现「账号服务暂不可用，请稍后再试。」，资料却照常显示。模拟器落盘的 hilog（`/data/log/hilog/`）里，那一刻 App 有一个 GET 失败：curl 56（接收失败，鸿蒙报 `errCode:2300056`），`connect` 和 `tls` 都是 0，36 毫秒就结束；紧接着的几个请求换了新连接（源端口变了），全部 200。

也就是说，请求落在一条复用的空闲连接上，对端（Cloudflare）已经把它关了。鸿蒙的账号传输 `HarmonyAccountTransport.request` 不重试，失败直接交给页面。桥接又把传输层的 BusinessError 映射成 `account_unavailable`（只有对话请求是 `account_network`），页面于是说「账号服务暂不可用」，让人以为服务坏了。

## Decision

- `AccountRetryPolicy.ts`：传输层抛出 curl 52（空回复）、55（发送失败）、56（接收失败），也就是连接在复用时已经失效，而且方法是 GET、PUT 或 DELETE 时，再发一次，只发一次。`HarmonyAccountTransport.request` 按它循环，每次重发都新建 `http.HttpRequest`，并记一条 `retried on a fresh connection` 的警告日志，不记正文和令牌。
- 带标签的请求被 `cancelRequests` 取消后不再重发：每次调用持有一个 `PendingCall`，取消时标成已取消，调用结束就移除，标签（AI 皮肤的 requestId 各不相同）不会越积越多。
- `AccountCloudBridge.handle` 里，传输层抛出的非 Error（鸿蒙的 BusinessError）对所有账号操作都报 `account_network`，不只对话；社区的两个操作照旧报 `community_unavailable`。共享界面的 `accountMessage` 把 `account_network` 写成「网络连接失败，请检查网络后重试。」

## Alternatives considered

- **所有方法都重试。** POST 的连接可能断在服务端处理之后，再发一次可能重复提交（登录、发布、反馈）。登录和刷新令牌都是 POST，它们落在失效连接上仍会失败一次。
- **连不上、超时也重试。** 这些不是连接复用的问题，网络真的断了，重试只会让用户多等一轮才看到失败。
- **关掉连接复用。** 每个请求多一次 TLS 握手，「我的」页一次要发五六个请求，代价大于偶尔一次的重发。

## Consequences

- **收益**：后台放久了回来，第一个读请求不再因为失效连接失败。真断网时页面说的是网络，不是账号服务。
- **代价**：真断网时，GET 要失败两次才报错，但第二次在新连接上会很快失败。

## Verification

- `platforms/harmony/tests/keyboard-logic.test.ts`：哪些错误码算失效连接、哪些方法重发、只重发一次；资料请求在传输层失败时报 `account_network`。
- `apps/desktop/tests/account/account-operation.test.ts`：`account_network` 的文案。
- 模拟器上 `hvigorw assembleHap` 通过，安装后「我的」页正常。后台闲置 5 分钟再切回，这次没有遇到失效连接，所以重发路径没有在设备上被实际触发过。

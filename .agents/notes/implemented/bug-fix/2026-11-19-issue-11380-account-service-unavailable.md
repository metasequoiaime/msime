# Agent Note: #11380 账号页把断网报成「账号服务暂不可用」

Status: implemented

## Problem

#11380 报告 macOS 0.59.4 打开设置中的“账号与同步”时，设置页已显示“水杉账号已登录”，账号页、设置同步和 QQ 群助手却同时显示“账号服务暂不可用，请稍后再试”。报告者没有补充构建号、重试结果和诊断日志（维护者 2026-11-13 的评论列了五项），所以无法确定报告者那边是哪一步失败。

代码里能确定的一条路径是：访问令牌已经过期、设置应用连不上 `api.msime.app` 时，三处都会落到这句笼统提示。原因有三层：

- `BackendAccountClient::refresh` 和 `profile` 走只返回 `AccountError` 的请求，`send()` 失败一律映射成 `AccountError::Unavailable`，断网和服务端 5xx 分不开。
- 会话取令牌（`access_token`、`refresh_holding_lock`、`credentials_for_login`）只认 `AccountError`；`authenticated_detailed` 的接口（QQ 群助手、设置同步）请求本身能报 `account_network`，但刷新令牌那一步的失败经 `From<AccountError> for AccountFailure` 转换后 `network_failure` 固定为 `false`，码是 `account_unavailable`。
- 账号页读资料的 `account_profile` 命令走 `profile()`，结果只有 `AccountError`；设置同步的 `account_preferences_schema` 和 `account_preferences_load` 也是。

共享页的 `accountMessage` 早已把 `account_network` 写成「网络连接失败，请检查网络后重试。」，缺的只是 Rust 没把这个码交上来。

## Decision

断网与服务端失败的区分从账号 HTTP 层一路带到页面，包括刷新令牌那一步：

- `AccountApi` 新增 `refresh_detailed` 和 `profile_detailed`，默认实现是 `refresh` / `profile` 的结果转成 `AccountFailure`；`BackendAccountClient` 用 `json_detailed` 实现，`send()` 失败经 `AccountFailure::from_send_error` 报成 `account_network`。原来的 `refresh` / `profile` 改为取详细版本的 `error`，返回值与以前相同。
- 会话的刷新路径（`RefreshFlight`、`refresh_holding_lock`、新增的 `access_token_detailed` 和 `credentials_for_login_detailed`）保留 `AccountFailure`。`access_token` 和 `credentials_for_login` 是取 `error` 的包装，只认 `AccountError` 的调用方看到的码不变。
- `authenticated_with_user_at` 取令牌时走详细版本，经 `AuthenticationError::from_failure` 交给调用方：返回 `AccountFailure` 的接口原样拿到 `account_network`，返回 `AccountError` 的接口仍是 `account_unavailable`。
- 新增 `BackendAccountSession::profile_detailed`、`preference_schema_detailed`、`preferences_detailed`。桌面设置应用的 `account_profile`、`account_preferences_schema`、`account_preferences_load` 改用 `call_session_detailed`；C 接口的 `profile`、`preferences_schema`、`preferences_load` 也改走 `detailed`，iOS、Android、macOS 原生回退窗口读到同样的码。
- 服务端回了 5xx 仍是 `account_unavailable`；服务端以 401 拒绝刷新令牌时照旧清掉会话、报 `account_unauthorized`；断网时会话留着，网络恢复后照常刷新。

## Testing

`crates/client-services/src/account/transport_tests.rs` 复现报告的状态：会话里的访问令牌已过期，账号服务地址连不上（本机一个没人监听的端口）。断言 `profile_detailed`、`qq_links`、`preference_schema_detailed`、`preferences_detailed`、`access_token_detailed` 都是 `account_network`，`profile()` 和 `access_token()` 仍是 `AccountError::Unavailable`，会话没有被清掉。另两条测试确认刷新时服务端回 503 仍是 `account_unavailable`、回 401 清会话，以及资料请求本身断开连接是 `account_network`。

## Alternatives considered

- 改共享页 `accountMessage` 的兜底文案，或给 `account_unavailable` 加自动重试：分不出服务端、网络和本地会话故障，会把真的服务端故障说成网络问题，也会掩盖别的错误。
- 给 `AccountError` 加一个 `Network` 变体：所有按 `AccountError` 取码的宿主和已有接口都会看到新码，影响面远超这次要修的几处；`AccountFailure` 已经有 `network_failure`，沿用它。
- 只把账号页的 `account_profile` 改成详细版本：QQ 群助手和设置同步在刷新令牌时断网仍报 `account_unavailable`，截图里三处同时报错的情形只修好一处。

## Consequences

- 断网（含刷新令牌时断网）在账号页、设置同步和 QQ 群助手显示「网络连接失败，请检查网络后重试。」，服务端 5xx 仍显示「账号服务暂不可用」。
- 资料和云端设置读取改走详细接口后，服务端回 404 的码从 `account_unavailable` 变成 `account_not_found`；共享页对这个码没有专门文案，仍显示同一句兜底提示。
- 失败发生在哪一步（刷新令牌还是请求本身）只体现在码上，没有单独记到诊断日志；账号模块目前没有日志通道。
- 报告者那边是否就是断网仍未确认。若是服务端 5xx 或本地会话问题，页面仍会显示「账号服务暂不可用」，需要报告者补充上面五项信息后再查。

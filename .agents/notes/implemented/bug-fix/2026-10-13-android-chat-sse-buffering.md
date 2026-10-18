# Agent Note: Android 对话 SSE 显式请求不压缩的响应

Status: implemented

## Problem

Android 对话请求已经使用 `stream: true` 和 SSE，但手机上短回复容易看起来像一次性出现。Pixel 3 XL 真机诊断确认客户端收到的是 `text/event-stream`：独立的网络测试收到 169 个增量，首个增量约 15 秒后到达，之后约 5.5 秒持续收到增量；另一条界面测试收到 13 个增量，在约 0.4 秒内结束，期间界面更新 4 次。已观察到的是首段等待和快速输出，没有确认网络压缩是本次反馈的根因。

Android 的 `HttpURLConnection` 默认请求 gzip。SSE 需要事件及时传递，显式请求不压缩，避免传输链路未来引入压缩缓冲，是这次修改的防御措施。[Android 官方文档](https://developer.android.com/reference/java/net/HttpURLConnection)说明了默认压缩和 `Accept-Encoding: identity` 的用法。

## Decision

Android 的 SSE 请求明确发送 `Accept-Encoding: identity`，保持事件以未压缩响应逐段传递；已有的 `ChatFragment` 增量回调和 50 ms 界面刷新节流继续沿用。

## Alternatives considered

- **只调整界面刷新间隔**：它只能改变已经收到的增量如何绘制，无法消除网络层对 SSE 的压缩缓冲。
- **保持 Android 默认的压缩协商**：能节约流量，而且当前实测已有持续增量；不用默认值，是为了明确 SSE 传输的低延迟要求，减少中间层压缩带来的不确定性。

## Consequences

流式响应不再主动请求 gzip，代理更容易立即转发每个 SSE 事件；代价是回复正文不使用传输压缩，流量略有增加。首个 token 仍受上游模型延迟影响，不承诺立即出现。

从 #8201 迁移计划 5c 起对话改经 Rust 的 `msime_client_account_chat_stream`：Rust 的 reqwest 没有开 gzip 之类的特性，不发 `Accept-Encoding`，服务端和中间层按不压缩回答，效果与这里显式请求 identity 相同。见 [Android 设置页面的账号与云端操作改调 Rust](../architecture/2026-10-18-android-settings-pages-through-rust.md)。

## Verification

- 临时诊断仅记录增量数和耗时，不记录消息、凭据或账号资料；诊断代码不进入最终 APK。
- 修改前真机的网络和界面测试见 Problem；另一条界面测试收到 24 个增量、期间更新 15 次。
- 最终 APK 真机网络测试：194 个增量，首段约 5.9 秒、末段约 14.8 秒，增量持续约 8.9 秒；没有退回非流式响应。临时测试包已卸载。
- Android 对话页契约检查、笔记门禁和 APK 构建通过。最终 APK 版本仍为 `0.4.0`。

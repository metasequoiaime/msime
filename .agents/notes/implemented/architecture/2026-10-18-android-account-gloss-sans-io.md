# Agent Note: Android 键盘的「水杉账号」释义改用 msime_client_account_gloss

Status: implemented

## Problem

#8201 迁移计划 5e。Android 键盘（`:ime` 进程）的「水杉账号」候选释义由 `voice/BackendTranslationClient.java` 自己做全套：Java 校验原文（每条不超过 2048 字节）、拼 `/v1/translate` 的请求体、经 `CloudApi` 先向 provider 要登录账号的令牌（`access_token`，带账号编号）、要不到再要匿名令牌，自己解读回答。解读规则是第四份（Rust、Windows 曾经那份、iOS 曾经那份、Java）：条数对上以后任何一条为空、带换行或控制字符、超过 4096 字节，整页释义作废。

client-services 早有这个接口的 sans-IO 一半 `msime_client_account_gloss`（request 校验并给出请求，parse 按状态码和服务端错误码解读，坏的一项只置 null），Windows、iOS 键盘扩展已经在用，Linux 用的是带 HTTP 的 `translate`。`:ime` 不注册 Rust 的会话存储（负责人决定，见 [Android 的登录会话、令牌、登录和刷新改由 Rust 持有](2026-10-18-android-account-through-rust.md)），所以 Rust 那一半不能连 HTTP 和令牌一起做。

## Decision

- `BackendTranslationClient.translate` 先调 Rust 的 `request`（`source_lang` 固定 ZH，目标语言原样交给 Rust 校验、转大写），拿到 method、path 和 body；Rust 拒绝时直接失败，不向主进程要令牌。
- 令牌只有一种来源：`BackendAnonymousAccount.accessToken`，即 Rust 的 `msime_client_account_access_token`（登录账号优先、否则本机匿名账号）。`:ime` 经 `AccountSessionProvider` 的 `shared_access_token` 向主进程要，主进程在两种会话都没有时补注册匿名账号。
- 请求发到 `CloudApi.ORIGIN` 加 Rust 给的路径，头是 `Accept`、固定的 `MSIME/Android`、Bearer 和 `application/json`。服务端回 401 时把被拒的令牌交回 provider 换一枚，换到不同的才用同一份请求体重试一次，第二次 401 不再重试。
- 状态码、响应体和 `Retry-After` 原样交给 Rust 的 `parse`：2xx 时得到与原文等长的 `[string|null]`，null 由 `CandidateTranslationStore.absorb` 跳过；其余状态码是账号错误码（`AccountStore.Failure`，限流带 `retry_after_seconds`），store 照旧吞掉。
- 不跟随重定向、30 秒超时、响应最多 256 KiB 不变；超过上限时不交给 Rust（parse 请求要把整个响应体带进 JSON，JNI 那一头的上限是 4 MiB）。
- 换号以后才到的旧回答只由 store 挡：`MSIMEInputService.verifyCandidateTranslationBinding` 在发请求前和回答到达时各读一次 `SyncSignals.binding`，`CandidateTranslationStore.bindTo` 在绑定变了时清空缓存（`CandidateTranslationStoreSmoke` 覆盖）。客户端自己不再核对账号编号。
- Java 的解读（`parseResponse`、`parseValues`、`successStatusCode`）和它的 JVM 冒烟 `CandidateTranslationResponseSmoke` 删掉；那几种整页拒绝的情形（`code` 是 200.5 或 true，某一项是 null 或数字）补进 client-services 的 `a_malformed_envelope_fails_the_whole_response`。`NativeClient.accountGloss` 是新的包内入口，JNI 的 `accountGlossRaw` 在 5a 已经导出。
- `check-host.sh` 守着：Java 里不能出现 `"/v1/translate"`，`BackendTranslationClient` 要调 `NativeClient.accountGloss`。

## Alternatives considered

- **让主进程经 provider 代 `:ime` 调 `msime_client_account` 的 `translate`**：Rust 连 HTTP、刷新和同一账号重试一起做，Java 一行解读都不用写，Linux 就是这样，这是它最强的理由。不用它：每翻一页候选都要多一次跨进程调用，并把主进程拉起来做按键路径上的联网；计划第 6 节写定的是「令牌经 provider 的 shared_access_token」，provider 只给令牌，与云剪贴板的端到端驱动一致。
- **`:ime` 也注册会话存储，直接用 `translate`**：负责人已经否掉，`:ime` 保留 `AccountSessionProvider`、不刷新、不注册，否则两个进程各自缓存会话，旋转的刷新令牌会被花两次。
- **provider 的 `shared_access_token` 多回一个 `user_id`，客户端照旧在 401 重试前核对账号**：能留住原来「换号后不重试、不交出旧回答」的那道检查。不加：store 已经按账号绑定挡住旧回答，多出来的只是换号瞬间用新账号的令牌多发一次同一页的请求；Windows 和 iOS 扩展也只在 store 一层隔离。

## Consequences

- **收益**：一页里有一条坏值时其余照常显示，带换行的译文折成一行，与原文相同的不再当作释义；解读规则只剩 Rust 一份。原文的长度上限按服务端放到 8192 字节（原来 Java 卡 2048）。
- **代价与已知上限**：
  - 只用 Tauri 开发合包登录的用户，原来 provider 会读合包那份会话给登录令牌，现在 `shared_access_token` 只看 Rust 的注入存储，这类用户改用本机匿名账号释义。
  - 换号与一次 401 撞在一起时，重试那一次带的是新账号的令牌；回答到达时 store 发现绑定变了，整个缓存清空、不显示。
  - 服务端回任意 2xx 都交给 Rust 解读（原来只认 200）。
- **没有验证到的**：设备套件里改写的 `BackendAccountSessionDeviceSmoke` 释义四项（真的 Rust 拼请求体、换令牌重试一次、坏的一项置 null 与限流的 Retry-After、256 KiB 上限）要在 API 28 和 35 的模拟器上跑，见 PR 正文。这四项第一次上设备（5f 的验收）就 `UnsatisfiedLinkError`：这个 instrumentation 原来以测试包自己为目标，跑在没有 `libmsime_android.so` 的测试包进程里；之后改成以产品包 `app.msime.android` 为目标、跑进应用进程，`check-host.sh` 按 jdeps 的类引用拦着以测试包为目标、却能引用到 `NativeClient` 的 instrumentation。

## Verification

- `platforms/android/check-host.sh`：新的 JVM 冒烟 `BackendTranslationClientSmoke`（Rust 那一半换成内存实现）核对原文和目标语言原样交给 Rust、只要一枚令牌、Rust 的方法路径和请求体原样发出、401 换到不同令牌才重试且只重试一次、Rust 拒绝请求时不要令牌也不联网、失败的状态码连同正文和 Retry-After 交给 Rust、超过 256 KiB 不交给 Rust。逐个改回（同一令牌也重试、不处理 401、换令牌时不交回被拒的令牌、先取令牌再校验、去掉上限、丢掉 Retry-After、去掉 Bearer 前缀）都失败；往 Java 里写 `"/v1/translate"`、去掉 `NativeClient.accountGloss` 时守卫失败。
- `cargo test -p msime-client-services --lib -- account::translate`、`cargo test -p msime-host-api --lib -- gloss`。
- 相关：[Windows 账号释义改用 msime_client_account_gloss](2026-10-17-windows-account-gloss-sans-io.md)、[iOS App 的账号、云端页面和云同步改调 Rust](2026-10-18-ios-account-through-rust.md)、[Android 候选翻译接入用户自己的翻译服务](../feature/2026-10-12-android-user-translation-services.md)。

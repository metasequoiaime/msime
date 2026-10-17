# Agent Note: Windows 账号释义改用 msime_client_account_gloss，账号请求自报 User-Agent，注销改用 account_delete

Status: implemented

## Problem

Windows 的「水杉账号」候选释义（见 [Windows 的释义列快捷键与「水杉账号」候选释义](../feature/2026-10-10-windows-gloss-columns-and-account-glosses.md)）自己拼 `/v1/translate` 的请求体、自己解读回复（`AccountGlossPolicy.h` 的 `account_gloss_values`）：条数对上以后，任何一条带换行、控制字符或超过 4096 字节，整页释义作废；`code` 写成字符串 `"200"` 也照收。client-services 早有同一个接口的 sans-IO 一半 `msime_client_account_gloss`（请求体和逐项规范化，坏的一项只置 null，换行折成空格），Linux 已经在用。

Server 启动时注册匿名账号、释义线程补注册、设置窗口的账号请求都不带 User-Agent，服务端「我的设备」把 Windows 显示成 Android。设置窗口注销用旧的 `delete`，会话不是 10 分钟内登录时它报的是 `account_unauthorized`，页面说「登录已失效，请重新登录」，用户以为被登出了。

## Decision

- `TranslationWorker::account_glosses` 的请求体取自 `msime_client_account_gloss` 的 `request`，回复交给 `parse`（只有 2xx 才有正文，`http_request` 对其余状态码返回空，401 照旧带着被拒的令牌重取一次）。curl、bearer、10 秒超时和 `stopping()` 中止不变。`account_gloss_values` 只剩把 Rust 的 `[string|null]` 换成每个词的释义：条数对不上整批作废，null 记作没有释义、进否定缓存。`account_gloss_url` 换成只有来源的 `account_gloss_origin`，路径用 Rust 给的。
- 新的 `common/ClientAgent.h` 读设备型号（`HKLM\HARDWARE\DESCRIPTION\System\BIOS` 的 `SystemProductName`）和系统版本（`RtlGetVersion`，内部版本号 22000 起写成 Windows 11），不带 JSON：Server 用 nlohmann、设置窗口用 WinRT JSON 各自拼 `msime_client_client_agent` 的请求。Server 启动时拼一次，注册匿名账号改调 `msime_client_ensure_anonymous_account_with_agent`，同一个字符串经 `TranslationWorker::set_account_directory` 交给释义线程的补注册；设置窗口每个 `msime_client_account` 请求都带同一个 `client_agent`（一个进程只拼一次，头文件要求同一个宿主的各次调用都带同一个）。拼不出时照旧不带。
- 设置窗口的「注销账号」改用 `account_delete`；`AccountPageModel.h` 的 `error_message` 补上 `account_network`（「网络连接失败，请检查网络后重试。」）和 `account_recent_login_required`（「需要重新登录后再注销：请先退出登录，重新登录后再试。」），与共享页 `accountMessage` 相同。

## Alternatives considered

- **在 C++ 里照 Rust 的规范化再改一遍 `account_gloss_values`** — 不跨 FFI；但这是第三份释义规范化规则（Rust、Linux 经 C 接口、Windows），4096 字节和与原文相同的判断已经各写过一次。
- **让 `msime_client_account` 的 `translate` 连 HTTP 一起做** — Windows 不用再管 curl；但它阻塞到拿到回复，释义线程在 Server 退出时就停不下来，而 `stopping()` 中止是这条路一直保住的东西。

## Consequences

- **收益**：一页里有一条坏值时其余照常显示，带换行的译文折成一行；新注册的匿名账号和新登录在「我的设备」里显示 windows、版本号和设备；登录超过 10 分钟后注销有明确提示，本机登录保留。
- **代价与已知上限**：`code` 是字符串 `"200"` 的回复不再收（服务端一直回整数）。`client_agent` 只对之后的注册和登录生效，已有的会话仍按注册时的 User-Agent 显示。注销后 Rust 会尽力删本机匿名账号的语音样本，Windows 从不做语音贡献，这一步通常什么也删不到，结果不提示。

## Verification

ctest `windows-account-gloss-policy`（链接 host 库，经 `msime_client_account_gloss` 核对换行折叠、坏值与原文相同置空、条数不对整批作废）、`windows-account-page-model`（两句新文案）、`windows-ai-endpoint-policy`。

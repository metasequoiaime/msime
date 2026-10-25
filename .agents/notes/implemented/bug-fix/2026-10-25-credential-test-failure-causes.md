# Agent Note: 配置测试说出余额不足、服务端故障和豆包握手被拒

Status: implemented

## Problem

#8990 报告豆包语音识别和 DeepSeek 润色都「填了正确的 API、选了正确的模型，但测试不通过」，没说平台、版本和界面上的提示。对照两家现行文档逐项核对了共享探测（`crates/client-providers/src/credential/`）：DeepSeek 的地址 `https://api.deepseek.com/chat/completions`、`Authorization: Bearer`、`thinking: {"type": "disabled"}` 和默认模型都对（文档现在推荐 `deepseek-flash`，`deepseek-v4-flash` 作为旧名仍被接受，由 V4.1 Flash 应答）；豆包的握手头（`x-api-key`，旧版控制台 `x-api-app-key` 加 `x-api-access-key`，`x-api-resource-id`、`x-api-request-id`）、首包与末包的帧头、序号和 gzip 都与《大模型流式语音识别 API》一致。没有凭据，无法在真实服务上复现「成功的请求被判失败」。

能确定的缺陷在失败提示上。DeepSeek 错误码表里 402 是余额不足，500 / 503 是服务端故障，`test_chat` 却把 401、403、429 以外的状态都说成「服务拒绝请求，请检查接口地址和模型配置」：新账户没充值时，用户照提示核对地址和模型，当然找不到错。豆包测试更糟，所有失败只有一句「豆包测试未成功或超时，请检查凭据、资源权限、接口地址和网络」；实测豆包对无效密钥在握手时回 HTTP 401 和 `{"error":"Invalid X-Api-Key"}`，这个状态码本来足以告诉用户是密钥的问题，探测却把它和超时、断线、协议错误一起丢掉了。

## Decision

- `credential::probe::rejected_status_message(status, other)` 是 HTTP 测试被拒时的唯一文案来源：401 / 403 是认证失败，402 是账户余额不足、去服务商控制台充值，429 是限流或额度不足，5xx 是服务暂时不可用、稍后重试，并提醒一直如此时确认服务支持所填的模型，其余状态用调用方给的建议。5xx 不能只说「稍后重试」：one-api 的 `middleware/distributor.go` 对「当前分组下对于模型 xxx 无可用渠道」回 503，填了中转没有的模型名的用户会照着提示一直重试。每句都带上「（HTTP N）」，从不带服务端正文。对话探测 `test_chat`（AI 辅助、语音润色）和整句识别探测 `credential::asr::test` 都走它。
- 豆包探测的 `Transport::probe` 返回 `Result<(), Failure>`。`Failure` 只有五类：`Rejected(u16)` 是握手没升级成 WebSocket 时的 HTTP 状态码（从 tungstenite 的 `Error::Http` 取，正文不读），`Service(Option<i64>)` 是连上后的服务端错误（错误帧带它的数字错误码，例如 45000001 请求参数无效；或结果 JSON 里的非零 `code`、非空 `error`），`Connection`、`Timeout`、`InvalidResponse`。设置页按类说出该改哪一项：401 让用户确认密钥来自豆包语音控制台、鉴权方式与之对应，403 让用户检查权限和资源 ID 对应的服务是否开通，握手 5xx 和《大模型流式语音识别 API》错误码表里的 550xxxxx（服务内部处理错误，55000031 是服务器繁忙）说服务暂时不可用、稍后重试，其余错误码（450xxxxx 是参数无效、空音频、音频格式这类请求问题）原样给出并让用户检查资源 ID 和识别选项。错误帧后面的 UTF-8 错误文字和握手响应的正文仍然不解析、不转发。
- iOS 的「测试连接」不走共享 Rust：`CustomServiceClient.test` 自己发请求，只要 2xx。HTTP 被拒的文案改由 `CustomService.swift` 里的 `CredentialStatusPolicy` 给出，豆包的由 `DoubaoVoiceCoordinator.swift` 里的 `DoubaoFailureMessage` 给出，都与共享 Rust 逐字相同。豆包握手被拒时 `URLSessionWebSocketTask` 只报笼统的 -1011，状态码留在 `task.response` 上，`DoubaoWebSocketTransport` 从那里取出抛 `rejected(status:)`。iOS 的帧解码以前把错误帧当成没有文字的最终结果，测试遇到 45000001 之类的错误帧反而报连接成功；现在 `DoubaoHostFrameCodec.decodeFrame` 对带数字错误码的错误帧抛 `DoubaoVoiceCoordinator.Failure.service(code:)`，听写遇到错误帧也说出错误码，不再是「豆包未返回可用的语音文本」。
- HarmonyOS 的设置页自己发对话和整句识别的测试请求（`pages/Settings.ets`），文案改由 `keyboard/settings/CredentialStatusPolicy.ts` 给出，与共享 Rust 逐字相同。HarmonyOS 的豆包测试走 `@ohos.net.webSocket`，`connect` 只回布尔值，拿不到握手状态码，这一项没有跟进。

## 历史笔记审计

- [AI 配置测试兼容需要思考的模型](../../implemented/bug-fix/2026-10-13-ai-credential-reasoning-probe.md) 部分重叠：它把预算耗尽、无效响应、超时与网络错误分成固定说明，本篇在同一原则（不透传服务端文本和凭据）下把 HTTP 状态码细分并写进文案，豆包探测补上同样的分类。
- [Android 语音润色与凭据测试](../../implemented/feature/2026-10-12-android-voice-polish-and-credential-test.md) 部分重叠：Android 经 `msime_client_test_voice_credential` 调同一个 `credential::test_service`，自动得到新文案，宿主不变。
- [Windows 设置语音页 host API](../../implemented/architecture/2026-10-11-windows-settings-voice-host-api.md) 部分重叠：`msime_client_test_api_credential` 的 `{ok, message}` 形状不变，只是 `message` 更具体。

## Alternatives considered

- 把服务端的错误文字（DeepSeek 的 `error.message`、豆包的 `{"error": …}`）直接显示：最省事，信息也最全；但这些文字可能带着部分密钥（DeepSeek 401 的正文就回显了密钥末四位）和请求 ID，探测从一开始就约定不把它们带到设置页，状态码和数字错误码已经足以区分该改哪一项。
- 按服务商写不同的文案（只对 DeepSeek 说 402 是余额不足）：更贴切，但 402 在 OpenRouter 等服务上也是余额不足，HTTP 语义本身就是「需要付费」，没有哪家把它用作别的意思，按状态码统一处理更少分叉。
- 把默认模型改成 DeepSeek 文档现在推荐的 `deepseek-flash`：旧名仍被接受、由同一模型应答，不是测试失败的原因；改它要同步偏好默认值、各平台目录产物和几十处测试断言，与本次修复无关，留给目录更新时一起做。

## Consequences

用户看到的失败提示从「检查接口地址和模型」变成能对上原因的一句话，带状态码也方便在 issue 里报告；代价是文案多了几种，HarmonyOS 有一份 TS 副本、iOS 有一份 Swift 副本需要和 Rust 同步，豆包在 HarmonyOS 上仍是笼统的一句。iOS 豆包测试仍只要握手成功、收到最终帧，不像 Rust 那样检查结果 JSON 里的非零 `code`。没有改变任何请求的内容和判定成功的条件，原来通过的测试仍然通过。

## Verification

- `rbuild cargo test --locked -p msime-client-providers --lib credential`：45 项通过，新增 `credential_chat_names_balance_and_server_failures_with_their_status`（402、5xx 不再提接口地址）、`rejected_handshake_reports_its_status_without_the_body`（本机假服务回 401 和 JSON 正文，探测得到 `Rejected(401)`）、`each_failure_names_its_cause_and_never_echoes_input`，原有的握手不跟随重定向、帧序列、超时测试改为断言具体的失败类别。把 `test_chat` 的状态分支临时换回旧写法，新测试在 402 处失败。
- `rbuild cargo clippy --locked -p msime-client-providers --all-targets -- -D warnings` 通过。
- HarmonyOS：`tsc --project platforms/harmony/tests/tsconfig.json` 编译通过，`credential-status.test.js` 8 项断言通过；`Settings.ets` 未在本机做 ArkTS 构建，交给 CI。
- iOS：`MSIMEDoubaoTransportTests` 新增 `testRejectedHandshakeReportsItsStatusWithoutTheBody`（本机 TCP 服务回 401、503 和 JSON 正文，传输层得到 `rejected(status:)`）；`MSIMEServiceTests` 新增按状态码给提示和豆包握手被拒、错误帧、超时的用例。
- 用伪造密钥直连两家服务确认文档描述的形状：豆包握手回 HTTP 401、`{"error":"Invalid X-Api-Key"}`；DeepSeek 回 HTTP 401、`authentication_error`。没有真实密钥，余额不足和豆包资源未开通的真实响应没有验证。

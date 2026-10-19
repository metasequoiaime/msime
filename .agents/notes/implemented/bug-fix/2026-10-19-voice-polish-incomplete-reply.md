# Agent Note: 语音润色拒绝不完整回复

Status: implemented

## Problem

macOS 与 Windows 的原生语音流程调用共享 `polish_cloud_text`，Linux 用户会话语音服务调用自己的 `polish`，Android 识别窗口调用 `VoicePolisher.polish`，iOS 语音页通过 `CustomServiceClient.request(kind: .ai)` 调用 `AppServicesBridge.parseResponse`，失败时都保留原始转写。这几条解析路径原先只检查 HTTP 成功和非空 `choices[0].message.content`，忽略同一条回复的 `finish_reason` 与顶层错误。因此服务返回 HTTP 200 和半段正文时，宿主会把完整转写替换成不完整结果，绕过原有回退。iOS 的非语音解析分支还服务于通用 AI 请求，具有同样的缺口。

## Decision

共享 C++ 解析器、Linux `polish` 与 Android `VoicePolisher.content` 在取正文之前拒绝非 `null` 的顶层 `error`，以及首个候选的 `length`、`content_filter` 结束原因。C++ 抛出固定 `VoiceError`，由现有宿主回退保留转写；Linux 在现有 HTTP worker 返回后的 `polish` 入口检查状态，直接返回传入的原始转写，沿用独立进程的超时与取消边界；Android 解析器返回空字符串，由现有 `sendable` 校验转换成 `null`，识别窗口原有的 `polished` 回退保留转写。不传递服务端错误正文。未提供结束原因、结束原因为 `null`、顶层 `error: null` 和正常 `stop` 结果保持兼容。

iOS 在 `AppServicesBridge.parseResponse` 的非语音分支应用同一规则，抛出固定 `ServiceFailure`，让语音页原有的失败处理保留转写，通用 AI 请求也拒绝这些明确失败的正文。JSON `null` 以 `NSNull` 判断，不能按真假值判断顶层错误。`voice: true` 的转写解析保持原有契约。

HarmonyOS 的 `HarmonyVoicePolisher` 将 HTTP 状态与字符串正文交给现有 `VoiceResponsePolicy.polishResult`，按同一规则返回 `null`，识别器沿用 `polished ?? finalText` 保留完整转写。该方法保留原有 1 MiB 响应上限、16 KiB 文字上限、控制字符检查和文字规范化；解析异常静默失败，不再进入网络异常日志。请求创建、重定向保护、超时、取消与释放仍由原润色类负责。

## 历史笔记审计

- [转写完整性](2026-10-19-voice-asr-incomplete-reply.md)：部分重叠，共享 C++ 转写入口沿用相同状态规则，失败时不投递正文；本篇的润色回退契约保持不变。
- [Windows 原生语音对齐](../../implemented/feature/2026-10-10-windows-native-voice-mac-parity.md)：部分重叠，保留润色失败静默回退和 30 秒预算，本篇补齐哪些回复应当失败。
- [原生语音设置接口](../../implemented/architecture/2026-10-11-windows-settings-voice-host-api.md)：无关，配置探测与实际转写润色是不同入口。
- [macOS 语音设置裁剪](../../implemented/simplification/2026-10-16-macos-voice-settings-prune.md)：无关，本篇处理实际语音请求，不恢复旧设置控件。

## Alternatives considered

- 只检查 HTTP 成功和非空正文：能兼容更多服务并使用模型已经写出的文字，但无法告诉宿主这只是半段结果，会覆盖完整转写。
- 强制 `finish_reason: stop`：判断最简单，但既有兼容服务可能省略或置空该字段；只拒绝明确报告的截断与过滤，避免扩大兼容性变化。
- 在各宿主各自检查：靠近转写回退位置，但会复制协议规则。共享 C++ 入口是 macOS 与 Windows 的共同路径，应在这里拒绝不可用结果；Linux 用户会话服务、Android 和 iOS 的独立解析入口按同一规则处理。HarmonyOS 的响应策略已有转写与流式回复解析，把润色规则放在同一策略层可以复用正文解析和现有纯逻辑测试入口；保留在系统 HTTP 类内则需要另建 ArkTS 运行替身与转译工具。

## Verification

新增回归在旧实现下先失败：`shared-voice-http-transport` 接受了 `length` 的非空结果，`voice-polish-request` 未回退原始转写。回归通过真实共享 curl 适配器与 macOS 原生 `MSIMEHTTPVoiceRequest` 发出本机合成 HTTP 请求，不访问外部服务、不使用真实输入或凭据。修复后两项通过，独立审查复跑也通过。`pnpm run verify-notes`、`git diff --check`、macOS 完整构建和本地 quick 门禁通过；quick 包含 Linux 90 项测试和 Windows 168 项可移植测试。本机缺少 MinGW 交叉工具链，Windows 完整构建被跳过；HarmonyOS 真打包因缺少暂存库与资源被跳过。

Linux 的 `scripts/test-linux-voice-polish-reply.py` 经真实 `polish` → `polish_request` → HTTP worker 子进程访问合成回环服务，12 种回复覆盖明确失败与兼容成功响应。旧实现出现 7 个预期失败，修复后通过。测试位于自动发现的 `scripts/test-*.py`，仓库契约门禁实际执行通过；不支持 Unix 信号的主机明确跳过。回环请求不经过代理，不访问外部模型服务。独立审查与本地 quick 门禁通过，后者包含 Linux 桌面外壳编译、原生 IBus/Fcitx5 构建、90 项原生测试，以及版本安装产物检查。未进行 Linux 系统输入法中的麦克风录音和交互验收。

Android 的 `VoicePolishReplyDeviceSmoke` 把同一份产品源文件编进 `editor-test.apk`，以测试包为 instrumentation 目标，通过反射调用私有 `content` 入口，不扩大生产 API。API 28 与 API 35 的真实 `org.json` 都在旧实现下出现相同的 7 个预期失败，修复后各自 12 种回复全部通过。`smoke.sh` 在 `--core` 返回之前执行这项回归，最低 API 的核心验收也覆盖它。独立审查和本地 quick 门禁通过，后者包含 Rust workspace、Android target、宿主 Java 与设备测试源码编译，以及既有 JVM 冒烟。这里只验证回复解析，未验证真实 HTTPS 服务、麦克风录音或完整语音交接交互；测试不联网、不录音，不读写用户设置。

iOS 的 `MSIMEServiceTests.CustomServiceTests` 通过 `URLProtocol` 替身返回合成 HTTP 200 回复，实际经过 `CustomServiceClient.request` → `URLSession` → `AppServicesBridge.parseResponse`，按请求路径确定回复，不共享可变测试状态。新增两项测试覆盖 13 种回复：截断、过滤、六种非 `null` 错误、空候选和四种兼容成功状态；旧实现出现 8 个预期失败。修复后在 Xcode 27 / iOS 27.0 模拟器上，整个 `CustomServiceTests` 的 30 项测试通过。已暂存校验资源、编译 Rust 模拟器静态库并运行 XcodeGen；生成差异仅为默认构建设置，没有源文件注册变化，丢弃这些无关差异后使用仓库工程完成修复后的验证。独立审查、`pnpm run verify-notes`、`git diff --check` 和本地 quick 门禁通过。未访问外部模型服务、录音或进行语音页完整交互验收。

HarmonyOS 的 `voice-polish-reply.test.ts` 由现有 `tests/run.sh` 自动发现。先将既有解析行为移到响应策略，再运行新增回归，截断、过滤和六种非 `null` 错误出现 8 个预期失败；加入状态判断后 22 种回复与边界情况全部通过，HarmonyOS 逻辑和 C++ 套件也通过。已从当前检出构建原生库、暂存校验资源与设置页、执行 `ohpm install` 后完成 `hvigorw assembleHap --no-daemon`。实际 HAP 编译暴露并修复已有的词库更新参数对象类型错误，不改变参数字段或更新流程。独立审查复跑类型检查、新回归和差异检查通过，本地 quick 门禁与笔记校验通过。测试不联网或录音；HAP 未签名，未在设备上验收系统 HTTP 或完整语音交互。

## Consequences

- 明确报告截断、过滤或错误的回复走现有失败回退，完整转写得以保留。失败没有新增日志或服务端错误透传。
- 未提供结束原因或提供 `null` 的旧服务继续兼容，因此仍不能保证这些服务的输出完整性。
- 覆盖范围是使用共享 C++ 润色入口的 macOS、Windows 原生流程、Linux 用户会话语音服务、Android 识别窗口使用的 `VoicePolisher`、iOS 语音润色与通用 AI 请求共用的非语音解析分支，以及 HarmonyOS 的原生语音润色。各平台的转写协议解析不受本篇修复覆盖。
- 请求参数、润色预算和错误提示沿用现有规则；明确失败时不再展示服务已经生成的部分正文。

## Sources

[OpenAI Chat Completions 响应说明](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/retrieve) 将 `length` 定义为输出预算耗尽，将 `content_filter` 定义为正文因过滤而省略。官方 Python SDK 的 `lib/_parsing/_completions.py` 在解析前也对这两种结束原因抛出异常。

[Android `JSONObject` 源码](https://android.googlesource.com/platform/libcore/+/refs/heads/main/json/src/main/java/org/json/JSONObject.java) 的 `isNull` 同时接受缺省字段和 JSON `null`；`false`、`0`、空字符串与空对象不属于空值，因此顶层错误不能按真假判断。

# Agent Note: 自定义翻译服务识别 DeepL 官方 API

Status: implemented

## Problem

#6829：用户想用自己的 DeepL API 密钥（Free 或 Pro）做候选词翻译。「自定义」翻译服务只会说 DeepLX 的协议：所有宿主都发 `Authorization: Bearer <key>`，正文的 `text` 是单个字符串。DeepL 官方 API 要求 `Authorization: DeepL-Auth-Key <key>`、`text` 为字符串数组，于是把 DeepL 地址填进「自定义」只会得到 403，设置页提示「服务未返回有效译文」。解析回复不是问题：`translations[0].text` 早就能读。

请求在五处各自拼：`crates/host-api/src/ffi/translation.rs` 的运行时描述器（Windows、macOS、iOS、鸿蒙键盘都发它）、`crates/client-core/src/credential/translation.rs` 的设置页连接测试（Windows、macOS 的 Tauri 设置页）、Linux 的 Python provider、鸿蒙设置页自己的连接测试。

## Decision

- 不加新的翻译服务选项，也不加偏好字段。「自定义」服务按地址识别 DeepL：https、默认端口、主机名恰好是 `api.deepl.com` 或 `api-free.deepl.com` 的地址按 DeepL v2 发请求，其他地址保持 DeepLX 兼容格式不变。
- 识别只看主机名，不看路径。自建 DeepLX 也提供 `/v2/translate`，按路径识别会改掉这些用户现在能用的请求。DeepL 地址的路径统一改成 `/v2/translate`，用户只填主机名也能用。
- DeepL 请求：`Authorization: DeepL-Auth-Key <key>`，正文 `{"text":[文本],"source_lang":"ZH"|"EN","target_lang":…}`。`target_lang` 中文写 `ZH-HANS`、英文写 `EN-US`（DeepL 文档把不带变体的 `ZH`、`EN` 标为只为兼容保留），其余语言直接用大写代码（FR、JA、ES、RU、DE、KO 都在 DeepL 的目标语言里）。没有密钥时不发请求。
- 主机按密钥选：以 `:fx` 结尾的是 Free 密钥，走 `api-free.deepl.com`，否则走 `api.deepl.com`。这是 DeepL 官方客户端库判断账号类型的做法；用户把 Free 密钥填在 Pro 地址（或反过来）时 DeepL 只会回 403，这里直接改到对的主机。
- 源语言照常写明，不让 DeepL 自动识别。候选词多是两三个字，「手紙」「大丈夫」这样的词自动识别会当成日语，译文就不是中文用户要的意思；规划阶段已经按文字判断了方向。
- 请求构造集中在 `msime_client_core::translation::custom_translation_request`。运行时描述器和设置页连接测试都调用它；鸿蒙设置页的连接测试改为调用 NAPI 的 `customTranslationHttpRequest` 和 `parseCustomTranslationResponse`，不再自己拼请求。Linux provider 是独立的 Python 进程，链接不到 Rust，在 `custom_translation_request` 里照写一份，由 `shared/contracts/custom-translation/cases.json` 约束：Rust 单测和 `platforms/linux/tests/dictionary/custom_translation_contract.py` 逐条跑同一组用例。
- 仍然一词一请求。DeepL 一次能收 50 段文本，但「自定义」在五个宿主上都是逐词请求、逐词缓存和负缓存；要批量就得给每个宿主新开一条腾讯那样的分组路径。字符额度按字符计，批量不省额度，只省往返。
- 设置页的连接测试遇到 DeepL 的 403（密钥无效）、456（额度用完）、429（过于频繁）给出对应的提示，只用状态码，不转述服务端正文。Rust 测试与鸿蒙设置页的文案逐字相同。鸿蒙那一份连同探测请求、DeepL 识别（按描述器的 `DeepL-Auth-Key` 头）、请求头和回复门槛放在 `keyboard/settings/CustomTranslationProbePolicy.ts`，`Settings.ets` 只负责发请求和调 NAPI：`platforms/harmony/tests/custom-translation-probe.test.ts` 在 node 下跑这些逻辑，期望文案直接从 Rust 的 `deepl_failure_message` 读出，DeepL 识别逐条对照共享用例的描述器；`scripts/test-harmony-custom-translation-probe.py` 不需要工具链，每次推送都按文本核对文案与 Rust 一致、两边探测同一个请求，以及连接测试仍走共享描述器（`customTranslationHttpRequest`）、共享解析（`parseCustomTranslationResponse`）和这个策略模块。去掉鸿蒙这一份文案要给 Host API 新开一个导出（头文件、FFI、NAPI 和类型声明各改一处），目前没有现成的共享入口，所以保留副本、由两道核对锁住。Linux 的 HTTP 子进程只回报成功或失败，仍是笼统的失败提示：要分出三种状态码，得让子进程协议带回状态码、provider 的连接测试回复多一个字段，再由设置页一侧映射文案，改动跨 Python provider、套接字协议和 Tauri 三层，不在这次范围内。
- DeepL 地址没有密钥时，共享构造不出请求，键盘每个词都拿不到释义，所以每个设置页都要在保存时说出来，而不是静默没有释义：macOS 原生窗口用共享描述器校验，不让保存；iOS 的 `TranslationProviderPreference.route(in:)` 同样问共享描述器能否构造请求，构造不出就算不可用，设置页显示「凭据不完整」，键盘也不再为它保留释义行（远程 http 地址同理，这类地址原本也发不出去）；共享 `packages/ui`（Windows、Linux、macOS 的 Tauri 设置页和鸿蒙设置页）在 API Key 下提示「DeepL 官方 API 需要填写 Auth Key」并禁用连接测试。网页设置页同步拿不到 Rust，`isDeepLApiEndpoint` 照写主机判断，由 `shared/contracts/custom-translation/cases.json` 的同一组用例约束。
- macOS 描述器校验的 `Authorization` 上限从 4103 提到 4111：`DeepL-Auth-Key ` 比 `Bearer ` 长 8 字节，4096 字节的密钥加前缀正好 4111。
- 设置页文案（共享 `packages/ui`、macOS 原生翻译窗口、iOS）改为「自定义（DeepLX 或 DeepL API）」，说明可填 DeepL 官方地址加 Auth Key，并提示使用 DeepL 时候选词会发送到 DeepL、按 DeepL 的数据政策处理。`PRIVACY.md` 的候选翻译一节写明 DeepL 地址按密钥改发到 Free 或 Pro 主机。DeepL API Free 与 Pro 的数据处理条款不同，文案不替 DeepL 承诺具体的保留期限。

## Alternatives considered

- **单独加一个「DeepL API」服务**：用户只填 Auth Key，用 Free/Pro 开关或 `:fx` 选主机，这是 #6829 字面上的要求，也最不容易填错地址。代价是新的 `deepl` 偏好段、新的 `TranslationService` 变体、`providers.rs` 的优先级、以及 Windows、macOS、iOS、鸿蒙、Linux 每个宿主的分发和设置页都要改，Android 还没有自定义翻译的运行时。识别地址只动请求构造，五个宿主一起生效；以后真要单独的选项，可以在它下面复用同一个构造函数。
- **按路径 `/v2/translate` 识别 DeepL 协议**：能覆盖自建的 DeepL 反向代理。但 DeepLX 自己就有 `/v2/translate`，现有用户的请求格式会被悄悄改掉。只认官方主机名，代理用户继续走 DeepLX 格式。
- **不写 `source_lang`，交给 DeepL 自动识别**：#6829 的建议，也省掉中文变体的问题（`source_lang` 本来就不收变体）。对整句可行，对两三个字的候选词不可靠，见上。
- **iOS 也照写一份 DeepL 主机判断**：Swift 判断「DeepL 地址且没有密钥」最直接，但又多一份会分叉的副本。iOS 设置页和键盘都链接共享库，直接问描述器能否构造，与 macOS 保存前的检查是同一个判断。
- **各宿主各写一份 DeepL 判断**：每个宿主都已经有自己的请求代码。但地址判断、主机选择、语言映射一旦分叉，同一个密钥会在一个平台能用、另一个平台 403。能共享的地方都调用 Rust，Python 那一份用共享用例锁住。

## Consequences

收益：填 DeepL 官方地址和自己的 Auth Key 就能用，Free 密钥填错主机也能用；DeepLX 用户的请求一个字节都不变。请求格式只有一处权威实现，鸿蒙设置页少了一份手写的请求和解析。

Android 键盘后来接上了自定义服务的运行时，同样走这个共享构造，见 [Android 候选翻译接入用户自己的翻译服务](2026-10-12-android-user-translation-services.md)。

代价：DeepL 和 DeepLX 共用一个选项，用户要知道填什么地址，靠设置页文案说明；DeepL 地址必须填密钥：macOS 原生窗口在填好密钥前提示无法保存，iOS 显示「凭据不完整」，共享设置页在 API Key 下提示并禁用连接测试。共享设置页的 DeepL 主机判断是 TypeScript 的一份副本，靠共享用例锁住。仍是一词一请求，一页九个候选要九次往返。Linux 的连接测试分不出 403 和 456。

## Verification

- `cargo test -p msime-client-core --lib translation`：共享用例、DeepL 主机识别（仿冒主机名、子域名、非默认端口、http 都不算）、DeepL v2 回复解析、错误正文不算译文、连接测试发出的鉴权和正文、空密钥不发请求、403/456/429 的提示。
- `cargo test -p msime-host-api --lib custom_translation`：描述器的 URL、请求头、正文，空密钥报错，最长密钥的头长度为 4111，DeepL 回复与错误正文的解析。
- Linux：在 `msime-linux-build-gate` 容器（Python 3.11）里跑 `custom_translation_contract.py`、`custom_translation_config.py`、`credential_test_contract.py`、`translation_provider_selection.py`、`translation_cache_parity.py`。
- `apps/desktop` 的 vitest（翻译设置，含按共享用例核对 `isDeepLApiEndpoint`、`customTranslationKeyIssue` 和 DeepL 空密钥提示的渲染；鸿蒙表情页的服务选项）、`tsc --noEmit`；鸿蒙 `hvigorw assembleHap` 编译 ArkTS。
- `python3 scripts/test-harmony-custom-translation-probe.py`；把鸿蒙 429 文案或探测请求的源语言改掉后它会失败。
- `bash platforms/harmony/tests/run.sh` 中的 `custom-translation-probe.test.ts`：探测请求、共享用例上的 DeepL 识别、请求头、回复门槛和 DeepL 状态码文案；把 456 文案或 `DeepL-Auth-Key ` 前缀改掉后对应的组会失败。
- iOS 的 `TranslationProviderTests` 新增了 DeepL 空密钥、DeepLX 无密钥、远程 http 的路由用例，键盘扩展与 `MSIMEKeyboardTests` 的 Swift 源码用 `swiftc -typecheck`（iOS 模拟器 SDK）通过；本机磁盘不足，没有构建模拟器原生库，XCTest 没有运行。
- 没有真实的 DeepL 密钥，没有对 DeepL 服务端发过请求；请求格式按 DeepL 的公开文档写成，Free 与 Pro 主机、`ZH-HANS`/`EN-US`、`DeepL-Auth-Key` 头都没有被真实服务确认过。macOS 的 ctest、iOS、Windows 运行时和设备上的候选释义都没有跑。

共享回复判定不论有无成功码都要求支持的结果结构，合法空回答仍负缓存，见[自定义翻译回复结构边界](../bug-fix/2026-10-15-custom-translation-reply-shape.md)。

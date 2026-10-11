# Agent Note: Windows 原生设置窗口自己选翻译服务、填凭据

Status: implemented

## Problem

原生设置窗口「标点与翻译」页「候选词翻译」组的最后一行是「翻译服务 / 打开」，用 `shell_links::expression` 拉起共享 Tauri 应用的「标点与翻译」页，服务的选择和凭据只能在那里改。整项工作的目标是任何设置都在原生窗口里改。共享页在 Windows 上画的是：「翻译服务」下拉（关闭、腾讯云机器翻译、小牛翻译、自定义 DeepLX 兼容服务、水杉账号），选中腾讯云、小牛或自定义服务时多一组它的凭据，每组带一个「测试配置」按钮。测试在 Tauri 里是 `test_api_credential` 命令，Windows 和 macOS 分支里内联了按服务分派到 `credential::{asr,doubao,translation,probe}` 的代码，C ABI 没有对应的入口。自定义接口地址的提示 `translationEndpointIssue` 用浏览器的 `new URL` 解析，C++ 里没有同一套 WHATWG 网址解析。

## Decision

- client-core 新增 `credential::test_service(service, config)`，就是 Tauri 原来内联的分派；Tauri 的 Windows/macOS 分支改为调用它。host-api 新增 `msime_client_test_api_credential`（`{service, config}` → `{ok, message}`，会联网，须在工作线程调用），`msime_client.h` 写明请求、上限和不得记录日志。
- client-core 新增 `translation::endpoint_issue`，逐条照搬 `translationEndpointIssue`，用 `url` crate（与浏览器同一套 WHATWG 规则）解析；host-api 新增纯函数 `msime_client_translation_endpoint_issue`，同时返回偏好库是否接受（空或 `is_supported_endpoint`），因为两者不完全重合，例如 `https:///api` 在网址规则下主机是 `api`、没有提示，偏好库却不收。
- 其余规则写成只有头文件的 `TranslationServiceSettings.h`：选中哪种服务（`use-translation-settings.ts` 在 Windows 上的分支，腾讯云默认开着但没有可用凭据时不盖过水杉账号）、`tencentCredentialIssue`、「未填写腾讯云凭据」警告的条件、三个测试按钮何时可用，以及每个凭据字段偏好库会不会接受（`preferences.rs` 的 `validate`）。由 `tests/ui/translation_service_settings.cpp` 核对，加进 CMake 测试。
- 页面上那一行换成「翻译服务」下拉，候选词翻译关着时不可选；选一种只打开它、关掉另外两种，选水杉账号写 `translation_account`，否则从文档里去掉这一项，与 `setTranslationProvider` 相同，然后重画。下面按所选服务画「腾讯云机器翻译」「小牛翻译」「自定义服务」三组之一，文案、占位符、读屏名、提示和警告照搬共享页；密钥框是 `PasswordBox` 加「显示」「隐藏」按钮，对应 `SecretInput`。
- 输入框每输入一个字：更新本组的提示和测试按钮（原处更新，不重建页面，输入框不丢焦点），偏好库接受的值写进文档、350 ms 后保存，不接受的值不写入、输入框标红，与候选字体输入框相同。共享页把不合法的草稿也拿去保存，失败后只显示一句笼统的错误；原生窗口的保存被拒会重读设置并重画，打到一半的内容会丢，所以先在本地拦下。
- 「测试配置」照 `CredentialTestSection`：按服务记结果和测试时的配置，配置改了就收起结果，测试中显示「测试中…」，同一服务一次只测一个；调用本身失败时显示共享页的「无法连接 provider，请确认服务已启动。」。结果跨页面重建保留，测试结束时就地更新按钮和结果。
- 删掉 `shell_links::expression`，`all` 从 6 项减为 5 项，导航测试去掉对它的核对；README 和 `SettingsNavigation.h` 的说明同步。与 [Windows 候选释义分栏与水杉账号释义](2026-10-10-windows-gloss-columns-and-account-glosses.md) 部分重叠：那篇让 Windows 认得水杉账号并给原来的「打开」行加了说明，这一篇把那一行换成了下拉。

## Alternatives considered

- **在 C++ 里自己解析接口地址** — 不用新增导出；但 WHATWG 网址规则（省略斜杠、IPv4 简写、大小写、空片段）很细，手写一份迟早和共享页的提示不一致，偏好库那边本来就用 `url` crate。
- **Rust 侧给整组翻译设置出一个「状态」导出（选中的服务、所有提示、按钮状态）** — C++ 更薄；但这些规则在共享页是 TypeScript，搬到 Rust 等于再造第三份，而且每输入一个字都要过一次 C ABI。只把需要网址解析的那一条放进 Rust。
- **只导出翻译三种服务的测试** — 范围更小；但 Tauri 的分派本来就是一段，拆开后 Tauri 还得留一份；语音和 AI 页以后变原生时也要用同一个入口。
- **不合法的值也写进文档** — 与共享页的草稿一致；但会触发保存被拒、重读、重画，输入中的文字丢失。

## Consequences

- 原生窗口不再为翻译服务拉起共享应用。
- `msime_client_test_api_credential` 也接受 `voice.asr`、`voice.polish` 和 `ai.assistant`，原生窗口的三种翻译服务和语音页（见 [原生语音页](2026-10-11-windows-settings-voice-page.md)）都用它，`ai.assistant` 还没有原生入口。
- 共享页的提示、警告文案或测试配置改了，要同步 `TranslationServiceSettings.h` 和 `main.cpp`；`translationEndpointIssue` 改了要同步 `translation::endpoint_issue`。
- 被拦下的值只标红、不写入；NiuTrans 的示例占位值之类共享页没有专门提示的情况，原生窗口也只有红框，没有文字说明。

## Verification

- `cargo test --locked -p msime-client-core --lib endpoint_issue`、`--lib test_service`、`--lib credential`，`cargo test --locked -p msime-host-api --lib _export_`（含两个新导出的测试，只用不联网就会回答的配置）通过；`cargo build --release --locked -p msime-host-api` 和 `cargo check --locked -p msime-desktop` 通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- `tests/ui/translation_service_settings.cpp` 和 `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口，也没有对真实服务发过测试请求：下拉、输入框、标红、显示/隐藏和测试按钮的实际交互都未在运行中的窗口里检查。

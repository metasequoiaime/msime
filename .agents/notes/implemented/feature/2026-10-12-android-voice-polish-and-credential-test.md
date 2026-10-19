# Agent Note: Android 原生设置页的润色配置与「测试连接」

Status: implemented

## Problem

Android 键盘按共享解析的 `mobile_voice_polish_configuration` 在识别后润色，读的是 `voice_input.polish_*`。但正式包的原生「语音输入」页只配得了识别服务（见[识别服务那篇](2026-10-12-android-voice-provider-settings.md)），润色的开关、服务、密钥、模型和方案一个都没有，只有不发布的 Tauri 开发合包能改。

两个服务也都没有「测试」：填错密钥要等到真按住空格说话才知道。共享层早有测试实现（`credential::test_service`，C ABI 是 `msime_client_test_api_credential`），但它要调用方先把接口和模型解析好再传进去。共享 React 页靠 TS 里的默认值表做这一步，原生页面没有这张表。

## Decision

- 新增 C ABI `msime_client_test_voice_credential({preferences_directory, service})`，`service` 取 `voice.asr` 或 `voice.polish`。共享层从存着的偏好解析出测试配置，再交给 `credential::test_service`，返回 `{ok, message}`。系统识别、本地识别和不认识的服务不联网，直接回答「当前服务不需要测试。」。Android 经 JNI `testVoiceCredentialRaw` 调用它，`verify-native.sh` 的符号清单也加上了。
- 解析的规则在 `voice::provider::credential_test_config`，与键盘用的是同一套：空的接口和模型补该服务的默认值，空的密钥取该服务的分槽，豆包带上鉴权方式、App ID（只在旧版鉴权时）、资源 ID 和三个开关。所以「测试」测的就是键盘会发的那一份。默认值表因此从两个解析函数里提出来，成为 `asr_provider_defaults` 和 `polish_provider_defaults`。
- `polish_provider_defaults` 里 SiliconFlow 和 DeepSeek 的默认值改成与共享设置页的 `POLISH_PROVIDER_DEFAULTS` 一致：SiliconFlow 用 `Qwen/Qwen3-8B`（原来是 `Qwen/Qwen2.5-7B-Instruct`），DeepSeek 用 `https://api.deepseek.com/chat/completions` 加 `deepseek-v4-flash`（原来是 `/v1/chat/completions` 加 `deepseek-chat`）。这两个值只在 `polish_model` 和 `polish_endpoint` 留空时才用到；Android 页面切换服务时会清空这两项，所以它们就是 Android 用户实际发出的值。桌面切换服务时写进偏好的、Android 出厂偏好里存的，也都是新值。
- `VoicePage`：
  - 加「润色」组。开关同时写 `polish_text` 和 `polish_enabled`，和共享页一样，键盘任一个开着就润色。打开后有润色服务（经 `VoiceProviderCatalog` 从共享 Rust 的 `POLISH_PROVIDERS` 读取，名称与顺序直接用于选择器；目录不可用时禁用服务选择）、API Key、模型、接口地址、润色方案（四个内置方案加三个自定义槽位），选自定义槽位时可以编辑它的内容。
  - 切换润色服务的规则同识别服务：通过 `VoiceProviderSettings` 调共享 `msime_client_voice_provider_update`，显式采用 `clear` 字段策略，凭据换槽并清空接口和模型。
  - 开关打开、共享解析却没给出润色配置时，开关下面写明识别结果不会被润色。
  - 识别服务（联网服务）和润色组各有一行「测试连接」，在工作线程上调用。结果用对话框显示：共享层的结果文案可能超过 Toast 的两行。
- 润色方案、提示词字段与状态文案仍在 `voice/VoicePolishSettingsPolicy.java`，配 JVM 冒烟；服务目录与名称在 `VoiceProviderCatalog` 中持有共享快照，不再保留 Java 版 `PROVIDERS` 和标签数组。新的行标题加进 `PageId.VOICE` 的搜索关键词。

## Alternatives considered

- **在 Java 里放一份默认接口和模型表，直接调用 `msime_client_test_api_credential`**：不用新增 C ABI。没有采用：这样默认值就有了第三份副本（Rust 解析、TS 页面、Java 页面）。EveryAPI 默认模型那次（[那篇](../bug-fix/2026-10-12-everyapi-asr-default-model.md)）就是副本之间各写各的出的事；而且测试的配置会和键盘实际用的那份分开，测试通过了键盘照样可能失败。
- **新 C ABI 接收完整配置，在 Rust 里补默认值**：调用方可以测还没保存的输入。没有采用：原生页面的每一项都是确认即保存，没有「未保存的表单」这个状态；直接读偏好目录，调用方连字段都不用拼。
- **测试结果用 Toast 显示**：更轻。没有采用：共享层的结果文案（例如「请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。」）在 Android 12 起的两行 Toast 里会被截断，正是 [#5553 那次](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)踩过的坑。

## Consequences

- 收益：正式包用户可以配置润色，可以当场知道识别和润色的密钥对不对；测试和键盘读同一份解析，结论可信。
- 代价：多一个 C ABI。润色的两个默认值改了，`polish_model` 留空、服务是 SiliconFlow 或 DeepSeek 的移动端用户会换到新模型。
- 仍缺：EveryAPI 在 `polish_provider_defaults` 里的默认模型 `openai/gpt-4o-mini` 不在 EveryAPI 的模型目录里，但 EveryAPI 不在 `POLISH_PROVIDERS` 里，哪个设置页都选不到它，这次没有改。
- 验证：`cargo test -p msime-client-core -- voice::provider`（含 `credential_test_config_resolves_what_the_keyboard_would_send`）；`cargo test -p msime-host-api -- credential_test`（含 `voice_credential_test_resolves_preferences_and_answers_offline`）；`cargo clippy` 两个库；`platforms/android/check-host.sh`（含 `VoicePolishSettingsPolicySmoke`、`SettingsSearchIndexSmoke`）；`gradlew compileFullReleaseJavaWithJavac`；`client_jni.cpp` 用 NDK clang 做 `-fsyntax-only -Werror` 编译。没有在真机上用真实密钥测试过。

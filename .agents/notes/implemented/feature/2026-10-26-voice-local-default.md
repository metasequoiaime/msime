# Agent Note: 六端出厂识别服务改为本地模型（离线）

Status: implemented

## Problem

共享偏好 `VoiceInputPreferences::default()` 的 `asr_provider` 是 `doubao`，不带任何凭据。桌面三端的新用户按下语音快捷键时落到需要 API Key 的豆包，什么都识别不了；Android 与 HarmonyOS 则把「豆包没填密钥」读作没有配置，悄悄改用系统识别。iOS 已经单独改成未配置时默认本地（[iOS 本地识别默认值](2026-10-18-ios-local-voice-default.md)），其余五端还停在豆包。维护者要求出厂默认改成「本地模型（离线）」，六端全改。

## Decision

出厂识别服务是 `local`：音频不离开设备、不需要 API Key，下载或导入模型后即可使用。只改出厂值，已保存的选择（包括旧版本写下的 `doubao`）原样保留，不做迁移。

- **权威值**：`crates/client-core/src/preferences.rs` 的 `VoiceInputPreferences::default()`。Rust 每次都完整写出 `asr_provider`，校验拒绝空值，所以读共享文档的宿主（macOS、Windows、Linux、Android、HarmonyOS 键盘）新装时拿到的都是 `local`。`asr_endpoint` 与 `asr_resource_id` 仍保留豆包的出厂值：本地模型不读它们，用户日后切到豆包时不必另补，HarmonyOS 的 `DoubaoCredentialPolicy` 也把它们当豆包的回退地址。
- **镜像**：共享 React 的 `defaultVoiceInput`、HarmonyOS 键盘的 `DEFAULT_VOICE_INPUT_CONFIGURATION` 同步改成 `local`。
- **原生「未设置」回退**：各宿主在字段缺失时回退的服务一并从 `doubao` 改成 `local`，让「未设置」只有一个含义——macOS 的 `InputController.mm` 两处、回退设置窗 `VoiceProviderSettings.mm`（含非法值回退），Windows 设置页 `main.cpp` 四处、Server 的 `server_main.cpp` 和 `VoiceInputConfig` 成员默认值，Linux IBus/Fcitx5 的三处（只影响边说边显示，`doubao` 与 `local` 结论相同）。
- **macOS 录音前检查**：`MSIMEVoiceASRTokenMissing` 不再把未设置的服务当豆包要求填 Token；`MSIMEVoiceLocalModelMissing` 把未设置的服务当本地模型，没有模型时拒绝录音并指向语音设置。
- **不改的地方**：`shared/voice` 的 `normalize_voice_provider` 仍把空值规整成 `doubao`。它是云端请求层的规整（接口地址、请求格式、语言参数），调用方此时已经决定走云端；宿主的回退都已改成 `local`，空值到不了这里。Windows 安装包里的 `config.default.toml` 是来源产品的模板，运行时不读，也不改。
- **macOS 语音运行库只在选用模型后下载**：省略运行库的 macOS 包，设置应用启动时按已保存偏好补齐运行库（`ensure_saved_packs` → `needed_by_preferences`），原来只看服务是不是 `local`。出厂改成本地后，这会让每个新用户第一次打开设置应用就在后台下载约 16 MB，哪怕从不用语音，违背[按需运行库](2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)的初衷。现在启动补齐还要求 `asr_model_path` 非空；没有模型时运行库用不上。设置页原来只在「切到本地」那一刻 `ensure`，出厂就是本地的新用户没有这一步，所以「本地识别下刚选用模型」也触发 `ensure`，用户主动切到本地时照旧立即下载。
- **Android / HarmonyOS 的取舍**：明确选了本地而没有模型时，Android 按[本地识别准备](../bug-fix/2026-10-21-android-local-voice-readiness.md)提示下载或导入，不回退系统识别（系统识别器可能走云端）；HarmonyOS 同样走本地模型缺失的提示。新用户因此要先准备模型才能用语音键，这是维护者确认过的代价。

## Existing notes audit

- [iOS 本地识别默认值](2026-10-18-ios-local-voice-default.md)：部分重叠。它当时把「同时修改共享 Rust 语音默认值」列为超出范围的备选，本篇做了这一步；iOS 仍用自己的 UserDefaults，缺模型时改用系统识别的行为不变。
- [鸿蒙豆包缺凭据](../bug-fix/2026-10-12-harmony-doubao-missing-credential.md)、[Android 键区内系统识别](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)、[Android 语音服务设置](2026-10-12-android-voice-provider-settings.md)：部分重叠。它们「豆包没填密钥不报错、交给系统识别」的理由建立在出厂值是 `doubao` 上；对按旧默认装好的用户仍然成立，规则不变，各篇已补一句指向本篇。
- [本地语音准确率](2026-10-17-local-voice-accuracy.md)：无关。那里的「默认」指模型目录里的默认模型，本篇只改默认服务。

## Alternatives considered

- **只改桌面三端，移动端保持豆包回退系统识别**：桌面端默认豆包本来就不能用，改本地是纯收益；移动端现在开箱就能经系统识别说话，改了之后要先下载模型。维护者选择六端统一，理由是离线、不上传音频是产品要给的默认，移动端的系统识别（Android 上常是云端）不满足这一点。
- **全改且移动端缺模型时回退系统识别（与 iOS 一致）**：保住移动端开箱即用。没有采用：需要推翻 Android「明确选了本地就不悄悄换识别器」的既有决定，维护者没有选这条。
- **把从没配置过的老用户（`doubao` 且没有任何豆包凭据）也迁移到 `local`**：老用户同样受益。没有采用：这是改写已存的值，文档里的 `doubao` 分辨不出是默认留下的还是用户选的（见 Android 键区那篇的同一判断），需要单独授权。
- **只改 Rust 出厂值，不动原生回退**：改动最小，字段缺失在实际文档里几乎不出现。没有采用：同一个「未设置」在 Rust 和宿主里含义不同，正是此前各宿主默认漂移的来源，macOS 在共享偏好下发前的那段时间还会按豆包要求填 Token。

## Consequences

- **收益**：桌面新用户第一次打开语音设置就看到本地模型，下载后即可离线使用，不再卡在「未填写 API Key」；六端出厂值一致，音频默认不离开设备。
- **代价**：macOS 新用户选用模型后运行库才开始下载，模型装好后还要等它下完（约 16 MB）才能录音。Android、HarmonyOS 新用户在下载或导入模型前语音键不可用，此前它们开箱即可经系统识别使用。模型和运行库按需下载，体积与首次准备时间由用户承担。
- **不变**：已保存的选择、豆包的出厂接口与资源 ID、云端请求层对空值的规整。

## Verification

- `cargo test -p msime-client-core -p msime-client-providers -p msime-host-api -p msime-tauri-mobile-platform`：1893 项通过；`cargo test -p msime-desktop -p msime-client-services` 全部通过，`local_recognition_needs_the_macos_voice_runtime` 覆盖出厂本地不下载运行库、选用模型后下载，依赖豆包出厂值的 `ios_voice_doubao_configuration_uses_shared_auth_and_current_preferences` 改为显式选豆包；`cargo clippy -p msime-desktop --all-targets -- -D warnings` 无告警。新增 `voice_provider_defaults_to_the_offline_local_model`，覆盖出厂值、出厂文档通过校验、旧文档的 `doubao` 原样保留。
- 前端：`voice-input-preferences.test.ts` 钉住 React 出厂值；`voice-provider.test.tsx` 按真实宿主注入本地模型客户端，要求出厂选中的「本地模型」是下拉里可用的一项；`settings.test.tsx` 里依赖豆包专属控件（鉴权方式、数字格式化）的用例改为显式选豆包；`resource-packs.test.tsx` 新增出厂本地下选用模型时下载运行库的用例（改动前失败）。全量 `vitest run` 5488 项通过，`tsc --noEmit`、lint、格式检查通过。
- HarmonyOS `platforms/harmony/tests/run.sh`：0 失败，`keyboard-logic.test.ts` 钉住键盘出厂值，豆包未配置的用例改为描述旧默认。
- macOS `VoiceProviderOptionsTest.mm` 以 CMake 同样的 `-Wall -Wextra -Werror -UNDEBUG` 编译运行通过：未设置的服务没有模型时拒绝录音、不要求 Token。
- 未在 Windows、Linux、Android、HarmonyOS 真机或模拟器上装包验收新装首次录音的提示。

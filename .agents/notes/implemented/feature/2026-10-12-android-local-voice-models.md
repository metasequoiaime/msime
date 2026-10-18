# Agent Note: Android 原生设置页可以下载、选用和删除本地语音模型

Status: implemented

## Problem

Android 键盘的本机识别早就接好了：识别服务是 `local`，或打开「离线识别」且没网时，都按 `voice_input.asr_model_path` 加载 `filesDir` 下装好的模型目录（`LocalAsrPolicy.installed`）。但正式包的原生设置页没有任何地方能装模型，[本地文件导入那篇](2026-10-09-local-voice-model-import.md)写明了「Android 原生设置页本来就没有模型下载、删除入口」。[识别服务那篇](2026-10-12-android-voice-provider-settings.md)补上了云端服务，本地模型因此仍不在可选列表里。

结果是：「离线识别」开关打开了也用不上，键盘的失败提示（「本地语音模型未安装……请在设置中重新下载」「可改用本地模型或豆包」）指向一个不存在的入口。只有不发布的 Tauri 开发合包能装模型。

## Decision

- JNI 新增四个函数，直接对应 C ABI：`voiceLocalModelsRaw`、`voiceLocalModelInstallRaw`（进度以 JSON 字节交给 `LocalModelProgress.onProgress(byte[])`，写法同资源包的安装）、`voiceLocalModelCancelRaw`、`voiceLocalModelRemoveRaw`。`verify-native.sh` 的符号清单也加上这四个。
- `core/LocalVoiceModels.java` 封装列出、安装、取消、删除，模型根目录是 `filesDir/voice-models`，与 HarmonyOS 和 Tauri 合包相同。目录里标了 `desktop_only` 的模型不列出，已经装上的照样列出，以便删除，规则同 HarmonyOS。
- 下载不另起服务，复用 `ResourcePackService`：模型在这里的键是 `voice-model:<id>`。状态、进度通知、流量确认、从通知取消、Android 15 前台服务超时、结果通知都与资源包相同，只有安装和取消调用的是模型的共享接口。镜像沿用 `voice_input.asr_model_mirror`。
- 下载完成后，如果 `asr_model_path` 还空着，就选用刚装好的模型（`LocalVoiceModels.adoptIfUnset`），和桌面设置页一致。这一步写在服务里而不是页面里，下载途中用户离开设置页也照样生效。
- `VoicePage`：
  - 「识别服务」多了「本地模型」。选它时会备好运行库；如果已有装好的模型而还没选用，就选用第一个。
  - 选了本地模型或打开离线识别时，出现「本地模型」组：运行库状态行加上每个模型一行（下载、取消、校验中、重试，已装的点开可以选用或删除）。删除正在用的模型时，一并清空 `asr_model_path`。
  - 选了本地模型却还不能用时，「识别服务」行说明缺什么（`VoiceProviderPolicy.localStatus`）。这里不说「暂用系统识别」，因为本地识别不会回退到系统识别。
- Android 的「离线识别」开关默认打开，首次使用仍需用户在「本地模型」组下载并选用模型；默认识别服务和模型目录不改，已有用户明确保存的开关值也不改。
- 行状态的纯规则放在 `voice/LocalVoiceModelPolicy.java`，配 JVM 冒烟。这个类不引用 `ResourcePackService`：后者导入了 androidx，进不了 `check-host` 的编译。

## Alternatives considered

- **另写一个 `LocalVoiceModelService`**：模型和资源包互不干扰，各自的代码更好读。没有采用：前台服务类型、通知渠道、超时处理、流量确认、取消竞态（`CANCELLING` 在共享层登记之前的补发）都要再写一遍，而这些正是资源包那边踩过坑才写对的部分。两个服务同时跑还会出两条进度通知、各自去抢前台服务的时长配额。
- **把共享 React 设置页的 `LocalModelManager` 嵌进 WebView**：模型列表、导入、许可证展示一次到位。没有采用：原生应用的其余页面都是原生的，单独嵌一个 WebView 页要接上 Tauri 命令桥，范围远超「能下载和选用」。
- **这一期同时做从文件导入**：网络不通的用户也能装上模型。没有采用：Android 的文件选择器给的是 content URI，原生库打不开，要像 HarmonyOS 那样先复制进缓存，并处理复制期间的取消。瘦包的 `voice-runtime` 也还不能从文件导入，单补模型导入仍然做不到完全离线。留作下一期。

## Consequences

- 收益：正式包用户可以装上本地模型，选「本地模型」做识别服务，或使用默认打开的离线识别作为没网时的后备；键盘失败提示里说的「在设置中下载」现在有对应的入口。
- 代价：`ResourcePackService` 不再只管资源包，`runningPacks` 改成遍历状态表，结果通知让所有模型共用一个位置（同一时间通常只下一个模型）。
- 仍缺：模型许可证的展示；下载镜像的设置入口。从文件导入模型和运行库后来由[不联网安装那篇](2026-10-12-android-offline-voice-import.md)补上。
- 验证：`platforms/android/check-host.sh`（含 `LocalVoiceModelPolicySmoke`、`VoiceProviderPolicySmoke`、`SettingsSearchIndexSmoke`）；`gradlew compileFullReleaseJavaWithJavac`；`client_jni.cpp` 用 NDK 28.2 的 clang 在 `-Wall -Wextra -Werror` 下做 `-fsyntax-only` 编译。没有做完整的 native 链接，也没有在真机上下载模型、做一次本地识别。

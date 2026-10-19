# Agent Note: Android 原生设置页可以选识别服务、填密钥

Status: implemented

## Problem

Android 键盘的语音入口早已按共享解析 `msime_client_mobile_voice_configuration` 选识别方式：本机模型、豆包流式、OpenAI 兼容上传和百炼都接好了。但正式包的设置是原生的 `VoicePage`，只有识别语言、自动标点、离线识别、启动方式和上传语音，没有任何一处能改 `voice_input.asr_provider`、填密钥或换接口。能配置这些的只有 Tauri 开发合包里的共享 React 设置页，而那个包不发布。

结果是正式包用户只能用设备自带的系统识别。#5553 那类系统识别本身失败的设备上，[键区内系统识别那次修复](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)给的出路提示是「可改用本地模型或豆包」，可用户在设置里找不到改的地方。共享默认的 `asr_provider` 是 `doubao`、不带密钥，这一项也从来没有对用户显示过。

## Decision

- `VoicePage` 最上面加「识别服务」组。联网服务的 id、名称和顺序来自 `client-providers::voice::provider_catalog::ASR_PROVIDERS`，经 `msime_client_voice_provider_catalog` 和 JNI `voiceProviderCatalogRaw` 读取；`VoiceProviderCatalog` 在前面追加平台特有的 `system` 和 `local`。Java 不保存联网服务目录。
- 选中联网服务后，组里列出它要的字段：
  - 豆包：鉴权方式（`doubao_auth_mode`）、旧版鉴权才有的 App ID（`asr_app_key`）、API Key 或 Access Token、识别接口（整句流式、双向流式或自定义地址，写进 `asr_endpoint`）、资源 ID（`asr_resource_id`）。
  - 其他服务：API Key、模型（`asr_model`）、接口地址（`asr_endpoint`）。
- 页面只写共享偏好里 `voice_input` 下的这些字段，键盘读的还是同一份共享解析，键盘侧代码不动。
- 切换服务时 `asr_endpoint` 和 `asr_model` 清空，由共享解析补上该服务的默认值（`mobile_voice_provider_configuration` 对空值本来就这样处理）。所以 Java 里不另放一份默认接口和模型表。
- 凭据沿用共享设置页 `swapTokenSlot` 的分槽语义：`asr_token` 是当前服务的密钥，`asr_tokens[id]` 存其他服务的；切走时把当前密钥收进旧服务的槽，切回来时取出。系统识别不留槽。保存密钥时同时删掉当前服务的槽，留空就是彻底删除这个服务的密钥。保存时如果服务已被别处换掉，整次不写。
- 「识别服务」那一行说明键盘眼下实际在用什么：选了联网服务、共享解析却没给出配置时，显示「未填写 API Key，暂用系统识别」或「密钥未填写完整，暂用系统识别」。出厂的豆包不带密钥，用户因此看得到自己其实在用系统识别。
- 凭据换槽、字段校验和状态文案仍在 `voice/VoiceProviderPolicy.java`，配 JVM 冒烟 `tests/voice/VoiceProviderPolicySmoke.java`。服务、接口名称和地址由不可变的 `VoiceProviderCatalog` 快照持有，页面在工作线程重载时读取一次；失败时保留当前配置，显示服务目录不可用并禁用服务与预设接口选择，不向用户提供第二份默认目录。搜索关键词仍在 `PageId.VOICE`，不作为可选服务来源。
- 豆包两种流式接口的 id、短名称、选择器标题和地址来自同一目录的 `doubao_stream_endpoints`；共享运行时的默认地址也引用这份预设。空的 `asr_endpoint` 按 ASR 目录中豆包的默认接口显示和选中，其余未命中的地址仍显示为自定义。现有 C ABI 只扩展响应，不新增安卓专用的 C 接口。
- 本地模型的下载、选用和删除由[本地语音模型那篇](2026-10-12-android-local-voice-models.md)提供，选项在系统识别之后。目录不访问网络、凭据、文件或设备；JNI 复用现有 UTF-8 字节数组传输和 Rust 字符串释放方式。

## Alternatives considered

- **保留 Java 静态服务表作为读取失败时的兜底**：旧界面在 JNI 不可用时仍可展示所有服务，交互不中断。没有采用：目录是随二进制内置的纯数据，读取失败意味着本机桥接或协议不完整；第二份默认表会重新形成漂移。失败快照只禁用目录选择，既有配置与其他语音设置保留，重新进入页面会再读。

- **切换服务时保留自定义地址（与共享设置页的 `fillProviderDefaults` 一致）**：理由是两边行为相同，填了代理地址的用户换服务后不用重填。没有采用：那个地址是为上一个服务填的，留着它，新服务的密钥就会发到旧服务的主机上；而且判断「是不是预设」需要 Java 里有一份全部预设地址表，这正是要避免的第二份副本。代价是换服务后自定义地址要重填。
- **新增共享 Rust 函数 `select_asr_provider` 并开 JNI**：换服务的规则只有一份，看起来最干净。没有采用：TS、ArkTS、Swift 已经各有一份，只给 Android 加一个 Rust 版并不能收敛成一份，反而要多一个 C ABI。共享解析对空值补默认，已经把最容易分叉的部分（默认接口和模型）留在 Rust 里了，Java 只剩换槽这几行。
- **把共享 React 设置页嵌进原生应用的 WebView**：所有字段一次到位。没有采用：原生应用其余页面都是原生的，单独嵌一个 WebView 页要接上 Tauri 命令桥，工作量和风险都远大于补几行原生设置。

## Consequences

- 收益：正式包用户可以改用豆包或任一上传式服务，系统识别失败时提示里的出路真的走得通。密钥只存在本机，设置同步只带 `voice_input.language`。
- 代价：换槽规则在 Java 里多了一份，和 TS 的 `swapTokenSlot` 靠冒烟里的对照用例保持一致。换服务时自定义地址会被清掉。
- 仍缺：润色服务的配置和凭据测试按钮后来由[润色与测试那篇](2026-10-12-android-voice-polish-and-credential-test.md)补上；本地模型的下载与选用由[本地语音模型那篇](2026-10-12-android-local-voice-models.md)补上，「本地模型」也因此进了可选列表。
- 目录验证：Rust C ABI 测试核对完整公开目录、流式选项与运行时默认地址；`VoiceProviderCatalogSmoke` 用合成服务名和地址核对快照、未知 id、默认选中与不可变性，`VoiceProviderCatalogDeviceSmoke` 在设备上读取真实 JNI，并检查严格解析和合成名称变更。`check-host.sh` 通过 248 项 JVM 冒烟并编译 189 项设备套件源；原生正式宿主的瘦包 APK 使用 Java 21 构建通过。在 API 28（`emulator-5566`）和 API 35（`emulator-5580`）安装该 APK 后，真实 JNI 目录套件通过，原生「语音输入 → 服务 → 识别服务」选择器能显示系统识别、本地模型与共享云服务；没有在真机上用真实密钥走过一遍识别。

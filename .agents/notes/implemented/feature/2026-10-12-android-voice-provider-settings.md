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
- `VoiceProviderSettings` 将最新偏好中的当前服务、地址、模型、凭据与字符串槽位映射为 `msime_client_voice_provider_update` 的请求，识别和润色共用这一适配层。共享 `provider_settings::provider_update` 计算换槽，并按显式 `field_policy: clear` 在服务真的变化时清空接口和模型，由运行时共享解析补上默认值；同一服务不改文档。该接口不读写偏好、不联网，宿主仍通过原有的修订号保存路径提交。
- 凭据沿用共享设置页 `swapTokenSlot` 的分槽语义：`asr_token` 是当前服务的密钥，`asr_tokens[id]` 存其他服务的；切走时把当前密钥收进旧服务的槽，切回来时取出。系统识别不留槽。保存密钥时同时删掉当前服务的槽，留空就是彻底删除这个服务的密钥。保存时如果服务已被别处换掉，整次不写。
- 「识别服务」那一行说明键盘眼下实际在用什么：选了联网服务、共享解析却没给出配置时，显示「未填写 API Key，暂用系统识别」或「密钥未填写完整，暂用系统识别」。出厂的豆包不带密钥，用户因此看得到自己其实在用系统识别。
- 凭据校验仍由 `voice/VoiceProviderPolicy.java` 调共享预检，状态文案也经 `msime_client_mobile_voice_status` 下沉到 `client-providers::voice::provider`；Java 只传入解析结果、系统识别能力和本地模型运行时事实，并严格解析状态信封。配套 JVM 冒烟保留凭据与桥接检查，设备冒烟覆盖状态文案；Java 不再实现凭据换槽或状态规则。`VoiceProviderSettings` 完整验证成功信封、目标服务、字符串凭据与槽位、已清空的接口和模型后才修改内存文档；JNI 不可用、超限或响应无效时抛出固定错误，原保存流程停止，不写下一半补丁，也不记录响应内容。服务、接口名称和地址由不可变的 `VoiceProviderCatalog` 快照持有，页面在工作线程重载时读取一次；失败时保留当前配置，显示服务目录不可用并禁用服务与预设接口选择，不向用户提供第二份默认目录。搜索关键词仍在 `PageId.VOICE`，不作为可选服务来源。
- 豆包两种流式接口的 id、短名称、选择器标题和地址来自同一目录的 `doubao_stream_endpoints`；共享运行时的默认地址也引用这份预设。空的 `asr_endpoint` 按 ASR 目录中豆包的默认接口显示和选中，其余未命中的地址仍显示为自定义。状态文案另经无文件、无网络的 `msime_client_mobile_voice_status` 暴露共享规则，参数只携带宿主能力和本地模型运行时事实，不携带凭据或偏好目录。
- 接口地址提示由 `provider_settings::endpoint_issue` 经 `msime_client_voice_endpoint_issue` 和 JNI 提供：留空使用默认地址，非空先沿用 Android 传输要求的小写 `https://` / `wss://` 前缀、无 `@`，再调用运行时的 `valid_mobile_voice_endpoint` 检查 2048 字节上限与 URL 形状。因此非法端口、损坏 IPv6 主机和带空格的主机在设置页就被拒绝。Java 只选择传输类型、传 UTF-8 并严格解析 `{accepted,issue}`，不再实现地址规则；无效信封、矛盾字段、JNI 不可用时返回固定提示，保存按钮保持禁用。传输边界在 Java、JNI 和 C ABI 限为 64 KiB，拒绝非法 UTF-16 / UTF-8，不回显输入、不读写偏好、不联网。
- 本地模型的下载、选用和删除由[本地语音模型那篇](2026-10-12-android-local-voice-models.md)提供，选项在系统识别之后。目录不访问网络、凭据、文件或设备；JNI 复用现有 UTF-8 字节数组传输和 Rust 字符串释放方式。

## Alternatives considered

- **保留 Java 静态服务表作为读取失败时的兜底**：旧界面在 JNI 不可用时仍可展示所有服务，交互不中断。没有采用：目录是随二进制内置的纯数据，读取失败意味着本机桥接或协议不完整；第二份默认表会重新形成漂移。失败快照只禁用目录选择，既有配置与其他语音设置保留，重新进入页面会再读。

- **切换服务时保留自定义地址（与共享设置页的 `fillProviderDefaults` 一致）**：理由是两边行为相同，填了代理地址的用户换服务后不用重填。没有采用：那个地址是为上一个服务填的，留着它，新服务的密钥就会发到旧服务的主机上；而且判断「是不是预设」需要 Java 里有一份全部预设地址表，这正是要避免的第二份副本。代价是换服务后自定义地址要重填。
- **另建 Android 专用的 `select_asr_provider` 与 C ABI**：可以直接表达 Android 的清空行为，接线简单。没有采用：现有 `msime_client_voice_provider_update` 已被 Windows 使用，能够复用凭据换槽并以显式字段策略保留平台差异；再建一份会重复服务切换规则。缺省 `fill_defaults` 继续替换出厂值并保留自定义字段，Android 显式选择 `clear`；不把 Windows 的保留策略直接套给 Android。
- **保留 Java 的地址预检**：不需要 JNI，JVM 测试能直接运行，输入时少一次桥接。没有采用：前缀和字符检查无法发现坏端口与损坏 IPv6，页面会允许保存运行时拒绝的地址。复用共享运行时解析后，Java 只适配参数和响应；原生传输仍保留自己的防御性检查。设置预检保留移动宿主已有的大小写与 `@` 限制，不放行传输层目前不能消费的地址。
- **把共享 React 设置页嵌进原生应用的 WebView**：所有字段一次到位。没有采用：原生应用其余页面都是原生的，单独嵌一个 WebView 页要接上 Tauri 命令桥，工作量和风险都远大于补几行原生设置。

## Consequences

- 收益：正式包用户可以改用豆包或任一上传式服务，系统识别失败时提示里的出路真的走得通。密钥只存在本机，设置同步只带 `voice_input.language`。
- 代价：Android 要经一次纯计算 JNI 取得补丁。请求按 UTF-8 字节在 Java、JNI 和 C ABI 限为 1 MiB；请求与响应都含凭据，不能记录。换服务时自定义地址仍会被清掉；原生页只保留本地模型首次选用与下载这些平台行为。
- 仍缺：润色服务的配置和凭据测试按钮后来由[润色与测试那篇](2026-10-12-android-voice-polish-and-credential-test.md)补上；本地模型的下载与选用由[本地语音模型那篇](2026-10-12-android-local-voice-models.md)补上，「本地模型」也因此进了可选列表。
- 状态接口验证：共享 Rust 单测覆盖系统识别、云端回退、本地模型与运行库优先级；Host API 测试覆盖 JNI 所需的 JSON 信封；Java 21 构建原生瘦包后，在 API 28（`emulator-5566`）与 API 35（`emulator-5580`）运行 `VoiceProviderSettingsDeviceSmoke` 均通过，覆盖真实 JNI 状态文案、空服务、本地就绪和严格信封解析。状态接口不读取偏好、不发送请求，也不保存测试凭据。目录验证：Rust C ABI 测试核对完整公开目录、流式选项与运行时默认地址；`VoiceProviderCatalogSmoke` 用合成服务名和地址核对快照、未知 id、默认选中与不可变性，`VoiceProviderCatalogDeviceSmoke` 在设备上读取真实 JNI，并检查严格解析和合成名称变更。`check-host.sh` 通过 248 项 JVM 冒烟并编译 189 项设备套件源；原生正式宿主的瘦包 APK 使用 Java 21 构建通过。在 API 28（`emulator-5566`）和 API 35（`emulator-5580`）安装该 APK 后，真实 JNI 目录套件通过，原生「语音输入 → 服务 → 识别服务」选择器能显示系统识别、本地模型与共享云服务；没有在真机上用真实密钥走过一遍识别。

- 换服务验证：共享层 11 项字段补丁测试与 3 项 C ABI 测试通过，包含既有默认策略、清空策略和严格解析；`check-host.sh` 通过 248 项 JVM 冒烟并编译 192 项设备套件源。Java 21 构建原生正式宿主 APK 后，在 API 28（`emulator-5566`）与 API 35（`emulator-5580`）运行 `VoiceProviderSettingsDeviceSmoke` 均通过，用合成内存文档经真实 JNI 验证识别/润色换槽、离线服务、空凭据、中文与补充平面字符、同服务不写、响应失败不部分改写和 UTF-8 请求边界，不保存或发送测试凭据。两台模拟器的原生识别服务选择、重进页面读取和恢复原选择均通过；没有在真机上使用真实密钥识别。
- 接口预检验证：4 项共享 Rust 测试、1 项 C ABI 测试与 clippy 通过；`check-host.sh` 通过 248 项 JVM 冒烟并编译 193 项设备套件源。旧 APK 在 API 35 的坏端口用例失败；Java 21 构建新原生宿主 APK 后，API 28 与 35 的 `VoiceEndpointSettingsDeviceSmoke` 均通过，覆盖真实 JNI、UTF-8 边界、非法 Unicode 和严格信封解析，导出符号核验通过。两台模拟器的原生编辑框中，坏端口禁用保存，有效地址与空值启用保存；取消编辑而未保存地址。测试只使用合成输入、不保存地址、不联网。

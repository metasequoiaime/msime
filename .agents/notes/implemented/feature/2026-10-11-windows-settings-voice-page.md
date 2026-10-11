# Agent Note: Windows 原生设置窗口自己画语音识别服务与润色

Status: implemented

## Problem

原生设置窗口的「语音输入」页只有总开关、结果提交策略、四个录音开关和五个快捷键，其余设置是一行「识别服务与润色 / 设置」，用 `shell_links::voice` 拉起共享 Tauri 应用的语音页。整项工作的目标是任何设置都在原生窗口里改。共享页（`voice-settings-content.tsx` 的桌面分支）在 Windows 上还有：按服务变化的页首说明或打开语音面板的一行；识别服务（七个云服务、Windows 系统识别、本地模型）和识别语言；预置模型与接入说明、模型、豆包的鉴权方式和流式接口、地址、凭据、资源 ID 和识别选项；本地模型管理器（列出、下载带进度、取消、从文件导入、使用、删除，下载镜像和手填目录）；两个「测试配置」；流式预编辑；文本润色的服务、模型、地址、令牌、方案和提示词；录音后端和麦克风。上一步已经把列录音设备、测试配置和本地模型需要的 C 接口准备好（见 [原生设置窗口语音页用到的 C 接口](../architecture/2026-10-11-windows-settings-voice-host-api.md)）。

还差两件事：模型放在哪里要知道本版本 Tauri 应用的 identifier，C++ 里只有 Inno Setup 的定义有它；下载镜像和模型目录是偏好库会拒收的字段（`valid_model_mirror` 要按 URL 解析），原生窗口边输入边保存，一个没输完的镜像地址会让整份偏好保存失败、重读、丢掉输入。

## Decision

- 页面照共享页的顺序重排：页首一组（本地识别、Windows 系统语音的说明，或「语音面板」打开语音面板的一行）、「识别」、「识别服务配置」（系统识别时不画）、语音快捷键、录音行为、「识别结果」（流式预编辑和原来在第一组的结果提交策略）、「文本润色」、「录音设备」。文案、读屏名、显示条件按 Windows 宿主的能力取（`voice_provider_settings`、`voice_stream_preedit`、`voice_commit_mode`、`voice_capture_devices` 都为真，`localVoiceModels` 带导入，`pickVoiceModelPath` 存在）。共享页不因总开关关着而禁用快捷键和录音开关，原生页也去掉了这层禁用。
- 规则写成只有头文件的 `VoiceSettings.h`：服务表和各服务的默认地址、模型、已知模型、文档（`voice-providers.ts`、`voice-provider-options.tsx`），`asrProviderUpdate`、`polishProviderUpdate`（令牌按服务存取，地址和模型还是默认值或空时换成新服务的），豆包流式接口，令牌标签，两个测试何时可用，识别语言建议，四个内置润色提示词（逐字照搬，另用脚本与 `polish-presets.ts` 比对过），录音后端选项，以及 `local-model-helpers.ts` 的大小、语言、进度、阶段和错误文案。由 `tests/ui/voice_settings.cpp` 核对，加进 CMake 测试。
- 换服务时把整张 `asr_tokens`、`polish_tokens` 写回，与共享页更新草稿的方式相同；选系统识别时把「自动识别」换成 `zh-CN`。换到或换走本地模型时，模型列表的状态清空、进行中请求的结果作废，对应共享页模型管理器的卸载和重新挂载。
- host-api 新增纯函数 `msime_client_voice_model_setting_check`：`{asr_model_mirror?, asr_model_path?}` → 每个字段偏好库收不收。判断就是 `Preferences::validate` 用的那两条，模型目录那条提成 `preferences::valid_model_path`，`validate` 改为调用它。镜像和目录输入框只写入它认可的值，不认可时标红。
- 版本表生成器给 `msime_edition.h` 加上 `MSIME_EDITION_TAURI_IDENTIFIER`，原生页的模型根目录是 `%APPDATA%\<identifier>\voice-models`，各版本与自己的 Tauri 应用管同一批模型。
- 安装在工作线程上调 `msime_client_voice_local_model_install`，进度回调在工作线程上解析，百分比或阶段变了才送回 UI 线程，就地更新进度条，不重画页面。下载用文档里的镜像（输入框只写认可的值，等于已保存的值），与 Tauri 读已保存镜像一致。错误串按 Tauri 的 `local_model_error_code` 归并后取文案；`local_model_install_running` 归为 `busy`。窗口关闭时取消所有安装。
- 文字输入框沿用翻译服务页的 `credential_input`：每输入一个字写进文档、停下后保存，不重建页面；预置模型、流式接口和可用录音设备的选中项，以及两个测试按钮，跟着输入就地更新。下拉框和开关的改动保存后重画整页。识别语言用 `AutoSuggestBox` 代替 datalist，最多 64 个字符；润色提示词的 `TextBox` 把 `\r` 换回 `\n` 再保存。
- 「识别服务与润色」一行和 `shell_links::voice` 删掉，新增 `shell_links::voice_panel`（`voice` 面板路由，托盘的「语音输入」也用它），`all` 仍是 6 项；导航测试核对它的路由参数。README 和 `SettingsNavigation.h` 的说明同步。

## Alternatives considered

- **在 C++ 里照 `validModelMirror` 自己判断镜像** — 不用新增导出；但共享页用浏览器的 `URL`、偏好库用 `url` crate，第三份手写解析早晚和偏好库对不上，对不上的结果就是保存被拒、输入丢失。
- **镜像不认可时照样写入** — 与共享页的草稿一样；但原生窗口没有草稿，保存被拒会重读设置，正在输入的内容就没了。
- **模型根目录写死 `app.msime.windows`** — 上一篇笔记原来这样写；但其他版本的 Tauri 应用用自己的 identifier，非 full 版本的原生页会去管 full 的模型目录。
- **安装进度每次都重画页面** — 写法简单；但下载一个上百 MB 的模型会触发成百上千次重画，用户正在输入的框也会丢焦点。

## Consequences

- 原生窗口不再为语音设置拉起共享应用；语音面板本身仍由 `MSIME.exe` 显示。
- 共享页的服务表、默认值、润色提示词、模型文案或测试条件改了，要同步 `VoiceSettings.h` 和 `main.cpp`。
- 原生窗口和 Tauri 应用各在自己的进程里登记安装，互不知道；两边同时装同一个模型会怎样没有验证过。
- 共享页的本地模型管理器在页面隐藏时仍挂载，原生页的状态则保存在窗口上，效果相同：离开页面再回来时进度和说明还在。

## Verification

- `cargo test --locked -p msime-host-api --lib voice_model_setting_check` 通过；`cargo test --locked -p msime-client-core --lib preferences::tests` 通过（121 项）。`--lib local_model` 里 `voice::local_models::tests` 有 11 项失败，撤掉本改动后同样失败，是这台机器检出的模型目录资源与校验值不符，与本改动无关。
- `cargo build --release --locked -p msime-host-api` 通过；Tauri 没有改动。
- `python platforms/windows/scripts/edition_windows.py gen` 重新生成 `msime_edition.h`，`scripts/test-windows-editions.py`、`scripts/test-editions.py`、`scripts/test-windows-state-dir-parity.py` 通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）；`tests/ui/voice_settings.cpp`、`tests/ui/settings_navigation.cpp`、`tests/ui/translation_service_settings.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口，没有下载模型、读录音设备或对真实服务发过测试请求：各组的排版、进度条、文件对话框、建议列表和测试按钮的实际交互都未在运行中的窗口里检查。

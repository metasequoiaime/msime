# Agent Note: Windows 原生设置窗口自己画语音识别服务与润色

Status: implemented

## Problem

原生设置窗口的「语音输入」页只有总开关、结果提交策略、四个录音开关和五个快捷键，其余设置是一行「识别服务与润色 / 设置」，用 `shell_links::voice` 拉起共享 Tauri 应用的语音页。整项工作的目标是任何设置都在原生窗口里改。共享页（`voice-settings-content.tsx` 的桌面分支）在 Windows 上还有：按服务变化的页首说明或打开语音面板的一行；识别服务（七个云服务、Windows 系统识别、本地模型）和识别语言；预置模型与接入说明、模型、豆包的鉴权方式和流式接口、地址、凭据、资源 ID 和识别选项；本地模型管理器（列出、下载带进度、取消、从文件导入、使用、删除，下载镜像和手填目录）；两个「测试配置」；流式预编辑；文本润色的服务、模型、地址、令牌、方案和提示词；录音后端和麦克风。上一步已经把列录音设备、测试配置和本地模型需要的 C 接口准备好（见 [原生设置窗口语音页用到的 C 接口](../architecture/2026-10-11-windows-settings-voice-host-api.md)）。

还差两件事：模型放在哪里要知道本版本 Tauri 应用的 identifier，C++ 里只有 Inno Setup 的定义有它；下载镜像和模型目录是偏好库会拒收的字段（`valid_model_mirror` 要按 URL 解析），原生窗口边输入边保存，一个没输完的镜像地址会让整份偏好保存失败、重读、丢掉输入。

## Decision

- 页面照共享页的顺序重排：页首一组（本地识别、Windows 系统语音的说明，或「语音面板」打开语音面板的一行）、「识别」、「识别服务配置」（系统识别时不画）、语音快捷键、录音行为、「识别结果」（流式预编辑和原来在第一组的结果提交策略）、「文本润色」、「录音设备」。文案、读屏名、显示条件按 Windows 宿主的能力取（`voice_provider_settings`、`voice_stream_preedit`、`voice_commit_mode`、`voice_capture_devices` 都为真，`localVoiceModels` 带导入，`pickVoiceModelPath` 存在）。共享页不因总开关关着而禁用快捷键和录音开关，原生页也去掉了这层禁用。
- 服务目录统一在 `client-providers::voice::provider_catalog`，识别与润色的运行时默认值也查询该目录；原生页通过纯函数 `msime_client_voice_provider_catalog` 取得按既有顺序排列的服务、地址、默认与建议模型、文档和名称，复制进持有字符串的快照后释放 JSON。Windows 在识别目录末尾追加系统识别与本地模型，空地址和模型保留原先的清空规则。换服务的字段补丁由 `client-providers::voice::provider_settings::provider_update` 统一计算，经 `msime_client_voice_provider_update` 提供给原生页：令牌按服务分槽保存和恢复，空令牌删除旧槽，地址和模型为空或任一服务的出厂默认值时替换，否则补丁省略该字段以保留原值。系统识别和本地识别清空默认地址、默认模型和自身凭据槽。豆包流式接口也从同一 JSON 的 `doubao_stream_endpoints` 读取，按目录的顺序、标识、标题和地址绘制及保存；空地址依据共享识别目录中豆包的默认地址匹配选项，不固定为某个标识或下标。接口快照持有字符串，选择「自定义地址」和手填地址仍保留原有临时状态，绘制与同步不改写偏好。按住录音的组合键、名称与缺键默认值引用[共享按住录音目录](../simplification/2026-10-25-voice-hold-key-catalog.md)的静态投影；WinUI 与 Windows 7 共用该投影。其余原生展示规则写成只有头文件的 `VoiceSettings.h`：豆包流式接口匹配，令牌标签，两个测试何时可用，识别语言建议，四个内置润色提示词（名称和正文引用[共享 Rust 目录的宽字符串静态导出](../simplification/2026-10-21-shared-polish-prompt-catalog.md)），录音后端选项，以及 `local-model-helpers.ts` 的大小、语言、进度、阶段和错误文案。由 `tests/ui/voice_settings.cpp` 核对，加进 CMake 测试。 `field_policy` 缺省为 `fill_defaults`，Windows 沿用该行为；Android 的识别与润色页经同一接口显式请求 `clear`，仅换服务时清空地址与模型，凭据换槽实现共用。
- 换服务接口仅接收当前服务、地址、模型、令牌及对应分槽，不传整个偏好文档。请求最多 1 MiB，标准 JSON 响应里的 `value` 是 `{provider,token,tokens,endpoint?,model?}`，省略与空串分别表示保留和清空。接口不做文件或网络访问，不记录请求、返回或解析失败详情；Windows 解析失败时只提示固定错误，不写设置。换服务时把整张 `asr_tokens`、`polish_tokens` 写回，与共享页更新草稿的方式相同；选系统识别时把「自动识别」换成 `zh-CN`。换到或换走本地模型时，模型列表的状态清空、进行中请求的结果作废，对应共享页模型管理器的卸载和重新挂载。
- host-api 新增纯函数 `msime_client_voice_model_setting_check`：`{asr_model_mirror?, asr_model_path?}` → 每个字段偏好库收不收。判断就是 `Preferences::validate` 用的那两条，模型目录那条提成 `preferences::valid_model_path`，`validate` 改为调用它。镜像和目录输入框只写入它认可的值，不认可时标红。
- 版本表生成器给 `msime_edition.h` 加上 `MSIME_EDITION_TAURI_IDENTIFIER`，原生页的模型根目录是 `%APPDATA%\<identifier>\voice-models`，各版本与自己的 Tauri 应用管同一批模型。
- 安装在工作线程上调 `msime_client_voice_local_model_install`，进度回调在工作线程上解析，百分比或阶段变了才送回 UI 线程，就地更新进度条，不重画页面。下载用文档里的镜像（输入框只写认可的值，等于已保存的值），与 Tauri 读已保存镜像一致。错误串按 Tauri 的 `local_model_error_code` 归并后取文案；`local_model_install_running` 归为 `busy`。窗口关闭时取消所有安装。
- 文字输入框沿用翻译服务页的 `credential_input`：每输入一个字写进文档、停下后保存，不重建页面；预置模型、流式接口和可用录音设备的选中项，以及两个测试按钮，跟着输入就地更新。下拉框和开关的改动保存后重画整页。识别语言用 `AutoSuggestBox` 代替 datalist，最多 64 个字符；润色提示词的 `TextBox` 把 `\r` 换回 `\n` 再保存。
- 「识别服务与润色」一行和 `shell_links::voice` 删掉，新增 `shell_links::voice_panel`（`voice` 面板路由，托盘的「语音输入」也用它），`all` 仍是 6 项；导航测试核对它的路由参数。README 和 `SettingsNavigation.h` 的说明同步。

## Alternatives considered

- **从 TypeScript 或 C++ 表生成 Rust 默认值** — 能复用现有展示元数据，改动集中；但共享业务层会依赖宿主工程的表和生成步骤，因此由已有默认值归属的 `client-providers` 拥有公开目录，宿主仅解析持有。
- **继续在 `VoiceSettings.h` 保留换服务算法** — 纯 C++ 测试容易运行，也不用 JSON 往返；但服务目录已归 Rust，宿主再维护默认值替换与凭据分槽会让规则继续漂移。因此原生页只转换字段和应用补丁，算法与用例归共享 Rust。
- **只在 C 接口拼一份目录 JSON** — 不用动运行时查找；但默认值仍有两份，设置页与真正发送请求的语音配置仍可能漂移，因此两者查询同一目录。

- **在 C++ 里照 `validModelMirror` 自己判断镜像** — 不用新增导出；但共享页用浏览器的 `URL`、偏好库用 `url` crate，第三份手写解析早晚和偏好库对不上，对不上的结果就是保存被拒、输入丢失。
- **镜像不认可时照样写入** — 与共享页的草稿一样；但原生窗口没有草稿，保存被拒会重读设置，正在输入的内容就没了。
- **模型根目录写死 `app.msime.windows`** — 上一篇笔记原来这样写；但其他版本的 Tauri 应用用自己的 identifier，非 full 版本的原生页会去管 full 的模型目录。
- **安装进度每次都重画页面** — 写法简单；但下载一个上百 MB 的模型会触发成百上千次重画，用户正在输入的框也会丢焦点。

## Consequences

- 原生窗口不再为语音设置拉起共享应用；语音面板本身仍由 `MSIME.exe` 显示。
- Windows 不再维护独立服务目录；修改共享 Rust 目录同时影响原生设置和语音运行时。目录不访问偏好、文件、网络或设备，原生页首次使用时同步读一次；JSON 字段由快照持有，控件只在快照存活期间使用字符串视图。后端兼容的 EveryAPI、Mistral 润色仍可解析旧配置，但不新增到原生页原有的四个选项。
- `packages/ui` 和 Android 的目录及换服务规则迁移仍待后续切片；共享页的模型文案或测试条件改了，仍要同步 `VoiceSettings.h` 和 `main.cpp`。
- 共享页的语音页已改成主页面加「服务配置」子视图，按住录音的三个开关合成一个选择、流式接口和接口地址合成一个控件（见[精简语音输入设置页](../simplification/2026-10-16-macos-voice-settings-prune.md)）。本页随后跟进了这些合并和组的先后，起初没有子视图控件，仍是一整页（那篇的「各平台跟进」）；现在已分成两个页内标签加「服务配置」子视图，见 [Windows 设置的语音页标签与服务配置子视图](2026-10-20-windows-settings-voice-ai-mac-parity.md)。
- 原生窗口和 Tauri 应用各在自己的进程里登记安装，互不知道；两边同时装同一个模型会怎样没有验证过。
- 共享页的本地模型管理器在页面隐藏时仍挂载，原生页的状态则保存在窗口上，效果相同：离开页面再回来时进度和说明还在。

## Verification

- `cargo test --locked -p msime-host-api --lib voice_model_setting_check` 通过；`cargo test --locked -p msime-client-core --lib preferences::tests` 通过（121 项）。`--lib local_model` 里 `voice::local_models::tests` 有 11 项失败，撤掉本改动后同样失败，是这台机器检出的模型目录资源与校验值不符，与本改动无关。
- `cargo build --release --locked -p msime-host-api` 通过；Tauri 没有改动。
- `python platforms/windows/scripts/edition_windows.py gen` 重新生成 `msime_edition.h`，`scripts/test-windows-editions.py`、`scripts/test-editions.py`、`scripts/test-windows-state-dir-parity.py` 通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）；`tests/ui/voice_settings.cpp`、`tests/ui/settings_navigation.cpp`、`tests/ui/translation_service_settings.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口，没有下载模型、读录音设备或对真实服务发过测试请求：各组的排版、进度条、文件对话框、建议列表和测试按钮的实际交互都未在运行中的窗口里检查。

### 共享目录验证

- 目录测试核对原有的七个识别与四个润色服务的顺序、标识唯一、名称与文档完整、建议模型含默认模型，以及每项与运行时解析的默认值相同；系统/本地/未知识别仍不解析为联网配置，旧的 EveryAPI、Mistral 润色默认值保留。
- C ABI 测试读取并释放真实导出，核对标准信封和全部目录字段；原生纯规则测试用注入目录验证选项与展示回退；令牌分槽、自定义值保留、系统/本地识别的行为由共享 Rust 换服务测试与真实 C ABI 导出测试核对。
- 本机相关 Rust 测试共 14 项、两个 crate 的严格 clippy、原生纯规则编译运行、格式与笔记校验均通过；quick 通过，包含 Linux 设置外壳编译、原生宿主 90 项测试、macOS 原生编译和共享 Apple bridge 编译。WinUI 3 设置工程由 PR 的 Windows CI 编译。没有在 Windows 运行中的窗口里验收本片。

### 共享换服务规则验证

- `voice_provider_settings` 集成测试覆盖服务切换、离线识别与切回、默认字段替换、自定义值省略、令牌槽删除/恢复、原快照不变及 JavaScript 空白字符语义。
- `voice_provider_update_export` 用真实 C ABI 检查识别与润色补丁、省略字段、无效 JSON/UTF-8/服务/类型、空指针和超过 1 MiB 的缓冲区；失败只返回固定错误，不回显凭据。
- 原生纯规则测试继续覆盖展示；WinUI 3 完整编译和原生设置页验收由 PR 的 Windows CI 覆盖，本机不以 C++ 纯规则测试声称已验收运行中的 Windows 窗口。

### 共享豆包流式接口验证

- `tests/ui/voice_settings.cpp` 注入三项合成接口，核对重排后的标识匹配、JavaScript 空白字符、改变默认地址、未知与空目录，以及快照副本持有字符串。新增用例在旧接口上编译失败，实现后以 `c++ -std=c++20 -Wall -Wextra -Werror` 编译运行通过。
- 下拉选项、选中后保存和偏好同步均读取同一份共享快照；显式选择自定义和手填地址的状态处理保持原有逻辑。本机没有 Windows 运行环境，尚未验收运行中的 WinUI 窗口。
- 本片 `bash scripts/verify-local.sh --quick` 通过，包含 176 项 Windows 可移植测试、仓库契约检查、Rust workspace 与共享 Apple bridge 编译。MinGW 与运行中的 Windows 宿主未在本机验证，完整设置工程由 PR CI 构建。

语音请求正文与本篇 Windows 设置页的展示正文、名称和顺序均由[共享 Rust 润色目录](../simplification/2026-10-21-shared-polish-prompt-catalog.md)静态导出；自定义槽位、选中规则与页面接线仍归原生页。

# Windows 功能迁移对照

这份文档记录 Windows 宿主对来源产品 MSIME-Windows 的迁移结果：迁移的范围与基线、一直盯着这件事的几道门禁、每个功能组落在本仓库哪里、哪些地方刻意与来源不同及其理由，以及本仓库特有的 Windows 进程与协议边界。

迁移已完成。功能对照不是一次读完 README 得出的结论，而是由五道检查持续地问同样几个问题：来源能被设定的每一项、来源界面能发起的每个动作、来源发布日志里的每一条、来源源码树里的每个文件，在这边分别由什么答复。下面「持续门禁」一节给出这几个问题今天的答案。

## 迁移范围与固定基线

目标是迁移 MSIME-Windows 的完整功能，而不是语音、设置页或某台机器上跑得起来的子集。分工固定为：公共业务逻辑放共享 Rust 层（`crates/client-core`、`crates/input-runtime`、`crates/host-api`），公共管理界面放 Tauri 壳与共享 React 设置页（`apps/desktop/`、`packages/ui/`），输入算法与组合状态仍归 C++ Engine，Windows 侧保留 TSF DLL 与 Server 两个进程及其协议边界。

对照使用的是固定对象，不是某个仓库当天的 HEAD：

- 来源：`metasequoiaime/MSIME-Windows` 的提交 `345cb87a3822f6ad7013bb29506fe3d856c1931a`。所有 reference 门禁统一经 `scripts/reference_source.py` 读取这个对象（`PINNED_SHA`），不跟随相邻检出的当前分支或可变远端 tip；环境变量 `MSIME_REFERENCE_DIR` 只覆盖检出位置，不能覆盖版本。检出存在但缺这个对象时，脚本判定为无效的验证环境并指出恢复命令，而不是回落到一个可变分支。
- 目标：`metasequoiaime/msime` 的 `develop`。
- Engine：来源把 C++ Engine 以 `engine/` 在自己树内维护；本仓库的输入引擎是移植成 Rust 的 `crates/engine`，不再取回或锁定任何 Engine 源码归档，也没有 overlay 这一层。下文各批次里提到的 `engine-lock.json`、`scripts/fetch_engine.py`、`vendor/MSIME-Engine`、`scripts/apply_engine_*.py` 与 `crates/engine-bridge`，是移植之前的做法，作为当时的记录保留；那些 overlay 承接的行为现在都直接写在 `crates/engine` 的对应模块里。两个 Engine 不因目录名或协议名相同而视为内容相同。

来源的功能入口以该提交的 `README.md`「功能简介」「核心功能指南」、`ui-html/webview2/settings/ime-settings/src/modules/sidebar.ts`、`server/src/settings/settings_app.cpp`、`installer/default_config/config.default.toml` 和 `engine/contracts/webview/messages.json` 交叉核对。README 只是入口索引，真正的判据是后面几道逐字段、逐动作、逐文件的检查。

## 持续门禁

按名字对界面、按记忆对功能，这两种做法在这次迁移里反复产生同样两类假结果：看着缺的其实是有意改名（`y_mode` 就是 `local_modes.temporary_english`，`cn_en_mixed_input_min_chars` 就是 `mixed_input.minimum_prefix`），看着有的其实只是某个无关标识符里恰好含同一个词。所以对照的结论写成可执行的脚本，由 `scripts/verify-local.sh` 每次运行。

| 问的是什么 | 谁在问 | 答案 |
| --- | --- | --- |
| 来源能被设定的每一项，本仓有没有 | `scripts/test-windows-config-keys.py` | 来源出厂配置的 180 个键（18 个段）全部有对应物 |
| 每一项设定对应共享层的哪个字段，那个字段今天还在不在 | `scripts/test-reference-config-coverage.py` | 180 项全部有着落，其中 8 项以 `!kind: why` 写明为什么没有字段 |
| 来源界面能对宿主发起的每个动作 | `scripts/test-reference-ui-actions.py` | 46 个动作全部有人答 |
| 来源发布日志里的每一条 `feat:` | `scripts/test-reference-feature-log.py` | 19 条全部过过一遍；锁定对象之前的 93 个 `feat:` 提交（91 个不同 subject）也逐条有答案，其中 4 个记为已知差距 |
| 来源源码树里的每个文件 | `scripts/test-reference-source-inventory.py` | 274 个 `.cpp`/`.h`：156 个同名、106 个改名并指向存在的路径、12 个写明不需要 |

这几道检查各自的判据：

- 配置键那道检查比的是**键名**而不是值。两边刻意不同的默认值由 `scripts/test-default-config-parity.py` 单独比对——它读安装模板 TOML 与 Rust 源码两份互相独立的材料，改错一边会指名报错，而不像单测那样用实现断言实现。本仓多出来的键不报告：适配可以提供更多，不能更少。
- 覆盖率那道检查的目标是从 `Preferences` 起的完整字段路径（如 `floating_toolbar.scale_percent`），逐段解析 Rust 结构体，并要求每一段都以属性访问或对象键的形式**在共享设置页上被用到**；只在注释或局部变量里出现同一个词不算。只在 preferences crate 里存在算「纸面支持」：字段在、能往返、没有用户改得动它。主体不存在于本平台的条目记在 `PLATFORM_LOCAL` 里。
- 界面动作那道检查不能按名字比：来源的四个界面（设置、悬浮工具栏、候选窗、托盘菜单）是 WebView2 文档，向宿主 post message，而这边其中三个是原生代码、根本没有消息。所以每个动作映射到一个在本仓库仍可检索到的 token，或记为有理由的缺席。
- 源码清单那道检查的第二种形式指向一个必须存在的路径而不是一句话；第三种形式（`DELIBERATELY_ABSENT`）是要带着怀疑读的部分——迁移正是在这里藏它没做的事——所以每条都写明用户得到的是什么替代物。
- 来源检出是可选的。没有检出时这五道检查各自报告它需要什么然后通过，与其他依赖外部材料的阶段一致。

### Windows 侧的编译与运行门禁

除上面五道对照检查外，与 Windows 直接相关的还有：

- `scripts/test-windows-32bit-compile.py`：32 位 TSF DLL 会被加载进每个 32 位宿主，所以同一份 C++ 必须两个架构都编得过。编译参数取自 x64 构建产出的 `compile_commands.json` 而非另一份手工清单，往 CMake 加源文件或 include 自动被覆盖；只换编译器并 `-fsyntax-only`，因此不需要 32 位库。
- `scripts/test-windows-path-encoding.py`：禁止 Windows 会编译到的 C++ 出现 `path::string()`。这个转换在 Windows 上按 ANSI 代码页走，`C:\Users\陆傲天` 这类 profile 要么转错要么抛；而在系统编码为 UTF-8 的机器上一点痕迹都没有，只能静态拦。
- `scripts/test-windows-native-run.py`：`platforms/windows/tests/` 里大部分是纯策略——对契约结构体的纯函数，整个翻译单元没有 Win32 调用。哪些属于这一类是发现出来的而非列出来的（能用宿主编译器独立编译链接的就是），这些源文件因此在任何机器上都真的被执行，而不只是链接通过。
- `scripts/test-installer-prerequisites.py`：钉住安装器对 WebView2 与 VC 运行库的注册表判据（不做文件探测：Setup.exe 是 32 位进程，`FileExists` 会被 WOW64 重定向）。
- `scripts/test-preferences-field-parity.py`：按名字配对 TS 类型与 Rust 结构体，两个方向都查字段集；Rust 独有的须在 `RUST_ONLY` 写明理由。
- 交叉构建：`platforms/windows/build-cross.sh x64`（MinGW + vcpkg，校验 vcpkg HEAD 与清单基线一致后装 `<arch>-mingw-static` 依赖，再依次构建 `msime-host-api`、`MetasequoiaImeDictionaryReplay` 与整个 CMake 工程），本机工具链不满足 DWARF 展开时走 `build-cross-container.sh`。`verify-local.sh` 在非 Windows 宿主上自动接这条路径，并用目录锁让多个 worktree 串行。
- pipe-only 配置（`-DMSIME_WINDOWS_PIPE_ONLY=ON`）在 x86_64 与 i686 两个架构上各配置构建一次：`windows_ipc.h` 的 `static_assert` 钉的是帧大小与字段偏移，两个位数都要成立。
- 运行：`platforms/windows/run-tests-wine.sh x64` 在 `xvfb-run -a wine` 下运行交叉产物（C++ 套件与 `cargo test --no-run` 产出的 Rust 套件），每个套件 120 秒超时，失败集合与 `scripts/known-failures.txt` 比对，只对不在清单里的名字失败。
- MSVC 全量构建与打包：`platforms/windows/Build-Client.ps1`（x64 出 Server/Watchdog/prepare/TSF，x86 出 TSF 与 Host DLL；完成前读五个 x64 EXE 与两对 TSF/Host DLL 的 PE 头做架构与类型门禁），安装包走 `platforms/windows/installer/Package-SimplySign.ps1`。
- CI：`.github/workflows/ci-platforms.yml` 的 Windows 作业跑在 `debian:trixie-slim` 容器里（环境与 `platforms/windows/cross/Dockerfile` 一致；Ubuntu 24.04 的 MinGW 头文件没有 `msimeui` SVG 渲染要用的 `d2d1_3.h`），执行 `build-cross.sh x64`。同一工作流的 `windows-scripts` 作业在 `windows-2025` 上用 pwsh 跑 `tests/tools/` 下发布脚本的探针测试（`portable_executable`、`runtime_dependencies`、`collect_notices`、`build_client`），不编译任何东西；这些测试随测试目录重组失修过一段时间（相对路径少了一层），此后由这个作业看住。`.github/workflows/release-windows.yml` 手动触发，在 `windows-2025` 上用 MSVC 构建并编出未签名的 Inno Setup 安装包，签名仍是发布机上的本地步骤。

### 原生测试套件

`platforms/windows/CMakeLists.txt` 注册 100 个 ctest，`platforms/windows/tsf/CMakeLists.txt` 注册 19 个，`platforms/windows/msimeui/tests/` 是一个聚合套件，`platforms/windows/tests/native-pipe/` 另有两个 Windows-only 管道用例（`windows-pipe-io`、`windows-aux-listener`）。源文件按职责分在 `tests/{candidate,clipboard,core,input,runtime,ui,voice}` 下。按领域看：

- 协议与会话：`windows-server`、`windows-session`、`windows-reply-codec`、`windows-reply-composer`、`windows-input-queue`、`windows-registration-inbox`、`windows-aux-message`、`windows-runner-control`。
- 焦点与按键：`windows-focus-gate`、`windows-focus-router`、`windows-main-frame`、`windows-tsf-focus-lease-protocol`、`windows-tsf-key-dispatch`、`windows-input-key-policy`、`windows-key-event-send-result`、`windows-terminal-deactivation-policy`、`windows-mode-authority`、`windows-dedicated-english` 与 `-controller`。
- 候选与外观：`windows-candidate-card-size`、`-shadow`、`-wheel`、`-menu`、`-menu-layout`、`-initialization`、`-palette`、`-skin`、`-appearance`、`-render-sync`、`-font-format`、`-completion-policy`、`-text-policy`、`-ui-action-policy`、`-action-availability`、`-translation-merge`，以及五个热重载用例（`windows-candidate-skin-reload`、`-theme-reload`、`-layout-reload`、`-font-reload`、`windows-floating-toolbar-reload`）。
- 工具栏与托盘：`windows-toolbar-layout`、`-icons`、`-click`、`-coordinates`、`-mode-command`、`windows-floating-toolbar-placement`、`-visibility`、`windows-tray-menu-layout`、`-dispatch`。
- 联网候选与翻译：`windows-cloud-candidate-worker`、`windows-ai-candidate-worker`、`windows-translation-worker`、`windows-translation-display`、`windows-provider-token`。
- 语音：`windows-voice-controller-protocol`、`-connection`、`-listener`、`-dispatch`、`windows-voice-control-message`、`windows-voice-session-epoch`、`windows-voice-hotkey-policy`、`windows-voice-session-policy`、`windows-doubao-transcript`、`windows-voice-capture-selection`、`windows-voice-providers`、`windows-voice-theme`、`windows-voice-review-result`、`windows-wave-overlay-scale`、`windows-polish-prompt`。
- 剪贴板：`windows-clipboard-text`、`windows-clipboard-monitor`（后者实际注册 `AddClipboardFormatListener`、验证重复 `start` 幂等与重复 `stop` 不崩溃，不改写系统剪贴板、不记录用户内容）。
- 配置、启动与守护：`windows-shared-config-keybindings`、`windows-tsf-config-frames`、`windows-preview-config`、`windows-shell-surfaces`、`windows-server-launch`、`windows-installer-launch`、`windows-first-run`、`windows-prepare-host`、`windows-state-directory`、`windows-watchdog-policy`、`windows-telemetry-consent`、`windows-maintenance-hotkeys`、`windows-diagnostic-log`、`windows-diagnostic-batch`、`windows-typing-statistics`。
- 输入策略：`windows-punctuation-policy`、`windows-paired-punctuation-host-policy`、`windows-edit-policy`、`windows-navigation-policy`、`windows-word-character-policy`、`windows-chinese-conversion`、`windows-preedit-caret`、`windows-fullscreen-foreground`。
- TSF 侧：`msime-tsf-client-key-router` 与 `-authenticated-client-key-router`、`msime-tsf-key-repeat-guard`、`msime-tsf-keyboard-cancellation`、`msime-tsf-smart-punctuation-fingerprint`、`msime-tsf-paired-punctuation-policy`、`msime-tsf-paired-punctuation-wiring`、`msime-tsf-punctuation-key-policy`、`msime-tsf-character-result`、`msime-tsf-raw-commit`、`msime-tsf-commit-and-continue-payload`、`msime-tsf-engine-response`、`msime-tsf-candidate-ownership`、`msime-tsf-preedit-caret`、`msime-tsf-host-focus`、`msime-tsf-prepared-options`、`msime-tsf-host-library-config`、`msime-tsf-module-path`、`msime-tsf-version-resource`、`msime-tsf-class-factory`。

`tsf/tests/exports/`、`tsf/tests/registration_profiles/`、`tsf/tests/registration_categories/` 是各自 configure 的独立子工程，做 PE 导出与注册契约检查而不加载 DLL。`platforms/windows/tests/tools/` 与 `installer/tests/` 下另有 PowerShell 套件（构建产物、通知收集、可移植可执行文件、运行时依赖、安装器入口与包内文件、TSF 注册、Watchdog 任务、升级与卸载生命周期、仓库根布局），它们需要 Windows 主机。CI 的「Windows release script tests」在 windows-2025 上跑 `tests/tools/` 的四个套件，以及 `installer/tests/` 中读 `msime_setup.iss` 的 `lifecycle`、`watchdog-task`、`tauri-layout`、`tsf-registration`；其余几个（安装器入口、包内文件、仓库根）仍只随 MSVC 构建与打包流程运行。

真词库行为另有一组探针，放在 `crates/engine-bridge/examples/` 与 `crates/input-runtime/examples/`，接收一个按 `resources/desktop-dictionary.lock.json` 备齐的资源目录。它们不挂进 `verify-local.sh`，因为锁定词库不在仓库里；但判据必须带真实词典的那几件事只有它们答得了——空词库对正确和错误的拼写一律回答「没有候选」。

## 功能分组与实现落点

路径相对本仓库；「来源入口」相对固定的来源提交。

| 功能组 | 来源入口 | 本仓落点 |
| --- | --- | --- |
| TSF 按键、焦点、edit session、UI-less | `windows/`、`server/src/ipc/` | `platforms/windows/tsf/`、`src/system/WindowsServer.cpp`、`src/ipc/SessionController.cpp`、`src/ipc/PipePeer.cpp` |
| 全拼、四种双拼、86 五笔、日语、辅助码 | README 对应指南、`engine/`、设置 `input.ts` / `helpcode.ts` | `crates/engine-bridge/`、`crates/input-runtime/`、`platforms/windows/src/ipc/SessionPump.cpp`、共享 `preferences.rs` |
| 候选分页、高亮、调频、preedit、以词定字 | README 候选调频 / preedit / 标点指南 | `src/candidate/CandidateWindow.cpp`、`src/candidate/CandidateAction.h`、`src/ipc/SessionController.cpp`、`src/ipc/ReplyCodec.cpp` |
| 中英文状态、独立英文候选、全半角、简繁、智能标点 | `server/src/english/`、设置 `input.ts` / `shortcut.ts` | `src/system/SharedConfigKeybindings.h`、`src/input/PunctuationPolicy.h`、`src/ipc/ReplyCodec.h` 的 TsfLocalConfig、`crates/client-core/src/chinese_conversion.rs` |
| K/T/U/E/M/J/Y/R 快捷模式、混输 | README 实用功能快捷模式 | Engine 桥接与共享偏好、`src/ipc/ServerSession.cpp`，混输排序在 `crates/input-runtime` |
| 谷歌云候选与 AI 联想 | README 云 / AI 联想、设置 `ai-settings.ts` | `src/candidate/CloudCandidateWorker.cpp`、`src/candidate/AiCandidateWorker.cpp`，由 `SessionController.cpp` 构造并投递输入队列 |
| 候选中英释义、腾讯云翻译、自定义翻译 | README 候选翻译 / 自定义翻译 | `src/candidate/TranslationWorker.cpp`、`src/candidate/CandidateTranslationPolicy.h`，共享 `crates/client-core/src/translation.rs` 与 `translation/store.rs` |
| 设置读取、保存、热更新与窗口行为 | `settings_app.cpp`、`config-sync.ts` | Tauri `load_preferences` / `save_preferences`、`src/system/PreferenceMonitor.cpp` 与 Server 的发布回调 |
| 损坏配置备份与修复 | `BackupCorruptConfig`、`SyncConfigWithInstalledTemplate`、`ReapplyRealCredentials` | `crates/client-core/src/preferences.rs` 的 `recover` / `recover_malformed`：原文件先备份为同目录 `preferences.json.corrupt-<UTC 时间戳>`，再保留每个仍可解析的字段（含服务凭据）；设置页读取报格式错误时提供「修复配置文件…」并可在文件管理器中显示备份；macOS 输入法进程在 JSON 解析失败时经 Host API `msime_client_recover_preferences` 自动修复一次 |
| API 凭据测试 | 来源 `server/src/settings/api_credential_test.cpp`，经设置窗 `apiCredentialTest` 消息测试 `translation.tencent` / `translation.niutrans` / 自定义翻译、`voice.asr`、`voice.polish` 与 `ai.assistant` | Tauri `test_api_credential` → `crates/client-core/src/credential/`（豆包、批量 ASR、腾讯 / NiuTrans / DeepLX） |
| 词库查询、增改删、导入导出、快捷短语 | `dictionary_manager.cpp`、设置 `dict.ts` / `tools-settings.ts` | Tauri `dictionary_request`、共享 `dictionary/access.rs` 与 `dictionary/import.rs`，回放 CLI 为 `crates/engine-bridge` 的 `MetasequoiaImeDictionaryReplay` |
| 语音热键、流式 / 批量 ASR、润色、声音 / 静音、上屏方式 | `server/src/voice-input/`、设置 `voice.ts` | `src/voice/`（`VoiceHotkey.cpp`、`VoiceInputSession.cpp`、`DoubaoAsrClient.cpp`、`CuePlayer.cpp`、`SystemAudioMuter.cpp`、`WaveOverlay.cpp`、`VoiceSessionEpoch.h`）+ Tauri 语音面板 |
| 录音设备选择 | 来源语音设置 | `src/voice/VoiceCaptureSelection.h` 与 `VoiceInputSession`，按稳定设备 ID 枚举、保存并透传给 `AudioCapture::start` |
| 手写 | 设置 `handwriting-settings.ts` 与模型资源 | `src/system/ShellSurfaces.h` → Tauri `recognize_handwriting` / `submit_handwriting_candidate`，界面在共享 `panels.tsx` |
| 屏幕键盘 | 来源面板 `server/src/keyboard-panel/KeyboardPanel.cpp` | Tauri keyboard route 与共享 `packages/ui/src/keyboard/`，投递走 Windows host 的 `send_key` |
| Emoji、颜文字、符号、剪贴板历史 | README 与来源 `clipboard_history.cpp` | `src/clipboard/ClipboardMonitor.cpp`、`ClipboardHistory.cpp`、`ClipboardPaste.cpp`、`ClipboardPresentation.cpp`，面板与目录走 Tauri `load_emoji_catalog` / `paste_clipboard_text`；最近使用空状态文字与来源一致 |
| 悬浮工具栏、托盘菜单、入口快捷键 | 来源 `window/*presenter*`、`ui-html/webview2/ftb` 与 `menu` | `src/candidate/FloatingToolbarWindow.cpp`、`src/candidate/TrayMenuWindow.cpp`、`src/input/MaintenanceHotkey.cpp`、`src/system/ShellSurfaces.h` 与 `ShellLauncher.cpp` |
| 皮肤、主题、字体、外观预览 | 来源 `appearance.ts` / `skin.ts`、`candwnd/skins` | `src/candidate/CandidateSkin.h`、`CandidatePalette.h`、`CandidateShadow.h`、`CandidateWindow.cpp`，共享 `packages/ui/src/upstream/` 与 `crates/client-core/src/skin/catalog.rs` |
| 打字统计 | 来源 Server 私有统计表与统计页 | 采集在 Server，存储与展示在共享 `crates/client-core/src/typing_statistics.rs` 与共享设置页；统计页含来源的 12 个月日历热力图与最近 30 天按日明细 |
| 更新、关于、帮助、反馈、重启 | 来源 `about-settings.ts` / `feedback-settings.ts` / `update-manifest.ts`、`restartServer` | 共享 `update-manifest.ts` 与设置页，Tauri `open_external_url` / `restart_input_method`，重启走固定 UTF-16LE `RestartServer` Aux payload |
| 服务守护、安装、升级、卸载、资源打包 | 来源 README 服务守护、`installer/`、构建脚本 | `platforms/windows/src/system/Watchdog.cpp` 与 `WatchdogPolicy.h`、`src/entrypoints/prepare_host_main.cpp`、`platforms/windows/installer/` |

### 逐项核对过的几处行为

这些是对照过程中判据比较细、结论值得单独记住的地方，都有对应用例：

- **输入方案**：全拼 `nihao` 首选 `你好`；四套双拼 profile 各自走通；微软双拼 `nihk` 的 preedit 切成 `ni'hao`；五笔 `gggg`/`hhhh`/`aaaa` 得到 `王`/`目`/`工`；日语罗马字 `nihon`/`sakura` 的假名读音与候选都正确，且反查全拼不带假名读音。ü 的两种写法（`nve`/`nue`、`lve`/`lue`，以及 j/q/x 后的 `u`）在本仓库都通，来源为 Google 解码器做的拼写改写在这边不需要。
- **调频持久化**：选过的候选在新开的会话里排到原位之前；`reset_learned_data` 之后回到出厂顺序，且这一步跑在确实写入过的 store 上；`learning: false` 时同样的选择在干净 store 上不改变任何顺序。五种调频模式各走各的规则。
- **翻页**：Engine 对单字母查询只给 24 个初始候选，`InputSession::expand_initial_candidates` 经 overlay 转发到公开门面后，运行时在末页触发展开——`j` 从 5 页变为 291 页。两条边界语义与来源一致：下一页正好是不满的末页时先展开再进去；已经在末页且当前页不满时，新到的候选填进当前页而不翻页。
- **自动造词**：锁定 Engine 的 `InputSession::commit` 内部已自带整条链，门面的 `select` 走的就是它，所以本仓库不需要来源在 Server 里手工串的那几步。判据是**简拼**——组合出来的短语答 `htpb`，重新生成的整句不答。
- **混输候选的座位表**：来源 `candidate_selection_policy.h` 的四种排布已实现进运行时的 `normalize_online_slots`，只在快照里确实存在云 / AI 候选时生效。插入是压缩的而非固定编号：没有英文候选时 AI 落第二。多于一个的云 / AI 候选按本地候选处理而不是丢弃。
- **以词定字**：`[` 上屏高亮候选的首个汉字、`]` 上屏末字，覆盖三字候选、组合被消耗、无汉字候选与越界索引。
- **辅助码**：单码调序与双码筛选按来源规格逐条核对，全拼与双拼两套方案及候选窗显示都在；五笔的逐键提示按共享 `wubi_code_hint` 显示严格前缀的剩余编码，回退与本地模式不标注。来源没有这个开关，它的逐键提示只是让前缀候选排在精确匹配之后；剩余编码提示是本仓各宿主共有的补充，Windows 从 2026-09-23 起才真正读取它（此前这句写在这里，宿主里却没有任何代码读这个偏好）。
- **八个快捷模式**（K/T/U/E/M/J/Y/R）：带锁定词库的 `ServerSession` 回归逐个验证 Shift 入口、候选生成与选词提交。Unicode 模式的数字键是「打码位」而不是选词序号，判断分在两层——引擎报 handled，运行时只把引擎拒绝的数字落到选词；这个组合有真引擎回归覆盖。未输完或无匹配的快捷模式输入（含只按了 Y 或 R）按来源 `PrepareCandidateList` 的做法显示原文（含前缀字母）作为唯一一条 Fallback 候选，空格上屏它，由 `scripts/apply_engine_local_mode_fallback.py` 进入锁定 Engine。日语方案下接管空格做「変換」的宿主都认这条例外：唯一候选的 `source` 为 9（`CandidateSource::Fallback`）时不开始转换，空格走普通上屏路径把原文提交——macOS `JapaneseSpaceCommitsFallback`、Linux IBus 与 Fcitx5 共用的 `JapaneseConversion::space`、Android `JapaneseSpacePolicy`、HarmonyOS `JapaneseSpacePolicy`（2in1 硬件空格在转换不接管时交给 `commitHighlighted`，触摸空格交给 `press(0x20)`）、iOS `KeyboardViewController.japaneseSpaceConverts`；Windows 宿主本身就按来源上屏它。
- **中英文状态**：按应用 / 全局的作用域是纯决策函数并有 `windows-mode-authority` 覆盖；标点重复与成对补全随配置帧下发并由 `windows-tsf-config-frames` 钉住；CapsLock 由 Server 持有并经 `CapsLockChanged` 帧下发；热更新有 `preference_monitor` 用例。macOS 上切到其他输入源（ABC 或其他输入法）会同时清空按应用与全局记忆的模式，切回时两种作用域都从 `default_ime_mode` 开始，对应来源 `ActivateEx` 重新写入 KEYBOARD_OPENCLOSE 与 `ClientDeactivated` 复位 `g_authoritative_cn_mode`；本输入法自身模式间的切换不触发复位，由 `shortcut` 原生测试的 `TestInputSourceModeReset` 钉住。
- **日语**：`-` 交给长音符输入而不是翻页，`-`/`=` 不再用作翻页键，TSF 与 Server 两侧保持一致，且这条走的是物理按键到真实 Engine 的完整路径。（2026-09-23 更正：此前 Server 一侧并不一致。`ServerSession::navigate` 用 `local_mode == "japanese"` 判日语，而日语是方案（`scheme` 3）不是本地模式，这个分支从未生效，开了 `-`/`=` 翻页时 Server 仍会翻页；`=` 在 TSF 里是组字中的标点，Server 却不认；空缓冲区的 `-` TSF 当输入开组字，Server 的 `edit` 只在已有组字时接；开了 `-`/`=` 以词定字时 `-` 被当成取字键吞掉。现在四处都按 `scheme == 3` 判：不翻页、`=`/`+`/`_` 走候选标点、空缓冲区 `-` 进 Engine、以词定字跳过长音符 `-`（`=` 仍取末字，与来源 `WordToCharacterDirection` 一致）。回归：`tests/input/japanese_keys.cpp` 用真实 Engine 走 `configured_key`，另有 `edit_policy`、`punctuation_policy`、`word_character_policy` 的纯函数用例。）
- **日语不做简繁转换**：来源 `CandidateTextForOutput` 在日语方案下原样输出假名与汉字。本仓 `ReplyComposer` 此前只看简繁开关，开着繁体时日语候选与上屏也被 OpenCC 转换。现在按 `traditional_projection`（开关打开且方案不是日语）决定，开关本身不动，切回中文方案即恢复繁体输出；`tests/input/japanese_keys.cpp` 钉住。
- **凭据测试**（本仓库新增的功能）：豆包走独立 WebSocket 握手，传输可注入，生产 WSS 不跟随重定向，连接阶段 5 秒、总时限 15 秒，消息与累计响应上限 1 MiB、最多 64 条消息，只发送一秒合成 PCM 静音并要求有效终态 JSON，不回传服务端诊断或识别文本；批量 ASR（OpenAI、SiliconFlow、Groq）用内存生成的一秒 16 kHz 单声道 PCM16 静音 WAV 以 multipart 上传，不访问麦克风，HTTPS、禁止重定向、5 秒连接 / 15 秒请求、256 KiB 响应上限；翻译侧要求真实译文字段而不是把任意 HTTP 2xx 当成功，自定义服务保留 HTTP/HTTPS 但禁止重定向。新版豆包 API Key 与旧版 App ID/Access Token 互斥，新版忽略残留 App ID。
- **语音**：五个语音快捷键开关、录音提示音与静音其他声音、豆包的整句流式与双向流式两个识别接口（设置页给带名字的下拉，选中即写入 `asr_endpoint`，地址不属于两个预设时显示「自定义地址」）都在。录音设备按稳定设备 ID 枚举、保存并透传给 `AudioCapture::start`。语音会话的代际由 `VoiceSessionEpoch.h` 持有，取消与失焦的旧结果不交付。
- **词库**：查询、增改删、按类型导出、快捷短语都走 Tauri `dictionary_request` 与共享 `dictionary/access.rs`；维护前后的 quiesce / resume 由 `dictionary_maintenance_handshake` 协调。
- **皮肤与外观**：Windows 消费候选字体与回退字体、主题颜色、横竖排布局与阴影字段；`windows-candidate-font-reload`、`windows-candidate-palette` 与阴影回归覆盖非法值的回退路径。来源的四套内置皮肤（fluent/wechat/graphite/willow_green）在本仓不存在：候选窗配色来自全局主题（`crates/client-core/src/skin/theme.rs` 的六个内置主题加 `custom`，目录见 `packages/ui/src/theme/theme-catalog.json`），各宿主经 `msime_client_resolve_theme` 取色；原生候选窗的两层阴影（环境层加接触层）与 `CandidateShadow.h` 的透明留白一致。外部皮肤的 `skin.toml` 在来源里由 toml++ 按完整 TOML 1.0 解析（`candidate_skin_catalog.cpp`），候选窗与设置页共用这一个加载器；本仓各宿主同样只有一个加载器，即 `crates/client-core/src/skin/catalog.rs`（`toml` crate）。设置页经它扫描目录，macOS 输入法经 host C ABI 的 `msime_client_skin_catalog` 与 `msime_client_skin_package` 调用它（`SkinManifestBridge.mm`），所以单引号字面量字符串、跨行数组、内联表、`1_000` 数字分隔、`\u` 转义这些写法在候选窗、悬浮工具栏和设置页上结果一致，设置页列为有效的包不会在候选窗里退回基底主题。字段校验与来源相同：schema_version 必须是整数 1、id 与目录名一致、各字符串长度上限、supports 枚举不重复、最小宽度 0–1000、装饰尺寸 0–500 / 0–1000 且同为零或同不为零、toolbar_stylesheet 是包内存在的单个 `.css`、preview 是包内相对路径、颜色不超过 80 字节。在此之上本仓接受 msime-skins 仓库的写法：`base = "fluent"` 只在清单里读作 `system`（msime-windows 只认它自己的四套内置外观，而 fluent 就是 Windows 原生 Fluent token），其余退役 id 照旧拒绝，偏好与主题边界上 fluent 也照旧是无效 id；装饰表可以整张省略；装饰图优先取 `decoration.image`，否则回落到 preview，`align` 为 left/center/right（默认 right）；`corner_radius_dip` 与 `[toolbar] corner_radius_dip` 为 0–32；`[candidate_window.background]` 的 image 必须是包内存在的图片，fit 为 cover/contain/stretch，opacity 为 0–1；候选配色的 `translation` 成为解析结果的 `secondary`；`[toolbar.dark|light]` 的六个颜色在加载时规范成 `#RRGGBB(AA)`，读不出的留空；`[license]` 只作元数据读取。越界或类型不对的值让整个包作为 issue 列出，而不是静默丢弃。
- **重启**：使用固定 UTF-16LE `RestartServer` Aux payload，有回归覆盖。
- **两个位数都编得过**：`CandidateWindow.cpp` 曾把无捕获 lambda 直接传给 `EnumFontFamiliesExW`，而 `FONTENUMPROCW` 是 `__stdcall`——在 x86_64 上是同一种调用约定，在 x86 上是不同类型。改成具名 `CALLBACK` 函数后，当前全部 Windows 源文件在 x86 语法检查下通过。
- **非 ASCII 用户目录**：`reset_learned_data` 拼 SQLite 的 `-wal` / `-shm` / `-journal` 路径时直接在 `path` 的 native 字符串上拼接，不过窄字符转换，因此 `C:\Users\陆傲天` 这类 profile 不会转错也不会抛；既有用例在 ASCII 与中文两种根目录下各跑一遍。
- **翻译**：缓存键按服务商与账号分域，凭据、端点、目标语言与启用开关任一变化都丢弃正负两种结果；腾讯请求签名按官方 TC3-HMAC-SHA256 独立算出已知答案钉住；本地自定义释义每次请求现算，编辑立即生效且恒胜过缓存的云端结果。
- **在线释义随取随存**：来源 `cloud_translation.cpp` 的 `ApplyTranslatedGroup` 对腾讯、自定义与小牛翻译每一批取回的非空结果都调用 `PersistGloss`，与是否上屏无关。三个桌面宿主一致：Windows `TranslationWorker.cpp` 的 `persist_english_glosses`、Linux `ClientEngine.cpp` / `FcitxEngine.cpp`，以及 macOS `InputController.mm` 在批次回调里调用的 `persistFetchedTranslations:forQuery:`，每次取回成功即写入用户释义库。过滤条件与来源相同：只存目标语言为英文的行，格式化后的释义不超过 32 个字符，且与原文按 ASCII 忽略大小写不相等；其他目标语言的释义只留在内存缓存里。过期回调（会话、代次或查询已变）不写入。`shortcut-translations` 原生测试的 `TestLearnedGlossRuntime` 钉住未上屏即可离线复用、法语行不入库与过期回复不入库。
- **云与 AI worker**：结果绑定 lease / query / 候选；去抖窗口内只付新的那次；同一前缀第二次吃缓存；空结果不入缓存；在途被取代时结果不交付；提供方抛异常只被记录而不带走 worker 线程；非法信封到不了提供方。
- **AI 候选只看 AI 助手开关**：来源 `event_listener.cpp` 的 `ai_eligible` 由英文模式、特殊模式、全拼 / 双拼、纯完整拼音、无辅助码且不在造词组成，`UpdateAiInput` 只再加上 `ai_assistant.enabled`、输入会话存在与非空的拼音切分；`general.candidate_translations`（显示候选释义）只控制释义与翻译请求。Windows 宿主有在线查询就提交 AI，macOS 的 `synchronizeAITranslations` 同样只以 `ai_assistant.enabled` 为开关，关闭释义既不阻止也不取消在途的 AI 请求；`shortcut` 原生测试的 `TestAiCandidatesIgnoreGlossSwitch` 钉住这一点。
- **AI 候选条数**：来源每次只给一条 AI 建议。本仓把条数做成共享的用户偏好 `ai_assistant.candidate_limit`（1–10，默认 3，`crates/client-core/src/preferences.rs`），macOS、iOS、HarmonyOS 设置页与 Windows 安装包的 `config.default.toml` 都暴露它，Windows `AiCandidateWorker` 按它取数。这是有意的扩展而不是缺口，不改回一条；想要来源行为的用户把它设为 1 即可。
- **智能标点**：重写标点前回读两个字符核对指纹（arm 时记下标点前面的字符），避免用户把光标移到文档里另一处同样的标点上时改错位置；文本存储读不出内容（终端与代理存储）判为匹配，以免在这些宿主里直接废掉该功能。读不回待改写字符时改走 SendInput 改写队列，执行前校验焦点 token、前台窗口与 500ms 期限。
- **成对补全关闭时的引号与书名号**：来源 `GetPunctuation` 的引号轮换（“ 之后是 ”）与 `<` `>` 嵌套计数（《〈〉》）不看成对开关，被排除的宿主（Excel）因此回落到左右轮换。锁定引擎在成对关闭时只给左半边，由 overlay `scripts/apply_engine_punctuation_alternation.py` 去掉 `PunctuationPolicy::translate` 里的两处开关判断；状态随会话存活、切换开关不重置，与来源一致。各宿主都只在成对开启且未排除时自己改写 ” → “，不会重复轮换；HarmonyOS 成对开启时由 `PairedPunctuationPolicy.reopenQuote` 做同样的改写。`crates/host-api/src/tests.rs` 经 FFI 驱动真实引擎钉住“”“”、‘’、《〈〉》与不重置。
- **候选窗绘制**：行按逐项测量而非定高，带按外观字体的回退（`ApplyFontFallback`）与卡片的显式阴影 pass。每个候选按来源 `CandidateList::MeasureItem` 分成三段：候选文字（含角标）、辅助码、译文。辅助码与候选同字号同颜色，接在文字后 4 DIP；译文字号为候选的 0.78，间隔为候选字号的 0.65，颜色按主题契约取 `secondary`（恒等于 `number`），即与序号同色，选中行取 `selected_number`（`candidate_row_number_color`），不再沿用来源 CSS 的「辅助码颜色 alpha 乘 0.62」；辅助码在选中行跟随选中文字色。候选文字本身比列宽还宽时在列内折行，行高取实测的折行高度（至少一行），文字在这个高度内垂直居中，辅助码与译文随之排到它下方。竖排时放得下就同一行，放不下就移到文字下方并按列宽换行，辅助码一旦下移译文也跟着下移；横排时译文总在文字下方。几何在 `CandidateCardSize.h` 的 `candidate_item_layout` / `candidate_page_layout`：竖排各行按自身高度堆叠、行间留来源的 `itemGap` 2 DIP；横排按来源 `CandidateList::Measure` 让每列取该候选的自然宽度（序号与分隔条、文字与辅助码那一行和译文那一行中较宽者、再加 8 DIP 列间距），从左向右排，放不下的那一列起新的一行，同一行各列取该行最高一项的高度、行与行之间同样留 2 DIP，卡片高度按夹紧后的宽度计算；换行高度由 DirectWrite 以绘制同款 WRAP 格式按绘制时的列宽实测（`wrap_measure` 对文字、辅助码、译文三段都给答案）。`paint` 以实际绘制宽度排版并缓存行矩形，`hit` 用这份缓存，点击与绘制不会错位；单行高度是来源的 `fontSize × 1.35 + 2`，卡片最小宽度 160 DIP（`kCandidateMinWidthDip`），选中条宽 3 DIP、圆角 1.5，以行的左缘为中心（来源 `itemRect.x - barWidth × 0.5`）；竖排决定上下翻转时的高度以来源的 `DEFAULT_WINDOW_HEIGHT_DIP` 232 按当前缩放取整作起点，再取与本轮显示过的最高卡片中的较大者，隐藏后复位（`candidate_vertical_decision_height`）；`windows-candidate-card-size` 覆盖这些规则。合成路径复用渲染目标时刷新 DPI，语音波形浮层的定位与缩放取自同一份 per-monitor 快照。
- **屏幕键盘**：普通键 450ms 首次延迟、75ms 间隔自动重复，粘滞修饰键与 Num Lock 单次切换；投递失败、失焦或关闭会停止重复且不自动重放。修饰键按下 / 释放的扩展键集合与来源逐键相同，布局为来源的超集（多出 F10–F12、PrtSc/Scroll/Pause、导航簇与 Menu 键）。
- **手写多字识别**：来源 `HandwritingPanel.cpp` 把全部笔画交给 Windows Ink 的 `RecognizeAsync(..., InkRecognitionTarget::All)`，中文识别器会把一行笔迹切成多个字，候选因此可以是整个词或短语。随包的 zinnia 识别器一次只认一个字，所以没有 Windows Ink 中文识别器的宿主（macOS、无识别器的 Windows 回退、无 provider socket 的 Linux）在 `crates/host-api/src/handwriting_cells.rs` 里先按书写方向把笔迹切成字格：笔迹长宽比至少 1.6 才按横排或竖排切分，按字高（竖排为字宽）估计字号，重叠或几乎相接的笔画并为一个部件，合起来不超过一个字宽的相邻部件并为一个字（左右结构的偏旁留有空隙也不会拆开），过长的格在最大空隙处再切开；超过 8 格或近似方形的笔迹整体按单字识别，所以单字仍按单字识别。每格用随包模型各自识别，候选先给各格首选拼成的整串，再按名次轮流把某一格换成它的次选，去重后共 12 个，中文在前由共享面板排序，与来源一致；任一格没有候选时退回整体单字识别。面板字号按候选字数缩放，与来源第 214 行同式。
- **剪贴板**：历史上限 50 条，文本边界为 4000 UTF-16 单位与 12000 UTF-8 字节；`normalize_clipboard_text` 只去掉 CF_UNICODETEXT 带来的东西（首个 NUL 起截断、剥掉尾部 `\r`），换行与空白是用户内容、原样往返。共享历史与 macOS 宿主同样按来源 `NormalizeClipboardText` 处理超长复制：剥掉尾部 NUL/`\r` 后截取前 4000 UTF-16 单位保存（不拆代理对），而不是整条拒绝；NUL 以外的控制字符（换页、ESC、DEL、C1 等）作为用户内容保存与粘贴，只拒绝含 NUL 的文本。macOS 的 IMK 输入源采集与 Tauri 监视器都跳过不含纯文本或带 nspasteboard.org 标记类型（Concealed／Transient／AutoGenerated）及密码管理器私有类型的复制，这是 macOS 平台惯例，来源只读 CF_UNICODETEXT、没有对应过滤；两处共用同一份类型表（`BackendClipboardCapture.swift` 与 `crates/host-macos/native/clipboard.mm` 各持一份，须保持一致）。
- **简繁转换**：共享层 `crates/client-core/src/chinese_conversion.rs` 按 OpenCC `data/config/s2t.json` 实现词级转换（兼容表归一化，再以 STPhrases ∪ 地区词派生表 → STCharacters 做最大正向匹配，完整 IDS 序列整体透传），数据取自来源钉的同一个 OpenCC 提交并登记在 `docs/third-party.md`。边界导出 `msime_client_simplified_to_traditional` 返回裸文本而非 JSON——它在每个候选上都要调用；Windows、Linux、macOS、Android、iOS 与 HarmonyOS 宿主都调这一个导出，不使用系统的 `LCMapStringEx`、`libicu`、`CFStringTransform`、`android.icu.text.Transliterator` 或 `i18n.Transliterator` 逐字转换；首次调用解析词典，之后单次约 1 µs。与该提交构建出的 OpenCC CLI 做对照，4 万行随机文本逐字节一致。
- **安装器**：完整安装器提供数据目录选择页，拒绝系统 / 用户关键目录的父级、受保护目录内部、路径穿越与未标记的非空目录，并在就绪页展示迁移源与目标；`config.toml` 只 `onlyifdoesntexist`，升级不覆盖用户数据；light 包固定原目录。安装前在第一屏之前查注册表确认 WebView2 与 VC 运行库（要求 14.20 以上而非只看 `Installed=1`），静默安装默认继续并把缺失写进日志。
- **云候选的首次同意（macOS）**：来源安装器的「联网功能」页在全新安装时说明云候选会把正在输入的拼写发给 inputtools.google.com，默认勾选、可取消，升级时跳过，只写 `cloud_candidates = false`。macOS 没有每个用户都会经过的安装步骤，输入法也可以在从未打开设置应用的情况下使用，所以由 IME 进程自己问：`AppearancePreferences.mm` 的 `resolveCloudCandidatesConsentWithPreferencesDirectory:userDataDirectory:` 在建立会话之前判断一次——宿主偏好里已有云候选选择、`preferences.json` 已存在或 Engine 用户数据目录（会话选项 `user_data`）已有内容即视为升级，不问、原值不动；否则记为待确认，结果存进 `MSIMEClientCloudCandidatesConsent`。待确认期间不发任何云候选请求，`InputController.mm` 的 `activateServer:` 在激活之后异步弹出不阻塞输入的「联网功能」对话框，文案沿用来源安装器页和 Linux 首次配置页，末句按来源的「设置 → 输入」指向共享设置输入页的「云候选」，按钮为「启用云候选」（默认，对应来源的默认勾选）与「不启用」；答案经 `answerCloudCandidates:` 写入共享 `cloud_candidates`，与 Windows、Linux 同一个字段。原生设置里改动云候选开关也算作答复。`shortcut` 测试的 `TestCloudCandidateConsent` 覆盖全新、升级（已有选择、已有 `preferences.json`、只有 Engine 用户数据）、空的用户数据目录、之后才出现的 `preferences.json`、拒绝与接受。
- **安装后的首次准备**：完整安装器在提升权限下写入词库、出厂配置和所有权标记，但不以安装器身份替用户执行 Host API 准备。Server 的生产首次启动因此除全新目录外，也接受已有且带 `.metasequoiaime-data` 所有权标记、尚无 `runtime-options.json` 的目录，在用户上下文中完成准备；普通已有目录、已有运行时配置、文件和符号链接不会被接管或重建，准备失败保留现有数据。独立的 `msime-client-prepare` 仍只接受全新目录。
- **升级后的用户词库回放**：来源 `installer/msime_setup.iss` 的 `ReplayUserDictionary` 在每次升级时以 `--data-dir` 运行随 Server 安装的 `MetasequoiaImeDictionaryReplay.exe`（源码在 `server/src/user-dictionary-replay`），失败就中止安装。Windows 安装器原样保留这一步，工具由 `crates/engine-bridge/src/bin/MetasequoiaImeDictionaryReplay.rs` 构建。没有按用户安装步骤的平台改用共享的 `msime_host_api::refresh_host_options`（导出为 `msime_client_refresh_host`）：比较运行时配置的词库代次与编译进去的词库锁，不一致时准备新代次、回放用户词库日志并原子改写 `resources` / `dictionaries`。Linux 由 IBus 与 Fcitx5 宿主在建立会话前调用，macOS 由 IMK 宿主启动时（`RuntimeOptionsRefresh.h`）和设置应用启动时（`macos_launch.rs`）调用。刻意的差异：macOS 与 Linux 没有按用户执行的安装步骤可以中止，失败时保留旧代次继续输入，下次启动再试。
- **检查更新**：各平台由 `.github/workflows/release-*.yml` 独立发布到同一个仓库、标签带平台前缀，所以读的是发行版列表而不是 `releases/latest`——后者返回的通常是别的平台那一个。只取本平台前缀、非草稿、非预发布的发行版，按版本号而不是列表顺序取最新，没有则显示「暂无可用发行版」。Server 遥测上报的版本号由 CMake 从 `platforms/windows/version.txt` 读入，`Build-Client.ps1` 的 `-TargetVersion` 把同一个版本号同时传给 Tauri 和 Server。
- **外部链接**：走 `msime_host_windows::open_url`，用 `ShellExecuteW` 把 https URL 直接交给默认浏览器，非 https 一律拒绝，与已有的 `open_directory` 共用同一段调用；不经 `cmd /C start`，不闪控制台窗口，URL 也不过 cmd 解析。

### 来源源码树的落点

上面那张表按功能组织，回答某个功能有没有；这一节按**来源的源码目录**组织，回答来源的每一块代码去了哪儿。两者互相校验：一块代码找不到落点就是缺口，哪怕对应功能在表里被标成有。`scripts/test-reference-source-inventory.py` 把这一节变成每次都会跑的断言。

**TSF DLL：逐文件对应。** 来源 `windows/src/` 下的 38 个 `.cpp` 在 `platforms/windows/tsf/` 全部都有，同名同目录结构。另有 6 个来源没有的：`EngineResponse.cpp`、`EngineSessionAdapter.cpp`、`HostOptionsPaths.cpp`、`PreparedHostOptions.cpp`、`Global/TsfPropertyGuids.cpp`、`Thread/ThreadState.cpp`——它们是进程边界带来的，来源把 Engine 放在同进程，这边 TIP 要通过契约与 Server 对话。再加一套来源没有的 TSF 测试（`platforms/windows/tsf/tests/`）。

**Server：按目的地分三类。**

| 来源目录 | 落点 | 归类理由 |
| --- | --- | --- |
| `ipc/`、`session/`、`watchdog/`、`log/` | `platforms/windows/src/ipc/`、`src/system/`，TSF DLL 与 Server 共用的协议头在 `platforms/windows/common/` | 进程边界与 TSF 协议，只能是原生 |
| `window/`（候选窗、悬浮工具栏、托盘） | `platforms/windows/src/candidate/` | 低延迟、不抢焦点，保留原生 Direct2D |
| `voice-input/` | `platforms/windows/src/voice/` + Tauri 语音面板 | 热键与上屏原生，界面在 Tauri |
| `cloud/cloud_ime.cpp` | `platforms/windows/src/candidate/CloudCandidateWorker.cpp` | 在输入队列上跑，跟着会话生命周期 |
| `cloud/tencent_tmt.cpp`、`cloud_translation.cpp` | `crates/client-core/src/credential/translation.rs` | 凭据与请求逻辑跨平台共享 |
| `cloud/custom_translation.cpp`、`translation_gloss.cpp` | `crates/client-core/src/translation.rs` | 同上 |
| `skin/candidate_skin_catalog.cpp` | `crates/client-core/src/skin/catalog.rs` | 皮肤目录跨平台共享 |
| `english/`、`emoji/`、`kaomoji/`、`conversion/` | Engine 与共享偏好 | 这些是输入算法的一部分，Engine 拥有 |
| `user-dictionary-replay/` | `crates/engine-bridge`（`MetasequoiaImeDictionaryReplay`） | 词库维护跨平台共享 |
| `settings/`、`webview2/`、`emoji-panel/`、`keyboard-panel/`、`handwriting-panel/` | Tauri 壳（`apps/desktop/`、`packages/ui/`） | 公共功能与界面放 Tauri；来源用 WebView2 自绘，这边两个桌面宿主共用同一个 Tauri 应用，入口契约见 `src/system/ShellSurfaces.h` |
| `utils/` | 分散在对应模块 | 工具函数不单独成目录 |

来源另有 `experiments/tsf-edit-control`，已迁入 `platforms/windows/experiments/tsf-edit-control` 并接成 Windows-only CMake 目标：Direct2D/DirectWrite 绘制的原生 Win32 编辑宿主与最小 demo，用来重复检查 TSF 文档上下文、preedit 与 display attribute、候选位置、软换行、选区、插入点和鼠标命中这些边界。它是验证工具而非产品组件，不注册 TSF、不启动生产 Server。来源工程特有而这边不存在的 `common.ver`、`InputScope.h`、`tsattrs.h` 依赖已去除，宿主仍用系统 TSF 头文件。

## 与来源刻意不同的取舍

这些差异都是适配本产品的进程结构与多平台共享层所做的取舍，每条都记着理由与替代物，不是欠账。

### 工具栏可选按钮改为默认关

`floating_toolbar` 的 `emoji`、`handwriting`、`voice`、`screen_keyboard` 在共享层全部默认关，新配置拿到的是五个按钮的精简工具栏：中英切换、标点、全角、简繁、设置。改动落在 `FloatingToolbarPreferences::default()` 与它的 serde 默认、`packages/ui` 的 `defaultFloatingToolbar`、以及 macOS 宿主的同名回退——四处必须一致，否则设置页显示的状态与工具栏画出来的会是两回事。

这推翻了上面《补上 handwriting / voice 开关》里那条决定。当时的理由是「它们从一开始就在工具栏上，开关出现的那一刻不该让两个按钮消失」，而这一次选择了相反的取舍：**从没动过这些开关的配置升级后会少掉这三个按钮**（`emoji`、`handwriting`、`voice`），要自己去设置里打开。接受这个代价是为了让默认工具栏保持精简；`floatingToolbarBoolean:defaultValue:` 与 serde 默认都只在键缺失时生效，所以显式开过的配置不受影响。

`emoji` 是来源自己的 6 个组件之一，这是本节里唯一一处组件默认值与来源不同；id、标签、顺序都没变。Windows 不跟着变：`platforms/windows/installer/config.default.toml` 逐项显式写出每个组件，镜像来源的默认配置，共享默认值在那里轮不到生效。跟着变的是不带出厂配置的宿主，即 macOS 与 HarmonyOS 2in1。

**候选相关的四个表面由原生 Direct2D 绘制。** 候选窗、候选浮出、悬浮工具栏、托盘菜单在来源是 WebView2 文档，在这边是原生窗口。来源把 WebView2 同时当作候选窗的可选渲染后端，这边候选窗只有 Direct2D 一种实现；配置键 `ui_backend` 作为契约保留并登记在字段漂移门禁的 `RUST_ONLY` 里。

**这四个表面走合成交换链，而不是来源的分层窗口。** 来源的输入法窗口用 `WS_EX_LAYERED`（`server/src/window/ime_windows.cpp`），DirectComposition 只出现在它的 WebView2 与设置路径里；这边统一走 `DeviceResources::EnsureForComposition`（`DCompositionCreateDevice` 加 `CreateSwapChainForComposition`）。理由是合成交换链避开 `UpdateLayeredWindow` 每帧的 CPU 拷贝，而这四个表面都是低延迟且不能抢焦点的。失败时也不回退到普通 HWND 交换链：那拿不到逐像素透明，候选卡片的阴影会退化成不透明矩形，静默变丑比明确失败更糟。代价记在这里：无 DirectComposition 的环境（如 Wine）要画出这套，是一次明确的渲染路径工作。

**设置页与各类面板走 Tauri 与共享 React。** 设置、表情、手写、屏幕键盘、语音面板都在独立的 Tauri 进程里，两个桌面宿主共用同一个应用，入口契约是 `src/system/ShellSurfaces.h`。

**菜单的禁用语义因此与来源不同。** 来源的菜单 HTML 里 `disabled` 出现零次——它的面板全在同进程内，永远可用。这边的面板在独立的壳里，壳可能不在，所以能力缺失的行**保持可见但置灰**，而不是点了没反应或干脆隐藏（`src/candidate/TrayMenuLayout.h` 的注释写明依据：保住来源「菜单从不隐藏条目」的可见性语义，同时诚实反映进程边界）。托盘菜单的动作与来源一一对应，且这边多出手写识别板。

**偏好文件每次整份写出，不做三方合并。** 来源的升级路径是模板三方合并——用户改过的键保留、仍停在旧默认值的键跟随新默认值、模板里没有的键与段落丢弃。这边的 `preferences.json` 每次把全部字段写出来，没有「旧默认值」这个概念，所以改默认值到不了已有用户；`deny_unknown_fields` 又让退役字段不能删（`ui_backend` 的注释已写明）。这是存储模型层面的取舍。附带的好处是来源那次「升级时配置解析失败被出厂模板覆盖、凭证清零」的缺陷在这边不存在：共享偏好用原子写入，普通读取在解析失败时返回错误而不是回落默认值后再写回；修复是显式动作，先备份原文件，再按字段保留仍可解析的值（凭据随所在段落逐字段保留），不需要来源那种单独回填凭证的步骤。

**智能标点在 Windows 与 macOS 上默认关闭。** 来源出厂把整族五个开关（`ime_config.cpp` 的初值与 `value_or(false)`、`config.default.toml`）全部设为关，Windows 随包的 `config.default.toml` 也一样；但那只是安装模板，运行中的宿主读的是共享偏好文档。默认函数 `smart_punctuation_default()` 写成 `!cfg!(any(windows, target_os = "macos"))`，同时用作 `Default` 值和缺键时的 serde 默认，覆盖智能标点、重复标点转中文与数字/字母后直出两半；空格转换在所有宿主上都默认关。macOS 是那套桌面产品的移植，跟着来源走。Linux、Android、iOS、HarmonyOS 一直是开着发的，让偏好在老用户脚下变掉比按平台不同更糟，所以保持原样；已存下来的值不受影响。macOS 原生侧在共享快照到达之前的回退值（`AppearancePreferences.mm` 的 `smartPunctuation` 与 `smartPunctuationRepeatToChinese`、`CloudAppearanceSettings.h` 的云端快照默认）与之一致。判据在 `scripts/test-default-config-parity.py`。

**三个语音开关在 Windows 与 macOS 上默认打开，其余宿主默认关闭。** 来源的 `config.default.toml` 出厂就打开 `mute_system_audio`、`doubao_enable_ddc` 与 `polish_text`，Windows 随包模板也一样，但运行中的宿主读的是共享偏好文档，而它原来三个全关，于是 Windows 的全新 profile 拿到的恰好与自己随包的模板相反——和智能标点是同一个坑。默认函数 `source_voice_default()` 写成 `cfg!(any(windows, target_os = "macos"))`，同时用作缺键时的 serde 默认；Linux、Android、iOS、HarmonyOS 保持原样，理由同上一条；已存下来的值不受影响。macOS 原生侧在共享快照写进 NSUserDefaults 之前使用的回退值（`VoiceProviderOptions.h` 的润色与 DDC、`MSIMEVoiceMuteSystemAudioEnabled`、原生语音设置窗口的勾选框）与之一致。`default_ime_mode` 同理：`DefaultImeMode::default()` 在 Windows 上是英文、其余宿主是中文，于是 Windows 上没有 `preferences.json` 的全新安装起手英文，与来源和随包模板一致（Server 的模式权威与 TSF 的 `ReadConfiguredDefaultImeModeChinese` 读的都是这份默认）；macOS 仍是中文：用户是从输入法菜单里主动选中本输入法之后才开始打字的。判据在 `scripts/test-default-config-parity.py`，模板与共享默认任一边改回去都会报出来。

**Emoji 混输在 Windows 与 macOS 上默认开启，其余宿主默认关闭。** 来源的 `config.default.toml` 出厂就是 `emoji_mixed_input = true`。Windows 靠随包的 `config.toml` 加上一次性的 `migrate_windows_legacy_mixed_input` 导入拿到它；共享默认 `source_mixed_emoji_default()` 写成 `cfg!(any(windows, target_os = "macos"))`，让 macOS 的全新 profile 得到同样的答案，Windows 在模板缺失或 `mixed_input` 段缺失时同样得到开启。macOS 原生侧 `mixedEmojiInput` 在共享快照或本地值出现之前的回退值与之一致。颜文字混输在所有宿主上仍默认关闭，与来源相同。Linux、Android、iOS、HarmonyOS 保持原样，理由同智能标点那一条；已存下来的值不受影响。判据在 `scripts/test-default-config-parity.py`。

**润色服务与 AI 助手在 Windows 与 macOS 上默认指向 DeepSeek，AI 助手默认开启；其余宿主保持 SiliconFlow / Qwen 与 AI 关闭。** 来源的 `config.default.toml` 出厂把 `polish_provider` 设为 `deepseek`、`polish_endpoint` 设为 `https://api.deepseek.com/chat/completions`、`polish_model` 设为 `deepseek-v4-flash`，`[ai_assistant]` 为 `enabled = true` 并用同一个 provider、接口与模型，Windows 随包模板也一样；运行中的宿主读的是共享偏好文档，共享默认若不跟随，模板的值就到不了全新 profile，与语音开关是同一个道理。共享默认里润色三项由 `default_polish_service()` 成组给出（沿用 `source_voice_default()` 的判定），AI 助手由 `source_ai_default()`（`cfg!(any(windows, target_os = "macos"))`）决定开关并附带 DeepSeek 接口与模型，`enabled` 缺键时的 serde 默认也用它；接口与模型缺键时仍为空，已存下来的空值不被重新解释。AI 助手默认开启但不带 Token，`chat_completion_http_request` 在 Token 为空或是占位值时返回 `InvalidConfiguration`，不会发出任何请求；原生 AI 设置窗口要求开启时接口有效，默认接口满足这一点。macOS 原生侧在共享快照写进 NSUserDefaults 之前的润色回退值（`SharedVoicePreferences.h` 的 `MSIMEVoicePolishDefault*`，供语音设置窗口、备用 provider 窗口和录音请求共用）与之一致。Linux、Android、iOS、HarmonyOS 保持原样，理由同智能标点那一条；已存下来的值不受影响。判据在 `scripts/test-default-config-parity.py`，它同时核对来源模板、Windows 模板与共享默认。

**智能标点的三个子开关不走独立 opcode。** 来源 `windows_ipc.h` 的 22/24/25 三个 opcode 在这边由一帧打包的标点配置携带。

**打字统计的落点与存储都与来源不同。** 采集放在 Server 而不是 TSF DLL：来源的 Engine 与 DLL 同进程，而这边 Server 是唯一看得到每一条上屏字符串的地方，共享 Host API 也链在这一侧，文本本来就要作为上屏载荷从 Server 走到 DLL，采集不让它多跨任何一道边界。唯一的例外是 TIP 不吃掉的按键：它们由应用自己插入，永远到不了 Server 的上屏出口，于是英文模式的字母、中文模式下的半角数字与标点表之外的符号由 DLL 在 `OnTestKeyDown` 的三个放行出口采集，按批经已有的 Aux 管道（`TypingStatistics` 动词）交给 Server 落盘，和上屏出口共用同一段代码——即便如此，来源为此另开的那条统计命名管道（`FANY_IME_STATS_*` 契约）这边仍然不需要。统计默认关闭，关闭时 Server 不回 OK，DLL 据此退避，不在关闭期间持续把按键字符送过管道。macOS 按同一条规则在 `handleEvent:client:` 的出口采集输入法没有吃掉的按键：Command 与 Control 组合键、AppKit 功能键区（方向键、F 键、Home/End、向前删除落在 `NSEvent.characters` 的 0xF700–0xF8FF）不计，Option 打出的字符计入，因为 Option 是 macOS 的字符层、相当于 Windows 的 AltGr；英文模式记为 `english`，中文模式下放行的数字与符号记为当前方案的来源；和来源一样，这是按键时的预测而非编辑确认。存储则做在共享 `crates/client-core/src/typing_statistics.rs` 与共享设置页，而不是来源的 Windows 私有 SQLite 表：这些维度和「每天多少字」是同一件事，后者早就在共享层、六个宿主写同一份文档，单开一套 Windows 私有存储会让同一个用户的统计分裂成两份。速度指标另有一处有意不同：来源只数 `cjk + latin`，而它的 `latin` 是纯 ASCII 字母、假名落在 `other`，于是纯日文输入的速度恒为零；这边有完整日文模式，所以假名与谚文等也算进可读字符，数字与标点仍然不算。「自动清理」的保留策略与来源一致：「永久保留」从不删除任何一天，「30/90/180/365 天」在每天第一次写入时删掉边界之前的日期，收窄设置时立即生效；和来源 `ClearThrough` 删掉概览所求和的 `stats_daily` 行一样，被删的日期同时从累计总数与分类中扣除，于是「累计」、分类占比与日均都只覆盖保留下来的日期，日均按保留日期的字数之和除以日期数计算。来源统计页的「日历热力图」在共享设置页原样保留：近 12 个月每天一格、每列一周且周一开始，月份只标在含该月 1 日的那一列，五档深浅按当天字数占窗口内最高一天的四分位分级，每格悬停显示当天字数或「无记录」，未来的日期不画；桌面宿主把它作为趋势之后的独立一节，手机放在「趋势」标签下，点按格子会把分类与占比切到当天。来源的「按日明细」表同样在桌面布局保留：列出最近 30 个有记录的日期、新的在上，列为日期、字数、汉字、字母、数字、标点、其他、活跃与速度，「平均速度」下同时标出累计活跃时长；分类列按这边更细的字符类别对应，「其他」含其他文字、表情、符号与历史未分类，列名用「汉字 / 字母」而非来源的「中文 / 英文」，因为假名等文字在这边不算英文；早于活跃时长记录的日期「活跃」与「速度」显示为未知（「—」），不按来源记作零；手机布局放不下九列，不画这张表。

**半截词的 preedit 不在 Windows 打开。** 其余五个宿主（macOS、Linux、HarmonyOS、Android、iOS）把「已选的那一段 + 读音」画在组字里，Windows 的 TSF 侧自己累积前缀，打开会重复；桌面外壳没有候选窗，不适用。

**候选文字超宽时按来源折行，放得下时仍用单行格式。** 来源 `CandidateList::MeasureItem` 对候选文字一律用 WRAP 格式实测高度、行高取它与单行高度中的较大者；这边在 `candidate_item_layout` 里只在文字的单行宽度超过列宽时才折行测量（`CandidateRun::text`），`paint` 也只对这样的文字改用 WRAP 格式，其余仍是 NO_WRAP，免得实测宽度与绘制之间的取整把本来排成一行的文字折成两行。两种做法对放得下的文字给出同一个一行高度，对放不下的给出同一个实测高度，所以这是实现上的差别而不是行为上的。横排仍按来源 `CandidateList::Measure` 按每项自然宽度分列，只有一个候选单独就比整行还宽时才被收窄到行宽并在其中折行；竖排则在卡片被工作区一半封顶后按行宽折行。行矩形在 `candidate_page_layout` 里一次算出，绘制与命中测试读同一份，折出来的每一行都点得中。卡片高度仍被工作区一半封顶，折行极多时底部会被截掉；来源 `Measure` 同样按可用高度截，这一点两边一致。选中行左侧的强调竖条也按来源 `CandidateList::Render` 画：高度固定为候选字号的 0.85（不小于 6 DIP），在行矩形内垂直居中，行比竖条还矮时从行顶开始（`candidate_selection_bar`）；此前按行高上下各内缩四分之一，折行把行撑高时竖条会跟着被拉长。卡片在光标上下的取舍同样照来源 `AdjustCandidateWindowPosition`：竖排用本次组字里出现过的最高高度判断，免得列表越打越长时中途从下方跳到上方；横排每次用当前高度判断，来源对横排没有「选定一侧后保持」的规则，这边也不另加。

**诊断日志固定写数据目录。** 来源先写桌面、失败再退回数据目录；这边固定写数据目录下的 `logs\server.log`，因为输入法在桌面上凭空出现文件不是用户预期的副作用。设置页的「Server 端日志」「TSF 端日志」两个开关（`diagnostic_log.server` / `diagnostic_log.tsf`）分别控制写入，内容只有 Server 启停原因、各组件是否就绪、退出码与 TIP 上报的诊断批次，不记按键、输入内容或候选文本；4 MiB 轮转为 `server.log.1`，最多保留两份；UTF-8 BOM 与 CRLF 行尾与来源一致；偏好发布时立即生效，无需重启 Server。

**macOS 的诊断日志留在 Application Support，并由设置页在 Finder 中显示。** 对应 `windows-diagnostic-log`：来源 `candidate_diag_log` 用一个开关覆盖按键延迟、候选窗、工具栏、托盘菜单与 IPC 生命周期，文件写到桌面。macOS 输入法用同一个 `diagnostic_log.server` 开关写偏好目录下的 `diagnostic.log`（1 MiB 轮转为 `.1`），记录焦点进出、偏好加载/应用/保存、`handleEvent:client:` 中不少于 8 ms 的 `[key-latency] stage=handle` 耗时（与来源 `ScopedServerKeyLatency` 的阈值相同）、候选窗的显示位置与构建耗时、隐藏原因，以及输入统计写入失败的类别；来源的 queue 与 reply-send 两段没有对应，因为 Engine 在输入法进程内，按键不跨 IPC；工具栏与托盘菜单的记录点也没有搬过来。文件不放桌面，设置页「在 Finder 中显示」由宿主解析位置后选中它。与 Windows 一样，不记按键、输入内容或候选文本，也不转写共享层返回的错误字符串。细节见 `platforms/macos/README.md` 的「诊断日志」。

**macOS 语音有三处刻意与来源不同。** 静音其他声音是整台默认输出设备而不是按进程，因为 macOS 13 没有公开接口，时序上改为开始提示音播完再静音、先恢复再播结束提示音，以保证提示音听得见；录音录满上传上限时自动结束并提交已录部分，而不是像来源那样提交时报超限并丢掉整段；提示音文件缺失时回落到系统声音而不是不出声。细节见 `platforms/macos/README.md` 的「语音输入」。

**Windows 语音的失败提示显示在语音浮层上，而不是来源的模态消息框；录满上传上限时自动结束并提交。** 来源 `voice_input_service.cpp` 用 `MessageBoxW` 报缺 Token、豆包启动失败、麦克风启动失败与识别失败；这边 `platforms/windows` 里没有任何模态对话框，已有的表面是 `WaveOverlay`，与 macOS 把失败留在浮层上的选择一致，文案沿用来源的句子（豆包那句把「请检查 config.toml」改成指向设置页的「语音输入」分区）。浮层提示由后台任务显示 4 秒，不占用控制线程；新的录音开始时旧提示随会话代际失效。批量录音的上限与 macOS 相同，取共享的 `batch_capture_sample_limit`（20 MiB WAV 上传预算），录满即结束并提交已录部分，而不是像来源那样在提交时报超限。

**macOS 的中文标点与全半角是每个应用的运行时状态，保存的值只当起点。** 来源把两者放在 TSF 线程管理器的 compartment 里（`MetasequoiaIMEGuidCompartmentPunctuation` / `DoubleSingleByte`），按 UI 线程、实际上按应用各一份，从不写回配置：每次 Activate 重置为半角、标点跟随默认中英文模式，Deactivate 清空，Ctrl+.、Shift+Space 与悬浮工具栏只翻转 compartment，中英文切换时 `SyncPunctuationWithImeMode` 让标点重新跟上模式。macOS 的 `AppearancePreferences` 对应地按前台应用的 bundle id 在内存里记 `runtimeChinesePunctuation` / `runtimeFullWidthInput`，不受 `ime_mode_scope` 影响；Ctrl+.、Ctrl+Shift+Space、Option+Shift+H 与工具栏只改当前应用，不写 NSUserDefaults、共享文档或云端快照；中英文切换按新模式重定标点：进入英文模式时标点变为英文（`punctuation_lock` 固定为中文时除外），回到中文模式时丢掉当前应用的标点覆盖、回到保存的起点；从本输入法切到别的输入源时与中英文模式一起清掉所有应用的两项覆盖，对应 Deactivate 一并清空三个 compartment，普通的焦点切换不清。唯一有意保留的差别是起点：共享设置页在每个宿主上都提供 `chinese_punctuation` 与 `character_width`，于是 macOS 以保存的值为每个应用的起点（与 iOS 键盘相同），而不是像来源那样标点总从中英文模式出发；这两个保存值一旦改变（设置页、共享文档或云端恢复），所有应用的对应覆盖一并作废。英文模式把两项运行时状态同样用在键入的 ASCII 上，对应来源 `KeyEventSink.cpp` 在输入法关闭时仍走的 `FUNCTION_PUNCTUATION` 与 `FUNCTION_DOUBLE_SINGLE_BYTE` 分支，转换与 Linux 宿主共用 `shared/input/EnglishModeOutput.h`；固定的 `punctuation_lock` 在英文模式下压过 Ctrl+. 与工具栏，对应 `Ipc.h` 的 `ResolvePunctuationOpen`。`shortcut` 原生测试的 `TestPerApplicationPunctuationAndWidth` 与 `TestEnglishModePunctuationAndWidthOutput` 钉住这些规则。

**macOS 用两个输入模式承担来源托盘语言栏的常驻模式图标。** 来源 `LanguageBar.cpp` 以 `TF_LBI_STYLE_SHOWNINTRAY` 注册语言栏按钮，`GetIcon` 在中 / 英之间换图，大写锁定时换成 Caps 图、日语模式换成「日」，`RefreshLanguageBarIcons` 同时重画全半角与标点两个按钮，所以无论悬浮工具栏是否打开，托盘上总能看到当前状态。macOS 的输入法没有托盘，菜单栏里的输入源图标就是对应位置：`Info.plist.in` 声明中文模式 `.Hans` 与英文模式 `.Roman`，中文模式带产品标志的模板图标、英文模式带「英」（来源在中 / 英之间换图，这边中文那一格换成了标志：带模式的输入法在菜单栏永远显示模式图标而非 bundle 图标，那是标志唯一能出现的位置，而常态本就是中文），`InputController.mm` 在中英文切换时通过 `selectInputMode:` 选中对应模式，系统报告的模式切换（从输入法菜单选择或 Ctrl+空格 / 地球键切到另一条）经 `setValue:forTag:client:` 反过来改中英文状态，两边互不回声（`InputModeIdentifiers.h`，`input-mode-identifiers` 与 `shortcut` 原生测试）。其余托盘内容按平台分派：Caps 由系统自带的大写锁定指示承担，「日」与全半角、标点按钮在默认开启的悬浮工具栏上显示，不另占菜单栏图标。代价是英文模式在系统设置的输入源列表和 Ctrl+空格轮换里是单独一条，名称为「水杉输入法 · 英」/「Metasequoia · EN」以免与中文条目同名。

**设置页有几处措辞与控件刻意与来源不同**：「始终使用英文标点」与这边的「中文标点」绑同一个 `chinese_punctuation` 但极性相反，只改名不反转控件即是错标；剪贴板管理来源写「关闭后立即清空」，这边写「保存关闭设置后清空」，因为这边的清空发生在偏好保存时；候选窗字体一项各宿主都只显示「主字体」（Windows 另有「候选窗英文字体」），不再提供来源的「中文补充字体」排序列表：补充字体由字体预设写入；Windows 渲染器不读 `candidate_font_family`，所以在 Windows 上选主字体时同时把它排到补充字体最前（`candidateMainFontPatch`）。

## Windows 进程与协议边界

**两个目标，边界写死在构建里。** TSF tip 是进程内 DLL（`platforms/windows/tsf/`，`OUTPUT_NAME MetasequoiaImeTsf`，经 `IME/MetasequoiaIME.def` 导出四个未修饰 COM 入口，链 Rust `msime-host-api` 的导入库）；Server 是独立的窗口子系统可执行文件（`src/entrypoints/server_main.cpp`，`OUTPUT_NAME MetasequoiaImeServer`）。另有 `MetasequoiaImeWatchdog`（对应安装器的登录任务）与 `msime-client-prepare`（准备 `runtime-options.json`）。

**Server 是窗口程序而不是控制台程序。** 控制台子系统会让 Watchdog 与 TSF DLL 每次拉起 Server 都带出一个黑色控制台窗口。现在链接为 Windows 子系统（MinGW `-mwindows`；MSVC `WIN32_EXECUTABLE` 加 `/ENTRY:wmainCRTStartup`，入口仍是 `wmain`）。`--config` 预览与 `--help` 从终端启动时挂到父控制台（`AttachConsole(ATTACH_PARENT_PROCESS)`），状态行与 Ctrl+C 照旧；受管启动（`--watchdog-managed` / `--production`）从不挂接，因为 TSF DLL 是在当前焦点程序里拉起 Server 的，那个程序本身可能是控制台程序。

**退出码是 Watchdog 契约的一部分。** 维护快捷键的「停止」（Ctrl+Shift+Alt+T）返回 `watchdog::stop_exit_code` 而不是 0——返回 0 会被 Watchdog 判为非正常退出并在两秒后重新拉起，等于停不下来。已打开的设置等 Tauri 窗口是独立进程，不随 Server 关闭，与「重启」时的行为相同。

**命名管道分三个角色**：Main、Aux、Diagnostic（`src/ipc/` 下的 `PipeListener`、`PipeService`、`PipeMainTransport`、`AuxListener`、`DiagnosticListener` 等）。协议侧的几条规则是这条边界的要害，都由用例钉住：

- hello 帧里的 `client_id` 不是认证。控制器进程不能复用 TSF 目标进程的 ID；对端由 OS 提供的信息验证（`PipePeer::bind` 校验客户端 PID、登录会话与 TokenUser SID），焦点租约另外单独绑定。
- `PipeRegistry` 的注册代际不等于激活 epoch。排队前检查不够，执行时仍要检查焦点、会话与代际。
- 未握手的 Aux 通道不是可接收凭据与识别文本的认证语音通道。
- 投递结果分 `Sent` / `DefinitelyNotSent` / `DeliveryAmbiguous` 三态，fallback 按三态分别处理，而不是把「不确定」当成「失败」重放。

**面板文本投递先校验再恢复焦点。** Windows host 导出与 Tauri 面板入口共用的 `valid_text` 判据，在恢复目标编辑器焦点**之前**拒绝空串、超过 4096 字节的文本和控制字符（`send_text` 复用同一判据，避免两个入口漂移），因此无效请求不会改变用户当前焦点。恢复焦点本身在 `SetForegroundWindow` 成功后立即读 `GetForegroundWindow` 确认目标确实是前台窗口，才继续全局 `SendInput`；语音路径与面板路径共享这道边界。

**偏好热更新的时序。** 监视器先在输入队列应用 `PreferenceSnapshot`，再从监视线程通知 `SessionController` 的发布回调；回调清理并按新配置重新发起当前候选页的翻译查询。应用延迟到未确认的回复完成之后，且回调提交的观察任务必须看到新的导航绑定与以词定字状态——「发布任务已入队」不等于「偏好已生效」。

**TSF DLL 的 COM 边界。** 类工厂契约由 `msime-tsf-class-factory` 钉住：从同目录加载出货 DLL，解析 `DllGetClassObject`，用固定 CLSID 取得 `IClassFactory`，实例化的对象实现 `ITfTextInputProcessor`；未知 CLSID 返回 `CLASS_E_CLASSNOTAVAILABLE`，已知 CLSID 但请求不支持的类工厂接口返回 `E_NOINTERFACE`，空输出指针在 `QueryInterface` 返回 `E_POINTER`、在 `CreateInstance` 返回 `E_INVALIDARG`；类工厂拒绝聚合；`DllCanUnloadNow` 钉住「类工厂或 TIP 仍被引用时不可卸载、全部释放后可卸载」，并覆盖 `LockServer(TRUE/FALSE)`。生产的 `DllGetClassObject` 先清空输出，再按 CLSID、然后按接口判定，不把这两类错误混为一谈。

**安装布局与注册。** 32 位与 64 位 TSF DLL 分别装到 `{commonpf32|64}\metasequoiaime\msime_v<ver>\` 并带 `regserver` 标志注册 TIP，PDB 同目录；Server 装在 `{commonpf64}\metasequoiaime\server`；应用数据装到用户选定的 `DataDir`；HKLM `Software\Metasequoia\MetasequoiaIME` 写 `VersionDir` / `ServerPath` / `DataDir`；`THIRD_PARTY_NOTICES.txt` 与 `LICENSE.txt` 随包分发（GPLv3 第 4、6 条）。`ISCC /DLightPackage=1` 出不含词库的轻量包。

### Windows 发布流水线产出真实安装包（2026-09-23）

- 流水线：`.github/workflows/release-windows.yml` 在 windows-2025（MSVC，Visual Studio 18 2026）上按 `installer/Package-SimplySign.ps1` 的顺序走完 `Build-Client.ps1` → `Collect-Notices.ps1` → `Prepare-PackageFiles.ps1` → `Compile-Installer.ps1`，只是跳过签名。原生依赖按 `platforms/windows/vcpkg.json` 的 baseline 装进 x64/x86 两个前缀并缓存；Inno Setup 固定 6.7.1，`ChineseSimplified.isl` 取自同版本标签并校验 SHA-256。
- 产物：`MetasequoiaIME_Setup_v<版本>.exe` 与 `.sha256` 作为 workflow artifact 上传；`publish` 输入默认关闭，打开时才创建 `windows-v<版本>` Release。安装包未签名，因为代码签名证书是只在发布机上的 Certum SimplySign 卡，签名仍是本地步骤。
- 第一次在 MSVC 上完整构建暴露并修掉的问题：Engine overlay 脚本按 ANSI 代码页读写 UTF-8 源；engine-bridge 的 MSVC 编译拿不到 vcpkg 头文件；strict 目标的 `/W4 /WX` 窄化、遮蔽与 `getenv` 弃用告警；TSF 引入 Engine 管道契约时被 SDK 的 `max` 宏改写；PowerShell 调 pnpm（`.cmd`）时 Tauri `--config` 的内联 JSON 丢了引号；`Collect-Notices.ps1` 按整个文件比较 Engine 标记；安装脚本 `[Code]` 里有先用后声明的 `UserConfigPath` 和保留字 `Protected`。
- 证据：https://github.com/metasequoiaime/msime/actions/runs/35815935438 成功，artifact `msime-windows-0.1.0` 内含 201 MB 的 `MetasequoiaIME_Setup_v0.1.0.exe`，下载后 `.sha256` 校验通过。安装包尚未在真实 Windows 上安装验收。Rust crate 与 npm 包的补充声明已由 #650 收进通知集合，随后的发布运行 https://github.com/metasequoiaime/msime/actions/runs/35830590535（`publish=false`）成功。
macOS（`shared/apple/TextClient.mm`，iOS 共用）与 Linux 两套前端改成：视图带非空 `reading` 时，组字就是假名。**唯一的例外是用户把光标移进字母中间**——引擎给的偏移是罗马字里的偏移，没有到假名的映射（与 `MSIMEPreeditCaretPosition` 拒绝为双拼猜测是同一条理由），这时继续显示光标所属的那串字母，而不是把光标画在不属于它的地方；正常打字永远碰不到这条，光标一直在末尾。判据写成 `composition_shows_reading`（Linux 侧，带四条用例），Apple 侧在 `TextClientTest` 里四条（假名显示、raw 样式同样显示假名、光标移进中间保留字母、其余方案不受影响），反向验证过。

**Windows 同日也改了，而且比预计简单**：上一段里「TIP 自己按按键本地追加组字」的说法只对没有宿主引擎适配器的回退路径成立。挂着适配器时 `_HandleCompositionInputWorker` **本来就**把 `readingStrings` 整串换成视图里的 `preedit`，所以改的只是「换成哪一个字段」。TIP 的视图结构体补上 `reading`（解析器一行，用例钉住），显示判据直接用共享那条。

**这一改顺带把「就地结束组字」那条路也解决了**：TSF 的模型里组字本身就是文档里的文字，`_HandleComplete` 只是终止组字，所以组字显示假名之后，回车落进文档的也就是假名——不需要再碰结束组字的代码。

顺带把那条判据从 Linux 的头文件挪到 `shared/input/CompositionDisplay.h`：三个宿主问的是同一个问题，而它们用三种语言、三套工具包写成，能共享的就是这一行和它的理由。Linux 侧原来的名字保留（`using`），用例不动。

本机证据：Windows 交叉构建链接、i686 语法 264 个源文件、原生 79 个用例（含解析器那条，反向验证时红）；macOS 129/129；Linux 容器 20/20。仍然没有在 Windows 主机上安装运行——那一级证据本表从头到尾都没有过。

增量记录（2026-09-21，日语模式的回车在桌面宿主上提交的是罗马字）：顺着「哪些引擎命令没有任何宿主路由」这条线索查下去，`MSIME_COMMIT_READING`（命令 11）只有触摸宿主在用。查完发现这不是「多出来的能力没人用」，而是**桌面三家都把日语的回车做错了**。

事实先摆出来，用真实引擎实测（`crates/input-runtime/examples/japanese_conversion.rs`）：日语方案下打 `nihon`，`editing_text` 是罗马字 `nihon`，`reading` 是假名 `にほん`；`CommitRaw` 提交 `"nihon"`，`CommitReading` 提交 `にほん`。而 macOS、Linux 的 IBus 宿主、本仓 Windows 宿主的回车**对所有方案一律发 `MSIME_COMMIT_RAW`**——于是日语用户按回车得到的是罗马字。空格那一侧同样错：macOS 的空格发 `MSIME_COMMIT_CANDIDATE`，直接把第一条转换上屏，用户根本够不到第二条。

正确规则本仓自己就有，写在两个触摸宿主里（Android 的 `MSIMEInputService.enter`/`space`、iOS 的 `KeyboardViewController.handleReturn`）：空格是「変換」——第一次按开始转换、之后逐条步进；回车提交用户停在的那一条，没按过空格就提交假名。本批把这条规则搬到 macOS：新增 `handleJapaneseConversionKey:`，只在方案为日语、有组字、且是裸键时接管空格与回车，其余方案与带修饰键的组合一个字节都不变；编辑读音会作废正在进行的转换（与触摸宿主保留 `japaneseConversionEditingText` 是同一件事）。

用例八条（`ShortcutTest`）：没按空格时回车发 `COMMIT_READING`、第一次空格不提交也不发命令、第二次空格发 `NEXT_CANDIDATE`、之后回车按候选身份 select 到步进到的那一条、步过末尾回到第一条、改读音后回车又变回假名、非日语方案空格与回车维持原样、日语但没有候选时空格照旧。反向验证两处（整条路由短路、把 `COMMIT_READING` 换回 `COMMIT_RAW`）都红在同一条断言上。

**Linux 两套前端同日补上（同一批的后半）**：决策部分抽成 `platforms/linux/src/core/JapaneseConversion.h` 一个纯状态机（空格/回车各一个入口，返回「开始转换 / 步进 / 回到首条 / 提交某条 / 提交读音 / 不接管」），IBus 与 fcitx5 各自执行结果——两者别的地方差得远（一个路由 keysym 自己画候选表，一个把候选列表交给 fcitx 面板），共享的只有这个决定。提交候选时都按**屏幕上那一条的身份** select，与 IBus 空格原有的「渲染页围栏」同一个理由：实时视图可能已经比用户看到的快一代。容器门禁 20/20（新增 `linux-japanese-conversion`，七组断言含单条候选、改读音作废、无候选不接管）。

**本仓 Windows 宿主同日也改了，但只改得动的那一半。** 先说清它的形状：回车在 TSF 一侧就地结束组字，只是把「我提交了什么」作为观察值告诉 Server（`KeyEventSink.cpp` 那段 localCommitObservation），所以 Server 改不了已经进文档的文字——这一半确实要动 DLL。而**挂着宿主引擎适配器时走的是另一条路**：`HostRawCommit.h` 的 `CommitHostRaw` 向引擎要 `MSIME_COMMIT_RAW` 再把返回的文字插进去，那是引擎驱动的提交，改得动也测得了。

改法利用引擎自己的判据：`MSIME_COMMIT_READING` 只在「方案是日语且有组字」时才回答，其余一律返回空结果（`InputSession::CommitReading` 的第一行），所以 DLL 不需要知道方案——先问读音，答得上就提交假名，答不上就照旧问原文。写失败时不拿原文当第二次机会（那会在假名没进去之后把罗马字塞进文档）。

顺带把原生运行器扩到 TIP 自己的测试目录：`platforms/windows/tsf/tests` 里多数同样是「对着契约结构体的纯策略」，此前只因为运行器只看一个目录而留在「需要 Windows 构建」那一堆里。加上伴随源文件表里两条（TIP 的回复解析器）与 `-fdeclspec`（这些源文件是给 MSVC 写的，clang 加这个开关就认），本机原生通过数 **68 → 79**，其中就包括这条回车用例；反向验证时它确实红。

剩下的一半（DLL 就地结束组字那条路）仍未改：它要动 `platforms/windows/tsf` 的组字缓冲，且本机只有交叉构建与原生策略用例两级证据，跑不起来也无法交互验证。

增量记录（2026-09-21，设置项映射；**这一批有一半是重复劳动，更正写在末尾**）：来源把整个配置面写在一个文件里——`installer/default_config/config.default.toml`，17 个段 178 个键——这是两边现有材料里最接近「这个产品一共能被设定哪些事」的清单。本表此前按页、按控件比过好几轮，每轮都在重复同样两类假结果：**看着缺的其实是有意改名**（`y_mode` 就是 `local_modes.temporary_english`、`cn_en_mixed_input_min_chars` 就是 `mixed_input.minimum_prefix`），**看着有的其实只是某个无关标识符里恰好含同一个词**。

这一批把 178 个键逐个落到本仓的共享偏好或共享设置页上，结论是**全部有着落**：172 个对得上字段（多数是改名或改成嵌套结构），6 个明确没有目标并写明理由——4 个是来源自己的配置模板写了、它自己代码里一次都没读的死键（`enable_emoji`、`clean_mode`、`soft_keyboard.background_img`、`utility.study_english_word`，都只出现在 `config.toml` 里），1 个是来源 Server 在新旧两套会话实现之间切换的内部开关（`input.session_backend`，本仓只有一套运行时），1 个是词库目录（本仓按宿主运行时选项传，不是偏好）。

把这张表写成 `scripts/test-reference-config-coverage.py` 挂进 `--quick`，两个方向都查：本仓这边的字段被改名或删掉时，对应的来源设置就成了孤儿，门禁报出来；机器上存在来源检出时（`MSIME_REFERENCE_ROOT` 或主检出旁边的同级目录），来源模板里多出来的键不在表里也报出来——**上游新增一项设置，从此会在这里变成一条失败，而不是什么都不发生**。两个方向各反向验证过。

方法上记一条：这比「逐页看控件」强的地方不在于更仔细，而在于比较对象是一份**机器可读、上游自己维护的清单**，所以它能一直被验证；页面截图和控件清单只要有人改了措辞就失效。

**更正（同日稍晚）**：上面写「此前只按页比过」是错的——`scripts/test-windows-config-keys.py` 早就在做「来源有而本仓没有的配置键」这一问，而且比对的是来源默认分支的**当前 tip**（180 个键），比本批用的固定提交还新。我没有先看一眼 `scripts/` 下已有哪些对来源的检查就动手，于是两道门禁在同一个方向上重复，任何上游新增设置会同时红两次。

保留下来的是这一批独有的那部分：**每个来源设置对应本仓哪一个字段**，以及那个字段今天还在不在——`test-windows-config-keys.py` 比的是本仓 Windows 安装模板里的键名，答不了「共享层里是谁在答这条」。因此本批的脚本去掉了「上游多出来的键」那一半，文档里写明那一问归另一道门禁，两边不再重叠。教训写在这里而不是 commit 里：**动手加对照门禁之前，先把 `scripts/test-reference-*.py` 与 `test-windows-*.py` 列一遍**。

增量记录（2026-09-21，macOS 自动补全的那一对没有告诉引擎，于是第二对书名号变成了〈〉）：按「Linux 调了哪些 FFI 而 macOS 一次都没调」这条线索查下去，`msime_client_balance_paired_punctuation_after_auto_close` 是其中一个。查完是两个缺陷，来源在同一段代码里把两件事都做了。

**其一：书名号的嵌套计数没有回退。** 引擎按「当前开着几个《」决定下一个 `<` 给《还是〈（`punctuation_policy.cpp` 的 `book_title_nesting_`），正常情况下用户打的 `>` 会把计数减回去。宿主自己补右符号时那次 `>` 永远不会发生，于是计数只增不减——**第二次从头打《》会得到〈〉**。来源在 `KeyHandler.cpp` 补完右符号之后紧跟着调 `BalanceNestPairAfterAutoClose(wch)`，注释写的就是这句「否则下一个 《》 退化成 〈〉」；Linux 宿主也调；macOS 从来没调过。

用真实引擎把这条钉死在 `crates/input-runtime/examples/punctuation_table.rs` 里：`<` `<` `>` `>` 依次给出《〈〉》并把计数解开，而「打一个 `<` 之后宿主说自己补完了」时下一个 `<` 仍是《——不说就是〈。

**其二：成对模式下引号的左右交替。** 引号只有一个物理键，引擎靠一个 toggle 交替给出“与”。宿主补右引号时，那次「本该给”」的按键不会发生，toggle 停在「下一个是右引号」，于是**用户下一次打引号得到的是”**。来源同样在这里改写：成对模式下每一次按键都开一对新的（`punctuationStr.back() == L'”' → L'“'`）。macOS 照做。

落点都在 `apply:` 里补右符号那一段，开关关掉时两件事都不做（引擎自己的交替与嵌套正是没有成对补全时该有的行为）。`MSIMEClientSession` 新增 `balancePairedPunctuationAfterAutoClose:error:`（iOS 共用这个类，纯新增）。

用例接在既有的成对标点整链用例后面：（不触发回退、《与〈各触发一次且带的是 `'<'`、右引号被读成新一对的开始、关掉开关后三者都恢复原样。反向验证两处（去掉回退调用、让引号改写成为空操作）分别红在各自断言上。全量 macOS `ctest` 129/129，`--quick` 通过。

增量记录（2026-09-21，Linux 两套前端接上半截词的显示，并顺手接通了本机的 Linux 门禁）：第二十七批把「半截词留在组字里」做进共享运行时时只开了 macOS，理由写的是「Linux/Harmony/Android 本机既没有容器也没有工具链，盲改等于没验」。**这条理由这次不成立了**：OrbStack 本机就装着，只是守护进程没起；起来之后 `platforms/linux/build-container.sh` 整套能跑——IBus 引擎、fcitx5 插件、全部 provider 入口和单测，`verify-local.sh --quick` 里那两个一直显示 skipped 的 Linux 阶段现在真的在跑。

**门禁一接上就抓到一个既有的破坏：fcitx5 插件自 `1644def60`（同日的「迁入加加辅助码，第六套方案」）起根本编不过。** 那次把 `jiajia` 加进 `cycleHelpcodeSchema` 的方案表，却把 `std::array<const char *, 5>` 的 5 留在原地，六个初始化器塞进五个位置，`-Werror` 之外这本身就是硬错误。macOS 侧同一份表有六项且有用例，所以只有 fcitx5 这一侧断了，而本机当时跑不了 Linux 构建，于是它就这么躺着。改成 `std::array schemas = {...}` 让大小跟着列表走，同一个错误不会再犯第二次。

**半截词本身**：新增 `platforms/linux/src/core/PhrasePreedit.h`，把「已选的那一段拼在读音前面」连同光标换算做成一个纯函数。之所以不是一次字符串相加，是因为两套前端数光标的单位不同——fcitx5 的 `Text::setCursor` 要的是字节偏移，IBus 的 `update_preedit_text_with_mode` 要的是 Unicode 标量位置，而运行时给的 `caret_position` 是读音（ASCII）里的字节偏移，前缀却是汉字。两个都由这个函数算出来，谁也不用自己转换，而这次转换只被测一遍。

IBus 一侧另有一处顺带修正：非 raw 样式此前把 `text.size()`（字节数）当成光标位置传进去，对全 ASCII 的拼音串来说两者相等，一旦串里有非 ASCII 就不再相等；现在统一用标量计数。

两套前端同时开始请求 `phrase_preedit`（`scripts/test-phrase-preedit-hosts.py` 这道守卫正是要求「请求」与「绘制」成对出现，现在它把 Linux 记在「留在组字里」那一侧）。造词回退那两条规则（同日的另一批）因此在 Linux 上一并生效。

验证：容器门禁 19/19（新增 `linux-phrase-preedit`），`--quick` 通过且 Linux 两个阶段不再 skipped。反向验证两处：把标量计数换成字节数、去掉光标越界的夹取，各自红在对应断言上。仍未做的是真实 Linux 桌面上的目视验收——没有 GTK/Qt 编辑器、没有 X11/Wayland 焦点，这条边界和本表其他 Linux 记录一致。Harmony 与 Android 仍未打开，它们的工具链本机确实没有。

增量记录（2026-09-21，词库维护逐表字段核对，另修三处会让用户白丢词条的地方）：把「五笔/英文/快捷短语各表的字段」与「保留用户数据」两项走完。来源是 `server/src/settings/dictionary_manager.cpp` 三个表各自的 import/edit 路径，本仓是共享导入解析器 + `host-api/src/dictionary.rs` + 引擎的 `validate_personal_dictionary_entry`。

**三处修掉的，都是「用户按来源的习惯操作，本仓静默拒绝」：**

1. **编码里的大写字母**。引擎自己就折叠大小写（`validate_personal_dictionary_entry` 第一件事就是把 key 转小写），来源的设置页也在自己那一侧折叠；而本仓在更上面一层的 `validate_entry` 直接拒绝大写。于是快捷短语编码打成 `QQ` 得到的是「invalid dictionary entry」这句既不说字段也不说原因的错误，而同一个编码小写就没事。现在在同一个边界折叠。
2. **权重 0**。引擎的下限是 1（`weight < 1` 直接拒），而来源导入英文表时，没有第三列的行写的就是 0。本仓的导入把这些行整行丢掉并计成失败。现在统一抬到引擎下限：丢的是 1 的排序差，不丢词。
3. **两列顺序**。来源**同一个设置页**导出的四张表顺序并不一致——拼音与五笔是「词 TAB 编码」，英文与快捷短语是「编码 TAB 词」（`dictionary_manager.cpp` 导出分支逐条核过）。本仓把这件事做成一个全局的格式下拉（标准 TSV / Windows TSV），于是用户从来源导出五笔、在这边选「Windows TSV」，每一行都被拒。现在：按所选顺序一行都读不出来时，再按相反顺序读一遍，成功则在结果里带 `swapped` 并在卡片上说明「该文件的两列与所选格式相反，已按文件本身的顺序读取」——不静默改语义。两个下拉的文案也从按来源命名改成按**列序**命名（词在前 / 编码在前），因为「Windows」这个词恰恰不能告诉用户该选哪个。

**核对确认不是缺口的一项：保留用户数据。** 四张表的每一次编辑都经 `edit_personal_dictionary` 落进 `personal_journal.user_dictionary_operations`，带 `user_inserted=1`、`display` 列（英文表取词本身），删除写成 `operation='delete'`；换代时 `stage_dictionary_state` 把这张表重放进新一代。来源在设置页里手工调 `record_user_insert` / `record_upsert` 做同一件事。两边都保得住，且本仓这条路对四种类型一视同仁。

**记一处引擎侧的差异（~~本仓改不了~~ 已在 2026-09-21 做掉，见本表末尾那批；当时写的原因也是错的）：英文表的「词」与「显示」不能不同。** 来源的 `english_words(word,display,weight)` 是两列各自写入，`IsAsciiWord` 只管 word，允许连字符与撇号，因此 `dont` / `don't` 这种「编码是字母、显示带撇号」的行在来源能存。本仓走引擎的个人词库路径，而引擎要求 `lowercase(value) == key` 且 key 只能是字母，于是这类行会在引擎那一步被拒（导入时按行计失败并报出行号，不是静默）。当时写的是「改它要动 `engine-lock.json`，影响面覆盖全部平台，照本表既有规矩单独决定」——**这个理由不成立**：来源自己那份引擎里这条校验一字不差，没有更新的引擎可提；差别在来源的导入根本不走这条校验。已用第四个 overlay 做掉。

验证：新增两条共享解析器用例（0 权重留在下限、相反列序被读出且标记）、一条设置页用例（`swapped` 的说明文案）、并把 `host-api/examples/dictionary_requests.rs` 这条真实引擎往返扩成「`QQ` + 权重 0 存成 `qq` + 权重 1、按 `qq` 打得出来、再用大写那条原样删掉」。反向验证三处（去掉折叠、去掉权重下限、去掉解析器的下限）各自红在对应断言上。顺带重新生成了 HarmonyOS 的设置页打包产物——共享 UI 改了文案，那个 bundle 的门禁会因此报陈旧。

增量记录（2026-09-21，造词途中回退已选的那一段：来源两条规则，本仓一条都没有）：第二十七批把「半截词留在组字里」做进共享运行时之后，用户能看到已选的那一段，却没有任何办法把它退回来——选错了前半截，唯一的出路是 Esc 掉整个组字重打。来源有两条规则专门管这件事，都在 `server/src/ipc/input_key_policy.h`：

- `ShouldRetreatCreatingWordSelection`：读音只剩至多一个字符、光标在其末尾、有选词历史时，退格**不是**删那个字符（那会连带结束组字、把已选的段带走），而是把最新一次选词撤销——它吃掉的读音回到屏幕上，已选的词回到那次选词之前的样子，用户可以重新选。
- `ShouldDropCreatingWordSegment`：光标前面什么都没有时按 Ctrl+退格，删的是那一段选词本身，且**不还原它的读音**——用户要的是删掉这一段，不是重新拼它。

两条都落在共享运行时（本仓的 Server 对应物），因此打开 `phrase_preedit` 的宿主自动拥有它们，目前是 macOS。来源另外要求客户端协商过 `CompositionRestore` 且非 UILess，因为它的 TSF 侧要靠一条回复重建组字；本仓这个条件就是 `phrase_preedit` 本身——宿主只有会画 `view.phrase_prefix` 才会打开它，而视图正是每个宿主得知组字变了的唯一途径。

数据结构照抄来源的 `CompositionState::selection_history`：每次让已持有的词变长的选词压一条 `{选词前的整个词, 这次吃掉的读音}`。存**整个词**而不是这次新增的那一块，是因为吃掉零个读音的选词不记录（来源同样拒绝空的 `consumed_raw_input_with_cases`），它夹在两条能记录的中间时，只有整词能把它们一起放回去。「这次吃掉的读音」由前后两次读音求差得到：引擎从读音前面取走它用掉的部分，所以差就是消耗量；读音不是简单变短（特殊模式改写、回退到别的拼法）时记为不可回退而不是猜一个。

还原读音的手法与来源不同，结果相同：来源直接把拼音串塞回引擎（`set_pinyin_sequence` 后 `recompute_candidates`），本仓的引擎只能通过按键到达，于是取消组字再把那串读音重新「打」一遍。用户看到的是同一件事——那次选词吃掉的拼音，连同它的候选，光标在末尾。

用例四组（运行时单测，配一个能表达「选词从读音前面取走一段」和「光标不在末尾」的测试引擎，原有 `Fixture` 两样都表达不了）：回退后读音与词各自回到选词前、多次选词是一个栈只弹最新的一条、读音多于一个字符时退格仍是普通退格、没有选词历史时两条规则都不介入。反向验证两处：整条规则短路、以及还原成空词而不是选词前的词，分别红在各自断言上。

**真实引擎与真实词库上跑过**：`crates/input-runtime/examples/phrase_progress.rs` 里补了这两段——`haitanpaobu` 选「海滩」后退格，屏幕上回来的确实是 `haitan` 且候选里重新出现「海滩」；把光标移到最前再按 Ctrl+退格，「海滩」消失而后面打的 `paobu` 原样留着。这同时证明了消耗量求差在真实引擎上成立（引擎确实把剩余读音留成原读音的后缀），否则这条规则会悄无声息地永不生效。

增量记录（2026-09-21，另外两个联网 worker 的排队覆盖，以及让它们真的在本机跑起来）：云候选 worker 的排队用例补完之后，`AiCandidateWorker` 与 `TranslationWorker` 仍然一行覆盖都没有。这两个出问题的表现和云那个一样只在时间维度上可见——打完字之后旧的 AI 联想才冒出来、同一个前缀被付了两次模型调用、用户刚关掉的提供方仍然给出一行译文——别处都发现不了。

两个 worker 各加一个可注入的入口，默认仍是真实那条路：AI 侧是 `Fetcher`（与云 worker 同形），翻译侧是 `Translator`，接在 `translate()` 上，因为缓存与提供方路由都在它里面，再往下就只剩 HTTP 了。顺手把 `translate()` 的签名从 `const Request &` 改成 lease + query——`serial` 是队列的记账，对一次翻译没有意义，只有它自己的递归调用在传。

AI worker 七条：结果带对 lease/query/候选、去抖窗口内只付一次且付新的那次、**同一前缀第二次直接吃缓存**、**空结果不入缓存**（否则提供方抽风一分钟会让这个前缀从此再也拿不到 AI 候选）、在途被取代能看见自己被取代且结果不交付、提供方抛异常只被记录而不带走 worker 线程、五种非法信封一个都到不了提供方。翻译 worker 六条：同样的四条，外加 `clear_cache()`（偏好改到提供方不适用时触发）必须让在途的那一份也作废——它是在用户已经离开的设置下产出的——以及不可翻译的查询交付「没有」而不是一行空译文。

反向验证四处：把空结果也写进缓存、把两处取消判据都拿掉、让 `clear_cache()` 不推进 serial、把交付前的 `!cancel()` 去掉，分别红在各自的断言上。

**并且这三个用例现在每次都在本机真的跑。** 上一批的原生运行器按「一个翻译单元自己能编过」筛选，而 worker 的类体在各自的 `.cpp` 里，于是这三个一直落在「需要 Windows 构建」那一堆里，只有手工编译时跑过一次。给运行器加一张具名的伴随源文件表（只有这三个，且共享宿主库没构建出来时照常跳过），原生通过数从 65 升到 68。

增量记录（2026-09-21，Ctrl+Enter 的译义副候选页：macOS 此前没有）：来源在 Server 里给高亮候选的译义开一页「副候选框」——只有一条释义就直接上屏，多条则把每条释义摆成一页，用数字、空格、方向键挑，Esc 退回原来的候选列表。两套 Linux 前端都跟了这条。macOS 没有：Ctrl+Enter 落进「任何 Ctrl 组合先结束组字再把键还给应用」，按下去只是把正在打的字上了屏，译义一条也没给。

现在这条路由排在那条兜底规则前面：候选窗开着、译义开关打开、高亮候选确实有译义时才拦截，否则 Ctrl+Enter 仍按原样结束组字（用例里两种回落都验了）。副候选页复用引擎视图的形状，面板、皮肤和摆位都不用改；选中一条之后走 `MSIME_CANCEL` 而不是结束组字——译义才是用户要的那个词，结束组字会把中文候选再补到译义后面。Esc 只需把进页前存下的视图放回去，因为进页没有改引擎里的任何状态。

切分规则本身（半角 `;` 与全角 `；` 都算分隔符，两边空白去掉，空的丢掉）是字典定的，与宿主无关，但三个宿主各存了一份实现。新增 `platforms/macos/tests/candidate/TranslationSensesAgreementTest.cpp` 把 Windows、Linux、macOS 三份头文件编到一起，用 18 个样例问同样的问题并要求答案一致——三份同样的规则会悄悄漂移，而某个宿主少给一条释义，看上去像字典差异而不是 bug。其中两个样例是专门挑的：与全角分号共享首字节的「，」不能被切成半个字符，换行是宿主的分栏符、不是释义分隔符。（后续：三份实现已合并为 `shared/input/GlossSenses.h` 的 `msime::input::gloss_senses`，Windows 的 `translation_senses`、Linux 的 `split_translation_gloss`、macOS 的 `candidate_gloss_senses` 保留为转发，调用点不变；该测试改为逐例钉住期望结果，并同时检查三个转发。）

增量记录（2026-09-21，维护快捷键已齐，另修关于页日志开关的名字）：来源《服务守护》一节的四组维护快捷键在 macOS 上都在，也都有宿主用例：`Control+Shift+Option` 加 1–8 删除候选（八个槽位逐个验过，含重复按键抑制）、加 C 清缓存、加 R 重新注册并退出、加 T 退出进程。来源写的是 `Ctrl+Shift+Alt`，macOS 换成 Option 是平台适配，共享设置页里那句说明也按平台改过。

顺带修掉关于页的一处：诊断日志开关在 macOS 上显示为「Server 端日志」，描述写着「排查 Server 通信……记录慢请求阶段、候选窗、悬浮工具栏、菜单、焦点会话和通信状态」。macOS 没有 Server 进程，输入法就是一个进程；它实际记的是焦点进出与偏好加载/应用/保存的结果（`msime_macos_diagnostic_write` 的六个调用点），落在应用支持目录的 `diagnostic.log`。Linux 早就各自命名为「IBus 宿主日志」并写了自己的说明，macOS 一直沿用 Windows 的那份。现改名为「输入法日志」，描述按它真正记录的内容与文件位置重写——来源那句「复现后可直接发送该文件」保留，因为这才是这个开关存在的理由。

增量记录（2026-09-21，云候选与 AI 联想的插入位置：不用联网也能验）：来源把位置写死了——云联想「返回了当前本地不存在的结果，会插入在首页的第二项」，AI 联想「不与本地以及云联想重复，会插入首页第三项」。位置是引擎定的，没有真实候选列表就看不出来，而联网这一块此前只有契约测试（请求怎么发、凭据怎么路由），没有「插进来之后长什么样」。

新增 `crates/input-runtime/examples/online_slots.rs`：不联网，直接把结果交给运行时的 `apply_online_candidate`——真实 provider 的回调走的就是这个入口。实测 `nihao` 的本地候选是 `你好 / 你好吗 / 你好啊 / 拟好`，注入之后变成 `你好 / 合成云候选 / 合成 AI 候选 / 你好吗`：云在第二、AI 在第三、本地首选不动。另两条：AI 单独到达时仍落第三而不是往上挤（来源是给座位编号，不是压缩排列）；云端返回的结果与本地已有的重复时不会插成两条。

增量记录（2026-09-21，标点表与「切换提示」：两条都不是缺口）：

**标点表。** 来源《标点与以词定字》最后一条列了六个不是「ASCII 键的中文孪生」的映射：`\` → 、、反引号 → ·、`Shift+6` → ……、`Shift+-` → ——、`Shift+,` / `Shift+.` → 《 》。它们来自引擎的表而不是任何宿主，所以此前两侧都没人验：共享用例用合成引擎，宿主用例不出宿主。新增 `crates/input-runtime/examples/punctuation_table.rs`，拿真实词库逐个打，六条全中；关掉中文标点后没有一个再产出中文标点（引擎此时不接管这个键，交给宿主原样输入，所以断言的是「不产出中文标点」而不是「产出 ASCII」——这是层次问题，第一版写错过一次）。

**切换中英文时显示提示。** 这一项来源没有（`ui-html` 里搜不到对应控件），是本仓在 macOS 上的增项（输入模式 HUD，能力位 `input_mode_hud` 只给 macOS 与 HarmonyOS）。属超集，不需要迁移。

至此来源《核心功能指南》各节都有对应的可重跑验证：输入方案/辅助码/日语（`schemes_dictionary`、`helpcode_dictionary`）、调频（`frequency_modes_dictionary`）、以词定字（`word_to_character_dictionary`）、混输与 emoji/颜文字顺序（`mixed_*_dictionary`）、八种快捷模式（`local_modes`）、标点表（本条）、智能标点（共享用例 + 默认值修复）、成对补全（#3380）、语音快捷键（`voice-hold-shortcut`）、词库导入导出与自定义释义（#3350）。剩下的是网络类（云候选、AI 联想、在线翻译）与真实编辑器里的手感。

增量记录（2026-09-21，成对标点自动补全：按第一条路实现，#3380）：上一条记的三条路里选了第一条，并把 Apple 来源缺的那段簿记补上了。落地形态与来源不同，因为平台不同：

- 左符号照常作为**提交文本**落进文档（引擎给的就是它），所以它任何时候都不会丢。
- 右符号作为**标记文本**跟在光标后面：`（|）`。组合开始后它继续留在标记文本的尾部——`（ni|）`、`（你好|）`——所以整段组合是在这对符号里面进行的。
- 上屏时右符号跟着提交文本一起落下（`你好）`），光标落到右符号之后，并把它记进 `PairedPunctuationTracker`，于是紧接着再打一次右符号会被跳过而不是打出两个。
- 组合被拆掉的每一处（失焦、`commitComposition:`、Esc、宿主重置）都先把欠的右符号补上，所以「打开了一对但没打完」不会把右符号吞掉；切到另一个客户端时则是丢弃而不是写进新文档。
- Excel 仍按来源的排除名单跳过。

与 Apple 来源的区别写在这里：它把整对符号一起作为标记文本送出（`setMarkedText:` 配 `(1, 0)` 的选择区），而标记文本会被下一次 `setMarkedText:` 替换——用户接着打字时那对括号会被顶掉。本仓把左符号先提交掉，只让右符号留在标记区，因此最坏情况是多一个或少一个右符号，不会丢掉用户已经落下的字。

用例：共享层 `MSIMEApplyTransitionWithPendingClosing` 四条（组合期尾随、上屏时带走、无待补时行为不变、行内预编辑为空时仍显示右符号），宿主级一条走完整条链（开对→组合→上屏→再开一对→拆掉时补上→关掉开关后恢复原样）。全量 `ctest` 126/126。

仍未验证的：真实编辑器里的观感。标记文本在不同应用里的呈现（下划线、选区）不完全一致，这一条要在编辑器里看。

增量记录（2026-09-21，成对标点自动补全：macOS 上整个没有发生，待定实现形态）：接着验标点那几条，这一条比前面几处都大。

**事实。** 引擎不做自动补全：用真实词库实测，`paired_punctuation` 打开、中文标点打开，输入 `(` 提交的只有 `（`（`[` → `【`、`"` → `“`、`<` → `《` 同样），随后按 `)` 才提交 `）`。补右符号是宿主的活：Windows 的 TSF 宿主自己做（`KeyHandler.cpp` 的 `punctuationStr.push_back(pairedClosing)` 之后整串插入），Linux 宿主也自己做（`ClientEngine.cpp` 的 `normalize_punctuation_pair` 把右符号接在提交文本后）。**macOS 两件事都没做**：`InputController.mm` 里唯一与配对相关的写入路径，要求提交文本长度 ≥2 且首尾成对才把右符号记进 `PairedPunctuationTracker`，而引擎永远只提交一个字符，所以那段永远不执行，跟踪器里也永远是空的——它的另一半（再按一次右符号时跳过而不是打出两个）因此也是死的。

于是 macOS 上：开关默认开、设置页写着「输入左侧符号时自动补全右侧符号，并将光标置于中间」、连 Excel 的排除名单（与来源同一个）都在，唯独功能不存在。

**为什么不能照抄 Windows。** Windows 插入两个字符后把光标左移一格（`moveCursorSync(CURSOR_LEFT)`）。IMK 没有对应的 API：输入法不能移动宿主应用的插入点。Apple 来源（MSIME-Apple）给出的是这个平台自己的答案，注释写得很清楚——把整对字符作为 **marked text** 送出，选择区落在中间（`setMarkedText:@"（）" selectionRange:NSMakeRange(1, 0)`），因为「IMK 没有 setSelectedRange，带插入点的标记文本是受支持且不抢焦点的表达方式」。

**没有定下来的地方。** 标记文本会被下一次 `setMarkedText:`/`insertText:` 替换。也就是说照抄这套之后，用户打完 `（` 再打 `nihao` 时，宿主为新组合送出的标记文本会不会把那对括号一起顶掉——取决于宿主接到下一个键时先提交还是先改写标记区，而这一点只能在真实编辑器里看。没有在真实编辑器里验证之前，不往这条会改写用户文档的路径上落代码。

**三条路，待定夺：**

1. 照搬 Apple 来源的标记文本方案，并把「下一个键先提交已标记的那一对、再开始新组合」这段补齐（本仓宿主需要新增这段簿记，Apple 那份没有）。功能与来源等价，需要真实编辑器验收。
2. 只插入两个字符（提交而非标记），光标落在右符号之后。功能上补齐了「自动补全」，但「光标置于中间」这半条不成立，描述要跟着改。
3. 保持现状，把 macOS 上这个开关的描述改成实话，或在这个平台隐藏它。

一并记下：`PairedPunctuationTracker` 与它的 `paired-punctuation` CTest 是为「再按一次右符号时跳过」写的，形态没问题，只是在补全落地之前没有输入。

增量记录（2026-09-21，智能标点：开关默认开、描述照抄来源、行为却不发生）：接着按指南往下验标点那几条时，发现的是一处默认值造成的「开关说了不算」。

来源的设置页在智能标点这一族上只有**两个**开关（智能标点、重复标点转中文），且出厂都是开；单独一个「智能标点」就意味着它自己的描述——「中文标点模式下，字母或数字后的 , . : 自动使用英文标点」。本仓把它拆成了三个：父开关之外另有「数字后直出」「字母后直出」两个细分项（还有一个来源没有的「中文标点后按空格转换」）。父开关在除 Windows 外的所有宿主上默认开，两个细分项默认关，而 `punctuation::route` 要求 `direct_digit || direct_letter` 才走 ASCII。

于是出厂状态下：智能标点是开的，它下面那句照抄来源的描述是**假的**，打 `v1,` 得到的是 `v1，`。用户得再找到另外两个开关才能得到描述里写的行为。

改法是让两个细分项跟随父开关的默认（`smart_punctuation_default()`），Windows 一侧完全不变（父开关本来就默认关，细分项跟着关），已存下的文档也不受影响（serde 默认只对缺键生效）。「按空格转换」不跟随：它重写的是用户已经看见落下的字符，来源也没有对应物，保持默认关。

`scripts/test-default-config-parity.py` 里那条「三个细分项必须在所有宿主上默认关」的断言按同一理由改写：它真正要守的是「Windows 新用户拿到整族关闭」，跟随父开关同样满足，而且现在守的是跟随关系本身。原有两条偏好用例与一条 host-api 用例一并更新——其中 host-api 那条此前是用出厂默认去断言「不转换」，现在改成显式关掉两个细分项再断言，意思更清楚。

增量记录（2026-09-21，八种快捷模式：拿真实词库把指南里的例子逐条打一遍）：到这一层为止，比对的都是「页面上写了什么」。八种快捷模式各自有偏好、有菜单项、有宿主用例，但没有一处检查「按下 Shift+T 再打 rq，屏幕上到底出什么」——宿主用例用的是空的资源文件（它要验的那道门只问文件在不在），共享用例用的是合成引擎。指南里最具体的那部分，恰恰是没人验的那部分。

新增 `crates/input-runtime/examples/local_modes.rs`，按来源《实用功能快捷模式》表里的例子逐条打，读回候选。它要已验证的词库发布，所以与旁边的 `runtime_dictionary` 一样是 example 而不是 CTest：

```sh
cargo run -p msime-input-runtime --example local_modes -- <verified-dictionary-directory>
```

本机实测（词库缓存在 `~/msime-shared/resources/<hash>/`）全部通过，抽样为证：`T` + `rq` → `2026年9月21日`；`U` + `4e00` → `一`，`+1f600` → `😀`；`E` + `xiaolian` → `😀`；`M` + `haixiu` → `(*/ω＼*)`；`J` + `nh` 的候选里有 `你好`（排在 女孩、你会 之后，与第八批记录的引擎排序一致）；`R` + `nihon` → `日本 / にほん`；`Y` + `hello` 空格上屏 `hello` 且模式自动退回中文，`R` 同理。`T` 的三种拼法（rq/riqi/date、sj/shijian/time、xq/xingqi/week）各自都验了，指南列三种就是说它们等价。

`K` 只验了「进入模式并吃掉编码字母」这一半——另一半要用户词库里先有短语，属于有状态的验收。

增量记录（2026-09-21，页内顺序与描述文案这两层）：顺序的两条断言（输入页、外观页）此前也只渲染 Windows 宿主，现按同一张宿主表对两个宿主各跑一遍。结果是齐的——macOS 的两页顺序与来源一致，没有缺口。

描述文案（每个控件下面那句 `<small>`）逐条比过来源的 51 条。本仓写的是忠实的节略版，规则都在：`U 模式` 那句「空格上屏；Shift+数字选词」、`J 模式` 的「双拼按当前方案转换声母」、`Y 模式` 的「上屏后回到中文」等都与来源一致。一处措辞差异是刻意的且更准：剪贴板管理，来源写「关闭后立即清空」，本仓写「保存关闭设置后清空」——本仓清空发生在偏好保存时，照抄会写出一句不成立的话。

顺带记一件事：`U 模式` 那句描述里的「Shift+数字选词」在 #3348 之前 macOS 其实做不到——页面写着、宿主没有。这类「描述先行、实现落后」的情况，比对文案本身是抓不出来的，得从描述反推去验实现。

增量记录（2026-09-21，再往下一层：控件里的选项，对 macOS 也钉一遍）：小节钉住不等于选项一致——上一条「每页候选项数量」就是在选项这一层出的问题。`referenceOptions` 原来只有六个控件、也只渲染 Windows 宿主，现在扩到二十一个控件并对两个宿主各跑一遍（42 条）。

查到两处真差异，都是标签，都已改：

- **润色方案的四个预设名**。本仓写的是 清理口语 / 忠实原文 / 中译英 / 自然口语，来源是 精炼整理 / 忠实校对 / 中翻英 / 口语整理。提示词本身当初是逐字从 `voice_providers.cpp` 抄过来的，名字就在同一张表里（`PolishPromptPreset`），却没跟着抄。
- **候选窗口主题的「跟随」**。同一页另外七个主题选择器和来源都写「跟随全局」，只有这一个写「跟随」。

顺带修掉 macOS 原生语音设置窗的一处：提示词方案那个下拉把**原始 id**（`cleanup`、`zh2en`…）直接当菜单项显示，而且保存的就是这个标题字符串。现在 id 与显示名分开，显示的是来源那四个名字，存的仍是 id。

这两处能漂掉是因为原本没有任何用例钉住它们——改完之后 837 条用例照样全绿，正说明这一点。新增的那 42 条现在钉住了。

增量记录（2026-09-21，设置页小节的那张表，补上 macOS 与来源剩下六页）：`referenceSections` 这张表把来源设置窗每一页的小节钉在渲染结果上，但它只渲染 **Windows** 宿主，而且只覆盖八页。对本次迁移来说这两点都不够：一个藏在 macOS 未声明的能力位后面、或藏在平台名分支后面的小节，在这张表下照样通过。

两件事一起做：

1. **同一张表再对 macOS 宿主跑一遍**，宿主能力按 `host_surface.rs::for_platform(Macos)` 逐字段给全（此前 Windows 那一份只给了六个，快捷键页的两节其实藏在 `mode_switch_shortcuts` / `panel_shortcuts` 后面——补上之后 Windows 这侧也多钉住了两节）。macOS 少掉的小节必须在 `macosAbsentSections` 里写明理由，而且条目失效也会失败。跑出来只有三条，都是平台适配而不是缺口：Windows 字体行上的「保存后自动应用」（macOS 换字体不需要重启）、「打开手写识别板」（手写面板要输入法进程的 IMK 会话，设置窗是另一个进程，macOS 改为提示从工具栏或输入法菜单打开）、以及帮助页的「快速上手 / 基本功能」（macOS 用 `macosHelpCards` 的术语—说明行回答同样三个问题）。
2. **把来源剩下的六页补进表**：语音输入、皮肤、词库、AI 辅助、快捷键、关于。补的过程中确认了几处形态差异都不是缺口——来源四套内置皮肤各自成节，这里是一个卡片选择器（四套皮肤本身都在）；来源「批量导入纯汉字词组」与「导出词库」两节，这里合成带类别选择的「本地词库管理」，导入说明里写明支持纯汉字自动注音；来源「TSF 端日志」是 Windows TIP 进程的，按平台门控。

顺带放弃了一版做法：先写了一个把来源 `section-title` 抽成清单、再在 `index.tsx` 文本里查找的脚本。它对这个页面不成立——大量标题是 `{label}辅助码方案` 这类表达式渲染的，纯文本查找必然误报。改用已有的渲染断言是对的，这一版没有提交。

增量记录（2026-09-21，出厂默认值逐项比对：四处首次启动就不一样的，待定夺）：把来源的出厂配置 `installer/default_config/config.default.toml`（150 个键）与共享 `Preferences::default()` 逐键比了一遍。除去该文件里的占位值（`FAKESECRET_…` 的三个 token、腾讯的 `<YOUR_…>`、示例提示词、以及作者自己的 `settings_page.theme = "dark"`）之外，剩下**四处是真的默认行为差异**，且本仓 Windows 模板与来源一致、其余宿主（含 macOS）走共享默认：

| 键 | 来源 / 本仓 Windows | 共享默认（macOS 等） |
| --- | --- | --- |
| `input.default_ime_mode` | `english` | `chinese`（2026-09-24 起 Windows 的共享默认也是 `english`） |
| `voice_input.mute_system_audio` | `true` | `false` |
| `voice_input.doubao_enable_ddc` | `true` | `false` |
| `voice_input.polish_text` | `true` | `false` |

第一条是用户装完立刻能感觉到的：来源那边装好后输入法起手是英文态，macOS 起手是中文态（`AppearancePreferences.defaultImeMode` 在没有存储值时回落到 `chinese`）。

**本轮不动它们**，理由是这不是「macOS 漏实现了什么」，四个字段每一个 macOS 都在消费；改的是默认值，而共享默认同时被 Linux、Android、iOS、HarmonyOS 使用，翻一处就翻五个宿主的首次启动行为。另外起手是中文还是英文，在 macOS 上与 Windows 的处境并不同——macOS 上用户是明确从输入菜单选中「水杉输入法」才开始打字的，选了中文输入法却起手英文，未必是这个平台想要的。这属于产品取舍，留给所有者定。

比对方法可重跑：`Preferences::default()` 用一个临时 example 序列化成 JSON，再按叶子名与来源 TOML 逐键对；不匹配的叶子名（来源有而共享没有同名字段的 80 个）多是命名差异，已在上一条记录里核过。

增量记录（2026-09-21，几条查过、判为「不是缺口」的，连同判据）：这一轮把来源设置窗写配置的 73 个键、宿主能力矩阵、以及 macOS 侧所有收窄共享取值的归一化函数逐个过了一遍，结果除了上面那条每页候选项数量之外没有别的缺口。判据记在这里，免得下一轮重查：

- **设置键映射**：来源 `settings_app.cpp` 里 `path == "…"` 的 73 个键逐个在共享偏好或共享设置页里找到对应物，差异全是命名（`paging_brackets` → `navigation.brackets`、`cn_en_mixed_input` → `mixed_input.english`、`utility.*_mode` → `local_modes.*`、`word_to_character` → `word_character`、`*_helpcode_schema` → `quanpin_helpcode.schema` 等）。`input.wubi_schema` 与 `input.japanese_schema` 在来源那边各自只有一个选项（86 五笔、罗马字），不需要共享字段。
- **宿主能力矩阵**：`HostCapabilities::for_platform` 里没有任何一项是「Windows 有、macOS 没有」。
- **悬浮工具栏缩放**：macOS 的白名单 75/100/125/150 与来源下拉逐项相同。
- **翻页键的缺省**：原生窗口那个三选一的「候选翻页快捷键」只在共享 `navigation` 没有显式布尔值时充当缺省，一旦设置页写过就以共享值为准；它推出的缺省（minus_equal 开、brackets 关、其余开、mouse_wheel 关）与共享 `NavigationPreferences::default()` 逐项相同。
- **候选字号**：`metasequoia::mac` 下那两个只认 16/18/20 的 `NormalizeCandidateFontSize` 属保留的 Apple 适配层（`MetasequoiaInputController.mm`，不编进产物）；发出去的宿主走的是 12–32。

混输最小前缀默认值的来源依据：

- **混输最小前缀的默认值**：来源固定对象 `467b9804`（包含 `1cf27e2f`）的 `installer/default_config/config.default.toml` 给出 5；门禁通过 `reference_source.py` 直接读取这个固定对象核对。

快捷短语编码与来源一致，只能包含英文字母（来源 `dictionary_manager` 的 `valid_code`），导入时先转小写，`NH` 存为 `nh`。新增、编辑和导入里带数字的编码一律拒绝：设置页提示「快捷短语编码只能包含英文字母，长度 1 到 32。」，文本导入不收这一行，计入失败数并列在错误明细里（来源 `ImportQuickPhrase` 同样走 `append_error`，跳过数只留给重复行），个人词库 JSON 文件整份拒绝，设置页在预览时就指出是第几条。检查集中在写入新内容的几层（`quick_phrase_code_is_well_formed`，由 client-core 导入解析、`PersonalWord::validate_new`、host-api 的 `replacement_for_engine` 与 iOS 逐条校验新词条所用的 `msime_client_dictionary_validate`、账户写入前的 `validate_new_dictionary_value`、设置页个人词库 JSON 的预览校验）；读取已存条目、编辑或删除时对原条目的核对、云端拉回的条目仍用宽松检查。因此已经存有数字编码的条目照常出现在列表里，照常导出和同步，但在 K 模式下打不出来：引擎在 K 模式吞掉一切非字母按键，`quick_phrase_query` 也只认 a-z。这类条目可以删除，或改成纯字母编码后保存；只改权重也要先改编码。重新导入导出的文件时，这些行计入失败数。

增量记录（2026-09-21，每页候选项数量：macOS 把共享默认值改写掉了）：来源外观页的「每页候选项数量」提供 3–9，默认 6；共享偏好 `candidate_page_size` 接受 1–9，默认也是 6。macOS 这一侧是 `NormalizeCandidatePageSize`：只认 5、7、9，**其余一律改写成 9**。于是一个谁都没动过的设置，在这个平台上显示并保存为 9，而别的平台是 6；从别处写下的配置（另一个宿主、手改、云端同步回来的外观快照）带着 4 或 6 进来也会被静默改掉。云外观校验器 `MSIMECloudAppearanceCandidatePageSize` 同样只接受这三个值，一份别的宿主写的快照会被整条拒绝。

这三个值来自 Apple 来源那个窗口，是本仓早先对齐它时引入的（#ecaf6a069「align macos candidate page sizes」），不是 macOS 的平台约束——面板画几行就是几行。按当前目标（复刻 MSIME-Windows）改回来：宿主接受共享偏好的整个 1–9 并把越界值拉到最近一端而不是顶到 9，未设置读作共享默认的 6，原生窗口与共享设置页都列出来源的 3–9，云快照按同一范围校验。（2026-09-23 订正：这句当时只对 macOS 原生窗口成立，共享设置页实际对所有宿主列出 1–9；现在共享设置页也只列 3–9，文档里存着 1 或 2 时把这个值留在列表里，不改写。）

顺带发现 `CandidatePageSizeTest.cpp` 根本没注册进 CMake——有文件、没目标，从来没跑过，这正是那条改写规则一直没被重新审视的原因。已接进 CTest（126 项），并把它从「5/7/9 的三值表」改成覆盖整个范围、越界拉到最近一端、以及窗口列出的那七项。

增量记录（2026-09-21，README 其余各节：自定义候选窗翻译在桌面端没有入口）：来源《自定义候选窗翻译》一节的做法是「在 `%LOCALAPPDATA%\metasequoiaime\` 下新建 `custom_translations.txt`」。这条指令迁到 macOS 就不成立了——同一个目录在 `~/Library/Application Support/` 下，Finder 默认不显示它，用户按文档照做会找不到地方。

共享设置页本来就有这一节（输入页的「自定义候选释义」，连 Tab 分隔、`#` 注释、同源词以最后一次为准这些说明都在），但它由 `client.customTranslations` 门控，而这个能力只有 HarmonyOS 提供。于是桌面三个宿主上整节不渲染：既没有来源那条文件路径的等价物，也没有界面。

按「公共功能+UI 放 Tauri」补上宿主这一端：两个命令读写 `<state>/user/custom_translations.txt`（引擎读的就是这个路径），语义与 HarmonyOS 那份一致——上限 1 MiB、清空即删除文件（留一个空文件会让引擎每次会话都读出一个空集合）、文件不存在时读作空文档。另加两条 macOS 上必要的细节：读取时剥掉 UTF-8 BOM（来源明确接受带 BOM 的文件，留着会在页面上显示出来又原样存回去），以及写入先落到同目录的临时文件再改名，让一次失败的保存留下上一份覆盖层而不是半份。

用例：Rust 侧两条（往返 + BOM + 清空删除 + 超限/NUL 拒绝且拒绝后旧覆盖层还在），UI 侧一条钉住桌面宿主也能拿到这一节。

增量记录（2026-09-21，快捷模式指南逐条核对，第一条差异：Unicode 模式的候选选择）：测试清单走完之后换入口——来源 README 的《实用功能快捷模式》八行，每行都是可核对的具体约定。Unicode（U）那行写的是「空格上屏首选；`Shift + 数字` 选其他候选」，理由就在同一行里：不加 Shift 的数字是正在输入的码位。来源实现为 `event_listener.cpp` 的 `is_unicode_shift_digit_selection`，与空格走同一条选择路径。

macOS 缺后半条。`ShouldRoutePhysicalCandidateDigit` 明确把 Unicode 模式和任何带修饰键的数字都排除在候选选择之外，而没有第二条规则把 Shift+数字 接回来——于是这个模式下键盘只能上屏第一个候选，面板里其余候选看得见、够不着（方向键还能挪高亮，但文档写的那条交互不存在）。

按来源补上，写成与既有那条并列的纯函数：候选面板可见、处于 Unicode 组合、且修饰键恰好只有 Shift。键位映射沿用既有的 1–9（不含 0），与来源的 `'1'..'9'` 相同。四条纯函数用例加一条宿主级用例（真的发一个 Shift+2 事件，断言选中第二个候选，同时不加 Shift 的 `2` 仍作为十六进制输入到达引擎）。反向验证过：去掉新分支，宿主那条在第 4496 行失败。

增量记录（2026-09-21，来源测试清单这一轮走完）：43 个文件按落点分完，macOS 侧这一轮到此为止。三处真实差异已各自修掉（方向键末尾不扩充、字体 face 名不解析、空槽位提示词），其余的判断依据记在这里，免得下一轮重查。

**对 macOS 不成立的（Windows 进程边界或 WebView2 自绘）**：`test_inline_protocol`（设置壳的资源内联）、`test_candidate_window_template`（WebView2 候选模板的锚点替换，macOS 是原生面板）、`test_candidate_size_estimator`（Direct2D 度量，对应物是 `CandidateRowFit.h`）、`test_ipc_protocol_constants`、`test_pipe_write_policy`、`test_terminal_deactivation_policy`、`test_active_client_state`、`test_outbound_session_state`、`test_async_request_origin`。

**归 Engine 或共享层、两边跑的是同一份的**：`test_quanpin_scheme`、`test_shuangpin_query`、`test_shuangpin_scheme`、`test_engine_shuangpin_session`、`test_wubi`、`test_japanese_romaji`、`test_jianpin_query`、`test_emoji_query`、`test_kaomoji_query`、`test_kaomoji_sql`、`test_date_time_query`、`test_english_dictionary`、`test_user_dictionary_journal`、`test_translation_gloss`、`test_custom_translation`。

**查过、无缺口的**：候选固定位置的独立配色、悬浮工具栏可见性三条件、翻页键六组开关、混输英文的最小前缀（两边都是 2，且都由偏好校验钉在 1–8）、剪贴板历史的 50 条上限与去重后置顶、词库分页的有界扫描与 `has_more`。

**一处实现不同、结果等价，实测过**：简繁输出。来源用 OpenCC 的 `s2t.json`（随包带 `assets/opencc`），macOS 用 ICU 的 `CFStringTransform(Simplified-Traditional)`，不带数据文件。ICU 这条一向被认为弱在词组上下文，所以按公认的难例实测了一遍：皇后→皇后、头发→頭髮、干面→乾麵、里面→裡面、面条→麵條、后天→後天、发展→發展、周杰伦→周傑倫，八条全对。结论是平台适配而不是缺口，不引入 OpenCC 与它的数据文件。

**记一处刻意保留的差异**：偏好文件的升级路径。来源 `test_config_template_merge` 钉的是三方合并——用户改过的键保留、仍停在旧默认值的键跟随新默认值、模板里没有的键和段落丢弃。本仓的 `preferences.json` 每次都把全部字段写出来，没有「旧默认值」这个概念，所以改默认值永远到不了已有用户；`deny_unknown_fields` 又让退役字段不能删（`ui_backend` 的注释已经写明这一点）。这是存储模型层面的取舍，不是某个功能的缺口，改动面也远超一次功能迁移，单独提出来由所有者决定，本轮不动。

增量记录（2026-09-21，来源测试清单第三批：语音润色的提示词槽位）：来源 `test_voice_providers.cpp` 的三条里，`voice_providers_empty_custom_falls_back_to_cleanup` 钉的是「自定义槽位留空时用精炼整理那份内置提示词」——它断言留空的自定义二拿到的文本含「整理助手」。本仓 `shared/voice/PolishPrompt.h` 在这一条上漂了：自定义一和自定义三留空回落到 `kCleanupPrompt`，自定义二留空回落到 `kFaithfulPrompt`。于是一个用户从没填过内容的槽位，模型收到的是「校对」而不是「精炼整理」，与另外两个空槽位的行为也不一致。

这个头文件是共享的：Windows 宿主 `platforms/windows/src/system/PolishPrompt.h` 只是把它 include 进来，macOS 经 `HTTPVoiceRequest.mm` 用的也是它，所以这一处同时影响两个桌面宿主。

漂移能发生是因为原有用例只断言了「不是 legacy 那份」和「非空」，没说是哪一份内置。现按来源改为 `kCleanupPrompt`，并把三个空槽位都钉到「与完全未配置时拿到的那份相同」。反向验证过：改回去，用例在第 56 行失败。

顺带把这个测试挪到它测的代码旁边（`shared/voice/tests/polish_prompt.cpp`），并在 macOS 的 CTest 里注册。它此前只在 Windows 构建里跑，而两个桌面宿主都发这份提示词——这正是漂移没人发现的原因。

增量记录（2026-09-21，来源测试清单第二批：字体族解析）：来源 `test_system_font_family.cpp` 钉住两条——下拉里列的是 DirectWrite 的族名且有序，以及**旧配置里存的 face 名要解析成 CSS 能匹配的族名**（`仓耳今楷05 W03` → `仓耳今楷05`）。第一条 macOS 已满足：`system_fonts::list` 走 `CTFontManagerCopyAvailableFontFamilyNames`，装进 `BTreeSet` 天然有序。第二条此前是空的：`resolve_css_families` 只在 Windows 上查别名，其余平台原样返回，注释写的是「其他宿主本来就只有族名」。

对 macOS 这条不成立，理由和 Windows 完全一样：偏好里的字体名不只来自下拉——旧版本写下的文件、迁移过来的配置、手改的 JSON 都算数，而 CoreText 认 PostScript 名（`PingFangSC-Semibold`）、CSS 不认。于是候选窗（走 `NSFont`）显示的是用户选的那个字体，紧挨着的设置页预览（走 CSS）悄悄回退成别的——只有预览是错的，两边还对不上。

按 CoreText 补上，形态与 Windows 那份对应但不照搬：CoreText 查不到名字时不报错，而是返回一个替代字体，所以只有「拿回来的就是问的那一个」时才采用答案——PostScript 名、全名、或者问的本来就是族名，三者之一匹配才算数，否则保留原值不动。实测：`Helvetica-Bold` → `Helvetica`，`PingFangSC-Semibold` → `PingFang SC`，`HiraginoSans-W3` 与 `Hiragino Sans W3` 都 → `Hiragino Sans`，`Synthetic W03` 原样返回。用例另钉住顺序与条数——调用方是按位置把答案配回候选字体/英文字体/回落字体三个字段的，少一条或换个顺序就会把一个字体的族名安到另一个设置上。

增量记录（2026-09-21，来源测试清单这条轴转向 macOS，第一批：候选导航）：前几批按来源的设置页、文案、配色比，这一批换一个入口——来源 `server/tests/src/` 的 43 个测试。它们是来源自己用断言钉住的行为，而「两边都有、行为不同」这一类恰恰只有它们抓得住。这一批走候选窗那一组（`test_candidate_ui_state`、`test_candidate_ui_owner`、`test_candidate_size_estimator`、`test_candidate_view_model`、`test_floating_toolbar_visibility_policy`）。

查到一处真实差异并修掉：**用方向键走到已加载候选的末尾时，引擎扣住的那批候选放不出来。**

来源 `server/src/ipc/event_listener.cpp` 的 `move_selection` 在两种情况下先调 `expand_initial_candidates()` 再移动——选中项已在最后一个候选上，或选中项在当前页尾且下一页是短尾页。本仓的共享运行时只在 `Action::NextPage` 上扩充（`expand_for_next_page`），`Action::NextCandidate` 直接把高亮夹在 `cached.candidates.len() - 1`。引擎对单字母查询有二十四条的初始上限，于是同一个查询下按 Page Down 能走到词库深处，按方向键（以及任何一次一条地走的宿主路径）走到第二十四条就停住，再按没有反应。macOS 的方向键走的正是这条路（`InputController.mm` 把 ↑/↓ 发成 `MSIME_PREVIOUS_CANDIDATE`/`MSIME_NEXT_CANDIDATE`，即 host-api 的 102/103），Linux 与 HarmonyOS 同理。

改在共享层（`crates/input-runtime`），两个触发条件按来源逐条对应，扩充后的重排复用原有路径（`rerank` + `demote_runner_up_readings`），四条用例钉住：走到末尾能取到扣住的候选、踏进短尾页前先填满（与翻页一侧同形，页面不会先短一下再长出来）、引擎没有存货时高亮停在最后一条不回绕、向上走永远不请求扩充（否则会在用户往回读的时候重排列表）。

同组其余四项没有缺口，一并记下判据：候选窗的固定位置项本仓已按来源的 `#379AD3` 单独着色（`InputController.mm` 的 `candidateFixed`，与来源 `candidate_view_model.h` 同值），不与高亮合并；悬浮工具栏可见性 `configured_enabled && !fullscreen && ime_active` 与来源 `floating_toolbar_visibility_policy.h` 逐项相同（`MetasequoiaFloatingToolbarShouldShow`），`ime_active` 也与来源同义地跟随输入法的选中而不是客户端焦点：`activateServer:` 置位，用户切到别的输入源时由 `MSIMEInputSourceMonitor` 的 `switchedAway` 调 `deactivateForInputSourceSwitch` 清除（对应来源 TIP Deactivate 发出的 `WM_IMEDEACTIVATE`）；客户端失焦（IMK `deactivateServer:`）对应来源的 `ClientSuspended`，不改变工具栏可见性，所以切换应用、点桌面或没有输入框的窗口时工具栏留在原处，属主控制器被释放时立即隐藏（`dealloc` 调 `deactivateForDelegate:`），每次应用激活时 `refreshVisibility` 再检查一次属主作为兜底；`candidate_size_estimator` 是 Direct2D 的度量工具，macOS 侧由 `CandidateRowFit.h` 和原生面板用例覆盖，属实现形态差异；翻页键的六组开关（minus/equal、逗号句号、方括号、Tab、PageUp/Down、方向键）macOS 全部消费；开关关闭时，Tab/Shift+Tab、PageUp/PageDown 与 ↑/↓ 在候选可见期间照样被吞掉，不发任何命令、不结束组字，候选原样保留——来源在任何候选模式下都把这几个键交给 Server（`CompositionProcessorEngine.cpp` 的 `FUNCTION_SERVER_CANDIDATE_KEY`），Server 以 `IsCandidateNavigationKey` 认出后默认回 `NavigationIgnored`，只有对应的 `GetConfigured*Enabled()` 为真才翻页或移动（`server/src/ipc/event_listener.cpp`）；候选不可见时这些键仍交还应用。另有 Home/End 落在当前页首尾。（**这句在 2026-09-21 的第二十五批被推翻**：来源不是没有 Home/End，而是在客户端一侧把它们分类成 `FUNCTION_MOVE_PAGE_TOP/BOTTOM`，选的是整份列表的首末项。见该批。）

方向键的朝向是刻意的平台适配：来源只认 ↑/↓，macOS 按候选窗朝向决定（竖排认 ↑/↓，横排认 ←/→），这是既有决定，不动。

增量记录（2026-09-20，视觉层）：文字各层比完，转到最影响「看起来一不一样」的东西——配色与度量。结论是这一层**本来就是精确移植**：

- 配色。来源 `styles/variables.css` 的深色 64 个、浅色 63 个变量，与本仓 `styles.css` 里两个主题块逐值相同，唯一差异是一条长阴影在本仓换了行、渲染值一致。
- 侧边栏。来源 `sidebar.css` 的 26 处像素度量逐条对过：宽 200px、纵向内边距 12px、item 的 6px/12px 内边距与 2px/8px 外边距、gap 15px、圆角 5px、指示条 4×22px 且 999px 胶囊圆角、图标 24px 容器配 22px 图、头部 25px 图标与 19px 标题、`contain: layout style paint`、`scrollbar-gutter: stable`，全部一致（本仓写作 Tailwind：`w-50`、`py-3`、`gap-[15px]`、`rounded-[5px]`、`before:rounded-full`、`translate-y-px` 等）。
- 卡片与分节。`.section` 的 18px 下边距、20px 24px 内边距、1px 边框、8px 圆角、卡片背景与阴影，`.section-title` 的 15px/550，`.section-header` 的两端对齐 gap 10px，`.divider` 的 0.5px，全部一致；本仓另加了来源不需要的窄屏适配。

也就是说用户最初说的「UI 完全不一样」，不在配色和度量，而在本轮前十三片修掉的那些：占位应用图标、侧边栏顺序与两对重复图标、各页 section 的顺序与叫法、选项与描述文案。

这一层此前同样没有任何检查。现把来源的 `variables.css` 按 `packages/ui/src/upstream/` 既有做法引入为 `settings-variables.css`，并加 `scripts/test-settings-palette-parity.py` 逐值比对、接入 `verify-local.sh`。本仓用 Tailwind 重新声明同一批名字而不是引用那张表，所以两份副本会一个色号一个色号地漂移而无人察觉——正是该有检查的形状。

写守卫时先做错一版：用 accent 的值本身当区分两个主题块的锚点，于是改 accent（最可能被改的那一个）会让锚点消失、抛异常栈而不是报出差异。已改用 `color-scheme` 作锚点，并在找不到锚点时给出说明。改一位十六进制验证：现在报「dark: --accent-color is #8e8cd9 here and #8e8cd8 in the source palette」。

增量记录（2026-09-20，按钮与操作项）：标题、选项、描述之后比最后一类可见元素——可点的操作。来源各 partial 的按钮逐个在本仓查过，只有两个没有同名对应物，且都不是缺口：`批量导入` 与 `新增短语`。来源把它们分在两处（dict 页的纯汉字批量导入、tools 页的快捷短语编辑器），本仓合成一个「本地词库管理」，以全拼/五笔/英文/快捷短语为类别，查询、新增、编辑、导入、导出、删除俱全，描述里写明导入支持「标准、Windows TSV、Rime 和纯汉字自动注音」——正是来源那个批量导入做的事。不把本仓的「导入」改名成「批量导入」：它同时兼管另外三种格式，改名反而说窄了。

顺带扫了反方向的问题——本仓有没有在某个宿主上显示却点不动的按钮。共享设置页一份、六个宿主，宿主能做什么以可选回调的形式到达；一个按钮若不检查回调是否存在，就会在做不到该功能的宿主上照常渲染、按下去什么也不发生。死按钮比没有按钮更糟，它宣称功能存在。扫下来全部已门控（`disabled={!client.x}`、可选链，或外层渲染条件），包括处理得很细的一处：外部皮肤的「打开目录」在沙箱宿主上由 `importsSkin` 改写成它实际做的事，而不是许诺一个打不开的文件夹。

这条不变量此前只靠逐个按钮写得仔细来维持，没有任何检查。现加 `scripts/test-settings-action-guard.py` 静态守卫并接入 `verify-local.sh`。

写这个守卫时自己先写错一版：正则只匹配 `client.X(` 这种调用形式，而面板那几个按钮写的是 `onClick={() => void openPanel(client.openScreenKeyboard)}`——把回调传进辅助函数而不是调用。我删掉一个 `disabled` 去验证，守卫却仍然通过，才发现这一版是摆设。已放宽为匹配 `client.X` 的出现（调用与传参都算），重新验证：删掉门控会报 `index.tsx:7541`，还原后通过。

增量记录（2026-09-20，开关描述文案）：标题和选项都钉住之后，比最后一层可见文字——每个开关下面那句 `<small>` 描述。逐条比对前先在代码里核实来源的说法对本仓是否成立，成立的才采用。

采用三条，每条的断言都验过：

- 智能标点。来源写「中文标点模式下，字母或数字后的 , . : 自动使用英文标点」，本仓原文是「根据输入上下文选择中文或英文标点形式」——说了等于没说。`SmartPunctuationRepeatPolicy.chineseMark` 处理的正是 `, . :`，`isAsciiAlphanumeric` 对应「字母或数字后」，来源的说法准确，采用。
- 重复标点转中文。来源写「智能标点输出英文标点后，2 秒内再次输入同一标点时替换为中文标点」，本仓原文是「短时间重复输入 ASCII 标点时转换为中文标点」，既没说 2 秒也没说前提。`SmartPunctuationRepeatPolicy.WINDOW_MS` 就是 2000，`KeyboardSession` 里那串前置条件也正是「智能标点已输出英文标点」的情形，采用。
- 成对标点自动补全。来源写「输入左侧符号时自动补全右侧符号，并将光标置于中间」，本仓原文是「自动补全成对引号和括号」，漏了光标行为。补全路径里确有 `moveCursorSync(CURSOR_LEFT)`，采用。

**不采用**一条：候选词翻译。来源写「仅在竖排候选窗口中显示中文与所选语种的互译，每项最多两个简短释义」，这两个限制对本仓都不成立——本仓支持同时显示两种语言（另有「候选词翻译第二种语言」一项），且 HarmonyOS 触屏的候选行是横排、翻译就渲染在那里。照抄会把不成立的限制写进界面。

改完撞到一个和先前「云候选」同类的问题：新描述里含「智能标点」四字，于是 `getByRole("checkbox", { name: /智能标点/ })` 同时命中「智能标点」和「重复标点转中文」两个开关，已改为锚定的 `/^智能标点/`。这类正则在描述文案变化时会失效，值得注意。

未验证：这三行没有在设备上目视确认——模拟器滚动粒度较粗，几次都跨过了标点那几节。改动由 343 条测试与 bundle 漂移门覆盖，HAP 安装后页面渲染正常。

增量记录（2026-09-20，下拉选项的文案）：section 标题已由 `referenceSections` 钉住，这一轮下沉一层比选项本身。来源三处与本仓不同（取值一致、只是标签）：「候选项排列方式」来源是 横向/纵向 且横向在前，本仓是 竖排/横排；「候选窗预编辑」来源是 拼音分词/不显示，本仓是 显示拼音/隐藏；「中英文状态」来源是 按应用记忆/全局统一，本仓是 按应用/全局。第二处同时是仓内不一致——紧挨着的「行内预编辑」对同一组 `pinyin`/`empty` 用的就是「拼音分词/不显示」。三处均已改用来源的说法，并新增 `referenceOptions` 表把这五个控件的选项逐项钉住。设备确认：外观页尾部现在显示 纵向 与 拼音分词。

同一轮把来源另外两个集中策略头文件核完，均无可修项：

`server/src/window/floating_toolbar_visibility_policy.h` 两条。`ShouldShowFloatingToolbar(configured_enabled, fullscreen, ime_active)` 本仓的 Windows 宿主已经实现（`platforms/windows/src/system/server_main.cpp`），连来源自己的用例也已移植（`platforms/windows/tests/core/fullscreen_foreground.cpp`）。HarmonyOS 覆盖了其中两项——工具栏只在 `desktop && toolbarEnabled()` 时创建，且活在键盘扩展里，我们不是当前输入法时它根本不存在。缺的 `fullscreen` 一项**没有可用信号**：输入法扩展只能拿到 `display.getDefaultDisplaySync()` 的尺寸与密度，前台窗口是否全屏属于窗管的特权查询，STATUS_BAR 面板在全屏下的去留由系统决定。这是平台能力差异，不是实现缺口。另一条 `ShouldDeferFloatingToolbarHide` 是 WebView2 首帧宽限期，无对应物。

`server/src/window/ui_backend_policy.h` 是在 Direct2D 原生渲染与 WebView2 之间按 surface 选择后端，即来源外观页那一项「界面渲染」。HarmonyOS 用 ArkTS 原生渲染，没有第二套后端，无对应物——与第二片记录的「界面渲染不引入」一致。

增量记录（2026-09-20，`input_key_policy.h` 逐条走完）：不再抽查点位，把来源 `server/src/ipc/input_key_policy.h` 里那八条 `constexpr` 当作契约整体核对。结果：

- `IsEnglishModeToggleKey`（Ctrl+Shift+E）、`WordToCharacterDirection`（无修饰键的 `-`/`=` 或 `[`/`]`）—— 上一片已确认相符。（2026-09-23 更正：这是就 HarmonyOS 说的；Windows 宿主当时并不相符。TSF 把 Ctrl+Shift+E 按 `FUNCTION_CANCEL` 在本地取消组字后照常把键发给 Server，Server 的 `translate_key` 却把它当普通 Ctrl 组合放行，英文模式根本不切换。现在 `ReplyComposer::basic_key` 先认 `is_english_mode_toggle_key`，无条件调 `ServerSession::toggle_dedicated_english`（取消组字再翻转 `msime_client_set_english_mode`，对应来源 `SetEnglishInputMode` + `ClearState`），按 Escape 同款的 `LocalCancel` 不回包；带 Alt 的组合不算。回归在 `tests/input/dedicated_english.cpp` 与 `tests/input/input_key_policy.cpp`。）
- `NormalizeNumpadDigitKey` —— Harmony 的 `HardwareKeyRouter.normalizeNumpad` 同样把小键盘 0–9 归一成主键盘数字，并在 `route` 入口只做一次（来源在 Server 边界做一次），且多填了缺失的字符。
- `ShouldLearnEnteredEnglishWord` —— `engine-bridge` 的 `commit_raw_with_policy` 里是 `before.dedicated_english || local_special_mode || (chinese_scheme && !complete_pure_pinyin)`，与来源逐项相同。
- `IsBackendIndependentCompositionResetKey`（Shift/Esc）与 `ShouldResetCompositionForImeMode` —— 这两条是 Windows 分体架构下 Server 与 TSF 的**同步**约定（TSF 已在本地取消组字，Server 必须跟着重置后端），HarmonyOS 的键盘扩展自己拥有组字，没有对应物。其用户可见效果「离开中文模式会清掉正在拼的字」在本仓由引擎的 `set_dedicated_english_mode` → `reset_composition()` 覆盖，已在上一片钉住。来源的 Shift 切换同样受 `ReadConfiguredSwitchLanguageHotkeys().shift` 门控，与本仓一致。
- `ShouldSendCompositionReply`、`InputSessionMatchesConfig` —— 都是 Windows IPC 回包协议的内部规则，非分体架构没有对应物。

来源自己有一份该策略的测试 `server/tests/src/test_input_key_policy.cpp`。把其中以词定字那组用例移植到本仓的路由测试上，补齐了此前没覆盖的三类：偏好错配（设为 minus_equal 时按方括号必须无效，反之亦然）、功能关闭、以及任一修饰键按下时都不生效（来源写作 `(modifiers & kKeyModifierMask) != 0`，本仓等价于 Ctrl/Alt/Meta 在更上面被 RELEASE、Shift 在分支内排除）。已把错配那一条的守卫放松验证过测试确实会红。

移植时自己先错了一次：夹具把 minus/equals 的 keycode 写成 2041/2042，实际是 2057/2058，于是「minus/equal 在其为配置项时生效」那条失败——是夹具写错不是代码问题。

增量记录（2026-09-20，硬件键盘的按键归属）：沿上一片往下核了四处，**全部相符**，结论记在此处以免再查：

- 以词定字的修饰键。来源 `WordToCharacterDirection`（`server/src/ipc/input_key_policy.h`）要求不带任何修饰键，`(modifiers & kKeyModifierMask) != 0` 直接返回 0。Harmony 的对应分支只显式写了 `!key.shiftKey`，看着像漏了 Ctrl/Alt，实际 `HardwareKeyRouter` 在更上面就有 `if (key.ctrlKey || key.altKey || key.logoKey) return RELEASE`，带修饰键的组合根本到不了那里，等价。
- 模式切换快捷键。Harmony 的 `mode_switch_shortcuts` 为真，实现不在 `HardwareKeyRouter` 而在 `InputModeRouting`，由 `KeyboardExtensionAbility` 在按键进引擎之前先行消费。
- `Ctrl+Shift+E`（仅指 HarmonyOS；Windows 宿主的 Server 侧 2026-09-23 才接上，见上一片的更正）。来源 `IsEnglishModeToggleKey` 绑的英文模式切换，`InputModeRouting` 已实现，套件里也有「Ctrl+Shift+E switches the composing language」。该模块头注释写明 `Ctrl+Shift+E`、`Ctrl+Shift+Space`（全半角）、`Ctrl+.`（标点集）三条都按 Windows 基线固定实现。
- 切换语言是否清空组字。来源是 `SetEnglishInputMode` 紧跟 `ClearState`；本仓经 `msime_client_set_english_mode` → `runtime.set_dedicated_english` → 引擎 `InputSession::set_dedicated_english_mode`，后者在标志真正翻转时调 `reset_composition()`，行为一致。

最后这条此前没有任何测试钉着：`input-runtime` 测试桩的 `set_dedicated_english` 用的是 trait 的空默认实现，所以该行为成立仅仅因为真实引擎恰好会重置。考虑到本仓引擎比来源新 467 个提交，这正是该钉住的一类风险。现让测试桩如实建模（模式真正改变时清空组字，重复设置同一模式不动），并加测试断言切换语言后组字消失、重复设置不误清。已把重置去掉验证过它确实会红（`left: "a"`, `right: ""`）。

增量记录（2026-09-20，组字期标点的上屏时机）：来源在组字进行中遇到标点时，先用高亮候选结束组字、再输出该标点——`IsCommitWithHighlightedCandidatePunctuationInCandidateMode`（`server/src/ipc/event_listener.cpp`）列出的是 `` ` ! @ # $ % ^ & * ( ) [ ] ; : \ " , < . > ? ' ``，并排除三类：`-`/`=`/Tab 永不触发，`,`/`.` 与 `[`/`]` 在被配成翻页键时也不触发。（来源 `1d2431ad` 之后表里另有 `-`、`+`、`/`：主键区的 `-`/`=` 仍按键码排除，小键盘的 `+`/`-`/`.`/`/` 与主键区 `/` 则用高亮候选结束组字后原样接上 ASCII 字符，见下文 2026-09-23 的记录。）共享运行时的 `punctuation()` 行为与之一致（先 `engine.finish(self.highlighted)` 再翻译标点），注释里也写明了原因。

差的是 HarmonyOS 的硬件键盘路由。`HardwareKeyRouter` 的标点分支写的是 `!composing && chinese && !japanese && isAsciiPunctuation(...)`，只在**没有组字**时把标点交给引擎；组字进行中则落到 `return RELEASE`，把键还给应用。于是在 2in1 上敲 `nihao` 再按 `!`，组字仍开着而 `!` 被插进编辑器里、排在还没上屏的拼音前面；触屏路径不受影响，它直接调 `KeyboardSession.punctuation()` 走运行时。现去掉 `!composing` 这一条：标点无论是否在组字中都归键盘所有，组字中的那次由运行时按来源的规则结束组字。翻页键不受影响——它们在更上面的 `composing` 分支里就被消费掉了，且按 keyCode 匹配（逗号是 2043），日语标点仍归应用。

写这条测试时自己先踩了一次：夹具用 `keyCode: 0` 配 `unicodeChar: ','` 去验「逗号仍然翻页」，而导航是按 keyCode 匹配的，于是逗号没被认成翻页键、落到了标点分支——是夹具写错，不是代码问题，已改为用真实 keyCode 并断言 `PREVIOUS_PAGE`。

增量记录（2026-09-20，把逐页核对固化成可执行的检查）：前四片的页面对照都是靠读两边的源码得出的，而两边的朴素搜索都会失真——来源用 `class="section-title ai-heading"` 这类组合类名，本仓大量标题由 `{label}` 表达式渲染，于是同一批结论被反复重新推导，我自己在本轮里就误判过「实用功能少了八种模式」「皮肤页没有外部皮肤」。现把已核实的对应关系写成 `referenceSections` 表并配一个 `test.each`，覆盖外观、输入、辅助码、实用功能、悬浮工具栏、屏幕键盘、手写识别板、帮助八页：任何一节被删掉或改名，这里直接失败。已用「把候选项排列方式改名」验证过它确实会红。

写这个表时发现两类先前没注意的门控，都不是缺失：其一，好几节挂在宿主能力后面（`candidate_font_controls`、`candidate_follow_cursor`、`ime_mode_scope`、`floating_toolbar_appearance`），用裸 fixture 断言会把「宿主没声明」误读成「界面没有」，故 fixture 按 `host_surface.rs` 给 Windows 的那组能力来写；其二，`候选窗主字体` 在本仓的 Windows 宿主上是被 `{!windows && …}` 有意隐藏的——Windows 显示的是「候选窗英文字体 + 补充字体」，那行还带 Windows 专属的「保存后自动应用」说明，而来源显示的是「主字体 + 中文补充字体」。这是 Windows 字体路径上的既有取舍，不属于 HarmonyOS 的迁移范围，表里以注释记录而不断言。

同一轮还核过几处怀疑、结论都是已实现或有意适配，一并记下以免再查：HarmonyOS 的 `InputCommand` 枚举值与 C ABI 的命令码逐一对应（分段命令是显式的 12/13/14，不是顺延的 9/10/11）；翻页键集合覆盖来源的 `IsPagingKey` 全部并多出鼠标滚轮；日语浊音/半浊音/小假名在触屏上以 `SURFACE_VARIANTS` 面板实现（点假名直接按下对应罗马字笔画），而不是来源的 `CycleKanaVariant` 循环命令。

增量记录（2026-09-20，输入行为层第一片）：先确认了一件决定工作量的事——两个仓库用的是同一个引擎 `github.com/metasequoiaime/msime-engine`，来源以 submodule 锁在 `6bd22549`，本仓以 `engine-lock.json` 锁在 `0531d421`，而后者比前者**新 467 个提交**（来源那个 commit 是本仓的祖先）。所以候选生成、分词、词库这些引擎行为不存在「缺失」，本仓跑的是同一引擎的更新版本；行为差异只可能出在宿主怎么驱动它。本仓的候选快照字段（candidates / candidate_codes / candidate_annotations / candidate_sources / candidate_positions / candidate_corrected）也与来源 `CandidateViewItem` 的 text/annotation/badge/translation/fixed_position 一一对应，并多一个纠错标记。

据此查到一处真实差异并修掉：引擎会把一部分候选扣在初始结果之后，要调 `expand_initial_candidates` 才放出来，而运行时里这个调用只有一处——`expand_for_next_page`，只在 `Action::NextPage` 时触发。翻页的宿主能拿到，改为「一次列出全部」的触屏宿主则永远拿不到。`all_candidates()` 是 `&self`，只读 `self.cached`。实测（`withholding_runtime(12, 8, 5)`）：翻页前 `all_candidates()` 返回 12 条，翻页到底后返回 20 条——号称「全部」的面板少了 8 条，正好是引擎扣住的那批。HarmonyOS 的触屏路径用的就是 `allCandidates`（`KeyboardSession.ets`，注释写明沿用 iOS 的做法放弃翻页），所以这 8 条在触屏上无法通过任何操作到达。现让 `all_candidates()` 先扩充再返回：这个调用本身就是「把全部给我」。扩充失败不致命，调用方仍拿到已有的那一代。

未验证：设备上的候选条数没有亲眼确认。改动在 Rust 侧，已用 `MSIME_OHOS_DEPS` 复用既有前缀重新编出 `libmsimeclient.so` 并打包安装成功，但要看到展开列表需要先在系统设置里把输入法设为当前输入法，这条 GUI 路径本次没有走通。证据是运行时里实测的 12→20。

增量记录（2026-09-20，候选窗与悬浮工具栏面板对照）：设置窗逐页走完后，转到打字时出现的两个面板本身。

候选窗。来源的呈现规则在 `server/src/window/candidate_view_model.h`：一项由 text + annotation + badge 串接，翻译另起 `cand-translation`，而固定位置的项会被单独套上 `color:#379AD3`。本仓 `KeyboardView.ets` 的候选行把固定位置和高亮合并成同一个 `candidateAccentColor()`，于是在任何「强调色即选中色」的皮肤上，已固定的候选和当前选中的候选看起来完全一样——而固定位置这个功能的意义就在那个标记。现按来源给固定位置项独立配色，规则收进 `CandidateSkinPolicy.rowTextColor`，四条断言钉住四种组合。

候选管理菜单不改。来源的桌面右键菜单是 置顶 / 固定排位→第 1–5 位 + 取消固定 / 删除；本仓是 优先显示 / 第 1–5 位 / 取消固定 / 删除词条…，把悬停子菜单摊平（触屏上没有悬停），措辞则与 iOS、Android 一致。`platforms/android/README.md` 写明这套顺序与措辞对齐的是 Apple 来源的长按菜单，三个触屏平台共用。HarmonyOS 是触屏平台，改成 Windows 的说法会破坏三端一致并推翻既有决定。「删除词条…」的省略号也有意义：本仓这一项 `confirmationRequired` 为真，来源那条不是。

悬浮工具栏已齐。来源设置页里的 6 个组件 id 与标签（character_set / emoji / fullwidth / punctuation / screen_keyboard / settings）与本仓 `floatingToolbarComponents` 完全一致，加上「中英文切换」这个始终显示项；本仓多一个 `english_mode` 属扩展。缩放、图标尺寸、组件三节的标题也与来源同名同序。

组件**默认值**有一处刻意不跟：`emoji` 在共享层默认关（与 `handwriting`、`voice`、`screen_keyboard` 一起），来源默认开。id、标签和顺序不变，只是新配置拿到的是精简工具栏。Windows 不受影响——`platforms/windows/installer/config.default.toml` 逐项显式写出每个组件（`floating_toolbar_emoji = true`），镜像来源自己的默认配置，共享默认值轮不到生效；变的是不带出厂配置的宿主（macOS、HarmonyOS）。理由与升级代价见下面《工具栏可选按钮改为默认关》。

工具栏的外框同样对齐来源 `floating_toolbar_presenter.cpp`。左缘对应 `ToolbarDragHandle` 的是水杉 logo：来源那条 2.5×14、`#8E8CD8` 的圆角竖条在 macOS 上不再单独绘制，工具栏最前面已经有 logo，再放一条竖条只是第二个同义的记号，拖动交给 logo 承担。logo 占 34pt 加 4pt 间距，其后隔 3pt 是对应 `ToolbarDivider` 的 1.2pt 分隔线（取全局主题解析出的候选窗描边色 `border`），再隔 4pt 才是第一个按钮；按钮宽为字号加 8pt、彼此相隔 2pt，字形取字号的 0.95，尾部留 6pt，都随缩放比例放大。这比来源更紧凑：按来源间距排开时字与字相隔过远，工具栏挡住的正文比五个按钮实际需要的多。来源 `WM_NCHITTEST` 把柄所在区域报成 `HTCAPTION`，macOS 侧由 logo 的 `mouseDownCanMoveWindow` 配合面板的 `movableByWindowBackground` 实现同样的拖动，按钮一律返回 NO，按住按钮不会拖走面板；按钮全部关闭时分隔线隐藏、logo 保留，与来源始终保留柄一致。来源柄上的 `IDC_SIZEALL` 光标按平台惯例换成 `NSCursor.openHandCursor`，macOS 没有公开的四向箭头光标。按钮的悬停与按下底色对应 `ToolbarIconButton::Render`：半径 `max(2, 高度×0.25)` 的圆角矩形，填色取全局主题解析出的候选行悬停色 `hover`（跟随系统时深色白 0.10、浅色黑 0.06），与工具栏底色、描边同出一套主题；面板从不成为 key window，所以悬停由 `NSTrackingActiveAlways` 的跟踪区报告，面板隐藏时一并清除悬停态。

未验证：固定候选的配色只有单测覆盖，没有在设备上目视确认——需要启用输入法、聚焦文本框、输入、长按候选、选固定这一串操作，本次没有完成。

增量记录（2026-09-20，其余各页逐页核对）：把来源剩下的 13 个 partial 与本仓对应页逐项比了一遍。结论是绝大多数已经齐备，先前几次「缺口」判断多半是提取方法的问题——来源用 `class="section-title ai-heading"` 这类组合类名，本仓大量标题由 `{label}` 表达式渲染，两边的朴素正则都会漏。实用功能页的 K/T/U/E/M/J/Y/R 八种模式（`localModeRows`）、皮肤页的「外部皮肤」（在 `skin/external-skins.tsx`）、快捷键页的「简繁切换」、语音页的「录音时静音其他声音」都确实存在且与来源同序；关于页的「Server 端日志」在 Linux 上改称「IBus 宿主日志」，「TSF 端日志」已按 Windows 门控，都是正确的适配。

真正的差异只剩两处措辞，已改：辅助码页两个方案块共用同一句「在候选窗口显示辅助码」，两个 checkbox 的无障碍名完全相同，来源是分别点名的「在候选窗口中显示双拼辅助码 / 全拼辅助码」，现按来源各自命名（移动端保留「候选栏」的说法）；语音页「录音时静音其他音频」改为来源的「录音时静音其他声音」。

本仓「启用 AI 辅助」没有改成来源的「启用 AI 联想」：来源那个开关只管第 3 个候选，本仓这个还管 iOS 键盘 AI 回复与 Android 选中文字润色（描述里写明），改名会把范围说窄。

顺带修了 develop 上先前就存在、与本片无关的 5 个失败用例（在干净 `origin/develop` 上复现过）：

- 豆包识别的测试按钮被 `harmonyPlatform && provider === "doubao"` 收窄，Windows 与 macOS 上不再出现，而这两个宿主的用例一直覆盖它；豆包属于共享探测，已并入 provider 列表。对应测试的期望原本 `...ASR_PROVIDER_DEFAULTS.doubao` 整个铺开，把载荷从不发送的 `documentation` 也断言了进去，改为显式列字段。
- 屏幕键盘用例 `Caps Lock and Shift invert letters` 同步断言按键投递，而投递是排队异步的，读到的是上一个按键；已按同文件另一条用例的写法 `await act`。同一条里 `getByRole("button", { name: "Ctrl" })` 不唯一（左右各一个），改为取第一个。该用例原先还期望 Caps Lock 会把键面变成大写——与来源相反：`ui/demos/msimeui-keyboard-demo/KeyboardPanel.cpp` 第 196 行的键面只取 `shiftActive_`，第 313 行送出的字符才用 `capsActive_ != shiftActive_`。本仓行为本来就与来源一致，是这条期望写反了，已按来源改正。

增量记录（2026-09-20，输入页与来源对齐）：第三片，比对来源 `partials/input.html` 的 29 项。

顺序：来源是 输入模式 → 输入方案 → 双拼/五笔/日语方案 → 翻页方式 → 候选词翻译 → 在线翻译服务 → 以词定字 → 标点若干 → 中英混输 → emoji/颜文字混输 → 默认中英文 → 中英文状态 → 简繁输入 → 云候选 → 拼音方案调频。本仓此前把「默认输入状态」「中英文状态范围」放在页首，把翻译相关放在标点之后，把中英混输排到接近页尾，与来源出入较大。现按来源重排，本客户端独有项贴着同类放：全拼纠错/模糊音/学习选词习惯跟在「以词定字」后，全角输入跟在「中文标点」后，中文标点后按空格转换/数字后直出/字母后直出跟在「重复标点转中文」后，标点锁定跟在「成对标点自动补全」后，中英文切换提示/显示英文释义/英文建议跟在「中英混输」后，手写输入、Android 手写输入、高情商回复与按键反馈分别留在首尾。

叫法：6 处改用来源的说法——候选翻译→候选词翻译、成对标点→成对标点自动补全、默认输入状态→默认中英文、中英文状态范围→中英文状态、繁体中文输出→简繁输入、云联想→云候选。最后一条同时消掉了本仓内部的不一致：emoji/颜文字那两项的描述本来就写作「云候选」。

两处刻意不改，都会把界面写错：来源的「始终使用英文标点」与本仓的「中文标点」绑的是同一个 `chinese_punctuation`，但极性相反，只改名不反转控件即是错标；来源的「在线翻译服务」这个名字本仓已用于 Linux provider 那一节（`index.tsx` 里 `aria-label="在线翻译服务"`），把「翻译服务」改成它会出现两个同名 section。

测试：顺序测试的标题提取改为只读 section-title 自身的文本节点，排除嵌套的 `<small>` 描述——先前用 `startsWith` 会把「中文标点后按空格转换」当成「中文标点」。外观页那条一并改成同一实现。另外「云联想→云候选」让 `findByRole("checkbox", { name: /云候选/ })` 同时命中 emoji 与颜文字开关（它们的描述里就有这三个字），已改为锚定的 `/^云候选/`。新测试已验证在重排前的顺序下失败。

设备证据（MateBook Pro 2in1 模拟器）：输入页首屏为 手写输入 → 高情商回复 → 输入方案（触屏变体），页尾为 简繁输入 → 云候选 → 拼音方案调频（调频方式 / 触发频次(第几次上屏触发) / 线性调频步长）→ 按键反馈，与来源同序。

增量记录（2026-09-20，外观页与来源对齐）：接着侧边栏往里做一层，比对来源 `partials/appearance.html` 的 21 个 section。

措辞：同一项设置两边叫法不同的有 10 处，统一改用来源的说法——全局主题→主题模式、设置窗口主题→设置界面主题、候选窗主题→候选窗口主题、工具栏主题→悬浮工具栏主题、Emoji 面板主题→表情面板主题、手写面板主题→手写识别板主题、语音面板主题→语音输入弹出条主题、候选布局→候选项排列方式、候选字号→候选窗字号、每页候选数量→每页候选项数量。只改共享设置窗，macOS 的 `AppearancePreferences.mm`、fcitx5 与 IBus 菜单里的同名字串不动：那是各平台自己的界面，有自己的参考。「候选窗补充字体」没有跟来源叫「候选窗中文补充字体」，因为本仓这一项也承担非中文回落，且另有「候选窗英文字体」一行，照搬会写错。

顺序：来源是 预览 → 界面渲染 → 跟随光标 → 字体 → 字号 → 颜色 → 每页数量 → 各类主题 → 排列方式 → 预编辑，而本仓把 8 个主题下拉全堆在预览之后，第一屏观感因此完全不同。现按来源重排，本客户端独有的几项贴着同类放——6 个候选配色跟在「候选文字颜色」之后，「双拼预编辑」跟在「候选项排列方式」之后。来源的「界面渲染」是 Windows 专有，不引入；「屏幕键盘主题」本仓在「屏幕键盘」页而非「外观」页，属位置差异，本次不动。新增测试只钉相对顺序，宿主隐藏某一节时不会误报，并已验证它在重排前的顺序下确实失败。

设备证据（MateBook Pro 2in1 模拟器）：外观页依次渲染为 候选窗口预览 → 候选窗口跟随光标 → 候选窗英文字体 → 候选窗主字体 …… → 表情面板主题 → 手写识别板主题 → 语音输入弹出条主题 → 候选项排列方式 → 双拼预编辑 → 行内预编辑 → 候选窗预编辑，与来源同序。

排查记录：中途两次装机后设置窗全白，一度怀疑是本次重排。实为宿主负载过高（磁盘 99%、多个交叉编译并行）时应用被 `THREAD_BLOCK_6S` 强杀，日志里是主线程卡在 `webViewTask`，不是页面报错——腾出空间后同一份产物渲染正常。判据：只含改名不含重排的产物在同样条件下也曾正常，而干净 develop 在磁盘紧张时同样会白屏。

增量记录（2026-09-20，侧边栏与来源对齐）：逐项比对来源 `ui-html/webview2/settings/ime-settings/src/partials/sidebar.html` 的 15 项与共享 `pages`。图标先比过一遍：已引入的 13 个与来源逐字节相同，但 `ai.svg` 与 `voice_input.svg` 当初没引入，代码于是让「语音输入」复用手写识别板的图标、「AI 辅助」复用帮助的图标——侧边栏上因此有两对完全一样的图标，这是引入不完整而不是设计选择。两个图标已补齐并改回各自引用。

顺序也对齐到来源：来源是 外观 → 输入 → 辅助码 → 快捷键 → 词库 → 皮肤 → 语音输入 → 屏幕键盘 → 手写识别板 → 实用功能 → AI 辅助 → 悬浮工具栏 → 帮助 → 关于 → 反馈，此前本仓把语音输入排在手写识别板之后、实用功能与 AI 辅助互换，且把「打字统计」插在输入和辅助码中间。现在 15 项与来源同序，本客户端独有而来源没有的几页（我的、AI 对话、社区、打字统计）整体排在这一段之前而不是插在中间，这也正是移动端宿主提升的那一组。macOS 不受影响：它按 `macosSidebarGroups` 分组，参照的是另一个参考窗。新增一个把这条平铺顺序钉住的测试，与既有的 macOS 分组测试同法，避免以后加页面时悄悄把来源的次序挤散。

设备证据（MateBook Pro 2in1 模拟器）：侧边栏渲染为 我的 → 打字统计 → 外观 → 输入 → 辅助码 → 快捷键 → 词库 → 皮肤 → 语音输入 → 屏幕键盘 → 手写识别板 → 实用功能 → AI 辅助 → 悬浮工具栏 → 帮助 → 关于 → 反馈，语音输入为麦克风、AI 辅助为机器人，每项图标互不相同。同时确认上一片留的开放项：应用图标换成来源标识后，状态栏托盘图标重装时未刷新确属系统对已启用输入法的缓存——模拟器重启后托盘显示的就是品牌标识。

增量记录（2026-09-20，HarmonyOS 失败形状对齐）：承接上一条留下的那类缺陷。共享设置 UI 解码失败的主契约是普通对象上的 `error.code`（`accountMessage`、`dictionaryErrorMessage`、`message` 以及社区、聊天、皮肤编辑器各自的 `switch (error.code)`），只把 `Error` 的 `message` 当兜底文案读。本宿主的 `unwrap` 一直抛 `new Error(reply.error)`，而 `Error` 实例没有 `code` 属性，于是这些表一条都匹配不上：账号的六种失败全都落到同一句「账号服务暂不可用」，取消登录不被识别为取消，词库失败丢掉自己那句具体建议，AI 失败则把机器码本身显示给用户。现改为与桌面端 Tauri 的 `CommandError { code }` 同形，抛 `{ code }`。页面启动失败那一处是唯一自己渲染错误的地方，两种形状都要认，故单独取文案。

偏好保存/读取那条路的错误来自 Rust C ABI，而 C ABI 的错误通道是全仓每个入口共用的单一字符串，里面是 `PreferencesError` 的英文 `Display` 文本而不是码——桌面端是在 Tauri 层按枚举匹配出码的。本片在本宿主的对应层（ArkTS 桥）做同样的事：`PreferencesErrorCode` 把共享 UI 确有文案的那几个变体的文本映回码，其余一律 `storage`，与桌面端对未命名失败的处理一致。文本匹配比按枚举匹配弱，所以刻意做窄：文案漂移只会让该失败退回泛化文案，不会给出错误的文案；并在 `crates/client-core` 加了一个把这些文案钉死的测试，漂移在改文案的地方就会被发现。不改 C ABI——iOS、Linux、Android 都在消费同一个错误通道。

设备证据（同一台 MateBook Pro 2in1 模拟器，同一复现路径）：接口地址填不可达的 `https://127.0.0.1:1/v1` 并填入 token 后点「获取模型列表」，上一版显示机器码 `ai_models_unavailable`，本版显示「获取模型失败，请检查地址、密钥和网络。」，与桌面端一致。合成卡顿在 `inputText` 唤起系统输入法后同样复现，移动窗口即重绘，与上一条记录的判断一致。

增量记录（2026-09-20，HarmonyOS 异步桥回推通道）：`#3236` 只恢复了无参数的同步方法，六个带参数、要做网络往返的方法仍然是坏的：`asyncMethodList` 与另一条官方异步注册路径在本 API 等级上都会挂住，页面等不到任何 settle。本片改为页面**同步**发起 `startRequest(kind, id, payload)`，宿主做完异步工作后用 `runJavaScript` 按请求号把结果回推，页面侧以请求号匹配 pending promise，30 秒超时。恢复的六条：`account`、`cloud_dictionary`、`cloud_dictionary_snapshot`、`ai_models`、`ai_test`、`api_credential`。

设备证据（MateBook Pro 2in1 模拟器，HarmonyOS 6.0.1(21)）：设置页加载正常；「我的」页点「刷新登录方式」后 NETSTACK 记录到一次真实 HTTPS 往返（`RespCode:200`，45 ms），即请求腿把异步工作真的发了出去。回包腿的直接观测在 AI 辅助页取得：把接口地址填成不可达的 `https://127.0.0.1:1/v1` 并填入 token 后点「获取模型列表」，宿主 `aiModels()` 捕获连接失败（`os_errno 111`，`curl_code 7`）并返回 `{ok:false,error:'ai_models_unavailable'}`，该字符串经 `runJavaScript` 回推后由页面渲染在「服务模型」区。请求与回包两腿都有设备证据。

该次测试中窗口一度整片变黑。这不是页面或进程故障：两个 `app.msime.client:render` 进程始终存活，faultlog 无新条目，宿主磁盘处于 98% 且模拟器 GL 交换管道掉到 3 KB/s（`DGLES d_eglSwapBuffers_special`），移动窗口即完整重绘、之前注入的点击也都已生效。记为模拟器合成卡顿，与本片改动无关；与本会话早先那次被误判为「显示层故障」的是同一个宿主磁盘条件。

本片发现但未修的一类缺陷：`apps/harmony/src/main.tsx` 的 `unwrap` 以 `new Error(reply.error)` 抛出，而共享 UI 解码错误的主契约是普通对象上的 `error.code`（桌面端 Tauri 的 `CommandError { code }`）。后果有三处已确认：`packages/ui/src/index.tsx` 的 `message()` 先判 `instanceof Error` 再查 code 表，所以偏好保存冲突在本宿主显示宿主返回的原文而不是「设置已在其他窗口修改。请重新读取后再保存。」；`packages/ui/src/account/account-page.tsx` 的 `accountMessage()` 要求 `"code" in error`，而 `Error` 实例没有该属性，于是全部账号失败一律落到泛化的「账号服务暂不可用，请稍后再试。」；AI 取模型失败在本宿主显示机器码 `ai_models_unavailable`，桌面显示中文兜底。影响面是 `main.tsx` 里全部 25 处 `unwrap` 调用点，不限于本片恢复的路径。下一片按桌面契约把 reject 形状改为 `{ code }`。

增量记录（2026-09-20，HarmonyOS 首次设备运行）：目标 `21da4a315`。此前 HarmonyOS 一栏的全部结论都只有源码与构建证据，本次首次在模拟器上实际运行，证据等级随之改变。

环境为 DevEco 自带的 HarmonyOS 6.0.1(21) phone 镜像，与项目 `compileSdkVersion` 一致；`bm install` 接受未签名 HAP。干净安装后启用输入法，本宿主日志域输出为：`module loaded` → `staged .../files/engine` → `session 1 created` → `panel ready: phone, soft keyboard`，系统侧返回 `Succeeded in enabling IME. status:FULL_EXPERIENCE_MODE`。即 ArkTS → NAPI → Rust → C++ Engine 整条链在设备上可用，Engine 会话与软键盘面板均真实创建。三个 ABI 的原生库均已构建，HAP 含全部 `libs/<abi>/`。

该次运行同时暴露并修复了一个只有在设备上才会出现的缺陷：暂存标记只记录"已暂存"而不记录暂存的是哪一代资源，因此包内资源更新后永不重新拷出，而共享校验对暂存目录做逐项精确比对，陈旧副本不是"旧"而是被直接拒绝（`existing resource generation has unexpected files`），键盘因此拒绝启动。设置页与键盘扩展此前各有一套规则写同一目录，也会留下已从资源集中移除的文件。现统一为一份按包内清单计算代次的实现。

补充（同日稍后）：磁盘恢复后重启模拟器，解锁屏幕并成功切换，`ime -g` 返回 `The current input method is: app.msime.client, status: FULL_EXPERIENCE_MODE`，即系统已把本输入法设为当前输入法。修复后的按代次暂存逻辑在设备上也已生效（日志出现 `staged .../files/engine`）。

仍未取得证据：把焦点交给真实编辑器后输入并上屏。该模拟器实例的 sceneboard 反复卡死（faultlog 中有多条 `sysfreeze-com.ohos.sceneboard`），`aa start` 报成功但画面不刷新，注入的触摸事件也不落到图标上，因此无法让任一编辑器取得焦点；输入法扩展在没有编辑器请求前不会被系统拉起。另仍未取得证据、焦点与选区、生命周期、真机签名与安装、麦克风授权流程。被系统启用并绘出面板不等于输入验收，这一栏不据此宣称平台接入完成。

增量记录（2026-09-20，HarmonyOS 第二批）：目标 `a09527a29`。来源对象为本地 `MSIME-Windows` 检出 `997fdfd9cb27ebbf3a8f998cdefae3274eb5deb9` 的 `README.md`「功能简介」「核心功能指南」，以及目标仓库内 `platforms/linux/src/core/ClientEngine.cpp` 与 `platforms/android/java/` 中已实现的同源行为；来源远端默认分支当前为 `1e4c331d5a7d62b1f219fcc0979a89dd5ead7309`，本批未读取该提交的新增内容，因此不把其后的任何变化计入。

本批补齐的来源功能：(1) 2in1 硬件键盘的五个语音快捷键（右 Alt 长按、Ctrl+Win、Ctrl+右 Alt、空格锁定、Ctrl+F9 开停、Esc 取消），共享设置页一直显示这五个 `voice_input.hotkey_*` 开关而本宿主一个都不消费；空格与 Esc 只在录音期间占用。(2) `Ctrl+Shift+E`、`Ctrl+Shift+Space`、`Ctrl+.` 三个模式快捷键，与 Linux 宿主同源，且和来源一样不受设置页那四项可关闭绑定的影响。(3) 直接英文输入的只读英文补全，走共享 `msime_client_english_completions_request`（此前 NAPI 未导出），全角字母归一为 ASCII，少于两字母不查询。(4) 录音设备选择：HTTP ASR 与豆包各自建流，故可用 `AudioSessionManager.selectMediaInputDevice` 路由；标识为设备类型加地址，缺失时回落系统默认，不把别的平台 backend 重解释为本平台设备。(5) 按应用记忆中英文状态（编辑器属性自 API 14 带 `bundleName`），映射不落盘、上限 64 个应用。(6) 切换到日语时写入 `last_chinese_scheme`。(7) 工具栏设置按钮可关闭；`Ctrl+Shift+Alt+1–8` 删除候选，来源不允许删除的候选来源仍拒绝执行但按键保持被占用。

本批修正的一类缺陷：共享设置页多处控件按平台名而非能力位判断，导致本宿主已经消费的偏好用户无法修改——候选英文字体、英文补全开关、双拼预编辑、模糊音/触摸方案列表/自定义皮肤/离线释义四个板块，均改为 `HostCapabilities` 能力位；同类问题在五笔剩余编码提示与离线释义上表现为宿主侧写死 `true`，离线释义的共享默认为关闭，等于未经询问即显示。

验证边界：全部切片通过 `hvigorw assembleHap`、`platforms/harmony/tests/run.sh` 与 `scripts/verify-local.sh --quick`；`msime-client-core` 对 `aarch64-unknown-linux-ohos` 的 `cargo check` 通过。`msime-engine-bridge` 要求 `MSIME_OHOS_DEPS` 指向为设备编译的 sqlite3 前缀，仓库不携带 amalgamation 也无固定来源，故 NAPI 动态库未构建，当前 HAP 不含 `entry/libs/<abi>/`。无 HarmonyOS 真机或模拟器运行，平台接入未完成。

增量记录（2026-09-19，HarmonyOS 批次）：目标起点 `55d3e531d`，逐项结果如下。(1) `hvigorw assembleHap` 在 develop 上以 33 个 ArkTS 编译错误失败，即 HarmonyOS 宿主有一段时间根本打不出包；本地没有任何检查会发现，因为 `platforms/harmony/tests/run.sh` 和所有 TypeScript 检查只编译 `.ts`，承载全部 ArkUI 代码的 `.ets` 只有打包这一步会编译。已修复并在 README 记为必跑门。其中两个错误是被漏掉的构建掩盖的真实缺陷：默认 `KeyboardPreferences` 记录缺 `voiceTheme` 与 `enabledSchemes`；`CandidateManagementAction.candidateActionsAvailable` 收的是方案名而视图持有的是 Engine 数字 id，于是 `scheme === 'japanese'` 这条守卫从未生效，日语候选会显示 Host API 必然拒绝的词库管理项。(2) `wubi_code_hint` 与 `candidate_english_gloss` 此前写死为 `true`；离线释义的共享默认是关闭，因此每台设备都在没被询问的情况下显示。(3) `input_mode_hud` 完全未消费，共享设置页又把该开关按平台名写死只给 macOS；现改为 `HostCapabilities::input_mode_hud` 能力驱动，macOS 与 Harmony 声明，2in1 徽标只在开启时创建。(4) 键盘选择器从不写 `last_chinese_scheme`，`KeyboardScheme.mapping` 写好却无人调用；从键盘切到日语后设置页的「中文」只能退回 quanpin。(5) 录音设备选择补齐：HTTP ASR 与豆包两条路径各自创建 `AudioCapturer`，因此可通过 `AudioSessionManager.selectMediaInputDevice` 路由；标识用设备类型加地址而非每会话重分配的 `id`，设备缺失或 backend 属于别的平台时回落系统默认，不把 Windows 端点标识重解释为 Harmony 设备。(6) `candidate_english_font` 可保存却不被候选面板读取；现按共享 `resolved-candidate-fonts.ts` 的顺序把英文字体排在中文字体之前，由 ArkUI 逐字形回退。以上均通过 `hvigorw assembleHap`、`platforms/harmony/tests/run.sh` 与 `scripts/verify-local.sh --quick`；没有 HarmonyOS 真机或模拟器运行，平台接入仍未完成。

增量记录（2026-09-20）：重新核对来源默认分支 `e1d53dd8f01fd351633f08374f189157f5cb47e9` 与目标 `origin/develop` `71008a3f9c905e7bc880d83f97e4a0d46d623ab5`。来源最新 Windows 切片中的 Ctrl+左右分词移动、`yo` 完整双拼音节、正反双辅助码缓存隔离、候选 Ctrl+Enter 译文提交和自定义数据目录均已在目标找到对应实现；其中 Engine 行为由锁定归档 `0531d4211ab17d3ba43dc8ef86c05ec574b02fa8` 加本地兼容 overlay 重建。重新运行 `python3 scripts/fetch_engine.py` 成功，并确认 overlay 生成 `double_helpcode_cache_key` 与动态候选按当前双辅助码键写入；这只是源码/构建证据，仍不替代 Windows 原生编辑器验证。

增量记录（2026-09-19）：逐键比较来源 `installer/default_config/config.default.toml` 与目标 `platforms/windows/installer/config.default.toml`，来源默认配置键在目标均有对应项；目标新增的 Windows 适配项（候选滚轮、Windows 混输最小前缀、字体回退、智能标点细分、模糊音/自动纠错拆分、提供方鉴权模式）均保留为显式字段，没有通过静默别名丢失。该项只证明配置契约覆盖，不替代运行时热更新和真实设置窗口验证。

增量记录（2026-09-20）：对照来源 `a898c3b1` / `e6419573` / `25769868` 与目标 `d50c30ad`，Windows Ctrl+Enter 译文提交已覆盖单条和多条释义；多条释义进入独立副候选页，候选导航键保持该页，空格/数字键精确上屏。已选分词后的 Ctrl+Backspace 现在保存宿主前缀与被消费的原始拼音；剩余拼音删空后再次按键会通过受限 ServerSession 字符重放恢复原始分词。原生 TSF 仍保留自身 CompositionRestore / creating-word history 边界，公共层没有接管 TSF 状态。该切片通过 x86/x64 MinGW 语法交叉编译、Rust workspace 检查、差异检查和本地 push hook；未执行 Windows 原生安装、TSF 注册或真实编辑器交互。

增量记录（2026-09-18）：Windows 屏幕键盘及其他非激活面板现在按前台编辑器所在显示器的物理工作区定位；`MonitorFromWindow`/`GetMonitorInfoW` 处理多显示器、任务栏避让和负坐标，查询失败时回退系统工作区。该切片已通过 x86_64 与 i686 MinGW 交叉检查、Rust 格式检查和差异空白检查；未执行 Windows 原生安装及真实编辑器交互验证。

增量记录（2026-09-19）：Linux IBus 候选操作菜单新增上一页/下一页动作；动作复用 `MSIME_PREVIOUS_PAGE` / `MSIME_NEXT_PAGE`，并沿用已渲染候选页的会话、代次栅栏，避免面板滞后时翻动更新后的 Engine 视图。新增 IBus smoke 覆盖翻页与回退；当前 macOS 环境缺少 IBus 开发包，未执行原生 Linux 面板构建或真实桌面交互验证。

增量记录（2026-09-19）：Linux 新增 IBus/Fcitx5 共用的候选译义拆分策略，兼容 Windows 词典使用的 ASCII 与全角分号，并过滤空译义、修剪外围空白；IBus 与 Fcitx5 的 Ctrl+Enter 现在对多译义显示各自平台的临时副候选页，选择后直接上屏并恢复 Engine 组合，单译义仍直接提交。纯策略和 IBus 合成 smoke 已覆盖；Fcitx5 原生构建仍需 Linux 开发环境验证。

增量记录（2026-09-19）：Fcitx5 临时译义页的分页回调现在同时校验候选页创建时的 session/generation；面板重绘后迟到的上一页/下一页事件会被丢弃，不会翻动新的 Engine 视图或新的译义 overlay。该切片通过 Rust 格式、差异空白和 `scripts/verify-local.sh --quick`；当前 macOS 环境仍未执行 Fcitx5 原生构建与桌面交互验证。

增量记录（2026-09-19）：Fcitx5 临时译义页的键盘导航补齐小键盘上下键与 PageUp/PageDown，和普通候选页及 IBus 的导航语义保持一致；翻页仍由本地译义分页状态处理，不把事件交给 Engine。

增量记录（2026-09-19）：Fcitx5 现消费共享 `tsf_preedit_style` 与 `candidate_preedit_style`：支持 raw/pinyin/empty 预编辑显示，并在候选辅助文本中按配置附加当前拼音预编辑；行为与 IBus 的共享配置语义一致。验证通过 Rust 格式、差异空白和 quick 本地检查；Fcitx5 原生 Linux 构建仍待相应开发环境。

增量记录（2026-09-19）：Fcitx5 状态菜单新增“双拼原始预编辑”和“五笔剩余编码”开关，仅在对应输入方案下启用，并通过共享偏好更新路径持久化；这补齐了 IBus 已有的方案特定展示设置。原生 Fcitx5 构建与真实菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 addon contract smoke 现在检查预编辑样式和方案特定显示动作的源码契约，并修正测试根目录解析，确保该检查可从仓库任意工作目录执行；这仍是静态契约验证，不替代原生 Fcitx5 构建或桌面交互。

增量记录（2026-09-19）：Fcitx5 状态栏新增按输入上下文循环切换全拼、双拼、五笔和日文方案的入口。切换前结束当前组合，保存共享 `scheme` 偏好，并用上下文级 override 重建 Host API session；方案 override 不写入其他输入上下文。已通过契约 smoke、Rust 格式、差异空白和 quick 检查；原生 Fcitx5 菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 双拼状态栏入口现在按 `xiaohe`、`ziranma`、`shoudao`、`microsoft` 循环切换 profile；仅在双拼方案下生效，结束组合后用上下文级 profile override 重建 session，并保存共享 `shuangpin_profile` 偏好。原生 Fcitx5 菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 状态栏新增词频模式循环入口，按禁用、固定、减半、线性、提升顺序切换；通过共享偏好快照即时更新当前 Engine，并异步持久化 `frequency.mode`。触发次数和线性步长仍沿用 Tauri/IBus 设置路径，原生 Fcitx5 菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 进一步新增词频触发次数和线性调整步长入口，各自在 1–10 范围循环，使用共享偏好快照更新当前 Engine，并持久化 `frequency.trigger_count` / `frequency.linear_step`。原生 Fcitx5 菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 状态栏新增候选主题循环入口，按跟随系统、浅色、深色切换并即时更新共享 `candidate_theme` 偏好。主题边框、圆角和面板配色仍由 Fcitx5/桌面 panel 决定，不伪造 IBus 或 Windows 原生窗口的不可表达装饰；原生菜单交互仍待 Linux 环境验证。（2026-09-23 更新：面板配色与边框此后改由 MSIME 生成的 `msime` classic UI 主题承载，边框见文末「Linux 候选外观补齐」；圆角仍由 Fcitx5/桌面 panel 决定。）

增量记录（2026-09-19）：Fcitx5 状态栏新增候选皮肤循环入口，按 `fluent`、`wechat`、`graphite`、`willow_green` 切换并持久化共享 `candidate_skin` 偏好；切换前结束当前组合，再重建当前输入上下文的 Host API session。Fcitx5/桌面 panel 不一定能表达 Windows 原生候选窗口的全部边框、圆角、alpha、间距等装饰，本切片只同步共享 skin preference，平台 panel 保留不可表达装饰的控制权；原生菜单交互仍待 Linux 环境验证。（2026-09-23 更新：Fcitx5 现在按皮肤画 Windows 同源的边框色与整数宽度，见文末「Linux 候选外观补齐」；圆角、alpha 阴影和间距仍不可表达，IBus 的文本属性画不了边框。）（2026-09-28 注：四个内置皮肤与共享的 `candidate_skin` 偏好已被全局主题模型取代，见 `skin/theme.rs`，这条记录只描述当时的行为。）

增量记录（2026-09-19）：Fcitx5 候选皮肤入口现消费共享 `candidate_skin_catalog` 中经过主机校验的外部皮肤 ID/标题，和 IBus 一样可从内置皮肤循环到外部皮肤；Fcitx5 仅将受限字符集的 ID 与长度受控标题交给 panel，不把外部路径或 CSS 直接注入平台菜单。原生 catalog 读取与桌面交互仍待 Linux 环境验证。（2026-09-28 注：皮肤循环已被 Fcitx5 的「主题」菜单取代，菜单列出全局主题与已安装的皮肤包，见 `platforms/linux/fcitx5/FcitxEngine.cpp`；这条记录只描述当时的行为。）

增量记录（2026-09-19）：Fcitx5 运行时 options 刷新现在同步更新候选皮肤 catalog 标题与可循环项；设置页重新扫描外部皮肤后，当前输入上下文的状态入口不会继续显示旧 catalog。该同步仍只更新平台菜单元数据，不把外部 CSS 或路径交给 Fcitx panel。

增量记录（2026-09-19）：Fcitx5 会话创建时现在加载共享 PreferencesStore 的 revision 快照，状态栏中需要原子偏好更新的动作（如候选主题）不再因缺少 revision 而静默失败；快照读取失败时仍保留已准备的输入会话，动作会等待后续偏好存储恢复。

增量记录（2026-09-19）：Fcitx5 偏好热重载现在将全局 revision 快照与当前输入上下文 override 分开处理；全局设置刷新不会覆盖当前上下文的输入方案、双拼 profile、辅助码 schema 或候选皮肤，且更新 Engine 时仍使用带 override 的有效快照。

增量记录（2026-09-19）：Fcitx5 状态动作在成功更新共享偏好后也会重新套用当前上下文 override，避免切换候选主题、标点、混输或其他设置时把该上下文的方案/皮肤状态意外恢复为全局值。

增量记录（2026-09-19）：Fcitx5 状态动作发送给 Host API 的偏好快照现在也包含当前上下文 override；持久化仍只写全局 PreferencesStore 字段，因此 Engine 有效配置与跨上下文共享配置保持分离。

增量记录（2026-09-19）：Fcitx5 候选布局与模式范围动作现在通过 Host API 即时更新当前会话并重绘候选面板，不再只写偏好文件后等待下一次会话创建；上下文 override 与全局持久化边界保持不变。

增量记录（2026-09-19）：Fcitx5 云候选、AI 联想、候选翻译开关和翻译目标语言动作现在通过统一有效偏好快照即时更新当前 Engine，再清理对应旧结果并重绘；持久化仍只修改共享偏好字段。

增量记录（2026-09-19）：Fcitx5“学习用户词频”动作现在通过有效偏好快照即时更新 Engine，不再只修改本地状态与偏好文件；切换后当前会话立即采用新的学习策略。

增量记录（2026-09-19）：Fcitx5 通过专用 Host API 更新九键布局、候选页大小、中文/成对标点和标点锁定后，会同步更新内存偏好快照，避免后续其他动作使用旧快照覆盖刚生效的值。

增量记录（2026-09-19）：Fcitx5 辅助码状态栏入口现在按蓝天、自然码、搜狗 2.0、搜狗 Plus、小鹤循环选择，分别作用于全拼/双拼对应的 helpcode 配置；切换前结束组合并用上下文级 override 重建 session，同时持久化共享 schema。原生菜单交互仍待 Linux 环境验证。

增量记录（2026-09-19）：Fcitx5 状态栏现提供 Unicode、日期时间、快捷短语、Emoji、颜文字、超级简拼、临时英文、临时日文八个本地输入模式开关；开关通过共享 `local_modes` 偏好快照即时更新 Engine，并持久化对应字段。原生 Fcitx5 菜单交互仍待 Linux 环境验证。

增量记录（2026-09-18）：Windows Tauri 语音取消命令现在按 `request_id` 从共享会话表退休对应会话；重复或未知请求保持幂等，不再让已取消的会话继续占用后续录音/识别生命周期。该修复已合入 `develop`（`e61f67bc`）；仍需 Windows 原生麦克风、取消竞态和真实编辑器上屏验证。

增量记录（2026-09-18）：Windows Tauri Emoji/剪贴板提交在真正写入前重新捕获当前外部前台窗口；识别或面板停留期间切换编辑器时，不再把文本粘贴到打开面板时的旧目标。面板自身仍在前台时保留原目标；仍需 Windows 原生剪贴板、Emoji 面板和真实编辑器验证。

增量记录（2026-09-18）：Windows 候选翻译 worker 现在在偏好热更新后没有可用翻译请求时，异步清空正/负缓存并推进请求序列；进行中的结果会被视为过期，重新启用翻译或切换提供方不会被旧的负缓存阻塞。目标 C++ 代码已通过格式与差异检查；完整 Windows 原生 provider/编辑器交互仍待验证。

增量记录（2026-09-19）：Linux IBus 与 Fcitx5 现接入 Windows 候选翻译快捷语义。候选页存在有效高亮译文时，Ctrl+Enter 提交当前已渲染译文并结束 Engine 组合；没有译文时仍将组合交给应用。IBus 通过 session/generation 与最近渲染候选身份校验，Fcitx5 使用其原生候选页的同一份共享 View；两者不伪造 Windows 的多译义副候选窗口。Linux 两个宿主同时接入 Ctrl+Backspace、Ctrl+Left、Ctrl+Right 的共享分段编辑命令。合成 provider/Engine bridge 回归已通过，原生 IBus/Fcitx5 桌面验证仍需 Linux 环境执行。

增量记录（目标基线之后）：Windows `ai.assistant` / `voice.polish` 已从共享设置按钮接到 Tauri `test_api_credential` 的 Windows 分支及 `client-core::credential::probe`。共享实现注入传输，生产 HTTPS 请求禁用重定向，5 秒连接/15 秒总时限、256 KiB 响应上限，公开错误不携带响应原文。Linux provider 路径保持不变。ASR（OpenAI、SiliconFlow、Groq、Doubao、system）与三类翻译凭据测试现均经过 Tauri 命令路由，桌面命令回归覆盖无凭据分类；合成请求测试和共享模块 Windows 交叉检查不能替代 Windows 原生设置窗口或真实服务验证。上表的明确缺口描述保留为固定目标提交时的状态。

Linux 在线 provider 的 AI 凭据测试与 Windows 终态契约对齐：只有响应中的 `choices` 为非空数组才报告连接成功；空数组、错误对象或无模型回答的 JSON 会判定失败。回归使用合成响应，不记录真实 token、提示词或服务端正文；该增量只收紧 Linux 凭据探测，不改变在线候选解析路径。

本次 macOS 增量：原生输入控制器现在按与 Tauri/Unix 宿主相同的优先级读取 `MSIME_CLIENT_HOST_OPTIONS` 指向的运行时配置（无该变量时读取 bundle/AppSupport 的 `runtime-options.json`）中的绝对 `voice_provider_socket`，再回退到 `MSIME_VOICE_PROVIDER_SOCKET`；只有路径存在时才把语音交给共享 Tauri provider，避免配置字段已保存但 macOS 错误回退到本地 Speech/Doubao。新增纯合成路径回归覆盖配置优先级、环境回退、相对路径和不存在路径拒绝；没有改变原生 Speech/HTTP/Doubao 的提交所有权。

1. **API 凭据测试真实接入**：来源已有独立设置任务队列；目标先复核可共享的提供方测试实现，补 Windows 路径和失败分类。测试只用合成凭据与本地模拟传输，不把真实凭据写入日志。
2. **Tauri 语音运行链**：明确控制器与输入目标是两个身份。OS 对端认证、有限消息/队列/关闭时限、request ID 与代际、事件与最终结果都必须连起来。Tauri 面板收结果后提交和原生直接提交只能有一个最终提交所有者，防止双重上屏。
3. **字段级配置与设备选择**：从来源 `ime_config.h` / `ime_config.cpp`、设置模块逐字段映射到共享偏好，再确认每个字段的原生消费者；设备选择单独验证稳定 ID 和实际采集。
4. **输入与面板产品行为**：按上表方案、在线候选、翻译、词库、面板、外观、服务/安装分组补差异与本地回归，每组及时合并。不因缺少一种验证环境而停止可执行的实现工作，也不声称未执行的原生验证通过。

## 语音传输必须保留的边界

剪贴板长度增量（2026-09-18）：固定来源 `MSIME-Windows` 远端默认分支 `develop` 的 `734b11e3b20c3ce44e5e96b877b37d6ed878c0a4`，`ClipboardHistory::kMaxChars` 为 4000 UTF-16 单位。Windows Tauri 粘贴宿主现使用共享历史的 12000 UTF-8 字节上限，不再误用普通输入的 4096 字节上限，避免完整中文历史记录可保存却无法粘贴。语音包装函数仍保留 4096 字节限制；空串、NUL 和超限均在系统操作前拒绝。跨宿主纯策略测试覆盖 4000 个合成汉字、补充平面字符与边界，Windows 条件编译交叉检查覆盖真实调用；未据此宣称完成 Windows 实机剪贴板验证。

翻译凭据测试增量：Windows 腾讯云、NiuTrans、自定义 DeepLX 测试现由 Tauri 调用 `client-core::credential_translation`，复用共享签名及解析。腾讯设置按钮传递当前编辑的 SecretId/SecretKey/地域；Linux 的 provider 请求保持原样。测试要求真实译文字段，不把任意 HTTP 2xx 当作成功；返回结果不包含服务端原文。自定义服务保留 HTTP/HTTPS 支持，禁止重定向。此项不改变固定基线记录，也不代表真实服务或 Windows 原生窗口验证完成。

- hello 中的 `client_id` 不是认证；控制器进程不能复用 TSF 目标进程 ID。必须由 OS 对端信息验证控制器，再单独绑定目标焦点租约。
- `PipeRegistry` 注册代际不等于激活 epoch。排队前检查还不够，执行时仍需检查焦点、会话及代际。
- 不能把未握手的 Aux 通道当成可接收凭据、识别文本的认证语音通道。
- `VoiceControlMessage` codec、来源仓库的派发包装、端点常量或 Engine 锁定归档更新都不是目标监听器、Tauri 客户端与流式结果已经接通的证据。
- #2228 / #2230 修复的是已有原生语音失败提示与旧完成回调隔离，不证明 Tauri 语音功能完成，也不证明 SendInput 回退焦点安全。

## 验证入口与本次限制

豆包凭据测试增量（来源仍为 `30a22e6f3d47adf783e8f038b1dafbd71edbb4f1`，本次目标起点 `91c5dc8b`）：Windows 设置已通过 Tauri 接到共享 `credential_doubao`，提供新版 API Key 与旧版 App ID/Access Token 的互斥认证头；新版忽略残留 App ID。传输接口可注入，生产 WSS 不跟随重定向，连接阶段 5 秒、总时限 15 秒，消息/累计响应 1 MiB、最多 64 条消息，解压复用有界共享解码器。只发送一秒合成 PCM 静音，要求有效终态 JSON，不返回服务端诊断或识别文本。内存 WebSocket、本地 TCP 重定向/超时和 UI 编辑值回归均使用合成数据；没有真实豆包凭据或原生 Windows 窗口验证。下段保留先前批量 ASR 增量时点记录；本项不补齐 Tauri 录音控制与最终上屏链。

批量 ASR 凭据测试增量：OpenAI、SiliconFlow、Groq 的 Windows 设置按钮现连接到 Tauri 和共享 `credential_asr`。请求使用内存生成的一秒 16 kHz 单声道 PCM16 静音 WAV，以 multipart 上传，不访问麦克风；界面提示可能计入服务用量。生产传输使用 HTTPS、禁止重定向、5 秒连接/15 秒请求时限及 256 KiB 响应上限。豆包需要独立 WebSocket 握手与最终协议响应，目前未借用批量入口，也未宣称豆包凭据测试或完整语音运行链已完成。

- 原生协议/会话：`platforms/windows/tests/runtime/pipe_io.cpp`、`platforms/windows/tests/runtime/server_smoke.cpp`、`platforms/windows/tests/runtime/session_pump.cpp`、`platforms/windows/tests/input/tsf_key_dispatch.cpp`。
- 配置/外观/启动：`preference_monitor.cpp`、`shared_config_keybindings.cpp`、`candidate_skin.cpp`、`shell_surfaces.cpp`。
- 语音：`voice_control_message.cpp`、`voice_providers.cpp`、`voice_session_epoch.cpp`；这些不覆盖 Tauri→Server→麦克风→ASR→编辑器全链。
- UI：`apps/desktop/src/credential-test.test.tsx`、`voice-recognition-client.test.ts`、`dictionary-safety.test.tsx`、`handwriting-pointer.test.tsx`。组件模拟测试不能证明 Windows 条件编译分支可用。
- 本次是源码对照与文档更新，不新增原生运行通过声明。GitNexus 索引已重建；概念查询遇到只读 FTS 错误，改为按固定提交文件与调用点核对。提交前仍运行 staged detect-changes。

后续修改本表时同时记录所用来源和目标提交。若来源默认分支变化，重新核对实际默认分支并固定对象；不静默把浮动 HEAD 的新功能算入旧基线已完成项。

Fcitx5 全角/半角动作增量（2026-09-19）：状态动作现在同时更新 Engine 运行时、有效偏好快照和持久化的 `character_width`（`fullwidth` / `halfwidth`），并覆盖原生 fixture 的即时生效与保存回读。此项不代表 Linux 原生桌面构建或交互验证已完成。

Fcitx5 繁体输出动作增量（2026-09-19）：状态动作现在同步 `traditional_chinese_output` 的当前上下文内存快照、revision 快照和持久化字段，避免后续偏好动作用旧快照覆盖当前繁体状态；原生 Linux 桌面构建与交互验证仍待相应环境。

Fcitx5 英文模式标签增量（2026-09-19）：状态栏中切换 Engine dedicated-English 模式的动作改为显示“英文输入模式”，与 IBus/Windows 语义区分于“英文候选”混输开关；动作 ID 与现有键绑定保持兼容。

Fcitx5 偏好保存串行化增量（2026-09-19）：多个状态动作连续修改共享 PreferencesStore 时，现在等待前一个异步 revision 保存完成再启动下一个，避免并发读取同一 revision 导致后一次字段更新丢失；原生 fixture 覆盖连续全角/半角切换的最终值。

Fcitx5 会话重建动作增量（2026-09-19）：方案、双拼 profile、辅助码 schema 和候选皮肤切换现在在重建当前输入上下文前等待对应偏好保存完成，避免新会话从尚未落盘的旧配置读取并短暂恢复旧状态。

Fcitx5 候选操作增量（2026-09-19）：未固定候选不再发布可执行的“取消固定” CandidateAction；只有候选已有固定位置时才提供该动作，匹配 IBus 菜单的禁用语义并避免面板误导用户。

Fcitx5 语音取消增量（2026-09-19）：取消或失焦关闭语音时不再直接销毁仍在运行的异步 provider future；先发送取消请求并标记结果丢弃，future 完成后由事件循环回收，避免 `std::future` 析构在输入线程同步等待网络任务。新的语音请求会等待旧任务完成并丢弃其结果，防止重叠会话。

Fcitx5 语音取消 fixture 增量（2026-09-19）：native fixture 新增合成延迟 provider future，验证取消入口在 100ms 内返回、延迟结果异步回收且不会提交文本；不使用真实音频、凭据或网络。

Fcitx5 候选管理边界增量（2026-09-19）：原生 CandidateAction 现在只对全拼/双拼/英文词典中可写入的候选发布固定、删除和位置操作；云候选、AI、Emoji、日文及其他只读来源不再显示会被 Host API 拒绝的管理入口，与 IBus 菜单和共享词典策略一致。

Fcitx5 候选 stale 栅栏增量（2026-09-19）：CandidateAction 的可见性现在同时校验当前焦点、输入启用状态、受限/私密上下文、session 和 generation；失焦或面板滞后的旧候选不会继续显示管理入口。

Fcitx5 剪贴板历史读取 stale 栅栏增量（2026-09-19）：异步本地剪贴板历史读取现在绑定请求路径与 clipboard generation；会话关闭、路径切换或禁用历史时会使旧结果失效，避免旧配置的条目污染新输入上下文。原生 Linux 桌面构建与交互验证仍待相应环境。

Fcitx5 云剪贴板读取 stale 栅栏增量（2026-09-19）：异步云剪贴板 provider 读取现在绑定 provider socket 与 generation；会话关闭或 provider 配置切换时，旧 provider 响应不会写入当前输入上下文。原生 Linux 桌面构建与云 provider 交互验证仍待相应环境。

Fcitx5 Emoji 目录读取 stale 栅栏增量（2026-09-19）：Emoji 条目与分组异步查询现在绑定当前会话 generation；会话关闭后，即使旧搜索词相同，旧目录结果也不会重新填充当前缓存。原生 Linux 桌面构建与 Emoji 交互验证仍待相应环境。

Fcitx5 剪贴板菜单预览增量（2026-09-19）：剪贴板与云剪贴板菜单标签现在按 UTF-8 字符边界截断，不再用字节截断导致中文或 Emoji 标签损坏；非法 UTF-8 输入只显示安全省略号。原生 Linux 桌面菜单交互仍待相应环境。

Fcitx5 在线候选来源隔离增量（2026-09-19）：云候选与 AI provider 请求现在只接受匹配当前请求槽位的 `source`，不再把一次 provider 响应中的另一来源候选跨槽位注入；与 IBus 的来源过滤一致。原生 Linux provider 交互仍待相应环境。

Fcitx5 候选动作执行 stale 栅栏增量（2026-09-19）：CandidateAction 触发时重新枚举动作现在也校验当前焦点、输入启用状态、受限/私密上下文，避免菜单创建后状态变化仍执行旧候选管理操作；原生 Linux 桌面构建与交互验证仍待相应环境。

增量记录（2026-09-20，Linux 本批：先把这个平台编译得出来，再谈功能对照）：本批的起因与 Windows 第三批同形——此前 Linux 的每一条记录都写着「逻辑回归通过」或「仍待 Linux 环境验证」，而事实是**这个平台的两块产物都构建不出来**，且没有任何一个门禁阶段编译过它们。先修构建，再修构建跑起来之后暴露的东西。

1. Tauri 外壳在 Linux 上有 36 个编译错误（`--all-targets` 54 个）。即设置窗口、全部共享面板（表情、剪贴板、手写、屏幕键盘、语音、云词典、云剪贴板）和账号界面在这个宿主上不是「缺某个功能」，而是构建不出来。成因是把面板投递从 crate 根挪进 `panel_input` 那次重构：crate 根与 `panel_window` 仍在无限定地调用被挪走的私有函数，`clipboard_history` 丢了 `linux_clipboard` 与 `Mutex`，四处 `window.label().as_str()` 用到本工具链仍 unstable 的 `str::as_str`，一处 `Vec` 需要元素类型标注，六个面板纯函数用例够不着新模块。为什么没人发现：`cargo check --workspace` 只看宿主 target，而 macOS 上 `msime-desktop` 因为 `tauri.macos.conf.json` 把还没构建的 app bundle 列为资源被整包排除（就是本地每次都打印的那行 `msime-desktop: skipped`）。安卓那条阶段的注释早就写过同一个道理，只是没人给 Linux 加一条（#3218）。
2. 原生宿主同样构建不出来：三个测试的相对 include 比源码移动后的层级少一级，其中一个连自己的 fixture 都引不到，`platforms/linux` 整个 target 配置不出来。修完后 IBus engine、Fcitx5 插件、全部 provider 入口与 18 个测试目标全部通过。跑起来之后发现 `linux-online-provider-contract` 约每五次失败一次，原因是 unix socket 的一个具体语义：`unix_release_sock` 在关闭方接收队列里还有未读数据时会给对端置 `ECONNRESET`，而 provider 客户端把请求正文和结尾换行分成两次写，fixture 的单次 `read` 有时只拿到正文、回复后 `close`，客户端于是在读到已排队的回复之前先拿到 ECONNRESET。两侧都改（客户端整行一次写出、fixture 读满一整行），连跑 30 次 0 失败（#3229）。
3. 两块构建都接进了门禁：有 Docker 时在固定的 `rust:1.97.1-bookworm` 容器里分别 `cargo check -p msime-desktop --all-targets` 和 `platforms/linux/build-container.sh`（编译加 `ctest`），Linux 主机上直接用系统 ibus 开发包跑，两者都没有时跳过并打印命令。两条都做了反向验证：插入只在 Linux 分支成立的错误后，阶段确实报错并 FAIL。
4. 隔离验收此前连启动都不可能：`check-container.sh` 自己算错了仓库根（少一级），`docker build` 拿到的上下文是 `platforms/platforms/linux/tests`；镜像两处 `COPY` 指着测试重组前的位置；`platforms/linux/tests` 下二十来个 Python 测试把根算成 `platforms/linux/tests` 再拼 `scripts/…`，随包在线/语音/剪贴板 provider、凭据测试、豆包鉴权、翻译缓存、录音设备这一整片自那次重组起一个都没跑过。修好之后容器内 `ctest` 19/19，三个 crate 的 Rust 测试、Host API 头导出校验、词典 CLI 与剪贴板验收全部通过，安装产物齐全（#3239）。
5. 跑起来后找到两个真实宿主缺陷。其一：嵌套偏好对象整体可省略（共享 `Preferences` 给默认值）但成员一个都不能少，宿主把单个键补进文档里本来没有的 `mixed_input` / `local_modes` 时写出残缺对象，Host API 判为 invalid options document——没有配置共享偏好目录的部署里，从 IBus 菜单切一下混输候选或任一本地输入模式，会话就再也建不起来。默认值改由新的 `msime_client_default_preferences` 从共享层发布（#3239）。其二：`rendered_view` 在首次渲染前和每次会话重建后都是 null，而 `value()` 在 null 上抛异常、异常被 `guarded` 吞掉，于是中英文切换之后打的第一个字母被静默丢掉；十八处读里有两处没带 `is_object()` 守卫，其中一处还排在会短路的身份栅栏之前（#3245）。

功能侧本批补齐四项，都是「设置页有开关、这个平台不消费」那一类：共享设置页的双拼预编辑选择此前按平台名只给 macOS，而 IBus 与 Fcitx5 都自己把快照的 `preedit` 写进平台预编辑并各自带着原生菜单开关（#3199）；`smart_punctuation_space_convert` 在 IBus 与 Fcitx5 上都是存得下、读不出效果的开关，现按来源规格补上——含来源自己踩过的那条坑，即改写前要回读标点前面的字符核对指纹，因为同一个标点在文档里通常不止一处而窗口内移动光标不是焦点变化（#3206、#3231）；Fcitx5 的「重复标点回切中文」同理从只有开关变为真的生效，并补上退格后重按同键走中文路径的另一半（#3233）。

另有两项是这个平台整块缺失而非字段缺失：桌面外壳的九个账号命令此前只为 Windows、macOS、Android、iOS 注册，Linux 上登录、资料、改名、登出、注销没有宿主可调；补齐后会话存在共享状态目录下的 owner-only 文件里（0600、原子发布、读取前核对普通文件/属主/权限/大小），这弱于 Windows 凭据管理器与 macOS Keychain 的静态加密，接 Secret Service 需要新增依赖、属于另外的决定（#3220）。设置页的「获取模型列表」与「AI 润色测试」也不可用，因为持有 token 的宿主是自己发 HTTP 的，而这个平台按设计把 token 留在 provider 的私有配置里；改为 provider 的两种新请求，并用新能力位 `ai_provider_credentials` 表达「凭据归宿主的 provider」，替掉原先按平台名藏控件的做法（#3254）。

本批的验证边界要说清楚：没有任何一项在真实 Linux 桌面上跑过，没有 IBus daemon 之外的 GTK/Qt 编辑器、没有 X11/Wayland 焦点与选区、没有 Fcitx5 实例。engine smoke 也还没走完——修掉上面第 5 条之后，它稳定停在更后面的位置（passthrough 加偏好热重载那一例里，`nihao` 的后续按键），那是本批修复之后才够得着的位置，单独查。本批后段本机 Docker 停了，因此最后两个切片的容器阶段按设计跳过；它们不触碰 C++。

增量记录（2026-09-20，Linux engine smoke 又往前走了一大段）：修掉「会话重建后第一个按键被静默丢掉」之后，隔离验收的 engine smoke 停在 `phrase()` 的第二次调用。原来的断言只说 "Phrase key not consumed"，而这个 lambda 有 56 个调用点，一句话指不到任何一个；现已改为报出是第几次调用、哪一个键，位置立刻就定住了。

停住的是 `ime_mode_scope` 那个循环（先 `app` 后 `global`）的第一轮，它连续要求三件事：FocusIn 后 `InputMode` 属性为 checked（中文）；`!key(Ctrl_L 按下) && key(Ctrl_L 松开)`，即配置的 Ctrl 快捷键在松开时被消费；紧接着 `phrase()` 打出 `nihao` 并要求仍是中文。这三条一起不可能成立——宿主里 Ctrl 松开被消费当且仅当它真的切了模式（`process_key` 的那一段除了 `toggle_input_mode` 没有别的消费路径），而 `toggle_input_mode` 先翻转 `input_enabled`，再写进按应用的记忆，`open()` 又用 `restore_app_input_mode()` 把刚写进去的新值读回来。带探针实测：Ctrl 按下时 `enabled=1`，松开被消费，随后的 `n` 看到 `enabled=0`，走透传、不被领取。

判据取自本次迁移的准绳：Windows 上配置了 Ctrl 快捷键就会切换中英文，`ime_mode_scope` 决定的是这个状态记在哪儿、而不是快捷键动不动它，宿主实现的正是这一条。所以错的是 fixture 那一侧。改为切两次并各自核对模式：默认中文 → Ctrl 切到透传（断言不再是中文）→ Ctrl 切回中文（断言是中文）→ `phrase()` 组中文候选。覆盖比原来更多，且与 Windows 一致。

改完之后这一整段循环通过，运行又往前走了两段：

其一，「Ctrl+Enter did not commit the rendered candidate translation」。把断言改成会报出它看到了什么之后，一次运行就说清楚了：`handled=1 committed=[synthetic gloss [1]] preedit=0 lookup=1`——译文上屏、组合清掉、预编辑也收了，只有候选窗还在。原因不是缺陷：候选窗的隐藏走的是 `schedule_candidate_hide` 的 24ms 定时器，宿主故意延后，免得组合中途候选列表短暂清空时面板闪一下。断言读的是一个按设计还没到的值，改成按条件等待。

其二（当前停住的位置），「NiuTrans preference change did not request a new translation」。fixture 在 `invoke("Reset")` 之后打开 NiuTrans 并保存 revision 2，然后等 provider 收到第二次请求。但 Reset 之后既没有组合也没有候选，而宿主的行为是「设置热更新后立即调度**当前**候选的翻译」——没有候选就没有要翻的东西。要么 fixture 少了一次重新组词，要么期望的是配置变更对已显示候选的重新请求而 Reset 恰好把它们清掉了。这一条同样是「fixture 写的和宿主做的哪个对」的取舍，本次不猜。

另外记一条：「Online misses did not merge with the displayed offline hits」在三次运行里只失败过一次，另两次走过去了，是时序相关而不是恒定失败。

在此之前的部分全部通过：容器内 `ctest` 19/19、三个 crate 的 Rust 测试、Host API 头导出校验、词典 CLI 与剪贴板验收、完整安装产物，以及 engine smoke 自身在此之前的全部断言（含缺 `mixed_input` 对象那一例、直接输入透传、偏好热重载后不带会话的宿主快捷键重载、`ime_mode_scope` 的两轮、离线释义先于在线回填）。

增量记录（2026-09-20，Linux engine smoke 首次跑完）：隔离验收的 IBus engine smoke 从上一批停住的位置一路走到结尾，`IBus D-Bus shared-runtime acceptance passed`。这一段把宿主缺陷和夹具缺陷分开处理，下面按性质列。

宿主侧三处真实缺陷：

1. **匿名客户端丢失中英文模式。** IBus 从 1.5.27 才报告客户端身份，而且客户端可以不报；此前这两种情况下的模式一律丢弃，于是每次焦点离开再回来都退回配置的默认模式。Windows 上模式挂在 TSF client 上不会这样，最接近的做法是给匿名客户端一个共享槽位——分辨不出来的窗口就当成同一个。
2. **迟到的焦点身份把会话拆了。** 守护进程可能先发一次不带 context/client 的 focus、稍后才补上身份，`focus_in_id` 把第二次当成切换直接 `focus_out`，用户在协商期间打的字就没了。`focus_in` 里本来就写着「IBus may replay focus after negotiating client identity」，这层包装却先把会话拆了。现在当前焦点还没有身份时就地认领，并把屏幕上的模式带进这个身份。
3. **中英标点的偏好改动到不了会话。** 运行时把宿主的标点开关记成一个 override，它压过随后下发的偏好；宿主在绝对偏好目录下走的是保存-读回-`update_preferences`。于是从菜单关掉中文标点之后，界面状态和偏好文件都变了，会话却还在按上一次内联切换留下的值转标点，逗号照样出「，」。三处修：新建会话时把 override 写进传给会话的偏好（否则焦点切换、内容类型变化重建会话后又悄悄退回文件值）、`apply_session_overrides` 同样带上、`apply_live_preferences` 记住会话最后被告知的值并在生效值变化时重新下发。

夹具侧的错误期望，每一条都先拿到证据再改：

- 候选窗的隐藏是 24ms 防闪烁定时器，直接类调用又不走 D-Bus 往返、信号还排在队里。十六处「候选窗应已关闭」和两处翻页断言改成等条件；翻页那两处原先 next-page 读到的还是上一页，previous-page 反因此「通过」，错的方向上互相抵消。
- 查找表只携带面板当前显示的那一页，而夹具把页大小设成 2。「日期模式给出 13 个以上候选」永远不成立——引擎给的是 17 个。改为翻页收集。
- 超级简拼：引擎对 `nh` 的排序是 女孩、你会 在 你好 之前，夹具却按空格提交首选。改为翻页找到候选再点。
- 词频学习：词频排序按用户敲下的切分来，你好吗 按三段词排，永远不会排进 你好 的两段列表。夹具点的就是它，于是「私密会话里不学习」和「普通会话里恢复学习」两条断言都是空的——两边都不动、两边都通过。改成选一个与 nihao 同为两段的候选（这份词库里是 拟好），两条断言这才各自成立。
- `CandidateClicked` 的参数是 `(index, button, flags)`，混合候选那两处写成了 `(0, index, 0)`；宿主要求 button 在 1..5，候选落在本页第 0 位时整个事件被丢弃。
- 绝对偏好目录下的菜单开关是一次保存（写盘、读回、再应用），本地模式禁用和 NiuTrans 那两处在同一轮里就断言，测的是旧偏好。
- 会话重建：这段要验证「重建之后旧会话排队的回调不能再提交文本」，需要一次不跑主循环的同步重建。原来用 ShuangpinProfile 触发，但绝对目录下它只是排一次保存、`apply_live_preferences` 是往现有会话推偏好、根本不重建。改用内容类型切换（private 提示），那是真正的原地 close/open，也是生产里真实存在的路径。

三处时序：在线候选那几处先固定等 1.7 秒再读精确计数，机器一忙就输，改成等请求到达再静置证明没有第二次；在线漏词合并的 2 秒预算不够——放开 provider 之后还要过工作线程读取、idle 合并、500ms 翻译去抖和 150ms settle，放宽到 8 秒，断言的是「最终会合并」而不是「多快」；修饰键绑定那两处把禁用后的偏好注入进来，但快照里带着偏好目录，重载 tick 会把文件里的值放回去，注入的偏好能不能活到按键松开取决于 tick 落在哪儿，改为注入时不带目录。

工具链一处：`build-container.sh` 用固定 tag `:local` 构建门禁镜像，而多个 worktree 会同时构建它——谁最后构建完谁决定所有人跑的是什么。实测表现为同一条命令时灵时不灵。改为按 checkout 路径散列命名，并把 `dbus-bin` 装进镜像（`--no-install-recommends` 下 `dbus` 不会带上它），免得每次手跑 smoke 都要现装。

验证：连跑八次。验证边界不变——仍然没有任何一项在真实 Linux 桌面上跑过，没有 GTK/Qt 编辑器、没有 X11/Wayland 焦点与选区、没有 Fcitx5 实例；engine smoke 覆盖的是 IBus 宿主经 D-Bus 的行为。

增量记录（2026-09-21，Windows 第六批：跟进来源新基线，先补 Windows 从来没记过一个字的打字统计）：来源远端默认分支已推进到 `b1ec3202676163927ff8632126379a42df90294b`，比第五批固定的 `1e4c331d` 多出一批提交，其中「输入统计」是一整个新功能组（DLL 采集、Server 聚合存储、设置页展示），本对照表此前没有任何一行覆盖它。目标起点 `10fba7d75`。

先说清两边不是一回事，免得下次照着来源的文件名找落点：**本仓早就有打字统计**（`crates/client-core/src/typing_statistics.rs` 加 `packages/ui/src/settings/typing-statistics.tsx`，1051 行），而且比来源那一页更宽——它有日历热力图、七日均线趋势、输入方案排行、字符类型饼图，以及来源完全没有的候选命中位置分布（首选命中率）。来源新增的是另外一组指标：活跃时长、打字速度、连续天数、当日 24 小时分布。所以这不是「有没有统计」的问题，是**两套指标各有各的缺口**。

本批只做一件事，因为它是其中最硬的一个：**Windows 上这个页面永远是空的**。`typing_statistics` 能力位对 Windows 声明为 `true`，设置页照常渲染，但 `platforms/windows/` 整个目录没有任何一处调用过 `msime_client_typing_statistics`——macOS、Linux、Android、HarmonyOS 都在各自的上屏出口记录，只有 Windows 没有。Windows 用户看到的不是「统计不准」，是除了 Tauri 面板粘贴之外一个字都没有。

落点选在 Server 而不是 TSF DLL，这一条与来源不同且是有意的：来源把采集放进 DLL，因为它的 Engine 与 DLL 同进程；本仓 Server 是唯一看得到每一条上屏字符串的地方，共享 Host API 也链在这一侧，而文本本来就要作为上屏载荷从 Server 走到 DLL，采集不让它多跨任何一道边界。来源因此需要一条新的统计命名管道（`FANY_IME_STATS_*` 契约、`stats_frames`、`stats_pipe`），本仓一条都不需要。

实现要点三条，每条都有用例：

1. `PendingReply::committed_text` 由七个产生完整上屏的出口填写，**部分选词不填**。本仓的 `prefix_` 是「Engine 已选、DLL 尚未上屏」的暂存，分词选词会连着走好几步 `partial_selection` 才由最后一条 `candidate_commit` 整串上屏；哪一步都记就会把同一个词记好几遍。`reply_composer` 里新加的断言钉的正是这条。
2. 记录发生在投递**确认之后**（`confirm` / `confirm_ui`），不是组好回复的时候。写失败或结果不确定时 pending 保留、不确认，于是也不记——「已上屏」的含义因此是「已送达」而不是「已组好」，和来源在 DLL 的三个组字出口读文档内容是同一个判据。
3. 归属取 `transition.commit_context` 而不是上屏后的 view。上屏会清掉本地模式，只看 view 的话一次 Emoji 模式的上屏会被记成全拼。来源标识与 Linux/Apple 宿主逐个相同，三家写的是同一份文档。

性能上避开一个自己先写错的版本：最初在 `confirm()` 里整份拷贝 `PendingReply` 再判断有没有提交，而 `confirm()` 每个按键都走一次，等于在输入热路径上对含候选列表的 transition JSON 做深拷贝。改成只在确实有提交时取那一小段字符串，`resolve_typing_source_from_transition` 全程按引用取字段。

验证：x86_64 MinGW 交叉构建整套通过（host DLL、TSF DLL、Server、msimeui 与全部原生测试可执行文件，含新增的 `windows-typing-statistics`），i686 语法门禁 249 个源文件通过，`verify-local.sh --quick` 通过。**没有在 Windows 主机上安装运行**，因此没有任何一项声称真实编辑器里打字后统计页出现了数字。

同批顺手修掉一个与功能无关但一直在的问题：`docs/windows-parity.md` 里有六行 `||||||| <sha>`。pre-commit 早就有冲突标记检查，它只匹配 `<<<<<<<` 和 `>>>>>>>`，而 `merge.conflictStyle` 为 diff3/zdiff3 时 git 还会写一行基线标记——手工解决时删掉认得的三种、留下这一种，钩子不出声。钩子的正则补上基线标记（并顺带改成只看新增行：原来的 `-G` 对新增和删除一视同仁，删除标记的那次提交会被它自己拦住），另加 `scripts/test-conflict-markers.py` 扫描整棵已跟踪树并挂进 `--quick`。后者不是重复：钩子看的是差异，只可能看见引入标记的那一次提交，标记一旦进了 HEAD 就再也没有东西看它一眼——这六行就是这么活下来的。两个方向都反向验证过会红。

本批查出但**未动**的三处，记下来以免下次重新推导：

- 来源新增的活跃时长、打字速度（按活跃分钟算，且只数可读字符）、连续天数与当日 24 小时分布，本仓一项都没有。这是下一片，做在共享 `typing_statistics` 与共享设置页里，不做成 Windows 私有的 SQLite 表。其中一处需要单独决定：来源的速度只数 `cjk + latin`，而它的 `latin` 是纯 ASCII 字母、假名落在 `other`，于是日文输入的速度恒为零——本仓有完整日文模式，照抄会得到一个对日文用户明显错误的读数。
- 共享设置页有 11 处以上 `window.confirm`（删词条、清空统计、关闭模糊音、放弃未保存修改等）。来源本批把它换成自绘对话框，理由是在 WebView2 里它是宿主模态窗口、不跟随页面主题、弹出期间页面自己的键盘与焦点处理全被挂起——这三条对本仓的每一个 webview 宿主同样成立。更要紧的是本仓这套页面还跑在 iOS WKWebView、Android WebView 和 HarmonyOS ArkWeb 里，而**全仓没有任何一个宿主注册过 confirm 面板回调**。这几个 webview 在没有回调时是弹窗还是直接返回 false，我没有在设备上验证过，所以这里不写结论；但「返回 false」意味着按钮按下去什么都不发生，正是本表第 789 行那条「死按钮比没有按钮更糟」。需要在 iOS/Android/HarmonyOS 上各按一次删除确认才能定性。
- 来源的第六套辅助码「加加」（`566ff8b8`）与全拼备选切分／调频重排（`1ea01d5e`、`e32eeade`）都在 Engine 及其码表资产里。本仓 Engine 由 `engine-lock.json` 固定独立归档，这些行为要进来只能是提锁，与第五批同一条理由：提锁影响面覆盖全部平台，单独决定。

  **（2026-09-21 更正：「只能提锁」是错的——没有更新的锁可提，这类改动走 overlay。三项均已落地：加加辅助码、`1ea01d5e` 备选切分、`e32eeade` 调频比较集。）**

增量记录（2026-09-21，Windows 第七批：活跃时长、打字速度、连续天数与时段分布）：接上一批留下的第一条。来源仍固定为 `b1ec3202676163927ff8632126379a42df90294b`，目标起点 `50d624ac1`。

这一片补的是两套统计里**本仓缺、来源有**的那一半：本仓的统计页宽在维度（日历热力图、七日均线、方案排行、字符类型饼图、候选命中位置），但没有任何时间轴——不知道打了多久，因此算不出速度，也不知道打在一天里的什么时候。来源新增的正是这两个轴。

落点与来源不同且是有意的：来源把它做成 Windows Server 私有的 SQLite 表（`stats_store.cpp` 662 行，加 `stats_aggregate` / `stats_overview` / `stats_frames` / `stats_pipe`），本仓做在共享 `crates/client-core/src/typing_statistics.rs` 与共享设置页里。理由是这两个轴和「每天多少字」是同一件事的不同维度，而后者早就在共享层、六个宿主都在写同一份文档；为新的两列单开一套 Windows 私有存储，等于让同一个用户的统计分裂成两份。

**活跃时长**按「上一次上屏到这一次之间的间隔」累加，超过 10 秒的间隔算休息。阈值直接取来源基线的 `kActiveGapLimitMs`，它的注释记着校准过程（来源 #429：设成 5 秒会把正常思考停顿算成打字时间）。间隔归属到本次上屏的日与小时，与来源同构。三条边界各有用例：第一次上屏不产生活跃时长；时钟回拨不累加**且不把水位往回移**（否则校正后的第一次上屏会被当成新会话）；同毫秒的两次上屏不是间隔。10 秒边界两侧都断言了。

**小时**由宿主给，理由和日期一样，而且六个宿主每处都从**同一个瞬间**取日和小时——分两次取，跨零点时会把这次上屏记到一天的日期和另一天的小时上。`hour` 在契约里可选：没有小时的宿主照常记字数。越界小时丢弃而不折到相邻桶。

**派生指标**集中在一个纯函数 `activityMetrics`，`todayKey` 由调用方传入。其中一处**与来源有意不同**，即上一批记的那个待决项：来源的速度只数 `cjk + latin`，而它的 `latin` 是纯 ASCII 字母、假名落在 `other`，于是纯日文输入的速度恒为零。本仓有完整日文模式，照抄会给日文用户一个明显错误的读数，所以把 `otherLetter`（假名、谚文等）也算进可读字符。数字与标点仍然不算——一串电话号码不是行文。两边都有用例钉住。

其余判据与来源逐条相同并各有用例：今天还没打字不算断签（今天还没过完）；「最快一天」要求当天至少一分钟活跃时长，否则两秒敲二十个字的那天永远占第一，而平均速度不设这个门槛（它是总量之比不是排名）；「最多的一天」并列取最早。另加一条来源没有的：日期加减走 UTC，因为这些键是日历标签不是时刻，用本地时间算会在夏令时切换那两天把一段没断过的连续记录算断。

保留策略删掉某天时必须连着删掉它的两个新轴，否则刚写完的文档过不了自己的校验、用户会因为一条陈旧条目丢掉整份历史；`reset` 连 `last_commit_ms` 一起清掉，它是重置后唯一还能说出用户情况的字段。两条都有用例。

验证：`cargo test -p msime-client-core typing_statistics` 13 项、`apps/desktop` 全套 91 文件 806 条用例、HarmonyOS 逻辑回归 1204 条断言、Windows 纯逻辑用例全部通过；`cargo fmt` / `clippy` / `tsc --noEmit` 干净；设置页动作门控、配色对照、偏好字段对照三个静态检查通过；HarmonyOS 设置 bundle 已重新生成。渲染断言反向验证过（去掉速度里的 `han` 会让五条用例变红）。

本批的验证空洞两处，写明而不是略过：`apps/desktop/src-tauri` 在本机被整包排除（缺 macOS app bundle 资源），所以 `panel_input.rs` 那处只核对了 `time::OffsetDateTime::hour` 在 vendored `time-0.3.55` 里的实际签名，没有编译过；Android、macOS、HarmonyOS 三个宿主的改动同样没有在各自工具链上编译，本机只有 Windows 交叉与 Rust/TS 这几条路径。没有任何一项在真机上看到过数字。

同批把上一批留的第二条待决项查实了，结论比预想的严重，**修复放在下一片**：共享设置页的 `window.confirm` 在 macOS 桌面和 iOS 上**根本不工作**，按钮按下去什么都不会发生。

这一条不是推断，是在本机量出来的。证据链四段：(1) 本仓 `Cargo.lock` 锁的是 `wry 0.55.1` 加 `tauri 2.11.5`；(2) wry 的 `WryWebViewUIDelegate`（`src/wkwebview/class/wry_web_view_ui_delegate.rs`）只实现了三个 `WKUIDelegate` 方法——文件上传面板、媒体捕获授权、新窗口创建，`runJavaScriptAlertPanel` / `runJavaScriptConfirmPanel` / `runJavaScriptTextInputPanel` 一个都没有；(3) `tauri-runtime-wry` 与 `tauri` 自己不另设 uiDelegate（全仓搜不到）；(4) 照着这个配置写了一个最小 WKWebView 探针（不设 JS 对话框方法）在本机跑，结果是 `confirm()` **2 毫秒内返回 false、什么都不显示**。WKWebView 本身不提供内建 JS 对话框，必须由 delegate 实现，没实现就等同用户点了取消。

受影响的是共享设置页里 11 处以上的确认：删除词条、清空打字统计、关闭模糊音并清空规则、恢复屏幕键盘默认值、放弃未保存修改后重新读取、删除云词条、把云词条加入本机词典、开始新对话等。在 macOS 与 iOS 上它们全部是死按钮——这正是本表第 789 行那条「死按钮比没有按钮更糟，它宣称功能存在」，而那一轮的静态守卫只检查按钮有没有按宿主能力门控，检查不到这一类。

其余宿主逐个核过：Android 走 wry 的 `RustWebChromeClient.onJsConfirm`，会弹真的 `AlertDialog`，功能正常，但按钮文案是写死的英文 `OK` / `Cancel`，与页面的中文界面不一致；Linux（WebKitGTK）与 Windows（WebView2）原生支持，工作正常。HarmonyOS 的 ArkWeb 在 `Settings.ets` 里没有注册 `onConfirm`，其默认行为没有在设备上验证过，这里不下结论。

来源本批（`fcf594e2`）正好把 `window.confirm` 换成了自绘对话框，理由是宿主模态窗口不跟随页面主题、弹出期间页面自己的键盘与焦点处理被挂起。那三条理由对本仓每个 webview 宿主都成立，而上面这条比它们严重得多。下一片按同一方向做：共享一个自绘确认对话框，把这 11 处换掉，顺带解决 Android 的英文按钮。

增量记录（2026-09-21，Windows 第八批：把上一批量出来的死按钮修掉）：接第七批末尾那条。共享设置页 16 处 `window.confirm`，在 macOS 桌面和 iOS 上按下去什么都不发生，证据链见第七批。这一批把它们全部换成页内对话框。

换法是 `useConfirm()`（`packages/ui/src/core/confirm.tsx`）：返回一个可 await 的 `confirm` 和一个要渲染的节点，页面靠使用这个 hook 接入，不用 provider 也不用 portal——共享 UI 是一棵很大的树，为一个对话框加一层上下文提供者要碰的地方比 16 个调用点还多。浮层沿用仓内已有的 `role="dialog"` 样式，Esc 与点遮罩都是取消，焦点进入时落在确认按钮、答完还回原处。

三处边界各有用例，都是自绘对话框特有而宿主模态没有的问题：弹着时再发一个请求直接答否，而不是把对话框从正在做决定的用户眼前换掉；组件在对话框开着时卸载答否，而不是让调用方永远等下去；从对话框内部按下、在遮罩上松开不算取消。

顺带修掉 `panels.tsx` 里的 `confirmAction`：它在 `typeof window.confirm !== "function"` 时**不问直接放行**，即在一个没有 confirm 的宿主上删除云端候选会无声执行。这是与上面相反方向的同一个错误——一个假定宿主对话框一定可用的封装，两种失败形状都被它占全了。

21 处测试原本 stub `window.confirm`，等于在断言一个用户根本看不见的控件；改成回答真正渲染出来的对话框（`apps/desktop/tests/support/confirm.ts`），顺带证明每条流程都确实走到了它。

新增 `scripts/test-no-host-dialogs.py` 挂进 `--quick`，禁止共享 UI 出现 `window.confirm` / `alert` / `prompt`。这个守卫自己先写错一版并且**错得很典型**：判断是否带 `window.` 前缀时看的是匹配位置之前的字符，而 `window` 就在匹配里面，于是它放过了 `window.confirm` 却抓住了 `alert` 和 `prompt`——只测一种写法就会以为它是好的。改看匹配文本后四种写法（含 `window . confirm`）全部拦下，裸 `confirm(`（hook 自己的方法）照常放行。

未做：Android 那边弹窗能出来，但按钮文案是 wry 写死的英文 `OK` / `Cancel`；改用页内对话框之后这个问题一并消失，因为按钮由本页渲染。HarmonyOS 的 ArkWeb 默认行为仍未在设备上验证，不过页内对话框对它同样成立，所以这条不再是缺口而只是一个没查完的事实。真机上按这些按钮仍未做过。

增量记录（2026-09-21，Windows 第九批：跟进来源新基线里剩下的界面项）：来源仍固定 `b1ec3202`，目标起点 `332ea2416`。第五批之后来源新增的提交里，输入统计（第六、七批）和 `window.confirm`（第八批）已做完，这一批把剩下的界面项走完。

**已移植两项，都是取值不变、只改可见文字：**

固定标点（来源 `4f074779` + `38da1e77`）。来源先补回误删的「始终使用中文标点」，再把那对互斥开关改成单选组。本仓**早就**是单值控件——`punctuation_lock` 的三个取值一直在一个下拉里，来源这两步走到的形状这边一开始就是——差的只有文案，四处全部改用来源说法（固定标点 / 跟随中英文状态 / 始终使用中文标点 / 始终使用英文标点），并补上来源那句「切换中英文时的标点形态，三者互斥」。控件保持下拉：来源换 radio 是为了从两个开关表达一个值，这边从来就是一个值，相邻每个单值设置也都是下拉。macOS 原生备用窗的同一控件一并改名，否则同一平台上两个窗口对同一个偏好用两个名字。

日语方案「罗马字」→「罗马音」（来源 `eb512885`），含下面那句说明。来源只改了这一处：实用功能页 R 模式的说明里它自己仍写「按日语罗马字处理」，所以本仓对应那句不动——这类「来源只改了一半」的地方要照着它改一半，否则下次对照会以为是本仓漂移。

两处都钉进 `referenceSections` / `referenceOptions`，反向验证过会红。

**核对确认不适用一项：** 来源 `fcf594e2` 的词库浮层定位。它的根因是 `#content-container` 上有 `contain: layout paint`，于是写在页面片段里的 `position: fixed` 提示条和弹窗退化成相对该容器定位、跟着内容一起滚。本仓逐个查过：`[contain:layout_style_paint]` 只出现在侧栏（`settings-style.ts`），设置外壳是 `flex h-full flex-col overflow-hidden bg-chrome`，云词典各面板根是 `min-h-screen bg-chrome text-body`，都没有 `contain`、`transform`、`filter` 或 `backdrop-filter`——`overflow: hidden` 不产生包含块——所以 `fixed` 照常相对视口解析。第八批新加的确认对话框就渲染在这些根下面，一并核过。带 `backdrop-blur` 的玻璃面板样式确实存在，但用在表情/键盘面板，不在这几个树里。

**不移植，理由同第五批：** 加加辅助码（`566ff8b8`）与全拼备选切分／调频重排（`1ea01d5e`、`e32eeade`）都在 Engine 与其码表资产里，只能通过提 `engine-lock.json` 进来，影响面覆盖全部平台，单独决定。

  **（2026-09-21 更正：「只能提锁」是错的——没有更新的锁可提，这类改动走 overlay。三项均已落地：加加辅助码、`1ea01d5e` 备选切分、`e32eeade` 调频比较集。）**

增量记录（2026-09-21，Windows 第十批：词库维护这一行往下查）：对照表「词库查询、增改删、导入导出、快捷短语」一行点名的四项里，先走 quiesce/resume 与失败恢复，再走导出编码。目标起点 `cb0365752`。

**quiesce/resume 与失败恢复：核对确认已做到，无需改动。** 这里记下结论以免下次重查。桌面侧在收到 `dictionary maintenance busy` 时才握手，成功后重试，然后**无条件**发 resume（注释写明「导入失败总比让输入法没有会话好」）。Server 侧 `quiesce_dictionaries` 成功时设一个 30 秒 deadline，控制线程每 tick 检查、过期就自己 resume——所以一个在 quiesce 和 resume 之间死掉的设置进程，最多让输入停 30 秒而不是停到重启。没有 Server 在听时 quiesce 返回真、resume 返回假，判据是「锁本来就空着，调用方该继续」。三层各自独立，任一层失效另外两层仍然成立。

**macOS 的 quiesce/resume 与来源对齐。** macOS 沿用 Linux 宿主的租约：设置窗口在用户数据目录写入带 30 秒过期时间的 `.msime-dictionary-quiesce`，发 `MSIMEDictionaryMaintenanceWillBeginNotification` 分布式通知作为立即唤醒，每个请求在约 2.5 秒内重试；分批导入时租约在各批之间保持并在每批前续期，整次操作结束后删除。IMK 只在租约存在时让出：上屏当前组合、关闭所有控制器的会话，租约期间不开新会话，删除后的下一次按键重开并恢复专用英文。三层对应关系与来源一致：通知对应 quiesce 请求，租约删除对应 resume，租约自带的过期时间对应 Server 的 30 秒 deadline；通知丢失时由每秒的偏好定时器兜底。租约的格式与读写由 `platforms/common/DictionaryQuiesceLease.h` 与 `crates/client-core/src/dictionary/quiesce.rs` 在 Linux 与 macOS 之间共用。

**导入编码：查出并修掉一处静默损坏。** 云词库文件面板用 `File.text()` 读用户选的文件，它只按 UTF-8 解码；而本地词库导入早就走 `decodeDictionaryBytes`，处理 UTF-8 BOM、UTF-16 两种字节序和 GB18030。同一个文件两个面板两种结果，云端这边更糟：UTF-16 解出来满是 NUL，被 `text.includes("\u0000")` 挡下（至少是拒绝）；GB18030 解出来是一串 `�` 且**不含 NUL**，守卫放行，一份全是替换字符的词库被静默上传到用户云端。实测 `"你好\tni'hao\n"` 的 GB18030 字节按 UTF-8 解码得到 `"���\tni'hao\n"`。改成调用同一个读取器，NUL 检查保留给真正的二进制文件。

顺带修共享导入解析器不剥前导 BOM。目前每个调用方都在更上游剥掉了，所以不是当下可触发的缺陷，但它是公共入口而这条不变量只靠「每个调用方都记得」维持。`str::trim` 不去掉它（U+FEFF 早就不是 White_Space），于是它活到第一行、落在该格式的第一列：词在前的格式里粘在词上，解析通过、存进引擎、永远匹配不上；编码在前的格式里落在编码上，判字母表非法，报一行失败且读者无从得知原因。两种都静默，其中一种损坏数据。

两处都反向验证过。（**这两项已于 2026-09-21 走完**，见下面那批增量记录。）

增量记录（2026-09-21，Windows 第十一批：词库各表的字段逐项对照）：接上一批，走对照表「词库」一行剩下的「五笔/英文/快捷短语/翻译各表的字段」。目标起点 `fd1db5f6a`。

逐项与来源 `server/src/settings/dictionary_validation.cpp` 对过，**行为一致**，差异都是本仓更严或更细，记下来免得下次重判：

- 权重缺省 10000：两边相同（来源 `kDefaultCodedImportWeight`）。
- 负权重拒绝：来源要求第三列全是数字（因而排除负号），本仓解析成 `i64` 后判 `weight < 0`，结果相同。本仓另外接受 `+5` 而来源不接受，无实际影响。
- 第三列含 `=` 视为元数据、取缺省：来源对所有格式都这么做，那是它容纳 Rime 文件的方式；本仓有显式的 Rime 格式，把这条规则限定在 Rime 内——比来源更精确，不是缺口。
- 跳过空行、`#` 注释与 `---` / `...` YAML 头：与来源 `ShouldSkipImportLine` 一致。
- 两列或三列、词与编码不得为空：一致。
- 每种码表的键长上限（全拼 256 / 五笔 4 / 快捷短语 32 / 英文 64）是本仓新增的防御边界，来源把长度交给 Engine 判，属于本仓更严。

**查出并修掉一处漂移隐患：快捷短语的长度上限。** 它的权威来源是 Engine 自己的 `contracts/ipc_protocol_limits.h`（`CandidateTextMaxLength` = 199）——短语最终写进的就是那个管道字段，超出即被截断，而故障只在使用时、离输入处很远才显现。这个值此前是**六处裸字面量**，分布在三个 crate 里，没有一处与该头文件相连；提 `engine-lock.json` 恰恰是唯一会改变该字段的操作，而那时没有任何东西会注意到。现收成一个常量并加 `scripts/test-quick-phrase-limit.py` 挂进 `--quick`，比对常量与头文件、同时禁止新的裸字面量。这里记一笔方法上的教训：我先只找到四处就动手，另外两处是守卫第一次运行时翻出来的——这类东西要用检查而不是用眼睛找。

本批查出但**未动**的一处，记下判据以免下次重新推导：新增词条的默认权重，设置页用 100000，而共享导入用 10000、来源的编辑路径缺省是 10。100000 高出一个量级看着像有意为之（手工加的词应当压过批量导入的词），但没有任何地方写明；更要紧的是设置页在宿主没有提供批量导入命令时会走一条逐条 `edit` 的回退循环，那条路径也用 100000——同一个文件在两种宿主上会得到相差十倍的权重。目前 desktop 与 HarmonyOS 都提供了 `import`，所以这条回退是潜在而非现行的。判定它是不是缺口需要来源设置页发送的权重，本批没有取到，因此不下结论也不改动。

  **（2026-09-22 更正：已从固定来源 `467b9804` 取到判据并修正。）** 来源设置页 `ui-html/webview2/settings/ime-settings/src/modules/dict.ts` 的新增对话框明确把缺省权重填成 10，Server 四条创建路径的 `IntValue(request, "weight", 10)` 也以 10 兜底；批量 coded import 则由 `server/src/settings/dictionary_validation.h` 的 `kDefaultCodedImportWeight = 10000` 定义。共享设置页现分别对齐这两个语义：手工新增为 10，无批量导入能力时的逐条 fallback 为 10000；fallback 同时不再用 `Number(weight) || default`，因此文件里合法的显式权重 0 会原样保留。

本行至此四项走完三项，剩「保留用户数据」。

增量记录（2026-09-21，Windows 第十二批：维护期间保留用户数据）：对照表「词库」一行的最后一项。目标起点 `a5833c5cb`。

**查出并修掉一处数据丢失路径。** 云词库快照激活的形状是「把旧内容搬进 backup、把新内容搬进 current」，任一步失败就回滚。回滚用的 `rename` 是尽力而为（`let _ =`），而紧接着是一句无条件的 `remove_dir_all(backup)`。这两件事凑在一起就是丢数据：回滚的某个 rename 失败，恰恰意味着 backup 里那份是用户词库仅存的副本，而下一行把它整个删掉——发生在一次**已经被扛住**的失败的收尾路径上。改成非递归的 `remove_dir`，它拒绝删除非空目录，而这个拒绝正是要点：回滚干净时目录是空的、照常清掉，回滚没搬干净时目录非空、留在盘上。成功路径那处不动，那里 backup 装的是被有意丢弃的旧内容。

**核对确认已做到，记下以免重查：** `reset_learned_data`（`engine-bridge/native/bridge.cpp`）是同类操作里做得对的那个，可以当参照：先把新内容写进 `.reset.<stamp>` 临时名，再把原件 rename 成 `.backup.<stamp>`，三份（主词库、英文词库、学习日志）**全部发布之后**才删 backup；任一步抛出走 `fail_cleanup`，它撤掉已发布的、从 backup 把原件搬回来，而且**不删 backup**——即使搬回的 rename 失败，数据仍在盘上。上面那处 Rust 回滚缺的正是这最后半句。

派生名一律在 path 自身的 native 字符串上拼接而不过 `path::string()`（第二批 #3059 的结论），`-wal` / `-shm` / `-journal` 三个 sidecar 在发布后删除，否则用户刚清掉的学习数据会在下次打开时回来。sidecar 删除位于 try 块内、发生在新文件已就位之后，抛出会把已成功的清除回滚并报成失败——这一点第二批已记录为已知形状，本批未改。

至此对照表「词库查询、增改删、导入导出、快捷短语」一行的四项全部走完。

增量记录（2026-09-21，Windows 第十三批：云候选与 AI 联想的五个轴）：对照表该行点名的五项「每个提供方、超时、取消、失焦后旧结果、凭据路由」逐条走。目标起点 `93075a413`。

**五项都核对确认已做到**，结论记在这里以免重查：

- 失焦后旧结果有**两道独立的栅栏**。Windows 侧 `FocusedSession::apply_cloud_response` / `apply_ai_candidates` 先 `prepared(lease)` 再 `gate_.with_active(lease, ...)`，过期 lease 的结果直接丢弃；Rust Engine 会话的在线请求栅栏 `OnlineRequestGuard::matches`（`crates/engine/src/session/online.rs`）比对 session id、generation、scheme、identity、query_text、cache_key、分词和两个资格位，所以同一 lease 内「先打 ni 后打 nihao」的旧回复也进不来。共享 Rust 层不另设栅栏是对的——身份归 Engine 所有，多一份副本就是多一处漂移。
- 超时两个 worker 各有各的值且都合理：云候选连接 2000ms / 总计 2000ms，AI 连接 2500ms / 总计 8000ms（LLM 本就更慢，照抄 2 秒会把它全判超时）。两者都是 `CURLOPT_PROTOCOLS_STR="https"`、不跟随重定向、`NOSIGNAL`。
- 取消不只是「丢弃结果」：`CURLOPT_XFERINFOFUNCTION` 接到取消判据上，被取代的请求在传输途中就会中止，write 回调里也再查一次。
- 响应与请求都有界：响应 256 KiB（AI 1 MiB）、query 16 KiB、AI 的 URL ≤ 2048 且必须 https、POST body 有大小上限。
- 提供方路由两边同形：来源 `ai_assistant.cpp` 只对 `deepseek` 特判（多一个 thinking 字段），其余走通用 OpenAI 兼容路径；本仓 `client-core/src/ai.rs` 完全一致。

**查出一处覆盖缺口并补上。** 整个 `platforms/windows/tests/` 里**没有一个文件提到过这三个 worker**——`CloudCandidateWorker`、`AiCandidateWorker`、`TranslationWorker` 的排队、去抖与取消一行覆盖都没有。这属于本表反复出现的「按源码核对为正确实现但零测试覆盖」，而这里出 bug 的表现是「打完字之后旧的云候选才冒出来」，不会在别处被发现。给 worker 加可注入的 fetcher（默认仍是真实那个），补四条用例：结果带对 lease 与 query、去抖窗口内只付一次且付新的那次、在途被取代的请求能看见自己被取代且结果绝不交付、五种非法请求一个都到不了网络。两种破坏分别红在不同断言上。

值得一记的是这条用例**在本机真的跑过**，不只是链接：`FocusGate` / `PipeTicket` 不含 Windows 头，curl 本机就有，用 clang++ 原生编译运行通过。此前 Windows 侧的用例在这台机器上一律只有「链接成功」这一级证据。（**AI 与翻译两个 worker 的同类用例已在 2026-09-21 补上**，见下面那批增量记录。）

增量记录（2026-09-21，Windows 第十四批：把「链接了」变成「跑过了」）：本表每一批的验证限制段都写着同一句话——没有 Windows 主机，所以 Windows 的用例只有「交叉构建链接成功」这一级证据。这一批发现那句话的适用范围比以为的小得多。

`platforms/windows/tests/` 绝大部分是策略：对着契约结构体的纯函数，整个翻译单元里没有一次 Win32 调用。这类源文件用**宿主编译器**就能编译、链接、执行。实测 92 个测试源里 59 个可以，约 25 秒跑完，此前它们在这台机器上从未执行过一次。新增 `scripts/test-windows-native-run.py` 挂进 `--quick`。

够格与否是发现出来的不是列出来的：能独立编译链接的，就是不需要 Windows 的。两类正常跳过——缺 Windows 头，或缺它在 Windows 构建里一起链接的其他翻译单元（未定义符号）。其余编译错误一律报失败：让编译不过的东西悄悄退出计数，正是套件消失的方式。两处 `HOST_DIFFERENCES` 各写明理由：`aux_message` 因为 `wchar_t` 在这里是 4 字节、Windows 是 2 字节，用宽字面量拼出来的线上字节形状不同、嵌入 NUL 那条根本表达不出来；`shell_surfaces` 断言的是 Windows 路径与环境块语义。另两处是编译期的宿主差异（`u8string()` 的 `char8_t`、只为 Windows 声明的 `preference_monitor_tests`）。

**这个 runner 第一次真用就查出四个没有任何东西在编译的测试**，全部在 CMakeLists 里从未出现过，与 `known-failures.txt` 记的 msimeui-tests 同型：

- `tests/input/tsf_key_dispatch.cpp`——原样就能通过，本批已注册进构建。
- `tests/clipboard/` 下的 `clipboard_history.cpp`、`clipboard_link.cpp`、`clipboard_presentation.cpp`——编译得起来但**断言失败**，原因是它们写的是已被废弃的旧契约。`normalize_clipboard_text` 的注释写明「只去掉 CF_UNICODETEXT 带来的终止符；换行（含 CRLF）与空白是用户内容，必须原样往返」，而测试期望 CRLF 折成 LF、首尾空白裁掉。实测存进去的正是未规范化的原串。**是测试过时不是代码有错**——行为改的时候没人更新它们，因为没有任何东西会编译它们。这三个是下一片。

顺带记一条方法：写这个 runner 时自己踩了四个坑（把子进程夹具当测试、执行继承调用者 stdin 导致永久阻塞、并行执行饿死等定时器的用例、编译失败被算成跳过因而改坏了也返回 0），每一个都是反向验证暴露的。第四个尤其值得一提——它让这个工具犯了它正要去发现的那个错误。

增量记录（2026-09-21，Windows 第十五批：把上一批查出的四个「没人编译的测试」处理完）：目标起点 `eaae1c858`。

`tsf_key_dispatch` 上一批已接上。这一批是剩下三个：`tests/clipboard/` 下的 history、link、presentation，在任何 CMakeLists 里都没出现过。

`clipboard_link` 与 `clipboard_presentation` **原样就通过**，只是没人编译，注册即可。

`clipboard_history` 编译得起来但断言失败，是测试过时。现行契约由 `normalize_clipboard_text` 的注释写明：只去掉 CF_UNICODETEXT 带来的东西——首个 NUL 起截断、剥掉尾部 `\r`——而换行（含 CRLF）与空白是用户内容、必须原样往返。测试期望的是 CRLF 折成 LF、首尾空白裁掉，那是被有意废弃的旧规则。四处按现行契约重写，每一处都保留原本要测的那件事：往返改为断言一字不差；去重改用只差一个尾部 `\r` 的一对（它们确实规范化成同一条），并补一条反面；「规范化后为空」改用 `"\r"`，因为 `" \t\r\n"` 在新契约下不为空；NUL 那条从「剔除并保留其后 4000 字符」改为断言截断，4000 UTF-16 单位的上限另用干净字符串单独测。三条规则各自反向验证过。

顺带把断言失败改成带行号——90 行里任何一条失败原本都报同一句话，定位这个问题时先吃了一次亏。

本机 runner 现在报「1 not tests」，只剩 `voice_wire_peer.cpp` 那个真正的子进程夹具。至此上一批查出的四个全部处理完毕。

增量记录（2026-09-21，Windows 第十六批：手写这一行）：对照表点名要比「模型打包、笔画缩放、撤销/清空、多候选、原编辑器上屏」。目标起点 `d90195fd4`。本机能验证的四项**全部核对确认已做到**，结论记下以免重查。

- **笔画缩放**做得比预期细。指针坐标不是直接用 `getBoundingClientRect` 换算，而是走 SVG 的 `getScreenCTM()` 逆矩阵，因为边框、viewBox 的留白、窗口缩放和 CSS transform 这几样单靠包围盒表达不出来；结果落在固定的 0–420 viewBox 空间里，与面板实际渲染多大无关。矩阵不可用（画布已分离或不可逆）时的回退按包围盒比例映射到**同一个** 0–420 空间，两条路径给出的坐标可比。
- **采样**同时做了两件事：半单位移动阈值去抖（注释写明与 Windows 面板一致），以及超过 256 点后隔点抽稀但保留原始起点与最新末点。
- **撤销/清空/重做**都在，且撤销后重做受同一个笔画上限约束。
- **多候选**上限 12，中文候选排在其他之前。

两项本机无法验证，仍需 Windows 主机：模型随包分发、以及识别结果提交回原编辑器。

**顺带钉住一处漂移隐患**（与第十一批快捷短语上限同型）：面板与共享契约各自写了三个上限——笔画（32 对 64）、每笔点数（256 对 4096）、候选（12 对 12）。目前面板一律更严或相等，也就是正确的方向，但两边的数字互不相连，靠的是「没人把面板那个数调大」。调大是最坏的那种静默：用户照常画，识别直接什么都不返回，因为请求在到达识别器之前就被拒了。新增 `scripts/test-handwriting-limits.py` 挂进 `--quick`，检查的是单向不变量——面板可以更严，不能更松。三个上限各自反向验证过。

增量记录（2026-09-21，Windows 第十七批：语音热键）：对照表「语音热键、流式/批量 ASR、润色、声音/静音、上屏方式」一行点名的第一项。目标起点 `e9632bfff`。

先按模块查覆盖：`src/voice/` 下 16 个模块里，`VoiceHotkey` 是唯一**零覆盖**的可测模块——`CuePlayer`、`DoubaoAsrClient`、`SystemAudioMuter` 是硬件与网络，本机测不了；其余每个都有用例，且第十四批之后它们在本机是真正执行的。

而这个低层键盘钩子里的规则恰恰是「错一个分支」的类型：修饰键被当作快捷键用过之后漏给应用（每次口述都弹菜单或开始菜单）、用户松手了录音还在继续、Ctrl+F9 只吞了按下而抬起那次仍送进编辑器。钩子本身在 Windows 之外跑不了，所以此前没有任何办法覆盖。

按本仓已有的 `*Policy.h` 做法把决策抽成 `VoiceHotkeyPolicy.h` 的纯函数——不含 `<windows.h>`，状态仍留在控制器的 atomic 里，钩子只改为调用。虚拟键码在策略里具名并在 `.cpp` 里逐个 `static_assert` 对住 Win32 的值，改名或打错不可能通过。

记下几条规则的**理由**，它们从代码里读不出来但改动时必须知道：激活顺序即行为（同时开着 RCtrl+RAlt 和 RAlt 时，单键的若排在前面会把两键的彻底吞掉）；两者对 Ctrl 的要求不对称是有意的（RCtrl+RAlt 要右 Ctrl，Ctrl+Win 任一都认）；压制闩只在**抬起**时清除，按下时清会让抬起那次无人认领地漏给应用；既不是按下也不是抬起的事件不动修饰键状态，否则会把用户还按着的键标成松开。五条反向验证过，各红在不同行。

本行其余四项仍需 Windows 原生验证。

增量记录（2026-09-21，Windows 第十八批：候选右键动作的可用性）：沿对照表「悬浮工具栏、托盘菜单、入口快捷键」一行按模块查覆盖。目标起点 `d889ee820`。

`src/candidate/` 下零覆盖的有六个，其中四个是本机测不了的（两个异步 worker 的网络部分、皮肤资产、窗口阴影的 Win32 绘制）。唯一既零覆盖又完全可测的是 `CandidateActionAvailability`——它决定候选右键那四项（置顶、固定排位、取消固定、删除）要不要给出来。

判据原本是三个裸数字 `source == 0 || 1 || 4`。它们是 Engine `CandidateSource` 枚举的**位置**（0 Database、1 UserDatabase、4 EnglishDictionary），以数值形态过线；Windows Server 经共享 Host API 与 Engine 通信，include 不到 `core/word_item.h`，这一侧在 C++ 里叫不出那个枚举的名字。于是 Engine 往枚举中间插一项，后面每项挪一格，编译一声不响，而「删除」开始被提供给云候选——Engine 随后必定拒绝，菜单里多出一项按下去什么都不发生。新增 `scripts/test-candidate-sources.py` 挂进 `--quick` 把具名常量对住枚举位置，反向验证过。

顺带把注释写准：原注释只说排除云和 AI 投影，而代码实际是**白名单**——快捷短语、Emoji、颜文字、生成项与兜底项同样被排除，因为它们不在这几个动作要编辑的词表里。白名单这一点本身有用例钉住：Engine 新增的 source 默认被拒，而不是默认被提供。

另外核对确认无缺口：单码点候选不提供「删除」这条与来源一致（`CandidateMenu.h` 的注释直接引了来源 `candidate_presenter.cpp` 的行号），且已有覆盖；来源那边不按 source 过滤，本仓这条按 source 的白名单是本仓自己加的一层，方向是更严。

增量记录（2026-09-21，Windows 第十九批：组字期标点的字符表终于有了用例）：沿 `src/ipc/` 与 `src/system/` 按模块查覆盖。目标起点 `5e14446f8`。

两个目录里零覆盖的十一个模块，多数是 Win32 资源或图标字体这类本机测不了的东西。其中**最该有覆盖的是 `PunctuationPolicy`**：它就是 2026-09-20 那批记过的「组字期标点的上屏时机」在本仓的实现，那一批把来源的字符表抄进了记录，却没有任何东西把实现钉住。这个表少一个字符，意味着那个标点不再用高亮候选结束组字——只在打字时看得见。

逐字符与来源 `IsCommitWithHighlightedCandidatePunctuationInCandidateMode` 比对：23 个字符两边完全相同。用例逐个断言，并覆盖三类排除（减号加号及小键盘孪生、逗号句号被配成翻页键、方括号被配成翻页键），以及「配了一对不影响另一对」。（此为当时的状态。来源 `1d2431ad` 随后把 `-`、`+`、`/` 加进表里，小键盘的加减号不再算翻页键；本仓对应的是 `literal_candidate_punctuation`，见 2026-09-23 的记录。`candidate_punctuation` 本身仍是这 23 个字符，数字键与 `/` 在它之前被分走。）

核对了一处写法不同但**结果等价**的地方，记下免得下次误判为缺口：来源的翻页排除额外要求「有活动组字」，本仓没有这个条件；差异只在「没有组字时按逗号」，而来源那时走 `else { ClearState(); return; }`，同样不把标点变成带高亮候选上屏，本仓返回 nullopt 落点相同。

另记一处冗余：策略里的 `wch <= 127` 永远不会是拒绝的原因，因为 `translate_key` 只把 0x21..0x7E 认作字符。这不是缺陷（第二道保证窄化转换拿不到表示不了的值），但用例注释写明了，断言钉的是行为而不是那一行——否则后来的人会以为它承重。这条也是本批唯一一条反向验证**不会变红**的规则，如实记下而不是编一个能红的断言。

顺带一提：交叉构建抓到我自己漏配的 include 目录，本机能编只是因为手动加了参数——这正是那一阶段存在的意义。

增量记录（2026-09-23，Windows TSF：成对标点补全的光标左移、跳过右半边、嵌套计数回退，以及候选打开时的小键盘标点）：对照来源 `KeyHandler.cpp` 核对了四处，四处都是真缺口。

**光标左移被丢弃。** 补完右半边后本仓直接 `PostMessage(_msgWndHandle, WM_PairedPunctuationCaretMove, ...)`，没有经过 `_QueuePairedPunctuationCaretMove`，于是 `_pendingPairedCaretFocusToken` 从未被置上，而消息处理函数只在令牌对得上时才发 `VK_LEFT`，这条消息每次都被丢掉，光标停在右半边之后。现改回来源的写法：`_PushPairedPunctuation` 记下这一对，再 `_QueuePairedPunctuationCaretMove(-1)`。

**再按右半边时不跳过。** `_TryStepOverPairedPunctuation` 与 `_PushPairedPunctuation` 都没有调用者，栈永远是空的，所以补出的 `）` 后面再按 `)` 会打出第二个。现按来源在没有组字、没有候选时先尝试跳过；哪些键能跳过由新增的 `PairedPunctuationStepOverCandidate` 判定（只认单字符的右半边，引号按键而不按翻译结果判定，`{}` 也算，因为本仓 `{` 同样会补全）。

**嵌套计数不回退。** `BalanceNestPairAfterAutoClose` 没有调用者，补过一次《》之后下一次 `<` 给的是〈。现与来源一样在补全后调用。遗留一处：候选打开时左半边由 Server 的引擎翻译，它的嵌套计数本仓 Windows Server 没有回退（`msime_client_balance_paired_punctuation_after_auto_close` 存在但需要一条新的 IPC），这次不改。

**候选打开时的小键盘标点（来源 `1d2431ad`）。** 小键盘 `+`/`-` 此前在 TSF 与 Server 两侧都被当成翻页键排除，按下后什么都不上屏；小键盘 `.` 与 `/` 则被翻译成中文标点。现在 TSF 的 `IsCandidateNavigationKeyBeforePunctuation` 只保留主键区 `-`/`=`、Tab、翻页与 Home/End，`CommitWithHighlightedCandPunc` 加入 `/`，`KeyEventSink` 先判断是否用高亮候选结束再看 `VK_DECIMAL`；Server 侧新增 `literal_candidate_punctuation`，在有组字时把这几个键交给 `msime_client_punctuation_ascii`，上屏高亮候选后接原样的 ASCII 字符。

用例：`msime-tsf-punctuation-key-policy`（跳过判定、导航键、原样字符）、`msime-tsf-paired-punctuation-wiring`（`KeyHandler.cpp` 的补全路径确实经过栈与带令牌的光标移动，且 `/` 在表里）、`windows-punctuation-policy` 补了 `literal_candidate_punctuation`。三者对着改动前的源码都会红（前两者头文件或断言失败，后者函数不存在）。本机只能跑这些不依赖 Windows 的部分，真实宿主里的光标左移与跳过没有在 Windows 上实测。

增量记录（2026-09-21，Windows 第二十批：哪些键改组字）：接上一批继续走 `src/system/` 的零覆盖模块。目标起点 `25a2325c8`。

`EditPolicy` 的 `edit_kind` 决定一个按键到底改不改组字——字母、手动分隔符、日语长音符、微软双拼的 ing 键、Unicode 模式的数字、退格与光标移动，七类规则挤在一个函数里，零覆盖。这里错一个分支就是一个输入缺陷，只在打字时看得见。

用例把七类都钉住，其中几条的**理由**值得记下：字母要求 keycode 与字符一致（字母键上挂着别的字符不算字母）；退格与方向键只在组字期算编辑，否则是应用自己的键；微软双拼的 ing 键只有当光标落在当前音节内的奇数位时才是字符，且位置从最后一个分隔符算起而不是从头算，光标越界会被夹住；预编辑样式偏好非法值**抛出**而不是退回默认，连大小写不同的 "Pinyin" 也拒。

四条反向验证会红。另两条如实记为**不会红**：`modifiers & ~1u` 与 Unicode 的 Shift+数字早退，都是同一规则的第二次声明——`translate_key` 对 Shift 以外的修饰键已返回 CancelAndForward，Shift+数字落空后本来也返回 None。作为这个函数自己的契约保留是合理的，但注释写明了，断言钉的是行为不是那两行。

这已是本轮第三次遇到同一个形状（第十九批的 `wch <= 127`、本批这两条）：一层纵深防御在它那一层是看不出是否承重的，而给它编一个「能红」的断言等于骗自己。写清楚它由谁真正保证，比让测试看起来更满更有用。

增量记录（2026-09-21，Windows 第二十一批：以词定字，以及纠正覆盖扫描本身）：目标起点 `0988f764e`。

第十四批记的以词定字覆盖在共享运行时一侧；决定「哪个键触发、取哪一端」的 `WordCharacterPolicy` 本身零覆盖。方向就是这个功能本身，而括号那一对同时又是 TSF 的翻页键，所以「什么时候**不**该触发」和「什么时候该触发」一样要紧——用例两面都钉。偏好那一侧有一条容易写反：`enabled` 为假时仍然校验 keys，因为一个写着本构建不认识的键名的配置是需要报出来的分歧，不该因为开关是关的就悄悄当成关闭。五条反向验证过。

**同时纠正一个方法错误，这一轮好几批都受它影响。** 用来找零覆盖模块的扫描只看 `platforms/windows/tests/`，于是把 `PolishPrompt` 误报成零覆盖——它是转发到 `shared/voice/PolishPrompt.h` 的薄头文件，测试在 `shared/voice/tests/`，而且覆盖得很细（槽位优先级、legacy 回退只作用于第一槽、空槽一律回落清理预设，每条还记着它修的是哪个缺陷）。改用全仓测试目录加词边界重扫后，`platforms/windows/src/` 仍无覆盖的是 24 个模块，其中绝大多数是 Win32 资源、图标字体、硬件与网络客户端这类本机测不了的东西；`VoiceHotkey` 仍在其中是对的——第十七批覆盖的是抽出来的 `VoiceHotkeyPolicy`，那个装着钩子的控制器本身依旧只能在 Windows 上验证。

记这一条是因为它改变了前几批结论的可信度等级：「零覆盖」这个判断此前是用一把范围过窄的尺子量出来的。

增量记录（2026-09-21，Windows 第二十二批：回到功能迁移本身）：本轮中段有十几批做的是审计与补测试——那是加固既有代码，不是把来源功能复刻过来。这一批起回到功能级差异：以来源 README 的功能清单为准逐条比，而不是比测试覆盖。

第一条就查出两处从未复刻的东西。来源写的是「输入统计（**默认关闭**）：只在本机记录字符数量与活跃时长，可在设置 → 统计中查看与**清理**」：

- **默认关闭**。来源 `config.default.toml` 与随包配置都是 `enabled = false`，本仓默认开启。对一个统计别人打了什么的功能，默认关闭是正确方向——应当被主动打开而不是需要被关掉。只影响全新档案。实现上有个坑：serde 的默认与 `Default` impl 是两处，文件不存在时走的是后者，改一处不改另一处就等于没改，是新加的用例抓出来的。
- **保留策略**完全缺失。来源是 `retention = forever | 30d | 90d | 180d | 365d`，每天首次写入时删除超窗记录，**非法值按 forever 处理**。这最后一条照搬了：一个本构建看不懂的偏好绝不能被当成删除更多的许可。累计总数与分类不进窗口，只裁剪每日各轴，与来源一致。UI 按来源 `stats.html` 的文案做在共享设置页，宿主不提供该能力时整个控件不渲染。

边界日期自己按历法算（Hinnant 算法），不引日期库也不过本地时区；未来日期保留而不是删掉——另一种选择是因为一块坏表丢掉真实记录。

三条反向验证会红。第四条「每次写入都清理」如实记为不会红：边界只取决于当天，逐次清理用更多功夫得到同一个结果，是开销差异不是可观察差异。这是本轮第四次遇到同一个形状，处理方式一致——写清楚，不编断言。

方法上记一条给下次：本轮前期用「测试覆盖」找缺口效率很高但方向偏了，**功能迁移要拿来源的功能清单逐条比**，覆盖率只是它的下游。


增量记录（2026-09-21，Windows 第二十二批：切到英文时组字怎么处理，以及一条自我纠错）：目标起点 `3e8eb7edc`。

先记纠错。上一批我据来源 Server 的 `ShouldResetCompositionForImeMode` → `ClearState()` 判定「离开中文态应当丢弃组字」，把 `setEnglishInputMode:` 的 `MSIME_FINISH_COMPOSITION` 改成 `MSIME_CANCEL`，结果本仓已有的 Shift 轻点用例立刻变红——那条用例钉的是「组字中途轻点 Shift 上屏的是已输入的原始字母」。读错了：Server 那次 `ClearState()` 是收到客户端状态快照后清自己的后端，TSF 侧早已处理完组字（`IsBackendIndependentCompositionResetKey` 的注释明说「TSF locally consumes these keys and completes/cancels its composition」）。**只读 Server 不读 TSF 客户端，就会把清后端误当成丢用户的字。**

去 `windows/src/` 读客户端，两条规则各自成立，而且**不是同一条**：

- Shift 的中英文切换走 `FUNCTION_TOGGLE_IME_MODE` → `_HandleToogleIMEMode`，提交 `GetKeystrokeBuffer()` 里的原始击键串。本仓 Shift 轻点先发 `MSIME_COMMIT_RAW` 再切，对得上（早前那批已修）。
- 英文输入模式开关（来源是 Ctrl+Shift+E）走 `FUNCTION_CANCEL` → `_HandleCancel`，`_RemoveDummyCompositionForComposing` + `_DeleteCandidateList` + `_TerminateComposition`，**什么都不上屏**。

本仓把第一条的做法抄到了两个入口：菜单「选择英文模式」、Ctrl+Option+Space、悬浮工具栏的切换，全都发 `MSIME_FINISH_COMPOSITION`，也就是把高亮的中文候选提交进文档。用户伸手切英文恰恰是因为屏幕上的候选不是他要的，这时候替他上屏一个没人选的词。改为 `MSIME_CANCEL`，Shift 那条不受影响——它在调用之前已经把原始字母上屏，到这里没有东西可丢。

用例钉的是行为而不是假 session 的默认应答：组字在途时切换，断言发出的是 `MSIME_CANCEL` 且客户端既没有收到 commit 也没有残留 marked text。反向验证过（改回 finish 红在 `TestControlOptionSpace`）。假 session 相应加了 `failCancel` 与 `cancelTransition`，因为「Engine 失败时不得切换模式」这条原来只能让 finish 失败。

同一批里修掉一个自己上一批（#3387）引入的回归，它是被完整 macOS ctest 抓到的，而 #3387 我只跑了 workspace 测试与 `--quick`：来源的座位表 `candidate_selection_policy.h` 每个来源只安排一个候选，因为来源那边每种只有一个；本仓一次可以拿到多个（AI 上限十条），而我把「第一个之后的」归进了本地候选——本地候选是抢第一座的，于是第二条 AI 建议被顶到空格键上，第一条反而靠后。改成按来源分组、整组落座。这条现在有 Rust 用例（`several_candidates_from_one_provider_take_their_seat_as_a_group`），在代码所在的那一层，不必等 macOS 那一侧的集成用例才发现；假引擎为此多了一个 `sources` 字段。注意用例的并行数组必须等长，否则座位表整个提前返回，什么都不验。

教训写在这里而不是留在会话里：**跨进程的功能，只读其中一侧的代码就下结论，必然读错一半。** 来源是 TSF 客户端 + Server 两半，本仓把两半合进一个 controller，于是来源里分属两侧的规则在这里看起来像一条。

（编号说明：「第二十二批」出现了两次。两条记录是同一天在不同 worktree 里并行写的，各自都已合并，谁也不比谁晚；不去改已合并记录的编号，后续从二十四继续。）


增量记录（2026-09-21，Windows 第二十四批：把功能对照变成可执行的检查）：接上一批。目标起点 `e431c8ad9`。

上一批把统计的默认与保留策略迁到共享层，但没同步到 Windows 出厂配置模板。补齐后更要紧的是把**找到它的方法**固化下来：来源的出厂配置是「那个产品能被告知做什么」最完整的一份清单（180 个键、18 个分节），它有而本仓没有的键就是一个没人迁移的功能，而别处不会有任何东西发现——设置页只渲染它知道的，偏好结构体只解析它声明的。新增 `scripts/test-windows-config-keys.py` 挂进 `--quick`，只比键名，本仓多出的不报。

当前结果：**来源 180 个键在本仓全部有对应物，本仓另有 30 个**。也就是说配置契约这一层的迁移是完整的——这比此前「逐项读过」的记录强，因为它每次 `--quick` 都会重新回答。

写这个检查时踩了两个会造成**假通过**的坑，记下来：参照检出按当前 checkout 的同级目录找，而本仓惯例是在 `~/worktrees` 下干活，于是它永远「skipped」、看起来像通过；用本地 `origin/HEAD` 定位参照修订，而那个符号引用是克隆时写一次的、这台机器上指向发布分支 `origin/main`，比默认分支少 30 个键——照它比会在上游多出 30 个键时报告「全部齐备」。当时的修法是问远端要默认分支，并打印实际 ref 和 SHA；迁移范围后来固定为 `467b9804`，现在进一步改成所有 reference 门禁只读取这个不可变对象，远端 tip 不再参与判定。

**加加辅助码这条要改一下此前的记法。** 前几批把它记为「在 Engine 里，只能提锁」，这不准确：本仓的 `engine-lock.json` 已经带 overlay 脚本机制（当前有两个），技术上完全可以再加一个把 `assets::helpcodes` 从五项扩到六项。真正的阻塞点是那张 7968 行码表本身——来源 `engine/helpcode/NOTICE.md` 写明它是从拼音加加 5.x 安装包内 `fzm.bin` **重建**的非官方数据。把它引进本仓是一次第三方数据的分发决定，而 ARCHITECTURE.md 要求引入新上游资产时连同来源提交、许可证文本、通知位置和分发限制一并提交。这该由仓库所有者定，不是实现层面能顺手做掉的事。记准阻塞点，比记一个听起来更技术性的理由有用。

增量记录（2026-09-21，Windows 第二十五批：Home/End 到的是列表两端，不是当前页两端）：目标起点 `cac0fb423`。接第二十二批的方向，继续读来源的 TSF 客户端一侧。

按来源客户端的 `KEYSTROKE_FUNCTION` 逐项对——这是一份很紧凑的功能清单，正合「拿来源的功能清单逐条比」。Home/End 在来源里分类为 `FUNCTION_MOVE_PAGE_TOP` / `FUNCTION_MOVE_PAGE_BOTTOM`（组字期与候选期两条路径都是），呈现层收到后调 `_SetSelection(0)` 与 `_SetSelection(-1)`，而 `CCandidateSessionState::SetSelection` 把 -1 读成 `Count() - 1`，末尾再 `AdjustPageIndexForSelection()` 把页跟过去。也就是说：**Home 回到整份列表的第一条、End 到最后一条，页码随之改变。**

本仓四个宿主（macOS 的 keyCode 115/119、Linux 的两套、Windows 宿主直接由 `FUNCTION_MOVE_PAGE_TOP/BOTTOM` 转发）都接到同一个共享动作上，而共享层只在当前页内移动高亮。用户已经在看着那一页，按 Home 最多挪几行，按 End 到不了列表末尾——一个几乎什么都不做的按键。改在共享层，四个宿主一起对齐。

动作名原本叫 `FirstCandidateOnPage` / `LastCandidateOnPage`，改完就是句反话，一并改名为 `FirstCandidate` / `LastCandidate`（C 常量同改，取值 104/105 不动，ABI 不变）。

一个来源那边不存在的问题：本仓的 Engine 对短查询只返回前一批候选，其余按需展开，所以 End 必须先展开再取末项，否则它停在「当时缓存里的最后一条」，再按一次还会继续往后走——那不是 End 键该有的行为。用例两条，分别钉「Home 连页一起回到第 0 条」与「End 触发展开后落在真正的末条」，都反向验证过。

顺带修掉 develop 上一处红：`typing-statistics` 这个 macOS 用例在新目录里直接 record，而 #3389 跟随来源把统计默认关成了 off，`record` 在未启用时按设计返回 0。用例先 `set_enabled` 再记，与设置页里的真实次序一致。这类红只在本机 ctest 才看得见（CI 不跑 macOS 原生门禁），所以合入前跑全套仍然是必须的。

本仓 Home/End 在候选窗未显示时移动组字光标（`MSIME_MOVE_HOME` / `MSIME_MOVE_END`），来源在那种情形下是吃掉按键什么都不做。这一条保留：它不与来源冲突，且是 macOS 上编辑光标的常规预期。

增量记录（2026-09-21，Windows 第二十六批：统计页关掉之后长什么样）：目标起点 `6d93278cf`。继续按来源的设置分页逐页比。这次是 `stats.html`——它是来源随统计功能新增的第 15 个 partial，此前几轮逐页对照都没覆盖到它。

逐节比下来只剩两处没迁：

**一、未开启时的引导。** 来源的三个状态由 `stats.ts` 切换：`statsDisabledEmpty` 在**未开启**时显示（有没有历史数据都显示），`statsNoDataEmpty` 在**已开启且无数据**时显示，`statsContent` 在有数据时显示。本仓只有后两者的等价物。前两批把统计默认值改成关闭（来源出厂就是关的），于是这个状态从边缘情况变成了**新用户的第一眼**——现在照来源补上「输入统计已关闭」一节，一句说明加一个启用按钮，放在内容之上；有历史数据时内容照常显示，跟来源一致。

顺带修掉一个因此暴露的错误文案：本仓的 `availabilityMessage` 没有「已开启」这个前提，关闭状态下会说「统计文件已建立，但当前还没有输入记录」，把用户支去多打几个字——而关着的时候打多少都不会被记录。这跟 iOS 上「没开完全访问却让人多打几个字」是同一类错误：**把「功能没开」说成「数据还不够」**。提成 `availabilityNotice()` 并以「已开启」为前提，关闭时返回空串。

**二、打开数据目录。** 来源「数据位置与隐私」一节有 `statsOpenDirectoryButton`。这一页写着「只保存在本机」，这个按钮是让这句话可以被**核对**而不是只能被相信。新增 Tauri 命令 `open_typing_statistics_directory`，复用既有的 `shared/skin_directory.rs`（那个模块本来就只认一个路径参数，不是皮肤专用）；目录由宿主给，webview 指定不了路径。按钮按能力裁剪：`TypingStatisticsClient.openDirectory` 是可选的，iOS/Android 不注入，于是那两个宿主上渲染不出按钮，而不是渲染一个按下去报错的。来源不显示路径文本，本仓也不显示，省掉一条把绝对路径送进 webview 的通道。

来源的 stats.html 其余分节（启用开关、输入速度、日历热力图、今日时段分布、字符分类、按日明细、数据清理的保留策略与清空）在前两批已经迁完。

验证：`--quick` 全绿。其中 harmony 设置包陈旧那道门禁再次生效——HarmonyOS 的设置窗口用的是仓库里预构建的包，改共享 UI 不重新生成，它就会比别的宿主少一截界面，这次它拦下了。本地 `msime-desktop` 此前一直因为缺 `target/macos/` 下的两个产物路径而 `cargo check` 不了（新增 Tauri 命令就没法编译验证），在 `target/` 下补上空目录后可以正常检查了——那是构建产物目录，不进版本库。

**同一批里顺手修掉一个把新检出全部卡住的东西。** 开这个 worktree 时 `--quick` 报 `vendor/MSIME-Engine could not be prepared from engine-lock.json`，栈顶是 `fetch_engine.py` 里的 `DEST.parent.mkdir(parents=True, exist_ok=True)` 抛 `FileExistsError`——`exist_ok=True` 只原谅「已经是目录」，不原谅「存在但不是目录」。

按规矩先查自己：`vendor` 是一个**自己指向自己的绝对路径符号链接** `vendor -> /Users/e-hu/Workspace/oss/ime/msime/vendor`，而且它是**被提交进仓库的**——`git log -- vendor` 指向今天 07:59 的 `dfbfa1c6f`（macOS 英文模式那一批），那次 `git add` 把一个本不该跟踪的条目扫了进去。也就是说每个新克隆、每个新 worktree 检出的都是这个坏链接，引擎根本准备不起来；老的 worktree 之所以没事，是因为它们的引擎在那之前就准备好了。报错信息指向的是「目录创建失败」，离「有人提交了一个符号链接」很远，所以它值得一道门禁而不是一次修复。

新增 `scripts/test-tracked-symlinks.py` 挂进 `--quick`：被跟踪的符号链接必须解析到仓库**之内**，绝对路径一律报错。从 index 读记录的目标而不是走工作树——重点本来就是「记进去的那个字符串」。反向验证：在仍带该链接的主 worktree 里跑是红的（`FAIL vendor -> /Users/…/vendor: an absolute path`），移除后绿。写它时又差点栽在同一个坑里：`ROOT` 取自脚本自身路径，于是在别的目录下跑它其实还在查脚本所在的那个 checkout，第一次「反向验证」得到的是假绿；把脚本复制进主 worktree 再跑才拿到真红。

**移除本身是并行的 #3393 先落地的**，这里记准：本批推上去时 develop 已经带上了那次修复（连同把整个 `/vendor/` 加进 `.gitignore`，比本批原先只去掉尾斜杠的写法更彻底，合并时取了它）。同一个坏链接被两条线各自撞上，恰好说明为什么它需要的是门禁而不是又一次修复——本批留下的是门禁那部分。

增量记录（2026-09-21，Windows 第二十七批：半截词该待在组字里，而不是先进文档）：接第二十五批继续读来源的 TSF 客户端。目标起点 `6d93278cf`。

候选只吃掉部分输入时，引擎会继续组字，并把选中的那一段交回宿主。来源把它留在组字里：`word_for_creating_word` 拼在读音前面显示，光标按它的长度前移（`preeditPrefixLength`），回车时把 `word_for_creating_word + 剩余原始输入` 作为一条提交出去。本仓是**立刻上屏**——用户还在打后半截，前半截已经进了文档：搜索框会拿半个词去搜，编辑器为它记一次撤销。

用真实词库探针确认了两侧行为（`crates/input-runtime/examples/phrase_progress.rs`）：`haitanpaobu` 选「海滩」后，本仓提交「海滩」、继续组 `paobu`；第二次选词只提交「跑步」。所以改成持有之后，两段要合成一条提交。

改在共享运行时。几条边界各自有理由，都钉了用例：

- **取消**丢弃已选的段，与来源 `_HandleCancel` 清 `word_for_creating_word` 一致。
- **失焦**把已选的段上屏。来源那边组字里的文本在 TSF 终止组字时留在文档里；本仓失焦本就取消组字，而在改动之前那一段早就在文档里了——失焦丢掉它是净损失。
- **退格把剩余读音删光**时上屏已选的段。这一条是有意不跟随来源：来源会继续显示「海滩」而读音为空，而本仓的不变式是「持有前缀 ⇒ 组字非空」，宿主里十几处 `[_view[@"editing_text"] length]` 的「是否在组字」判断才不用动。（**2026-09-21 起这条只在「没有可回退的选词」时才走到**：有选词记录时退格先触发下面那批补的回退规则，见该批。）
- **不是选词产生的提交**（例如标点结束组字）不进前缀，否则会被塞到无关文本前面。候选页上按数字选词算选词。

前缀作为 `view.phrase_prefix` **单独一个字段**交给宿主，而不是拼进 `editing_text`：`caret_position` 是进入那串文本的偏移，而各宿主按自己的字符串单位读它（Apple 是 UTF-16，其余按字节）——此前两者一致只是因为编辑文本全是 ASCII，前缀一旦是汉字就不再一致。宿主自己拼、自己按自己的单位移光标。

按宿主分期打开（`HostOptions.phrase_preedit`，默认关=旧行为）。本批只开 macOS：本机能跑完整 ctest，而 Linux/Harmony/Android 三个宿主本机既没有容器也没有工具链，仓库又没有 CI，盲改等于没验。不开的宿主一个字节都没变。

（**紧接着的核对推翻了这句里关于 Windows 的部分**：本仓 Windows 宿主**已经**在自己的 IPC 层做了同一件事——`ReplyComposer` 把共享运行时的增量选词结果累积成整个已选前缀，选词后还有剩余输入时发 `NeedToCreateWord`，全部完成后发含完整前缀的 `Normal`，TSF 侧则沿用来源的 `word_for_creating_word` 显示，并额外有 `_creatingWordRestoreHistory` 在退格跨过边界时恢复上一段。它这样做是因为那半边说的是旧 DLL 的协议。所以 Windows **不该**打开 `phrase_preedit`，打开会双重累积。真正还没对齐的是 Linux 的两套前端、Harmony 与 Android——它们逐段上屏，等能验证时再打开。）

增量记录（2026-09-21，Windows 第二十八批：把「功能是否迁完」从记忆变成每次重新回答的问题）：目标起点 `b3a55e80f`。

上一批逐页比到 `stats.html` 之后，按文件名继续往下走已经开始失真：`tools-settings.html` 的九项（剪贴板、K/T/U/E/M/J/Y/R 模式）在文档里一次都没被点名，实际早就齐了——`localModeRows` 八行加剪贴板，`super_jianpin` / `temporary_english` / `temporary_japanese` 只是换了个比 `jianpin_mode` / `y_mode` / `r_mode` 更说明问题的名字。按文件名统计覆盖率会把这种情况报成缺口。

换一条真正机械的轴：来源 `engine/contracts/webview/messages.json`。那是它**自己生成**的一份清单，列出四个界面（设置、悬浮工具栏、候选窗、托盘菜单）能向宿主请求的全部动作，共 46 条。它比任何一次逐页阅读都可靠，因为来源新增一项能力时这份文件会跟着长——`statsRequest` 多出 `openDirectory` 就是这么来的，而本仓直到有人重读那一页才发现。

逐条对完的结论：**46 条全部有对应物，其中 13 条是有意没有对应物的**。有意没有的分三类：来源在 WebView2 文档里自绘标题栏，于是要向宿主请求拖动、缩放、命中测试、窗口按钮和最大化按钮矩形（`dragStart` / `resizeStart` / `resizeHitTest` / `windowControl` / `maximizeButtonRect` / `focus`）——Tauri 窗口是带系统装饰的真窗口，这些请求没有对象；来源的候选窗是 WebView2，分不清真实鼠标移动和「窗口在静止鼠标下被移动」时合成出来的那一次，所以有一整套 arm/probe 仪表（`candidatePointerArmed` / `candidatePointerMotion` / `candidateFrameProbe` / `candidatePointerProbe` / `contentTruncated`）——本仓是 Direct2D 原生窗口，收到的就是真实消息，没有要消歧的东西；`copyText` 和 `ready` 属于「文档能力不足」的补偿，本仓的 webview 有剪贴板 API，原生工具栏创建出来就是 ready。

几处值得记下的对照结果：来源工具栏是**七**项，比本仓配置里的六个开关多一个 `language`（中/英/日）——查下来本仓 `FloatingToolbarWindow.cpp` 把 language 无条件画出来，配置注释也写着「中英文切换始终显示」，是一致的；托盘菜单七项逐项相同；候选窗右键的置顶 / 固定排位 1–5 / 取消固定 / 删除四项都在 `CandidateMenuLayout.h` 里；`importHans`（批量导入纯汉字词组到拼音词库）本仓并进了导入格式自动识别（标准 / Windows TSV / Rime / 纯汉字自动注音），是适配不是缺失。

新增 `scripts/test-reference-ui-actions.py` 挂进 `--quick`。它不能像配置键那样按名字比——来源那四个面是往宿主发消息的 WebView2 文档，本仓有三个面是原生代码、根本没有消息——所以每条动作映射到「本仓答复它的那个 token」，或者记成有意缺席并写明理由。来源有而这张表没有的动作就是发现：一项在没人看的时候到岸的能力。

反向验证两条路径都走过：把 `statsRequest` 从表里删掉，它报 `FAIL statsRequest (settings)`；而「token 写错」这条不是模拟的——第一版把选词和关闭右键菜单分别写成了 `MSIME_SELECT` 和 `close_menu`，两个在本仓都不存在，检查当场报红，逼我去源码里找到真名 `select_candidate` 和 `CandidateFlyoutWindow`。这正是它该有的行为：映射表写得对不对，由仓库本身来判。

附带一条环境结论，写进 AGENTS.md 免得下次再烧一小时：**`vendor/MSIME-Engine` 必须是真实目录，不能是符号链接。** 多个 worktree 共用一份已准备好的引擎，最自然的做法是链过去，但 `crates/engine-bridge/native/bridge.h` 用的是 `../../vendor/MSIME-Engine/common/...` 这样的相对包含，编译器得从 `vendor/MSIME-Engine` 用 `..` 爬回仓库根才能找到它——而 `..` 跨过符号链接后去的是**物理**父目录。链到 `~/.cache/...` 时它爬到了 `~/.cache/`，头文件当场没有。之前之所以侥幸没事，是因为链的目标那一侧恰好也有一个真实的 `vendor/`，`..` 爬上去正好落在它上面。省空间用硬链接复制（同一文件系统 `cp -al`）而不是 `ln -s`。

增量记录（2026-09-21，Windows 第二十九批：第二次启动应该把窗口拉到前面，而不是什么都不做）：目标起点 `6b4b138ae`。

上一批把上游界面动作清单核对完之后，换到**源文件层**继续找：上游 `server/src/` 共 160 个 `.cpp`/`.h`，按名字映射下来有 67 个在本仓没有同名对应物。多数是有意改名或并进 Rust（`cloud_ime` → `CloudCandidateWorker`、`tencent_tmt` → `credential/translation.rs`、`candidate_size_estimator` → `CandidateRowFit` 等），逐个看过去，真正有缺口的是 `single_instance`。

上游用它守四个进程：Server、设置窗、emoji 面板、手写面板、屏幕键盘面板。本仓 Server 和 watchdog 都有对应的互斥量（`server_main.cpp`、`Watchdog.cpp`），面板这一侧是 Tauri 窗口，`open_panel_window` 本来就复用已有窗口并 `set_focus`，等价物在。**但设置窗这一侧漏了一半**：上游第二次启动时，带 `--about` 就把「打开关于」这个意图转发给已在运行的窗口，不带参数则 `SetForegroundWindow(existing)`；本仓的 single-instance 回调写成

```rust
if let Some(route) = launch_route_from_args(&args) { ... }
```

**没有 route 时整个分支被跳过，什么也不发生。**

这不是个理论问题：本仓所有由产品自己拉起的界面都带 `--route=`（Windows 侧在 `ShellLauncher.cpp:35` 拼这条命令行），所以「无参数启动」这条路径**只**在用户自己点开始菜单项、快捷方式或安装后的图标时发生——也就是普通人打开设置的主要方式。表现是：设置窗已经开着但被别的窗口盖住，再点一次图标，毫无反应。

改法是把 Option 从调用处消掉：新增 `second_launch_route()` 返回 `SurfaceRoute` 而不是 `Option<SurfaceRoute>`，没有显式 route 时回落到 `SurfaceRoute::Settings(None)`——那条路径本来就已经做了 `show()` + `unminimize()` + `set_focus()`，缺的只是走到它。

**这里的保证来自返回类型，不是来自测试，如实记一下。** 新增的用例钉的是「回落到哪个 surface」（空参数、只有可执行文件路径、`--route=` 解析失败三种输入都回落到设置窗；显式 route 仍然优先），但它盖不住调用处——谁要是把 `if let Some(...)` 写回去，用例照样绿。真正让这个缺陷无法复发的是函数签名不再返回 `Option`，调用方没有可丢弃的东西。写守卫去检查调用处长什么样只会脆，不如把类型摆对。

macOS 对应同一份 `settings_launcher.cpp` 语义，但只对设置窗生效：设置启动（`settings`、`settings:<分类>`，以及 Finder / 启动台不带 route 的启动）共用一个 `app.msime.macos` 进程，`tauri-plugin-single-instance` 在 macOS 上是 `/tmp` 下的 Unix socket。第二次设置启动把自己的 `--route=` 转发给已在运行的设置进程后立即退出，运行中的窗口 `show()` + `unminimize()` + `set_focus()` 并经 `settings-route` 切到请求的页面，所以菜单里的「水杉输入法设置…」「关于水杉输入法…」「检查更新…」都是把已开的窗口拉回前台并换页，而不是再开一个窗口。面板启动（屏幕键盘、emoji、手写、语音 / 面板会话、云剪贴板、云词典）不注册这个插件，仍是每个输入会话一个进程：它们的环境里带着会话身份和配置路径，面板关闭时进程随之退出。IMK 侧 `DesktopSettingsLauncher.h` 对所有 route 都保留 `createsNewApplicationInstance = YES`，否则 LaunchServices 会直接激活任意一个正在运行的实例（可能是隐藏的面板进程）并丢掉参数；这个短命的第二进程正是把 route 送到已开窗口的载体。判定在 `macos_settings_launch()`，由 `macos_single_instance_admits_only_settings_launches` 覆盖。

增量记录（2026-09-21，Windows 第三十批：等云候选该等多久）：沿对照表「谷歌云候选与 AI 联想」一行里「超时、取消、失焦后旧结果」逐项比。目标起点 `6b4b138ae`。

来源给云候选的预算写在 `cloud/cloud_request.cpp`：连接 2000ms、总计 2500ms；防抖 `kIdleDelay` 500ms。防抖本仓对得上（macOS 的 `_cloudTimer` 0.5 秒、Linux 的 `online_delay_source`）。**预算不对**：macOS 给 2 秒，本仓 Windows 宿主给连接 2000/总计 2000。也就是说一条在 2.0–2.5 秒之间返回的云候选，来源会显示，本仓丢掉——而这事在慢网上很常见，因为请求本来就要等打字停顿 500ms 之后才发。

丢掉之后**看不出来**：没有云候选的查询和被超时砍掉的查询，在屏幕上长得一模一样。

根因不是某个宿主写错了数，而是这个数在共享层根本没有声明：每个宿主用自己的网络库（Apple 的 NSURLSession、Windows 的 libcurl），各写各的。现在在 `client-core` 的 `cloud::candidates` 里声明一次，C 头文件里给 C++/Objective-C 宿主同一对常量，两个宿主都改读它，并加 `scripts/test-cloud-request-budget.py` 挂进 `--quick`：声明与来源的数对齐、两个宿主都读常量、且各自那个请求的初始化里不许再出现字面量。反向验证过（把 macOS 改回 2 秒，脚本两条同时变红）。

一处如实记下的限制：NSURLSession 没有单独的连接超时，只有请求/资源两个。总预算 2500ms 能照搬，连接那 2000ms 是它的子集，代码注释写明了这一点，脚本也只要求 macOS 读总计那一个常量。

脚本里那条「不许写字面量」的检查限定在云候选那个初始化方法内：同一个文件里翻译和 AI 的初始化各自也设 deadline，但那是在校验共享层下发的 descriptor 里声明的值（`timeout_ms`、`connect_timeout_ms`），是宿主在守约而不是自己发明数——第一版没限定范围，把这三处全报成了缺陷。

增量记录（2026-09-21，Windows 第三十一批：窗口在页面画出来之前是什么颜色）：目标起点 `a822b2f3c`。接上一批，继续按上游 `server/src/` 的文件名往下找。这次落在 `settings_splash` 和 `emoji_panel_splash` 上。

上游为什么要写这两个文件：WebView2 窗口一创建就在屏幕上，而文档要等 bundle 加载完才有东西可画，中间那段空白由平台按默认色绘制——Windows 上是白的，不管用户用的是什么主题。它的办法是一个跟设置窗边框对齐、跟着 DWM 圆角走的 Direct2D 浮层（`SettingsSplash::Show(owner, light)` 带主题参数），在 `ContentLoading` 时撤掉；**并且在控制器创建失败、导航失败等每一条失败路径上也撤掉**，免得 WebView2 起不来时浮层永远糊在那里。

本仓这边窗口在 `tauri.conf.json` 里声明，既没有 `visible: false` 也没有 `backgroundColor`，所以打开设置会先闪一下系统默认底色。适配而不是照搬：不必再糊一层浮层，直接把窗口底色设成页面**马上要画的那个颜色**——共享样式表的 `--chrome-bg`（深 `#202020`，浅 `#f3f3f3`）。这样那几帧里没有东西可看，而不是有个白框。主窗口在 `setup` 里按 `window.theme()` 设（窗口由配置创建，这是第一个能拿到主题的时机）；面板窗口在 `WebviewWindowBuilder` 上设，主题从设置窗读——上游也给面板各写了一个 splash，同一个问题。

比「隐藏到页面就绪再显示」稳：那条路一旦 bundle 起不来，用户连个能关掉的窗口都没有，正是上游要在每条失败路径上撤浮层所防的事。给底色没有这个失败模式。

`--chrome-bg` 现在有两份（CSS 一份、Rust 一份，窗口底色读不了 CSS）。新增用例直接读 `packages/ui/src/styles.css` 比对两个值，反向验证过：把常量改一位，用例报 `left: "#212020" right: "#202020"`。这种重复漂移起来的症状是「开窗时闪错一帧颜色」，没人会为它提 bug，所以值得一道用例盯着。

顺手修掉 develop 上一处红：`typing_statistics_status_reports_file_availability_without_content` 断言新建文档 `enabled: true`，那是 #3389 跟随上游把统计默认翻成关闭之前的值。这个用例的主题是可用性上报，`enabled` 只是顺带，改成出厂默认；后半段原本设 `false`（翻转后等于没动），改成设 `true`，「开关会变」这层覆盖才还在。这种红只有在本机跑 `cargo test -p msime-desktop` 才看得见。

增量记录（2026-09-21，Windows 第三十一批：把第三十八批欠下的那条覆盖补上）：目标起点 `a26aca7e3`。

2026-09-20 的第三十八批核对结论是「本地优先级实现正确、但零测试覆盖」，并写明正确做法是把合并判据抽成纯函数再钉住，同时记下**本批没有做**的理由：当时本机 Docker 已停，Windows 套件跑不起来，不做无法验证的改动。

现在能验证了，而且不靠容器：`scripts/test-windows-native-run.py` 会用宿主编译器编译并**真正执行** `platforms/windows/tests/` 里不依赖 Win32 的那些源文件，纯策略头的用例正属于这一类。所以把那条判据抽进 `CandidateTranslationPolicy.h` 的 `untranslated_texts`，`TranslationWorker` 改为调用它（json 解析仍留在 worker 里），新增 `candidate_translation_merge.cpp`，quick 门禁里的原生执行数从 64 升到 65。

判据本身钉了六条：已有本地释义的候选不再问云端；没有的按计划顺序全问；同一文本在计划里出现两次只问一次；**释义为空的条目不算已答**；别的文本的答案不顶替本条；计划里的空文本直接丢弃。

第四条如实记为「本仓当前的生产者打不到」：共享的候选释义请求在返回前就把空释义过滤掉了（`translation.rs` 的 `filter_map`），所以 worker 那一侧看不到空释义。它是这个函数自己的契约而不是第二道防线，注释写明了，反向验证也确认这条断言在**函数被改坏时**会红（把判断改成只比文本，第 51 行立刻失败）。这已是本轮第四次遇到「纵深防御那一层看不出是否承重」的形状，处理方式与前三次一致。

顺带一提：worker 本体仍只有交叉构建这一级证据（它要 curl 和 Win32，宿主上跑不起来），这一点没有变化，本批也没有声称它变了。

增量记录（2026-09-21，Windows 第三十二批：上游发过的每一个功能都被看过）：目标起点 `a26aca7e3`。

这一轮把三条轴都走完了，结论先写在前面：**按上游 README 的功能清单逐条核对，只剩加加辅助码一项没有对应物**，而那一项的阻塞点已经记在第二十四批——7968 行码表是拼音加加 5.x 的非官方重建数据，引不引进来是仓库所有者的第三方数据分发决定。其余全部有对应物：全拼 / 双拼四套 / 86 五笔、日文罗马字与 R 模式、五种辅助码、云候选与 AI 联想、竖排候选窗的中英互译与腾讯云翻译、混输与独立英文候选、词库管理、语音输入、手写 / 屏幕键盘 / 悬浮工具栏 / 剪贴板历史、默认关闭的输入统计、K/T/U/E/M/J/Y/R 八个模式、智能标点与成对标点与以词定字与简繁转换、候选外观由全局主题提供（`skin/theme.rs` 的 `GlobalTheme`：`system`、`shuishan`、`light`、`paper`、`night`、`ink`、`custom` 七个 id，取代来源的 fluent/wechat/graphite/willow_green 四套皮肤）。

这一批具体核对过、本仓已有而此前文档没点过名的几项：上游安装器那页「联网功能」同意页本仓 `msime_setup.iss` 423–475 行完整有（我一开始从「没有 `[Tasks]` 段」推断它缺，是错的——上游根本没用 `[Tasks]`，用的是 `[Code]` 里的 `CreateInputOptionPage`）；`MAX_PINYIN_LENGTH` 本仓在 `tsf/Key/KeyEventSink.cpp` 里用得比上游还多（延迟按键预算、shadow 原始输入、上报输入长度三处都夹了）；语言栏深浅色图标在 `LanguageBar.cpp` 的 `IsSystemDarkMode` / `ResolveThemeIconIndex`，资源比上游多一对 `jp-*.ico`；候选窗「隐藏」的拥塞宽限期连常量都一样是 24ms（`CandidateMailbox.h` 对 `internal_late_event`）；词条导入的默认权重 10000 与 Rime YAML 头跳过都在 `dictionary/import.rs`。

**新增第三道检查 `scripts/test-reference-feature-log.py`。** 前两道分别是「上游能被配置成什么」（180 个键）和「上游界面能请求什么」（46 个动作），但两者都漏掉**不新增键也不新增消息、只改行为**的功能——机器卡顿时候选窗不再频闪、拼音缓冲加了上限、安装器不再默认静默联网，这三件事没有一件会让前两道检查变红。上游的 CHANGELOG 会记下它们，因为那是从它自己的提交生成的：一个 `feat:` 提交就是一条 bullet。

于是这道检查记住「已经读过的 bullet」以及**每条落在本仓哪里**，遇到没人看过的 bullet 就报红。重点不是每个功能都必须存在——有几条是发布流程、不属于产品行为——而是没有一条会在没人注意的时候溜过去。表里刻意要求写具体位置：写「已经支持」这四个字就等于把这道检查作废了。

反向验证：把「拼音输入长度上限」那条从表里删掉，它报 `FAIL set max len limit for pinyin input`。当前 19 条全部有记录。

增量记录（2026-09-21，Windows 第三十三批：六处核过是对的，以及一处真缺的覆盖）：目标起点 `1c2ec8197`。本批以核对为主，逐条记下判据，免得下一轮重查。

**候选行的拼接顺序。** 来源 `candidate_view_model.h` 的 `CandidateViewHtml` 是「文本 + 注解 + 角标」拼成一段，译文单独一段，固定位整段着 `#379AD3`。macOS 的 `CandidateDisplay` 同序（文本 → 注解 → 角标 ☁️/🤖），译文与固定位颜色也已各自对齐。macOS 多出的 `*` 纠错标记在 2026-09-20 的记录里已写明是本仓多出的一项（引擎新版本才有 `candidate_corrected`），不是错位。

**英文候选的两个上限。** 来源 Server 写死 `kMixedCandidateLimit = 5`、`kDedicatedCandidateLimit = 1000`、混输最小前缀默认 2。本仓不走那条 Server 路径，而 Engine 自己的 `candidate_queries.cpp` 就是 `query_prefix(prefix, 5)` 与 `query_prefix(prefix, 1000)`——同样的两个数，因为来源那边本来也是照着引擎写的。最小前缀 2 已由 `--quick` 的默认值脚本钉住。

**小键盘数字选词。** 来源在 Server 边界把 `VK_NUMPAD0..9` 归一成数字键（`NormalizeNumpadDigitKey`）。macOS 的 `PhysicalCandidateDigitSlot` 已经把 83–92 映成候选 1–9，注释直接引了 Windows 的归一化，且不接受小键盘 0；小键盘的标点键另有一张表。

**皮肤目录。** 来源 `IsBuiltIn` 是 fluent/wechat/graphite/willow_green 四个，`IsSafeId` 是「首字符字母数字、整体只允许小写字母数字与 `. _ -`、上限 64」。本仓 `skin/catalog.rs` 的 `safe_id` 与 `IsSafeId` 逐条相同；内置 id 是有意的分歧：本仓没有内置皮肤，内置配色在 `skin/theme.rs` 的七个全局主题里，`skin/catalog.rs` 只经 `is_reserved` 把这七个 id 保留下来，外部皮肤包不能占用，来源的四个内置 id 在这里没有特殊含义。

**用户词库日志重放。** 来源有一个独立 exe 走 Engine 的 `user_dictionary::replay`，把日志重放进新装词库。本仓同样有这个工具（`crates/engine-bridge/src/bin/MetasequoiaImeDictionaryReplay.rs`），只是 macOS 的词库更新不走它：走的是快照 staging，把用户词条、固定位、选择记录成流写进新一代（`stage_dictionary_state`）。两条路都保住用户数据，形态差异，不是缺口。

**AI 那一半的时序。** 来源防抖 650ms、连接 2500ms、总计 8000ms，命中缓存时跳过防抖直接出；缓存键是 provider + endpoint + model + 分段拼音。macOS 的 `_aiTimer` 是 0.65 秒，`client-core/src/ai.rs` 的 descriptor 是 2500/8000，`MSIMEAICacheKey` 是同样四项，命中时也是直接 apply 不等防抖。唯一差别是本仓给缓存加了 4096 条上限而来源不清（只在 Stop 时清），方向是更严。

**Linux 的 AI 时序后补对齐（2026-09-23）。** 上一段只核了 macOS，Linux 当时并不一致：IBus 与 Fcitx5 两个引擎把云候选和 AI 共用一个 500ms 防抖，provider 的 AI 请求总计 7 秒且不单独限连接，缓存命中也要等满防抖。现在两个引擎分开计时（云 500ms、AI 650ms）；输入一变就先向 provider 发一次只查缓存的探测（`OnlineQuery.ai_cache_only`，provider 在这种请求上连云候选也不查、未命中也绝不出网），命中立刻上屏，未命中才在 650ms 后发真请求。provider 的 AI 请求改为总计 8 秒、连接 2.5 秒（`ConnectBoundedHTTPSConnection` 只在握手阶段用 2.5 秒，读响应仍是 8 秒），引擎侧等 AI 回包的 socket 期限相应放到 9 秒，多出的一秒留给子进程启动，云候选与缓存探测仍是 500ms。顺带修掉设置页「润色测试」永远失败的问题：它向 HTTP 子进程要 8 秒，而子进程的上限写死 7 秒、超限直接拒绝，于是每次都返回空；原有用例全部 mock 了 `fetch`，走不到这道校验，所以一直没被发现。现在上限与调用方共用 `AI_REQUEST_TIMEOUT`，`ai_candidate_cache.py` 用真实的子进程校验逻辑跑一遍候选请求与润色测试，`scripts/test-cloud-request-budget.py` 把 Linux 的这几个数钉在 `client-core/src/ai.rs` 的 descriptor 上。

**本批唯一真动的地方**：macOS 在建会话时额外请求 `phrase_preedit`（第二十七批加的），此前没有任何用例钉住——这个请求一旦丢了，半截词会退回逐段上屏，而那看起来就是普通打字，没有别的东西会发现。把这一步抽成 `MSIMESessionOptions` 并在 `ShortcutTest` 钉住：请求被加上、文件原有的键原样透传、不修改调用方的字典、以及「文件里写着 `phrase_preedit: false` 也不作数」——旧文件根本早于这个行为。反向验证过。

增量记录（2026-09-21，Windows 第三十四批：给「半截词」的分期推广加一道守卫）：目标起点 `d49536d18`。

第二十七批把「半截词留在组字里」做进共享运行时，并按宿主分期打开，本批给这个分期状态加一道静态守卫。理由是这件事**做一半会无声丢字**：宿主要是请求了 `phrase_preedit` 却不画 `view.phrase_prefix`，用户已经选中的那一段既不在文档里也不在屏幕上，而组字在他按 Esc 之前不会结束——那一段就这么没了。反过来只画不请求则是死代码。

`scripts/test-phrase-preedit-hosts.py` 按宿主判定，并**把共享渲染器与宿主自身的源码分开**：Apple 的 `TextClient.mm` 同时服务 macOS 与 iOS，而这两个宿主今天不在同一边，所以「共享渲染器能画」不构成任何单个宿主的证据。当前状态被钉为：macOS 持有并绘制，Linux / Windows / HarmonyOS / Android / iOS / 桌面壳逐段上屏。

两个方向都反向验证过：给 Linux 加一行请求而不加渲染会红；把 macOS 两处读取（共享渲染器与候选窗标签）同时去掉也会红——只去掉其中一处不会，因为另一处仍在画，这正是判据要的语义。

第一版被注释骗过：`InputController.mm` 的注释里写着 `view.phrase_prefix`，删掉真正的读取之后守卫仍判为「在画」。改成先剥掉 `//` 与 `/* */` 再匹配——被删掉实现、只留注释，恰恰是这个守卫最该抓的形状。

记下 iOS 的位置：它用的就是那份共享渲染器（`KeyboardViewController.mm` 走 `MSIMEApplyTransition`），所以接上只差会话选项里的一行；但本机跑不了 iOS 用例，不做无法验证的改动，按现状钉住。

增量记录（2026-09-21，Windows 第三十五批：把最后一项迁过来——加加辅助码）：目标起点 `2569cb526`。

第三十二批的结论是「按上游 README 功能清单逐条核对，只剩加加辅助码没有对应物」，并把它记成「等仓库所有者做第三方数据决定」。**那个决定已经有了**：仓库所有者明确要求完整复刻，本批照此执行。该做的不是替他决定，而是把决定所需要知道的事实摆清楚，然后把活干完——这两件事都做了，见 `resources/helpcodes/NOTICE.md`。

**为什么不能提锁。** 前几批写过「可以加一个 overlay 把 `assets::helpcodes` 从五项扩到六项」，但没说清另一条路为什么不通：`git ls-remote metasequoiaime/msime-engine HEAD` 回来的就是 `engine-lock.json` 钉的那个 `f611f2ff`。引擎已经并进上游主仓（README 的「引擎在仓内」），独立引擎仓库停在并入那一刻，而加加是 2026-09-20 的 `566ff8b8` 加进上游 `engine/` 的——**没有更新的引擎提交可以提**。所以码表只能由本仓携带。

**注册只需一条 asset 条目。** `HelpcodeUtils::load_helpcode_keymap` 按 `entry.schema == schema` 在 `metasequoia::assets::helpcodes` 里找，`is_supported_helpcode_schema` 查的是同一个数组，所以一条条目同时决定「能不能加载」和「算不算合法」。`contracts/assets/generate.py` 把契约生成那个数组，`product.py` 又从同一份契约派生打包文件清单——于是 overlay 改 `assets.json` 之后跑引擎**自己的**生成器，而不是手改生成出来的头文件再祈祷三者一致。overlay 会断言生成结果里确实出现了 `{"jiajia", helpcode_jiajia}`。

端到端验证过：删掉 `vendor/MSIME-Engine` 重跑 `scripts/fetch_engine.py`，归档按锁文件的 sha256 校验后展开、overlay 生效、数组从 5 变 6、7968 行的表在位。重复执行幂等（引擎将来自带这套方案时整个 overlay 原样退出，不覆盖它自己的表）。

**改到的地方是一份有界清单**，一次改完：共享偏好的 `HelpcodeSchema` 枚举与 `as_str`、共享 UI 的类型与标签表（标签用上游的「加加」，排在最后）、engine-bridge 的方案遍历、Windows 出厂配置两行注释、macOS 的三处方案列表与弹出菜单标题与 `NormalizeHelpcodeSchemaPreference` 的上界（5→6，漏掉它会让第六项被静默归零成蓝天）、Linux 的 fcitx5 与 IBus 两处列表、标签与校验链。

**证据强度如实记。** `helpcode_settings_reach_the_real_engine` 现在把 `jiajia` 也跑了一遍真实引擎会话，但它的 `resources` 是空临时目录——它证明的是**方案被注册并被接受**（未注册的方案会让 `Session::new` 直接失败，用例里的 `"unknown"` 就是这么断言的），不是码表内容被读到。而这套方案恰恰是唯一一张**不随锁定归档一起来**的表，也就是唯一可能丢失、被坏合并截断、或存成引擎读不出内容的编码的那张。所以另加一个用例，按引擎 `load_helpcode_keymap` 的解析规则（首个 `=` 切开、取其后两个字符、两个都必须是 `a`-`z`，否则**静默丢弃**）读本仓携带的这张表，断言条目数 7968 与通知里引用的几个码（好=nz、你=de、中=ks、国=ky），并要求没有任何一行被静默丢弃。反向验证：把 `好=nz` 改成 `好=NZ`，它报 `the Engine drops these silently: ["好=NZ"]`。

真正的候选过滤在这一层验不了——那需要真实词库，而这些单元测试跑在空目录上。macOS 那几处改动本机也编不了：`platforms/macos` 的 CMake 配置要求 Sparkle 2.9.6，这台机器上没有，整场会话里 macOS 这一级都是 skipped；只对能独立编译的两个头文件做了 `-fsyntax-only`。`--quick` 全绿（其中 harmony 预构建包那道门禁又一次因为共享 UI 改动而拦下，已重新生成）。

同一批里修掉一个自己踩出来的门禁坑：`scripts/verify-local.sh` 判断 macOS 那一级要不要跑，用的是「`target/macos-isolated` 目录是否存在」。我为了在本机跑 macOS 用例去 configure 了一次，因为缺 Sparkle 2.9.6 而以 `FATAL_ERROR` 失败——但 CMake 在报错之前已经写下了目录和 `CMakeCache.txt`，于是门禁认为「已配置」，随后的编译与 ctest 两级一起变红。判据改成「有没有生成出构建系统文件」（`build.ninja` / `Makefile`），那是只有走完的 configure 才会写的东西。两个方向都验过：只有 `CMakeCache.txt` 时跳过，有 `Makefile` 时才跑。

增量记录（2026-09-21，Windows 第三十六批：两处新行为的交叉情形，以及候选窗锚点核对）：目标起点 `1b9e3d86d`。

**成对标点与半截词同时在场。** 两者都是同一段 marked text 的尾巴/头部，而且分居光标两侧：待补的右括号必须留在最后（用户要看见将要补上的是什么），已选的半截词必须留在最前（那是已经定下来的文字），光标在两者之间——打字从那里继续。#3380 与 #3395 各自有用例，交叉情形没有。补在 `TextClientTest`：组字中断言 `海滩paobu）` 且光标在 7，结束时整条词连同右括号一次上屏。反向验证过（把前缀改成追加而不是前置，第 453 行变红）。

**候选窗锚点。** 来源 `GetCandidateLayoutCaret`：跟随光标关掉时，用本次组字会话开始时捕获的锚点（`g_candidate_session_anchor`），捕获一次后整段会话复用；跟随光标打开、或者报上来的 y 是 `INVALID_Y` 时走实时光标。macOS 的 `_candidateAnchorValid` / `_candidateAnchorCaret` 逐条相同，并且在跟随开关本身变化时重置锚点（`MSIMEValidCaret` 对应那个无效值判断）。核过，不是缺口。

增量记录（2026-09-21，Windows 第三十七批：把「完整」落到文件级的记账上）：目标起点 `ac48a74ba`。

此前三道检查都是从**外面**看来源：它能被配置成什么（180 个键）、它的界面能请求什么（46 个动作）、它发布过什么（19 条 changelog bullet）。三道都答不了迁移真正要回答的那个问题——**它的源码树里还有没有东西是本仓没有对应物的**。README 的功能清单太粗（二十几条），一条「词库管理」背后是十三个文件。

新增 `scripts/test-reference-source-inventory.py`，走完来源 `windows/`（TSF 文本服务）、`server/src/`（承载候选窗、工具栏、设置程序与词库的进程）和 `ui/src/`（它自研的 Direct2D 控件框架）下全部 **274 个 `.cpp`/`.h`**，每个必须以三种方式之一落地：

1. 本仓有同名文件（容许两边的命名习惯差异）；
2. `ANSWERED_BY` 指名本仓哪个文件以另一个名字答复它，**且那个路径必须存在**；
3. `DELIBERATELY_ABSENT` 写明为什么本仓不需要答复它。

三者都不沾的就是发现：一个没人记过账的文件。

**当前结果：274 个全部有着落——159 个同名对上，99 个改名后对上，16 个有意没有。** 有意没有的集中在两处：来源候选窗有 Direct2D 和 WebView2 两套渲染后端，本仓只有 Direct2D（`windows_webview2` / `candidate_window_template` / `inline_protocol` / `skin_css_policy` / `ui_backend_policy` / `webview_utils` 六个文件），`ui_backend` 作为配置契约保留；以及 emoji / 颜文字 / 英文三个 `*_ime` 属于 Engine 自己的表，由共享运行时去问，不在各平台重写。另有 `serial_task_queue`——它把来源设置窗的后台工作串到一个线程上，而本仓的设置窗是 Tauri 应用，命令本来就跑在它的异步运行时上。

第二条形式刻意指**路径**而不是一句话：一句「已经支持」谁都能写，而一个必须存在的路径会在本仓自己把文件删掉或改名时立刻报红。第三条形式的理由是该被怀疑着读的部分——迁移要藏起没做的事，就藏在那里——所以每条都写「用户拿到的是什么」，而不是「换了种方式处理」。

反向验证两条路径：删掉 `cloud_ime` 那条记录，它报 `nothing here is recorded as answering this file`；把它指向一个不存在的文件，它报 `recorded as answered by ... which does not exist`。写的时候也真的被自己抓到过一次——`ime_paths` 最初写成 `system/ServerResources.h`，而那个文件在 `ipc/` 下，检查当场报红。

至此四道检查从四个方向回答同一个问题，每次 `--quick` 重新回答一遍：配置能力、界面能力、发布过的功能、以及源码文件。

增量记录（2026-09-21，Windows 第三十八批：组字里的两段要看得出分界）：目标起点 `f95bb51b2`。

来源用 TSF 显示属性把组字分成两类：正在输入的那段是 `TF_ATTR_INPUT`，画点线下划线；已转换的那段是 `TF_ATTR_TARGET_CONVERTED`，不画下划线（`windows/src/DisplayAttribute/DisplayAttributeInfo.cpp`，颜色一律交给应用默认）。也就是说来源里「已经定下的」与「还在打的」在屏幕上是分得开的。

第二十七批把半截词留进组字之后，本仓的 marked text 里也有了这两段，但整段只有一个样式——「海滩paobu」连成一条下划线，用户看不出已经选定的到哪儿为止。

按 macOS 自己的惯例补上分段，而不是照搬点线：AppKit 这边的约定是**已定下的那段细下划线、正在处理的那段粗下划线**（日文输入法在 macOS 上都是这样），并给两段各自的 `NSMarkedClauseSegment` 序号，走 segment 的客户端能看见两段。权重方向与来源相反是平台惯例差异，语义一致。

只在 AppKit 生效：`#if TARGET_OS_OSX`。UIKit 那边的 `UITextDocumentProxy setMarkedText:` 只收纯字符串，而 UIKit 宿主目前不持有半截词（见第三十四批的守卫），非 OSX 路径与改动前逐字节相同。

两处构建细节记下来免得重查：`apple-client` 此前只链 Foundation/Security/CoreText，要加 AppKit；而 `emoji-swift-host-test` 那条 `swiftc` 命令用 `-force_load` 直接吃静态库，**拿不到 CMake 的传递链接库**，得在它自己的命令行上补。后者是 `ctest` 报 `Not Run` 而不是编译失败的那一类，容易看成无关。

用例：断言两段的下划线样式与 clause 序号、以及「没有半截词时仍然是纯字符串」（不持有半截词的宿主完全不受影响）。测试里的假客户端相应把 `marked` 改成 `id` 并加 `markedString`。反向验证过（把第二段改成细下划线，第 475 行变红）。

增量记录（2026-09-21，Windows 第三十九批：候选窗落点的零覆盖）：目标起点 `d449d7e20`。本批换了个找法——扫 macOS 侧源码里没有任何用例提到的文件（190 个源文件里有 68 个），再从中挑既是纯逻辑、又决定可见行为的。

`CandidatePlacement.h` 正是这样一处：两个函数决定候选窗出现在哪儿（贴光标下方、下方放不下就翻到光标上方、再夹进所在显示器的可见区），以及「报上来的光标矩形值不值得用」。此前一行用例都没有，而它错了的表现是用户直接能看见的——候选窗出屏幕、或者在第二块显示器上打字而窗口停在主屏角落。

用例钉七类：常规落在下方且留 4pt、下方放不下翻到光标**整体**上方（不是基线上方，否则会盖住正在打的那一行）、贴右边缘时停在边缘、**负坐标显示器**上夹到那块屏自己的左边缘（夹到 0 会把窗口拖回主屏）、窗口比屏幕还宽/还高时内层夹取不产生「右夹点比左夹点更靠左」的反向结果、光标贴顶时翻转后仍在可见区内。另有一组是 `MSIMEValidCaret`：零高度（客户端没实现光标查询时的返回值）与各字段 NaN/INF 都拒绝，而零宽度要接受——插入点本来就没有宽度。

反向验证过：去掉翻转那一行，第 33 行立刻红。

这批没有改任何行为，只是把一处「错了很显眼、却谁都没测」的地方钉住。同样的扫法剩下的 67 个文件多数是 SwiftUI 视图与窗口控制器，要 UI 夹具才谈得上验证，不在本批范围。

增量记录（2026-09-21，Windows 第四十批：候选窗 WebView2 后端的可移植那一半）：目标起点 `66710ebf1`。

上一批的记账里有 16 个文件记成「有意没有」，其中最大的一块是候选窗的第二套渲染后端。这一批把其中**可移植且可验证**的部分迁过来，并修正三条记错的账。

**先修正三条错账。** `skin_css_policy` 被我写成「属于 WebView2 渲染器，本仓没有」——错的。它回答的是「第三方皮肤 CSS 进入 webview 文档时，里面的 URL 允许做什么」，而本仓**正在**把皮肤 CSS 送进 webview（皮肤页的工具栏预览），对应物是 `packages/ui/src/skin/skin-toolbar-css.ts`，而且比来源那版更完整：它把不受信任的样式表解析进构造样式表而不是拼接、加 `@scope`、按 `hasUnresolvedCssResource` 丢掉资源没解析成功的声明、隔离动画、并且构造样式表本身就会丢弃 `@import`。记账现在指向它。

**迁过来的是文档侧的两个契约**，放共享层（`crates/client-core/src/candidate_document.rs`），因为里面没有一行是 Windows 特有的：

- `SplitCandidateTemplatePayload` / `InflateCandidateTemplate`：槽 0 是编码串、槽 1–9 是当前页候选，用 `,` 连接，而颜文字里合法地含逗号，所以写入方把字面逗号转义成 ``。填充时候选不足 10 个就在**第一个未使用槽的 `<!--nAnchor-->` 处截断**——空行根本不输出，而不是输出后用 CSS 藏起来。
- `InlineWebViewProtocolScripts`：把 schema/runtime 两个契约脚本内联进页面，并把源码里的 `</` 转义成 `<\/`——那个序列会提前闭合外层 `script` 元素，解析器不管它是不是在 JavaScript 字符串里。

**一个只有动手做才会发现的细节**：整份候选窗文档（605 行）里全是 JavaScript 的花括号，拿去做 `fmt::format` 会直接抛错。被填充的其实是 `candwnd/body/` 下那个 59 行的**片段**——本仓早就把它拍平成 `*_dark.html` / `*_dark_measure.html` 贮存在 `packages/ui/src/upstream/candidate-themes/` 下了。所以除了合成模板的用例，还加了一个直接读**本仓贮存的那份片段**、用真实 payload 渲染、断言三条候选各自出现在自己的行号里、并且第 4–9 行被整段切掉的用例。此前没有任何东西读过那个文件，它悄悄漂移不会有人发现。

**顺带修掉一个本仓自己内部对不上的地方。** `ui_backend_policy` 说三种拼法（`webview2` / `webview` / `web`）都算 WebView2，其余回落原生。本仓 Windows 侧根本不读这个键（它是记录在案的 RUST_ONLY 契约键），但共享的 `UiBackend` 类型**读不了本仓自己出厂配置里写的 `d2d`**：`serde` 的 `snake_case` 只认 `direct2d`。实测确认 `d2d` / `webview` / `web` 三个值都是 `unknown variant` 错误，而偏好文档解析失败不是丢一个字段、是整份回落。加上只读的 serde 别名（写出去仍然只有一种拼法），并用例钉住五种拼法都读得了、未知值仍然报错。反向验证：去掉 `d2d` 别名即报红。

**没迁的是什么，说清楚。** 剩下 12 个文件里核心是 `windows_webview2.cpp`（5326 行）：WebView2 合成控制器、不抢焦点的宿主窗口、把候选数据送进文档的资源处理器。这台机器上既没有 Windows 宿主，交叉构建用的又是 MinGW，而 WebView2 SDK 是面向 MSVC 的——也就是说那 5000 行在这里**一行都编译不了、一个字节都验证不了**。按本仓的证据分级，那会是一整批只有「读起来像对的」这一级证据的代码。它是下一个增量，不是这一批能负责地合进来的东西。

增量记录（2026-09-21，Windows 第四十一批：装词库时不许出现「旧的已删、新的没到」的窗口）：接上一批的零覆盖扫描。目标起点 `1e7dbfe3c`。

`DictionaryInstaller.mm` 编进 bundle、导出符号、零覆盖。它校验指纹、跑 `PRAGMA quick_check`、原子写出临时文件——到这里都很稳妥——然后 **先 `removeItemAtURL:` 删掉现有 `msime.db`，再 move 新文件**。这两步之间任何失败（磁盘满、沙箱拒绝、崩溃）都让用户**一个词库都不剩**，而且从这里恢复不了。改成 `replaceItemAtURL:`：原文件一直在，新文件提交不成功就放回去；本来就没装过的情况没有可替换的东西，move 本身就是全部操作。失败路径顺手清掉自己暂存的文件。

顺带记两条：这个函数当前**没有活的调用方**（两个调用方 `DictionaryRuntime.mm` 与 `MetasequoiaInputController.mm` 都是保留的 Apple 适配器，不参与编译），所以这不是线上缺陷；但它是导出符号又编在 bundle 里，接上就会带着那个窗口，因此按缺陷修而不是按死代码放着。

用例覆盖八种情形：首次安装、覆盖安装、指纹不符、文件被截断（`quick_check` 该拦的正是这种——文件头仍然有效、能打开，读到页才发现）、指纹长度不足 64（否则会与真值的前缀比较）、空指纹、源/目标不是文件 URL、源文件不存在。每一条都额外断言**已安装的那份内容没被动过**，以及临时文件没有留下。反向验证过（把指纹判断短路，第 83 行立刻红）。

同批记下一处查过不是缺陷的：`CandidateFontSize.h` 与 `CandidateAppearance.h` 里的 `NormalizeCandidateFontSize` 只认 16/18/20，而来源与共享偏好接受 12–32。看着像我早前修过的「页大小 5/7/9 快照」那一类，但线上路径不走它——`AppearancePreferences.fontSize` 自己按 12–32 判，已有用例覆盖（`SkinPreviewTest` 用 12 和 32）。那两个头一个只被保留的 Apple 适配器用，另一个（`CandidateAppearance.h`）当前没有任何使用者，是拆分文件时留下的重复定义。按仓库规矩不删，记在这里，免得下一轮扫描再追一遍。

增量记录（2026-09-21，Windows 第四十二批：没人会去收的暂存文件）：本批不是对照来源，是本机装了这份构建之后的实证。

当时在旧状态目录里发现了两个 `.tmp*` 文件（6.2k 与 726 字节，大小正好对应 `preferences.json` 与 `typing-statistics.json`）。这些文件是共享层原子写入的暂存文件：`NamedTempFile` 在析构时会自己删掉，但**进程被杀就不会析构**——而每次重新安装输入法，`install.sh` 第一件事就是停掉正在运行的实例。写到一半被杀，暂存文件就永远留在用户的数据目录里，每次一个，之后没有任何代码再看它一眼。

在 `atomic_write` 里加一道清扫：目录里 `.tmp` 开头、**一天以上**没动过的普通文件删掉。用「够老」而不是「拿着锁」作判据是有原因的：统计文档把暂存文件写进同一个目录，用的是它自己的锁，只按文件名清会把别人正在进行的写入删掉；而暂存写入以毫秒计，一天之外的必然是没人要的。清扫挂在偏好保存这条不频繁的路径上，不放进每次上屏都会走的统计写入。

用例钉四条：一天以上的暂存文件被清、刚刚创建的留下、名字不像暂存文件的（无论多老）留下、名字像但其实是目录的留下；并断言这次保存本身照常生效。反向验证过。

增量记录（2026-09-21，性能：先量后改，结果是把半秒从每次启动里拿掉）：目标起点 `2aad6f9fb`。

**先说量到了什么，因为结论和一开始的猜测不一样。**

`keystroke_latency` 的基线是 `character` p50 0.43ms / p95 0.90ms，无超过一帧的停顿。数据里有个稳定信号：第 2、3 个按键 p50 约 0.7–0.8ms，而第 1、4、5 个是 0.36–0.45ms，贵约 1.8 倍，三次独立运行都复现（`pinyin_timing` 的注释专门警告过同一构建连续跑能差三倍，所以这一步不能只跑一次）。

于是给 `refresh()` 临时插桩，把引擎快照、后处理（rerank / demote / normalize）和与上一帧的整套比较分开计时。结果是**运行时自己的工作合计约 0.017ms/次，占一次按键的 4%**（快照 0.017、后处理 0.001、比较约 0），其余 96% 在 `engine.character()` 里。也就是说那个隆起在 C++ Engine 的解码里，按 ARCHITECTURE.md 输入算法归 Engine，本仓不改。插桩已撤。

顺带把重排也量清楚了：`rerank_latency` 的同进程 A/B 显示它 p50 只加 0.02ms，却把 p95 从 2.03ms 抬到 4.68ms、p99 从 2.70 抬到 8.56、最坏 +12.38ms——尾巴几乎全是它。但它在 `chinese-ime-lm`（另一个仓库，按 rev 钉住），而且 483 次按键零次超 16ms 预算；更关键的是 `keystroke_latency` 那条路径根本没挂重排（要 `set_reranker` 显式开启），所以它不是上面那个隆起的原因。

**真正能在本仓动的是启动。** `prepare_host` 实测冷启 1.15s、热态 0.46–0.59s，而它在 Windows 上是 **Server 每次启动都跑**（`src/entrypoints/server_main.cpp:546/575`）。原因是 `ResourceStore::verify` 对整套资源重算 SHA-256，而桌面资源集是 **169MB**（`msime.db` 74MB、日语词典 64MB、三元/二元各 12MB）。也就是每次登录、每次 Server 重启，用户在能打第一个字之前先等半秒钟做一次哈希。

这件事安装时已经做过一次：`install()` 是边复制边校验字节的，而且目录名就是规格的 generation 摘要。启动时重算，是在重新证明已经证明过的事。

**改法照搬本仓已有的先例**：`scripts/fetch_engine.py` 写一个标记文件记下准备过什么，匹配就跳过。新增 `VerifiedMarker` 记下 generation、目录路径，以及每个文件的尺寸与修改时间；下次启动三者全对就跳过哈希，任何一项对不上（规格变了、文件大小变了、被重写过、不见了、标记读不出来）就照旧全量哈希并重新记录。标记写在 **state root** 而不是资源目录旁边——Windows 上资源装在 Program Files，Server 没有写权限。

实测：首次 0.44–0.71s，之后 **0.00s**。反向验证也做了端到端：`touch` 其中一个文件之后立刻回到 0.32s 重新哈希，再之后又回到 0.00s。

**这道缓存挡不住什么，写明白**：它记的是尺寸和 mtime，不是哈希。一个精心构造成尺寸与 mtime 都不变的替换文件会被放过。这是自觉的取舍——资源装在安装目录里，能往那儿写的人本来就已经拥有「启动时哈希」也拦不住的权限；而现实里的损坏（写了一半、被换掉、版本错配）都会改动尺寸或 mtime。写标记失败不影响启动：下次照旧哈希，也就是这个函数原本的行为。

顺手修掉一个每次构建都在报的警告：`candidate_document.rs` 里那个 `.map(|tail| ...)` 闭包没用到参数（`unused variable: tail`），是第三十九批我自己带进来的。

增量记录（2026-09-21，Windows 第四十三批：悬浮工具栏上手写与语音的开关）：本批来自用户直接反馈——「悬浮工具栏 手写和语音 设置中没有打开关闭的按钮」。

来源的悬浮工具栏有六个组件开关（全角、标点、简繁、Emoji、屏幕键盘、设置；中英文切换常显），本仓 macOS 的工具栏在此之上多画了两个按钮：手写识别板与语音输入。多出来本身是有意的（对照表第四十三批记过「目标多出手写识别板」），但这两个按钮**没有任何开关**，用户关不掉——代码里写着「Handwriting and voice are always present」。

补上：共享偏好加 `floating_toolbar.handwriting` 与 `floating_toolbar.voice`，当时两个都默认开——它们从一开始就在工具栏上，开关出现的那一刻不该让两个按钮消失。**这条决定后来被推翻，见《工具栏可选按钮改为默认关》：现在两个都默认关。**设置页按宿主能力显示：新增 `floating_toolbar_handwriting` / `floating_toolbar_voice` 两个能力位，只有 macOS 报 true，别的宿主的工具栏根本没这两个按钮，给它们开关等于关掉不存在的东西。macOS 侧原生偏好与工具栏面板一并消费，并去掉了「这两个按钮永远存在」的计数。

顺带修掉 develop 上五个红的设置页用例，都是并行批次留下的：四个是「加加」辅助码进了 UI 但用例的选项清单没跟（第六套方案），一个是词库条目编辑——用例点的是页脚的「保存设置」，而条目编辑器有自己的「保存」，页脚那个只写偏好文档，碰不到词库；看提交记录是页脚按钮改名时被一并替换掉的。

另记一条本批发现、留待单独处理的：本机装上诊断版之后，日志显示**偏好每秒被重新应用一次**（`preferences_applied` 一秒一条）。那是监视器在空转，不是本批范围。

增量记录（2026-09-21，Windows 第四十四批：偏好每秒被整份重新应用一遍）：本批同样来自本机实证——给输入法装上带诊断日志的构建之后，日志里 `preferences_applied` **一秒一条**。

`startPreferencesMonitoring` 起了一个 1 秒的定时器轮询偏好文档，这是热更新的设计，没问题；问题是读回来之后**不看内容就整份应用**：走一遍所有偏好、再把快照送进 Engine（`updatePreferencesSnapshot:`）、再写一行诊断。一秒一次，而绝大多数时候文档和一秒前一模一样。

偏好文档本来就带 `revision`（保存时自增），所以在应用前比一下：与本会话已应用的相同就直接返回。两条规则让跳过是安全的——没有 revision 的文档（该字段之前写的）永远应用，因为它对自己的内容什么都没说；本地编辑调用的 `reset()` 会连同已应用的 revision 一起忘掉，于是文档被回滚到一个本会话见过的 revision 时，下一次读仍然会应用。

用例：`preference-load-state` 补上这两条规则（这个用例文件本来就有，是管代次与在途读的，本批在其后追加）；`ShortcutTest` 用受控的异步读钉住「同 revision 读两次只应用一次、Engine 只被打扰一次；revision 变了才再应用」。反向验证过。

过程记一条给自己：写新用例时我用 heredoc 直接覆盖了 `PreferenceLoadStateTest.cpp`，而那个文件本来就存在——CMake 里重复的 `add_test` 让 configure 失败，于是 ctest 一直在跑**旧的二进制**，我对着陈旧结果查了好几轮。教训是加用例前先看同名文件在不在，以及 configure 失败要当成硬失败看，不能只看 ctest 的结论。

增量记录（2026-09-21，Windows 第四十五批：测试往用户的偏好目录里丢垃圾）：又一条本机实证。查 Shift 那条时顺手看了一眼 `defaults domains`，发现本机堆了 5433 个 `msime.*` / `app.msime.test.*` 的偏好域。

成因仓库里早就写明了：`TestPreferenceSuite.h` 的注释说 `removePersistentDomainForName:` 只清空域、**不删盘上的 plist**，所以必须走 `MSIMERemoveTestPreferenceSuite`。问题是有三个用例没走——`SchemeRoundTripPreferencesTest` 只调了 `removePersistentDomainForName:`（半个清理，文件照留）、`CandidateRowLayoutTest` 根本没清、`AppleStoredPreferencesTest` 用固定名字写了一个域也没清。三个都改成用那个助手。

加 `scripts/test-preference-suite-cleanup.py` 挂进 `--quick`：macOS 用例里凡出现 `initWithSuiteName:` 的源文件，必须同时出现 `MSIMERemoveTestPreferenceSuite`；只调 `removePersistentDomainForName:` 的会被单独点名，因为那读起来像已经清理了。当前 12 个用例开域、12 个都清。

本机那 5433 个文件已清掉（4222 个是 42 字节的空壳，其余是断言 abort 后留下的；`app.msime.*` 里真实的六个应用域一个没动）。

增量记录（2026-09-21，Windows 第四十六批：单按 Shift 切不了中英文，以及它暴露的两件事）：用户报「shift 切换中英文切换不了」。本机装了带诊断的构建、按真实事件序列查完，结论分两半。

**一、终端类宿主根本收不到修饰键事件，这不是本仓能修的。** IMK 里宿主会调输入法的 `recognizedEvents:` 问「你要哪些事件」，默认只有 keyDown。日志显示：在 iTerm/VS Code 这类宿主里，`activateServer` 触发了、`recognizedEvents:` **一次都没被调用**，于是系统按默认值只送 keyDown，`flagsChanged` 永远到不了，单按 Shift 在这些应用里无法实现。换到备忘录这类原生 Cocoa 宿主，`recognizedEvents:` 立刻被调用（mask=0x1400），`flagsChanged` 也正常送达。这一条按平台限制记录，不是缺陷。

**二、「当前按着哪些键」的记录会永久污染，一次丢失的抬起就让 Shift 终身失效。** `MSIMEModifierTap` 用一个集合记录按下未抬起的键，用来实现「按着别的键时不算轻点」。集合只靠 keyUp 清除，而 keyUp 可以不来：焦点在按键按下时移走、宿主吃掉抬起、或者（本次排查中我自己制造的）宿主请求的事件种类里没有 keyUp。留下的那一项之后**永远**判为「还按着」，每一次 Shift 都被拒绝，而屏幕上没有任何提示。

改成不信任自己的记录：集合仍然决定「要问哪些键」，是否真的按着则查 HID 的键盘状态（`CGEventSourceKeyState`，不需要任何权限）。查询做成可注入，测试用自己的合成键盘作答——测试本来就在模拟键盘，这部分也该由它模拟。原有用例相应补上「这个键此刻按着/松开」的模拟，并新增一条用例专门钉「抬起丢了之后 Shift 仍然有效，而真按着时仍然不算轻点」。反向验证过。

排查过程里有两条值得记的：其一，诊断日志在偏好应用之后才配置，于是 `recognizedEvents:` 这种发生在会话建立时的调用记不下来——探针改成在 `activateServer:` 一开始就配置日志才看见真相。其二，我一度把 `NSEventMaskKeyUp` 从掩码里去掉做对照实验，结果正好触发了上面第二条，用户那次复现失败是我造成的，不是原有缺陷。

增量记录（2026-09-21，Windows 第四十七批：一条自我纠错，以及它暴露的一个流程缺陷）：本批撤回第三十批（#3399）对云候选等待预算的改动。

那一批的依据是来源 `server/src/cloud/cloud_request.cpp` 里的 `CONNECTTIMEOUT_MS 2000` / `TIMEOUT_MS 2500`，据此把本仓两个宿主从 2000/2000（Windows）与 2 秒（macOS）统一提到 2500。**这个文件在钉住的来源提交 `e1d53dd8` 上根本不存在**：来源那边云候选早已从 libcurl 改成 WinHTTP，`cloud/cloud_ime.cpp` 里是 `kTimeoutMs = 2000`，四个阶段各 2000ms。也就是说本仓原来的数字是对的，我按一份**更旧的本机检出**把它改错了。

现在把三处数字改回 2000，并把注释写成来源实际的形态（WinHTTP 四段超时）；共享声明与 `--quick` 守卫保留——宿主各写各的这个风险是真的，只是当时对齐到了错的值。

流程缺陷本身更值得记：本机的来源检出停在 `997fdfd9`，而对照表第一节钉的基线是 `e1d53dd8`，两者之间 158 个文件、近 8000 行改动。**我整天都在对着旧检出做对照而没有先核对 HEAD。** 发现的方式是偶然的：比皮肤 CSS 时发现来源那边是单层阴影、本仓是两层，而记录里写着「本仓补齐到来源的两层」——方向反了，一查才知道检出更旧。

按这条重验了今天所有据此改过行为的结论，其余四条在基线上一致：英文模式开关走 `FUNCTION_CANCEL`（#3390）、Home/End 的 `MOVE_PAGE_TOP/BOTTOM` 与 `SetSelection(-1)` 取末项（#3392）、造词期 `word_for_creating_word` 的显示与 Shift 提交原始击键（#3395）、TSF 显示属性的点线/无下划线两类（#3408）；另有英文候选 5/1000、颜文字 2/3、AI 的 650/2500/8000 也都未变。

下次对照前先 `git -C <来源> log -1` 与对照表第一节的固定提交核对，不一致就用 `git show <基线>:<路径>` 取文件，而不是读工作区。

增量记录（2026-09-21，Windows 第四十八批：macOS 缺了按分词单位编辑组字）：用基线重新读 `input_key_policy.h` 时发现的——它比我之前读的旧检出多出四条规则，全是造词期的编辑：`ShouldRetreatCreatingWordSelection`、`IsSegmentBackspaceKey`、`IsSegmentCaretKey`、`ShouldDropCreatingWordSegment`。

其中两条是纯按键路由：**Ctrl+Backspace 按分词单位删、Ctrl+← / Ctrl+→ 按分词单位移光标**，且只认裸 Ctrl（带 Shift/Alt/Win 就还给应用）。共享运行时早就有 `SegmentBackspace` / `SegmentMoveLeft` / `SegmentMoveRight` 三个动作与 `segment_raw_boundaries`，Linux 两套前端与 Windows 宿主都已接线，**只有 macOS 没接**——这三个键在 macOS 上落进「任何 Ctrl/Option/Command 组合都先结束组字再交还应用」那条规则，于是它们唯一的效果是把正在打的字上屏。

接上，判据照 Linux 已有的那条（裸 Ctrl + 有活动组字），并且必须放在那条「交还应用」规则**之前**——第一版放在按键 switch 里，永远到不了。用例钉四类：三个键各自发出对应命令、带别的修饰键时不接管（仍是结束组字后交还）、没有组字时不接管、以及「只有候选页没有编辑文本」也算组字（前面的选词已经把拼音吃掉了，剩下的正是要编辑的部分）。反向验证过。

另两条（退格退回上一段选择、分段退格删掉上一段）当时记成「需要 Engine 侧的选择历史与客户端能应用回复的协商，macOS 没有那层协商，留待单独评估」。**这句是错的，下一批已纠正**：那层协商在本仓不是 TSF 回复而是 `phrase_preedit`，共享运行时里的 `retreat_phrase_selection` 早已实现并有用例，而 macOS 既请求了 `phrase_preedit` 也把两个键路由到了对应命令——这两条在 macOS 上一直是活的。

### macOS 真实编辑器验收：这台机器给不出，原因与判据都记下来（2026-09-21）

这份表里 macOS 每一条的证据级别，最高是「CTest 129 个用例通过 + 装得上、在系统输入法列表里选得中」。**缺的那一级是「在一个真实编辑器里敲字，看见候选、看见上屏」**，今天试了两次，两次都没拿到，原因不在产品。

第一次的失败方式值得记：合成按键是发给「当前前台应用」的，不是发给某个窗口的。目标编辑器抢不到前台时，按键照样发出去，于是它们落进了当时真正在前台的那个窗口——也就是跑着这场会话的终端。**这不是验收失败，这是往别人的输入框里打字。**

所以第二次的探针改成每发一个键之前都问一次前台是谁，不是目标就中止并打印是谁拿走了焦点。今天的结果是它**拒发**：前台被 Safari 拿走了。另一半原因是 macOS 14 起收紧了程序化激活——一个没有 bundle、自己又不在前台的命令行进程调 `activateWithOptions:` 不会生效，脚本只能请求激活、不能保证保持。这台机器同时还有人在用，前台是争的。

判据因此写成这样，后面谁再做这件事照抄：**合成按键的正确失败是「一个键都没发」，不是「发完了发现打错了地方」**。逐键校验前台、不匹配就中止，是这条判据唯一的实现方式；把检查放在循环外面（开头查一次然后连发十个键）等于没查，因为焦点恰恰是在那十个键中间被拿走的。

收尾也是判据的一部分：临时文稿按未修改关闭（不留存盘对话框），输入法切回 ASCII 布局（`TISCopyCurrentASCIICapableKeyboardInputSource`），临时二进制删掉。验收没做成不等于可以把机器留在验收中途的状态。

这条不影响别的行：macOS 那 129 个用例、五个绘制宿主的半截词一致性、以及本表开头那五道对照门禁，都不依赖真实编辑器。缺的就是缺的，写在开头「本机拿不到的证据」里，不拿低一级的证据顶上去。

### 冷检出装不出 Engine：原因是仓库改名，判据是先验内容再重钉（2026-09-21）

今天早些时候这条被记成「已测量、未解决」：`engine-lock.json` 里 Engine 归档的 SHA-256 对不上，服务端稳定地给出 `40df62bf…` 而锁里写着 `829a6cf1…`，于是**任何冷检出都准备不出 `vendor/MSIME-Engine`**，只能从一个暖 worktree 拷一份过去。当时不动它的理由写得没错：照服务端今天给什么就改成什么，等于让锁去同意它本该校验的东西。

原因今天找到了：**仓库被改名了**，`msime-engine` → `msime-engine`（`updated_at` 就是今天）。GitHub 为一个 commit 现生成的 tarball 把所有文件放在 `<当前仓库名>-<sha>/` 下，改名因此改掉了归档字节，而内容一个字节没变。同一份锁里另外四个依赖归档照旧通过，指向的正是这一个仓库而不是 GitHub 的打包方式变了。

**重钉之前先验内容，这个顺序才是重点**——改名和掉包在你去看之前长得一模一样。做法：clone 该仓库、解析到 `f611f2ff…`、把归档里每个文件按 blob 哈希与那个 commit 比。**462 比 462 全中**；commit 里有而归档里没有的只有四个 submodule gitlink，GitHub 的 tarball 从不带它们，而本锁本来就按 `dependencies` 另行取。验过之后才把锁改成新仓库名与新校验和，**pin 的 commit 一个字没动**。

判据写进 `scripts/fetch_engine.py` 的文件头供下次照做：**对 GitHub 现生成的 tarball 取哈希，同时也是在对仓库名取哈希**；下次再遇到，要重跑这个比对，而不是看「两次下载同一个摘要」就认——稳定只说明服务端自洽，不说明它给的是那个 commit 说的东西。

效果：本批这个 worktree 是冷的，`--quick` 从五条 FAIL（vendored engine、cargo check、windows cross build、pipe-only x86_64 与 i686——全是 Engine 准备不出来的下游）变成全绿。

### 辅助码方案在 Linux 两个菜单里叫错了名字（2026-09-21）

来源的帮助页只有六段话，其中一段是实打实的行为与名词：「辅助码方案目前支持自然码辅助码、蓝天小雨点、首右2.0、首右plus和小鹤」。照着它去数本仓的标签，发现 **Linux 的两个菜单把「首右」写成了「搜狗」**——fcitx5 状态栏的 `辅助码：搜狗 2.0`，以及 IBus 属性列表里的两个单选项。搜狗是另一家公司的输入法，这个标签等于在切换辅助码的菜单里念了一个跟该方案毫无关系的产品名。标识符 `shouyou2_0` 一路都是对的，错的只有给人看的那一行字。

**为什么此前的门禁一条都没拦住：它们比的全是标识符。** 配置键、UI 动作、设定→字段映射、源文件清单，四道对照门禁看的都是 `shouyou2_0` 这种东西，而它在每一份拷贝里都正确。没有任何一道门禁看过用户读到的名字。

改法按本仓既有惯例（`ShuangpinProfileNames.h` 就是这个形状）：把表收成 `platforms/linux/src/core/HelpcodeSchemaNames.h` 一份，两个前端都用它——状态栏与 IBus 属性列表本来就是同一个设定上的两个菜单，却各抄了一份表，而共享设置页抄的是第三份、且是对的。

新门禁 `scripts/test-settings-label-parity.py`（本批新加时叫 `test-helpcode-schema-labels.py`，下一批扩到两个设定后改的名）比的是**名字**，而且**对着来源比而不是让四份拷贝互相比**——四份一致地错正是它要抓的状态。它读来源默认分支 tip 的 `helpcode.html` 下拉项，与共享设置页、macOS 设置窗口、macOS 后端页、Linux 表四处逐条对。反向验证过：把 Linux 表改回「搜狗 2.0」立刻报 FAIL 并指名是哪一处、来源叫什么。

这一批是在容器里真编译出来的，不是「逻辑回归通过」：容器门禁由 20 个用例变 21 个（新增 `linux-helpcode-schema-names`），全部通过。编译还抓出两处我写错的命名空间——`FcitxEngine.cpp` 在 `msime::fcitx_host` 里，引用 `msime::linux_host` 的表要写全限定名，两个前端各一处。**这正是此前每条 Linux 记录只写「逻辑回归通过」时漏掉的那一级证据。**

顺带按同类问题扫了双拼方案名那张表（`ShuangpinProfileNames.h`）：小鹤 / 自然码 / 首道 / 微软四个都对，Linux 菜单里省掉「双拼」后缀是菜单语境的简写，不是错名，不动它。

### 同一个设定在 macOS 上读到两种写法，以及第五条被推翻的「本机没有」（2026-09-21）

上一批的教训是「门禁比的全是标识符，没人看过用户读到的字」。把那条教训往 macOS 这边推，又落出两件事。

**一、`platform.macos.candidate_page_shortcut` 在 macOS 自己的两个界面上写法不同。** 设置窗口写按键本身（`- / =`、`[ / ]`、`Page Up / Page Down`），后端设置页写成描述（「减号 / 等号」「方括号」）。同一个产品的同一个设定，用户对着两个屏幕看，得自己想明白「方括号」和「[ / ]」是同一个选项。来源的快捷键页是按 `<kbd>` 写按键的，设置窗口那套对，后端页改成它。

这个设定本身是**平台自己的**三选一（来源给的是七个独立开关，不是三选一；本仓的共享 navigation 七项与来源一一对应，macOS 七项全认），所以上游定不了它的措辞，判据只能是「两处必须一致」。上一批那道门禁因此从 `test-helpcode-schema-labels.py` 改名为 `scripts/test-settings-label-parity.py`，一个设定归上游定的比上游，平台自己的比自己两处。两半都反向验证过。写的时候被自己的正则绊了一下：选项文案里带方括号（`[ / ]`），按 `[^\]]*` 取数组会在字符串内部的 `]` 处停下，只读到第一项——**给人看的字里有正则元字符，这是抽文案类检查的固有坑**，改成按行取再抽引号内的字符串。

**二、「这台机器没有 Sparkle」也是错的。** 对照表此前写着「`platforms/macos` 的 CMake 要求 Sparkle 2.9.6，这台机器上没有，整场会话里 macOS 这一级都是 skipped」。实际 `~/deps/Sparkle-2.9.6/Sparkle.framework` 一直在。这是同一类判断里第五条被推翻的（容器、Android SDK、Xcode、DevEco，加这条），而且是**目标平台**，代价比前四条都大。

所以 `--quick` 现在会自己找 Sparkle：找到就配置 `target/macos-isolated` 并补上 configure 唯一还缺的那半——`cargo check` 不产出静态库，先 `cargo build -p msime-host-api`。删掉构建目录重跑验证过这条分支真的会走到。**macOS 从「永远 skipped」变成每次 `--quick` 都真编译**，本批的 Swift 改动就是这么验的：产物 `MSIMEBackend.dylib` 里查得到新文案、查不到旧文案。完整套件 129 个用例通过。

**三、顺带纠正上一批一句写错的话。** 上一批说「退格退回上一段选择、分段退格删掉上一段」需要 Engine 侧的选择历史与「客户端能应用回复」的协商、macOS 没有那层协商、留待单独评估。**这句是错的。** 那层协商在本仓不是 TSF 回复而是 `phrase_preedit`（`retreat_phrase_selection` 的注释就写着「Here that condition is `phrase_preedit`」），共享运行时早已实现两条规则并各有用例（61 个运行时用例通过），而 macOS 既请求了 `phrase_preedit`（`InputController.mm:2980`）也把两个键路由到了命令 12/13/14（宿主用例在 `ShortcutTest.mm:1599`）。**这两条在 macOS 上一直是活的**，原记录已就地改正。写「留待评估」之前该先查一遍共享层有没有人已经做了。

### 英文词的「编码」与「词」终于可以不同（2026-09-21）

对照表开头「需要所有者拍板」那一栏里的第一条，本批做掉了：**`dont` 现在能打出 `don't`**。

**先把此前写错的原因纠正过来。** 第二十四批把它记成「要动 `engine-lock.json`、影响面覆盖全部平台、照规矩单独决定」。去翻来源自己那份引擎（引擎早已搬进来源仓库的 `engine/`，独立 engine 仓冻结在本仓锁的那个 commit）才发现：**那条校验在来源里一字不差**，`normalized != entry.key` 就在它自己的 `personal_dictionary.cpp` 里。没有更新的引擎可提，所以「等一个 lock 提升」等不到东西。

真正的差别是**走哪扇门**：来源的 `dictionary_manager.cpp` 直接把 `word` / `display` 绑进 `english_words` 的 INSERT，再自己调 `record_user_insert` 记一笔journal，**从不经过那个校验函数**；本仓所有个人词条写入都走引擎的请求/回执路径，而正是这条路上有校验。两种做法各有代价，本仓这条给的是重试、冲突检测、换代重放与云同步——不该为一类词条在旁边另开一条写入路径。

**所以改的是规则，不是通路**：第四个 overlay `scripts/apply_engine_english_display.py`，把英文分支放宽成来源 `IsAsciiWord` 的那条（编码是字母/连字符/撇号，词只要非空——非空在上面的通用边界里早就查过了）。底下的表本来就放得下：`english_words(word, display, weight)` 是两列，`apply_english` 也一直在写 display，卡住的只有一个 `if`。放宽只增不减，此前合法的条目全部照旧合法，没有任何已存词库会因此失效。

**同一条规则在本仓写了五份**（导入解析器、PersonalWord 传输校验、账号校验的两处、host-api 自己的条目检查）。我先改了四处，真引擎往返就在第五处红了——`e-mail` 被 host-api 挡下，报的还是那句既不说字段也不说原因的「invalid dictionary entry」。五处现在都接到 `dictionary::english_code_is_well_formed` 一个判据上。

**证据是真引擎往返，不是逻辑回归**：`host-api/examples/dictionary_requests.rs` 扩了三段——`dont`/`don't` 存得进且**按两段文本列出来**（不是折成一段）、带连字符的 `e-mail` 存得进、以及最关键的一条：Shift+Y 进临时英文后打 `dont`，候选里真的出现 `don't`。两条都能原样删掉、列表回到空。反向验证过：把 overlay 从 lock 里摘掉，红的正是「英文词可以不同于编码」那一条。

**顺带修掉这套 overlay 机制自己的一个陷阱，它差点让我把反向验证做成假的。** 摘掉 overlay 后第一次重跑，用例**照样通过**——因为 `cargo` 根本没重建：`build.rs` 只把 `engine-lock.json` 和 `fetch_engine.py` 列为重建依赖，而 `fetch_engine.py` 的准备标记只含 overlay 脚本的**文件名**不含内容。两者合起来的后果是：**改了某个 overlay 的正文，树不会重做、产物不会重建，磁盘上的源码显示新规则而二进制里跑的是旧规则**。现在标记按脚本内容哈希，build.rs 也把 lock 里列出的每个 overlay 列为重建依赖（用扫描而不是解析，免得给构建脚本加一个 JSON 依赖）。验证方式是只改 overlay 正文、文件名不动，重跑后树与产物都跟上了。

### 全拼备选切分：保护位不该盖掉用户自己调出来的名次（2026-09-21）

「需要所有者拍板」那栏的最后一条，本批做掉，这一栏就空了。

**先纠正理由，和上一批同一个毛病。** 第五批与第十九批都写着这类引擎侧能力「只能通过提 `engine-lock.json` 进来，影响面覆盖全部平台，单独决定」。引擎早已搬进来源仓库的 `engine/`，独立 engine 仓冻结在本仓锁的那个 commit——**没有更新的锁可提**，等下去等不到东西。而本仓对引擎侧改动本来就有 overlay 机制，写这两条记录时已经用过三次。

**而且缺口比记录里写的小得多。** 记录的口径是「全拼备选切分在来源 Engine 里，本仓没有」，实际 `merge_alternative_segmentations` 一直就在本仓引擎里，缺的只是来源后来的修复 `1ea01d5e`。本仓那段与来源**修复前**的状态逐字一致，直接照搬 hunk 即可。

**它修的是什么。** `xian` 可以读作 `xian` 也可以读作 `xi'an`，引擎在列表靠前留一个保护位给另一个读音的最佳词，好让 西安（按权重自然排在第 16）不翻页就够得着。原条件是「排在 index 1 之后的都拉上来」，于是任何这样的词都会被重排——包括本来就在首页、位置是权重排出来的那些。更糟的是调频也按权重排：用户把某个词调上去，它一旦成为自己那组读音里最重的，保护位就认出它、把它拽回第 2 位，表现就是「选一次跳到第二，之后怎么调都停在第二」。改成只对**掉出首页**（默认 page_size 6）的词生效。

**在出货词库上确实有变化，不是空转。** 扫 58 个有歧义的音节，四条首页变了，方向一致——都是把常用字让回给了被罕见切分占走的位置：`tian` 由 `天 提案 田` 变成 `天 田 提案`，`shuan` 的 熟谙、`zuan` 的 祖安、`niao` 的 尼奥 各自后退一两位。`xian` 不变：西安 自然位置 16、在首页外，照旧被提升——那正是保护位存在的理由。

新增 `crates/engine-bridge/examples/alternative_segmentation_dictionary.rs`，用真实出货词库钉两个方向：`xian` 里 西安 仍在 index 1；`tian` 里 田 必须排在 提案 前面。按本仓惯例（`frequency_dictionary` / `mixed_input_dictionary` 同类）手动跑、文档记录，不入门禁——它需要那 170MB 词库。反向验证过：把 overlay 从 lock 里摘掉，红的正是第二条断言，而且**这次重建是自动发生的**，因为上一批刚修好的「改 overlay 正文不重建」在起作用。

`--quick` 全绿；macOS 按目标平台另跑了一遍套件，129 个用例通过。

**这一栏空了，但 `e32eeade`（调频比较集跨键合并）还没移植**——它动的是 `user_dictionary_journal.cpp` 的 49 行，属于调频而不是切分，留作下一批，理由不再是「要提锁」而是「还没做」。

### 调频比较集跨键合并：选多少次都不动的那个候选（2026-09-21）

上一批末尾记的「还没做」，本批做掉。来源 `e32eeade` 的引擎 hunk，照搬。

**它修什么。** 打 `jian` 时列表里混着另一种切分 `ji'an` 的词。`adjust_candidate_ranking` 写新权重用的是中点算术——在目标位置的前后邻居权重之间取中值——这要求比较集按权重降序，而**显示顺序不是权重顺序**：备选切分占着保护位、置顶候选钉在固定位，都与权重无关。按显示顺序取基准，就是 #400 里把 `写` 写成 506（`西鄂` 的权重 6 加 500）的来路。

本仓引擎当时的解法是把比较集按音节数切开，`西鄂` 因此被挡在外面——症状没了，代价是**每个候选被关进自己的组**：吉安 权重 1，只能跟 积案 / 几案 比，可学权重封顶一万出头，永远追不上 见 的 3460998。用户选多少次，这个候选都纹丝不动。改成在比较集上按权重降序排一次：不变式在源头恢复（任何行都不可能坐在高于自身权重的位置上充当基准），墙就不必要了。写入仍只落在 `entry_key` 的行上。

**实测（真实出货词库，promote 模式逐次选）**：吉安 修复前 `74 → 49 → 41 → 36 → 32 → 32` 卡死，修复后 `74 → 4 → 3 → 2 → 1 → 0` 一路到首位，与来源提交里写的「连选五次一路走到首位」一致。#400 那条护栏同时守住：`xie` 里 `写` 仍是每选一次上移一位 `3 → 2 → 1 → 0 → 0`，没有出现乱写权重的表现。

新增 `crates/engine-bridge/examples/frequency_comparison_set_dictionary.rs` 把这两串数字都钉成断言（起点 74 也钉了——出货词库若变动，这个夹具的前提就不成立，该让它红而不是悄悄测别的东西）。反向验证过：摘掉 overlay，红的正是「必须能走到首位」，报的就是 `[74, 49, 41, 36, 32, 32]`。

既有的 `frequency_dictionary` 五模式回归照旧通过——这条比新用例更重要，因为本批动的正是写权重的那段算术。`--quick` 全绿，macOS 129 个用例通过。

**至此来源在 Engine 侧的三项全部落地**（加加辅助码、备选切分保护位、调频比较集），「需要所有者拍板」那一栏不再有内容。

### 「有字段」不等于「用得着」：给设定加一道可达性判据（2026-09-21）

指令里「公共功能+UI 放 tauri」这条轴，此前没有任何门禁把守。`test-reference-config-coverage.py` 把来源 178 项设定各自映射到本仓的一个字段，但它的判据是「这个名字出现在 `preferences.rs` **或** 共享设置页之一」——**只要 `preferences.rs` 里有个字段就算过**。那正是「设定存在、能读能写、用户却改不了」这种纸面支持的形状。

本批把这条判据收紧：映射到字段的目标必须**在共享设置页里被提到**，否则记进 `PLATFORM_LOCAL` 并写明理由。当前只有一条例外：`appearance.ui_backend`——来源用它在 Direct2D 与 WebView2 两套界面之间选，而本仓候选窗、悬浮工具栏、输入法菜单由各平台原生绘制，根本没有第二套后端可选。

**加这道判据时抓到的第一条，是门禁自己的映射写错了。** `general.candidate_arrow_navigation` 映射到了同名字符串，而那是 serde 的 alias（为读旧配置留的），真实字段叫 `arrows`，共享页面里那行 `["arrows", "上 / 下（移动候选项）"]` 正是来源翻页区的同一项。写 alias 让一个本来可达的设定看起来不可达。已改成指向真实字段。

**这道判据的能力边界写在它自己的文件头里，不含糊：** 它查的是「字段名出现在页面源码中」，比「真的渲染了一个控件」弱——光有类型声明也能满足它。它拦的是真实会发生的那种失误（往 crate 里加了偏好字段、页面上什么都没做），拦不住「控件被删而类型还在」。反向验证也按这个口径做：把页面里 `arrows` 的**每一处**都改名才会红；只改掉那行控件文案不会红，这一点我实测过，所以报错文案改成只声称「这个名字在页面里一处都找不到」。

顺带记一个自己踩的坑：第一次反向验证我写成 `python3 ... | head -3` 再取 `$?`，拿到的是 `head` 的退出码 0，差点把「没红」记成结论——本仓规则里早写着关键命令不要接管道，这次是在门禁上重犯。

### 上一批那道判据自己有个洞：共享设置页是一个目录，不是一个文件（2026-09-21）

上一批加的「设定必须在共享设置页里够得着」，`page_text()` 只读 `packages/ui/src/index.tsx` 一个文件。**页面其实是一个目录**：抽出去的组件都在旁边，比如候选字体那组控件（含回退字体编辑器）在 `candidate/candidate-font-controls.tsx`。只读一个文件，所有这类控件都看不见。

这个洞是我自己顺着它往下查时撞出来的：我想把判据从「名字出现」推到「真有控件」，先拿一段分析扫了一遍，报出五条「只有类型声明」的嫌疑——其中回退字体那两条纯属误报，它的控件一直都在，只是在另一个文件里。现在读整棵 `packages/ui/src` 的 `.ts`/`.tsx`。

**顺带把判据往前推了一格**：目标不能在整棵共享树里只以 `name: type;` / `name: value,` 的形态出现——那是「往类型里加了字段、别处什么都没接」的形状。三个语音润色自定义槽记为例外并写明理由：它们的字段名是运行时拼的（`polish_prompt_${slot}`），字面搜索看不见写它们的那个 textarea。

**这条新判据的边界是实测出来的，不是推断的，而且比我原本想写的弱。** 我本来想说它能保证「有控件」。反向验证时把回退字体编辑器从组件里整段剥掉，门禁**没有红**——因为 `candidate-font-family.ts` 与 `resolved-candidate-fonts.ts` 仍在读这个字段做校验和字体解析。要区分「控件在写」和「辅助模块在读」得去解析 JSX，那不值当。所以它声称的只是「共享树里没有任何代码读写它」，并用一个真正没接线的字段验证过会红。

**结论本身也值得记下来：**178 项映射里，除 `appearance.ui_backend`（来源用它在 D2D 与 WebView2 间选，本仓各平台原生绘制）外，**每一项在共享 Tauri 页面里都有位置**。这是这条轴第一次被量过，而不是凭印象说「都做了」。

两个操作上的教训：撤临时试验改动时我用了 `git checkout <file>`，而那个文件同时装着本批的真实改动，一并被丢掉，只能重做——试验前复制一份、事后拷回，像处理那两个 tsx 那样。另外 `--quick` 这一跑先红在 `vendor/MSIME-Engine` 准备失败上，原因是 GitHub 返回 504，重试即过；这类瞬时网络失败不算环境受阻。

### 双拼把 yo 识别为完整音节（2026-09-22）

固定 Windows 来源的 `5e641fb5` 从双拼兼容排除表里移除了 `yo`，让四套方案都把它识别为完整的零声母音节并命中“哟”。目标曾迁入同一修复，后来提 Engine 锁时删除了 lock 中的单项 patch；留下的 Rust 回归只断言 preedit 非空，所以错误的 `y'o` 仍会通过。

本批用出货词库先复现：小鹤双拼输入 `yo` 的 preedit 是 `y'o`，无法按 `yo` 命中“哟”。`apply_engine_shuangpin_yo.py` 只移除这一项历史排除，不放开相邻的其它兼容拼写。原单测改为逐方案精确断言 preedit 为 `yo`；`schemes_dictionary` 再用固定发布词库逐方案断言边界和“哟”候选，避免空词库继续掩盖解析错误。macOS、Windows、Linux 与移动端都通过公共 Engine 获得该修复，无需复制宿主逻辑。

### 设置页切换后继承上一页滚动位置（2026-09-22）

继续按来源远端实际默认分支 `develop` 核对；本批固定来源提交 `467b9804dac9bcea7dfac654d293dc9f99f57b09`，没有读取相邻检出的未提交内容。来源的 `2b2a546b` 修复了所有设置子页共用滚动容器时的一处可见缺陷：在长页面滚到下方，再切到另一页，新页面会继承旧的 `scrollTop`，看起来像从页面中间打开。

共享 Tauri 设置页也是同样的结构：全部子页复用 `#settings-content`，此前 `selectPage` 只改 React 状态，没有重置这个元素。修复放在 `packages/ui`，不是 macOS 原生窗口里：页面状态提交后以布局 effect 把共享容器归零。因此侧栏、首页卡片、移动端选择器以及浏览器返回造成的页面变化都遵守同一条规则，也不会在切换后的第一帧短暂画出旧偏移。

设置页回归用例先把共享 `main` 容器滚到 480，再从「外观」切到「辅助码」，同时断言目的页标题与 `scrollTop == 0`。这项是 Windows 来源行为迁入公共 UI；macOS 通过承载同一 Tauri 页面直接获得，不复制平台专用实现。

### 独立选择的整句候选也要学习（2026-09-22）

来源仍固定为远端默认分支 `develop` 的 `467b9804dac9bcea7dfac654d293dc9f99f57b09`。复核 `01c5bca3` / `663f7230` 后确认当前目标只覆盖了其中一种形状：用户先选一段、再用 Generated/Fallback 整句完成余下组合时，`InputSession::commit` 会把两段合成用户词组；用户直接选择一条完整整句时，`learn_candidate` 把它当成非词库候选跳过。实测锁定资源上的 `haitanpaobu`：直接选择来源 9（Fallback）的「海滩跑步」后，简拼 `htpb` 仍然找不到它，证明这不是静态差异而是可达缺口。

修复沿用来源 Engine 的语义，通过 `apply_engine_standalone_sentence_learning.py` 进入锁定 Engine：Generated/Fallback 没有 SQLite 行可调权重，因此选中时按 canonical quanpin 创建用户词组；仅限全拼/双拼式候选，要求完整读音与汉字数一致，最多 7 音节，并服从统一的 `learning` 开关。本仓的所有原生宿主都通过公共 Engine session 选择候选，所以来源 `663f7230` 那条绕开 Engine 的 Windows Server 旁路不复制；macOS 直接得到公共实现。

`phrase_creation_dictionary` 现在覆盖三种真实词库往返：分段造词照旧可由简拼找回；独立选择来源 8/9 的整句后也能由简拼找回；关闭学习时两种选择都不写。加 overlay 前新增断言稳定失败在「直接选择来源 9 后没有存入」，应用后通过，因此测试确实覆盖本批行为而不是只证明候选本来存在。

### 整句词格只接收精确读音（2026-09-22）

来源 `e2a5f5f9` 修了两个互相放大的缺口：跨度查不到精确键时不应把前缀/简拼降级行塞进词格；语言模型只看汉字时还要用词条权重压住零权重的生僻读音。目标 Engine 的打分器不是来源的 KenLM：它用 `bigram.bin` / `trigram.bin` 加 `edge_log_prob`，后者已经把每条词库权重放进边分数，零权重的「卷(gun)」「而(neng)」天然比常见读音低十多个自然对数单位。因此不再叠加来源的 reading-prior 参数，避免破坏本仓已经用句集标定的 bigram/trigram 权重；真实资源打开整句 alternatives 后，`gunqi` 没有生成「卷七」，`nengfasheng` 也没有生成「而发生」。

真正仍缺的是精确跨度边界。修复前真实资源把 `gun'qiu` 的「滚球/棍球」当作 `gun'qi` 的数据库命中放在最前，因为 `query_segments_keyed_flat` 在精确键为空时会扫前缀范围，而插入位置又只按汉字/音节数判断“完整命中”。`apply_engine_lattice_reading.py` 把全拼与双拼共用的词格 lookup 改为批量精确键查询，并在查询前把 `jv/jve/lue` 等 ü 的等价拼法归一到词库存法；整句门槛降到两个完整音节，插入边界改为比较 canonical key。修复后两条错误前缀候选仍作为普通低位候选保留，首位变为 fallback「滚其」，Generated alternatives 只从 `gun'qi` 的行组成（「滚起/滚气/滚奇…」）。

`lattice_reading_dictionary` 用锁定发布资源同时检查默认排序和所有整句 alternatives；Engine 侧的表驱动回归还钉住两音节合并、prefix row 不得挡住 Generated，以及 `jv/lue` 精确查询归一化。公共 Engine session 是 macOS 原生宿主的唯一拼音候选通路，所以平台层无需复制词格算法。

### Google 解码器边界使用它认识的 ü 拼写（2026-09-22）

来源 `ccbaa3a6` 指出的边界在目标 Engine 同样存在：词库与词格把小鹤 `nt` 转成 canonical `nve`，但本地 `googlepinyinime-rev` 和云候选 InputTools 只认 `nue`；原样送入会把它重新切成 `nv + e`。锁定发布资源上的修复前证据是 `ntdddswu`：数据库能给出“虐待动物”，来源 9 的 Google fallback 却是“女儿带动物”。

`apply_engine_google_umlaut.py` 新增唯一的边界转换函数，按完整音节把 `nve/lve` 转成 `nue/lue`，把 `jv/jve` 等转成 `ju/jue`；词库 key、候选 canonical pinyin、缓存 key 和已提交拼音都不改。全拼/双拼整句 fallback 与全拼/双拼云查询统一在送出前调用它，手动分隔符继续保留，所以 `nu'e` 不会被拼成 `nue`。修复后同一真实资源探针不再出现错误 fallback；跨平台 `InputSession` 回归同时断言小鹤云查询发送 `nue'dai'dong'wu` 而缓存仍使用原始 `ntdddswu`。macOS 原生宿主通过公共 Engine 的 online query 与候选 session 自动获得这项行为。

### 统计页回到前台时刷新（2026-09-22）

来源 `f743cc8f` 修的是拉取式统计页的生命周期：窗口一直开着，用户切到其他程序输入再回来时，页面既没有重建也没有重新切换模块，旧数据会永久留在屏幕上。共享 React 页此前只给 `mobile=true` 的 iOS/Android 分支监听 focus 与 visibility；macOS 原生宿主承载的桌面 Tauri 页面没有这条路径，存在同样缺口。

刷新协调现在属于共享 `TypingStatisticsPage`：桌面与移动端都在窗口重新聚焦、文档重新可见时刷新，并在页面挂载期间每 5 秒做一次可见性兜底。所有自动触发共用 1 秒节流与单一在途请求；超过 15 秒的请求视为失联，新请求可以接管，迟到响应不会覆盖新数据。相同 JSON 快照不提交 React 状态，卸载时清理 focus、visibility 与 interval。桌面回归用例先渲染统计页，再模拟回到前台并连续发送两次 focus，断言只读一次；移动端原有回前台用例继续通过。

### 词库翻页回到结果顶部（2026-09-22）

来源 `fcf594e2` 同时修了 WebView2 设置页的浮层定位与词库翻页。目标的浮层问题已经由共享 React 页内确认框消除：它不调用宿主 `window.confirm`，支持遮罩取消、Esc 与焦点还原，在 macOS 的 WKWebView 中也可用。翻页行为却仍有同一缺口：每页 100 条结果随设置主页面滚动，用户在底部点下一页后会保留旧滚动位置，新页第一条留在屏幕上方。

共享词库管理现在与来源一样给结果区独立的有界滚动面，并让上一页、下一页统一在请求前把该滚动面归零。回归用例把首屏结果区滚到 480，再点下一页，断言滚动位置立即回到 0，并继续验证第二页请求与状态。该实现属于共享 Tauri/React，macOS 原生宿主承载的设置页直接获得同样行为。

### 关闭统计时在 macOS 采集入口短路（2026-09-22）

来源 `94df9a79` 修复的八项里，管道监听唤醒、`ERROR_FILE_NOT_FOUND` 重连、IPC 尾部填充、TSF range 克隆与同 tick 去重都属于 Windows 的 Server / DLL / TSF 形态，macOS 没有对应对象；不能为了字面一致照搬。跨平台语义是统计关闭后采集入口直接短路，不读取或分类上屏文本，也不接触传输与存储。

macOS 此前每次上屏仍会计算本地日期与来源、把完整文本序列化成 JSON、投递后台队列并跨 FFI 读取统计文件，最后才在共享 store 里发现 `enabled=false`。现在 IMK 进程以默认关闭的原子位守住 `MSIMERecordTypingStatistics` 第一行：启动或激活时通过只读 FFI 从共享统计文档恢复开关，Tauri 设置进程写入成功后只用 `NSDistributedNotificationCenter` 跨进程发送一个布尔值，不发送路径、统计或输入内容。共享 store 也在构造字符分类前读取开关，守住通知与后台任务之间的竞态。关闭时不再检查、分类、序列化或排队任何上屏文本；开启时原有聚合存储路径不变。

### 长按 Backspace 清空组字后不删除正文（2026-09-22）

来源 `62c9f5cf` 修复的是按键所有权跨越组字终点的问题：一次 Backspace 长按从活动组字里开始，自动重复删掉最后一个预编辑字符后，后续 repeat 仍属于输入法；如果这时按“当前有没有组字”重新判断，它会落回编辑器并开始删除已经上屏的正文。

macOS 原生宿主此前存在同一条可达路径。`MSIMEInputController` 把第一个 Backspace 送进共享 Engine，应用返回的空 `view`；下一次 `isARepeat` 再送 Engine时得到未处理，`handleEvent` 返回 `NO`，同一次物理按住便交给当前 `NSTextInputClient`。现在控制器在非 repeat 的 Backspace 到达时记录这次 hold 是否始于活动组字；组字已空但 hold 仍 armed 时直接吞掉 repeat，不再请求 Engine，也不让客户端收到。新的非 repeat 会重新判定，因此用户松开后再次按 Backspace 仍能正常删除正文；空 sender、客户端切换和有效的 `deactivateServer` 都清掉状态，所有权不会跨焦点泄漏。

### 自定义数据目录与安全迁移（2026-09-22）

来源 `bc37ac9a` 解决的是产品数据根目录不能离开系统盘：安装器让用户选择目录，三个进程通过环境变量／注册表／默认目录的同一优先级找到它；改位置时先停输入法，迁移用户词库、配置与皮肤，并以 `.metasequoiaime-data` 标记目录所有权，避免把用户原有文件夹整棵删掉。

此前第 1306 行把 macOS 的 `preferences_directory` 开发 override 记成“已有对应实现”，结论过早：它只证明 HostOptions 能承载绝对路径，发行设置页没有选择入口，设置 Tauri bundle 与 IMK bundle 也各自在自己的 Application Support 路径找配置，用户无法实际完成迁移。现在共享“关于”页提供数据目录状态、原生 AppKit 文件夹选择器和明确确认；平台适配层把两个固定 `runtime-options.json` 当作定位器，两边始终指向同一个可移动状态根，不照搬 Windows 注册表。

迁移覆盖整个状态根（用户词库及 journal、偏好、统计、剪贴板历史、皮肤和缓存），应用 bundle 中只读的已校验资源不搬。开始前终止独立 IMK 进程；目标必须是绝对、真实、非符号链接的空目录，不得是当前目录、其父子目录、两个定位器目录或卷根。宿主先在目标卷暂存完整副本，拒绝源树内的符号链接，按最终路径重新调用 Host API 生成 HostOptions；两个定位器均原子发布成功后才清理旧数据。准备或发布任一步失败会恢复旧定位器和空目标，旧目录继续有效。只有默认专用目录或带所有权标记的目录会自动清理；显式开发配置指向的无标记目录只复制并向用户报告保留旧副本。

IMK 启动时从 HostOptions 的 `preferences_directory/skins` 设置候选窗与悬浮工具栏皮肤根目录，因此皮肤不再滞留在旧的固定 Application Support 路径。Rust 回归覆盖成功切换、准备失败回滚、非空／嵌套／符号链接目标拒绝和无标记源目录保留；共享设置页回归覆盖路径显示、选择、确认及移动调用，AppKit 目录选择器另由原生构建覆盖。尚未在真实外置卷上执行安装后迁移与编辑器输入验收，不能把本地文件事务和原生构建表述为该层证据。

`ShortcutTest.mm` 以宿主事件序列钉住三个方向：第一键清空组字、随后 repeat 被消费且 Engine 调用数不增加、下一次新按键重新交还客户端；另验证切换文本客户端会解除 armed 状态。反向移除 repeat 栅栏时用例稳定失败在第二次 Engine 调用。实现只落在 `Info.plist` 指定的 `MSIMEInputController`；保留的旧 `MetasequoiaInputController` 不是当前产品入口，不复制一份状态机。

### 中文标点后的空格是改写手势，不是正文空格（2026-09-22）

固定来源 `d4a07964` 重构智能标点后，「中文标点后按空格转换」成功时只把刚上屏的中文标点改成对应 ASCII，空格本身被消费；转换表覆盖 `。，！？；：、` 和单独出现的引号、方括号、书名号、圆括号。macOS 此前只认逗号、句号、冒号三项，改写后还把空格继续交给编辑器，并在全角模式下改成全角英文符号，三处都与来源及本仓已有共享策略相反。

macOS 原生宿主现在按完整映射回读光标前的实际中文标点，只有它与刚按下的 ASCII 键相符才原位替换并消费空格。Shift 不再把 `! ? :` 等本来就靠 Shift 输入的字符误判成竞争快捷键。全角模式、活动组合、跨文本客户端、插入其它按键、光标已移到不同字符，以及宿主仍持有自动补全右半边的成对标点都会放弃改写；后者避免产生 `(<右半边>` 这类混合对子。原生 `smart-punctuation-space` 回归用合成文本客户端覆盖完整映射、单次消费、失效条件、全角与成对补全边界，不记录真实输入。

### 日语减号从物理按键走到真实 Engine（2026-09-22）

固定来源 `5a5dea6b` 的产品行为此前已经落地：macOS 在固定日语方案和临时日语模式下都按物理 ANSI `-` / `=` 键绕开候选翻页与以词定字，锁定 Engine 也已支持 `- → ー`、`n- → んー`、单独 `-` 的「ー / -」候选。复核 `b149a500` 时也确认数字键与空格选择使用候选窗已绘制按钮携带的 `(session, generation, index)`；窗口仍是旧代次时按键会被消费，不会按后台新列表误选。高亮候选的辅助码和翻译颜色同样已经跟随 selected text token，`a54e2e08` 没有剩余缺口。

原有日语回归只证明了辅助分类器和替身会话收到 ASCII，不能证明真实产品链路最后得到长音符。本批把这一证据补在 macOS 原生 `ShortcutTest` 的真实会话夹具中：从物理 keyCode 27 进入 `handleEvent:`，经过 `MSIMEClientSession` 与锁定 Engine，再由 `NSTextInputClient` 看到预编辑并按回车提交；分别断言单独 `-` 得到 `ー`，`n-` 得到 `んー`。该夹具不需要词库，使用合成文本客户端，不记录真实输入。

### 五笔逐键提示与调频持久化（2026-09-22）

固定来源提交 `5403b2e35ce27bc80dbce19535ceb23cc9ae2754` 把五笔查询从精确码扩为有界前缀查询，并接通同码候选调频与 user-journal。本仓锁定 Engine 此前已有较早的前缀实现，但排序按码长优先、返回 200 条且选择候选不会写入调频，因此“逐键有候选”并不代表行为已经对齐。

`apply_engine_wubi_prefix_learning.py` 在 Engine 边界内迁入来源语义：精确等长码先于高权重前缀行，随后按权重和稳定键序排列，最多返回 50 条；选择五笔候选时把该候选权重写为同码组最大值加一，并在同一 SQLite attached-database 事务中写入 `wubi` journal，删除仍复用已有的精确行加墓碑事务。公共宿主继续只消费 Engine 快照，不在 Rust、Tauri 或任一平台宿主复制码表查询与调频算法。

合成词库回归覆盖三条可见不变量：权重更高的长码不能压过已经完整匹配的简码；一键前缀在 Engine 入口被截到 50 条；选择同码第二项后，新会话中它升到首位且 journal 恰有一条 upsert。来源同日的四码唯一自动上屏与第五键“顶字后保留余码”还涉及 Windows TSF/Server 新协议，本片没有把 Engine 调频完成误写成那条链路已经迁完，后续单独接协议和宿主状态机。（后续已接：a93e3288f 在 `src/ipc/ReplyComposer.cpp` 以 `auto_wubi_commit` 走 `ReplyPath::AutoCommitAndContinue`，回归在 `tests/input/reply_composer.cpp`；Windows 实机仍未验证。）（2026-09-23 补齐：a93e3288f 只接了四码唯一自动上屏，第五键顶字（来源 536d0a95）此前没有——完整但不唯一的四码后再敲字母，Engine 把五笔码截在四位，这个字母被直接丢掉。现在由共享 `crates/input-runtime` 在字母键到达 Engine 之前判定：五笔、非独立英文、无本地模式、非拼音回退作答、编辑串正好四个字母且光标在末尾、有候选、未在造词，就先选首选再把这个字母送进新组合，各平台宿主拿到的都是「上屏 + 新组合」的一次转换；Windows 沿用同一条 `AutoCommitAndContinue`，余码非空时按键回复带新组合的 preedit，TSF 按消费数 4 裁自己的缓冲、保留第五个字母。来源 7b7b19f4 的「推送后 HideCandidate 抑制 1 秒过期」也已迁入：`src/input/ContinuationHide.h`，由 `FocusedSession` 在推送确认送达时记账、下一次 HideCandidateWnd 消费，超过 1 秒不再抑制。回归：`crates/input-runtime` 的 `a_letter_after_a_complete_wubi_code_commits_the_first_candidate_and_starts_the_next`、engine-bridge 真引擎断言第五个字母会被 Engine 丢弃、`tests/input/reply_composer.cpp`、`tests/input/continuation_hide.cpp`。）

### 固定来源推进到 345cb87a（2026-09-22）

来源远端默认分支在 `467b9804` 之后新增四个提交，现逐项核对完并把 reference 门禁固定到 merge commit `345cb87a3822f6ad7013bb29506fe3d856c1931a`：

- `837af70b` 让全拼云查询保留手动撇号边界。目标由 `apply_engine_manual_segmentation_cloud.py` 在公共 Engine 边界实现；回归直接断言 `qi'e'huan` 原样进入 query text，macOS 经同一 Host API 获取该查询。
- `99a8a355` 让云/AI 候选选中后按手动分词落库。目标将 CloudSuggestion/AiSuggestion 纳入公共 Engine 的句子学习路径，以会话的 canonical segmentation 补齐在线候选自身缺少的读音；真实词库探针选中“企鹅幻”后能用 `qeh` 找回。macOS、Windows 与 Linux 都通过同一 `InputSession::select`，不复制来源 Server 的旁路。
- `d328895f` 只让 Windows 安装器忽略本地 `staging/` 中转目录，不改变产物、运行时或用户功能；目标的打包树没有这个来源目录，不需要迁移。
- `56bfc046` 与 `345cb87a` 是上述提交的 merge commits，没有额外内容。

六道 reference 门禁均只用 `git show` / `git grep` 读取这个新固定对象；相邻仓库当前 checkout、未提交内容及以后继续移动的远端 tip 仍不参与验证。

### 语音组字快照的截断与分帧边界（2026-09-22）

固定来源测试 `server/tests/src/test_voice_composition_pipe.cpp` 确认，超出 `kMaxSnapshotChars` 的流式语音组字快照不是应当被拒绝，而是截断到 2048 个 `wchar_t` 后按固定 worker packet 分帧。本仓 `voice_composition_bytes` 此前在调用共享协议编码器前提前拒绝超长文本，可能让中间结果无法更新 TSF 组字，最终结果退回 SendInput fallback。

现在由共享协议负责截断和分帧，Windows ReplyCodec 只拒绝零 generation、NUL 文本和非法消息类型。回归覆盖单帧中文/Emoji、多帧重组、恰好 `kMaxChunkChars` 的边界、超限快照截断，以及上述非法输入；测试从实际 404-byte worker frame 反解并核对顺序、generation 和 first/last 标记。

`bash platforms/windows/build-cross.sh x64` 已成功完成 Windows x64 GNU host/TSF DLL、Server 和原生测试目标的交叉构建，`git diff --check` 通过。Wine 执行级验证因 Docker daemon 未运行而跳过；因此本批有交叉链接和静态回归证据，但没有 Windows/Wine 运行时验收证据。

### 首次启动路径支持非 ASCII 用户目录（2026-09-22）

Windows 的安装位置、资源目录和用户状态目录可能包含中文、日文等非 ASCII 字符。首次启动准备流程已经使用 UTF-8 JSON 将这些路径交给共享 Host API；此前回归只使用 ASCII 合成路径，无法证明这一条实际保持不变。`windows-first-run` 现把安装资源与新用户状态放在合成的 `用户目录` 下，验证资源路径、生成的状态文件和后续拒绝规则仍然成立。测试不记录真实用户路径或输入，也不改变生产路径。

### TSF 类工厂复制赋值契约清理（2026-09-22）

`CClassFactory` 的私有复制赋值运算符原本声明为返回引用，却没有返回值；这会在 Windows x64 交叉构建中产生 `-Wreturn-type` 警告，也让一个不可复制的 COM 类保留了未定义行为入口。现改为显式删除复制赋值操作，保持类工厂不可复制并消除该警告。x64 TSF DLL/Server 交叉构建、Windows 合成测试和 i686 语法门禁均通过；没有把这些证据表述为真实 Windows COM、TSF 注册或编辑器验收。

同一边界随后补上引用计数的原子观察：`DllCanUnloadNow` 以及类工厂 `AddRef`/`Release` 的返回值现在通过 `InterlockedCompareExchange` 读取 `dllRefCount`，不再与 `InterlockedIncrement/Decrement` 并发读写普通 `LONG`。加法和清理语义保持不变；验证为 x64 交叉链接、宿主可执行合成测试和 i686 语法门禁，仍不等于真实 Windows COM 运行时验收。

### 安装器数据目录选择与删除边界（2026-09-22）

此前六道 reference 门禁只盘点来源 `windows/`、`server/` 与 `ui/src`，没有覆盖 `installer/`。沿固定来源 `345cb87a` 对照安装器时确认两处真实缺口：来源有可见的数据目录选择页，目标只有隐藏的 `/DATADIR` 参数；目标的 `DataDirIsSafe` 又只拒绝与少数系统目录完全相等的路径，像 `C:\Users` 这种包含用户关键目录的父级仍会通过。安装结束后目标会写入所有权标记，卸载则递归删除带标记目录，因此这不是界面差异，而是可达的数据删除边界。

目标安装器现在把程序本体继续留在 Program Files（满足 `uiAccess`），在完整安装包向导中单独选择词库、配置和皮肤目录；浏览器允许当场新建文件夹，联网选择页排在目录选择之后，就绪页同时显示迁移源与目标。交互与静默 `/DATADIR` 共用同一个判据：仅接受本机盘符下的专用目录，拒绝盘符根、`.` / `..` 绕过、系统与程序目录内部、包含 Windows / Program Files / AppData / 用户目录的父级，以及未带本产品所有权标记的非空目录；创建后还实际写入并删除探针确认可写。light 包不携带词库和静态资源，因此隐藏选择页、固定使用已登记目录，并明确拒绝改变目录的 `/DATADIR`，避免迁移出一个缺资源的数据根。这里刻意比来源更严格：来源允许接管非空目录，却会在随后写标记并于卸载时整棵删除，和它自己的“不会动原有内容”提示矛盾；本仓按数据安全要求拒绝接管。

`windows-installer-launch` 现在直接读取出货的 `msime_setup.iss`，钉住选择页、前后级关系、双向关键目录边界、路径穿越、非空目录所有权、写探针、静默参数复用验证和就绪页披露；本地宿主编译执行通过。安装器 staging 与本地签名证书也恢复来源的目录级 `.gitignore`，避免数百 MiB 产物或机器私钥从短期 worktree 进入提交。尚未在真实 Windows 上编译 Inno 安装包或执行安装/迁移/卸载，因此不越级声称产品级验收。

### Windows 面板文本投递先校验再恢复焦点（2026-09-22）

面板文本请求此前会先恢复目标编辑器焦点，再由 Windows host 拒绝空串或超长文本；因此无效请求仍可能改变用户当前焦点。Windows host 现在导出与 Tauri 面板入口共用的 `valid_text` 判据，在恢复焦点前拒绝空串、超过 4096 字节的文本和控制字符；`send_text` 也复用同一判据，避免两个入口漂移。host 回归覆盖空串、超长文本、换行、Tab 与合成中文。这里只验证源码契约与测试，尚未进行真实 Windows 编辑器验收。

### Windows 面板恢复焦点确认实际前台窗口（2026-09-22）

面板投递此前只检查 `SetForegroundWindow` 的返回值，没有在全局 `SendInput` 前确认当时的实际前台句柄；前台若未落在记住的编辑器，输入可能进入别的窗口。Windows host 的 `focus` 现在在句柄有效且调用成功后立即读取 `GetForegroundWindow`，只有目标仍是前台窗口才报告成功；语音路径原有的前台核对与面板路径因此共享同一安全边界。这里只验证源码与交叉编译契约，尚未进行真实 Windows 编辑器验收。

### Windows 原生 README 的生产装配状态校正（2026-09-22）

`platforms/windows/README.md` 的早期分段仍把 `PipeService`、`RegistrationInbox`、`SessionController`、`SessionWorkers` 和 `SessionPump` 写成“尚未接入生产”的下一步，和当前 `WindowsServer`/生产 `server_main.cpp` 已实际装配的路径相矛盾。本次只校正文档状态：明确生产入口已经启动指定管道、消费登记票据并运行焦点路由与输入队列，同时保留“仅 x86/x64 交叉编译，尚未 Windows 原生 TSF 注册、安装和真实编辑器验收”的证据等级。没有把交叉编译或 `server_smoke` 静态/合成运行写成系统入口验收。

### 可控 TSF 编辑宿主（2026-09-22）

固定来源 `345cb87a3822f6ad7013bb29506fe3d856c1931a` 的 `experiments/tsf-edit-control` 已迁入 `platforms/windows/experiments/tsf-edit-control`，并作为 Windows-only CMake 目标接入。它提供 Direct2D/DirectWrite 绘制的原生 Win32 编辑宿主与最小 demo，可重复检查 TSF 文档上下文、preedit/display attribute、候选位置、软换行、选区、插入点和鼠标命中等边界。

这是一项验证工具，不是产品组件：它不注册 TSF、不启动生产 Server，也不替代第三方编辑器验收。实验源码中来源工程特有但目标树不存在的 `common.ver`、`InputScope.h` 和 `tsattrs.h` 依赖已去除；宿主仍使用系统 TSF 头文件。交叉构建最多证明能够编译和链接，真实 TSF、TIP 激活与编辑器输入仍待在 Windows 上运行验证。

### Win32 剪贴板监听注册回归（2026-09-22）

`ClipboardHistory` 原有用例只覆盖文本边界和持久化策略，没有触及生产 `ClipboardMonitor` 的 `AddClipboardFormatListener`、消息窗口创建与停止清理。新增 Windows-only `windows-clipboard-monitor` 合成用例，实际注册消息监听器、验证重复 `start` 幂等，并验证重复 `stop` 不崩溃；测试不改写系统剪贴板，也不记录用户内容。它补的是 Win32 注册生命周期证据，不宣称已经完成真实复制、粘贴或第三方编辑器上屏验收。

### TSF DLL 类工厂与 TIP 对象边界（2026-09-22）

新增 Windows-only `msime-tsf-class-factory` 回归：从测试进程同目录加载出货 DLL，解析导出的 `DllGetClassObject`，用固定 CLSID 取得 `IClassFactory`，实例化对象并确认它实现 `ITfTextInputProcessor`，随后完整释放 COM 对象和模块。该测试不调用 `DllRegisterServer`、不写 TSF 注册表，也不调用 `ITfTextInputProcessor::Activate`；因此交叉链接证明了 DLL/类工厂/对象 ABI 边界，仍不等于 TIP 注册、激活或真实编辑器验收。

本批把同一回归扩展到 COM 生命周期与错误路径：未知 CLSID 返回 `CLASS_E_CLASSNOTAVAILABLE`，已知 CLSID 但请求不支持的类工厂接口返回 `E_NOINTERFACE`，空输出指针返回 `E_INVALIDARG`；类工厂拒绝聚合，并由 `DllCanUnloadNow` 钉住“类工厂或 TIP 仍被引用时不可卸载、全部释放后可卸载”。生产 `DllGetClassObject` 现在先清空输出并按 CLSID 再按接口判定，避免把这两类错误混为一谈。测试不注册 TIP、不写 TSF 注册表、不调用 `ITfTextInputProcessor::Activate`；交叉构建仍不等于 Windows 原生 COM 运行时验收。

本批继续补齐类工厂的 COM 契约：`CClassFactory::QueryInterface` 对空输出指针返回 `E_POINTER` 并在处理 IID 前清空输出；`CreateInstance` 对空输出指针返回 `E_INVALIDARG`；第二个类工厂接口引用和 `LockServer(TRUE/FALSE)` 的加锁、解锁行为都由 `DllCanUnloadNow` 生命周期断言覆盖。测试仍只使用合成输入，不注册 TIP、不写 TSF 注册表、不调用 `ITfTextInputProcessor::Activate`，因此没有真实 Windows 注册、TIP 激活或编辑器验收证据。

### Windows 偏好发布回调的生效时序（2026-09-22）

偏好监视器的生产链路先在输入队列应用 `PreferenceSnapshot`，再从监视线程通知 `SessionController` 的发布回调；回调会清理并按新配置重新发起当前候选页的翻译查询。原有用例已经覆盖延迟到未确认回复完成后应用，本批另加一个顺序断言：回调提交的观察任务必须看到新的导航绑定和以词定字状态，防止未来把“发布任务已入队”误当成“偏好已经生效”。测试只使用合成 JSON 和队列状态，不触碰用户配置；这仍是源码/交叉构建证据，不是 Windows 原生编辑器验收。

### 安装后已有数据目录的首次运行准备（2026-09-22）

完整安装器会在提升权限下把词库、出厂配置和所有权标记写入所选 `DataDir`，但不能以安装器身份替用户执行 Host API 准备。此前 Server 的首次准备只接受“目录不存在”，所以正常安装后的第一次启动会跳过准备，随后读取不存在的 `runtime-options.json` 并退出；默认目录和自定义目录都受影响。

现在保留独立 `msime-client-prepare` 的全新目录契约；生产首次启动额外允许已有且带 `.metasequoiaime-data` 所有权标记、但尚无 `runtime-options.json` 的目录，在用户上下文中完成准备。普通已有目录、已有运行时配置、文件和符号链接仍不会被接管或重建；准备失败也不覆盖并保留现有数据。回归新增安装器所有权目录的成功准备与可恢复缺失配置路径，并继续覆盖普通不完整目录和错误资源不创建状态。验证为 host 编译运行的合成首次运行测试、x64 MinGW 语法检查和 `git diff --check`；安装包、真实 Server 首次启动和 Windows 系统入口仍未在真实 Windows 上验收。

### 简繁转换改为 OpenCC 词级（2026-09-23）

第十六批用系统 `LCMapStringEx` 逐字映射，那条差异不再保留：逐字表解决不了一对多的字（发→發/髮、干→乾/幹、面→面/麪），「头发」会成「頭發」，与来源的 OpenCC `s2t` 输出不同，用户看得见。

- 共享层：`crates/client-core/src/chinese_conversion.rs` 按 OpenCC `data/config/s2t.json` 实现（一次兼容表归一化，再一次 STPhrases ∪ 地区词派生表 → STCharacters 的最大正向匹配，完整 IDS 序列整体透传）。数据取自来源 `vendor/opencc` 子模块钉的同一提交 `26753884f1984add422f3b0249ccee8613deaff6`，登记在 `docs/third-party.md`，许可证由 `Collect-Notices.ps1` 收进 Windows 通知。
- 边界：`msime_client_simplified_to_traditional`（`crates/host-api`）返回裸文本而非 JSON 响应，因为它在每个候选上都要调用。首次调用解析词典，release 约 4 ms，之后单次约 1 µs。
- Windows：`platforms/windows/src/input/ChineseTextConversion.cpp` 改调这个导出；上屏、候选呈现的调用点不变。
- 证据：用 OpenCC 在该提交构建出的 CLI 做对照，4 万行随机文本逐字节一致；Rust 单测与 host-api 边界测试；Windows 侧为交叉构建与原生用例，未在真实编辑器里验收。
- Linux（ICU）与 macOS（`CFStringTransform`）仍是逐字转换，要统一只需改调同一个导出，本批不动。

### Linux 简繁转换改调共享 OpenCC 导出（2026-09-23）

上一条记的「Linux（ICU）仍是逐字转换」这次收掉。Linux 原来用系统 ICU 的 `Simplified-Traditional` transliterator，逐字映射解决不了一对多的字，「头发」会成「頭發」，与 Windows 现在的 OpenCC 词级输出不一致，还让 IBus 和 Fcitx5 两个宿主都硬依赖 `libicu`。

- `platforms/linux/src/system/ChineseTextConversion.cpp` 改调 `msime_client_simplified_to_traditional`，结果用 `msime_client_string_free` 释放；返回 NULL（非法 UTF-8、内嵌 NUL）时保留原文。C++ 签名不变，`ClientEngine.cpp` 与 `fcitx5/FcitxEngine.cpp` 的调用点不动。
- 去掉 `platforms/linux/CMakeLists.txt`、`platforms/linux/fcitx5/CMakeLists.txt` 里的 ICU `pkg_check_modules` 和链接，以及两份测试镜像里的 `libicu-dev`；仓库内没有别的 ICU 使用者。Debian 包的共享库依赖由 `dpkg-shlibdeps` 生成，随之不再带 ICU。
- 证据：`linux-traditional-output-test` 钉了与 `chinese_conversion.rs` 单测相同的词级对照（头发→頭髮、干面→乾麪、后天→後天、里面→裏面等）和 NULL 回退，Fcitx5 原生测试加了「头发→頭髮」。Linux 容器构建门禁（`platforms/linux/build-container.sh`，镜像已不装 `libicu-dev`）31/31 通过；Fcitx5 原生测试只编译链接，运行需要锁定资源目录，本次未跑。未在真实桌面会话里目视验收。
- macOS（`CFStringTransform`）仍是逐字转换。

### Server 不再弹控制台窗口；维护快捷键「停止」真正停下；诊断日志落盘（2026-09-23）

- 控制台窗口：`msime-client-server`（`MetasequoiaImeServer.exe`）原是控制台子系统程序，Watchdog 与 TSF DLL 每次拉起它都会带出一个黑色控制台窗口。来源的 Server 是窗口程序，没有这个窗口。现在链接为 Windows 子系统（MinGW `-mwindows`；MSVC `WIN32_EXECUTABLE` 加 `/ENTRY:wmainCRTStartup`，入口仍是 `wmain`）。`--config` 预览与 `--help` 从终端启动时挂到父控制台（`AttachConsole(ATTACH_PARENT_PROCESS)`），状态行与 Ctrl+C 照旧；受管启动（`--watchdog-managed` / `--production`）从不挂接，因为 TSF DLL 是在当前焦点程序里拉起 Server 的，那个程序本身可能是控制台程序。
- 停止快捷键（Ctrl+Shift+Alt+T）：原来只是让主循环退出、返回 0，Watchdog 把 0 当成非正常退出，两秒后又把 Server 拉起来，等于「停止」无效。现在返回 `watchdog::stop_exit_code`，与来源 `window_hook.cpp` 的 `ExitProcess(kStopExitCode)` 一致，Watchdog 随之退出。已打开的设置等 Tauri 窗口是独立进程，不随 Server 关闭，与「重启」时的行为相同。
- 诊断日志文件：受管模式下 stdout/stderr 没有去处，原来写到控制台的诊断无人可见。设置页「Server 端日志」「TSF 端日志」两个开关（`diagnostic_log.server` / `diagnostic_log.tsf`）原本只影响控制台输出，现在分别控制写入数据目录下的 `logs\server.log`：Server 启停原因、各组件是否就绪、退出码，以及 TIP 上报的诊断批次。来源写到桌面、失败再退回数据目录；这里固定写数据目录，因为输入法在桌面上凭空出现文件不是用户预期的副作用。文件达到 4 MiB 时轮转为 `server.log.1`，最多保留两份；新文件带 UTF-8 BOM，行尾 CRLF，与来源一致。只记状态，不记按键、输入内容或候选文本。偏好发布时立即生效，无需重启 Server。
- 证据：MinGW 交叉构建与 Wine 下的原生用例；未在 Windows 主机上目视确认。

### 检查更新改读本仓库的 Windows 发行版；Server 上报的版本号取自 version.txt（2026-09-23）

- 检查更新：来源的「关于」页读 `https://msime.app/update.json`，那份清单描述的是来源产品，`releaseUrl` 指向 `metasequoiaime/MSIME-Windows`。共享设置页在 Windows 上沿用了这个地址，而校验只接受本仓库 `metasequoiaime/msime/releases` 下的链接，所以 Windows 上每次检查都显示「检查失败」；就算放行，也会把用户带去下载另一个产品。现在 Windows 与其他平台一样读本仓库的发行版列表。
- 发行版按平台挑选：各平台由 `.github/workflows/release-*.yml` 独立发布到同一个仓库，标签带平台前缀（`windows-v1.2.0`、`linux-v1.2.0`）。原来的 `releases/latest` 只返回整个仓库最新的那一个，通常属于别的平台（写作时是 `macos-v…`），带前缀的标签又解析不出版本号，于是 Linux 等平台的检查同样一直失败。现在读取列表，只取本平台前缀、非草稿、非预发布的发行版，按版本号而不是列表顺序取最新；没有则显示「暂无可用发行版」。
- 版本号：Server 的遥测原本写死 `0.1.0-dev`。现在由 CMake 从 `platforms/windows/version.txt` 读入（发布工作流读的也是它），`Build-Client.ps1` 带 `-TargetVersion` 构建安装包时同一个版本号同时传给 Tauri 和 Server。
- 证据：设置页用例覆盖 Windows 读列表、跨平台与预发布过滤、按版本比较；MinGW 交叉构建。未在 Windows 主机上实际点「检查更新」。

### 外部链接改走 ShellExecuteW；Windows CI 门禁真正编译（2026-09-23）

- 外部链接：设置页「打开链接」在 Windows 上原来是 `cmd /C start "" <url>`，从 GUI 进程起一个控制台，会闪一下黑框，而且 URL 要经过 cmd 解析。现在走 `msime_host_windows::open_url`，用 `ShellExecuteW` 把 https URL 直接交给默认浏览器，非 https 一律拒绝；与已有的 `open_directory` 共用同一段调用。
- CI：`ci-platforms.yml` 的「Windows GNU cross build」跑在 ubuntu-24.04 上，那里的 MinGW 头文件没有 `d2d1_3.h`（`msimeui` 的 SVG 渲染要用），凡是真正进入构建步骤的运行都失败，develop 上的绿色只是因为构建被跳过。现在该作业跑在 `debian:trixie-slim` 容器里，环境与 `platforms/windows/cross/Dockerfile` 一致；改动 `ci-platforms.yml` 本身也会触发全部平台门禁，门禁的修改因此能被自己验证。
- 证据：`msime-host-windows` 与 `msime-desktop` 对 `x86_64-pc-windows-gnu` 通过 clippy（无新增告警）；本 PR 的 CI 运行即 Windows 门禁的验证。未在 Windows 主机上实际点击链接。

### Linux 先选中输入法、后做首次配置时有引导（2026-09-23）

来源的安装程序在安装时就把词库和出厂配置写进数据目录，输入法一能选中就能用，不存在「选中了但没准备好」这个状态。Linux 安装包按设计不产生用户状态，首次配置要由 `msime-linux-setup` 或设置窗口的首次配置页完成；此前用户若先在 IBus 里选中 MSIME，启动器只往 stderr 写一行就退出，界面上什么也看不到，Fcitx5 则只显示笼统的「MSIME：请检查运行配置」，两者都不告诉用户下一步做什么。

适配方式不是照搬安装程序（包管理器安装不应按用户写状态，也不能替用户决定是否联网下载词库），而是在这个状态下把用户引到已有的首次配置页：新增随装脚本 `platforms/linux/scripts/msime-linux-first-run-guide`，有图形会话时脱离调用方打开 `msime-linux-settings`（窗口在缺少 `runtime-options.json` 时自己进入首次配置页），并用 `notify-send` 发一条通知；IBus 启动器与 Fcitx5 插件在「没有显式覆盖、用户与系统配置都不存在」时调用它，Fcitx5 面板同时显示「水杉输入法尚未完成首次配置：请打开「水杉输入法」设置，或在终端运行 msime-linux-setup」。显式覆盖无效、文件不可读或悬空符号链接仍按配置损坏处理。

限流放在脚本里、两个宿主共用：`$XDG_RUNTIME_DIR/msime-client/first-run-guide.stamp` 存在就不再弹窗与通知，每个登录会话只引导一次，因为 ibus-daemon 每次选中都会重新拉起启动器、Fcitx5 每次聚焦都会激活输入法，按时间过期的冷却期会让继续打字的用户每隔几分钟被打断一次；会话没有 `XDG_RUNTIME_DIR` 时退到跨会话保留的缓存目录，只能按 5 分钟冷却期限流。Fcitx5 只在激活输入法时拉起引导，按键只显示面板提示，打字途中弹出的窗口可能抢走键盘焦点；插件另外把自身的拉起频率压到 30 秒一次。通知里的后续步骤按宿主区分：Fcitx5 下次按键就会重读配置，写「完成后即可直接输入」；IBus 组件已退出，写先切换到其他输入法再切回、仍不行就 `ibus restart`（后者未在真实 IBus 会话里验证）。脚本不创建状态目录（`msime-linux-setup` 拒绝准备已存在的目录），不发起任何网络请求。

证据：`platforms/linux/tests/core/first_run_guide.py` 用桩替换设置窗口、`notify-send` 与 `msime-linux-ibus`，验证有图形会话时各调用一次、同一会话内（包括记录很旧时）不再调用、并发调用只引导一次、缓存目录下冷却期过后与时钟回拨后恢复、按宿主区分的通知文案、无图形会话与配置损坏时不调用、只装输入法时通知改指向终端命令；`platforms/linux/tests/core/first_run_guidance.cpp` 覆盖 Fcitx5 的配置定位与「尚未配置」判定；`fcitx5_contract.py` 钉住激活与按键两条路径都走新提示且只有激活拉起引导；`platforms/linux/fcitx5/tests/native.cpp` 在真实插件上验证面板提示、按键不被拦截、按键不拉起引导、激活只拉起一次、不写失败诊断（该测试需要校验过的资源目录，构建门禁只编译不运行）。未在真实 Linux 桌面上目视确认弹窗与通知。

### Linux 候选外观补齐：纠错标记、英文字体、边框、悬浮工具栏主题（2026-09-23）

- 纠错标记：Fcitx5 的候选行此前不画 `*`，IBus 与 Windows 都画。现在 `FcitxCandidate` 在（简繁转换后的）候选文本之后、云/AI 角标之前加 `*`，与 Windows `event_listener.cpp` 和 IBus `ClientEngine.cpp` 同序；只改显示文本，选词走 session/generation/index，上屏文本仍是 Engine 原文。
- 英文字体：`candidate_english_font` 此前在 Linux 上可以保存、却没有宿主消费，`host_surface.rs` 因此对 Linux 隐藏该控件。现在两个 Linux 宿主写给桌面 panel 的 Pango 字体描述把英文字体排在主字体和补充字体之前（去重、去首尾空白），Pango 按字形逐个回落，效果与 Windows 先用英文字体、缺字再回落一致；未设置（缺省、null、空白）时描述与原来逐字节相同，仍按「未改动的默认值不覆盖桌面字体」处理，只选了英文字体也算一次字体选择。`HostCapabilities::candidate_english_font` 对 Linux 改为声明，设置页因此显示该行，未设置时说明为「跟随候选主字体」。
- 边框（历史记录：四套内置皮肤已被全局主题模型取代，见 `skin/theme.rs`，下面按皮肤列出的边框与 willow_green 的行为不再是现状）：Linux 调色板补上 Windows 皮肤的容器边框——fluent 浅色为黑色 0.12、深色为 `#9B9B9B` 0.18，wechat 为 `#DEDEDE`/`#292929`，graphite 为 `#E2E5E9`/`#30353B`，willow_green 无边框。外部皮肤在 Windows 上以 fluent 为底，因此默认带 fluent 的边框；皮肤包的 `border` 字段（`#rrggbb`、`#rrggbbaa` 或 `transparent`）经 `candidate_display_preferences` 生效，用户的 `candidate_border_color` 仍优先；自定义颜色沿用皮肤宽度，willow_green 始终无边框；皮肤包里的 `rgba()` 等本宿主不解析的写法保留原边框，与 Windows 对无法解析值的处理相同。Fcitx5 classic UI 用 SOURCE 运算符绘制边框，半透明边框会把桌面透出来，所以颜色在解析时先与面板底色合成为不透明色再写入 `BorderColor`。宽度取整数：Windows 原生卡片用 Direct2D 画 1.5 DIP 的抗锯齿描边（fluent；wechat/graphite 为 1），Fcitx5 classic UI 的 `BorderWidth` 只能是整数，所以所有带边框的皮肤在这里都取 1px，作为最接近且不改变布局的近似值；Background/ContentMargin 取 `max(2, 宽度 + 1)`，在现有宽度下保持 2，布局不变。IBus 的 lookup table 属性画不了边框，IBus 继续无边框，两端的颜色解析仍共用 `resolve_candidate_colors`。
- 悬浮工具栏主题：Linux 的悬浮工具栏是 IBus 属性菜单和 Fcitx5 状态菜单，由桌面 panel 按自己的主题绘制，没有 Linux 宿主读取 `toolbar_theme`（只有 macOS `FloatingToolbarPanel.mm`、Windows `server_main.cpp` 和 HarmonyOS `KeyboardSession.ets` 读取）。设置页照「菜单主题」的做法对 Linux 隐藏该项。
- 证据：Linux 构建门禁容器（GCC 12，`-Wall -Wextra -Werror`）编译并运行字体策略、调色板、Fcitx 主题用例；Fcitx5 原生用例的纠错标记断言在容器内手动运行；`cargo test -p msime-client-core host_surface`、clippy；设置页 vitest 与 `tsc`。未在真实 Fcitx5/IBus 桌面上目视确认边框、字体和标记。

### Linux 候选翻译只问用户选中的那一家（2026-09-23）

- 缺口：Linux 的腾讯密钥存放在 provider 自己的 `tencent-provider.json` 中，在设置页换成别的服务或选「关闭」，这个文件都不会被删。socket 协议只携带 `custom_translation` 和 `niutrans`；host-api 生成的 `tencent_tmt` 字段会在 `TranslationQuery` 反序列化时被丢掉，而且 Linux 偏好里本来就没有腾讯密钥，这个字段始终是空的。结果是 `msime-linux-online-provider` 只要收不到另外两家的配置，就去读腾讯文件并发出请求：选「关闭」、选 NiuTrans 但 App ID/API Key 不全、选自定义服务但 endpoint 为空，这三种情况下候选文字都会发给腾讯。来源 `cloud_translation.cpp` 的 `ActiveProvider()` 同一时刻只认一家（NiuTrans 优先，其次自定义，再次腾讯），`ResolveCredentials()` 在 `tencent_tmt.enabled=false` 时返回空，`WorkerLoop` 发现所选服务不可用时直接 `continue`，从不退回腾讯。
- 协议：`TranslationQuery` 新增 `provider`（`none` / `tencent` / `niutrans` / `custom`）。host-api 只根据三个 `enabled` 开关计算这个值，规则与来源 `ActiveProvider()` 一致，三家都没开时为 `none`；所选服务配置不全时也照实填写。协议里只说明选的是哪家，不传任何腾讯密钥。
- provider：`translations()` 按 `provider` 分派。`none`、未知值、所选的 NiuTrans 或自定义服务没有随请求带来配置，都返回空结果，不访问网络；只有 `provider=tencent` 时才读取腾讯文件。后续发请求也按所选服务分派，请求里夹带的其他服务配置不会改变去向。缺少该字段的请求来自旧版宿主，这时沿用旧规则（NiuTrans、自定义、腾讯依次取第一个可用的），混用新旧版本时行为不变。
- 宿主侧：`UnixSocketProvider::translate` 遇到 `none` 直接返回空结果，连本地 socket 都不连接，候选文字不会离开输入法进程。IBus 与 Fcitx5 引擎代码没有改动：它们转发的是 host-api 生成的查询，`provider` 变化会让去重键变化，从而重新发起请求。
- 证据：`platforms/linux/tests/dictionary/translation_provider_selection.py` 放了一份有效的腾讯凭据，断言关闭、NiuTrans 缺凭据、自定义缺 endpoint、未知服务这几种情况都不会发出网络请求，并断言只会请求所选的那一家，旧版宿主的请求保持原来的选择。该用例在改动前的脚本上失败 8 项，改动后全部通过。input-runtime 单测覆盖 `provider` 在 JSON 往返中不丢失、缺省时仍为缺省、`none` 不连接 socket；host-api 单测经 `msime_client_translation_provider_request` 走真实 socket，确认腾讯、NiuTrans（凭据不全）、自定义（endpoint 为空）三种选择原样到达 provider，关闭时不发生连接。未在 Linux 桌面上连接真实翻译服务做验收。

### HarmonyOS 2in1 硬件键盘从第一个字母起组字（2026-09-23）

2in1 上此前字母键被领进组合串后，预编辑与候选窗都不出现：硬件字母能延长组合串，却起不了组合串。诊断日志补上 `upper`/`shift`/`caps` 三项后，2in1 实例给出 `upper=true shift=false caps=false`：该设备对不带修饰键的字母键报出的 `unicodeChar` 是**大写**，而触摸路径送的是小写。Engine 的 `InputSession::handle_character` 只在已有组合串时把大写字母当辅助码收下，没有组合串时返回未处理。`HardwareKey` 注释里「shift 与 caps lock 已由系统应用」的假设对字母不成立。

修复分两处，都在宿主：`HardwareKeyRouter.normalizeLetterCase` 按 `shift XOR capsLock` 重建字母大小写，与 Windows 宿主从虚拟键与键盘状态推出字符同理；`press()` 与 `HardwareKeyDispatch.apply` 改为报告按键是否被消费，Engine 拒收的字母连同它的抬起一起交还应用，对应 TSF 的 eaten/not-eaten 模型——此前被领取后什么都没发生的键就是用户打了却看不见的字符。

同一轮设备验证还挖出三个此前把 2in1 整个挡在门外的缺陷，修复顺序即暴露顺序：

1. 保存过的自定义键盘设计里 `photo` 为 JSON `null` 时，`CustomKeyboardSkin.decodePhoto` 读 `null.length` 抛出，`onCreate` 不建会话、不建面板、不注册按键监听，输入法完全失效。
2. Rust 端 `phrase_prefix` 带 `skip_serializing_if = "String::is_empty"`，ArkTS 却把它声明为必填 `string` 并直接取 `.length`，于是每次 render 都抛。改为可选并以空串兜底。
3. `msime_client_personal_dictionary_sync` 按 `{options, action}` 信封解析请求，而 C 头文件写明、Android 与 HarmonyOS 两个调用方实际传的都是与 `msime_client_create` 相同的裸 HostOptions。每次调用都以 `invalid dictionary request` 失败：Android 静默吞掉，HarmonyOS 则每 2 秒重建一次 Engine 会话、永不停止。改 Rust 实现去服从头文件契约，并加 host-api 回归用例。

修复后在 2in1 实例的浏览器页内搜索框里注入 `n f d`，候选窗出现 `nfd / 1 你发的`，空格上屏 `你发的`，浮动工具栏同时显示。

### HarmonyOS 2in1 悬浮工具栏：手写板与语音按钮；表情与语音面板的返回键（2026-09-23）

- 按钮：macOS 工具栏多出的手写识别板与语音输入（第四十三批）现在也画在 2in1 的工具栏上，位置同 macOS：表情之后是手写，屏幕键盘之后是语音，齿轮最后。两个按钮读共享偏好 `floating_toolbar.handwriting` / `floating_toolbar.voice`，旧文档缺这两个字段时按共享默认值显示。工具栏最多九个按钮，宽度随之加宽。
- 手写：新增 `DesktopSurface.HANDWRITING`，候选窗打开的是手写板本身，不带手机上那排空格/回车/语言键，因为 2in1 上这些键由实体键盘负责。识别结果放在候选条上，与输入方案无关；关闭手写板会清掉笔迹。
- 语音：按钮打开的是已有的语音面（快捷键用的也是它），用户在面板里点开始。无论从哪条路径离开语音面，只要录音还在进行，就会取消，避免窗口关掉之后后台还在录。
- 能力位：`floating_toolbar_handwriting` / `floating_toolbar_voice` 由 `SettingsFormFactorCapabilities` 投影，只在 2in1 上报 true，所以设置页只在 2in1 上显示这两个开关。手机没有悬浮工具栏，仍然是 false。
- 顺带修复：表情面板的「‹」和语音面板的「返回」原来只调用 `show(SURFACE_NONE)`，那只改得了手机面板的状态。2in1 上由工具栏打开的是 desktop surface，这两个键点了没有任何反应，只能再点一次工具栏才能关掉。现在当前 desktop surface 正是这张面时，返回会关掉它；如果是在 2in1 屏幕键盘里打开的同一张面，返回仍然只退回键盘。
- `ToolbarButton` 新成员加在枚举末尾：`PanelSurfaceAction.SCREEN_KEYBOARD = 5` 按数值镜像屏幕键盘按钮，插到中间会让屏幕键盘快捷键打开别的面。按钮的显示顺序由 `buttons()` 决定，和枚举顺序无关。
- Rust 与前端依赖的通知：发布工作流在 Windows runner 上用 `platforms/linux/collect-notices.py`（与 Linux 发布同一个收集器）从 Cargo 解析出的 Windows 依赖图收集 `msime-host-api`、`msime-engine-bridge`（词库回放工具）和 `msime-desktop` 静态链接的 crate 许可证文件，再从 `apps/desktop` 的 `node_modules` 收集前端打包进去的 npm 包，两份文件作为 `-SupplementalNotices` 交给 `Collect-Notices.ps1`，随 `THIRD_PARTY_NOTICES.txt` 进安装包。

### HarmonyOS 2in1 硬件键盘：Ctrl+Shift+F 简繁、U 模式、音节分隔符与微软双拼的 ing（2026-09-23）

对照来源 `server/src/ipc/event_listener.cpp` 与 `input_key_policy.h` 逐键核查 2in1 硬件键盘，查实四处与 Windows 行为不一致，均在 `HardwareKeyRouter` / `InputModeRouting` 里修正。手机软键盘走 `Action::Character`，不受影响。

- Ctrl+Shift+F：来源 `HandleImeKey` 在 `IsCharacterSetShortcut` 上调 `SetConfiguredCharacterSet`，切换的是简繁；设置页这一项也写着「Ctrl+Shift+F 切换简繁」。本宿主原来把它和 Ctrl+Shift+Space 一样当成全角/半角。现在 `ModeGesture.TOGGLE_CHARACTER_SET` 只表示简繁，全角/半角改用 `TOGGLE_WIDTH`。简繁切换与工具栏「简/繁」按钮共用 `KeyboardSession.toggleCharacterSet`，写回 `traditional_chinese_output`。和来源一样，只在中文状态下切换，但开关打开时英文状态下也会吃掉这个组合键。
- U 模式：组字进行中数字 1–9 一律被当成选词、0 放给应用、`+` 被当成标点，于是 `u4e00`、`U+1F600` 都打不出来。现在 `local_mode` 为 `unicode` 时，不带 Shift 的 0–9 进入组字，Shift+1–9 选词（来源注释 “U-mode: Shift+1..9”），`+` 只在输入恰好是 `U` 时进入组字。规则与 Linux 宿主的 `unicode_digit` / `unicode_plus_key` 一致。
- 拼音分隔符 `'`：来源 `IsManualPinyinSeparatorKey` 把它送进输入串，本宿主原来把它当成标点，先上屏高亮候选再插一个引号。现在组字中、光标不在开头时进入组字，范围与 Linux 的 `accepted_apostrophe` 相同：非五笔的普通输入，或者 emoji、kaomoji、临时日语三种本地模式。
- 微软双拼 `;`：来源 `IsMicrosoftShuangpinIngKey` 把它当作 ing 韵母。现在视图报告 `microsoft_shuangpin` 时，只要它是本音节的第二键（从最后一个 `'` 数起按键数为奇数），就进入组字；否则仍是标点。

路由所需的状态由 `KeyboardSession.hardwareSpelling()` 从最近一次引擎视图中读出，通过 `HardwareSpelling` 传给 `HardwareKeyRouter.route`。这几条规则放在翻页键判断和数字选词之前，因为 Shift+= 本身就是 `U+` 里的 `+`。

### HarmonyOS：设置页保存后键盘立即生效（2026-09-23）

来源里设置程序保存后发 `WM_APPLY_IME_CONFIG`（`server/src/config/ime_config.cpp:1566`），服务端收到后重读配置并刷新候选窗与工具栏，另有 300 ms 的 `TIMER_ID_CONFIG_SYNC` 兜底（`server/src/window/ime_windows.cpp:2199`、`2673`）。本宿主的设置页（EntryAbility）与键盘（InputMethodExtensionAbility）是两个进程，键盘只在 `onCreate` 读一次偏好，所以设置页改了方案、皮肤、键盘高度、工具栏或模式切换键后，要等输入法进程被系统回收重建才会生效。

适配方式不照搬定时器：键盘记住建会话时读到的偏好文档 `revision`，每次编辑框获得焦点（`attach`）时比对一次存储中的 `revision`，这正是用户能用新设置打字的最早时刻，代价只是一次小文件读取。变了就走现有的 `restartIdleSession` 重建引擎会话（保留中英文与九宫格状态），随后通知两处：

- 视图（`onViewPreferencesReloaded`）：重读皮肤、几何、方案与键面，手机上按新高度调整面板；
- 输入法扩展（`onPreferencesReloaded`）：重新绑定模式切换键；2in1 上按新设置开关浮动工具栏与模式角标（两者共用唯一的 STATUS_BAR 面板，工具栏优先），已开的工具栏按新按钮集重排。

正在组字或处于本地模式（U 模式、emoji 等）时不重建，下次获得焦点再取。云同步把偏好写入同一文档，同样通过这条路径生效。键盘自己经 `changePreferences` 写入时会同步记下新 `revision`，不会因此触发重建。

### HarmonyOS 2in1 硬件键盘：Ctrl+Shift+E 英文候选模式（2026-09-23）

纠正上文「`Ctrl+Shift+E` ……`InputModeRouting` 已实现」的判断：本宿主原来把它和 Shift 轻点一样当成中英切换，进入的是直接英文，硬件字母原样交给应用。来源里这是两件事：Shift 由 TSF 切中英（`FUNCTION_TOGGLE_IME_MODE`，上屏原始字母），`Ctrl+Shift+E` 是 `IsEnglishModeToggleKey` → `SetEnglishInputMode` + `ClearState`，打开服务端的英文输入模式，字母进入组字、候选框列英文词（`UpdateEnglishInput`），TSF 侧按 `FUNCTION_CANCEL` 处理，什么都不上屏。macOS 宿主也区分两者（`toggleDedicatedEnglishMode:`，工具栏显示 En）。

本宿主的引擎侧本来就是 dedicated-English 标志，差别只在硬件键是否送进引擎。现在 `ModeGesture.ENGLISH_CANDIDATES` 调 `KeyboardSession.toggleEnglishCandidates()`：打开英文模式并记下英文候选子模式，`HardwareKeyRouter` 此时把字母送进组字，音节分隔符、U 模式和微软双拼的特殊键规则都不生效（引擎在该模式只收字母），标点和中文态一样结束组字后按标点设置输出。再按一次、Shift 轻点或工具栏语言按钮都回到中文；密码与网址类编辑框强制直接英文。工具栏语言按钮显示 `En`，与来源和 macOS 一致。手机软键盘的字母一直走引擎，两种英文在手机上本就是同一个模式，不受影响。

### HarmonyOS 2in1：表情面板的剪贴板页（2026-09-23）

来源的表情面板最后一页是剪贴板历史（`server/src/emoji-panel/EmojiPanel.h` 的 `Page::Clipboard`）。本宿主的剪贴板历史原来只能从手机键盘的工具面板进入，2in1 没有入口。现在 2in1 的表情面板（工具栏 ☺ 打开的 `DesktopSurface.EMOJI`）在「表情 / 颜文字 / 符号」之后多一个「剪贴板」页：点条目上屏，可固定、删除、清空（二次确认）。手机不加这一页，仍从工具面板进入，两处共用同一份条目列表 `clipboardEntries()`。

与来源的差异：来源由 `ClipboardMonitor` 在后台记录每次复制；HarmonyOS 只把剪贴板读取权限给系统应用，本宿主也不在后台记录，所以这一页和手机一样用「保存当前」按钮手动保存。设置里关闭剪贴板历史时这一页只显示提示，来源的「启用」按钮不照搬，开关统一留在设置页。

### HarmonyOS 2in1：全屏应用上隐藏悬浮工具栏

来源的窗口钩子（`window_hook.cpp` 的 `OnWinEvent`）在前台窗口全屏时隐藏悬浮工具栏，退出全屏后按配置恢复，游戏或视频画面上不会压着一条没人在用的状态栏。HarmonyOS 以前在 2in1 上始终显示工具栏。

- 现在编辑框每次获得焦点时，都用 `InputClient.getCallingWindowInfo()` 查询所在窗口的状态：`WindowStatusType.FULL_SCREEN` 时隐藏工具栏，焦点回到非全屏窗口时再显示。
- HarmonyOS 不会主动通知输入法窗口状态的变化，只能在查询时拿到，所以编辑框保持焦点期间才进入全屏的窗口，要到下一次聚焦时才会生效。
- 工具栏被设置关闭后再重新打开时，隐藏标记会随之清掉，不会把新开的工具栏误当成已隐藏。
- 手机没有悬浮工具栏，这一改动不涉及手机。

### HarmonyOS 2in1：行内预编辑（`tsf_preedit_style`）

来源用 TSF 在文档里画正在拼写的内容：`raw` 是按下的字母，`pinyin` 是分好词的拼音，`empty` 则不画。Linux 的 IBus 和 Fcitx5 读的是同一个字段。HarmonyOS 以前在任何形态下都只在候选区显示拼写，文档里什么也没有，「行内预编辑」这个设置在鸿蒙上也就不起作用。

- 2in1 现在通过编辑框的预上屏文本（`InputClient.setPreviewTextSync`）按设置显示拼写。已经选定的半个词（`phrase_prefix`）排在拼写前面，和候选窗的画法一致。提交时用 `insertText` 替换预上屏文本；取消或拼写清空时写入空预上屏，再 `finishTextPreview`。
- 只有编辑框的属性声明了 `isTextPreviewSupported` 才启用。编辑框接口报错（例如 12800011）时，这次聚焦期间停用预上屏；这时如果文档里还留着拼写字母，就结束预上屏，并在确认光标前正是这些字母之后删掉它们。
- 每次预上屏变更都和插入一样登记为键盘自己的编辑，所以编辑框回传的 textChange 不会被当成用户改了文本，也就不会结束组字。
- 预上屏期间，提交一律走同步插入，保证它排在下一次预上屏更新之前；智能标点读取光标前的字符时会去掉预上屏部分，读到的是用户写下的文字，而不是拼写字母。
- 手机不变：拼写仍显示在按键上方那一条，手机没有贴着光标的候选窗，这个设置在手机上不生效。
- 未在真机或模拟器上验证预上屏的回声和替换行为，上面的处理依据的是 SDK 文档中的接口约定。

### HarmonyOS 2in1 硬件键盘：Home / End 跳到候选列表首尾

- Windows 在有候选时把 Home / End 映射成 `FUNCTION_MOVE_PAGE_TOP` / `FUNCTION_MOVE_PAGE_BOTTOM`，焦点跳到候选列表的第一项和最后一项。2in1 现在也这样处理：有候选时 Home 发 `FIRST_CANDIDATE`（104），End 发 `LAST_CANDIDATE`（105，host-api 会先展开整张列表再取最后一项）；没有候选时仍然是移动编辑光标。
- 手机不受影响：Home / End 只从硬件键盘分发进来，手机的软键盘没有这两个键。

### HarmonyOS 2in1：菜单主题（`menu_theme`）

- 订正（2026-09-23）：Windows 的 `menu_theme` 只决定托盘语言菜单是深色还是浅色，候选右键菜单并不读它。来源 `windows_webview2.cpp` 用 `GetConfiguredThemeMenu()` 选托盘菜单页面，`tray_menu_presenter.cpp` 同样只给托盘菜单取色；候选右键菜单的 `menuFill` / `menuBorder` 在 `candidate_presenter.cpp` 里跟着候选窗主题（`candLight`）和皮肤走。本仓 Windows 宿主一致，`server_main.cpp` 只把 `menu_theme` 交给 `TrayMenuWindow`。鸿蒙 2in1 没有托盘；它把候选词管理条按 `menu_theme` 配色，跟随全局时和其他面板一样回落到全局主题，这是借用这个设置的平台取舍，不是 Windows 的对等行为。常用标点、括弧、释义这几条属于键盘本身，仍然用键盘配色。
- 手机的设置页不显示这一项（`mobile_settings` 为真），手机上的管理条也继续用键盘配色。

### HarmonyOS 硬件键盘：全角模式与交给应用的字符统计

- 全角：Windows 开着全角且候选窗未打开时，会吃掉每个可打印 ASCII 键（`' '` 到 `'~'`，`CompositionProcessorEngine.cpp` 的 `IsDoubleSingleByte`，由 `KeyEventSink.cpp` 分类为 `FUNCTION_DOUBLE_SINGLE_BYTE`），改为插入全角形式。鸿蒙原先把这些键直接交给应用，于是英文字母、空闲时的数字和空格、英文模式的标点、引擎拒收的大写字母都以半角出现。现在路由在全角开启且未组字时返回 `WIDEN`，会话按 `FullWidthInputPolicy` 插入（空格变成 U+3000，与其他路径一致）；引擎拒收的大写字母也改为插入全角。Ctrl / Alt / Win 组合键仍交给应用；中文模式的标点仍走标点路径，那条路径本来就会把字面标点转成全角。
- 统计：Windows 会把交给应用的字符也记进打字统计（`stats_passthrough.h` 的 `ShouldCountPassthroughChar`：可打印、未按 Ctrl / Alt / Win，Shift 可以；控制字符和 DEL 不算）。鸿蒙原先只统计自己插入的文字，硬件键盘直接交给应用的字符漏记。现在按键按下且被交给应用时按同一规则计入（`TypingStatisticsPolicy.countsPassthrough`）；键盘不组字的编辑框（例如密码框）不计，这是本仓的取舍：输入法不记录密码框里敲了多少字。
- 手机接实体键盘时走的是同一条硬件键路径，所以行为一致；软键盘本来就由输入法自己插入文字，不受影响。

### HarmonyOS：切换中英文时上屏的原始字母不再学成英文词

- Windows 只在按 Enter 时把输入的原始字母学成英文词（`event_listener.cpp` 里 `VK_RETURN` 分支调用 `ShouldLearnEnteredEnglishWord`）。Shift 切换中英文时 `_HandleToogleIMEMode`（`KeyHandler.cpp`）只把按键缓冲原样上屏，不学习。
- 鸿蒙切换中英文时发的是 `MSIME_COMMIT_RAW`（2）。这条命令会走 `commit_raw_with_policy`，按 Enter 的规则学习；在 Ctrl+Shift+E 英文候选模式下，引擎自己的原始上屏也会学习。于是在中文模式下打了 `hello` 再切到英文，`hello` 会被记进英文词库。
- 共享层新增 `MSIME_COMMIT_RAW_WITHOUT_LEARNING`（15）：同样把字母原样上屏，但不学习。在英文候选模式下，它读取预编辑后取消组字，绕开引擎自带的学习；临时模式的引导字母照 `InputSession` 的做法去掉。鸿蒙切换中英文的边界改用这条命令；Enter 和触屏上的原样上屏仍用 2，继续学习。手机和 2in1 走的是同一条路径。
- Linux 的 Ctrl+Space 等切换组合键（`FcitxEngine.cpp`）同样发 2，存在同样的差异。那是 Linux 宿主的事，这里没有改动。

### Windows：候选右键菜单每次打开都针对当前候选；托盘切换悬浮工具栏不再关菜单（2026-09-23）

- 候选右键菜单：`CandidateWindow::show_context_menu` 只在第一次右键时创建 `CandidateFlyoutWindow`，选择回调按值捕获了那一次的 `CandidateClick`（会话、代次、候选序号），此后每次右键复用同一个浮出窗，置顶 / 删除 / 固定排位发出的都是第一次右键那个候选——在会话或代次已经变化后被 Engine 拒绝，或者落到同序号的另一个词上。同一段代码也只在创建时设过一次配色，之后切换皮肤、候选窗主题或系统深浅色，右键菜单仍是旧色。来源每次打开都重建菜单（`candidate_presenter.cpp:560-563` 刷新配色，`:698-765` 按当次候选建菜单）。本仓仍复用浮出窗（它持有两个窗口和两套 Direct2D 设备），改为每次打开时记录当次目标并重设配色：目标由 `CandidateMenuLayout.h` 的 `CandidateMenuTarget` 保存，一次选择消耗一次打开；菜单行到 Engine 动作的映射抽成 `candidate_menu_action`。`windows-candidate-menu-layout` 覆盖连续多次打开、未选即再开、子菜单行不消耗目标等情形。
- 托盘菜单：来源的「悬浮工具栏」行是开关，点了原地翻转、菜单留着（`tray_menu_presenter.cpp:175-186`），其余行先 `Hide()` 再打开对应界面（`:188-199`）。本仓以前对所有成功的命令都关菜单。现由 `TrayMenuLayout.h` 的 `tray_menu_closes_after` 决定：只有工具栏开关留着菜单，并按 Server 报告的实时状态重画这一行。`windows-tray-menu-layout` 覆盖。
- `CandidateMenu.h` 核对后不接入真实菜单：它是扁平的一级菜单（置顶、第 1–5 位、取消固定、删除平铺），而来源和本仓实际使用的 `CandidateMenuLayout.h` 都是「固定排位」带二级子菜单的结构；两份头文件定义了同名的 `CandidateMenuCommand` / `CandidateMenuItem`，不能同时包含。目前只有 `tests/ui/candidate_menu.cpp` 引用它，保留不删。第十八批那句「`CandidateMenu.h` 的注释直接引了来源行号」说的单码点不给删除的规则，真实菜单里由 `CandidateMenuLayout.h` 的 `candidate_menu_items` 实现，行为相同。
- 验证层级：两个菜单的规则在本机用 clang 编译并运行对应的头文件测试；Windows 窗口代码经 `build-cross.sh x64` 交叉编译通过，没有在 Windows 桌面上实际点过。

### Windows 语音与来源逐项对齐：录音上限、失败提示、空格锁定、豆包数组结果、快捷键文案（2026-09-23）

- **批量录音上限**：原先固定 60 秒，超过后在提交时报「录音超过 60 秒上限」并丢掉整段。现在上限取共享的 `batch_capture_sample_limit`，采集回调按 `voice_batch_capture` 只收下剩余空间并标记已满；Server 主循环每一轮调用 `VoiceInputSession::maintain()`，发现已满就走正常的 `stop()`，提交已录部分，与 macOS 相同。流式豆包不在本地缓冲音频，不受这个上限约束。
- **失败提示**：缺 Token / 接口地址 / 模型、豆包 `Start()` 失败、麦克风打不开、录音途中采集回调失败（`AudioCapture::callback_failed()`，由 `maintain()` 在录音期间与 `stop()` 时检查）、豆包结束时的 `LastError()`，以及批量识别失败都会给出提示。批量识别失败时显示共享 `CloudAsrError::user_message()` 带出的服务商消息或 HTTP 状态，拿不到时才用「语音识别失败」。提示显示在语音浮层上（理由见「与来源刻意不同的取舍」）。原先的失败提示在控制线程上 `Sleep(1200)`，而浮层窗口也在这条线程上，显示与隐藏两条消息被连着处理，提示实际上从未出现；现在由后台任务轮询 4 秒后隐藏，控制线程不再阻塞。走 Tauri 语音面板的录音仍由面板报告失败，不弹浮层。
- **空格锁定**：按住录音时按空格锁定，浮层显示 ✓ / ✗ 两个按钮，对应来源 `ControlLoop` 的 `set_actions_visible(true)`。
- **豆包数组结果**：`bigmodel_nostream` 的 `result` 可以是分段数组，`DoubaoTranscript.h` 按顺序拼接各段 `text`，对象形式与 `payload_msg` 信封照旧，与来源 `ExtractTranscript` 一致。
- **快捷键文案**：共享设置页只在 Windows 上改为「长按右 Alt 录音」「长按右 Ctrl+右 Alt 录音」「长按 Ctrl+Win 录音」「长按录音时按空格锁定」，说明文字写明松开结束、空格锁定后再按快捷键或 ✓ 结束、Escape 或 ✗ 取消；macOS 与其他平台的文案不变。
- **验证**：新增 `windows-voice-session-policy` 与 `windows-doubao-transcript` 两个纯逻辑 ctest，在 macOS 上用宿主编译器经 `scripts/test-windows-native-run.py` 运行通过，撤掉改动（60 秒上限、只认对象形式）时会失败；x64 MinGW 交叉编译链接通过；设置页用例在 `apps/desktop` 的 vitest 下通过。没有在 Windows 上实际运行。

### Windows 宿主读取共享设置页已提供的几项设置（2026-09-23）

一次审计找出共享设置页在 Windows 上显示、但 Windows 宿主从不读取的几项设置。逐项对照来源后的处理：

- **五笔剩余编码提示（`wubi_code_hint`）**：候选投影按 macOS 的 `WubiCodeHintPolicy` 规则算出每个候选在已输入前缀之后剩下的编码（仅五笔方案、严格前缀、非拼音回退、非本地模式），偏好开启时候选窗把它写成注释 `(hy)`，与 macOS 的写法相同。开关和候选排列方式放在同一个原子值里下发，所以切换它会让同一代候选重新绘制。来源没有这项设置，见上文「辅助码」一条。用例：`windows-wubi-code-hint`、`windows-candidate-layout-reload`。
- **悬浮工具栏的中英切换按钮（`floating_toolbar.english_mode`）**：来源的中/英按钮始终显示，没有这个开关；共享设置页提供了它，macOS、Linux 和设置预览都遵守，Windows 只读另外六项。现在 Windows 也读它，缺省为显示，因此不改这项的用户看到的仍是来源的样子。用例：`windows-floating-toolbar-reload`。
- **屏幕键盘高度（`touch_keyboard_height_adjustment`）**：Windows 的屏幕键盘是 Tauri 面板，固定按 1100×400 打开。现在按设置预览的同一算式 `400 + clamp(调整值, -12, 48)` 决定窗口高度，对设置页命令、托盘菜单启动路由和二次启动三条打开路径都生效；窗口已存在时重新打开会按新高度调整。页面本身按窗口高度伸缩，不需要另外传参。其他宿主不在本次范围内。
- **每页候选项数量**：见上文 2026-09-21 那条的订正。共享设置页现在对所有宿主列出来源的 3–9；共享偏好仍接受 1–9，文档里存着 1 或 2 时这个值留在列表里并保持选中，保存别的改动不会把它改写掉。
- **候选英文释义的默认值（`candidate_english_gloss`）**：来源默认显示 `english.db` 的释义，共享默认关闭。这一项**刻意保留差异**，不单独改 Windows 的默认：共享偏好没有按平台区分默认值的机制（`HostCapabilities` 只描述能力，不带偏好默认值），所有宿主都读同一份 `Preferences::default()`，而 Linux、Android、HarmonyOS 的测试和 README 都把「默认关闭」写成了约定。为 Windows 单独翻默认值需要先引入按平台的偏好默认，这属于产品取舍，留给所有者定。

### Windows 面板、托盘、悬浮工具栏与检查更新的对齐（2026-09-24）

- 屏幕键盘：来源的按键在松开时才发送，按下后把指针滑出按键再松开就取消。本仓以前在按下时立即发送，还带长按连发。现在 Windows 上按下只记录按下的键，并释放隐式的指针捕获；只有主按钮在同一个键上松开才发送，`pointercancel` 清除记录。滑出后再滑回原键松开仍然发送，和来源一致。修饰键、NumLock、键盘或辅助技术触发的点击（`detail` 为 0）仍按点击处理。其他平台保持按下即发送、长按连发。`apps/desktop/tests/input/keyboard-repeat.test.tsx` 覆盖这一行为。
- 手写笔画上限：来源不限制笔画数。本仓的共享面板在其他平台停在 32 笔（与 Linux socket provider 的上限一致）；Windows 现在放宽到共享契约 `crates/client-core/src/panels.rs` 的 `MAX_STROKES`（64 笔），超出后仍然提示「笔画已达上限」。剩下的差异是：来源无上限，本仓在 64 笔处停止，这是识别请求的共享边界，没有改动。这项修改不需要 `crates/host-api/src/handwriting_cells.rs`。`apps/desktop/tests/core/handwriting-pointer.test.tsx` 覆盖这一行为。
- 表情与手写面板的尺寸和位置：来源的表情面板是 550×610 的窗口（`server/src/emoji-panel/main.cpp`），手写面板也在工作区居中打开。本仓以前统一放在工作区底部居中。现在由 `SurfaceRoute::panel_for` 按宿主给出几何：Windows 上表情面板为 550×610、居中，手写面板保持共享尺寸 980×650、居中，键盘和其他面板仍在底部居中；其他平台不变。已知差异：本仓的尺寸是逻辑像素，来源按物理像素建窗；居中计算沿用原有的底部居中换算，高 DPI 下的差异此前就存在，这次没有改动。
- 检查更新：来源在更新提示里给出安装包的 SHA-256 和未签名警告，并在十秒后放弃请求。本仓以前对 Windows 发行版不读取资产，也没有超时，网络卡住时按钮会一直处于忙碌状态。现在从发行资产中选出唯一的 `MetasequoiaIME_Setup_v*.exe` 和它的 GitHub digest，标记为未签名，警告改用来源的措辞（SmartScreen 拦截、uiAccess 失效），并给出 `Get-FileHash` 命令。没有 digest 时，提示用户下载同名 `.sha256` 文件核对。请求十秒后中止，并显示「检查失败，请稍后重试」。超时是所有平台共用的。
- 托盘菜单与悬浮工具栏的配色：来源的原生托盘菜单（`TrayMenuPresenter::ApplyTheme`）和悬浮工具栏（`FloatingToolbarPresenter::ApplyTheme`）使用固定的中性色，只随各自的深浅色偏好切换，不跟随候选皮肤。本仓是有意的分歧：两者跟随全局主题，由 `CandidatePalette.h` 的 `toolbar_palette(CandidatePalette)` 和 `tray_menu_palette(CandidatePalette)` 从解析后的候选窗调色板派生（工具栏取底色、正文、悬停、描边与选中色，托盘取菜单的底色、正文、悬停与描边）。来源行尾的开关在本仓画成工具格（`TrayMenuWindow.cpp` 的 `TrayMenuRowKind::Tool`）：开启时整格填主题的 accent，图标与说明取 `candidate_on_accent`（按亮度取黑或白，所以 ink 的白色 accent 上仍然可读）；关闭时和普通行一样，只在悬停时填 hover。托盘中能力缺失的行用 `number` 色变暗而不是隐藏；来源的菜单没有这种行，所以这是本仓自己的取值。`windows-candidate-palette` 覆盖这些颜色。
- 打字统计的 30 天明细表和永久保留由 PR #652 处理，这一批没有改动。
- 验证层级：TypeScript 部分在本机跑了 vitest 和 typecheck；`msime-client-core` 在本机跑了 cargo test；配色头文件测试在本机用 clang 编译并运行；Windows 窗口和面板定位代码经 `build-cross.sh x64` 和 `cargo check --target x86_64-pc-windows-gnu` 交叉编译。以上都没有在 Windows 桌面上实际操作过。

### Windows：启动与崩溃上报改为用户开启，默认关闭（2026-09-24）

- 原状：`server_main.cpp` 在 `wmain` 第一行同步调用 `msime::telemetry::start`，Server 每次被拉起都先向 `https://api.msime.app/v1/telemetry/events` POST 一次、写一次 `%LOCALAPPDATA%\MSIME\telemetry.json`，端点慢或不可达时最多拖住启动 8 秒；`std::set_terminate` 的崩溃回调同样无条件上报。没有任何偏好能关掉它，安装器「联网功能」页却写着云候选是「唯一一个装完就会生效的联网功能」。
- 偏好：共享层新增 `telemetry_enabled`，默认 `false`，缺省的旧文档读作 `false`，所以升级不会替用户打开。设置页「关于」里的「匿名使用统计」开关只在 `host.platform === "windows"` 时显示，说明文字逐项写出发送内容（随机事件 id、类型、平台名 windows、版本号；崩溃事件另带固定文本 `std::terminate`）和端点。其余平台的上报行为不变，也不显示这个开关，因为它们的宿主不读它；macOS 的 `preference_coverage.py` 因此把它列为不适用。
- Server：读到已保存的偏好后由 `TelemetryConsent.h` 的 `telemetry_consented` 判断，只有显式 `true` 才算开启。开启时启动事件改在不等待的后台线程里发送，与 Linux 的 IBus 宿主一致，不再阻塞启动；关闭时不发送、不写 `telemetry.json`。崩溃回调读一个随偏好发布更新的原子开关，设置页保存后立即生效；启动事件在下一次启动时按新值决定。`curl_global_init` 移到 `wmain` 开头、任何线程创建之前。
- 安装器：「联网功能」页补上一句，说明匿名使用统计默认关闭、要在「设置 → 关于」开启，以及开启后发送什么。
- 证据：`msime-client-core` 的 `telemetry_is_opt_in_and_survives_a_save`（默认、旧文档、非布尔值拒绝、保存往返）；设置页用例覆盖 Windows 显示且默认关闭、保存写入 `telemetry_enabled: true`、其余五个平台与无宿主时不显示；`windows-telemetry-consent` 在本机用 clang 编译运行，并做过变异检查。`build-cross.sh x64` 交叉编译链接通过（含 Server 与该用例）。没有在 Windows 主机上运行 Server，也没有抓包确认开关关闭时不发请求、开启时启动不再被阻塞；安装器文案未在真实向导里看过。

### 安装器升级与卸载生命周期（2026-09-23）

对照来源 `installer/msime_setup.iss` 与 `server/src/main.cpp` 逐条核对后，下面六处差异都成立，一并修掉；另外顺带发现一处数据目录标记的问题。

- **卸载删不掉自定义数据目录**：`DataDir` 注册表值带 `uninsdeletevalue`，卸载步骤里就已被删掉，`usPostUninstall` 再调 `GetDataDir` 只会回落到默认的 `{localappdata}\metasequoiaime`，自定义目录因此永远留在磁盘上。现在 `InitializeUninstall` 像缓存 `VersionDirName` 一样先调 `ResolvePreviousDataDir` 缓存路径，`usPostUninstall` 只删这个缓存路径，且仍要求它带所有权标记。注册表值照旧删除。
- **数据目录的权限与完整性级别**：Server 与设置窗口以 Medium 完整性级别运行，写不了提升权限的安装器创建的对象。与来源相同，`[Dirs]` 给数据目录加 `Permissions: users-modify`，`ssPostInstall` 在写完词库、配置与词典重放之后再执行 `icacls /grant *S-1-5-32-545:(OI)(CI)M` 与 `/setintegritylevel (OI)(CI)M`，以覆盖安装器之后写入的文件。代价与来源一样：数据目录在每用户的 LocalAppData 下，Users 组的修改权限也让同一台机器上的其他本地用户能改它。
- **升级与卸载不停设置窗口和面板**：设置窗口和表情 / 屏幕键盘 / 手写面板是同一个 Tauri 外壳 `msime-client-settings.exe`（面板由 `MSIME_CLIENT_PANEL` 选择），窗口关闭后隐藏驻留约 10 分钟，升级时会报文件占用。`StopImeProcesses` 依次停 Watchdog、Server 与这个外壳，升级和卸载都只经它停进程；安装包里其余可执行文件（`MetasequoiaImeDictionaryReplay`、`msime-client-prepare`）是一次性进程。
- **TSF 拉起的 Server 不恢复 Watchdog**：来源 Server 启动时若命令行里没有 `--watchdog-managed`，就拉起同目录的 Watchdog。本仓库的 `parse_server_arguments` 把 `--production` 与 `--watchdog-managed` 合并成 `Managed`，区分不出来，于是 TSF DLL 用 `--production` 重启崩溃的 Server 后，要到下次登录才重新有 Watchdog。`ServerLaunch` 新增 `supervised`，只在 `--watchdog-managed` 时为真；受管启动拿到单实例锁后，若非 Watchdog 所启，就用 `CreateProcessW` 拉起同目录的 `MetasequoiaImeWatchdog.exe`。Watchdog 有自己的单实例互斥量，会接管已在运行的同目录 Server 而不再起第二个，TIP 未启用时自行退出。
- **登录任务的 `/TR` 未加引号**：schtasks 把不带内层引号的 `/TR` 值在第一个空格处拆成程序与参数，`C:\Program Files\...` 会被存成程序 `C:\Program`。与来源相同，改回 `/TR "\"<路径>\""`（c306e5985 当初去掉了内层引号）。`watchdog-task.ps1` 除了静态检查，还会在提升权限的 Windows 上把安装器实际传给 schtasks 的整串参数以临时任务名注册一次，读回任务 XML 核对 `Command` 就是完整路径、`Arguments` 为空，结束后删除任务。
- **开始菜单的「设置」找不到 `DataDir` 里的 `runtime-options.json`**：Server 拉起外壳时通过 `MSIME_CLIENT_STATE_DIR` 与 `MSIME_CLIENT_HOST_OPTIONS` 指路，开始菜单快捷方式没有这两个变量，外壳只在自己的 `app.msime.client` 目录和 `%LOCALAPPDATA%\MSIME-Client` 下找，装在默认 `{localappdata}\metasequoiaime` 或自定义目录时都找不到。现在没有这两个变量时，外壳按 Server 的 `production_state_directory` 同样的顺序（`METASEQUOIA_IME_DATA_DIR`、HKLM 64 位视图的 `DataDir`、`%LOCALAPPDATA%\MSIME-Client`，注册表读取在 `msime_host_windows::server_state_directory`）找 Server 的状态目录，其中已有 `runtime-options.json` 时就用它，状态目录取其中的 `preferences_directory`（与 Server 相同）。
- **升级清理删掉了所有权标记**：`CleanAppDataExceptUserFiles` 在 `PrepareToInstall` 写入 `.metasequoiaime-data` 之后、`ssPostInstall` 之前执行，保留名单里没有标记文件，升级中途失败时重试和卸载都不再认这个目录。标记现在列入保留名单。

回归：新增 `installer/tests/lifecycle.ps1`（进程停止顺序、卸载用缓存路径、权限与标记），`watchdog-task.ps1` 改为要求内层引号并加 Task Scheduler 往返，`tauri-layout.ps1` 原本断言一个已不存在的 `DataDirIsSafe`，改为断言现在的 `DataDirRejectionReason`；这几个套件与 `tsf-registration.ps1` 接进 CI 的「Windows release script tests」。`windows-installer-launch` 另外钉住卸载使用缓存路径、安装器的 `--production` 启动不算受监管；`windows-server-launch` 钉住 `supervised` 的取值。发布流水线上的 `install-smoke.ps1` 改用 `/DATADIR` 指定的自定义数据目录安装，安装后核对登录任务存下的程序是完整的 Watchdog 路径且没有参数、数据目录授予 Users 修改权限并带 Medium 完整性标签，卸载后核对这个自定义目录已被删除。

### Windows 对齐遗留四项（2026-09-24）

- **数据目录迁移改为移动，升级不再清掉 Server 状态。** 目标的 `DataDir` 是 Server 的状态根（`preferences.json`、`user\` 下的用户词库、`cache\`、`logs\`、`runtime-options.json`、统计与剪贴板历史等），安装器却按来源布局只认 `msime_user.db`、`config.toml` 与 `skins`：`CleanAppDataExceptUserFiles` 每次完整升级都把其余状态删掉，换目录时 `MigrateUserDataDir` 也只复制这三样且从不删旧目录。现在升级只清安装包自己的条目（`IsPackageAppDataItem`，按 `Prepare-PackageFiles.ps1` 的 app_data 清单加来源布局独有的几项）。换目录与来源一样是移动：复制旧目录里除安装包条目、所有权标记、写入探针和 `runtime-options.json`（记着旧目录的绝对路径，由 Server 首次启动在新目录重新准备）之外的全部条目；删除推迟到 `ssPostInstall` 最后（`FinishDataDirMove`），词典重放或登录任务失败时旧目录原样保留，且只删带所有权标记或默认位置的目录。来源的 `DelTree(OldDir)` 在新目录嵌在旧目录里时会把刚迁过去的数据一并删掉；这里此时只删旧目录顶层中不含新目录的条目。`windows-installer-launch` 钉住这些契约（改动前的脚本在第一条就失败）；发布流水线的 `install-smoke.ps1` 增加原地重装保留状态、迁到嵌套新目录后状态齐全、旧 `runtime-options.json` 未带过去、旧目录只剩新目录这几项实机检查。
- **全新安装起手英文。** 来源与随包模板都是 `default_ime_mode = "english"`，但没有 `preferences.json` 时 Server 的模式权威和 TSF 读的是共享默认（中文）。`DefaultImeMode::default()` 现在在 Windows 上是英文、其余宿主仍是中文；`scripts/test-default-config-parity.py` 同时比对模板、来源与 Rust 默认。
- **候选窗几何与来源一致。** 最小宽度 160 DIP、单行高 `fontSize × 1.35 + 2`、行间与横排行间 2 DIP、选中条以行左缘为中心（`x - 1.5`，宽 3）、竖排翻转判定以 232 DIP × 缩放为起点；`windows-candidate-card-size` 覆盖，旧头文件下编译不过。
- **门禁漏洞。** 覆盖率改为完整字段路径并要求页面实际用到（原来 `menu`、`scale`、`font` 这类裸词随处可命中）；`feature-log` 除 CHANGELOG 外还逐条核对锁定对象之前的 `feat:` 提交（原来漏了七十多条，查出 4 个已知差距：普通 Backspace 删空拼音后退选分词、纠错候选的 `*` 标记、ü 别名候选的轻标记、双拼整句候选的排序）；源码清单的同名匹配只认 Windows 与共享代码、非测试目录，被同名遮住或指向不存在来源的表项报为过期；界面动作改用真实实现符号并整词匹配、排除测试与其他平台；`preferences-field-parity` 从 `Preferences` 出发比对所有可达结构体与页面上的类型（含内联对象），13 个扩到 20 个；`installer-prerequisites` 按函数切片检查 HKLM32/64 两个视图，不再被卸载代码里的同名字符串满足；`windows-native-run` 补上 TIP 子目录的 include，并把"仓库里有的头文件找不到"判为失败而不是跳过（多跑了 2 个测试）；`settings-label-parity` 在来源检出存在但解析不出标签时失败而不是跳过。
- **验证层级。** 候选几何与安装器契约测试在本机 clang 编译运行；`msime-client-core` 本机 cargo test；全部 reference 门禁以固定来源本机运行通过，且每个漏洞都在副本里注入过、旧脚本通过而新脚本失败；安装器由发布流水线在 Windows runner 上编译并跑 `install-smoke.ps1`。未在真实 Windows 编辑器里目测候选窗。

# Windows WinUI 3 设置

`MSIME.Settings.vcxproj` 是 Windows 设置窗口的原生宿主（安装后为 `msime-client-settings.exe`）。它使用 Windows App SDK 的 WinUI 3 控件，按 Windows 11 设置的样式绘制：标题栏搜索框、280 宽的侧栏（带页面筛选）、每行一张 Fluent 卡片。它不启动 Tauri 作为设置界面，也不拥有 TSF 或输入会话。

## 页面

侧栏的 18 个页面分成 6 组，依次是打字（输入、标点与翻译、快捷键、词库）、外观（主题、候选窗口、悬浮工具栏）、更多输入方式（屏幕键盘、语音输入、手写输入）、工具（剪贴板、打字统计、插件、AI 辅助）、账号（账号与同步）、支持（维护与诊断、帮助与反馈、关于），每组以组名开头。分组与顺序和共享设置界面的 `settingsNavGroups`（`packages/ui/src/settings/settings-page-registry.ts`）一致，定义在 `SettingsNavigation.h`，由 `tests/ui/settings_navigation.cpp` 核对，并对照共享路由词表检查；组名和页名在 `main.cpp` 中。

- 输入、标点与翻译、快捷键、词库、主题、候选窗口、悬浮工具栏、屏幕键盘、语音输入、手写输入、维护与诊断、帮助与反馈、关于：在本窗口中原生绘制。
- 剪贴板、打字统计、插件、AI 辅助、账号与同步：跨平台服务页，点击「打开」后用 `MSIME.exe --route=…` 在共享 Tauri 应用中显示对应页面。剪贴板打开共享应用的「剪贴板」页（`settings:tools`），本机剪贴板历史开关和云剪贴板入口都在那里。
- 桌面端没有社区页：社区皮肤在共享应用「主题」页的「社区皮肤」标签里，本窗口「主题」页的「社区皮肤」一行打开它，`community` 路由打开本窗口的「主题」页。AI 对话是 AI 辅助的子页，`chat` 路由打开「AI 辅助」。
- 其他平台下载不单独成页：「关于」页的「版本与更新」组可以打开产品下载页或复制链接。

原生页面不绘制的部分（候选字体选择、皮肤与自定义主题编辑、词库管理、背单词、辅助码插件、语音识别服务、屏幕键盘与手写的详细设置、帮助、反馈表单）以按钮的形式跳到共享应用的对应路由，路由 id 与托盘和其他启动方使用的相同。

「输入」页的辅助码方案和共享设置的辅助码下拉框（`packages/ui/src/settings/pages/helpcode-page.tsx`）规则相同：该方案选用了辅助码插件（`plugins.helpcode_pack_quanpin` / `helpcode_pack_shuangpin`）时，输入引擎用插件替换内置方案，这一行把插件列为当前项并说明已被替换，选内置方案会同时清掉插件。本窗口不扫描插件目录，只显示插件 id，选用插件仍在共享应用里。

`--route=settings:<id>` 指定打开的页面，可以是共享路由的设置分类 id（`appearance`、`dictionary`、`about` 等，映射见 `route_aliases`），也可以是本窗口的页面 id；其他值打开默认的「输入」页。

## 外观与主题预览

窗口本身按设计稿的 Fluent token 绘制，强调色固定为浅色 `#005FB8`、深色 `#60CDFF`（与候选窗口原生调色板相同），不跟随 Windows 的个性化强调色。「设置界面主题」只覆盖本窗口的明暗。

「主题」「候选窗口」「输入」「标点与翻译」页的候选窗口预览和「自定义」主题卡片不自己配色：它们用与 Server 相同的请求（全局主题、去掉键盘设计的 `custom_theme`、候选窗口自己的明暗与排列方式、数据目录下的 `skins`）调用 `msime_client_resolve_theme`，再用 `src/candidate/CandidatePalette.h` 合成调色板，并按 `CandidateWindow.cpp` 的规则绘制选中行、序号、选中条和翻译，所以自定义主题的 `candidate_colors` 与皮肤包会反映在预览里。其余主题卡片使用 `msime_client_theme_catalog` 给出的预览色。

## 默认输入法提示

每页标题上方的橙色提示条对应设计稿的 deskStrip：水杉输入法不在当前用户的键盘列表中时显示「去添加」，已添加但不是默认输入法时显示「设为默认」。状态通过 `input.dll` 的 `EnumEnabledLayoutOrTip` 读取（无头文件和导入库，运行时加载），按钮分别调用 `InstallLayoutOrTip` 和 `SetDefaultLayoutOrTip`；添加失败时打开「语言和区域」。`input.dll` 无法回答时不显示提示条。窗口重新获得焦点时重新检查。

## 保存

设置窗口通过 `msime_client_load_preferences` 和 `msime_client_save_preferences` 读取、校验并以 compare-and-swap 方式保存共享偏好。每个控件改动后立即保存，滑块停止拖动后保存；保存被拒绝（其他窗口已更新或与其他设置冲突）时重新读取并在页面顶部说明。窗口重新获得焦点时重新读取，以反映托盘、共享应用或同步带来的变化。数据目录必须与 Server 的状态根相同：Server 启动它时注入 `MSIME_CLIENT_STATE_DIR`，直接用这个目录；从开始菜单直接启动时没有这个变量，就按 Server 的方式自己解析，先用 `common/StateDirectory.h` 的 `resolve_state_directory()`（本版本的数据目录环境变量、安装器在 HKLM 记下的 `DataDir`、`%LOCALAPPDATA%\<本版本的状态目录>`），那里的 `runtime-options.json` 写了绝对路径的 `preferences_directory` 时再改用它，与 `server_main.cpp` 的 `production_preview_document` 和 Tauri 外壳的 `windows_server_preferences_directory` 一致。以前开始菜单启动直接回落到 `%LOCALAPPDATA%\MSIME-Client`，而默认安装的 Server 用的是安装器的 `DataDir`（`%LOCALAPPDATA%\metasequoiaime-full`），这个窗口里改的开关（包括 `/`、`@` 和 K 模式、辅助码）就都写进了 Server 不读的文件。顺序在 `SettingsStateDirectory.h`，由 `tests/ui/settings_state_directory.cpp` 核对。

「候选窗口」页「游戏」组的两张程序列表（`game_compatibility.overlay_processes`、`excluded_processes`）在写入前先按偏好库的规则规范化和校验（`GameProcessList.h`）：去掉首尾空白、ASCII 字母转小写，要求以 `.exe` 结尾、不超过 64 个字符、不带路径和 `\ / : * ? " < > |` 及控制字符，两张表之间不重复、合计不超过 32 条。不合法时在列表下方说明具体原因，不写入文档：保存被拒时窗口只能给出上面那句笼统的说明，用户看不出是哪一项出了问题。这组设置只由 TSF DLL 在每次激活时读取，对已打开的游戏要切换一次输入法才生效，取舍见 [决策笔记](../../../.agents/notes/implemented/feature/2026-10-08-windows-game-candidate-overlay.md)。

## 设置文件、数据目录与许可声明

「维护与诊断」页的「设置文件」组导出和导入设置。「导出…」用系统的保存对话框选位置，`msime_client_export_settings` 读出已保存的偏好并换算成设置文件（`app.msime.client.preferences`，规则在 `crates/client-core/src/settings_document.rs`），再先写临时文件后改名写到所选位置。「导入…」用打开对话框选文件，读入（上限 1 MiB）后交给 `msime_client_import_settings`：它保留本机的语音、AI 辅助和翻译服务配置与密钥、诊断日志、使用统计和剪贴板历史开关（所以也不会清空已存的剪贴板历史），文件里的方案本版本不提供时保留本机的方案，然后按本窗口读到的修订号比较并交换写回。导入成功或被拒都会重新读取设置；错误码换成说明的规则在 `SettingsDocumentFile.h`，由 `tests/ui/settings_document_file.cpp` 核对。

同一页的「数据目录」行显示状态根并可在资源管理器中打开。移到其他磁盘要重新运行完整安装包：数据目录登记在 HKLM 的 `DataDir`，改它要管理员权限，安装器的「选择数据位置」一步已经负责复制、切换和删除旧目录，本窗口只说明这条路径。

「关于」页的产品名按版本取（`MSIME_EDITION_DISPLAY_NAME`），「许可与隐私」组的「第三方组件许可」用关联程序打开安装目录下的 `THIRD_PARTY_NOTICES.txt`（本窗口所在 `server` 目录的上一级，与安装器的 `DestDir` 一致）。取舍见 [决策笔记](../../../.agents/notes/implemented/feature/2026-10-10-windows-settings-file-and-local-data-parity.md)。

## 检查更新

「关于」页的「检查更新」在本窗口中完成，不再打开共享应用：工作线程调用 `msime_client_update_check`（`crates/client-core/src/update_check.rs`，与共享设置页、鸿蒙走同一份 Rust 逻辑），请求带 `platform: windows`、本版本 id 和当前版本，结果写在该行下方：「已是最新版本」「暂无可用发行版」「检查失败，请稍后重试」，或「发现新版本 vX」加未签名提示与安装包的 SHA256，按钮随之变成「前往下载」，打开该版本在 GitHub 上的发布页。只接受本仓库的 tag 页地址。当前版本是编译期宏 `MSIME_WINDOWS_VERSION`：`Build-Client.ps1` 用 `/p:MsimeVersion=<TargetVersion>` 传入，开发构建取 `platforms/windows/version.txt`，与 Server 上报的版本相同。

## 连接 AI 助手

「维护与诊断」页的「连接 AI 助手」即共享设置页的同名区块：通过 `msime_client_mcp_status` 和 `msime_client_mcp_install` 显示与 `msime-client-settings.exe` 同目录的 `msime-mcp.exe`、可复制的 MCP 配置，并把它写入 Claude Desktop 或 Cursor 的配置文件。服务器指向的运行时选项取自 Server 注入的 `MSIME_CLIENT_HOST_OPTIONS`，直接启动时取 Server 状态目录（`resolve_state_directory()`，不是 `preferences_directory`）下的 `runtime-options.json`，也就是 Server 启动时读的那一份；两者都没有时说明输入法尚未初始化，不提供配置。写入逻辑与 Tauri 外壳共用 host-api 的 `mcp_clients`。

## 与共享应用的分工

Windows 的表情、手写和屏幕键盘面板以及上面列出的跨平台页面由同目录的共享 Tauri 应用 `MSIME.exe` 提供。本窗口通过 `src/system/ShellLauncher.cpp`（与托盘相同的启动器）启动它，数据目录和运行时选项都已知时一并传入，保证两边读写同一份设置。Server 将设置路由发送给 `msime-client-settings.exe`，将面板路由发送给 `MSIME.exe`。

## 构建

在 Visual Studio 开发者命令行中构建：

```powershell
msbuild MSIME.Settings.vcxproj /p:Configuration=RelWithDebInfo /p:Platform=x64 `
  /p:HostApiLibrary=C:\path\to\msime_host_api.dll.lib
```

Windows App SDK 版本由 `WindowsAppSDKVersion` 属性控制，默认值与项目文件固定，发布构建使用 self-contained unpackaged 模式。项目没有 XAML 编译器：应用自己实现 `IXamlMetadataProvider` 并转交给 WinUI 控件库的元数据提供者，界面全部在 `main.cpp` 中用代码构建。

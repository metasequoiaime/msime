# Windows WinUI 3 设置

`MSIME.Settings.vcxproj` 是 Windows 设置窗口的原生宿主（安装后为 `msime-client-settings.exe`）。它使用 Windows App SDK 的 WinUI 3 控件，按 Windows 11 设置的样式绘制：标题栏搜索框、280 宽的侧栏（带页面筛选）、每行一张 Fluent 卡片。它不启动 Tauri 作为设置界面，也不拥有 TSF 或输入会话。

## 页面

侧栏的 18 个页面分成 6 组，依次是打字（输入、标点与翻译、快捷键、词库）、外观（主题、候选窗口、悬浮工具栏）、更多输入方式（屏幕键盘、语音输入、手写输入）、工具（剪贴板、打字统计、插件）、账户与社区（账户与同步、社区）、支持（维护与诊断、帮助与反馈、关于），每组以组名开头。分组与顺序和共享设置界面的 `settingsNavGroups`（`packages/ui/src/settings/settings-page-registry.ts`）一致，定义在 `SettingsNavigation.h`，由 `tests/ui/settings_navigation.cpp` 核对，并对照共享路由词表检查；组名和页名在 `main.cpp` 中。

- 输入、标点与翻译、快捷键、词库、主题、候选窗口、悬浮工具栏、屏幕键盘、语音输入、手写输入、维护与诊断、帮助与反馈、关于：在本窗口中原生绘制。
- 剪贴板、打字统计、插件、账户与同步、社区：跨平台服务页，点击「打开」后用 `MSIME.exe --route=…` 在共享 Tauri 应用中显示对应页面。
- 其他平台下载不再单独成页：「关于」页的「版本与更新」组可以打开产品下载页或复制链接，旧的 `download` 路由也打开「关于」页。

原生页面不绘制的部分（候选字体选择、皮肤与自定义主题编辑、词库管理、背单词、AI 服务商配置与对话、语音识别服务、屏幕键盘与手写的详细设置、帮助、反馈表单、检查更新）以按钮的形式跳到共享应用的对应路由，路由 id 与托盘和其他启动方使用的相同。

`--route=settings:<id>` 指定打开的页面，可以是共享路由的设置分类 id（`appearance`、`dictionary`、`about` 等，映射见 `route_aliases`），也可以是本窗口的页面 id；其他值打开默认的「输入」页。

## 外观与主题预览

窗口本身按设计稿的 Fluent token 绘制，强调色固定为浅色 `#005FB8`、深色 `#60CDFF`（与候选窗口原生调色板相同），不跟随 Windows 的个性化强调色。「设置界面主题」只覆盖本窗口的明暗。

「主题」「候选窗口」「输入」「标点与翻译」页的候选窗口预览和「自定义」主题卡片不自己配色：它们用与 Server 相同的请求（全局主题、去掉键盘设计的 `custom_theme`、候选窗口自己的明暗与排列方式、数据目录下的 `skins`）调用 `msime_client_resolve_theme`，再用 `src/candidate/CandidatePalette.h` 合成调色板，并按 `CandidateWindow.cpp` 的规则绘制选中行、序号、选中条和翻译，所以自定义主题的 `candidate_colors` 与皮肤包会反映在预览里。其余主题卡片使用 `msime_client_theme_catalog` 给出的预览色。

## 默认输入法提示

每页标题上方的橙色提示条对应设计稿的 deskStrip：水杉输入法不在当前用户的键盘列表中时显示「去添加」，已添加但不是默认输入法时显示「设为默认」。状态通过 `input.dll` 的 `EnumEnabledLayoutOrTip` 读取（无头文件和导入库，运行时加载），按钮分别调用 `InstallLayoutOrTip` 和 `SetDefaultLayoutOrTip`；添加失败时打开「语言和区域」。`input.dll` 无法回答时不显示提示条。窗口重新获得焦点时重新检查。

## 保存

设置窗口通过 `msime_client_load_preferences` 和 `msime_client_save_preferences` 读取、校验并以 compare-and-swap 方式保存共享偏好。每个控件改动后立即保存，滑块停止拖动后保存；保存被拒绝（其他窗口已更新或与其他设置冲突）时重新读取并在页面顶部说明。窗口重新获得焦点时重新读取，以反映托盘、共享应用或同步带来的变化。Server 启动它时注入 `MSIME_CLIENT_STATE_DIR`，所以设置和输入法宿主使用同一个数据目录；从开始菜单直接启动时回落到 `%LOCALAPPDATA%\MSIME-Client`。

## 连接 AI 助手

「维护与诊断」页的「连接 AI 助手」即共享设置页的同名区块：通过 `msime_client_mcp_status` 和 `msime_client_mcp_install` 显示与 `msime-client-settings.exe` 同目录的 `msime-mcp.exe`、可复制的 MCP 配置，并把它写入 Claude Desktop 或 Cursor 的配置文件。服务器指向的运行时选项取自 Server 注入的 `MSIME_CLIENT_HOST_OPTIONS`，直接启动时取数据目录下的 `runtime-options.json`；两者都没有时说明输入法尚未初始化，不提供配置。写入逻辑与 Tauri 外壳共用 host-api 的 `mcp_clients`。

## 与共享应用的分工

Windows 的表情、手写和屏幕键盘面板以及上面列出的跨平台页面由同目录的共享 Tauri 应用 `MSIME.exe` 提供。本窗口通过 `src/system/ShellLauncher.cpp`（与托盘相同的启动器）启动它，数据目录和运行时选项都已知时一并传入，保证两边读写同一份设置。Server 将设置路由发送给 `msime-client-settings.exe`，将面板路由发送给 `MSIME.exe`。

## 构建

在 Visual Studio 开发者命令行中构建：

```powershell
msbuild MSIME.Settings.vcxproj /p:Configuration=RelWithDebInfo /p:Platform=x64 `
  /p:HostApiLibrary=C:\path\to\msime_host_api.dll.lib
```

Windows App SDK 版本由 `WindowsAppSDKVersion` 属性控制，默认值与项目文件固定，发布构建使用 self-contained unpackaged 模式。项目没有 XAML 编译器：应用自己实现 `IXamlMetadataProvider` 并转交给 WinUI 控件库的元数据提供者，界面全部在 `main.cpp` 中用代码构建。

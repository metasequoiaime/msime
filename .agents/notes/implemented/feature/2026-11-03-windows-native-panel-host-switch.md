# Agent Note: 现代 Windows 包带上原生面板宿主，用环境变量切换面板承载

Status: implemented

## Problem

[全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)定下 Win10/11 的面板改用 `panels/win7` 那套原生宿主，与 Win7 共用一份实现。在那之前，现代包里表情、手写、屏幕键盘、语音、云剪贴板、云词库和背单词 7 个面板路由只能拉起 Tauri/WebView2 的 `MSIME.exe`：原生宿主只在 `MSIME_WINDOWS7_CLIENT` 的 Win7 产品构建里编成 `msime-client-panels-win7.exe`，现代构建既不编它也不打包它，`ShellSurfaces.h` 的 `shell_executable_names` 在现代分支对面板只返回 `MSIME.exe`。要逐个面板换成原生、在真机上对比两种宿主，先得让现代包里同时有这两个程序，并且能在不重编的情况下切换，而缺省行为一点不变。

## Decision

现代构建把同一份 `panels/win7` 源码编成中性名字的 `msime-client-panels.exe`，随 Server 输出打包；面板请求交给哪个程序由环境变量 `MSIME_WINDOWS_PANEL_HOST` 决定，缺省仍是 `MSIME.exe`。

### 构建与打包

- `panels/win7/CMakeLists.txt` 新增选项 `MSIME_PANELS_MODERN_WINDOWS`（缺省 OFF）。OFF 时与原来逐字相同：`msime-client-panels-win7`、读 `version-win7.txt`、链接 HarfBuzz 并定义 `MSIME_WINDOWS7_EMOJI_SHAPING`。ON 时目标名是 `msime-client-panels`，版本取 `MSIME_WINDOWS_VERSION` 或 `version.txt`，不找也不链接 HarfBuzz，给面板库公开定义 `MSIME_PANELS_MODERN_WINDOWS=1`。Win10/11 的 DirectWrite 有 `IDWriteFactory2` 字体回退，`common/Windows7EmojiFont.h` 在那里直接画系统字体，不用随包的 Noto 字体也不用 HarfBuzz 整形，所以现代包不需要新增依赖，`platforms/windows/vcpkg.json` 不动。
- `Build-Client.ps1` 在 x64 的 WinUI 设置之后配置并编译这个变体，`CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO` 指向同一个 `bin`，`MSIME_HOST_LIBRARY` 用设置窗口链接的那份导入库：full 是 Rust 输出的 `msime_host_api.dll.lib`，其余版本是脚本用 `lib /DEF` 按版本表 `host_dll` 生成的同名导入库，否则面板宿主会去找 full 的 DLL。`-DBUILD_TESTING=OFF`，不往产品目录里编测试程序。PE 架构检查的 x64 EXE 列表加上它。
- 产品目录整体复制进 `server_exe`，所以安装器（`server_exe\*` 递归安装、按目录停进程）、符号包（`server_exe/*.pdb`）和 SignPath 暂存（有同名 PDB 的 EXE）都自动带上它。另外两处改成显式要求：`Prepare-PackageFiles.ps1` 在现代包里缺 `msime-client-panels.exe` 就在替换旧暂存之前失败；`SignPath-PackageBinaries.ps1` 把它列进必须提交签名的入口。Win7 包不变，仍只带 `-win7` 程序。

### 切换开关

- `ShellSurfaces.h` 新增 `PanelHost { Shared, Native }`、变量名 `panel_host_variable` 和纯函数 `panel_host_from_setting`：值等于 `native`（ASCII 不分大小写）才是 `Native`，其余一律 `Shared`。`shell_executable_names` 和 `shell_executable` 多一个必传的 `PanelHost` 参数，现代分支在 `Native` 时对面板返回 `msime-client-panels.exe`，设置请求不受影响；Win7 分支忽略它。没有缺省实参，每个调用方都必须表态。
- 选了 `Native` 就只找 `msime-client-panels.exe`，不退回 `MSIME.exe`：开关的意义是确定用哪个宿主，悄悄退回会让测试机上「看起来原生面板在工作」其实是 WebView。程序不在时托盘和工具栏的手写、云剪贴板、云词库、背单词照旧显示为不可用，表情和屏幕键盘照旧退回系统自带的面板。
- `ShellLauncher.cpp` 的 `configured_panel_host()` 读本进程环境。Server 启动时读一次，托盘与悬浮工具栏的可用性（`preview_shell`）和每次启动都用这一个值；WinUI 设置的 `open_shell` 每次点击读自己的环境，文件查找挪到后台线程，失败提示带上实际找的文件名。逻辑都在头文件里，`settings/main.cpp` 只改接线的几行。
- 原生宿主内部跳转（键盘面板的「设置」「语音」、语音面板的「设置」）经 `settings/win7/SurfaceLauncher.h` 的 `surface_executable_name`，按 `MSIME_PANELS_MODERN_WINDOWS` 选 `msime-client-settings.exe`/`msime-client-panels.exe` 或两个 `-win7` 程序。它发出的 `settings:screen-keyboard`、`settings:voice` 都在 WinUI 设置的 `route_aliases` 里。
- 原生宿主认的路由从 `main.cpp` 的一长串比较抽到 `panels/win7/PanelRoutes.h`，测试据此核对托盘、工具栏和设置窗口发出的每个面板路由都被它接受，开关打开后不会有哪一行拉起一个立即以退出码 2 结束的进程。

### 测试

`windows-shell-surfaces`、`windows7-shell-surfaces` 覆盖开关解析、6 个托盘面板路由与 4 个设置类请求在两种状态下的程序名、原生宿主是否认这个路由、开关打开时不退回共享应用；Win7 配置两种状态都是 `-win7` 程序。`windows-settings-navigation` 对设置窗口的 6 个面板入口（含语音）做同样的核对。键盘面板测试在两个变体里分别断言 `surface_executable_name`。`build_client.ps1`、`package-files.ps1`、`signpath-payload.ps1`、`lifecycle.ps1` 和 `install-smoke.ps1` 跟上新的构建步骤、暂存要求、签名清单、安装文件和 `shell_executable_names` 的分支形状。CI 的 `windows-settings` 作业在原有 Win7 变体之后，用同一套依赖再编一次现代变体、核对程序名并跑面板 CTest。

## Alternatives considered

- **`runtime-options.json` 加字段。** 它是 Server 准备好、设置窗口和面板都会读的宿主文档，加一个布尔字段看起来最「正规」，也能被 `MSIME_CLIENT_HOST_OPTIONS` 带到子进程。不选：这份文件由准备流程生成、按硬链接发布，用户改它要找到 LocalAppData 下的状态目录、手写 JSON，还要防止下次准备或升级覆盖；它也是共享契约，加字段要过 Rust 侧的 schema。一个只在迁移期给维护者测试机用的开关不值得进持久化格式。
- **共享偏好（`preferences.json`）加一项、设置页给开关。** 能在界面里切换、即时生效。不选：这是跨平台的偏好 schema，按仓库规则新增偏好要先写进共享 Rust 并给各平台界面，而这个开关只服务 Windows 的迁移期，迁完就删；做成用户可见设置还会让普通用户切到尚未补齐功能的原生面板。
- **注册表值（HKCU）。** 改完不用注销，Server 每次读都能拿到新值。不选：仓库里 Windows 宿主的同类开发开关（`MSIME_CLIENT_SETTINGS_COMMAND`、`MSIME_HANDWRITING_MODEL`、状态目录覆盖）都是环境变量，环境变量还能原样被 Server 拉起的设置窗口继承；注册表要多一份读取代码和清理说明。注销重登的代价对测试机可以接受。
- **按目录里有没有 `msime-client-panels.exe` 自动选。** 零配置。不选：现代包从这一版起总带着它，自动选就等于缺省切换，不再是行为中立的一步；`ShellSurfaces.h` 原有的注释也明确产品类型不能按目录残留文件猜。
- **开关打开但原生宿主缺失时退回 `MSIME.exe`。** 包不全时用户仍能打开面板。不选，理由见上面「切换开关」：测试机上要的是确定性，缺失应当显示为不可用而不是换一个宿主。

## Consequences

- 收益：现代安装包同时带两个面板宿主，维护者在测试机上 `setx MSIME_WINDOWS_PANEL_HOST native` 并注销重登即可整体切到原生面板，删掉变量即恢复，不用重编也不用换包；之后每个面板的原生化 PR 只改面板本身，不再碰构建、打包和签名。
- 收益：同一份源码在 CI 里按两个变体各编一遍、各跑一遍面板 CTest，Win7 改动弄坏现代变体（或反过来）会在 PR 上暴露。
- 代价：现代包多一个 EXE（PDB 进符号包、EXE 进签名请求），构建多一次 CMake 配置和编译；开关缺省关闭期间它只是随包携带。
- 代价：开关在 Server 启动时读一次，改完必须让 Server 重启并拿到新环境，README 写的是注销重登；只重启 Server 进程不一定够，看门狗或按需拉起它的应用可能还带着旧环境。
- 原生宿主目前与 Win7 完全相同：进程声明系统 DPI 感知（`compat::process_context(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE)`），多显示器不同缩放下会被系统拉伸；外观是 Win32 原生控件，没有深色模式；每次点击起一个新进程，只有云词库用互斥量限制单实例且第二次打开直接退出而不是唤起已有窗口；`ShellLauncher.cpp` 启动后会对子进程的可见顶层窗口调 `SetForegroundWindow`，与手写、键盘等 `WS_EX_NOACTIVATE` 面板「不抢焦点」的意图相冲突（Win7 包今天就是这样）；表情在 Win10/11 走 Direct2D `DrawText` 而没有 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`，是单色字形。这些和与 React 面板的功能差距（18 键分组、长按连发、手写重做与只复制、语音识别语言与 Esc、背单词 SAPI 朗读等）都留给后续逐个面板的 PR，在开关改为缺省打开之前补齐。
- 现代变体没有在 Windows 上实际编译和运行过之前，只有 MinGW 的语法检查和 macOS 上的纯头文件测试作证；以 PR 上 `windows-settings` 作业的结果为准。

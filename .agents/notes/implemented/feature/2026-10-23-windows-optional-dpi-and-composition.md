# Agent Note: DPI 与 DirectComposition 按系统导出能力回退

Status: implemented

## Problem

Server、候选窗和 `msimeui` 直接导入 `GetDpiForWindow`、`SetThreadDpiAwarenessContext` 等新 API，以及 Windows 8 起才有的 `dcomp.dll`。旧系统会在执行已有的 Direct2D 与分层窗口回退前装载失败。单纯改成动态导入仍然不够：六个窗口把线程 DPI API 缺失当作窗口故障，会在创建或绘制时抛错。

## Decision

`platforms/windows/compat/WindowsDpi.h` 集中解析六个 DPI 入口。现代系统有入口时直接转发参数和返回值，包括失败与零结果；不因新 API 调用失败而再次修改进程状态。旧系统窗口 DPI 由真实 HWND 的 GDI DC 查询，系统 DPI 由屏幕 DC 查询，无效窗口返回 0。线程上下文缺失时不修改进程 DPI；独立 Server 在创建窗口前一次性启用系统 DPI 感知，`msimeui::Application` 的进程初始化也使用对应回退。

窗口共用 `ThreadDpiScope`：支持线程 API 的系统保持原有切换、异常和恢复语义；缺少该入口时沿用进程 DPI。语音与无障碍原有的非抛错作用域继续保留其行为，调用同一探测入口。

受控 TSF 编辑器使用同一 DPI 适配，以便在旧系统验证真实 TSF 输入。Server 冒烟测试自己的线程上下文查询与比较也按导出探测：新系统与 Wine 保留既有严格断言，旧系统核对实际进程 DPI。不能让验收程序比被测产品先因缺少新 API 无法装载。编辑器原有的 Windows min/max 宏冲突以带括号的标准库调用解决；COM 接口的未使用参数显式标记，鼠标回调 cookie 显式初始化，不降低 `/W4 /WX`。

`DeviceResources` 动态装载系统目录内的 `dcomp.dll` 并解析 `DCompositionCreateDevice`，取消 `msimeui` 的静态链接依赖。合成器可用时仍调用原入口；不可用时由既有 `EnsureForComposition` 进入分层窗口绘制。系统材质的既有失败回退保持。

实际 Win7 依赖装载检查发现 TSF 光标定位仍静态引用 `LogicalToPhysicalPointForPerMonitorDPI`。同一适配表补充点坐标转换：新入口存在时原样透传，旧系统调用真实 `LogicalToPhysicalPoint`，保留失败而不重复转换。导入拒绝表覆盖这两个每屏点转换入口，回归比较真实旧 API 的返回值和坐标，并核对新版失败不会调用旧函数。

### 边界与历史审计

这是 [完整 Win7 支持](../../implemented/feature/2026-10-23-windows7-support.md) 的装载与 DPI 基础切片，不能据此宣称 Win7 产品已支持。设置应用、Rust 标准库与第三方依赖、共享面板和安装包仍需完成适配。没有修改产品默认值、TSF/Server 协议、设置入口或其他平台实现。

[候选渲染边界](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md) 部分重叠：候选仍归 Server，TSF DLL 不画界面。按 DPI、DirectComposition、Win7 关键词检查活跃架构与提案笔记，没有需要替换的既有兼容决定；完整支持提案保留未完成验收清单。

## Testing

旧 MinGW 在 `_WIN32_WINNT=0x0601` 下不声明 `WS_EX_NOREDIRECTIONBITMAP`，版本适配使用 SDK 固定样式值并保留原来的运行时系统版本判断；两个目标版本的跨编译与静态导入检查通过。

`windows-dpi-compat` 注册为 Windows CTest。测试使用真实 HWND 与 GDI DC 验证回退 DPI，注入缺失、新 API 成功及失败的导出表，验证线程作用域不改变进程状态、新 API 失败不触发旧进程 API，以及上下文退出时恢复。真实系统分支还解析本机导出并执行线程上下文往返。

MSVC 的 x64 与 x86 DPI 测试在 Windows 11 通过，`dumpbin /imports` 确认两种产物均没有六个新 DPI API 的静态导入。MinGW 对适配头、测试和修改到的 `msimeui` 绘制源进行严格语法检查通过。`scripts/test-windows-dpi-compat.py` 由契约运行器自动发现，实际编译测试 PE 和 `DeviceResources` 对象，并用 `objdump`、`nm` 检查静态导入；缺少交叉工具链时明确跳过，Windows CTest 继续承担运行覆盖。

Windows 11 上 `msimeui` 与 `msime-windows-server` 静态库构建通过。UI 套件运行 46 项，其中 5 项失败；在同一台 SSH 构建机、同一个 CMake 目录用固定基线 `fe0d55ad8b` 的源码重建对照，再恢复本切片重建，失败名称完全一致：`hwnd_target_refreshes_dpi_when_reused` 和四项 `text_selection_*` 方向键用例，未增加失败。该比较只证明这台机器上的已有测试无新增失败，不替代登录桌面的窗口验收或 Win7 验收。

完整 Server 已用同源的 Win7 Rust 核心链接，`--help` 运行通过，实际 EXE 没有六个新 DPI API 或 DirectComposition 静态导入。`windows-server` 在窗口显示断言失败；同一构建目录切回固定基线、再恢复本切片分别重建运行，两者均在 `require(IsWindowVisible(window.handle()) && !window.failed())` 失败，没有增加失败。

MSVC x64 的 `windows-server-smoke` 与受控 `msime-tsf-edit-control-demo` 完整构建通过，最终 PE 导入表没有六个新 DPI API、线程上下文查询/比较或 DirectComposition 入口。更新测试适配后再次运行 Server 冒烟测试，仍只停在同一窗口显示断言。这不是编辑器输入验收通过的证据，真实登录桌面和 Win7 上的输入仍待验证。

quick 的首轮编译通过，但两个契约检查超时；第二轮仅 Linux 的 `linux-floating-panels-x11` 在“切换方案按钮没有弹出菜单”处失败，同一二进制的定向复跑通过。第三轮 `20261023-195145` 完整 quick 于 15 分 45 秒内通过，没有修改检查或扩大失败基线。Studio 缺少 Windows 交叉工具链、Node 和 HarmonyOS 完整环境，相应阶段跳过；Windows 原生验证和笔记门禁另行执行。真实 Win7 验收仍需完成。

## Alternatives considered

- **只降低 `_WIN32_WINNT`。** 改动最少，也能让编译器隐藏较新的声明。但现有源代码仍调用这些入口，现有 DLL 的静态导入也不会因此变成动态回退，无法解决装载失败。
- **只在 Win7 构建里替换调用。** 现代包的调用无需调整，但两个构建会各有一套接口语义，测试难以覆盖两者之间的漂移。使用同一能力探测入口可以在现代 Windows 上验证缺失导出的行为，同时保留原生 API 转发。
- **缺少线程 DPI 时在每次窗口操作里调用 `SetProcessDPIAware`。** 可以复用作用域入口，但进程 DPI 不是可恢复的线程局部状态，会影响其他窗口；因此只在独立宿主初始化时设置。

## Consequences

旧系统能够到达已有的绘制回退，现代系统保持原生 DPI 与 DirectComposition 路径。代价是多一层缓存的函数指针转发，并需要同时验证缺失 API 和原生 API 分支。Win7 的系统 DPI 与现代每显示器 DPI 能力不同，旧系统上的缩放、屏幕坐标和跨进程光标定位必须继续实测，不能用 Windows 11 上的注入测试代替。

完整产品的两架构 Release 构建与 DPI CTest 在 Windows 11 通过。Win7 依赖检查已越过 TSF 的每屏点转换导入，新增 UI Automation 生命周期测试也通过；豆包 WinHTTP WebSocket 的静态导入也由专用传输替换后，完整产品两架构的 Win7 模块导出和 DLL 装载检查均通过。MSVC 增量链接可能保留已删除的导入，遇到对象已无符号但 DLL 仍有旧导入时，只删除对应 DLL 与 `.ilk` 重链；正式产品使用 Release 链接。

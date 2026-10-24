# Agent Note: Win7 专用 Rust 核心构建与延迟系统导入

Status: implemented

## Problem

Rust 的普通 Windows MSVC 目标没有 Win7 运行保证。改用 Win7 目标并自行构建标准库后，实际核心 DLL 仍静态导入 `combase.dll`、WinRT 错误 API 和 `ActivateAudioInterfaceAsync`：第三方库的这些导入会在业务代码选择回退前阻止 DLL 装载。新版 cpal 的默认麦克风与扬声器也使用异步激活，不能靠给缺失接口返回“不支持”保留语音和音效。

## Decision

`Build-Windows7Core.ps1` 在仓库固定的 Rust 1.97.1 上用 `build-std=std,panic_unwind` 构建 `x86_64-win7-windows-msvc` / `i686-win7-windows-msvc`。前置条件是安装该工具链的 `rust-src`，不修改普通构建的目标和发布入口。启用 `build-std` 的 `RUSTC_BOOTSTRAP`、产物目录与符号配置只在脚本期间设置，退出时恢复环境。

Win7 核心与 MCP 的标准库、Rust 和原生依赖统一以 `target-feature=+crt-static` 构建；C++ 延迟加载对象也使用静态 CRT。编码参数保留调用方设置，脚本退出时恢复。原因是 TSF 会作为插件进入目录各异的编辑器，[微软 UCRT 部署约束](https://learn.microsoft.com/en-us/cpp/windows/universal-crt-deployment)明确指出 Windows 8 之前无法从插件目录解析应用本地 UCRT 转发。只在 Server 旁带 CRT 能启动 EXE，却不能保证编辑器冷装载核心；不能依赖宿主已安装运行库或预加载 DLL。

`compat/Windows7DelayLoad.cpp` 使用 MSVC 自带的延迟加载机制，只链接进 Win7 核心和 MCP。三个已确认的系统模块从系统目录探测，可用函数返回同一个系统地址；Win7 缺少的 `combase.dll` 以系统 `ole32.dll` 作为模块，调用它本来就提供的旧 COM 函数。WinRT 错误描述通过传统 `IErrorInfo` 进入线程错误槽，HRESULT 仍由 Rust 错误对象保存。未知 DLL 和未知函数继续失败，不提供通用空实现。

cpal 默认录音与播放所用的虚拟 GUID 在旧系统上由 `IMMDeviceEnumerator::GetDefaultAudioEndpoint` 和 `IMMDevice::Activate` 完成激活。WRL 实现原异步操作接口，在工作线程进入 MTA 后激活设备，通过原完成回调交付结果；操作对象保留结果和 COM 引用，调用方可以重复读取。系统原有异步接口可用时仍走原接口。产品没有使用的新系统进程回环参数不作虚假模拟。

`Test-Windows7CoreImports.ps1` 使用 MSVC `dumpbin` 同时读取普通与延迟导入，拒绝已发现的不兼容静态接口和没有对应钩子的延迟函数，产出机器可读清单。解析器对每个 DLL 至少读出一个具名导入，否则失败；普通导入与延迟导入的行格式不同，不能把延迟描述符的 `Characteristics` 当成函数。

x64 与 x86 均产出同源 MCP，不能将 32 位安装包的 MCP 入口指向不可执行的 x64 程序。核心 DLL、导入库与 PDB 在 MCP 构建前收集：MCP 会重新构建同目录的 host-api 依赖，不带最终 DLL 的兼容链接参数，可能覆盖该目录中的 DLL 与符号。收集后的目录只包含核心与 MCP，没有完整设置和面板，不传给普通完整产品的发布流程。

### 范围与历史审计

- [完整 Win7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分落地。此切片没有完成设置、面板、全部运行库、安装升级和 Win7 实机验收，完整提案继续保留。
- [DPI 与绘制回退](2026-10-23-windows-optional-dpi-and-composition.md)：互补。前者处理原生宿主接口，本切片处理 Rust 依赖的装载边界，两者都不能单独代表产品已支持 Win7。
- 按 Win7、combase、延迟加载检索活跃笔记，没有既有 Rust 兼容导入决定需要替换；既有候选材质中的动态 WinRT 装载不改变。

## Testing

Windows 11 上的 x64 与 x86 专用构建完整产出同源核心 DLL 与 MCP。x64 清单记录核心 293 个普通导入与 4 个延迟导入、MCP 267 个普通导入与 1 个延迟导入；x86 分别记录 284／4 与 264／1。两个架构的实际 DLL 都通过同一导入审计与装载检查，MCP 的 `--version` 返回源码平台版本 `0.3.0`。最终收集程序的 CLI 与 MCP 协议经合成目录验证，详见 [AI 助手连接](2026-10-24-windows7-mcp-connection.md)。x86 使用 VS 的 `x64_x86` 宿主工具链：32 位 `cl.exe` 读取系统账号位于 `System32` 下的 Cargo 缓存会遭 WOW64 重定向，不能据此修改 ring 依赖或误判缺少源码。

`windows7-delay-load` 验证系统目录装载、原生函数地址透传、旧 COM 地址、未知导入拒绝、错误描述经真实 COM 线程槽往返，以及录音和播放默认 GUID 的工作线程回调、结果重复读取和输出引用。音频设备缺失时保留真实的端点错误，不据此宣称录音或播放通过。

`windows7-core-load` 实际装载收集后的 x64 与 x86 DLL，验证 ABI 3、DirectWrite 字体枚举、WASAPI 设备枚举和响应释放，不录音、不输出字体或设备身份。Win7 SP1 x64 上两架构核心装载、兼容接口、全部依赖与 TSF 冷启动均通过；真实 TSF 输入见[产品构建](2026-10-24-windows7-product-build.md)。

同源的 x86 与 x64 TSF DLL 均已链接对应 Win7 核心，并通过实际 DLL 类工厂创建、引用和卸载测试。该检查在 Windows 11 上执行，不包含安装注册、应用输入或 Win7 运行验收。

Win7 完整安装实测曾在注册 TSF 时以错误 127 失败。全新外部进程的探针稳定复现旧包失败，预加载相邻 UCRT 才能通过；静态 CRT 修复后，x64 与 x86 的 TSF 和核心分别从独占空目录的新进程冷装载通过。最终产品的 x64 全部 46 个 DLL、3369 项导入／69 个模块与 x86 全部 44 个 DLL、3235 项导入／67 个模块通过。两个架构各自执行两次独立冷装载，不依赖预先加载 UCRT。

同源重新签名的 x64 完整包在 Win7 SP1 x64 通过安装、升级、合成数据保留、嵌套数据目录迁移与卸载；分别在 64 位与 WOW64 子进程经注册表 COM 创建真实 TIP 成功。该结果不替代真实编辑器输入或独立 32 位 Win7 系统验收。

## Alternatives considered

- **把 UCRT 随 TSF 依赖闭包安装。** 无需系统更新，也能复用动态 CRT 产物；但 Win7 的插件目录转发限制使全新进程无法加载，预加载 `ucrtbase.dll` 后通过恰恰会掩盖缺陷。改用静态 CRT，并在未加载产品依赖的外部进程逐个验收 DLL。系统级安装 UCRT 会修改所有应用共享组件，也使干净 SP1 的安装依赖更新，因此不作为本产品的隐含前提。
- **全局降级依赖与 Rust 工具链。** 可以复用普通构建，但会扩大现代 Windows 和其他平台的回归范围。专用目标与最终链接参数把兼容实现限制在兼容构建中。
- **给缺失的音频接口直接返回 `E_NOTIMPL`。** 足以消除装载障碍，保留错误传播；但新版 cpal 的普通默认设备也会调用它，会让现有语音与音效失效，因此实现真实的旧 WASAPI 激活。
- **手工改写第三方导入库或生成绑定。** 可以把旧 COM 函数改到 `ole32.dll`，但还要维护第三方源码、导入符号和链接顺序。采用工具链提供的延迟加载机制，并以实际 PE 清单防止新增接口未经适配进入产物。

## Consequences

同一共享核心可以生成独立的 Win7 目标产物，已发现的新版系统静态导入不再阻止它装载，普通构建保持原流程。代价是维护标准库构建、一个 MSVC 专用兼容对象和具名导入白名单。该清单不证明所有系统导出都兼容，也不覆盖随包的 VC 运行库、ASR 运行库和其他 DLL；必须继续在真实 Win7 和现代系统上验证完整产品。

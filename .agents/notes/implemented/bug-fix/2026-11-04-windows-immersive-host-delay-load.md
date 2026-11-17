# Agent Note: Windows 沉浸式进程加载 TSF 宿主库

Status: implemented

## Problem

Windows 11 26H2 的开始菜单、搜索和设置进程能够加载 `MetasequoiaImeTsf.dll`，但不会把 TSF 所在的 IME 目录加入普通 DLL 搜索路径。`MetasequoiaImeTsf.dll` 对 `msime_host_api*.dll` 的静态依赖因此解析失败，TSF 初始化后立即卸载；Explorer 等已有其它加载路径的进程不受影响。#10629 的 Procmon 记录确认了主 DLL 加载成功、宿主库查找全部 `NAME NOT FOUND`。

## Decision

把 TSF 对版本宿主库的导入改为 MSVC 延迟加载，并在延迟加载通知中用当前 TSF 模块的绝对路径拼出同目录宿主库路径，再以 `LOAD_WITH_ALTERED_SEARCH_PATH` 加载。版本表提供宿主库文件名，所有版本和 ARM64X 原生半边沿用各自的名称；没有路径时不回退到当前目录或 `PATH`。路径拼接单独提供可测试函数，空路径和相对路径直接失败；模块路径读取复用有长度上限的 `ReadModulePath`。MSVC 通知钩子匹配 `delayimp.h` 的常量声明。ARM64EC 最终链接要为原生半边补上 `/DELAYLOAD`：`/LINKREPROFULLPATHRSP` 写出的响应文件只列链接输入的文件（每行一个带引号的绝对路径，模块定义写成 `/DEF:`），不含 `/DELAYLOAD` 这类选项，所以 `tsf/Arm64XDelayLoad.cmake` 按 ARM64 那遍推导 `MSIME_TSF_HOST_DLL_NAME` 的同一条规则，从响应文件里唯一的 `<dll>.lib` 宿主导入库得出延迟加载的 DLL 名，没有或多于一个就让配置失败。

## Alternatives considered

- **只在第一次调用时依赖 delayimp 报错**：成功路径的启动工作最少，但加载失败会以 `0xC06D007E` / `0xC06D007F` 异常越过 COM 边界，终止编辑器进程；改为在激活入口集中处理。

- **把宿主库复制到 `System32` 或加入全局 `PATH`**：污染系统范围并扩大 DLL 劫持面，也不能解决多版本并装的命名隔离，因此不采用。
- **静态链接宿主库**：会显著增大 TSF，破坏现有共享宿主生命周期和版本隔离；同时需要重做构建与安装布局，因此不采用。
- **在 `DllMain` 中预加载宿主库**：静态导入在进入 `DllMain` 前已经解析，无法修复当前失败路径；延迟加载通知才位于正确的解析时机。

## Consequences

沉浸式进程可从 TSF 安装目录解析宿主库，默认搜索路径不再决定产品行为。新增路径策略和 ARM64X 响应文件行为测试，后者的夹具按链接器实际写出的格式构造；最初的夹具假定响应文件里有 `/DELAYLOAD` 行，PR 上的检查不跑 MSVC 的 Arm64X 链接，直到 nightly 42128918255 才在 ARM64EC 配置阶段报 `lists no ARM64 delay imports`（#10980）。Windows 11 26H2 开始菜单、搜索和设置的真机验收仍需在 PR 上由 Windows 环境完成。AppContainer 读取权限与开始菜单提交异常不在本修复范围；整目录 ACL 授权已[否决](../../rejected/bug-fix/2026-11-04-windows-immersive-data-acl.md)。

## 激活时的加载失败边界

#11542 的附件给出 `0xC06D007E`；现有通知钩子在绝对路径加载失败时主动抛出同一异常，而 `ActivateEx` 在建立崩溃采集与引擎会话前没有处理它。这是已确认的错误处理缺口，不能据此断言报告者具体缺失哪一个文件，也不能把透明遮挡窗口归入同一根因。

`TsfHostImports.cpp` 在 `ActivateEx` 的最前面通过 `__HrLoadAllImportsForDll` 解析该版本的全部宿主导入，只把 delayimp 的缺模块和缺过程 SEH 转为 `HRESULT_FROM_WIN32`。失败时尚未持有 ThreadMgr、登记回调或启动后台工作；成功后各宿主调用使用已解析的导入。入口不在 `DllMain` 中加载，不回退当前目录或 `PATH`，不吞访问越界等其他异常。加载失败不缓存为永久失败，补齐依赖后允许重新激活。MinGW 继续使用系统静态导入解析，不改变其生命周期。

独立 MSVC 夹具直接编译生产加载钩子和导入检查，以合成 DLL 覆盖成功、缺 DLL、缺导出、补齐后重试与当前目录同名诱饵；每个场景在独立测试进程里执行。PR 的 Windows 原生 CI 实际构建并运行 x64 / x86 两套专项。不访问用户配置、不注册输入法，不以这些回归代替 QQ 或 Notepad-- 的报告者环境确认。

旧调用路径的对照在 x64 / x86 上分别因缺 DLL、缺导出退出 `0xC06D007E` / `0xC06D007F`，加入激活检查后两个架构的五个场景全部通过。通知钩子的 DLL 名转换使用栈数组，避免加载失败的 SEH 越过尚未释放的临时字符串。

继续查到安装器升级清理的独立缺口：递归删除旧版本目录会留下仍被应用映射的 TIP，却能删掉尚未被延迟加载的宿主 DLL。系统目录的 PowerShell 清理和旧 Program Files 目录清理都先尝试删除 `MetasequoiaImeTsf.dll`；入口仍被映射时整组保留，释放后下一轮清理再移除。入口删除成功后无法再从这个旧路径加载新的 TIP，才继续删其依赖。这不改变现有卸载文件表与重启删除策略。

升级回归从实际安装器读取并执行它拼出的清理命令，以系统 DLL 的临时独立副本模拟旧 TIP 映射，检查依赖保留、未占用版本清理、大小写不敏感的当前版本保留、解除映射后重试和空根目录移除。没有在测试里复制清理规则，也不改系统 DLL、注册输入法或执行安装。

资料依据：[微软延迟加载错误处理](https://learn.microsoft.com/en-us/cpp/build/reference/error-handling-and-notification)、[集中解析全部延迟导入](https://learn.microsoft.com/en-us/cpp/build/reference/linker-support-for-delay-loaded-dlls)。本段沿用原有同目录加载决定；历史 WER、Windows 7 兼容和小栈笔记分别处理不同机制，不替代它们。

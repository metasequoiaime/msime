# Agent Note: Windows 本地安装在 VS 2022 上编不出设置窗口，且版本号被写成 0.0.1

Status: implemented

## Problem

在一台只装了 Visual Studio 2022 生成工具的 Windows 11 上按 `installer/LocalInstall.md` 跑 `installer/test.ps1`，暴露出两个问题，CI 都看不到：

1. WinUI 3 设置工程 `platforms/windows/settings/MSIME.Settings.vcxproj` 在 restore 之后、编译之前失败，报 `Microsoft.NuGet.targets(198,5): error : 序列不包含任何元素`（Sequence contains no elements）。VS 2022 的 MSBuild 经 `Current/Microsoft.Common.Targets/ImportAfter/Microsoft.NuGet.ImportAfter.targets` 引入旧的 `Microsoft.NuGet.targets`，它的 `ResolveNuGetPackageAssets` 按 `NuGetTargetMoniker` 去 `project.assets.json` 里找目标。C++ 工程没有 `TargetFrameworkMoniker`，`NuGetTargetMoniker` 求值为空，而 restore 写出的目标是 `native,Version=v0.0` 及其各 RID，任务找不到匹配就抛异常。CI（`release-windows.yml`）用 Visual Studio 2026（生成器 `Visual Studio 18 2026`），不再引入这个文件，所以一直是绿的；而 `Build-Client.md` 和 `Build-Client.ps1` 的默认生成器正是 Visual Studio 2022。
2. 不传 `-TargetVersion` 时装上的版本是 0.0.1。`test.ps1`、`test-light.ps1`、`Invoke-LocalInstall.ps1`、`Prepare-PackageFiles.ps1` 把 `TargetVersion` 默认值写死成 `'0.0.1'`，`Invoke-LocalInstall.ps1` 总把它显式传给 `Build-Client.ps1`，于是 [更新检查](../architecture/2026-10-09-update-check-in-rust.md) 里记录的「开发构建读 `platforms/windows/version.txt`」这条回退永远轮不到：原生、Tauri 和 WinUI 设置都编成 0.0.1，安装器写进卸载项的 `DisplayVersion` 也是 0.0.1。更新检查会认为它比已发布的版本旧，提示用户「升级」回发布版，把本地构建覆盖掉。版本目录又因为之前的本地安装已经占用 `msime_v0.0.1`，被 `GetVersionDir` 加后缀成 `msime_v0.0.1.1`。

## Decision

- `MSIME.Settings.vcxproj` 的 Globals 设 `<ResolveNuGetPackages>false</ResolveNuGetPackages>`。原生 PackageReference 的头文件、库、WinRT 元数据和 Windows App SDK 运行时都由 `obj\*.nuget.g.targets` 导入的各包自带 targets 处理，旧解析器这一步对本工程没有产出；VS 2026 不引入该文件，这个属性对它没有影响。
- 四个本地安装脚本的 `TargetVersion` 默认值改为 `platforms/windows/version.txt` 的第一行。显式传 `-TargetVersion` 的行为不变；`LocalInstall.md` 里对应那句同步改写。

## Alternatives considered

- **把 `NuGetTargetMoniker` 设成 `native,Version=v0.0`** — 让旧解析器找到目标、照常运行，看起来更「正确」。否决：它会把包里的托管引用和 copy-local 项按 project.json 时代的规则解析进 C++ 工程，引入一套本工程不需要、CI 上也从未执行过的行为；关掉它才和 CI 的实际构建一致。
- **要求本地构建也装 Visual Studio 2026** — 和 CI 完全一致。否决：`Build-Client.md` 与脚本默认生成器写的就是 VS 2022，改要求等于让文档宣称支持的环境编不过。
- **本地安装默认 `TargetVersion` 为空，交给 `Build-Client.ps1` 回退** — `Build-Client.ps1` 和设置工程本来就支持空值回退。否决：`Prepare-PackageFiles.ps1` 必须把一个具体版本写进 `msime_setup.iss` 的 `MyAppVersion`，空值无处可写；在入口统一读 `version.txt`，四个阶段拿到的是同一个值。

## Consequences

- **收益**：Visual Studio 2022 上能完整编出 WinUI 设置窗口；本地安装默认就是当前 Windows 版本，不会被更新检查当成旧版本。
- **代价**：本地安装与已发布的同号版本在版本号上无法区分，需要区分时显式传 `-TargetVersion`。`version.txt` 缺失时四个脚本在参数绑定阶段就报错，不再静默用 0.0.1。把这些脚本复制进夹具目录调用的测试必须在脚本目录的上一层放一份 `version.txt`：`tests/package-files.ps1` 不传 `-TargetVersion` 调 `Prepare-PackageFiles.ps1`，引入这个默认值时没有补上，它因此在参数绑定阶段失败，直到 [空格在宿主会话里选词](2026-10-16-windows-space-selects-in-host-session.md) 那次才补。
- **重访信号**：如果某个 NuGet 包将来依赖旧解析器的 copy-local 输出，VS 2022 上会少文件，要重新评估这个属性；Visual Studio 2022 不再受支持时，这个属性可以随之删除。

## Verification

- Windows 11（Visual Studio 2022 生成工具 17.14）：不改工程时 `msbuild MSIME.Settings.vcxproj -t:ResolveNuGetPackageAssets` 复现上面的错误；加上该属性后 restore 加这一目标退出 0，`ResolveNuGetPackages=false` 下 `installer/test.ps1` 的完整构建、本地签名和安装包编译通过。
- 新默认值解析为 `0.2.1`（`version.txt` 的当前值）；`platforms/windows/installer/tests/installer_entry.ps1` 通过；`tests/package-files.ps1` 当时没有跑，补上夹具里的 `version.txt` 后通过。

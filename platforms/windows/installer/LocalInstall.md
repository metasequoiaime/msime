# Client local installation workflow

The full `test.ps1` and existing-installation-only `test-light.ps1` wrappers use `Invoke-LocalInstall.ps1` and the actual Client `Build-Client.ps1`. They do not reference legacy windows/scripts, server/scripts or ui-html build projects.

```powershell
.\platforms\windows\installer\test.ps1 -X64Dependencies C:\deps\x64 -X86Dependencies C:\deps\x86 -NoticesDirectory target/windows-notices
```

开发机上一键编译安装：先把两个依赖前缀写进环境变量，之后不带参数运行即可。

```powershell
setx MSIME_WINDOWS_X64_DEPS C:\deps\x64-windows
setx MSIME_WINDOWS_X86_DEPS C:\deps\x86-windows
.\platforms\windows\installer\test.ps1          # 完整包，首次安装用
.\platforms\windows\installer\test-light.ps1    # 轻量包，已装过完整包、只改了 TSF / Server / 设置时用
```

不传的参数这样取默认值：依赖前缀取上面两个环境变量，当前进程里没有时再读用户和系统环境变量（`setx` 之后不用重开终端），都没有就在编译前报错；第三方声明目录是 `target/windows-notices`；CMake 生成器沿用 `target/windows-full/x64` 已有缓存里的那个（与缓存不同的生成器会让 configure 失败），没有缓存时按 vswhere 找到的 Visual Studio 主版本选 `Visual Studio 18 2026` 或 `Visual Studio 17 2022`。完整包在 `target/desktop-resources` 不存在时，先照发布工作流用 `cargo run -p msime-client-core --example install_resources` 下载锁定的桌面资源，暂存时仍逐个校验。不必在 VS 开发者终端里运行：找不到 `lib.exe` 时，入口用 vswhere 找到的 Visual Studio 进入 x64 开发者环境（`Enter-VsDevShell`，只改本次会话的环境变量）。`-NoInstall` 只打出签好名的安装包，不启动安装程序。`-RustOutputs <目录>` 原样交给 `Build-Client.ps1`，跳过所有 cargo 和 pnpm 命令；只在 Rust 侧确实没改动、那份产物与当前源码同源时用，否则 Server、TSF 与宿主 DLL 的协议对不上。

This is an explicit local build/sign/install command, not a unit test: the signing scripts create or reuse a local test certificate and may modify the local trust store, and the final stage launches the installer. For building alone, use `platforms/windows/Build-Client.ps1`. For a signed public release, use `Package-SimplySign.ps1` instead. None of these scripts trigger CI.

The shared sequence is Client build, package preparation, binary signing, Inno compilation, installer signing, then installer launch. Exceptions and nonzero script/native exit status stop the sequence. The caller's working directory is restored. Full/light selection is passed consistently through preparation, compilation, signing and launch. Both architectures are selected explicitly from `target/windows-full`, with the Tauri shell beside the x64 Server.

Supply prepared native dependency prefixes and a notice directory containing `THIRD_PARTY_NOTICES.txt`. Supplying notices is not a completeness or permission check; review the limitations in `../Notices.md`. Full packages additionally need the locked `DesktopResourcesDirectory` (default `target/desktop-resources`), the pinned Engine assets and the Windows toolchains documented in `../Build-Client.md`.

`TargetVersion` is the single version for one installation build: `Build-Client.ps1` passes it to the native build as `MSIME_WINDOWS_VERSION` and to Tauri as the desktop `version`, and `Prepare-PackageFiles.ps1` rewrites `#define MyAppVersion` in `msime_setup.iss` from it. The value checked into `msime_setup.iss` is therefore whatever the last staging run wrote, not an independent source of truth. Leaving `TargetVersion` unset in the local install scripts (`test.ps1`, `test-light.ps1`, `Invoke-LocalInstall.ps1`, `Prepare-PackageFiles.ps1`) takes it from `platforms/windows/version.txt`, the same file the native build and the WinUI settings project fall back to, so a local install carries the current Windows version rather than a placeholder.

`tests/installer_entry.ps1` substitutes every side-effect stage and verifies ordering, arguments, light-mode propagation, missing notices, and failure at all six stages. It drives substituted stages, so it covers the orchestration rather than signing, compilation or installation themselves. `tests/build-failures.ps1` forwards to this regression instead of referring to removed component scripts. Registration behaviour, UIAccess and dependency closure on a given machine are properties of that machine's installed package, checked by running the installer there rather than by this suite.

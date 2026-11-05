# Agent Note: Windows 版本声明 edition.json 不带 BOM，读取端容忍开头的 BOM

Status: implemented

## Problem

#10816：11/3、11/4 两个 Windows nightly 上，托盘卡片的「云剪贴板…」「云词库…」点了没有任何反应，11/2 的包正常。

这两行打开的是共享应用 `MSIME.exe` 的面板。`MSIME.exe` 启动时先跑 `check_windows_edition`（`apps/desktop/src-tauri/src/lib.rs`），它经 `Edition::of_windows_package` 读 Server 目录里的 `edition.json`（`Edition::PACKAGE_MARKER_FILE`），读不出就 `exit(1)`。#10648 为了让 `msime_setup.iss` 改写版本号后保留 UTF-8 BOM，把 `Prepare-PackageFiles.ps1` 里同一写法的另外两处——`server_exe/edition.json` 和 `app_data/config.default.toml`——也一起改成了 `UTF8Encoding($true)`。`Edition::declared_by_package` 用 `serde_json::from_str` 解析，serde_json 不认开头的 U+FEFF，报 `expected value at line 1 column 1`，于是带 BOM 的声明被当成坏声明，`MSIME.exe` 一启动就退出。

打包测试 `package-files.ps1` 用 `Get-Content -Encoding UTF8` 读声明，这个读法会吞掉 BOM，所以没有拦住。

## Decision

- 只有 `.iss` 要 BOM（Inno Setup 6.3 之前按 ANSI 代码页读无 BOM 的脚本）。`edition.json` 和 `config.default.toml` 恢复为不带 BOM：JSON 不允许 BOM；仓库里存的 `config.default.toml` 本身也不带 BOM，按 #10648 自己「不该顺带改掉被跟踪文件的编码」的理由，它也不该被加上。
- `Edition::declared_by_package` 解析前去掉开头一个 U+FEFF。Windows 上的工具（PowerShell 5 的 `-Encoding utf8`、记事本另存为）很容易写出 BOM，内容正确的声明不该因此被当成坏声明并让设置应用退出。只去一个：去掉之后剩下的内容照旧按 `deny_unknown_fields` 的 JSON 严格校验，未知版本、多余字段、两个 BOM 仍然是错误。
- `package-files.ps1` 按字节断言 `server_exe/edition.json` 和 `app_data/config.default.toml` 开头不是 `EF BB BF`。

## Alternatives considered

- 只改打包脚本、不动读取端。这是最小改动，也足以修好新包；但读取端对一个三字节前缀如此脆弱，任何手工或其他脚本写出的声明都会让 `MSIME.exe`、`msime-mcp.exe` 静默退出，而用户看到的只是「点了没反应」。去 BOM 不放宽校验，代价只有一行，所以两头都做。
- 只改读取端、保留带 BOM 的写法。这样新的 `MSIME.exe` 能读，但 JSON 带 BOM 本身不规范，其他读取方（如以后的 C++ 侧、第三方工具）仍可能踩到，而且 `config.default.toml` 的编码改动没有理由，应当回退。

## Consequences

- 11/3、11/4 nightly 已经装上的带 BOM 声明，在装上含本修复的新包后不再影响：新包同时换掉 `MSIME.exe` 和 `edition.json`，而新的 `MSIME.exe` 即便遇到旧的带 BOM 声明也能读。
- `declared_by_package` 也是 macOS 设置应用和 Linux 设置应用读声明的入口，它们的声明由 Python 生成、不带 BOM，这一改动对它们没有行为差别。
- 未在真机 Windows 上安装验证；Windows 打包与安装冒烟由 CI 的 Windows 作业覆盖。

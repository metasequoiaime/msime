# Agent Note: TSF DLL 的版本资源取产品线版本

Status: implemented

## Problem

Win7 0.4.0 装到 Win7 SP1 上，`MetasequoiaImeTsf.dll` 的文件属性写的是 0.1.0.0，而 Server、设置窗口嵌入的版本和安装登记都是 0.4.0；现代 Windows 0.3.0 也一样。#2371 当初把 `MetasequoiaIME.rc` 的版本对齐到 Cargo 工作区的 `workspace.package.version`，并用 `tsf/tests/version_resource.cmake` 守住，那时还没有 Windows 自己的产品版本文件。后来版本号改由 `platforms/windows/version.txt`（现代）和 `version-win7.txt`（Win7）各自递增，工作区版本一直是 0.1.0，测试反而把这个旧数字钉死了。

## Decision

TSF DLL 的版本资源用本次构建所属产品线的版本，第四段为 0：

- `platforms/windows/ProductVersion.cmake` 取版本：安装包构建脚本传的 `MSIME_WINDOWS_VERSION` 优先，否则现代 Windows 读 `version.txt`，`MSIME_WINDOWS7_CLIENT` 时读 `version-win7.txt`。这与 Server 上报的 `MSIME_WINDOWS_VERSION` 同源；预发布后缀不进数字版本。它从 `tsf/IME/msime_tsf_version.h.in` 生成 `msime_tsf_version.h`，TSF 和只编资源的导出测试工程都引入它。
- `MetasequoiaIME.rc` 的 `FILEVERSION`、`PRODUCTVERSION`、`FileVersion`、`ProductVersion` 全部取自生成的宏，不再写数字。
- `version_resource.cmake` 改为检查 rc 只用生成的宏、模块读两条产品线的版本文件，并用模板把两份版本文件各生成一次核对结果。
- 顺带把资源里样例遗留的 `CompanyName "MSFT"` 和描述改为 `Metasequoia`（与安装器的 `AppPublisher` 一致）和 `Metasequoia IME text service`；`LegalCopyright` 不动。

## Alternatives considered

- **把 Cargo 工作区版本改成产品版本**：测试不用改。但现代 Windows 与 Win7 两条版本线不同，工作区只有一个版本，而且它还是所有 crate 的版本，跟着某个平台发版递增会牵动全部平台。
- **用编译定义把版本直接传给资源编译器**：省掉生成的头文件。字符串宏经编译定义传给 rc.exe 与 MinGW windres 时引号转义各不相同，只编资源的导出测试工程也要同样传一遍；生成头文件两边都一样。

## Consequences

文件属性里的版本与关于页、更新检查、安装登记一致，发版改 `version.txt`／`version-win7.txt` 时 DLL 版本自动跟上。代价是多了一个生成的头文件和一个 CMake 模块，TSF 与导出测试工程的配置都依赖它。

## Verification

`version_resource.cmake` 对新 rc 通过，对原来写死 0.1.0.0 的 rc 失败。用一个只引入 `ProductVersion.cmake` 的空工程配置三次：缺省得到 0.3.0.0（`version.txt`），`MSIME_WINDOWS7_CLIENT=ON` 得到 0.4.0.0，`MSIME_WINDOWS_VERSION=1.2.3-beta` 得到 1.2.3.0。实际编进 DLL 的资源以 Windows CI（MSVC 与 MinGW 交叉编译）为准。

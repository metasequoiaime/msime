# Windows 包管理器定义

这里是 winget、Scoop 和 Chocolatey 的包定义模板，以及把一个 `windows-v*` 发布填进模板的 `render.py`。三个包都只做一件事：静默运行发布页上的那个 Inno Setup 安装包（`release-windows.yml` 产出的 `MetasequoiaIME_Setup_v<版本>.exe`），不另编一份二进制，也不改安装包的行为。只有完整版，不提供轻量包（`_light`）。

本仓库不向任何外部仓库发布；下面「发布步骤」写的是维护者手动发布时要做的事。

## 目录

| 路径 | 内容 |
| --- | --- |
| `winget/` | 多文件清单（ManifestVersion 1.12.0）：`version`、`installer`、`defaultLocale`（en-US）和 `locale`（zh-CN） |
| `scoop/msime.json` | Scoop 清单，带 `checkver` 与 `autoupdate` |
| `chocolatey/` | `msime.nuspec` 与 `tools/chocolateyinstall.ps1`、`tools/chocolateyuninstall.ps1` |
| `render.py` | 用一个发布的版本号、安装包地址、SHA-256、日期和发布说明地址填模板 |

模板里的 `@VERSION@`、`@INSTALLER_URL@`、`@SHA256@`、`@SHA256_UPPER@`、`@RELEASE_DATE@`、`@RELEASE_NOTES_URL@` 由 `render.py` 填写；其余内容在发布之间不变。

## 三个包共同依据的安装包事实

这些都取自 `../installer/msime_setup.iss`、它包含的 `../installer/editions.iss`（由版本表 `shared/contracts/editions.json` 生成）与 `release-windows.yml`，`scripts/test-windows-package-managers.py` 核对两边一致。安装包按版本（edition）各打一个，包管理器只发 full：AppId、显示名和安装包名都取 full 那一份。

- **只有 x64**：`ArchitecturesAllowed=x64compatible`。ARM64 的 Windows 11 可以通过 x64 模拟安装，这与直接运行安装包一致；包定义里只声明 x64。
- **按机器安装**：`PrivilegesRequired=admin`，程序装到 `%ProgramFiles%\metasequoiaime`，安装包自己请求提权（winget 的 `ElevationRequirement: elevatesSelf`）。输入法要注册 TSF DLL、COM 类和登录任务，没有按用户安装的形态。
- **卸载项**：full 的 `AppId={A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}`（版本表的 `inno_app_id`），Inno 写入的卸载键是 `{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}_is1`（winget 的 `ProductCode`），显示名 `Metasequoia IME 水杉输入法`，发布者 `Metasequoia`。
- **静默参数**：安装 `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART`，卸载 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`，与 `../installer/tests/install-smoke.ps1` 在发布机上跑的相同。卸载程序会把自己复制到临时目录再启动并立即返回，所以 Scoop 和 Chocolatey 的卸载脚本等它删掉自己的文件再结束。
- **升级就地覆盖，卸载删数据**：新版安装包直接覆盖旧版，保留数据目录；卸载程序会删除它拥有的数据目录（用户词、配置、皮肤）。因此 winget 用 `UpgradeBehavior: install` 而不是 `uninstallPrevious`，Scoop 的卸载脚本在 `scoop update` 时什么都不做，只在 `scoop uninstall` 时运行卸载程序；Chocolatey 升级本来就不运行卸载脚本。
- **前置组件**：安装包不带 Visual C++ 2015-2022 x64 运行库和 WebView2 Runtime，静默安装时缺了也只写日志继续装（`InitializeSetup`）。winget 声明 `Microsoft.VCRedist.2015+.x64` 与 `Microsoft.EdgeWebView2Runtime` 依赖，Chocolatey 声明 `vcredist140`（≥ 14.20，与安装包要求的版本下限一致）与 `webview2-runtime`。Scoop 没有系统级依赖的机制，写在 `notes` 里。
- **版本号**：发布标签是 `windows-v<MAJOR.MINOR.PATCH>`，安装包旁边有 `<安装包名>.sha256`。
- **数据目录**：安装包接受 `/DATADIR=<空目录>`。Chocolatey 用包参数 `/DataDir:` 传过去；winget 和 Scoop 不暴露它，沿用旧安装的目录或默认的 `%LOCALAPPDATA%\metasequoiaime`。

### Scoop 为什么用安装包

Scoop 的惯例是便携应用：解压到 `scoop\apps\<名字>`，不写系统。输入法做不到——TSF DLL 必须以 COM 服务器注册到 HKLM，Server 要装在 Program Files 里才满足 uiAccess，还要创建登录任务——直接解压 Inno 安装包（Scoop 的 `innosetup: true`）得到的是一堆不会被系统加载的文件。所以这份清单用 `installer.script` 运行安装包，Scoop 目录里不留任何东西，`scoop install` 会弹 UAC。这样的清单不符合 Scoop 官方 Main / Extras bucket 的收录规则，发布方式是项目自己的 bucket（见下文）。

## 渲染

`render.py` 只用 Python 标准库：

```sh
# 最新一个已发布、非预发布的 windows-v* 发布
python3 platforms/windows/packaging/render.py --latest --output out/package-managers
# 指定标签
python3 platforms/windows/packaging/render.py --tag windows-v0.1.0 --output out/package-managers
# 发布任务里，用刚构建出的安装包（地址按发布页的规则推出）
python3 platforms/windows/packaging/render.py --version 0.1.0 --installer dist/MetasequoiaIME_Setup_v0.1.0.exe --output out/package-managers
# 已知摘要
python3 platforms/windows/packaging/render.py --version 0.1.0 --sha256 <hex> --release-date 2026-10-03 --output out/package-managers
```

走 GitHub 时只读发布元数据：GitHub 为每个发布附件记录的 `sha256:` 摘要，与发布页上的 `.sha256` 小文件互相核对，不下载安装包。草稿和预发布默认拒绝（`--tag` 加 `--allow-prerelease` 可以强制渲染）。设置 `GH_TOKEN` 或 `GITHUB_TOKEN` 可以提高 API 限额。

输出目录按各自仓库的布局：

```
winget/manifests/m/Metasequoia/MetasequoiaIME/<版本>/Metasequoia.MetasequoiaIME*.yaml
scoop/msime.json
chocolatey/msime/msime.nuspec
chocolatey/msime/tools/chocolateyinstall.ps1、chocolateyuninstall.ps1
```

发布页上的安装包如果在发布之后被替换（例如换成本地 SimplySign 签过名的版本，见 `../installer/README.md`），摘要随之改变，必须在最终的安装包就位之后再渲染。

## 检查

```sh
python3 scripts/test-windows-package-managers.py
```

只用标准库：核对上面那些安装包事实、渲染全部模板并检查填写结果、用一份录制的发布元数据走一遍 GitHub 路径、检查 Scoop `checkver` 的正则只取最新的非预发布 Windows 版本。

加 `--schema-dir DIR` 再按官方 schema 校验渲染结果，需要 PyYAML、jsonschema 和 lxml。DIR 里放：

```sh
w=https://raw.githubusercontent.com/microsoft/winget-cli/39739564a4aaf1071b17d163ec332a08b9bcf05c/schemas/JSON/manifests/v1.12.0
for t in version installer defaultLocale locale; do curl -fsSLO "$w/manifest.$t.1.12.0.json"; done
curl -fsSLO https://raw.githubusercontent.com/ScoopInstaller/Scoop/e6aa3b366bdee8ed138c1e0f7b85192ebdd35d0f/schema.json
curl -fsSLO https://raw.githubusercontent.com/chocolatey/NuGet.Client/b73ef145832e67a60b21d92c0b0dbcc55f96534a/src/NuGet.Core/NuGet.Packaging/compiler/resources/nuspec.xsd
```

Chocolatey 用的 nuspec 架构是它自己维护的 NuGet 分支里的那份（多了 `packageSourceUrl`、`projectSourceUrl`、`docsUrl`、`bugTrackerUrl`），其 `targetNamespace` 是占位符 `{0}`，检查脚本校验前换成 `http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd`。

## 发布步骤

先确认发布页上的安装包就是最终版本（已签名，或确定不再替换），再渲染：

```sh
python3 platforms/windows/packaging/render.py --tag windows-v<版本> --output out/package-managers
```

### winget

1. 在 Windows 上本地验证：`winget validate --manifest out\package-managers\winget\manifests\m\Metasequoia\MetasequoiaIME\<版本>`，再以管理员身份 `winget settings --enable LocalManifestFiles` 后 `winget install --manifest <同一目录>`，装完 `winget list Metasequoia.MetasequoiaIME` 应能按 ProductCode 认出已安装版本，最后 `winget uninstall Metasequoia.MetasequoiaIME`。
2. fork `microsoft/winget-pkgs`，把 `winget/manifests/m/Metasequoia/MetasequoiaIME/<版本>/` 原样放进去，开 PR（或用 `wingetcreate submit <目录>`，它会代为 fork 和开 PR）。首次提交是新包，需要通过 winget-pkgs 的自动验证（会在沙箱里下载、扫描并安装）和人工审核；未签名的安装包能提交，但 SmartScreen 信誉更差。
3. 之后每个版本重复第 2 步；`wingetcreate update Metasequoia.MetasequoiaIME --version <版本> --urls <安装包地址> --submit` 也可以，但它会重新生成清单，依赖和说明以这里的模板为准，提交前要比对。

### Scoop

1. 建一个 bucket 仓库（例如 `metasequoiaime/scoop-bucket`），把渲染出的 `scoop/msime.json` 放到 `bucket/msime.json`。
2. 用户安装：`scoop bucket add msime https://github.com/metasequoiaime/scoop-bucket`，然后 `scoop install msime`。不建 bucket 也可以直接 `scoop install <msime.json 的 raw 地址>`。
3. 后续版本：bucket 里配 Scoop 的 Excavator（`ScoopInstaller/GithubActions` 的 `excavate` 任务），它按 `checkver` 发现新的 `windows-v*` 发布、按 `autoupdate` 改写地址并从 `.sha256` 取摘要，自动提交；也可以每次发布后重新渲染、手动提交。
4. 在 Windows 上验证：`scoop install .\out\package-managers\scoop\msime.json`、`scoop update msime`（数据目录应保留）、`scoop uninstall msime`。

### Chocolatey

1. 在 Windows 上打包并本地安装：`cd out\package-managers\chocolatey\msime`，`choco pack`，然后以管理员身份 `choco install msime --source . -y`；可选 `--params "'/DataDir:D:\msime-data'"`。验证后 `choco uninstall msime -y`。
2. 在 community.chocolatey.org 注册账号，取 API key：`choco apikey --key <key> --source https://push.chocolatey.org/`。
3. `choco push msime.<版本>.nupkg --source https://push.chocolatey.org/`。社区仓库会先跑自动校验（validator）、在测试机上安装卸载（verifier），再进人工审核；首个版本审核时间最长。包从发布页下载安装包并按 `checksum64` 校验，没有内嵌二进制，所以不需要 `VERIFICATION.txt`。
4. 首次审核通过前，`owners` 里的 `Metasequoia` 要换成实际的 Chocolatey 账号名。

## 发布任务接入

`release-windows.yml` 的 `package-definitions` job 在发布之后运行：`render.py --tag windows-v<版本> --output target/package-managers`（预发布加 `--allow-prerelease`），再用 `choco pack` 打出 `chocolatey/msime.<版本>.nupkg`，整个目录作为构建产物 `msime-package-definitions-windows-<版本>` 上传。摘要取自发布页本身，不是构建机上算的那份。这个 job 不向任何外部仓库推送；上面的发布步骤可以直接用这个产物，跳过自己渲染。没有勾选发布（`publish=false`）时它不运行，因为定义里的安装包地址只有发布之后才存在。

# Windows 包管理器定义

这里是 winget、Scoop 和 Chocolatey 的包定义模板，以及把一个 `windows-v*` 发布填进模板的 `render.py`。三个包都只做一件事：静默运行发布页上的那个 Inno Setup 安装包（`release-windows.yml` 产出的 `MetasequoiaIME_Setup_v<版本>.exe`），不另编一份二进制，也不改安装包的行为。只有完整版，不提供轻量包（`_light`）。

本仓库不向任何外部仓库发布；下面「发布步骤」写的是维护者手动发布时要做的事。

## 目录

| 路径 | 内容 |
| --- | --- |
| `winget/` | 多文件清单（ManifestVersion 1.12.0）：`version`、`installer`、`defaultLocale`（en-US）和 `locale`（zh-CN） |
| `scoop/msime.json` | Scoop 清单（不带 `checkver` 与 `autoupdate`，原因见「签名」） |
| `chocolatey/` | `msime.nuspec` 与 `tools/chocolateyinstall.ps1`、`tools/chocolateyuninstall.ps1` |
| `render.py` | 核对安装包的签名，再用一个发布的版本号、安装包地址、SHA-256、日期和发布说明地址填模板 |

模板里的 `@VERSION@`、`@INSTALLER_URL@`、`@SHA256@`、`@SHA256_UPPER@`、`@RELEASE_DATE@`、`@RELEASE_NOTES_URL@` 由 `render.py` 填写；其余内容在发布之间不变。

## 三个包共同依据的安装包事实

这些都取自 `../installer/msime_setup.iss`、它包含的 `../installer/editions.iss`（由版本表 `shared/contracts/editions.json` 生成）与 `release-windows.yml`，`scripts/test-windows-package-managers.py` 核对两边一致。安装包按版本（edition）各打一个，包管理器只发 full：AppId、显示名和安装包名都取 full 那一份。

- **只有 x64**：`ArchitecturesAllowed=x64compatible`。ARM64 的 Windows 11 可以通过 x64 模拟安装，这与直接运行安装包一致；包定义里只声明 x64。
- **按机器安装**：`PrivilegesRequired=admin`，程序装到 `%ProgramFiles%\metasequoiaime`，安装包自己请求提权（winget 的 `ElevationRequirement: elevatesSelf`）。输入法要注册 TSF DLL、COM 类和登录任务，没有按用户安装的形态。
- **卸载项**：full 的 `AppId={A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}`（版本表的 `inno_app_id`），Inno 写入的卸载键是 `{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}_is1`（winget 的 `ProductCode`），显示名 `Metasequoia IME 水杉输入法`，发布者 `Metasequoia`。
- **静默参数**：安装 `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART`，卸载 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`，与 `../installer/tests/install-smoke.ps1` 在发布机上跑的相同。卸载程序会把自己复制到临时目录再启动并立即返回，所以 Scoop 和 Chocolatey 的卸载脚本等它删掉自己的文件再结束；3 分钟后文件还在（例如拒绝了 UAC），Scoop 报错让 `scoop uninstall` 失败、保留记录，Chocolatey 给出警告。
- **升级就地覆盖，卸载删数据**：新版安装包直接覆盖旧版，保留数据目录；卸载程序会删除它拥有的数据目录（用户词、配置、皮肤）。因此 winget 用 `UpgradeBehavior: install` 而不是 `uninstallPrevious`，Scoop 的卸载脚本在 `scoop update` 时什么都不做，只在 `scoop uninstall` 时运行卸载程序；Chocolatey 升级本来就不运行卸载脚本。
- **前置组件**：安装包不带 Visual C++ 2015-2022 x64 运行库和 WebView2 Runtime，静默安装时缺了也只写日志继续装（`InitializeSetup`）。winget 声明 `Microsoft.VCRedist.2015+.x64` 与 `Microsoft.EdgeWebView2Runtime` 依赖，Chocolatey 声明 `vcredist140`（≥ 14.20，与安装包要求的版本下限一致）与 `webview2-runtime`。Scoop 没有系统级依赖的机制，写在 `notes` 里。
- **版本号**：发布标签是 `windows-v<MAJOR.MINOR.PATCH>`，安装包旁边有 `<安装包名>.sha256`。
- **数据目录**：安装包接受 `/DATADIR=<空目录>`。Chocolatey 用包参数 `/DataDir:` 传过去；winget 和 Scoop 不暴露它，沿用旧安装的目录或默认的 `%LOCALAPPDATA%\metasequoiaime`。
- **不支持以 SYSTEM 身份静默安装**：默认数据目录 `%LOCALAPPDATA%\metasequoiaime` 按运行安装包的账户解析，SYSTEM 的落在 `C:\Windows\System32\config\systemprofile` 下，安装包拒绝 Windows 目录里的数据目录（`DataDirRejectionReason`），所以 Intune、winget-autoupdate、SYSTEM 计划任务这类托管部署会失败；拒绝时静默安装只写日志并以非零退出码结束（`NextButtonClick` 用 `SuppressibleMsgBox`），不会卡在看不见的对话框上。Chocolatey 可以用 `/DataDir:` 绕开，winget 没有办法，两边的说明（winget 的 `InstallationNotes`、nuspec 的描述）都写了这一点。

### Scoop 为什么用安装包

Scoop 的惯例是便携应用：解压到 `scoop\apps\<名字>`，不写系统。输入法做不到——TSF DLL 必须以 COM 服务器注册到 HKLM，Server 要装在 Program Files 里才满足 uiAccess，还要创建登录任务——直接解压 Inno 安装包（Scoop 的 `innosetup: true`）得到的是一堆不会被系统加载的文件。所以这份清单用 `installer.script` 运行安装包，Scoop 目录里不留任何东西，`scoop install` 会弹 UAC。这样的清单不符合 Scoop 官方 Main / Extras bucket 的收录规则，发布方式是项目自己的 bucket（见下文）。

## 签名

包管理器只能指向签过名的安装包。`release-windows.yml` 在 CI 上打出并先行发布的 `MetasequoiaIME_Setup_v<版本>.exe` 没有签名（签名证书是只在发布机上的 Certum SimplySign 卡）；它里面的 x64 Server 以 `MSIME_SERVER_UIACCESS=ON` 构建，未签名的 uiAccess 程序系统拒绝启动，装上之后只能打英文（见 `../installer/Sign-InstalledServer-Local.ps1`）。能用的是维护者用 `../installer/Package-SimplySign.ps1` 签名后、连同新的 `.sha256` 替换到同一个发布上的那一份。替换会改变摘要，所以包定义必须在替换之后渲染。

因此：

- `render.py` 在渲染前用 `Get-AuthenticodeSignature` 核对安装包的签名，状态不是 `Valid` 就失败，不写任何文件。走 GitHub 时它会下载安装包本身来核对。这个检查只有 Windows 有：其他系统上的 `osslsigncode verify` 只能对着 TLS 用的 CA 证书包验证，代码签名证书的根常常不在里面，合法签名也报失败（python.org 的安装包实测如此），所以 `render.py` 在 Windows 以外的系统上直接拒绝，在 macOS 或 Linux 上请手动触发 `package-definitions-windows.yml`。
- 渲染包定义的是单独手动触发的 `package-definitions-windows.yml`，不在 `release-windows.yml` 发布之后自动运行。
- Scoop 清单不带 `checkver`/`autoupdate`：Scoop 的 Excavator 会在新的 `windows-v*` 发布出现时自动改写 bucket，那时发布页上还是未签名的安装包，而它不经过 `render.py` 的签名检查。

## 渲染

`render.py` 只用 Python 标准库，在 Windows 上运行（签名检查用 PowerShell 的 `Get-AuthenticodeSignature`）：

```sh
# 最新一个已发布、非预发布的 windows-v* 发布
python3 platforms/windows/packaging/render.py --latest --output out/package-managers
# 指定标签
python3 platforms/windows/packaging/render.py --tag windows-v0.1.0 --output out/package-managers
# 本地签好名、还没上传的安装包（地址按发布页的规则推出）
python3 platforms/windows/packaging/render.py --version 0.1.0 --installer dist/MetasequoiaIME_Setup_v0.1.0.exe --output out/package-managers
```

走 GitHub 时，GitHub 为每个发布附件记录的 `sha256:` 摘要与发布页上的 `.sha256` 小文件互相核对，再下载安装包，核对它就是摘要说的那一份并检查签名。草稿和预发布默认拒绝（`--tag` 加 `--allow-prerelease` 可以强制渲染）。设置 `GH_TOKEN` 或 `GITHUB_TOKEN` 可以提高 API 限额。

输出目录按各自仓库的布局：

```
winget/manifests/m/Metasequoia/MetasequoiaIME/<版本>/Metasequoia.MetasequoiaIME*.yaml
scoop/msime.json
chocolatey/msime/msime.nuspec
chocolatey/msime/tools/chocolateyinstall.ps1、chocolateyuninstall.ps1
```

发布页上的安装包被替换之后摘要随之改变，所以只在签名的安装包就位之后渲染（见上文「签名」）。

## 检查

```sh
python3 scripts/test-windows-package-managers.py
```

只用标准库：核对上面那些安装包事实、渲染全部模板并检查填写结果、用一份录制的发布元数据走一遍 GitHub 路径（包括拒绝未签名的安装包和与摘要不符的下载）、确认没有签名的文件过不了真实的签名检查、Scoop 清单不自动更新，以及包定义只在手动触发的工作流里渲染。

加 `--schema-dir DIR` 再按官方 schema 校验渲染结果，需要 PyYAML、jsonschema 和 lxml。DIR 里放：

```sh
w=https://raw.githubusercontent.com/microsoft/winget-cli/39739564a4aaf1071b17d163ec332a08b9bcf05c/schemas/JSON/manifests/v1.12.0
for t in version installer defaultLocale locale; do curl -fsSLO "$w/manifest.$t.1.12.0.json"; done
curl -fsSLO https://raw.githubusercontent.com/ScoopInstaller/Scoop/e6aa3b366bdee8ed138c1e0f7b85192ebdd35d0f/schema.json
curl -fsSLO https://raw.githubusercontent.com/chocolatey/NuGet.Client/b73ef145832e67a60b21d92c0b0dbcc55f96534a/src/NuGet.Core/NuGet.Packaging/compiler/resources/nuspec.xsd
```

Chocolatey 用的 nuspec 架构是它自己维护的 NuGet 分支里的那份（多了 `packageSourceUrl`、`projectSourceUrl`、`docsUrl`、`bugTrackerUrl`），其 `targetNamespace` 是占位符 `{0}`，检查脚本校验前换成 `http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd`。

## 发布步骤

先把签名的安装包和它的 `.sha256` 替换到发布上（`gh release upload windows-v<版本> MetasequoiaIME_Setup_v<版本>.exe MetasequoiaIME_Setup_v<版本>.exe.sha256 --clobber`），再在 Windows 上渲染；不在 Windows 上时手动触发 `package-definitions-windows.yml` 取它的产物（见下文「工作流」）：

```sh
python platforms/windows/packaging/render.py --tag windows-v<版本> --output out/package-managers
```

### winget

1. 在 Windows 上本地验证：`winget validate --manifest out\package-managers\winget\manifests\m\Metasequoia\MetasequoiaIME\<版本>`，再以管理员身份 `winget settings --enable LocalManifestFiles` 后 `winget install --manifest <同一目录>`，装完 `winget list Metasequoia.MetasequoiaIME` 应能按 ProductCode 认出已安装版本，最后 `winget uninstall Metasequoia.MetasequoiaIME`。
2. fork `microsoft/winget-pkgs`，把 `winget/manifests/m/Metasequoia/MetasequoiaIME/<版本>/` 原样放进去，开 PR（或用 `wingetcreate submit <目录>`，它会代为 fork 和开 PR）。首次提交是新包，需要通过 winget-pkgs 的自动验证（会在沙箱里下载、扫描并安装）和人工审核。
3. 之后每个版本重复第 2 步；`wingetcreate update Metasequoia.MetasequoiaIME --version <版本> --urls <安装包地址> --submit` 也可以，但它会重新生成清单，依赖和说明以这里的模板为准，提交前要比对。

### Scoop

1. 建一个 bucket 仓库（例如 `metasequoiaime/scoop-bucket`），把渲染出的 `scoop/msime.json` 放到 `bucket/msime.json`。
2. 用户安装：`scoop bucket add msime https://github.com/metasequoiaime/scoop-bucket`，然后 `scoop install msime`。不建 bucket 也可以直接 `scoop install <msime.json 的 raw 地址>`。
3. 后续版本：签名的安装包替换上去之后重新渲染，把新的 `msime.json` 提交到 bucket。不要给 bucket 配 Excavator 自动更新，原因见上文「签名」。
4. 在 Windows 上验证：`scoop install .\out\package-managers\scoop\msime.json`、`scoop update msime`（数据目录应保留）、`scoop uninstall msime`。

### Chocolatey

1. 在 Windows 上打包并本地安装：`cd out\package-managers\chocolatey\msime`，`choco pack`，然后以管理员身份 `choco install msime --source . -y`；可选 `--params "'/DataDir:D:\msime-data'"`。验证后 `choco uninstall msime -y`。
2. 在 community.chocolatey.org 注册账号，取 API key：`choco apikey --key <key> --source https://push.chocolatey.org/`。
3. `choco push msime.<版本>.nupkg --source https://push.chocolatey.org/`。社区仓库会先跑自动校验（validator）、在测试机上安装卸载（verifier），再进人工审核；首个版本审核时间最长。包从发布页下载安装包并按 `checksum64` 校验，没有内嵌二进制，所以不需要 `VERIFICATION.txt`。
4. 首次审核通过前，`owners` 里的 `Metasequoia` 要换成实际的 Chocolatey 账号名。

## 工作流

`package-definitions-windows.yml` 只能手动触发（`gh workflow run package-definitions-windows.yml -f version=<版本>`，预发布加 `-f prerelease=true`），在签名的安装包替换到 `windows-v<版本>` 之后运行。它在 Windows runner 上跑 `render.py --tag windows-v<版本> --output target/package-managers`：下载发布页上的安装包，核对摘要，要求 `Get-AuthenticodeSignature` 报告 `Valid`；再用 `choco pack` 打出 `chocolatey/msime.<版本>.nupkg`，整个目录作为构建产物 `msime-package-definitions-windows-<版本>` 上传。发布页上还是 CI 打的未签名安装包时它失败，不产出任何定义。它不向任何外部仓库推送；上面的发布步骤可以直接用这个产物，跳过自己渲染。`release-windows.yml` 不渲染包定义，因为它发布的正是那份未签名的安装包。

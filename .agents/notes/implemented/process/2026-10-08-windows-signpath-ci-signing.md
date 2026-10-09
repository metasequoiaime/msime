# Agent Note: Windows 发布在 CI 里经 SignPath 签名

Status: implemented

## Problem

`release-windows.yml` 只出未签名的安装包。未签名的 Server 带 `uiAccess=true` 起不来，Defender 会拦未签名的辅助进程，CS2 的 Trusted Mode 也只放行签了名的 DLL。签名一直是发布机上的本地步骤（`Package-SimplySign.ps1`，Certum SimplySign 虚拟卡），每次发版都要维护者手工重打六个版本的安装包、替换发布附件和符号包。

SignPath Foundation 的开源计划已经批准了项目 `msime-windows`，签名令牌存在组织 secret `SIGNPATH_TOKEN` 里。但这个 SignPath 项目登记的仓库只有 `metasequoiaime/msime-windows`：他们的条款要求签名的二进制来自登记的源码仓库（"at the noted source code repository"），本仓库要先由 SignPath Foundation 加进同一个项目才能用。

## Decision

`release-windows.yml` 带上 SignPath 签名路径，由仓库变量 `WINDOWS_SIGNING_PROVIDER=signpath` 打开，默认关闭，关闭时行为与原来一样（未签名，Depot runner）。

- **开关和配置**：`source` 任务决定本次签不签。变量是 `signpath` 时，`SIGNPATH_TOKEN`、`SIGNPATH_ORGANIZATION_ID`、`SIGNPATH_PROJECT_SLUG`、`SIGNPATH_SIGNING_POLICY_SLUG` 缺一个就失败，不悄悄退回未签名；变量是别的值也失败。开关变量必须建在本仓库，不能建在组织级：msime-windows 用同名变量打开它自己的 SignPath 路径，组织级的会让两个仓库一起打开。
- **只签 `main` 和 `develop`**：其他分支的 `-alpha` 是试验构建，照旧不签名，不占审批。
- **签名测试**：派发时勾上 `signpath_test`，不看开关、不限分支，用 SignPath 的 `test-signing` 策略走一遍两个签名任务。测试证书不受系统信任，策略不要审批、也不核对来源，所以这种运行从不发布，并且用单独的并发组，不挡真正的发布。正式证书还没签发时（`release_certificate_2026` 处于 CsrPending），用它先验证托管 runner 构建和整条签名链路。
- **runner**：SignPath 对开源项目检查签名请求之前的每个 job 都在 GitHub 托管 runner 上，所以签名时 `rust`、`release`、`package` 从 `depot-windows-2025-8` 换到 `windows-2025`，sccache 改用 GitHub Actions 缓存（`SCCACHE_GHA_ENABLED`），`rust` 的超时放宽到 120 分钟。
- **两次批量签名请求**：发布签名每个请求都要审批人在 SignPath 里批准。六个版本各签两次是十二次审批，所以任务拆成 `release`（构建、暂存、收符号，上传不带 PDB 的暂存包和本项目二进制）→ `sign-payload`（六个版本的二进制合成一次请求）→ `package`（按版本放回签好的二进制、编译安装包、装卸冒烟）→ `sign-installers`（六个安装包合成一次请求，重算 `.sha256`，作为 `msime-windows-signed-<版本>` 交给下游）。一次发版批准两次。
- **签什么**：只签本项目从源码构建的二进制（`installer/SignPath-PackageBinaries.ps1`）：`server_exe` 下有同名 PDB 的 EXE（`Prepare-PackageFiles.ps1` 要求每个 Server EXE 带 PDB，Windows App SDK 自带的除外），以及各架构的 TIP 和本版本的宿主 DLL（版本表 `host_dll`）。Windows App SDK、ONNX Runtime、sherpa-onnx 和 vcpkg 构建的 DLL 保持上游原样：SignPath Foundation 条款是 "The team must only sign software artifacts built from their own source code"，同时允许签名的安装包里带未签名的上游 DLL。
- **artifact configuration**：`installer/signpath/msime-payload.xml` 和 `msime-installer.xml` 是评审过的副本，要原样贴进 SignPath 项目，slug 分别是 `msime-payload` 和 `msime-installer`，不与 msime-windows 的 `payload` / `installer` 冲突。
- **下游不分签没签**：签名时只有 `msime-windows-signed-<版本>` 匹配 `msime-windows-*`，不签名时是六个 `msime-windows-<版本名>-<版本>`。coexistence 和 publish 按模式取，arm64 按开关选产物名。`package` 之后的任务用 `!cancelled() && !failure()`，被跳过的签名任务不连带跳过它们。
- `docs/code-signing-policy.md` 是本仓库的签名政策页（SignPath Foundation 要求项目公开），写明当前未启用、签名范围、角色和流程。

## Alternatives considered

- **只在 msime-windows 签，本仓库继续出未签名包** — 那边已经有 SignPath 路径，项目也只登记了那个仓库，不用等 SignPath Foundation。但 Windows 发布要迁到本仓库，迁过来时还得再做一遍，而且在那之前本仓库六个版本的安装包一直要手工签。两边都做，msime-windows 那份（metasequoiaime/msime-windows#709）立刻可用，本仓库这份等登记完成后开开关即用。
- **每个版本在自己的 job 里签两次，编译留在 `release` 里** — job 结构几乎不变，不用在 job 之间传暂存包。但一次发版要批准十二次，任何一次没批都会让那个版本超时失败。
- **签名时也留在 Depot runner 上** — Depot 8 核、带 Depot Cache，构建快得多。但 SignPath 对开源项目的检查明确要求 "All jobs of the GitHub workflow leading up to the signing request were executed on GitHub-hosted agents"，Depot 在 GitHub 看来是自托管。
- **把 TIP 旁边的 vcpkg DLL 一起提交签名** — CS2 Trusted Mode 需要系统目录里每个 DLL 都签名，SimplySign 的本地流程就是全签。但这违反 SignPath Foundation 条款，会危及整个项目（包括 msime-windows）的签名资格。

## Consequences

- 开关打开前什么都不变。打开要四步：SignPath Foundation 把 `https://github.com/metasequoiaime/msime.git` 加进 `msime-windows` 项目；在 SignPath 里建 `msime-payload` 和 `msime-installer` 两个 artifact configuration；在本仓库建 `SIGNPATH_ORGANIZATION_ID`、`SIGNPATH_PROJECT_SLUG`（`msime-windows`）、`SIGNPATH_SIGNING_POLICY_SLUG` 变量；最后建 `WINDOWS_SIGNING_PROVIDER=signpath`。
- 签名时构建在 4 核托管 runner 上，比 Depot 慢；两个签名任务各自等审批，最多等 80 分钟。暂存包（不带 PDB）每个版本多传一次 artifact。
- 不签名时多了暂存包的上传下载和一个 `package` 任务，代价是几分钟。
- CS2 Trusted Mode：SignPath 签的安装包里 TIP 旁边的 `fmt.dll`、`libcurl.dll`、`zlib1.dll` 未签名，按 [CS2 Trusted Mode 的 System32 部署](../../implemented/feature/2026-10-08-windows-cs2-trusted-mode-system32-deployment.md) 的分析大概率仍被拒；要解决得让 TIP 和宿主 DLL 静态链接这几个依赖。
- 托管 runner 上的构建、两个签名任务和 artifact configuration 的通配写法都还没真正跑过，第一次签名发布就是它们的验证。

## Verification

- `actionlint` 通过 `release-windows.yml`、`ci-platforms.yml` 和 `package-definitions-windows.yml`。
- `installer/tests/signpath-payload.ps1` 用合成暂存包核对认定规则：Stage 只交本项目的十二个文件，上游 DLL 和没有 PDB 的 EXE 一个不带；换成别的版本、缺 PDB、SignPath 少送回文件或送回未签名文件时都失败。它在 `ci-platforms.yml` 的 `windows-scripts` 作业里跑。
- 未验证：托管 runner 上的完整发布、真实的 SignPath 签名请求。要等 SignPath 登记本仓库后，在 `develop` 上派发一次（`publish` 关掉）验收。

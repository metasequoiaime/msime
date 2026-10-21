# Agent Note: Windows 版本发布页的符号包挪到 windows-symbols，加 SHA256SUMS 和下载指引，每个安装程序的 .sha256 保留

Status: implemented

## Problem

维护者定下的发布页标准（`.agents/skills/release-platform/SKILL.md` 第 5 步）是：版本发布页只放用户该下载的安装包和一份只列这些安装包的 `SHA256SUMS`，发布说明开头是从 `shared/contracts/editions.json` 生成的「下载哪个」指引，用户不手动下载的文件挪到别处。macOS（[macOS 版本发布页只放 DMG](2026-10-20-macos-release-page-only-dmgs.md)）和 Android（[Android 版本发布页加 SHA256SUMS 和下载指引](2026-10-20-android-release-page-sha256sums.md)）已经按它发。

windows-v0.3.0-beta 上是 18 个附件：六个安装程序、六个 `.exe.sha256`，以及六个各约 470 MB 的符号包（`msime-windows-<版本 id>-<版本号>-symbols.zip`，合计 2.8 GB），和安装程序混在一起；没有 `SHA256SUMS`，发布说明也没说该下哪个。windows-v0.2.0、0.2.1 也是这样。

## Decision

- **符号包挪到滚动的 `windows-symbols` release**，照 macOS 的 `macos-symbols`：新脚本 `platforms/windows/scripts/publish-symbols.sh <version> <release> <zip>...` 把 `Collect-Symbols.ps1` 写出的六个符号包传上去，名字里的版本号换成发布名（beta 是 `msime-windows-full-0.3.0-beta-symbols.zip`，免得覆盖同一版本号的正式版），release 不存在时以预发布、非 latest 创建，上传后逐个和 GitHub 记录的 `digest` 比对。它是 publish 任务里版本页之后的单独一步，与 macOS 的 dSYM 一样；符号包是否六个齐全、版本号是否对得上，在发布版本页之前就核对，缺一个什么都不发。
- **版本页上传的清单是六个安装程序、`SHA256SUMS` 和六个 `.exe.sha256`**。`SHA256SUMS` 在 publish 任务里按 `dist/*.exe` 现算（`sha256sum` 写、`sha256sum -c` 核对），安装程序必须每个版本恰好一个，每个 `.exe.sha256` 必须与 `SHA256SUMS` 里那一行相同，否则什么都不发。重跑旧版本时 `publish-release.sh` 会删掉版本页上不在清单里的资产，所以重跑也会把以前挂上去的符号包清掉。
- **发布说明开头是 `edition_windows.py download-guide` 生成的下载指引**：一张「版本 | 安装程序」表，名字和安装程序名取自版本表，完整版排第一、标为推荐；说明其他版本可以和完整版装在一起、每个安装程序都支持 x64 和 ARM64（`msime_setup.iss` 的 `ArchitecturesAllowed=x64compatible`）、没加进键盘列表时设置窗口会提示「去添加」、`SHA256SUMS` 和 `.exe.sha256` 各是什么。带后缀的预发布另加一句：设置里的「检查更新」只提供正式版（`crates/client-services/src/update_check.rs` 跳过预发布）。整段夹在与其他平台同一对 `<!-- msime-download-guide -->` 标记之间。

### 为什么 .exe.sha256 不挪走

Windows 的应用内更新不读它：`update_check.rs` 用的是 GitHub 给资产记录的 `digest`，而且只提供正式版。挡住撤掉它的是另外两处：

- `platforms/windows/packaging/render.py` 生成 winget、Scoop、Chocolatey 清单时，拿 `<安装程序>.sha256` 与 GitHub 摘要互相核对（`scripts/test-windows-package-managers.py` 检查 workflow 仍上传它）。维护者用 SimplySign 签名后替换安装程序的手工流程也是连同它一起换。
- 设置页（原生 WinUI 窗口和共享页面）拿不到 GitHub 摘要时，未签名警告让用户「从发行页一并下载 `<安装程序>.sha256`」。已发出去的版本里这段文案改不了。

撤掉它要连 `render.py`、两处设置文案和维护者的替换流程一起改成读 `SHA256SUMS`，是一件单独的事，由维护者决定做不做。下载指引写明了它不用下载，SKILL.md 的核对条目也写明不把它当作多出来的附件。

## Alternatives considered

- **一步到位，`.exe.sha256` 也撤掉，`render.py` 和设置文案改读 `SHA256SUMS`** — 最强的理由是版本页完全符合标准，和 macOS 一样只有安装包和 `SHA256SUMS`。否定：这会改动包管理器清单的生成、两处用户能看到的文案和维护者的人工签名流程，影响面远大于挪符号包；而附件里真正碍事的是 2.8 GB 的符号包，`.sha256` 每个 100 字节左右，下载指引已经说明它们不用下载。
- **把 macOS 的 `publish-dsym.sh` 改成两个平台共用的脚本** — 最强的理由是少一份几乎相同的上传与摘要核对逻辑。否定：两边的命名规则（macOS 一个 dSYM 覆盖全部版本，Windows 每个版本一个符号包、要换发布名）和调用方式不同，合并要改动 macOS 的发版路径并重新验证它；各平台的发版脚本本来就放在自己的 `platforms/<os>/scripts` 下。
- **符号包上传放在版本页之前** — 最强的理由是先确认符号在，再让用户看到版本。否定：齐全与否已经在发布版本页之前核对过，剩下的失败只可能是网络或 API，那时版本页已经可以下载，符号包重跑这一步补上即可；与 macOS 的顺序一致，读 workflow 的人不用记两种。

## Consequences

- **收益**：版本发布页从 18 个附件变成 13 个（六个安装程序、`SHA256SUMS`、六个 `.exe.sha256`），去掉了 2.8 GB 用户不需要的下载；发布说明第一段告诉人该下哪个。符号包按发布名长期留在 `windows-symbols` 上，不会像 workflow artifact 那样 90 天后过期。
- **代价与已知上限**：
  - 版本页仍比标准多六个 `.exe.sha256`，原因和撤掉的前提见上文。
  - 维护者替换签名安装程序时多一步：把 `SHA256SUMS` 里那一行一起换（`platforms/windows/packaging/README.md`「发布步骤」给了命令），符号包改传到 `windows-symbols`（`platforms/windows/installer/README.md`）。
  - 已发布的 windows-v0.2.0、0.2.1、0.3.0-beta 不在这里改：它们的符号包仍在版本页上，也没有 `SHA256SUMS` 和下载指引。补救要把符号包按发布名传到 `windows-symbols`、从版本页删掉，从六个 `.sha256` 合成 `SHA256SUMS` 并和 GitHub `digest` 核对后上传，再把下载指引加到发布说明前面；改公开发布前要先问维护者。
  - 官网下载页的更新记录（msime-web）会把下载指引当正文显示，与 macOS、Android 一样要在 msime-web 里按那对标记跳过。

## Verification

- `python3 scripts/test-windows-release-page.py`（14 项）：把 workflow 里「Publish independent Windows release」和「Upload the symbols to windows-symbols」两步的脚本原样取出来，在临时目录里配上仓库的版本表、`edition_windows.py`、`publish-symbols.sh`，记录参数的假 `generate-release-notes.py`、`publish-release.sh` 和一个假的 `gh` 来跑。版本页上传清单正好是六个安装程序、`SHA256SUMS` 和六个 `.exe.sha256`，没有符号包；`SHA256SUMS` 只列安装程序、摘要正确；发布说明以下载指引开头、链接指向这个 tag，beta 写明是预览版；少一个安装程序、少一个符号包、多一个安装程序、`.exe.sha256` 与安装程序对不上时什么都不发。符号包以发布名传到 `windows-symbols`，release 以预发布、非 latest 创建，beta 的符号包与正式版并存；别的版本号的符号包在上传前被拒绝。下载指引另测完整版排第一、名字取自版本表、每个 Windows 版本都有一行、缺完整版时拒绝生成。对 develop 上旧版 workflow 跑同一组发布步骤测试，7 项全部失败。
- `scripts/test-windows-package-managers.py`、`scripts/test-editions.py` 和其他平台的发布页测试照常通过；`actionlint .github/workflows/release-windows.yml`、`shellcheck platforms/windows/scripts/publish-symbols.sh` 通过。
- 还没有在真实的 Release Windows 运行里跑过：publish 任务只在打开 publish 的运行里执行，下一次发布才会第一次创建 `windows-symbols` release。

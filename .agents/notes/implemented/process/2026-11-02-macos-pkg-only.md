# Agent Note: macOS 只发 pkg 安装包

Status: implemented

## Problem

macOS 从 0.51.1 到 0.56.x 发的是 DMG：拖进「应用程序」、再打开一次设置应用、点「立即安装」。拖完不打开就等于没装，`安装说明.txt` 和设置页的提示都在补这个缺口。维护者决定改成只发 pkg：双击安装包、按提示装完，发布页上仍然每个版本只有一个安装包。

DMG 不是当年比较过 pkg 之后选的。仓库最开始（MSIME-Apple 时期）每次发布都出 pkg（`productsign` 用 Developer ID Installer 签名、公证、staple，2026-09-16 到 09-18 有 5 次成功运行，例如 35318102805），另有 ZIP 加 `Install.command` 和 Sparkle 用的 update.zip。2026-09-22 仓库换成 monorepo，78ec6fde9f 把旧 pkg 工作流原样搬回来，但它依赖的 `package_release.sh`、`InstallerDistribution.xml.in`、根目录 `CMakeLists.txt` 在新树里都不存在，从没跑过；18 分钟后 8a697a1674 在「六平台独立发版」的整体重写里把它换成只打 tar.gz 的占位。09-24 的 #707 引入「设置应用加启动时安装输入法」的架构，DMG 跟着它一起来，全文没提 pkg；09-25 的 f0854fee72 修 secret 名时只接了 Application 身份和公证凭据，Installer 身份一直没接回来。找不到任何否决 pkg 的记录：pkg 是被重写连带丢掉的。签名材料一直在：`MACOS_DEVELOPER_ID_CERTIFICATE_BASE64` 里同时有 Application 和 Installer 两个身份（导入日志 2 identities imported），`MACOS_DEVELOPER_ID_INSTALLER` 自 2026-09-06 起没动过。

旧 pkg 装的是输入法本体，只开 `enable_currentUserHome`，那时没有设置应用。现在的架构是设置应用装在 `/Applications`、输入法由设置应用装进 `~/Library/Input Methods` 并由 Sparkle 静默更新，pkg 要按这个架构重新设计。

## Decision

从 0.57.0 起，每个版本（`shared/contracts/editions.json` 里带 macOS 标识的 6 个）发一个 localSystem 域的分发包 `<dmg_prefix>-X.Y.Z-universal.pkg`，载荷只有该版本的设置应用，装到 `/Applications`。输入法仍由设置应用装进当前用户目录，Sparkle 照旧用 zip 更新，`dmg_prefix` 这个字段名和值都不改（它冻结在 `editions.frozen.json` 里，Sparkle 更新包也用它命名）。已发的 0.56.x 及以前的版本不补 pkg、不重跑：`publish-release.sh` 会删掉发布页上不在新清单里的资产，旧 DMG 的外链会 404。

### 包的结构

- 组件包由 `pkgbuild` 打：`--ownership recommended`（应用归 root:wheel）、`--compression latest` 加 `--min-os-version 13.0`、`--identifier <settings_bundle_id>.pkg`（例如 `app.msime.macos.pkg`，定下就不再改，Homebrew 的 `uninstall pkgutil:` 按它找回执）、`--version` 取设置应用的 `CFBundleVersion`（等于输入法的构建号，跨版本单调）。`pkgbuild` 在 `package-release.sh` 现有的 `chmod -R u+rwX,go+rX,go-w` 和 `check_app` 之后运行：它按磁盘上的权限位归档，#10329 那类 0700/0600 的文件会原样进包。载荷根目录给 0775，与 `/Applications` 相同。
- 组件表先 `pkgbuild --analyze` 再改写：顶层只能是这个设置应用；它和每一层 `ChildBundles`（内嵌的输入法、Sparkle 的 Updater.app 和 XPC）都显式 `BundleIsRelocatable=false`；顶层 `BundleOverwriteAction=upgrade`、`BundleIsVersionChecked=true`。发布 runner 是 macos-15，`pkgbuild` 要到 macOS 27 才默认不可重定位。可重定位的组件会让 Installer 去升级别处同 bundle id 的副本：内嵌输入法与用户已装的输入法同 id，Sparkle 的子 bundle 与别家应用里的 Sparkle 同 id。
- 分发文件（`platforms/macos/resources/installer/InstallerDistribution.xml.in`）：`minSpecVersion=2`；`customize="never"`；`hostArchitectures="arm64,x86_64"`（缺 arm64 时 Apple 芯片在 Rosetta 下评估分发文件和运行脚本）；只开 `enable_localSystem`；`os-version min="13.0"`；不加 `RequireLogout`；`pkg-ref` 下 `must-close` 该版本的设置应用（写在顶层会被忽略）。标题和欢迎页、结束页有中文和英文两份，结束页取代 `安装说明.txt`：首次安装后注销重登，再到系统设置添加输入法。
- postinstall（`pkg-postinstall.sh`，按版本渲染应用名）只做一件事：`COMMAND_LINE_INSTALL` 非空（installer 命令行、Homebrew、经 installer 的 MDM 工具）就退出；控制台用户是 root、loginwindow、_mbsetupuser、空或 uid<501 就退出；确认 `"$2/<应用名>.app"` 是真实目录；`launchctl asuser <uid> /usr/bin/sudo -u <user> -H /usr/bin/open -n` 打开它。`-n` 避免 LaunchServices 激活输入法拉起的同 bundle id 面板进程，`-H` 让设置应用以用户自己的 HOME 运行、把输入法装进用户目录。所有分支 `exit 0`，`set -u`；不写任何输入法目录、不注册输入源、不结束进程、不改属主。
- 渲染、组件表改写和展开后的结构检查都在 `platforms/macos/scripts/installer_package.py`。

### 签名、公证和检查

- `release-macos.yml` 映射 `MACOS_INSTALLER_IDENTITY: secrets.MACOS_DEVELOPER_ID_INSTALLER`，导入 p12 时补 `-T /usr/bin/productbuild -T /usr/bin/productsign`，用不带 `-p codesigning` 的 `security find-identity -v` 核对 Installer 身份，并打印两张证书的 issuer 和 notAfter（Application 证书已核实挂在 G2 下、2031-08-16 到期；Installer 证书的中间证书没核实过，若是旧的那张，最晚 2027-02-01 到期）。
- `productbuild --sign … --keychain … --timestamp` 一步出签名的分发包，时间戳服务失败时仿照 `codesign_timestamped` 重试。有 Application 身份和公证凭据、却没有 Installer 身份时在编译之前失败；两者都没有时（fork、`ci-macos-package.yml`）出未签名的 pkg、跳过公证和 staple 并告警。
- 公证与 staple 的对象换成 pkg，沿用「6 个版本先全部提交、再统一取结果、逐个装订」的流程；`SHA256SUMS` 在 staple 之后算。
- `finish_edition` 对每个 pkg：签了名的跑 `pkgutil --check-signature`（断言 Developer ID Installer 和可信时间戳），公证过的跑 `spctl -a -t install`；`pkgutil --expand-full` 之后核对分发文件、`PackageInfo`（`install-location="/Applications"`、identifier、版本、`<relocate>` 为空、有 postinstall）、Bom（`lsbom` 的每一项都在 `./<应用名>.app` 下、归 root，AppleDouble 的 `._` 项按同级文件判断）、`Scripts` 里只有 postinstall，再对载荷里的应用跑 `check_app`。

### 渠道

- 发布页仍按[macOS 版本发布页只放 DMG](2026-10-20-macos-release-page-only-dmgs.md) 定下的标准：每个版本一个安装包加只列它们的 `SHA256SUMS`，开头是下载指引（保留 `<!-- msime-download-guide -->`）。那篇笔记管发布页上放什么，这篇只把安装包格式从 DMG 换成 pkg，两篇部分重叠、互相链接。
- 官网（msime-web）按文件名认包：完整版正好是 `msime-macos-X.Y.Z-universal.pkg`，其他版本 `msime-macos-<版本>-X.Y.Z-universal.pkg`，测试版的 tag 可以带 `-beta`、文件名不带；nightly 的 `…-universal-nightly.<运行>.<尝试>.pkg` 只计入下载统计。官网要看到第一个非预发布的 pkg 才切换，所以 0.57.0 必须发成正式版。
- Homebrew：6 个 cask 改用 `pkg` 段（brew 以 sudo 跑 installer，固定是 localSystem 域）；`binary` 写死 `/Applications/<应用名>.app/Contents/MacOS/msime-mcp`（pkg 不认 `--appdir`）；`uninstall pkgutil: "<settings_bundle_id>.pkg"`，`delete` 兜底删 `/Applications/<应用名>.app`（Sparkle 换过的 bundle 和回执对不上）和输入法；`auto_updates true`，升级交给 Sparkle。
- nightly：`PLATFORM_SUFFIXES["macos"]` 是 `.pkg`；清理旧附件时把退役的 `.dmg` 也算进去（仍被某个平台状态引用的照样保留），否则 nightly 上现有的 6 个 DMG 永远删不掉。

### 相关笔记

- [macOS 版本发布页只放 DMG](2026-10-20-macos-release-page-only-dmgs.md)：部分重叠。发布页的标准不变，安装包格式由这篇接管。
- [macOS nightly 用 Developer ID 签名并公证](2026-10-15-macos-nightly-notarized.md)、[发布打包提速](2026-10-20-macos-release-faster-packaging.md)、[nightly 只发原生安装文件](../simplification/2026-10-15-nightly-native-downloads.md)：部分重叠。签名跟着 secrets 走、先全部提交再统一取公证结果、nightly 只放原生安装文件这些决定都沿用，其中写的 DMG 现在是 pkg，挂载 DMG 的检查现在是展开 pkg。
- [首次安装要重新登录](../bug-fix/2026-10-09-macos-first-install-login-required.md)：结束页沿用它的结论，安装包不加 `RequireLogout`。
- [「所有用户」安装输入法](../bug-fix/2026-10-23-macos-system-input-method-install.md)：设置应用自己生成的系统安装包（identifier `<input_method_bundle_id>.system-install`）与这里的 `<settings_bundle_id>.pkg` 不冲突；关掉重定位的依据与它相同，那篇的合成测试只覆盖顶层组件，这里把每层子组件也显式关掉。

## Alternatives considered

- **postinstall 把应用 chown 给安装它的用户** — 最强的理由是和 DMG 时期一样：管理员手动更新设置应用时 Sparkle 不用再弹授权。不用：设置应用的 Sparkle 只在手动检查时运行（`settings_updater.py` 把自动检查和自动更新都设成 False），后台静默更新的是 `~/Library/Input Methods` 里的输入法，与 `/Applications` 的属主无关；chown 给一个人，多用户机器上其他人照样要授权，若安装者是借管理员密码装的普通用户，就成了所有人都会运行的应用对他一人可写；每次重装 pkg 又被 Installer 改回 root。root:wheel 是 pkg 安装应用的通行状态。
- **同时开 currentUserHome 域（「仅为我安装」，装到 `~/Applications`，不要管理员）** — 最强的理由是给没有管理员密码的普通用户留一条路，这正是相对 DMG 的主要退步；Homebrew 走 `-target /` 固定选 localSystem，多开这个域并不妨碍它。不用：安装位置要确定，postinstall、cask 的 `binary` 路径和 LaunchServices 找设置应用都只认 `/Applications`；currentUserHome 下脚本以谁的身份运行没有验证过；同行的 currentUserHome pkg 报过安装位置错乱。以后有普通用户的需求再单独评估。
- **不带 postinstall** — 最强的理由是安装包里没有任何以 root 运行的代码，审查面最小。不用：装完不打开设置应用，输入法就不会装上，这正是 DMG 时期「拖进去不打开」的老问题；postinstall 只做一次 `open -n`，范围很小。真机验证若发现弹窗时机或面板进程有问题，退回无脚本、只在结束页提示打开。
- **DMG 和 pkg 并存** — 最强的理由是保住普通用户的安装路径，官网和老用户也不用一次切换。不用：维护者要求只发一种格式，发布页的标准是每个版本一个安装包；两种格式并存时下载指引、官网的格式判断、cask 和 nightly 都要维护两套，用户也要多做一次选择。

## Consequences

- **收益**：安装是标准的「双击、按提示、装完自动打开」，不再有「拖进去没打开」的半安装；结束页把首次安装要注销重登写在用户一定会看到的地方。Homebrew 卸载有回执可查。MDM 和命令行部署有了签名的分发包，安装时不弹窗。
- **代价**：
  - 每次安装都要管理员密码，管理员账户拖 DMG 时不用。没有管理员密码的普通用户装不了：DMG 时期他们可以把应用放在 `~/Applications` 或下载目录里打开，输入法照样装进自己的用户目录。
  - 应用归 root:wheel，管理员手动更新设置应用时 Sparkle 要授权，普通用户更新不了设置应用（DMG 时期普通用户本来也写不了 `/Applications`）。这条建立在 Sparkle 2.9.6 能经授权替换 root 拥有的 bundle 上，只读过源码，没有实测，macOS 27 上若走不通要重新评估 chown。
  - Homebrew 安装、升级、卸载都要 sudo；老的 app 型 cask 第一次升级时会先删掉输入法，之后要用户自己打开一次设置应用（brew 会不会重开被 quit 的应用没核实）。
  - 留在 `~/Applications` 等别处的旧设置应用不会被替换，LaunchServices 可能挑中它，设置页和已装输入法的版本对不上（输入法本身不会被降级）。
  - 装比已装版本旧的 pkg 时 Installer 显示成功、实际跳过应用（`BundleIsVersionChecked`）。
  - 日文、越南文、藏文版的安装器文案只有中英文两份。
  - 未签名的 pkg 比 DMG 小（见下面的验证），公证耗时没有实测，第一次 nightly 的摘要里会有。

## Verification

- 契约测试：`scripts/test-macos-package-resources.py` 核对安装包文件名（官网约定的形状）、`pkgbuild` 参数、`pkgbuild` 在 chmod 和 `check_app` 之后、缺 Installer 身份时在编译前失败、`productbuild` 的时间戳重试，并对 6 个版本渲染分发文件、postinstall 和欢迎/结束页，拼出与 `pkgutil --expand-full` 相同布局的合成安装包跑 `installer_package.py check`，再逐项改坏（可重定位、安装位置、identifier、缺 postinstall、Bom 越界或不归 root、开了 currentUserHome、customize、RequireLogout、must-close 写错或写在顶层、多一个脚本）确认都被拦下；postinstall 在命令行安装、没有参数、没有应用、应用是符号链接时都以 0 退出且不打开任何东西。`test-macos-editions.py` 对每个版本断言 cask 的 pkg 文件名、pkgutil 回执、`/Applications` 下本版本的应用，且不残留 full 的名字。`test-nightly-partial-publishing.py` 覆盖 pkg 的 nightly 文件名和退役 DMG 的清理。
- 在 Mac Studio 上用无签名路径真打了 0.56.1 的 full 和 wubi 两个 universal pkg，`finish_edition` 的展开检查和载荷上的 `check_app` 都通过。`PackageInfo` 是 `install-location="/Applications"`、`relocatable="false"`、`<relocate/>` 为空，列出设置应用、内嵌输入法、Sparkle.framework、Updater.app 和两个 XPC 共 6 个 bundle；Bom 的 655 项除 `.`（40775）外都在 `./MSIME.app` 下、归 uid 0，其中约一半是构建机给文件加的 `com.apple.provenance` 对应的 AppleDouble 项。体积：full 121,685,972 字节、五笔 144,792,438 字节，比 0.56.0 的 DMG（142 MB、166 MB）小，签名和公证之后会略大一点。
- 真机安装（需要管理员密码和图形会话）由维护者按 PR 正文的清单在 macOS 13、15、26、27（含一台 Intel）上做，合并前完成。

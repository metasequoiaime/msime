# Agent Note: macOS 输入法本体经 Sparkle 自己更新

Status: implemented

## Problem

macOS 的升级要用户自己做三步：在设置页或官网看到新版本，下载新的 DMG，把应用拖进「应用程序」替换旧的，再打开一次设置应用，它启动时才把内嵌的新输入法装进 `~/Library/Input Methods`。替换了却没打开，输入法就一直停在旧版本，和首次安装「拖进去不打开就等于没装」是同一个缺口。

`platforms/macos` 的输入法早就编进了 Sparkle 2.9.6 和 `UpdateController`，宿主有 `SUFeedURL` 就走 Sparkle（`MSIMEUpdateRouteForHost`），但发布包从来没有配置 feed、公钥和 appcast，于是这条路径从未打开过，菜单里的「检查更新…」只能把人带到下载页。

打开它有一个硬约束：Sparkle 的安装进程（Autoupdate 的 `SUPlainInstaller performInitialInstallation:`）用标准比较器比新旧 bundle 的 `CFBundleVersion`，新的更小就以 `SUDowngradeError` 拒绝安装；这一步在独立进程里，不经过输入法设置的任何委托或比较器。而 `CFBundleVersion` 原来是 `build-number.txt`，每个版本从 1 重新开始，0.53.0 的构建 3 升到 0.54.0 的构建 1 会被当成降级。

设置应用的更新范围由[完整设置应用在应用内更新](2026-10-18-macos-settings-in-app-updates.md)扩展；下文的输入法独立更新机制继续保留。

## Decision

Sparkle 更新的是输入法本体，也就是 `~/Library/Input Methods` 里那份 bundle；`/Applications` 里的设置应用不在它的范围内，仍随 DMG 更新。

### 版本号

`CFBundleVersion` 由 `platforms/macos/CMakeLists.txt` 算出：`((主 × 1000 + 次) × 1000 + 修订) × 1000 + build-number.txt`，0.54.0 的构建 1 是 `54000001`。它跨版本单调递增，满足 Sparkle 的降级检查；仍是一个整数，因为设置应用把构建号按 `u64` 解析（`macos_input_source.rs` 的 `BundleVersion`），带点的写法会让它读不出版本、把已装的输入法当成可以覆盖。次版本号、修订号和构建号必须小于 1000，CMake 在配置时检查。`build-number.txt` 的含义不变，每个新版本仍从 1 开始。已经装在用户机器上的旧版本构建号是个位数，比任何新值都小。

### 只有发布包带 feed

- `CMakeLists.txt` 的缓存变量 `MSIME_SPARKLE_APPCAST_BASE_URL` 非空时，`Info.plist.in` 的 `${MSIME_SPARKLE_PLIST_KEYS}` 展开成 `SUFeedURL`（`<base>appcast-<版本 id>.xml`）、`SUPublicEDKey`（`platforms/macos/sparkle-public-key.txt`）、`SURequireSignedFeed`、`SUVerifyUpdateBeforeExtraction`、`SUEnableAutomaticChecks`、`SUAutomaticallyUpdate` 和 `SUScheduledCheckInterval`（86400 秒）。前两项安全设置与 MSIME-Apple 时期发出去的 0.48 版相同：appcast 本身要带签名，更新包解压之前先验签。为空时一个键都不写，开发构建和 `scripts/install.sh` 装的输入法不会被正式版覆盖。
- 密钥的选择已由[换一对有备份的 Sparkle 密钥](../process/2026-10-15-macos-sparkle-key-rotation.md)取代，下面一句保留当时的依据。密钥沿用 MSIME-Apple 时期的那一对：公钥 `rSAufajnup+T+d+I4LTs4EAhe5M8bwHemWDKao3CB/E=` 随 macos-v0.48.6 的构建发出去过（当时的 feed 是 `MSIME-Apple/releases/latest/download/appcast.xml`），私钥是仓库 secret `SPARKLE_ED_PRIVATE_KEY`（2026-09-04）。那批旧安装只认这把公钥，换一对新的就永远没法给它们签更新。
- `package-release.sh` 只在拿到 `MSIME_SPARKLE_PRIVATE_KEY`（workflow 从 `SPARKLE_ED_PRIVATE_KEY` 传入）且有 Developer ID 身份时传入地址（默认 `https://github.com/metasequoiaime/msime/releases/download/macos-appcast/`）。Sparkle 要求新旧 bundle 出自同一签名身份，ad-hoc 包无从更新，所以没有身份时不开 feed。编译之前它从私钥得出公钥（32 字节种子用 OpenSSL 3 推出，Sparkle 旧的 96 字节格式直接取后 32 字节），与 `sparkle-public-key.txt` 比对，不是一对就失败：对不上的话每个更新包都会被当作签名无效拒绝，而发版本身照常成功。
- 发布包只编 full，`edition_bundle.py` 把 bundle 改成其他版本时把 `appcast-full.xml` 换成 `appcast-<版本 id>.xml`。

### 更新包与 appcast

更新包的存放位置已由[macOS 版本发布页只放 DMG](../process/2026-10-20-macos-release-page-only-dmgs.md)取代：更新包与 appcast 一起放在 `macos-appcast` 上，不再挂在 `macos-vVERSION` 上、也不写进 SHA256SUMS。下面两段保留当时的做法。

每个版本在签名之后把输入法 bundle 打成 `<dmg_prefix>-VERSION-input-method.zip`（随 DMG 挂在 `macos-vVERSION` 上、写进 SHA256SUMS），用 Sparkle 的 `generate_appcast --ed-key-file -` 生成并签名 `appcast-<版本 id>.xml`，私钥只经标准输入传入。目录里只放这一个 zip，appcast 只有这一版、不生成增量包。打包脚本核对 appcast 指向这个 zip、`<sparkle:version>` 等于 bundle 的 `CFBundleVersion`、zip 带 `sparkle:edSignature`、appcast 自己带 `sparkle-signatures` 签名。

`release-macos.yml` 只在 main 上的正式版发布之后，把 appcast 上传到滚动的 `macos-appcast` release（预发布、`--latest=false`，不存在时创建）。上传前确认 zip 已经在 `macos-vVERSION` 上，已发布的 appcast 构建号更高时停下，免得重跑旧版本的发布把它退回去。beta、alpha 和 [nightly](../../implemented/process/2026-10-15-macos-nightly-notarized.md) 的输入法同样带 feed，但不更新 appcast，它们装上之后跟随正式版。

### 在输入法进程里运行

- `input_method_main.mm` 在带 feed 时启动时就创建更新控制器，Sparkle 才会按计划在后台检查。
- 输入法是 `LSBackgroundOnly`，Sparkle 的提醒窗口会从正在打字的应用那里抢走焦点。`SUAutomaticallyUpdate` 让更新静默下载；`UpdateController.mm` 的 `updater:willInstallUpdateOnQuit:immediateInstallationBlock:` 返回 `YES` 接管安装，若返回 `NO`，过了 `SUScheduledImpatientCheckInterval` Sparkle 会自己弹窗。安装在用户切到别的输入源时进行（`switchedAway` → `installDownloadedUpdateIfReady`），这一刻没有人在用水杉打字；应用退出时 Sparkle 也会装。
- `updaterShouldRelaunchApplication:` 返回 `NO`：直接启动输入法 bundle 是「独立设置窗口」那条启动路径。系统在下次需要输入法时从新 bundle 拉起它。
- 菜单与悬浮工具栏的「检查更新…」在带 feed 时直接交给 Sparkle，显示它自己的界面；没有 feed 时仍先打开设置应用的「关于」页。

## Alternatives considered

- **给 Tauri 设置应用加 `tauri-plugin-updater`，更新整个 `MSIME.app`** — 最强的理由是发布单位就是 `MSIME.app`，更新它之后设置应用启动时会把内嵌的输入法一并换掉，两边版本始终一致；更新逻辑还能和 [Rust 里的检查更新](../architecture/2026-10-09-update-check-in-rust.md)放在同一层。没有采用：用户要的是 `platforms/macos` 原生输入法自己的更新，Sparkle 和它的接线已经在那里；而设置应用落后于输入法是现有设计本就允许的方向（设置应用从不降级已装的输入法，新输入法读旧设置应用写下的配置）。
- **在输入法里给 Sparkle 提供先比短版本号再比构建号的比较器，`CFBundleVersion` 不动** — 最强的理由是不改任何人已经习惯的构建号，`CMakeLists.txt` 原来的注释也是这样设想的。否定：比较器只影响查 appcast 时选哪一项，安装进程的降级检查固定用标准比较器，跨版本升级仍会在最后一步被拒。
- **`CFBundleVersion` 写成 `<短版本号>.<构建号>`，如 `0.54.0.1`** — 最强的理由是可读，Sparkle 的标准比较器也能正确逐段比较。否定：设置应用按 `u64` 解析构建号，读不出时会把已装的输入法当成可以覆盖；四段也超出 Apple 对 `CFBundleVersion` 的格式要求。
- **让 Sparkle 在后台检查到更新时照常弹出提醒** — 最强的理由是这是 Sparkle 的默认体验，用户马上知道有新版本。否定：提醒窗口出自输入法进程，会抢走用户正在打字的应用的焦点。
- **为这次接入新生成一对密钥** — 最强的理由是私钥在本机钥匙串里有可见的副本，能当场验证配对，不用等发版。否定：0.48 版已经带着旧公钥发出去，换钥匙就等于宣布那批安装永远收不到 Sparkle 更新，而且同一个产品会有两把更新钥匙要保管。这次确实先生成过一对（钥匙串账户 `msime-sparkle`），发现旧 secret 后已删除，没有用于任何发布。
- **把 appcast 放在官网（msime-web）** — 最强的理由是官网地址稳定，也便于将来做国内镜像。没有采用：那个仓库不在这里，发布流程没法原子地同时更新它；GitHub 的固定 tag 地址同样稳定，由同一个 workflow 维护。

## Consequences

- **收益**：装过一次之后，输入法本体自己升级，不再依赖用户下载 DMG 并记得打开一次；`CFBundleVersion` 跨版本单调递增，Sparkle 的降级检查和设置应用的版本比较都成立。
- **代价与已知上限**：
  - 2026-10-15 时维护者的登录钥匙串里没有这把私钥，能确认的副本只有仓库 secret（secret 读不出来）。secret 丢了或被覆盖，已经发出去的输入法再也收不到更新，只能靠下一次手动装 DMG 换上新公钥；需要维护者自己找到并另存一份备份。
  - secret 与公钥是否配对，第一次带 secret 打包时才知道：不配对时打包在编译之前失败，不会发出收不到更新的包。
  - 0.48 那批旧安装的 feed 指向 MSIME-Apple 的 `releases/latest`，仓库合并之后那里不是 macOS 的 appcast，它们暂时收不到这条路径的更新。
  - 设置应用不随 Sparkle 更新，会落后于输入法，直到用户装下一个 DMG；设置页上的新功能要等那时才出现。
  - 静默更新只在用户切换输入源或输入法进程正常退出时安装。从不切换输入源、也不注销的用户，要等手动「检查更新…」或下一次注销。如果这类用户多，可以考虑在输入法长时间闲置时安装。
  - 更新包从 GitHub 下载，国内可能很慢；dl.msime.app 的放行名单与 Sparkle 是否经它下载，见[macOS 版本发布页只放 DMG](../process/2026-10-20-macos-release-page-only-dmgs.md)。
  - 每个版本多一个十几 MB 的 zip（现在放在 `macos-appcast`，不在版本发布页上）。
  - 第一个带 feed 的版本发出之后，要再发一个版本才能在真实环境里验证一次完整的升级。

## Verification

- `python3 scripts/test-macos-editions.py`：六个版本改写后的 `SUFeedURL` 各自指向 `appcast-<版本 id>.xml`，其余 Sparkle 键保留；改写代码撤回时日文、越南文和藏文版仍指向 `appcast-full.xml`，检查失败。
- `ctest -R update-controller`：后台安装转发给驱动且不激活进程，没有后台安装能力的驱动不受影响。
- 以 `-DMSIME_SPARKLE_APPCAST_BASE_URL` 配置的本地构建，0.53.0 构建 1 的 `CFBundleVersion` 是 `53000001`，Info.plist 带全部 Sparkle 键；不带该变量时一个都没有。
- `package-release.sh` 的公钥核对：32 字节种子推出的公钥与 `generate_keys` 打印的一致，96 字节旧格式取出的公钥正确，其他长度报错。
- `generate_appcast` 产出的 appcast 带 `sparkle-signatures`，`sign_update --verify` 对 feed 和 zip 都通过，改动一个字节的 zip 被拒。

## 系统安装范围扩展

2026-10-23 的[系统输入法安装支持](../bug-fix/2026-10-23-macos-system-input-method-install.md)让设置应用同时识别用户和系统目录，沿用现有安装位置修复并保留版本防降级。单独的系统副本是正常安装；仅主动安装或修复才请求管理员权限。首次安装的登录提示和输入法自身的 Sparkle 更新协议保持原有约定。

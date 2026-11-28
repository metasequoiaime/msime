# Agent Note: macOS 安装包直接装好输入法，不再弹设置应用的安装窗口

Status: implemented

## Problem

维护者报告：macOS 的 pkg 装完之后，又弹出一个自制的安装窗口（设置应用的「立即安装」页），点了之后再要一次管理员密码。

原因是两层设计叠在了一起。[只发 pkg](../process/2026-11-02-macos-pkg-only.md) 定下的 postinstall 只做一件事：以登录用户身份 `open -n` 设置应用，由它把输入法装进 `~/Library/Input Methods`，那时不需要权限。[只装系统目录](2026-11-16-macos-system-directory-install-and-updater.md)（#11492）之后，输入法只装 `/Library/Input Methods`，设置应用的首次安装改成自己用 `pkgbuild` 打一个系统组件包、经 `osascript … with administrator privileges` 运行 `installer`（`macos_input_source.rs` 的 `build_system_package`、`SYSTEM_INSTALL_SCRIPT`、`install_system_bundle`）。于是一次安装要两次授权、两个安装界面，而 postinstall 本来就以 root 运行，第二次授权毫无必要。

另外，命令行、Homebrew 和 MDM 安装（`COMMAND_LINE_INSTALL`）的 postinstall 什么都不做，这些安装从来没装过输入法，要用户自己打开设置应用再授权一次。

## Decision

安装包自己装好输入法，整个安装只有 Installer 的一次管理员授权。postinstall（`platforms/macos/resources/installer/pkg-postinstall.sh`，`installer_package.py render` 按版本填入设置应用名、输入法 bundle 名、bundle id 和可执行文件名）按顺序做三件事，所有分支都以 0 退出，原因写进 `/var/log/install.log`：

1. **输入法（root，不分图形或命令行）。** 源是 `"$2/<设置应用>/Contents/Resources/<输入法>.app"`，不是真实目录或是符号链接就不装。登录用户（有正常登录用户时）的 `~/Library/Input Methods` 里已经有一份时不碰系统目录：用户副本优先，与设置应用的 `installed_bundle_path_in` 相同，那份由设置应用更新。否则装进 `/Library/Input Methods`：在同一目录里建 0700 的隐藏暂存目录（`.<输入法>.app.pkg-staging-<pid>`，不以 `.app` 结尾），`ditto` 进去，`chown -R root:wheel`，目录 0755、文件 0644、带执行位的 0755（与 `make_readable_by_everyone` 和更新助手一致），核对可执行文件和 `CFBundleIdentifier`、`codesign --verify --deep --strict`，全部通过后把旧的改名挪开、新的改名进来、删掉旧的；任何一步失败都清掉暂存，挪开过旧的就挪回来。已装的一份构建号（`CFBundleVersion`）更大，或者相同且随附的不带修订或修订相同（`MSIMEInputSourceRevision`），并且校验通过时不动它，与设置应用的 `refresh_decision` 一致、从不降级；校验不过的一律替换。构建号按长度再按字典序比较，不经过 shell 的 64 位整数。`$3` 不是 `/`（装到别的卷）时什么都不做。
2. **更新助手（root）。** 系统目录里有可用的输入法时，从 `Contents/MacOS/msime-system-updater` 和 `Contents/Resources/app.msime.system-updater.plist`（`packaged_system_updater` 读的同两个文件）装到 `/Library/PrivilegedHelperTools/app.msime.system-updater`（755）和 `/Library/LaunchDaemons/app.msime.system-updater.plist`（644），收件目录 1777，`launchctl bootout` 后 `bootstrap`、失败隔两秒再试一次，与 `SYSTEM_INSTALL_SCRIPT` 逐项相同。用户副本优先、系统目录没有输入法时不装：助手从不新建安装，装上也无事可做。
3. **登录用户这一侧（只在图形安装、有正常登录用户时）。** 沿用原来的判断：控制台用户不是 root、loginwindow、_mbsetupuser，uid 不小于 501。以 `launchctl asuser <uid> sudo -u <user> -H` 运行：`lsregister -f` 已装的 bundle、跑它的 `--register-input-source`；刚换过 bundle 时再停掉该用户正在运行的旧输入法进程，并像 `refresh_system_input_source_lists` 那样退出系统设置和键盘设置扩展、清掉「添加」对话框的输入源缓存、结束两个切换器进程。顺序与设置应用相同：先登记再停进程。最后 `open -n` 设置应用。

**装完仍然打开设置应用。** 它不是为了安装，而是为了准备用户数据：输入法建会话靠 `~/Library/Application Support/app.msime.macos/runtime-options.json`，这份文件只由设置应用第一次启动时从内嵌的 `EngineResources` 准备（`platforms/macos/README.md` 的「词库准备与 Tauri 设置应用」），没有它输入法把每个键原样交给应用。输入法装好并校验通过时 `first_install_pending_for` 为假，设置应用直接进设置页，启动检查报「已是最新」。输入法没装上（源损坏、签名校验不过、目录异常）时打开的就是原来的首次安装窗口，它是兜底，代码不删。用户副本优先时打开它，由它的启动检查去更新那份副本。

命令行安装只做前两步，不登记输入源、不停进程、不打开设置应用：这类安装的控制台用户不一定是发起安装的人（MDM、SSH）。全新的机器下次登录时系统列出 `/Library/Input Methods` 里的输入法；词库要等用户第一次打开设置应用时准备，cask 的提示写明这一点。

设置应用里的首次安装窗口、`build_system_package`、`SYSTEM_INSTALL_SCRIPT`、`install_system_bundle` 都保留，用于兜底、「修复输入源」和「迁移到系统目录」。

### 契约测试

`scripts/test-macos-package-resources.py` 的 `check_postinstall` 对每个版本做静态检查（四个名字、`set -u`、所有 `exit` 都是 0、用户判断、`as_user`、`open -n`、bash 语法，装了 shellcheck 时跑它），`check_updater_install_matches_app` 逐项比对 postinstall 与 `SYSTEM_INSTALL_SCRIPT` 和 `SYSTEM_UPDATER_*` 常量的路径、权限和载入命令，任何一边改了另一边不跟着改就失败。`run_postinstall_cases` 在完整版和五笔版上真跑渲染出的脚本：把 `stat`、`id`、`dscl`、`chown`、`codesign`、`launchctl`、`lsregister` 换成记账的桩，把 `/Library` 下的路径换到临时目录（被替换的每一处都必须原样出现在脚本里），`ditto`、`find`、`chmod`、`mv`、`plutil` 是真的。场景有：全新安装（内容、权限、框架链接、无暂存残留、root 属主、签名校验、助手的权限和载入、用户一侧的顺序和身份）、升级、同构建号不动、同构建号新修订替换、已装更新不降级、已装更新但损坏替换、暂存校验不过保留旧版并打开设置应用、用户副本优先、命令行安装、登录窗口、没带助手、没有内嵌输入法或它是链接、没有参数、装到别的卷。

## Historical audit

- [只发 pkg](../process/2026-11-02-macos-pkg-only.md)：部分取代。postinstall「只打开设置应用，不写任何输入法目录、不注册输入源、不结束进程、不改属主」那一条由这篇接管；载荷、组件表、分发文件、签名公证、渠道都不变。那篇在相关笔记里链回这里。
- [只装系统目录，由 root 更新助手静默替换](2026-11-16-macos-system-directory-install-and-updater.md)：部分重叠。「安装包自己不往 `/Library/LaunchDaemons` 和 `/Library/PrivilegedHelperTools` 写东西」不再成立，助手的契约、安装位置和设置应用的授权安装都不变。那篇在相关笔记里链回这里。
- [首次安装要重新登录](2026-10-09-macos-first-install-login-required.md)：仍然成立，结束页照旧写明首次安装后注销重登。
- [首次安装窗口覆盖损坏的 bundle](2026-10-26-macos-first-install-corrupt-bundle.md)：仍然成立，那条路径现在是兜底。

## Alternatives considered

- **把输入法作为安装包的第二个组件放进载荷，直接装到 `/Library/Input Methods`** — 最强的理由是全交给 Installer：原子替换、`BundleIsVersionChecked`、回执和 `pkgutil --files` 都是现成的，postinstall 里没有任何写文件的代码。不用：输入法已经内嵌在设置应用里（设置应用的启动检查、修复和迁移都从那份复制），再放一份进载荷，每个安装包多出约 120 MB（完整版 121 MB 的包会翻倍，五笔版更大）；载荷里两个同 bundle id 的输入法副本，一个在 `/Applications/<应用>.app/Contents/Resources`，一个在 `/Library/Input Methods`，组件表的重定位只要有一层没关就会被 Installer 拿去升级另一处，用户副本优先的规则也没法在组件包里表达（Installer 不知道控制台用户的家目录里有没有一份）。
- **保留设置应用驱动的安装，只是让 postinstall 等它装完** — 最强的理由是只有一套安装代码，postinstall 继续保持「只打开应用」的最小审查面。不用：设置应用以用户身份运行，写不了系统目录，只能再要一次授权，这正是被报告的问题；命令行安装也仍然装不上输入法。
- **postinstall 以 root 调用设置应用里现成的安装逻辑（例如给设置应用加一个 `--install-input-method` 命令行入口）** — 最强的理由是复制、校验、交换只写一遍（Rust 里已经有带回滚的实现）。不用：要以 root 运行整个 Tauri 设置应用的可执行文件，它启动时会读写 HOME 下的应用数据、初始化 WebView 相关的状态，root 下的 HOME 是 `/var/root`；为了这一步再拆出一个独立的 root 可执行文件，审查面比一段几十行的 shell 大。shell 版本与 Rust 的共同约束（路径、权限、载入方式）由契约测试锁住。
- **装完不打开设置应用，只靠结束页** — 最强的理由是安装结束得干净，也避开 `open -n` 与输入法面板进程同 bundle id 的老问题。不用：不打开就没有 `runtime-options.json`，用户按结束页添加输入法后打不出中文，只看到恢复提示。

## Consequences

- **收益**：图形安装只要一次管理员密码，装完不再出现安装窗口；升级时旧输入法进程被停掉、输入源缓存被刷新，新增的模式不用重新登录就能添加。命令行、Homebrew 和 MDM 安装第一次真的装上了输入法和更新助手，此后的更新由助手静默完成。
- **代价与已知上限**：
  - postinstall 里有一段以 root 改系统目录的 shell，和 Rust 的同类代码是两份实现；契约测试只比对两边的路径、权限和载入命令，比对不到复制和校验的细节。
  - 装进系统目录的输入法不在任何回执里（`pkgutil --files app.msime.macos.pkg` 列不出它），卸载靠 cask 的 `delete` 和设置应用自己的卸载。
  - 命令行安装不登记输入源：升级时旧输入法进程继续跑旧代码，直到它退出或用户注销；`lsregister -f` 不做，设置应用 `refresh_launch_services` 记过的那种 LaunchServices 记录指着旧副本的情况可能出现，届时要注销重登或在设置页点「修复输入源」。词库要等用户第一次打开设置应用。
  - 多用户的 Mac 上只停当前登录用户的旧进程，别的已登录用户的输入法继续跑旧代码，与设置应用的 `stop_running_copies` 相同。
  - 已经装在用户目录的老安装，安装包不迁移也不更新系统目录，仍由设置应用的启动检查更新那份副本，「迁移到系统目录」照旧要再授权一次。
  - 真实的 Installer 运行（root、图形会话、`launchctl asuser`、真实签名）没有在这次改动里跑过，只有合成测试；合并前由维护者按 PR 正文的真机验收清单验收。

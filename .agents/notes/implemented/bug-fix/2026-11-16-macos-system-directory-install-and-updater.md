# Agent Note: macOS 输入法只装系统目录，由 root 更新助手静默替换

Status: implemented

## Problem

输入法的各个模式（水杉双拼、水杉五笔等）会在系统输入法菜单里变灰、选不了，正在用时还会被切回 ABC，而同一台机器上别的第三方输入法照常可选。

macOS 维护一份「安全输入源」名单，任何进程开着安全输入（密码框、终端的安全键盘输入，或某个应用失焦后没有释放）时，只有名单里的输入源可选。对不随系统自带的输入法，HIToolbox 的 `islGetInputSourceIsSecureCandidate` 只看一件事：bundle 路径是否以 `/Library/Input Methods/` 开头（macOS 27.2 上反汇编确认）。装在 `~/Library/Input Methods` 的那份 `TSMInputSourcePropertyIsSecure` 为 0，不进 `TISCreateSecureKeyboardInputSourceList`。同机对照：系统自带的日文和 ABC、两个装在系统目录的第三方输入法都是 1；水杉和另一个装在用户目录的第三方输入法都是 0。

首次安装默认装给当前用户，所以默认安装恰好落在会被停用的位置。

## Decision

输入法只装进 `/Library/Input Methods`。首次安装窗口不提供安装范围的选择；两处都没有安装时，安装位置是系统目录。已有的用户副本仍然优先，不自动搬走。这一条取代了[同时识别用户与系统安装](2026-10-23-macos-system-input-method-install.md)里「没有安装时保留当前用户默认值，首次安装页允许明确选择所有用户」的决定，改变安装默认值由维护者在处理这个缺陷时直接要求。

系统目录归 root、权限 755，以登录用户身份运行的输入法和设置应用换不了自己。Sparkle 2.9.6 在这种位置上的静默安装以 `SUInstallationAuthorizeLaterError` 放弃，之后改成弹标准更新窗口加密码框。所以新增唯一的特权部件 `msime-system-updater`（`crates/macos-system-updater`），由 launchd 以 root 运行。

### 更新助手的契约

- 位置：程序在 `/Library/PrivilegedHelperTools/app.msime.system-updater`，任务在 `/Library/LaunchDaemons/app.msime.system-updater.plist`，标签 `app.msime.system-updater`。收件目录 `/Library/Application Support/app.msime.system-updater/inbox` 权限 1777，工作目录 `work` 权限 0700。
- 触发：`QueueDirectories` 指向收件目录，目录非空时 launchd 启动助手。助手不接收参数，不读环境。
- 提交：提交方 must 先用点开头的名字把 bundle 拷进收件目录，拷完再改名。助手 never 处理点开头的条目；修改时间离现在超过 15 分钟的点开头条目按丢弃处理。
- 判断只针对工作目录里的拷贝：bundle 标识 must 是版本表 `shared/contracts/editions.json` 里的输入法标识；系统目录里 must 已有同标识的安装，助手 never 新建安装；拷贝 must 满足从已装副本推导的代码要求（同标识、同 Developer ID 团队）；构建号 must 严格大于已装的。
- 替换：先改名到目标旁边，再用 `renameat_with(EXCHANGE)` 原子交换，然后删掉旧的。
- 无论结果如何，提交都从收件目录移走，否则 launchd 会反复启动助手。

### 安装和更新怎么用它

- 管理员授权的系统安装（`SYSTEM_INSTALL_SCRIPT`）在同一次授权里装上助手和任务并载入。助手装不上不算安装失败。
- 设置应用的启动检查发现系统副本旧了时调用 `update_through_system_updater`：提交随附的 bundle，等已装副本的构建号变成它；换好后校验、登记、停旧进程、刷新输入源列表。助手没装、构建号不更大、提交被拒或超时，都报 `authorization_required`，并收回自己的提交。
- 助手和任务文件随设置应用分发（`Contents/MacOS/msime-system-updater`、`Contents/Resources/app.msime.system-updater.plist`），安装包自己不往 `/Library/LaunchDaemons` 和 `/Library/PrivilegedHelperTools` 写东西。
- 输入法自己的更新（`platforms/macos/src/core/SystemUpdateStaging.h`）：装在系统目录时 Sparkle 的 `hostBundle` 是用户缓存目录里的暂存副本 `~/Library/Caches/<标识>/system-update/<名字>.app`，`applicationBundle` 也是它。每次触发只做磁盘状态允许的一步（`MSIMESystemUpdateNextStep`）：让 Sparkle 把下好的更新装进暂存副本，把变新的暂存副本提交给助手，系统目录里的那份变新之后退出本进程。退出只发生在原来装更新的那些时刻（切走输入源、熄屏、闲置），同一个构建只提交一次。
- 助手自己的版本记在任务文件的 `EnvironmentVariables.MSIME_SYSTEM_UPDATER_VERSION` 里。改了助手的行为 must 把它加一；设置应用启动时发现系统安装的助手没装或版本更小，报 `authorization_required`，「修复」会在授权安装里重装助手。
- 迁移：`migrate_to_system` 经一次管理员授权装好系统副本，校验通过后才停掉并删除用户副本，再登记一次。用户副本比随附的新而且完好时装的是用户副本那一版。授权取消或安装失败时用户副本原样留着。

## Historical audit

- [同时识别用户与系统安装](2026-10-23-macos-system-input-method-install.md) 部分取代：默认位置和范围选择改了；系统组件包、授权脚本、权限归一、用户副本优先和重复安装提示都保留。它否掉的「一律安装到系统目录」现在被采用，当时的反对理由（改变默认值、首次安装都要管理员）仍然成立，只是输给了安全输入下不可用。它的「启动检查只提示、不弹授权」保留：启动检查仍然不弹授权，只是多了一条不需要授权的路。
- [输入法 Sparkle 更新](../feature/2026-10-15-macos-sparkle-input-method-updates.md) 部分重叠：单调构建号和禁止降级被助手沿用。装在用户目录时不变；装在系统目录时 Sparkle 改为更新暂存副本，替换交给助手。
- [只发 pkg](../process/2026-11-02-macos-pkg-only.md) 部分重叠：安装包的载荷仍然只有设置应用，postinstall 仍然只打开它。

## Alternatives considered

- **把系统目录里的 bundle 设成管理员组可写。** 不需要任何特权部件，更新可以由用户进程原地完成，同机另一个第三方输入法的 bundle 是 775。但这样任何以该用户身份运行的程序都能替换一个在密码框里可用的输入法，等于拆掉系统靠这个目录提供的保护。
- **每次更新弹管理员授权。** 实现最简单，Sparkle 自带这条路。但输入法是后台进程，授权窗口会从正在打字的应用那里抢走焦点，发版又很勤，会有人长期不更新。
- **SMAppService 的 daemon。** 系统推荐的做法，任务随 bundle 走。但它要求用户到系统设置里批准登录项，任务文件必须在调用方的 app bundle 里，而安装已经有一次管理员授权可以直接装任务。
- **在源码里写死团队标识。** 判断更直白。但仓库是开源的，别人用自己的证书构建时助手会拒绝他们自己的更新；从已装副本推导团队，信任的是管理员装上去的那一份。
- **保留安装范围的选择，只改默认值。** 给不想输管理员密码的人留一条路。但那条路通向一份安全输入下必然变灰的安装，选项本身就是缺陷。

## Consequences

收益：默认安装在安全输入期间可选；系统副本可以不经授权更新；助手的每个拒绝条件都有单元测试，签名要求用两份真实的 Developer ID 签名 bundle 核对过（同团队通过，换团队、换产品被拒）。

代价和缺口：

- 首次安装要管理员密码，没有管理员权限的用户装不了。
- 多了一个 root 进程，它的输入是任何本地用户都能写的目录。它的安全性取决于「只判断 root 专有目录里的拷贝」和代码要求这两点，改这个 crate 的人必须守住。
- 助手自己只在管理员授权的安装里更新。修了助手的缺陷之后，已装用户要点一次「修复」并输密码才拿到，靠任务文件里的版本号提醒；忘了加版本号就没有提醒。
- 已经装在用户目录的安装不自动迁移，要用户在「维护与诊断 › 输入法服务」里点「迁移」。启动检查对这种安装报 `migration_recommended`（安装、更新和当前版本都改报它），设置页顶部和 `authorization_required` 一样给出提示，指向「迁移」（#11464）；修复、迁移和首次安装窗口的结果仍报实际做了什么。用户目录里留着损坏的 bundle 时，首次安装仍沿用那个位置。
- 系统目录的安装多占一份 bundle 大小的磁盘（现在约 220 MB）放暂存副本。
- 输入法自己的更新链只验证了纯逻辑：步骤判断、暂存副本的准备和提交有测试，Sparkle 以暂存副本为更新对象、助手替换、进程退出后换新这一整条没有在真机上运行过。
- launchd 载入、`QueueDirectories` 触发和真实的 root 替换没有在真机上运行过，需要用 Developer ID 签名的构建单独验收。

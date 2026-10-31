# Agent Note: 词库更新回收用不上的旧版本

Status: implemented

## Problem

[词库独立更新](../feature/2026-10-18-dictionary-auto-update.md)把每个版本装进 `dictionary-updates/generations/` 下的不可变代次：桌面代次目录、语言代次目录，加上一份清单 `<桌面代次>.json`。`installed.json` 只记当前选择和可回退的上一个，但旧版本从来没有删过，`ResourceStore` 只清理安装中断留下的 `incoming-*` 暂存目录。每发一次词库，每台装了自动更新的设备就多留约 260 MB。维护者的 macOS 机器上 10 月 18 日到 23 日装了 5 个版本，占 1.4 GB，实际加载的只有一个。鸿蒙设置应用每次选择变化都把整个 `generations/` 复制进共享目录，再由键盘整份导入，堆积的旧版本也跟着一起被复制。

旧版本不能简单地按「不是当前也不是上一个」来删。输入会话运行时直接读下载代次里的文件（`msime-others.db`、日文模型、`helpcodes/`），偏好变化重建 Engine 时用同一个 `options.resources` 重新打开；同一个输入进程里新会话用新版本、旧会话留在旧版本是正常情况，`loaded.json` 只记得最近建的那个会话。旧会话做词库快照时还要靠自己那一版的清单校验（`set_for_directory`，测试 `an_existing_session_can_export_its_own_resource_generation_after_an_update` 守着隔了两次更新的情况）。

## Decision

检查更新（`check`）结束于 `ready`（装好新版本）或 `current`（已是最新）时，仍持着 `update.lock` 调用 `remove_old_versions`。它留下四类版本：`installed.json` 的当前选择和上一个、`loaded.json` 记录的输入法最近一次加载的版本、以及有会话持着占用凭证的版本。其余版本逐个处理：先取独占锁，再删它的桌面代次和语言代次目录（被留下的版本也引用的语言代次不删）、`verification/<代次>` 校验标记，目录都删掉后才删清单和锁文件。

会话占用：`host-api` 建会话时在 `Session::new` 之前调用 `dictionary_updates::lease(state, resources)`，资源目录是 `generations/` 下的代次时，以共享锁打开 `generations/<代次>.lock`，凭证存进 `HostSession::_generation_lease`，放在 runtime 之后，Engine 先关文件再放锁。所有会话用途都取，候选预览和快照会话同样读资源目录。随包资源、鸿蒙键盘自己的镜像目录、锁文件建不了的环境没有凭证，不影响建会话。`ScopedFileLock` 为此加了 `try_shared`，标准库在 Unix 上用 `flock`、Windows 上用 `LockFileEx`，Android 走 rustix 的 `flock`，都按打开的文件计，同一进程里会话和回收也互斥。

锁在单独的 `.lock` 文件上，不在清单上：清单经 `replace_private_file` 原子替换，回退后再装回同一版本会换掉它的 inode，锁在旧 inode 上的会话会被回收方看漏。锁文件只建不换。

删除目录：`ResourceStore::remove` 在 `resources.lock` 下先把代次目录原子改名成 `incoming-<uuid>`，再删。改名之后代次名下要么是完整目录、要么什么都没有；删到一半失败时留下的是暂存目录，下一次安装的 `sweep_abandoned_stages` 会清掉。直接删代次目录的话，删到一半剩下的目录会让之后重新安装同一代次时 `install` 走「已存在 → 校验」的分支并一直失败。Windows 上目录里有文件被打开时改名失败，代次原样保留，清单也留着，下次检查再试。发布与删除共用的同目录改名辅助函数由 `publish_generation` 改名为 `rename_without_replacing`。

没有清单指向的代次目录不在这里删。典型的是下载完成后因用户关掉开关、没有发布的那一代，留着下次发布时 `install` 直接复用，不必重下。清单内容与文件名对不上、读不出来的版本也不动。

## Alternatives considered

只留 `installed.json` 的两个版本加 `loaded.json`，代码最少，也能覆盖输入进程整体重启的常见情况。但同一进程里长寿命的旧会话就会失去资源目录：偏好变化重建 Engine 时打不开文件，快照校验退回随包基线后被拒，正是现有测试守着的场景，所以加了会话占用。

在清单 `<代次>.json` 上加锁，不必多一个文件，只读打开也能锁，键盘扩展这类写不进状态目录的环境同样能占用。但清单会被原子替换，锁会落在被替换掉的 inode 上，回收方看不到，所以改用只建不换的锁文件。写不进状态目录的环境本来也用不上下载代次：iOS 键盘没有完全访问时读不到共享容器，鸿蒙键盘用自己的镜像。

复用引擎的 `DictionaryAccess` 会话租约。它锁的是 `RuntimePaths::dictionaries`（已准备的工作词库目录）和 `user_data`，不是下载代次目录；拿它判断某一代下载资源有没有人用，要从工作词库目录反推资源代次，而回收方取独占维护租约还会因为任何活着的会话持着 `user_data` 而总是失败。

按时间保留（例如保留最近 N 天写入的版本）。桌面输入进程可以连续运行几周，时间窗多长都可能删掉还在用的版本，也解释不了为什么是这个 N。

回收放在输入宿主建会话时。输入宿主能直接知道自己在用哪一版，但[原设计](../feature/2026-10-18-dictionary-auto-update.md)明确不在活跃输入会话里清理，iOS 键盘和鸿蒙键盘对状态目录也没有完整的写权限；回收跟着下载和安装留在设置宿主的检查任务里，与安装共用 `update.lock`。

## Consequences

每台设备的 `dictionary-updates/generations/` 稳定在两到三个版本（当前、上一个，以及输入法还没换上新版时它在用的那一个），不再随发版无限增长；鸿蒙设置应用和键盘之间每次复制的目录也随之变小。回收只在每日检查或手动检查时发生，已经积攒了旧版本、但暂时没有新词库的设备，在下一次检查判定「已是最新」时就会被清理。

在这次改动之前编译的输入进程不取占用凭证。升级后旧进程还在运行时，它用的版本只靠 `loaded.json` 保护；各平台升级安装包时都会重启输入进程，残留窗口很短。

鸿蒙键盘的镜像跟着设置应用一起变小，键盘在旧版本上的会话不在设置应用的凭证与 `loaded.json` 里。键盘导入时本来就整份替换镜像目录，会话在焦点边界重建后采用新版本，这次只是把镜像里保留的范围收窄到当前与上一个版本。

资源包安装复用下载代次里同字节文件的 `generation_copy` 可选的来源少了，当前版本通常就有同一份文件；找不到时照常下载。

语言词库路径在会话聚焦时才跟上最新选择。一个会话的语言词库来自某一版，而那一版在它两次聚焦之间又被两次更新挤出保留范围、随后偏好变化触发重建时，重建会打不开语言词库，下一次聚焦就换上新的路径。需要一天内连发两次词库且用户一直不聚焦输入框，暂不为它单独加占用。

## Verification

`cargo test -p msime-client-core resources::`：删除后目录与暂存都不留、同一代次能重新下载，只接受 64 位小写十六进制的代次名。`cargo test -p msime-client-providers dictionary_updates`：会话占用、最近加载、当前与上一个各自保住自己的版本，凭证放掉、输入法载入新版后对应版本连同清单、锁文件和校验标记一起删除，回退照常可用；没有清单指向的目录不删；随包资源取不到凭证，多个会话可同时占用同一代次。`an_existing_session_can_export_its_own_resource_generation_after_an_update` 改为由会话持凭证。`cargo test -p msime-host-api`：建在下载代次上的会话从创建到销毁一直持锁；把凭证换成 `None` 后该测试失败。

## Related notes

[全平台词库独立更新](../feature/2026-10-18-dictionary-auto-update.md)定义了不可变代次、`loaded.json` 和旧会话按自己代次做快照的规则，这里在它之上补回收。[资源安装锁的释放](2026-10-15-resource-install-lock-release.md)约束 `resources.lock` 的持有范围，`ResourceStore::remove` 沿用同一套规则。

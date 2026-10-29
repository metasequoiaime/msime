# Agent Note: 移动端「词库 › 学习」的「清除学习记录」

Status: implemented

## Problem

桌面三端的共享设置页有「清除全部学习数据」，移动端没有：Android 和 iOS 的设置页里，用户想把学偏了的候选顺序、整句联想和自造词清掉，只能卸载重装。引擎早有 `reset_learned_data`（host-api 里是 `msime_client_dictionary` 的 `{operation:"reset"}`），但移动端从没接过，而且直接接上有两个洞：

- 命名词库（Android 的自建、导入和社区词库）的词是经个人词库队列写进用户词库的。`reset` 把工作主词库换回随包的一份，这些词跟着没了，`DictionaryCollections/index.json` 却还写着启用：列表上是「已启用」，打字时一个词也出不来。
- Android 的账号云同步有「个人词库」分类。只清本机不打标记时，两边都没变，下一轮什么也不传，云端留着清除前的词；以后云端一有改动，下载下来的快照会把它们整份带回本机。

## Decision

Android 的词库页（`LexiconPage`）和 iOS 的词库页（`DictionarySettingsView`）在「学习」组末尾各加一行「清除学习记录」，点了先弹确认框，确认后在主线程之外清除。

清除走引擎现成的那一条：`msime_client_dictionary` 的 `reset`，独占维护权下换回随包的工作词库和英文词库、清空学习日志——调频、置顶、固定位置、选词计数、最近选词、整句联想、拼写纠错、快捷短语槽位，以及用户自己添加和修改的词（自造词）。共享偏好、常用语（`CommonPhrases`）、剪贴板和打字统计不在这些文件里，不动。键盘会话开着时 Rust 报 `dictionary maintenance busy`，什么也不换，页面说「键盘正在使用词库，请先收起键盘再试」。Android 的键盘在失焦时销毁会话，iOS 的键盘扩展收起时就销毁会话（为避开 0xdead10cc），所以在设置页里通常拿得到。

`reset` 在 host-api 里多做一步：options 带 `preferences_directory` 时，趁还拿着维护租约调 `DictionaryCollectionsStore::requeue_enabled_after_reset`，把每个已启用集合的词重新排「加入」、清空 `preexisting.json`（重置以后用户词库里没有用户原有的词了；和随包词库重复的词送出去会再留回执、重新记成原有的），返回 `{reset:true, collections_requeued:n}`。重排失败时报 `learned-data reset collections unavailable`，桌面映射成 `dictionary_reset_rejected`；再清一次是安全的。从没建过集合目录的宿主（桌面、iOS）什么也不做，也不建目录。

Android 的 `DictionaryCollectionsStore.resetLearning` 在 `reset` 成功之后才 `SyncSignals.markDirty(context, SyncSwitch.DICTIONARY)`，接着用集合的 `synchronize` 在两秒预算内把重新排队的词写回去，剩下的由键盘空闲时继续。云同步的状态机见到「本机有改动、云端没变」就整份上传（`Mode::Upload`），而 `upload_blocked` 保证重新排队的词全部写回之前不上传，所以上传的是「清除后、命名词库的词都在」的那一份；云端在这之间被别的设备改过时按合并处理，云端的词会并回来，这是同步本来的规则。

iOS 经 `msime_client_prepare_host` 解析出与键盘相同的词库路径（与五笔查码、词库同步同一条），调 `MetasequoiaInputSessionBridge.resetLearnedData`。iOS 的账号同步只有设置和常用语（`capabilities.dictionary` 为假），不打标记；没有命名词库，导入和从社区添加的词都是自造词，跟着清掉，所以成功后 `CommunityDictionaryImports.forgetAll()`，社区词库的胶囊回到「添加」，并请键盘下次出现时重新列出个人词库。

### 文案

两端逐字相同的部分在 Android 的 `ClearLearningCopy`（`platforms/android/java/app/msime/android/dictionary/`）和 iOS 的 `ClearLearningCopy`（`platforms/ios/SharedUI/dictionary/`）：行标题「清除学习记录」、说明「删除学到的调频、联想和自造词，设置保留」、确认框标题「清除学习记录？」、按钮「清除」，以及确认框第一句「将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词），无法撤销。」。之后各说各的真话：Android 说设置、常用语、剪贴板和打字统计不受影响，已添加的词库保留、它们的词会重新写入，开着云同步时账号里的个人词库也会换成清除后的这一份；iOS 说导入和从社区添加的词条也一并删除，设置、常用语、剪贴板和打字统计不受影响，不提同步。`scripts/test-mobile-clear-learning.py` 核对两端相同的那几句逐字一致。

### 鸿蒙不提供

鸿蒙的设置应用和键盘各有一份 Engine 数据，分别在两个沙箱里（见 [鸿蒙统一「我的词条」](2026-10-27-harmony-unified-user-words.md)）。设置页的 `dictionary` 请求作用在设置应用那一份上：清掉的用户词会经「我的词条」镜像删到键盘，但键盘自己日志里的调频、置顶、整句联想和拼写纠错都还在。确认框若照 Android 的写法说「将永久删除本机学到的……」就是假的。要在鸿蒙上做，得先让键盘能接一个清除请求：设置应用在共享目录发布一个带 id 的清除请求，键盘在建会话之前（它拿得到维护权的唯一时机）看到新的 id 就对自己那一份 `reset`、记下已处理的 id，再处理镜像在键盘一侧「一次少了一大半词」的保护（现在它把这种情况当成日志坏了，不传删除）。这是一条新的跨沙箱协议，本地编译不了 ArkTS、也没有设备验收，这次不做；`docs/harmony-parity.md` 的 `resetLearnedData` 一行改写成这个真实原因。

## Alternatives considered

- **在平台代码里先 `reset`、再单独发一个集合动作重新排队。** 最强的理由是不改 `reset` 的契约，Android 自己拼两步就行。没用它：两步之间进程被杀或第二步失败，集合就停在「启用却没有词」的状态，而用户看到的是「已清除」；把重排放进 `reset`、在同一把维护租约里做，失败时整个请求报错、再清一次就把两步都重做。也不能反过来先重排再重置：重排的「加入」若赶在重置之前送进去，会被记成用户原有的词（`preexisting`），之后停用集合删不掉它们。
- **只清本机、不打云同步标记，界面写明「只清本机」**（`docs/learning-sync.md` 给尚未实现的输入习惯同步定的就是这条）。最强的理由是不会因为一台设备上的操作改掉账号里的数据。没用它：账号的「个人词库」分类是整份快照，不打标记时云端留着旧词，之后任何一次云端改动都会把整份旧词带回本机，等于清除只管到下一次同步；用户在确认框里被明确告知会同步到账号。输入习惯同步实现时另有它自己的重置屏障，与这里无关。
- **鸿蒙照样加入口，只清设置应用那一份。** 最强的理由是三端入口一致（移动端界面改动三端同步落地的惯例）。没用它：键盘的学习记录清不到，确认框的说法会是假的，用户点完以后候选顺序一点没变，比没有入口更糟。

## Consequences

- Android 和 iOS 的用户不用卸载就能清掉学偏的候选和自造词；Android 的命名词库照常可用，账号里的个人词库跟着换成清除后的一份。
- `msime_client_dictionary` 的 `reset` 返回多了 `collections_requeued`，带 `preferences_directory` 的宿主重置以后会多一批待发送的「加入」。桌面三端走 `msime_client_settings_dictionary`，没有集合目录，行为不变。
- 个人词库队列和集合待发送队列里清除之前排着、还没应用的请求，清除之后照常应用：用户在清除前刚提交的导入或编辑会在之后写进词库。Android 本地备份恢复留下的待合并输入记录（`pending-learning-merge.ndjson`）同理会在之后并回来。这些都是用户此前明确发起的操作，这次不拦。
- 「我的词条」的 WebDAV 网盘同步不经这里打标记；它下一次运行开头的核对会看到自造词没了，按它自己的规则处理（大批删除会被挡下、等用户确认）。
- iOS 的键盘扩展没开完全访问时用不到 App Group，它自己目录里的学习记录 App 碰不到；这种情况下 App 清的是 App Group 里那一份。
- 鸿蒙暂缺这一项，缺口和做法写在上面。

## Testing

- client-services：`collections_enabled_words_are_queued_again_after_a_learned_data_reset`（启用的重新排队、停用的不送、重置以后集合的词停用时照常删掉）、`collections_requeue_after_reset_leaves_a_missing_directory_alone`。
- host-api：`a_learned_data_reset_marked_for_upload_replaces_the_cloud_copy`（真实 Engine 加假后端：清除后不打标记这一轮不上传，打了标记下一轮上传的快照一个词也没有）、`a_learned_data_reset_keeps_the_words_of_enabled_named_dictionaries`（自造词清掉、启用词库的词写回、写回之前上传被挡住）。
- Android：`ClearLearningSmoke`（文案和失败提示）、`check-host.sh` 钉住页面接线和「`reset` 成功之后才打标记」的顺序，Gradle 编译 full 和 pinyin 两个版本的 Java。
- iOS：`ClearLearningTests`（键盘会话开着时是忙，销毁会话以后清掉自造词；文案）。
- `scripts/test-mobile-clear-learning.py`：两端文案逐字一致、两端都先确认再在主线程之外清除、鸿蒙的设置页没有 `resetLearnedData`。
- 没有在 Android 模拟器和 iOS 模拟器上跑过：本机盘只剩几 GB，编译机经 SSH 起不了模拟器。合并前要在 Android 模拟器上点一次（`AGENTS.md` 对改了文件读写的要求），iOS 的 `ClearLearningTests` 要在模拟器上跑一次。

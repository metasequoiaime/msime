# Agent Note: 移动端「词库 › 学习」的「清除学习记录」

Status: implemented

## Problem

桌面三端的共享设置页有「清除全部学习数据」，移动端没有：Android 和 iOS 的设置页里，用户想把学偏了的候选顺序、整句联想和自造词清掉，只能卸载重装。引擎早有 `reset_learned_data`（host-api 里是 `msime_client_dictionary` 的 `{operation:"reset"}`），但移动端从没接过，而且直接接上有几个洞：

- 命名词库（Android 的自建、导入和社区词库）的词是经个人词库队列写进用户词库的。`reset` 把工作主词库换回随包的一份，这些词跟着没了，`DictionaryCollections/index.json` 却还写着启用：列表上是「已启用」，打字时一个词也出不来。
- Android 的账号云同步有「个人词库」分类。只清本机不打标记时，两边都没变，下一轮什么也不传，云端留着清除前的词；以后云端一有改动，下载下来的快照会把它们整份带回本机。
- 清除时如果还有一份下载下来、等键盘激活的云端快照，重置以后它因本机版本变了而冲突。同步状态里还记着它（`activation`），下一轮就把它当成「没激活的快照」，按合并把云端清除前的词并回本机，待上传标记也拦不住。
- Android 本地备份恢复会把输入记录和输入习惯另存成待合并文件（`pending-learning-merge.ndjson`、`pending-habits-merge.ndjson`），由键盘空闲时合并。清除前排下的这两份不删，键盘下次空闲就把清除前的调频、置顶、整句联想和拼写纠错并回来。

## Decision

Android 的词库页（`LexiconPage`）和 iOS 的词库页（`DictionarySettingsView`）在「学习」组末尾各加一行「清除学习记录」，点了先弹确认框，确认后在主线程之外清除。

清除走引擎现成的那一条：`msime_client_dictionary` 的 `reset`，独占维护权下换回随包的工作词库和英文词库、清空学习日志——调频、置顶、固定位置、选词计数、最近选词、整句联想、拼写纠错、快捷短语槽位，以及用户自己添加和修改的词（自造词，含用户加的快捷短语）。共享偏好、常用语（`CommonPhrases`）、剪贴板和打字统计不在这些文件里，不动。键盘会话开着时 Rust 报 `dictionary maintenance busy`，什么也不换，页面说「键盘正在使用词库，请收起键盘、稍等几秒再试。」：Android 的键盘在输入框失焦时才销毁会话，之后还会短暂写一次词库；iOS 的键盘扩展收起时就销毁会话（为避开 0xdead10cc）。

`reset` 在 host-api 里多做两步，都在同一把维护租约里：

- options 带 `preferences_directory` 时，重置之前先删掉那里的待合并输入记录和输入习惯（待合并、已认领和失败计数共六个文件，`dictionary_snapshot::discard_pending_learning`）。删不掉时报 `learned-data reset rejected`，这时什么也还没清，重试即可。
- 重置之后调 `DictionaryCollectionsStore::requeue_enabled_after_reset`，把每个已启用集合的词重新排「加入」、清空 `preexisting.json`（重置以后用户词库里没有用户原有的词了；和随包词库重复的词送出去会再留回执、重新记成原有的），返回 `{reset:true, collections_requeued:n}`。重排失败时重置已经做完，不能让宿主以为什么都没清、不打同步标记，所以照样回答 `reset:true`，`collections_requeued` 为 null，另带 `collections_error`。从没建过集合目录的宿主（桌面、iOS）什么也不做，也不建目录。

Android 的 `DictionaryCollectionsStore.resetLearning` 在 `reset` 成功之后调云同步的新操作 `dictionary_reset`（`SyncSignals.dictionaryReset`）：取消状态里绑定账号排在激活队列里、还没激活的云端快照，不管开关开没开都忘掉等激活的记录（`SyncState::forget_dictionary_before_reset`），再像 `mark_dirty` 一样给词库打待上传标记；它不推进 `state_version`，与打标记一样不让进行中的那一轮作废。接着用集合的 `synchronize` 在两秒预算内把重新排队的词写回去，剩下的由键盘空闲时继续。云同步的状态机见到「本机有改动、云端没变」就整份上传（`Mode::Upload`），而 `upload_blocked` 保证重新排队的词全部写回之前不上传，所以上传的是「清除后、命名词库的词都在」的那一份。成功提示按结果说：已添加词库的词没写完时说会在键盘空闲时写回，重排失败时说没能重新写入，同步开着却没能打上标记时说账号里的个人词库仍是清除前的那一份。

iOS 经 `msime_client_prepare_host` 解析出与键盘相同的词库路径（与五笔查码、词库同步同一条），调 `MetasequoiaInputSessionBridge.resetLearnedData`。键盘有没有完全访问，状态目录都是 App Group 里同一个 `MSIME`（`bootstrapOptions`）：没开完全访问时键盘写不了 App Group，学到的东西本来就落不了盘，没有另一份 App 清不到的学习记录。iOS 的账号同步只有设置和常用语（`capabilities.dictionary` 为假），不打标记；没有命名词库，导入和从社区添加的词都是自造词，跟着清掉，所以成功后 `CommunityDictionaryImports.forgetAll()`，社区词库的胶囊回到「添加」，并请键盘下次出现时重新列出个人词库。

### 文案

两端逐字相同的部分在 Android 的 `ClearLearningCopy`（`platforms/android/java/app/msime/android/dictionary/`）和 iOS 的 `ClearLearningCopy`（`platforms/ios/SharedUI/dictionary/`）：行标题「清除学习记录」、说明「删除学到的调频、联想和自造词，设置保留」、确认框标题「清除学习记录？」、按钮「清除」、确认框第一句「将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词，含快捷短语），无法撤销。」、确认框最后一句「开着词库网盘同步时，删掉的自造词也会从其他设备上删除（一次删得多时会先请你确认）。」，以及忙和失败两句提示。中间各说各的真话：Android 说设置、常用语、剪贴板和打字统计不受影响，已添加的词库保留、它们的词会重新写入，开着云同步时账号里的个人词库也会换成清除后的这一份，登录同一账号的其他设备同步时会跟着换掉、或者提示选择；iOS 说导入和从社区添加的词条也一并删除，设置、常用语、剪贴板和打字统计不受影响，不提账号。`scripts/test-mobile-clear-learning.py` 核对两端相同的那几句逐字一致。

「登录同一账号的其他设备」那一句说的是账号词库整份快照的本来规则：清除后上传的快照是空的（没有命名词库）时，服务端那条 reset 加上空词库在另一台设备看来和「删除云端数据」一样，那台设备停下来问合并、使用云端还是关闭同步；快照里有命名词库的词时，没有本机改动的那台设备照常下载，用它整份替换自己的词库。「网盘同步」那一句说的是「我的词条」WebDAV 同步的规则：一次核对里删得少于 50 个或不到原有一半时删除照常发到其他设备，多了先挡下等用户确认（`MASS_DELETE_MIN`）。

### 鸿蒙不提供

鸿蒙的设置应用和键盘各有一份 Engine 数据，分别在两个沙箱里（见 [鸿蒙统一「我的词条」](2026-10-27-harmony-unified-user-words.md)）。设置页的 `dictionary` 请求作用在设置应用那一份上：清掉的用户词会经「我的词条」镜像删到键盘，但键盘自己日志里的调频、置顶、整句联想和拼写纠错都还在。确认框若照 Android 的写法说「将永久删除本机学到的……」就是假的。要在鸿蒙上做，得先让键盘能接一个清除请求：设置应用在共享目录发布一个带 id 的清除请求，键盘在建会话之前（它拿得到维护权的唯一时机）看到新的 id 就对自己那一份 `reset`、记下已处理的 id，再处理镜像在键盘一侧「一次少了一大半词」的保护（现在它把这种情况当成日志坏了，不传删除）。这是一条新的跨沙箱协议，本地编译不了 ArkTS、也没有设备验收，这次不做。`docs/harmony-parity.md` 的 `resetLearnedData` 一行原来只写了共享设置页入口（`personal_dictionary_request_json`）对移动端返回 `learned-data reset is unavailable on mobile`，这句没错，但移动端走的是另一个接口；现在在它后面补上鸿蒙不接 `msime_client_dictionary` 的 `reset` 的原因。

## Alternatives considered

- **在平台代码里先 `reset`、再单独发一个集合动作重新排队。** 最强的理由是不改 `reset` 的契约，Android 自己拼两步就行。没用它：两步之间进程被杀，集合就停在「启用却没有词」的状态，而用户看到的是「已清除」；把重排放进 `reset`、在同一把维护租约里做，宿主只看一个回答。也不能反过来先重排再重置：重排的「加入」若赶在重置之前送进去，会被记成用户原有的词（`preexisting`），之后停用集合删不掉它们。
- **重排失败时整个 `reset` 报错，让用户再清一次。** 最初就是这样做的。最强的理由是简单：再清一次会把两步都重做。没用它：重置已经做完，报失败时 Android 不打同步标记，用户被告知「失败」而数据其实已经没了；集合索引坏了时每次重试都再清一遍、再报失败，云端一有改动就整份带回旧词。
- **只清本机、不打云同步标记，界面写明「只清本机」**（`docs/learning-sync.md` 给尚未实现的输入习惯同步定的就是这条）。最强的理由是不会因为一台设备上的操作改掉账号里的数据。没用它：账号的「个人词库」分类是整份快照，不打标记时云端留着旧词，之后任何一次云端改动都会把整份旧词带回本机，等于清除只管到下一次同步；用户在确认框里被明确告知会同步到账号和同账号的其他设备。输入习惯同步实现时另有它自己的重置屏障，与这里无关。
- **等激活的快照在 `reset` 里取消，不动同步状态。** 最强的理由是只改一处。没用它：取消以后队列里那份算「没激活就结束了」，同步状态里的 `activation` 还在，下一轮照样按合并把云端旧词并回来；必须连同步状态一起改，而 `msime_client_dictionary` 不知道同步状态目录，所以放进云同步的 `dictionary_reset`。
- **iOS 只在键盘开了完全访问时显示这一行（键盘往 App Group 写标记）。** 最强的理由是没开完全访问时清除像是没有作用。没用它：两边用的是同一个状态目录，没开完全访问时键盘学到的本来就落不了盘，没有清不到的另一份；而且键盘没开完全访问时写不了 App Group，写下的标记在权限收回后会过期（见 [iOS 统计为零时的完全访问提示](../bug-fix/2026-10-10-ios-statistics-zero-count-full-access.md)）。
- **鸿蒙照样加入口，只清设置应用那一份。** 最强的理由是三端入口一致（移动端界面改动三端同步落地的惯例）。没用它：键盘的学习记录清不到，确认框的说法会是假的，用户点完以后候选顺序一点没变，比没有入口更糟。

## Consequences

- Android 和 iOS 的用户不用卸载就能清掉学偏的候选和自造词；Android 的命名词库照常可用，账号里的个人词库跟着换成清除后的一份，同账号的其他设备跟着换或者被问一次。
- `msime_client_dictionary` 的 `reset` 返回多了 `collections_requeued`（重排失败时为 null，另带 `collections_error`），带 `preferences_directory` 的宿主重置以后会多一批待发送的「加入」，待合并的输入记录和输入习惯会被删掉。`msime_client_cloud_sync` 多了 `dictionary_reset`。桌面三端走 `msime_client_settings_dictionary`，没有集合目录，行为不变。
- 个人词库队列和集合待发送队列里清除之前排着、还没应用的请求，清除之后照常应用：用户在清除前刚提交的导入或编辑会在之后写进词库。iOS 上刚添加、还在排队导入的社区词库也是这样，这时「已添加」已经清空，词却随后导进来。这些都是用户此前明确发起的操作，这次不拦。
- Android 写回命名词库要等键盘空闲，「我的词条」网盘同步若恰好在这段时间核对一次，这些词会先作为删除发出去、写回后再作为新增发出去，其他设备上来回一次。
- 清除时恰好有一轮云同步正在下载快照、而队列在重置之后才记下本机版本时，那份快照仍可能激活；这一轮由设置页点清除和后台同步撞在同一时刻，概率很低，这次不处理。
- 鸿蒙暂缺这一项，缺口和做法写在上面。

## Testing

- client-services：`collections_enabled_words_are_queued_again_after_a_learned_data_reset`（启用的重新排队、停用的不送、重置以后集合的词停用时照常删掉）、`collections_requeue_after_reset_leaves_a_missing_directory_alone`。
- host-api：`a_learned_data_reset_marked_for_upload_replaces_the_cloud_copy`（真实 Engine 加假后端：清除后不调 `dictionary_reset` 这一轮不上传，调了下一轮上传的快照一个词也没有）、`a_learned_data_reset_keeps_the_words_of_enabled_named_dictionaries`（自造词清掉、启用词库的词写回、写回之前上传被挡住）、`a_learned_data_reset_forgets_a_snapshot_still_waiting_for_activation`（只打标记时云端旧词被并回来，`dictionary_reset` 之后整份上传）、`a_learned_data_reset_reaches_the_other_devices_on_the_account`（两台设备：空快照让另一台停下来问，有词的快照让另一台整份下载）、`a_learned_data_reset_drops_learning_still_waiting_to_be_merged`、`a_learned_data_reset_still_succeeds_when_named_dictionaries_cannot_be_requeued`。
- Android：`ClearLearningSmoke`（文案和各种结果提示）、`check-host.sh` 钉住页面接线和「`reset` 成功之后才调 `dictionary_reset`」的顺序，Gradle 编译 full 和 pinyin 两个版本的 Java。
- iOS：`ClearLearningTests`（键盘会话开着时是忙，销毁会话以后清掉自造词；文案）。
- `scripts/test-mobile-clear-learning.py`：两端相同的文案逐字一致、两端都先确认再在主线程之外清除、鸿蒙的设置页没有 `resetLearnedData`。
- 没有在 Android 模拟器和 iOS 模拟器上跑过：本机盘只剩几 GB，编译机经 SSH 起不了模拟器。合并前要在 Android 模拟器上点一次（`AGENTS.md` 对改了文件读写的要求），iOS 的 `ClearLearningTests` 要在模拟器上跑一次。

# Agent Note: 鸿蒙设置里加的粤语词同步到键盘

Status: implemented

## Problem

[粤语个人词条](../feature/2026-11-02-cantonese-personal-words.md)（#10594，Refs #10544）让 HarmonyOS 手机和平板的设置页可以加粤语词，词写进设置应用自己沙箱里的 Engine 日志。键盘扩展有它自己的一份日志，两份之间的「我的词条」靠 `msime_client_user_words_mirror` 交换（[设计](../feature/2026-10-27-harmony-unified-user-words.md)）。交换读写本机词库用的是 `dictionary_webdav` 的 `EngineDictionary`，它的 `word_kind` 只认拼音、两版五笔、快捷短语和英文，粤语行读出来就被丢掉；协议核心的 `WordKind` 也没有粤语。结果是设置页加的粤语词从来不进发布文件，键盘的日志里没有，粤语方案的候选里也就没有，删掉也同样传不过去。

iOS 的应用和键盘扩展共用 App Group 容器里的同一份日志，Android 的设置和输入法在同一个应用的数据目录里，都没有这层交换，不受影响。

## Decision

- 协议核心的 `WordKind` 加 `Cantonese`（名字 `cantonese`，按字母顺序排在最前，`Ord` 与名字顺序仍一致）。`WordKey` 认的粤语编码就是 Engine 存进日志的写法：不带声调、单个空格隔开的音节，每个 1 到 8 个小写字母；声调和 `'` 分隔由 Engine 写入时收掉，`WordKey` 不再规范化。
- `EngineDictionary` 默认仍不读写粤语，`with_cantonese()` 打开；只有 `user_words_mirror` 调它。没打开时 `read` 跳过粤语行，`write` 不写粤语词（报成 Engine 不收），免得 WebDAV 收到一个粤语词写进来、下一次核对又读不出来、当成本机删掉了。
- 交换的发布文件和私有状态仍是第 1 版格式，不升版本：只是多了一个类别名。老的状态文件照常读；降级到不认识 `cantonese` 的版本时，带粤语的状态读不懂，按原有规则从头开始（并集，不删词），带粤语的对方发布整份不认、这次不合并。两个进程来自同一个安装包，一起升级。
- 键盘这边按 `DictionaryEntry::DEFAULT_WEIGHT`（100000）写进来，与各端新粤语词的默认权重相同，不会排在发布的同音词后面。设置页里改过的权重不交换，与其他类别一样。

## Alternatives considered

- **鸿蒙上把「我的词条」的粤语类型藏起来（像 wubixsj 那样）**：改动最小，不碰交换。但 #10544 要的就是加了粤语词能在候选里出现，鸿蒙的设置页是唯一的加词入口，藏起来等于这个平台没有这项功能，与 iOS、Android 不一致。
- **让 WebDAV 同步也带上粤语词**：一个开关都不用加，两条路的「我的词条」完全一致。不做：网盘上的格式没有这个类别，还没升级的设备读到带 `cantonese` 的检查点会整份不认（`ReplicaFile::into_replica` 对认不出的类别返回 `None`），要先给协议升版本；#10594 也明确把粤语排除在云同步之外，留到第二期一起定。
- **交换的格式升到第 2 版**：升级后老设备上的版本号不同，读不懂就从头开始，墓碑全丢，以前在一边删过的词会回到两边。只多一个类别名，第 1 版的读者遇到它的代价（整份不认、下次再来）比丢墓碑小。

## Consequences

- HarmonyOS 设置页加的、删的粤语词在键盘下一次空闲重建会话后生效，与其他类别同一时机。
- `EngineDictionary` 多了一个范围开关，WebDAV 那条路的行为不变：`cantonese_words_stay_out_of_the_webdav_sync_but_not_out_of_the_mirror` 守着。
- 验证：`rbuild cargo test -p msime-host-api --lib -- user_words_mirror dictionary_webdav`（含新的 `a_cantonese_word_added_in_settings_reaches_the_keyboard_and_so_does_its_removal`，去掉 `with_cantonese()` 时它在设置一侧记下 0 个改动、失败）、`rbuild cargo test -p msime-client-core --lib dictionary_sync`。没有在 HarmonyOS 真机或模拟器上跑过（模拟器没有共享沙箱，本来就不交换）。

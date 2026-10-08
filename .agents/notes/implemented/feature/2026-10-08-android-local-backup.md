# Agent Note: Android 本地备份与恢复

Status: implemented

## Problem

#5659：换手机、双持或者升级出问题要降级时，用户想把设置、词库和输入记录整份带走，而且全部在本机完成，不经过任何云端。Android 宿主已有的导出都不满足：「导出我的数据」要登录、由服务端生成；词库页只能导出单个词库的文本；云同步要账号。

## Decision

- 「我的 → 我的内容 → 备份与恢复」（`BackupPage`，`PageId.BACKUP`）：「导出备份」经系统文件选择器（SAF，不申请存储权限）写一个 zip，预填文件名 `<应用名>-<版本名>-<yyyyMMdd-HHmm>.zip`；「从备份恢复」选一个 zip，先读出说明让用户确认，再恢复。
- 包的内容与云同步相同，复用同一套导出和应用（`LocalBackup`，格式规则在纯 Java 的 `LocalBackupPolicy`）：
  - `manifest.json`：`format = "msime-android-backup"`、`version = 1`、应用名、版本名、产品版本、导出时间和各部分的条数；
  - `settings.json`：`msime_client_account_settings_export` 导出的设置文档（与云同步上传的同一份，凭据和诊断日志永远不在里面），不含皮肤库；
  - `android-local.json`：Android 本地设置文件里显式写过的值（含不随云同步的键盘高度、滑行输入、隐私模式等）；
  - `skins.json`：自定义键盘皮肤的设计参数（`CustomSkinLibrary.exportDesigns`，不含照片）；
  - `phrases.json`：自己添加的常用语（本机预置、没被认领的示例除外）；
  - `dictionary.ndjson`：`export_snapshot` 写的个人词库快照，格式与云端词库快照相同。
- 恢复是合并，不是覆盖，各部分互不牵连，失败的部分在结果里点名：设置文档交给 `msime_client_account_settings_apply`，字段表按备份里每个值自己的类型声明（备份是 client-core 自己导出的，所以应用的映射与云同步完全一致），偏好按修订号比较并交换写回，被键盘抢先写了就重来一次；本地设置逐项按 `AndroidLocalSettings.Spec` 校验后写回，不认识的键跳过；皮肤按 id 和更新时间合并；常用语逐条添加，本机已有的跳过；个人词库在本机用户词库还是空的（新手机、重装）时把整份快照交给词库快照激活队列（`CloudSync.enqueueDictionarySnapshot`，请求来源记为 `local-backup`），原样恢复，与云同步第一次同步而本机没有词时的做法相同；本机已经有词，或键盘还没打开过、队列里已有一份快照时，经个人词库导入队列合并（`CloudSync.queueWords`，与云同步「合并」同一条路）。两条路都在键盘下次打开时写入。恢复后标记设置、皮肤和词库待同步，开着云同步时会照常上传。
- 格式版本比本应用新时拒绝恢复并提示升级；`format` 不对或没有 manifest 时说明不是备份文件。选中的文件先复制进应用缓存（最多 600 MiB，词库快照最多 512 MiB），只按固定的条目名读，每个条目有自己的大小上限，不按条目名解压到磁盘。
- 为了复用，`CloudSync` 里的设置应用（`applySettings`）、按键反馈（`hostFeedback`）、用户词数（`userWordCount`）、词库快照导出与入队（`exportDictionarySnapshot`、`enqueueDictionarySnapshot`）和导入队列（`queueWords`）从同步会话里提成静态方法，云同步本身的行为不变。

## Alternatives considered

- **直接打包 `files/bootstrap/state` 整个目录，恢复时整份换回去** — 最完整，连学习到的候选位置调整、输入统计都能带走；但键盘进程随时开着词库和偏好文件，整目录替换要和键盘的会话协调（现有的词库快照激活为此专门有一条只在没有会话时执行的队列），还会把账号会话、凭据和诊断日志一并带出本机，跨版本恢复时文件格式也没有任何兼容层。
- **在 client-core 里新写一套备份格式和 FFI** — 格式归共享代码，以后其他平台可以直接用；但需要新的 C ABI、JNI 和各平台的接线，而 Android 已有的同步导出/应用正好覆盖同样的数据，并且已经处理了版本过滤、取值校验和并发写入。其他平台要做本地备份时，再把格式下沉到 client-core。
- **恢复时整份覆盖（删掉本机多出来的常用语和词）** — 恢复后与备份一模一样；但用户在新手机上已经打过的字、加过的词会被悄悄删掉，而合并不会丢任何东西。

## Consequences

- **收益**：不登录也能把设置、皮肤、常用语和个人词库带到另一台设备或降级后的版本；文件名带版本号，降级时容易找到对应的包；所有内容在用户自己选的位置，不经过云端。
- **代价与已知上限**：学习到的候选位置调整和选词计数是 Engine 内部状态，没有只读接口，不在备份里（与云同步相同）；输入统计、剪贴板历史、自定义皮肤的照片和社区短语包也不在里面；命名词库的词条随个人词库一起恢复，但名字和启用状态不恢复。恢复的词要等键盘下次打开才写入；本机已有词时走导入队列，队列同时只收 128 批、每批 128 个词，超出的部分在结果里报为跳过，可以等键盘处理完后再恢复一次。导出和恢复跑在设置页共享存储的串行线程上，词库很大时这期间其他设置页的读写会排队。真机上的端到端流程没有验收过，只有格式规则的 JVM 冒烟和 Gradle 编译。

## Verification

`platforms/android/tests/settings/LocalBackupPolicySmoke.java`（`bash platforms/android/check-host.sh`）；页面与打包代码经 Gradle `compileFullDebugJavaWithJavac` 编译。

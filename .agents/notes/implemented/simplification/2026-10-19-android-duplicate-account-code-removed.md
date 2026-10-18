# Agent Note: 删掉 Android 已经交给 Rust 的账号与云端重复实现

Status: implemented

## Problem

#8201 迁移计划 5f。5b 到 5e 把 Android 的会话、登录、设置页面、快照队列、云同步和键盘释义一组组改调 Rust（[会话与登录](../architecture/2026-10-18-android-account-through-rust.md)、[设置页面](../architecture/2026-10-18-android-settings-pages-through-rust.md)、[快照队列与云同步](../architecture/2026-10-18-android-cloud-sync-through-rust.md)、[键盘释义](../architecture/2026-10-18-android-account-gloss-sans-io.md)），每组只删被改写的类里当场没人调用的部分，剩下的整类和整段留到这一组：Java 的同步接口和合并规则、`BackendAccount` 的对话和云剪贴板开关、`CloudApi` 的 multipart、JNI 里给 Java 快照队列用的准备与激活入口，以及只测它们的冒烟和守卫脚本。留着它们的代价是新代码可能接着调用旧的那一份，守卫脚本也在盯着已经不跑的代码，绿了也说明不了什么。

## Decision

- 删掉没有调用方的 Java 实现：`SyncApi`（偏好、常用语、快照的上传下载与流式传输）、`SyncMergePolicy` 的合并、节流和皮肤预算规则、`CloudSync.queueWords`、`BackendAccount` 的对话（流式与非流式、SSE 读取）和云剪贴板开关与删除连同它们用的 JSON 请求路径、`CloudApi` 的 multipart 编码和 413、415 两个判断、`FeedbackApi` 按文件头校验截图（Rust 读文件头）、`AccountIdentity.shortSubject`（匿名身份 5b 起在 Rust）。
- 本地备份恢复还要从快照文件里读用户词、换算种类、去重分批，这部分不在 Rust 里，从 `SyncApi` 和 `SyncMergePolicy` 挪进 `dictionary/DictionarySnapshotWords`，规则一字不改；`LocalBackup` 自己记下要剔掉的皮肤库键名。
- JNI 去掉 `snapshotPrepareRaw`、`snapshotDiscardRaw`、`snapshotActivateRaw`、`aiSkinPlanRaw` 和只给准备用的 `SnapshotReader`，Java 一侧去掉对应的包装和重算快照记录数的 `inspectSnapshot`。准备、激活整份快照从 5d 起在 Rust 的 `msime_client_snapshot_queue` 里，AI 皮肤从 5c 起是 `msime_client_ai_skin_generate`。host 库照旧导出这些 C ABI，其他平台还在用。
- `SignIn` 登录成功后记账号绑定时，用户 id 读 Rust 刚存下的会话（`DeviceDataApi.currentUserId`，即 `status`，不联网），不再另发一次 `GET /v1/users/me`。
- 留着的：`snapshotVersionRaw`（`SessionStoreDeviceSmoke` 用它造一份真实的旧队列状态）、`sessionStoreUnregisterRaw`（设备冒烟注册完要收掉）；`aiSkinJobRaw`、`cloudDictionaryRaw` 是 5a 加的入口，产品一直没有接上，只有设备冒烟核对它们的请求不合法码，这一组不动，是否删由负责人定。
- `check-host.sh` 加一道守卫：Java 里出现 `"/v1/` 请求路径的只能是云剪贴板的端到端加密驱动（`CloudClipboardApi`、`CloudClipboardE2e`）和不在迁移范围内的打字统计与 QQ 绑定（`TypingUploadApi`）。
- 测试跟着删或改写，不留空转的守卫：`SyncApiSmoke`、`SyncMergePolicySmoke`、`BackendAccountEventStreamSmoke` 删掉，还成立的那几项（种类换算、分批、权重、单行与总记录数上限）进 `DictionarySnapshotWordsSmoke`；`BackendAccountSessionDeviceSmoke` 里「请求绑定到发起它的登录」那几项从已删的云剪贴板开关改成走键盘的云剪贴板上传，对话几项删掉；守卫脚本 `test-android-sync-phrase-response-bounds.py`、`test-android-sync-revision-policy.py`、`test-android-snapshot-prepare-request-bounds.py`、`test-android-snapshot-activate-bounds.py` 删掉，`test-android-account-binding.py`、`test-android-file-reader-links.py`、`test-native-snapshot-readers.py`、`test-android-named-daemon-thread-policy.py` 去掉指向已删代码的那几项；`check-host.sh` 里一道从 develop 起就对不上函数签名、一直空过的 `queueImport` 守卫删掉。

## Alternatives considered

- **`SyncApi.java` 留着、只剩读快照词那一个函数。** 理由是改动最小，`LocalBackup` 一行不用改。没有采用：类名和文件头都在说云同步，下一个人会以为同步还在 Java 里，守卫脚本也还会把它当同步接口来盯；读快照词和种类换算本来就是两处各一半，合成一个以用途命名的类更好找。
- **`SignIn` 记绑定的那次 `/v1/users/me` 先不动，只删没人调用的代码。** 理由是 5f 写的是删重复实现，这一处有调用方，改它是行为变化。没有采用：它就是 Rust `status` 的又一份，而且更差，登录刚成功就断网时读成空、被当成换了账号，清掉上一个账号的待激活快照和同步绑定；它也是新守卫唯一要放过的例外，留着就得在守卫里给它开口子。
- **把 5a 没接上的 `aiSkinJobRaw`、`cloudDictionaryRaw` 一起删掉。** 理由是没有产品调用方的 JNI 入口只增加维护面。没有在这里做：它们不是重复实现，是计划 5a 按头文件接好的入口，删不删是要不要给 Android 留这两个能力的决定，不该夹在清理里顺手做掉。

## Consequences

- **收益**：Android 主进程里账号与云端的请求、合并和快照激活只剩 Rust 一份，Java 自己拼云端路径的地方只剩三处，并且有守卫盯着；Java 净删约 1,400 行、测试约 680 行、JNI 胶水约 140 行，守卫脚本约 120 行。
- **用户可见的变化**：只有登录那一处。登录成功以后记账号绑定不再联网，登录刚成功就断网时不再被当成换了账号（迁移差异 M105）。其余都是删掉没人调用的代码，行为不变。
- **代价**：`DictionarySnapshotWords.read` 和 `SignIn` 都要 org.json 或在 `home/` 里，JVM 冒烟跑不到，只靠 Gradle 编译、源码守卫和设备冒烟；`BackendAccountSessionDeviceSmoke` 改写的几项只在设备上跑。

## Verification

- `check-host.sh`：JVM 冒烟、设备套件源文件编译、`client_jni.cpp` 和 `JniCallbacks.cpp` 用固定 NDK 编译都通过。新守卫在把 `SignIn` 改回读 `/v1/users/me` 时失败；`DictionarySnapshotWordsSmoke` 在去掉单行上限时失败。
- Gradle 编译整个应用的 Java（`compileFullReleaseJavaWithJavac`）通过；`scripts/test-android-jni-names.py` 核对 Java、C++ 和 `verify-native.sh` 三处的 JNI 名字一致。
- 设备上跑没跑、在哪台 AVD 上跑，见这一组的 PR 说明。

# Agent Note: Android 不联网安装本地语音模型和语音运行库

Status: implemented

## Problem

[Android 本地语音模型](2026-10-12-android-local-voice-models.md)只能联网下载。模型和语音运行库都放在 GitHub Release 上，国内网络常常只有几十 KB/s，几百 MB 的模型下不完；#6166 要的正是「自己下好再导入」。共享层的模型安装早就接受 `files`（[本地文件导入](2026-10-09-local-voice-model-import.md)），但 Android 原生设置页没有接它。而资源包（其中 voice-runtime 是瘦包本地识别的前提）在共享层根本没有不联网的安装：`resource_pack_adopt` 只收编已经解压好的文件，而 voice-runtime 来自上游的 `.aar` 归档，用户下到的就是那个归档。

## Decision

- 共享层：
  - `resource_packs::import(state_root, pack, files, …)` 与模型的 `local_models::import` 同构。`ResourcePack::import_artifacts` 给出要从外面取得的文件（有归档的资源包是那个归档，其余是锁文件里的全部文件）；`LocalFileFetcher::matching_sources` 按长度、必要时按 SHA-256 把它们对应到用户的文件；随后走与下载完全相同的 `install_archive_members_with` / `install_files_with`。所以取成员、校验、暂存和发布只有一套，下载阶段报成 `import`，读错误报成 `local_model_import_unreadable`（`local_models::imported`）。
  - 为此把 `LocalFileFetcher` 和 `ImportSource` 从按模型匹配改为按一组来源匹配，模型的导入行为不变。
  - `resource_packs` 的列表多出 `import_files:[{name,url,size}]`。`resource_pack_install` 接受可选的 `files`（至多 16 个绝对路径），带了就走 `import`、不看 `sources`；进度阶段多一个 `"import"`。C 头文件和 FFI 文档同步更新。
- Android：
  - `ResourcePacks.install` 和 `LocalVoiceModels.install` 多一个 `files` 参数，`Pack` 和 `Model` 带上 `importFiles`。
  - `ResourcePackService.requestImport` 收到文件选择器给的 URI 之后，在下载线程上把长度对得上的文档复制进 `cacheDir/voice-model-import/<uuid>/`（`OfflineImportPolicy`：长度报不出来的也复制，但读到超过最大的那个文件就丢掉），再交给共享层安装。不论结果如何，复制出来的文件都删掉。
  - 导入和下载走同一条队列和同一条通知，状态多一个 `IMPORTING`（「导入中 x%」）。复制途中可以取消，失败和结果通知写「导入失败」「已导入」。URI 的读权限经启动服务的 Intent 的 `ClipData` 加 `FLAG_GRANT_READ_URI_PERMISSION` 交给服务，用户离开设置页后复制照样进行。
  - 设置页的「本地模型」组多一行「从文件导入」。可选的目标是还没装的模型，以及（瘦包还缺的话）语音运行库。选中后先说明要下载哪些文件（上游文件名和大小，「下载地址」用浏览器打开上游链接），再打开多选的文件选择器。

## Alternatives considered

- **voice-runtime 只让用户导入解压好的两个 `.so`，复用 `resource_pack_adopt`**：共享层一行都不用改。没有采用：用户能下到的只有上游的 `.aar`，让人自己解压、再找到对应 ABI 目录里的两个文件，几乎没人做得对；而且 `adopt` 是按文件名找、改名移走源文件，不适合用户随手选的文件。
- **把 URI 直接交给共享层读，不复制**：省一份磁盘空间和一遍拷贝。没有采用：共享层的 C ABI 只收绝对路径，Android 的 content URI 要经 `ContentResolver` 打开，绑一个 fd 进 Rust 意味着新增一套 ABI 和生命周期约定；HarmonyOS 同样是先复制进缓存，两边做法一致。
- **在设置页里复制，复制完再启动服务**：代码少一层。没有采用：几百 MB 的复制要几十秒，页面被关掉或进程进入后台时复制就断了；放进前台服务里，复制和安装共用一个可取消的进度通知。

## Consequences

- 收益：网络不通的用户也能装上本地模型和语音运行库，瘦包因此真正做到可以完全离线识别。资源包的不联网安装也是共享层能力，其他宿主需要时直接可用。
- 代价：导入期间缓存里多一份文件副本，峰值占用是原文件、副本和解压结果三份。`pendingImport` 只放在页面字段里，在文件选择器打开期间旋转屏幕或页面被重建，这次选择就会被丢掉，要重新选。
- 验证：`cargo test -p msime-client-core -- resource_packs:: voice::local_models`（`import_files_are_the_archive_or_every_pinned_file`、`importing_matches_by_size_and_publishes_nothing_on_wrong_bytes`）；`cargo test -p msime-host-api -- pack_downloads voice_local_model`（`install_from_files_validates_the_list_and_names_a_missing_file`）；`platforms/android/check-host.sh`（含 `OfflineImportPolicySmoke`）；`gradlew compileFullReleaseJavaWithJavac`。没有在真机上用真实的上游归档导入过。

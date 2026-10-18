# Agent Note: 本地语音模型从本地文件安装

Status: implemented

## Problem

#6166：本地语音识别只能在设置页点「下载」安装，下载走 GitHub Releases（或镜像），网络不通或下载太慢的用户装不上模型，「完全离线」就无从谈起。手填模型目录这条路也走不通：识别器 `shared/voice/LocalAsr.cpp` 只认带 `msime-model.json` 的目录，而这个文件只有安装器会写，用户自己解压上游归档得不到它。用户希望能用自己的下载工具把数据包下好，再在设置里导入，并能删除已装的模型、能找到下载页。

## Decision

- 共享安装器 `crates/client-providers/src/voice/local_models.rs` 新增 `import(root, id, files, progress, cancel)`。`files` 是宿主文件选择器给出的绝对路径，顺序和文件名都不看：对目录里这个模型要的每个文件（压缩包，以及带 `url` 的附加文件，内嵌的 `resource` 不算），先按长度找；只有一个长度对得上时直接用它，有几个时逐个算 SHA-256 挑出对的那个。一个都没有时报 `LocalModelError::MissingImportFile`（`local_model_import_missing: <上游文件名>`），长度相同但摘要都不对时报 `ChecksumMismatch`。多选的无关文件忽略。
- 对上之后用 `LocalFileFetcher`（实现已有的 `Fetcher` trait，把目录里的下载地址映射到本地文件）调用原来的 `install_model`，镜像列表为空。校验、暂存、解压、写 `msime-model.json`、整体改名发布全部复用下载的那一套，文件不对或被改过时什么也不会装上。进度里原来的 `download` 阶段报成 `import`，其余阶段不变。
- `LocalModelStatus` 新增 `import_files: [{name, url, size}]`，设置页据此列出不联网安装要下载的文件，每个文件直接链到目录里的上游下载地址（`github.com/k2-fsa/sherpa-onnx` 的 release 资产），不另建官网页面。
- C ABI 不新增函数：`msime_client_voice_local_model_install` 的请求多一个可选的 `files`（至多 16 个绝对路径），带了就走 `import`。这样导入和下载共用同一个按 id 的登记，取消、互斥（`local_model_install_running`）和删除时的拒绝不用另写；iOS 和 HarmonyOS 的绑定也不用加新符号。
- C ABI 在验证目录 id、登记任务并释放登记锁后，先同步报告一次 `download`（带 `files` 时为 `import`）的零字节进度，`total` 沿用归档字节数，然后检查取消标记。初始回调必须先于 provider 调用，宿主因此能重入取消，且在文件访问、HTTP 客户端构造和网络请求前结束。Android 的 `ResourcePackService.CANCELLING` 已保存登记前的取消意图，首次回调让它及时补发；只在安装开始前检查一次仍有检查与 C ABI 登记之间的窗口。未知 id 不登记、不回调。
- 只接受内置目录里的 id，不开放自定义或第三方模型：安装器、删除和识别器都只认目录 id 和清单格式，放开要先定清单格式和安全边界。
- 各宿主：
  - 三个桌面宿主（macOS、Windows、Linux 共用 Tauri 设置页）新增命令 `voice_local_model_import(id)`，在宿主侧用 `tauri_plugin_dialog` 的 `blocking_pick_files` 多选文件，不按扩展名过滤（认文件靠内容），用户关掉对话框时返回 `null`。命令只在三个桌面目标上注册，`main.tsx` 也只在这三个平台给 `localVoiceModels` 加 `import`。
  - 共享设置页 `LocalModelManager`：`LocalVoiceModelClient.import` 是可选的，宿主不提供就不显示按钮。未安装的模型在「下载」旁边多一个「从文件导入」，下面列出要下载的文件链接；导入成功后和下载一样，没有选用模型时自动选用。
  - HarmonyOS：设置页 `voice_local_model` 新增 `import` 操作，用 `DocumentViewPicker` 多选，选择器给的是原生库打不开的文档 URI，所以先复制进 `cacheDir/voice-model-import/<uuid>/`；只复制长度和目录里需要的文件对得上的那些（`LocalVoiceModelPolicy.importSizes`），免得用户多选了一个大文件白占空间。缓存副本在安装结束后删掉。复制那段时间原生安装还没登记，原生取消找不到它，所以 `SettingsBridge` 记着正在导入的 id：取消先记下来，复制每个文件前和开始安装前检查，看到就以 `local_model_cancelled` 结束，不再安装（review 发现最初的版本在复制时点「取消导入」没有作用，复制完照样装上）。
  - iOS：`LocalSpeechModelsView` 用 `.fileImporter(allowsMultipleSelection: true)`，在整个安装期间保持安全范围访问权，把原文件路径交给共享安装接口，不复制。
    下载和导入都由每次独立的 `LocalSpeechModelDownload` 保存取消意图；启动前取消不调用安装器，登记前取消在 C ABI 的首次进度中同步补发。主线程只用短锁保存意图，由每次独立的后台串行队列转发共享取消；FFI 不持该锁，避免共享登记锁被其他模型删除占用时阻塞页面。结束标记在同一队列排空旧取消后设置，避免旧对象的取消误伤下一次重试；安装器与外部进度闭包不持锁。管理器在发布运行状态前保存该对象，离页只取消而不提前解除运行标记，等实际退出后再清理。进度同时核对页面代次和对象身份，旧任务不能覆盖重试；后台返回成功前离页也不自动选用模型。
    删除前释放识别器，但只在删除成功返回主 actor 后、当前选择仍命名被删模型时清空 `asr_model_path`。等待期间改选其他模型会保留；等待期间改选被删模型则会清空；失败时不改选择。判断与保存之间没有挂起点，不使用删除开始时的选择快照。
  - Android 原生设置页本来就没有模型下载、删除入口，本次不补；瘦包的 `voice-runtime` 也不能从文件导入，所以 Android 还不能完全离线。
- 删除已有的 `remove` 能力不变，各宿主的「删除」按钮沿用。

## Alternatives considered

- **按文件名对应** — 实现最简单，用户也容易理解「把这几个文件选上」。但下载工具常会改名（加序号、`.download` 后缀、浏览器去掉扩展名），按名字对应会让一个字节完全正确的文件被拒；内容是唯一可靠的身份，长度先筛一遍，大多数情况下不用额外读一遍文件。
- **新增 `msime_client_voice_local_model_import` C ABI** — 分诊建议的形状，名字更直白。但它要复制一遍 install 的登记、取消、进度回调和删除互斥，iOS 的 `@_silgen_name`、HarmonyOS 的 NAPI async work 也都要再加一套；在 install 请求上加一个可选字段，所有这些都自动成立。
- **只靠后续进度补发登记前的取消** — 宿主已有补发逻辑，不必改共享接口。但模型下载首次进度在读取数据后才出现，无法在阻塞网络读取前取消；导入也可能在报告进度前以缺文件失败。初始回调复用已有阶段和 JSON 字段，不增加新状态或 ABI 符号。
- **离页时直接清空运行标记并异步调用一次共享取消** — 页面能立即恢复可操作状态，但共享任务未登记时取消会丢失；旧任务还在执行时允许重试，又会碰到同 id 互斥或让迟到的取消影响新任务。按次对象保存意图并等实际退出，与 iOS 资源包的取消契约一致。
- **持对象锁同步转发共享取消** — 取消和结束天然串行，容易防止旧请求影响重试。但共享取消会等待登记锁，而另一模型的删除可能持该锁遍历目录；主线程直接取消或等待进度补发持有的对象锁都会卡住。改由后台队列串行转发并在退出前排空，短锁只保护状态。
- **按删除开始时的选择决定是否清空** — 一次快照便于把清理绑定到发起操作时的状态，但后台删除期间用户仍可选择模型：既会清掉新选的其他模型，也会漏掉刚改选的被删模型。成功后核对当前选择才与磁盘结果一致；提前清空则会在删除失败时丢掉仍可用的模型选择。
- **复用手填目录（`VoiceModelPathSection`）让用户自己解压** — 不需要任何新代码。但用户解压出的目录没有 `msime-model.json`，识别器拒绝它；就算补写清单，也没人校验里面的文件是否完整、是否被替换过，这正是安装器要挡住的东西。
- **官网建一个「语音数据包」下载页** — issue 的原意，能放国内镜像。需要有人建页面并跟着目录版本持续同步；直接链到目录里的上游地址不用维护，国内访问不畅时用户仍可以用镜像或别的下载工具拿到同一个文件，导入时照样按 SHA-256 校验。

## Consequences

- **收益**：不联网也能装上目录里的模型，校验和发布与下载完全一致，没有第二条信任路径；新 ABI 只是 install 请求上的一个可选字段，老宿主不受影响。
- **代价与已知上限**：导入时同一个文件最多读两遍（长度相同的候选有几个时先算摘要，安装时再校验一遍），842 MB 的 Fun-ASR-Nano 归档在慢盘上要多花几秒；HarmonyOS 先复制进缓存，峰值占用是原文件、缓存副本和解压结果三份。所选文件打不开或读到一半出错（权限、文件被移走、可移动磁盘拔掉、iCloud 占位文件）报 `local_model_import_unreadable`：`LocalFileFetcher` 打开和定位失败、算摘要时读失败直接报它；复用的 `download()` 把来源读错误都归成 `Network`，而导入根本不联网，所以 `import_model` 把结果里的 `Network` 改成它。最初的版本把前者报成 `local_model_io`（页面说「无法写入，请检查存储空间」）、后者报成 `local_model_network`，都把问题指向了别处。Android 原生应用要补模型管理和 `voice-runtime` 离线导入时，需要重新拆一期；其中下载、选用和删除已由 [Android 本地语音模型](2026-10-12-android-local-voice-models.md)补上，从文件导入模型和运行库由 [Android 不联网安装](2026-10-12-android-offline-voice-import.md)补上。

## Verification

`scripts/test-ios-local-model-cancellation.py` 编译真实 `LocalSpeechModelManager` 与下载对象，替换安装和偏好边界，以条件同步覆盖下载、导入各自的启动前取消、登记前取消、离页取消、共享取消阻塞、首次进度补发阻塞与退出后重试。测试只剥离同文件的 SwiftUI 视图部分，不复制管理器逻辑。旧实现的启动前取消与离页提前解除运行标记均先稳定失败，持锁同步转发取消的版本在阻塞回归中超时；回归不下载真实模型。`LocalSpeechModelDownloadTests` 还通过原生 Swift 桥接在共享首次进度中取消下载和导入。

同一真实管理器回归还以删除屏障覆盖改选其他模型、改选被删模型、保持选中、删除未选中模型及删除失败五种情形，并检查页面状态与持久化选择一致。旧实现先在前两种交错场景失败；测试不操作真实模型文件。

`cargo test -p msime-host-api --locked voice_local_model_initial_progress_can_cancel_before_provider_io` 通过真实 C ABI 在首次回调重入指定 id 的取消，覆盖下载与导入，断言初始阶段、零字节、归档总量、取消时任务已登记、退出后登记释放且模型根目录未创建。下载请求带明文合成镜像，即使初始回调回归也会在联网前被拒绝；导入使用空文件列表。旧实现回归先以 `local_model_invalid_mirror` 失败，修复后两条路径均以 `local_model_cancelled` 结束。此回归不代表设备上真实下载验收。

`cargo test -p msime-client-core voice::local_models`（`importing_*`、`cancelling_an_import_leaves_nothing_behind`、`listing_names_the_files_an_offline_install_needs`）；`cargo test -p msime-host-api voice_local_model_install_from_files`；`cargo test -p msime-desktop local_model`；`apps/desktop` 的 `vitest run tests/voice/local-voice-models.test.tsx`；HarmonyOS 的 `platforms/harmony/tests/keyboard-logic.test.ts`（`LocalVoiceModelPolicy` 组）。没有在真机上断网导入过真实的上游归档。

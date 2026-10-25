# Agent Note: Android 手写关掉 ML Kit 的 datatransport 诊断上传

Status: implemented

## Problem

基础 APK 的手写用 `com.google.mlkit:digital-ink-recognition:19.0.0`。它经 `com.google.mlkit:common` 带进 Google datatransport（`transport-runtime` 2.2.6、`transport-backend-cct` 2.3.3），清单合并进 `TransportBackendDiscovery`（元数据 `backend:com.google.android.datatransport.cct.CctBackendFactory`）、`JobInfoSchedulerService` 和 `AlarmManagerSchedulerBroadcastReceiver`。

2026-10-25 在 Pixel 3 XL（Android 12，`:ime` 进程）上实测：打开手写面板查模型、写字识别时，每次都写一条 `TransportRuntime.SQLiteEventStore: Storing event with priority=VERY_LOW, name=FIREBASE_ML_SDK for destination cct`，并排 jobId `-674225152` 的上传任务（要求设备空闲、不计流量网络、至少延迟 1 天）。强制运行该任务后主进程打出 `TransportRuntime.CctTransportBackend: Making request to: https://firebaselogging.googleapis.com/v0cc/log/batch?format=json_proto3` 和 `Status Code: 200`；PCAPdroid 只抓水杉 UID，看到 `firebaselogging.googleapis.com` 的 DNS 查询和一条 SNI 为该域名的 HTTPS 连接（`172.217.112.4:443`，收 4.7 KB、发 1.9 KB）。模拟器上读出的事件元数据有 country、product、mcc_mnc、locale、manufacturer、net-type、os build、sdk-version、fingerprint、model、tz-offset、device、hardware，载荷里有包名、应用版本、SDK 版本、`ml_sdk_instance_id`（存在 `shared_prefs/com.google.mlkit.internal.xml`，长期不变）和模型语言 `zh-Hani-CN`。输入法自己不上报这些，隐私说明也没写，属于没有披露的第三方上报。

## Decision

`platforms/android/AndroidManifest.xml` 声明 `xmlns:tools`，用 `tools:node="remove"` 删掉三个 datatransport 组件：

- `com.google.android.datatransport.runtime.backends.TransportBackendDiscovery`：`MetadataBackendRegistry` 读不到它的元数据，`cct` 后端不存在。`DefaultScheduler` 先查后端，查不到只记 `W TransportRuntime: Transport backend 'cct' is not registered` 就返回，事件不落库、不排任务。
- `com.google.android.datatransport.runtime.scheduling.jobscheduling.JobInfoSchedulerService`：从旧版本升级的设备数据库里还有事件，`WorkInitializer` 在进程启动时给这些上下文排任务，`JobScheduler` 拒绝不存在的服务（`IllegalArgumentException: No such service`），异常被 datatransport 的 `SafeLoggingExecutor` 接住，只记一条 `E TransportRuntime.Executor: Background execution failure.`，不崩溃。旧事件留在本地，没有上传途径。
- `com.google.android.datatransport.runtime.scheduling.jobscheduling.AlarmManagerSchedulerBroadcastReceiver`：API 21 以下的闹钟上传入口，一并删掉，不留第二条路。

识别、模型检查和模型下载不经过 datatransport，照常工作。`ml_sdk_instance_id` 仍由 ML Kit 在本机生成，只是不再有途径发出去。

`platforms/android/check-host.sh` 检查清单里三条 `tools:node="remove"` 都在，缺一条就失败。依赖链里只有 ML Kit 用 datatransport：`play-services-auth` 21.1.1、`play-services-base` 18.5.0、`play-services-fido`、`googleid` 和 `androidx.credentials` 的 POM 都不依赖它，仓库代码也不引用它。

不在本次范围：iOS 键盘扩展经 `MLKitCommon` 14.0.0 链接 `GoogleDataTransport` 10.1.0，只做了静态核对，没有实测，也没有关；`apps/desktop/src-tauri/gen/android` 的 Tauri 外壳同样依赖 ML Kit，但不进任何发布构建。

## Alternatives considered

- **只删 `TransportBackendDiscovery`，保留上传任务服务** — 最强的理由：旧事件的任务到点运行时 `Uploader` 找不到后端，按致命错误给事件记失败次数，攒够次数后删掉，旧数据能在本地被清走。否掉：要等多轮任务，每次 `:ime` 启动都可能重新排任务，`dumpsys jobscheduler` 里一直挂着一个 datatransport 任务，和“没有上传路径”的说法对不上；后端不存在时任务本来也发不出去，清理旧事件不值得留着这个入口。
- **在 Gradle 里排除 `transport-backend-cct` / `transport-runtime`** — 最强的理由：代码整个不进 APK，比清单删除更彻底。否掉：ML Kit 混淆后的类直接调用 `TransportRuntime.initialize`、`TransportRuntime.getInstance` 并 `new CCTDestination`，排除后 `:ime` 里会出现 `NoClassDefFoundError`，ML Kit 是否接住没有保证，可能拖垮手写。
- **找 SDK 自带的关闭开关** — 查过 APK 里的字符串，只有内部的 `MLKitLoggingOptions{enableFirelog=…}`，没有公开 API 或清单开关，不能依赖。

## Consequences

- **收益**：手写不再写诊断事件、不再排上传任务，`firebaselogging.googleapis.com` 从水杉的网络流量里消失；识别、模型下载和上屏不受影响。
- **代价与已知上限**：升级前已经存下的事件留在 `databases/com.google.android.datatransport.events`，不删也不发，升级后每次进程启动会多一条 `TransportRuntime.Executor` 错误日志，直到用户清数据。清单删除依赖 datatransport 用组件发现后端、用 JobScheduler 上传这两个实现细节；升级 ML Kit 或 datatransport 时必须重新检查合并清单并重做网络测量，换成别的上传方式时这道闸会失效。
- **相关**：OCR 试验阶段对 ML Kit 文字识别做过同样的遥测测量，后来换成 ONNX Runtime 并关掉了它的遥测，见 [Android 键盘内拍照与可选离线 OCR 模块](../feature/2026-10-24-android-offline-ocr.md)。

## Verification

- `aapt2 dump xmltree --file AndroidManifest.xml` 对比修复前后的合并清单：只少了上面三个组件（含 `backend:` 元数据），其余条目和 `INTERNET` 权限不变。
- 模拟器 `msime-issue-9601-api28`（API 28，arm64）全新状态装修复包：手写面板提示下载，下载经 `dl.google.com` 完成（约 25 MB 落在 `files/mlkit_digital_ink_recognition`），写字后上屏「十工」。期间及最后一次识别后 120 秒，logcat 只有 `Transport backend 'cct' is not registered`，`events` 表 0 行，`dumpsys jobscheduler` 没有该 UID 的任务，整机 tcpdump 只有 `dl.google.com` 和 `api.msime.app` 两个域名，没有 `firebaselogging.googleapis.com`。
- 同一模拟器先装修复前的包写字（存 3 条事件、排出 jobId `-674225152`），再覆盖安装修复包：进程启动时记一条 `No such service ... JobInfoSchedulerService`，被 `Background execution failure.` 接住，没有崩溃；之后写字上屏「工」，没有新事件和任务。
- 覆盖安装后第一次打开手写面板，下载按钮可能停在没有文字的“检查中”状态，结束进程后正常。真机上修复前的包被覆盖安装后也是这样，和本改动无关。
- 修复包没有在真机上验证：测量期间这台手机正被另一个会话用来装发布包，修复包会被它覆盖。

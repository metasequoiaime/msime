# Agent Note: iOS 手写调用 ML Kit 前关掉它的使用统计

Status: implemented

## Problem

iOS 键盘扩展的手写经 CocoaPods 链接 `MLKitDigitalInkRecognition` 8.0.0，它带进 `MLKitCommon` 14.0.0、`MLKitMDD` 10.0.0 和 `GoogleDataTransport` 10.1.0。`MLKitCommon` 里的 `MLKAnalyticsLogger` 在检查模型、下载模型和识别时记使用统计，是否记录由 `-[MLKAnalyticsLogger isStatsCollectionEnabled]` 决定：它从 `NSUserDefaults.standardUserDefaults` 读 `com.google.firebase.ml.<模块>.<上下文>.isStatsCollectionEnabled`，`objectForKey:` 为空时直接返回 YES，也就是没写过就当作同意；`logEvent:withName:` 和 `logThrottledEvent:withName:` 开头先问它，为 NO 时整条事件跳过。

2026-10-25 在 iPhone XR（iOS 18.7.10，商店版 0.52.0，键盘打开「允许完全访问」）上用 rvictl 镜像整机流量，tcpdump 写带进程元数据的 pcapng，同时跑 idevicesyslog。每次手写识别，扩展都打出这两行：

```
MSIMEKeyboardExtension(CoreFoundation)[pid] <Debug>: found no value for key com.google.firebase.ml.digitalInk.__MLKIT_DEFAULT.isStatsCollectionEnabled in CFPrefsSearchListSource<…> (Domain: app.m…
MSIMEKeyboardExtension[pid] <Fault>: -[MLKITx_SRLRegistry internalService:scopeTags:]: No binding was found for required, single-bound service: CCTPolicyVending_API
```

约 6 分钟里（写字、收起键盘让扩展进入后台、再写字），扩展没有发起任何网络连接，也没有 Google 或 Firebase 的 DNS 查询，抓到的只有 Apple 的系统守护进程。也就是说 ML Kit 每次都想记统计，只是上传要用的 Clearcut（CCT）传输没有链接进来，所以今天什么都没发出去。这是碰巧：升级 pod 可能把 CCT 绑定带进来，Android 的基础 APK 就一直在把 `FIREBASE_ML_SDK` 事件传到 `firebaselogging.googleapis.com`，见 [Android 手写关掉 ML Kit 的 datatransport 诊断上传](2026-10-25-android-mlkit-handwriting-telemetry.md)。

## Decision

`platforms/ios/KeyboardExtension/Sources/handwriting/MLKitStatsOptOut.swift` 的 `MLKitStatsOptOut.apply()` 往进程自己的 `UserDefaults.standard` 写四个 `false`：

- `com.google.firebase.ml.digitalInk.__MLKIT_DEFAULT.isStatsCollectionEnabled`
- `com.google.firebase.ml.mddDownloader.__MLKIT_DEFAULT.isStatsCollectionEnabled`
- `com.google.firebase.ml.commonDownloader.__MLKIT_DEFAULT.isStatsCollectionEnabled`
- `com.google.firebase.ml.vision.__MLKIT_DEFAULT.isStatsCollectionEnabled`

`HandwritingRecognizer.init()`（`HandwritingInputView.swift`，只编进真机构建）第一句就调用它，在 `DigitalInkRecognitionModel`、`ModelManager` 和 `DigitalInkRecognizer` 被碰到之前。扩展里只有这个类调用 ML Kit；设置 App 不链接 ML Kit（Podfile 只给 `MSIMEKeyboardExtension` 加了这个 pod），不需要写。

键名全部来自二进制，不靠猜：

- 格式串 `com.google.firebase.ml.%@.%@.isStatsCollectionEnabled` 在 `MLKitCommon` 的 `MLKAnalyticsLogger.o` 里，由 `+[MLKAnalyticsLogger keyForContext:loggerType:]` 用 `moduleNameFromLoggerType:` 的结果和上下文名拼出来。没有配置 Firebase 应用时上下文名是 `__MLKIT_DEFAULT`（设备日志里就是它）。
- `moduleNameFromLoggerType:` 是 7 项的查表，按 loggerType 0 到 6 依次是 `commonDownloader`、`digitalInk`、`linkFirebase`、`mddDownloader`、`naturalLanguage`、`remoteConfig`、`vision`。
- 链接进扩展的代码实际用到的 loggerType：`+[MLKDigitalInkRecognizer initialize]` 传 1（`digitalInk`），`-[MLKMDDLogger init]` 传 3（`mddDownloader`），`MLKitCommon` 的 `MLKModelDownloadLogger` 传 0（`commonDownloader`），`MLKCustomModelManager` 传 6（`vision`）。CocoaPods 给扩展加了 `-ObjC`，这几个类都会进最终的二进制。其余三个模块名没有调用点，不写。
- 公开头文件里没有关闭统计的 API，`MLKitDigitalInkRecognition` 和 `MLKitMDD` 也没有别的 data-collection 键。

键写在 `standardUserDefaults` 而不是 App Group：ML Kit 只读前者，设备日志里的 `Domain: app.m…` 就是扩展自己的域。每次创建识别器都重写一遍，代价可以忽略，用户清掉偏好或系统迁移后也能恢复。

## Alternatives considered

- **什么都不做，因为 CCT 传输没链接** — 最强的理由：实测今天一个字节都没发出去，加代码像是在防一个假设。否掉：没发出去全靠缺一个绑定，pod 的任何一次升级都可能补上它，而补上之后没有任何东西会提醒我们；Android 那边正是这样一路上传到被抓包发现。另外 ML Kit 在进程内照样组装事件、读取它的本机实例 ID，关掉开关后这些都不再发生。
- **像 Android 那样把上传部件去掉** — 最强的理由：从结构上没有上传路径，比依赖一个偏好更彻底。否掉：iOS 上上传部件本来就不在，缺的正是 CCT 绑定；`GoogleDataTransport` 是 `MLKitCommon` podspec 的硬依赖，要拿掉只能改写 podspec 或 fork pod，升级时更容易出错。两者也不冲突，以后 CCT 被带进来时，这个开关仍然先把事件拦在记录之前。
- **只写设备日志里看到的 `digitalInk` 一个键** — 最强的理由：这是唯一被实测证实的键，其余看起来像过度防御。否掉：模拟器上下载模型时 `mddDownloader` 也被读了，而 `commonDownloader` 和 `vision` 在链接进来的类里有确定的调用点，都不是猜的；没有调用点的三个模块名就不写。
- **写进 App Group 的共享 defaults，由 App 统一设置** — 最强的理由：App 和扩展一处管理。否掉：ML Kit 读的是 `standardUserDefaults`，共享 suite 它不看。

## Consequences

- **收益**：手写不再记 ML Kit 使用统计，将来 pod 升级把 CCT 绑定带进来，也不会有事件可发；识别和模型下载不受影响。
- **代价与已知上限**：依赖 ML Kit 内部的偏好键名和「为空即同意」的实现，`MLKitCommon` 改了格式串、上下文名或模块表就会失效且没有报错。升级 ML Kit 时要重新从二进制里核对格式串和 `moduleNameFromLoggerType:` 的表，并重做一次真机抓包加日志检查：识别时应看到 `looked up value 0 for key …isStatsCollectionEnabled`，不应再出现 `CCTPolicyVending_API` 的 Fault。
- **测试边界**：ML Kit 没有 arm64 模拟器切片，`HandwritingInputView.swift` 只编进真机构建，测试 target 也不链接 ML Kit，所以单测只能证明 `apply()` 写对了键，调用顺序由 `ProjectConfigurationTests.py` 按源码检查。

## Verification

- `platforms/ios/KeyboardTests/input/MLKitStatsOptOutTests.swift`：键表与四个字面值一致，`apply()` 后四个键在 `UserDefaults.standard` 里都是 `false`。
- `platforms/ios/tests/settings/ProjectConfigurationTests.py` 的 `test_mlkit_stats_collection_is_switched_off_before_the_first_mlkit_call`：App 的 Podfile 段不引入 ML Kit；`App`、`KeyboardExtension`、`KeyboardTestHost`、`SharedUI` 里只有 `HandwritingInputView.swift` 导入 MLKit；`HandwritingRecognizer.init()` 第一句是 `MLKitStatsOptOut.apply()`。去掉这一句时该用例失败。
- 真机 SDK 编译：在 Mac Studio 上 `pod install --deployment` 后先构建 `Pods-MSIMEKeyboardExtension`，再用 `xcodebuild -sdk iphoneos` 构建 `MSIMEKeyboardExtension`，`HandwritingInputView.swift` 和 `MLKitStatsOptOut.swift` 都编译通过；这次没有构建 Rust 宿主库，链接停在 `ld: library 'msime_host_api' not found`。
- 模拟器上跑真实 ML Kit：在 Mac Studio 上另建一个临时 App，用与仓库 `Podfile.lock` 相同的 pod 版本（`MLKitDigitalInkRecognition` 8.0.0、`MLKitCommon` 14.0.0、`MLKitMDD` 10.0.0、`GoogleDataTransport` 10.1.0、`GoogleUtilities` 8.1.0、`PromisesObjC` 2.4.0，链接参数与扩展相同），把三个 ML Kit 二进制 arm64 切片的 `LC_BUILD_VERSION` 平台从 iOS 改成 iOS Simulator，直接编译 `MLKitStatsOptOut.swift`；在 iOS 26.5 模拟器（iPhone 16）上全新安装后下载 `zh-Hani-CN` 模型并识别三次「中」的笔迹，用 `log stream --level debug` 只看这个进程。不写开关时，查模型、下载和第一次识别各读一次 `digitalInk` / `mddDownloader`，日志为 `found no value for key com.google.firebase.ml.digitalInk.__MLKIT_DEFAULT.isStatsCollectionEnabled in CFPrefsSearchListSource<…>`，每次读完都跟一条 `F … -[MLKITx_SRLRegistry internalService:scopeTags:]: No binding was found for required, single-bound service: CCTPolicyVending_API`，共 6 条，另外读了 `com.google.firebase.ml.instanceID`；与设备上看到的一致。卸载重装后先调用 `apply()` 再跑同一流程：同样的位置变成 `looked up value 0 for key com.google.firebase.ml.digitalInk.__MLKIT_DEFAULT.isStatsCollectionEnabled in CFPrefsPlistSource<…> (Domain: app.msime.ios.mlkit-stats-harness, …)`，`CCTPolicyVending_API` 的 Fault 为 0 条，`instanceID` 不再被读，识别结果仍是「中,牛,口中」。这一轮没有读到 `commonDownloader` 和 `vision`。
- 没有在真机上验证修复后的构建：这次按要求没有使用手边的两台 iPhone。模拟器这一轮也没有抓网络流量，CCT 绑定不存在时本来就发不出去，这里验证的是事件在记录前就被拦下。

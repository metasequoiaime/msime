# Agent Note: Android 键盘内拍照与可选离线 OCR 模块

Status: implemented

## Problem

#9207 要求在键盘区域完成相机预览、拍照、离线识别和选字上屏。报告人的补充明确要求模块不进入基础安装包，由用户从本项目 GitHub 下载后手动导入。外部相机应用、仅导入图片或 Google Play 按需下载不能代替该流程。

## Decision

设置页提供「卸载配套模块」，确认后经共享的资源包删除接口只移除已发布的 OCR 包，下载的原始文件及其他输入法数据保留。导入／卸载中禁用冲突操作，失败可重试；卸载成功清掉之前的导入失败提示。JNI 只传有界请求，不另写目录删除。旧版模块也可卸载；用户再次导入同一固定模块后仍能识别。输入法下一次打开 OCR 时重新验证安装状态，即使类加载器有缓存也不能使用已经卸载的包（#10235）。

拍照识字设置页提供「下载地址」（#10242）：从共享资源包的 `importFiles` 读取当前宿主锁定的模块地址，以系统 `ACTION_VIEW` 选择器交给浏览器或第三方下载工具。下载完成后用户仍经原有文件选择器导入，模块身份与完整摘要校验由共享层持有。Google Play 渠道、模块目录不可用或没有下载地址时禁用按钮；设备没有可处理地址的应用时提示重试：系统选择器在没有匹配应用时只显示系统自己的空列表、不会抛异常，所以先用 `queryIntentActivities` 判断，清单 `<queries>` 为此声明了 https 的 `VIEW`，否则 Android 11 起该查询恒为空。地址不在 Java 里另存一份，避免宿主升级模块锁后还引导下载旧模块。

下载入口的验证：Mac Studio 上 `bash scripts/verify-local.sh --quick` 通过，设置 `ANDROID_HOME` 后 Gradle `:app:compileFullDebugJavaWithJavac` 成功；本机 `pnpm run verify-notes` 通过。GitHub 发布附件的文件名、大小和地址与模块锁一致。没有在设备上点选第三方下载工具，具体工具对该 HTTPS 附件地址的接管仍需设备复验。

可选模块 `msime-ocr-v1.apk`（`platforms/android/gradle-app/ocr-module`）的识别引擎是 ONNX Runtime 1.31.0（`com.microsoft.onnxruntime:onnxruntime-android`，MIT）加 PaddlePaddle 的 PP-OCRv6 tiny 检测与识别模型（Apache-2.0）。ONNX Runtime 只是模块的依赖，基础 APK 不带它，也不带模型；选型的测量与放弃的方案见下文[引擎选型](#引擎选型)。最初的可行性试验用的是 Google ML Kit 的 `text-recognition-chinese:16.0.1`，下文「产品接入边界」「产品设备证据」里各轮设备运行都是那个试验模块，宿主一侧的导入、选择器、相机、撤权与手写共存路径与引擎无关，换引擎时没有改动。

模块按上游 PP-OCR 管线自己实现前后处理，不依赖 OpenCV：检测输入长边限 960、两边取 32 的倍数、BGR 顺序按均值方差归一化成 NCHW；DB 后处理按 0.2 二值化，8 邻接连通域取凸包和最小外接矩形，框内均值低于 0.4 的丢弃，再按 `面积 × 1.4 / 周长` 外扩（三个阈值取自固定提交的 `inference.yml`）；用 `Matrix.setPolyToPoly` 透视裁出每行，边界外按边缘像素延伸，行高不小于宽 1.5 倍的竖排行逆时针转 90°；识别输入高 48、宽按比例（至少 320、至多 3200）补零，CTC 贪心解码。纯 Java 的部分在 `OcrPipeline`，由 `platforms/android/check-host.sh` 的 JVM 冒烟 `tests/ocr/OcrPipelineSmoke.java` 覆盖。

阅读顺序与上游不同。上游 `sorted_boxes` 只按左上角从上到下、行内从左到右排，评测里两首竖排五言诗都被读成先左列；中文竖排从右往左读，所以多数框是竖排时按列从右到左、列内从上到下。分行（分列）比四个角的平均坐标，阈值是框自身行高（列宽）的一半：上游写死的 10 px 在大分辨率照片上会把同一行拆开，而用轴对齐外框做阈值时拍歪的长行外框远大于行高，第一版实现就因此把一段英文的前两行并成一行、按 x 排反，41 张评测图的重跑发现了这一点。横竖混排时按多数框的方向排，少数方向的框按同一规则插入，没有单独处理。

会话参数是硬性要求：关掉 CPU 内存池与内存模式（默认开着时输入法进程峰值 PSS 330–440 MB，关掉后约 121 MB）；会话配置 `mlas.disable_kleidiai=1`（Apple 芯片上的 arm64 模拟器，包括本项目的 CI，报告 SME 特性却不支持，KleidiAI 在建会话时执行 `rdsvl` 触发 SIGILL，整个进程崩溃）；线程数限 2–4，`session.intra_op.allow_spinning=0`，识别结束后线程不再自旋占 CPU。`OrtEnvironment` 在模块类加载器内是进程单例，创建时 `setTelemetry(false)`；每个 `OcrModule` 实例在 `initialize` 建检测与识别两个会话，`close` 释放，重开不重建环境。模型和字典从模块 assets 读成 `byte[]` 交给 ONNX Runtime，输入法进程里不写文件；ONNX Runtime 的 Java 层在 Android 上只用 `System.loadLibrary`，不像桌面那样把原生库解压到临时目录。

onnxruntime-android 的 AAR 自带向 Microsoft 上报的 1DS 遥测：清单合并进 `ai.onnxruntime.TelemetryInitializer`（ContentProvider）和网络权限，Provider 在应用启动时初始化 Java 端的 `HttpClient` 发送通道，原生层经它把事件发往 `mobile.events.data.microsoft.com`。1.31.0 里 `HttpClient` 调用的 JNI 方法 `isDeviceIdCollectionEnabled` 在原生库里找不到，通道初始化抛 `UnsatisfiedLinkError` 而失败，这只是今天碰巧不发，不能当作保证。模块清单删除这个 Provider 和 `INTERNET`、`ACCESS_NETWORK_STATE`（宿主经类加载器使用模块，Provider 本来就不会在宿主里运行；独立安装的调试模块同样没有它），并在创建环境时关掉原生一侧的遥测。设备上的实测见下文，识别和其后 65 秒内该 UID 没有任何套接字，抓包里也没有设备发出的流量。

模型在构建时下载：`resources/ocr-models-android.lock.json` 是只供构建的锁，固定 Hugging Face 上 `PaddlePaddle/PP-OCRv6_tiny_det_onnx` 提交 `2ba1506c0380b8f0b03dd142459aac66d4421f6c`、`PaddlePaddle/PP-OCRv6_tiny_rec_onnx` 提交 `2612ab37152ae0a677521bae4e1e3d4fb4cf7c30` 的 `inference.onnx` 与 `inference.yml` 的长度和 SHA-256（与 Hugging Face API 报告的 LFS 摘要一致），从识别模型 `inference.yml` 抽出的 6904 条 CTC 字典的 SHA-256，检测后处理参数，以及三份许可证全文的上游地址和摘要。`scripts/fetch_ocr_models.py` 由模块的 Gradle 任务调用，限长下载、校验后才落盘；字典只按这份 yml 的固定形状抽取，遇到不认识的写法就失败，抽出结果与 Ruby Psych 解析整份 YAML 的结果逐字节相同。它不往 `resources/ocr-module-android.lock.json` 加字段：那份锁由 Rust 以 `deny_unknown_fields` 解析，是运行时唯一的信任依据，只固定整个模块 APK。

发布由手动触发的 `publish-android-ocr-module.yml` 完成：构建 release 模块，检查每个 `.so` 是 STORED、`zipalign -c -P 16` 通过、两套 ABI 和模型、许可证资产都在、不超过 128 MiB，算出 SHA-256，`publish` 勾选时上传到 `ocr-module-android-v1` 标签的 `msime-ocr-v1.apk`，标签下已有同名资产时拒绝替换，并把应提交的锁 JSON 写进运行摘要。宿主不看模块签名，发的是 AGP 输出的未签名 release 包。

候选模块接口只传递 Android `Context`、`Bitmap` 和文本行，不暴露引擎类型。基础 APK 不依赖 OCR 模型和运行库；普通 Gradle 构建不包含试验模块。正式导入需要核验项目分发的模块身份、接口版本、ABI 和完整性，限制读取大小，先发布完整版本再切换，输入法进程只读。相机仅在用户打开面板并授权后工作，关闭面板或切换编辑器必须停止相机、作废识别任务并拒绝迟到结果。识别内容只在当前面板中待选，不写日志或自动复制。

模块不带 ML Kit，父加载器是启动类加载器，宿主手写依赖的 ML Kit 容器和自动初始化 Provider 与 OCR 互不可见；隔离探针断言模块加载器载不到 `MlKit`。探针识别断言同时要求合成英文单词、数字和中文，不能只认出数字就报告英文识别成功。

ML Kit 试验模块阶段，宿主清单为 OCR 在 `MlKitComponentDiscoveryService` 下声明过 `VisionCommonRegistrar`、`TextRegistrar`，探针清单也照样声明；换成 ONNX Runtime 后这两项只为 OCR 存在，已从宿主和探针清单删除，合并后的清单里只剩手写依赖自己带来的组件。手写的实际推理在 OCR 前后由产品专项和真实 `:ime` 手写专项验证，结果见下文。

多字号探针在用例之间关闭并新建模块实例，覆盖连续打开工具的生命周期，而不是只测进程内第一次调用；ML Kit 试验模块曾在这里因为重复初始化 SDK 容器失败，ONNX Runtime 模块共用一个环境、只重建会话。

参考实现固定为 `LeanBitLab/LeanType@be774536dfb415634da176831852e0b8eed247dc` 及 `LeanBitLab/LeanType-OCR-Plugin@968949a7b54b8f0e3a4a5063ca03812554cf23ef`，仅用于研究，不复制未提交源码。其插件使用 ML Kit 私有初始化和识别文本日志，不沿用这两点。公开初始化接口见 [Google MlKit API](https://developers.google.com/android/reference/com/google/mlkit/common/MlKit)；模型安装方式见 [文本识别 Android 指南](https://developers.google.com/ml-kit/vision/text-recognition/v2/android)。

## 引擎选型

评测在 Apple 芯片主机上的 arm64 API 35 模拟器（`msime-client-test-api35-issue9124-live`）里进行：41 张合成图（25 张中文，其中 3 张竖排；16 张英文），系统字体渲染后加模拟拍照的模糊、噪声、透视和光照退化。模块按 `OcrRuntime` 的方式从只读 APK 经 `DexClassLoader` 加载，宿主进程有网络权限。CER 是 NFKC 规范化、去掉空白后的编辑距离；延迟是三轮常温识别的中位数与 p90；峰值 PSS 是每秒一次 `dumpsys meminfo` 的最大值。

| 方案 | 中文 CER | 横排中文 CER | 竖排全对 | 英文 CER | 常温中位 / p90 | 峰值 PSS | 双 ABI 模块 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ML Kit `text-recognition-chinese:16.0.1` | 8.64% | 4.42% | 0/3 | 1.07% | 15.9 / 24.7 ms | 111.5 MB | — |
| PP-OCRv6 small，内存池关 | 3.39% | 0% | 1/3 | 0.40% | 118.5 / 260.3 ms | 184.5 MB（开 442.4） | 102 MiB |
| PP-OCRv6 tiny，内存池关（评测原型） | 4.07% | 0.71% | 1/3 | 1.07% | 49.8 / 117.1 ms | 121.0 MB（开 328.8） | 约 78 MiB |
| PP-OCRv6 tiny，本模块 | 0.68% | 0.71% | 3/3 | 1.07% | 46.5 / 91.5 ms | 93.7 MB（11 个采样） | 77.4 MiB |

本模块一行用同一个评测宿主重跑：横排中文和英文的每张输出与评测原型逐字相同，差别只在竖排列序修正后三张竖排全对，中文 CER 从 4.07% 降到 0.68%。剩下的错误是 `巳` 认成 `已`、`……` 多一个点、`Il1-O0o` 一类易混字符和句末的点。建会话约 100–115 ms，首张图约 110 ms。模块 `msime-ocr-v1.apk` 81,146,926 字节，其中 `lib/arm64-v8a` 34,300,736 字节（32.7 MiB）、`lib/x86_64` 40,916,520 字节（39.0 MiB），模型、字典和许可证压缩后约 5.8 MB。

ML Kit 的遥测证据：识别时 logcat 记下多条 `Storing event ... name=FIREBASE_ML_SDK for destination cct`，`shared_prefs/com.google.mlkit.internal.xml` 里写入 `ml_sdk_instance_id`，开始识别约 50 秒后该 UID 出现到 `172.217.117.4:443` 的连接，同时段抓包里出现 `firebaselogging.googleapis.com`。同一流程下 tiny 模块的 103 个每秒套接字采样全部为空。基础 APK 的手写 ML Kit 走的是同一条 datatransport 上传，处理见 [Android 手写关掉 ML Kit 的 datatransport 诊断上传](../bug-fix/2026-10-25-android-mlkit-handwriting-telemetry.md)。

## ONNX Runtime 模块的设备验收（2026-10-25）

全部在 arm64 API 35 模拟器 `emulator-5690` 上，模块由本分支在 Mac Studio 构建，产品包按 release 模块的完整摘要写临时锁后构建（`MSIME_ANDROID_ABIS=arm64-v8a` 的开发包），本机用发布密钥重签后侧载；临时锁没有提交。

- 隔离探针（`OcrProbe`）：无网络权限的探针 App 经 `DexClassLoader` 加载模块，三种字号的中英文合成图全部与简体原文完全一致（ML Kit 时代 API 28 有 `文/丈`、API 35 有 `測/测`），竖排两列右列在前；模块加载器载不到 `MlKit`；OCR 前后宿主手写客户端可创建。普通安装方式的对照（`DirectOcrProbe`）结果相同，四张合成图逐字节一致，两边识别结果 JSON 逐字节一致。
- 产品专项（`OcrProductDeviceSmoke`，按 `ocr-product-smoke.sh` 的设备阶段逐项运行）：联网准备手写模型；断网后系统文件选择器的取消、损坏导入拒绝、有效导入后识别、取消替换与坏模块替换后原模块仍可识别；固定摘要导入、只读封装、可写或篡改拒绝、相机拍摄合成图后整行上屏与只选 `2468` 上屏、隐藏后重开相机、跨编辑器迟到结果拒绝；撤权后入口只显示授权提示；同一宿主手写识别器在 OCR 前后识别「中」结果一致；真实离线 `:ime` 里相机 OCR 前后手写识别、上屏、撤销和画布恢复，PID 不变。结果 `coexistence=0 picker=0 product=0 ime_handwriting=0`；结果、上屏和部分选择截图已逐张核对。
- 两份 ONNX Runtime 共存：产品开发包自带 sherpa-onnx 的 `libonnxruntime.so`。专项在 OCR 前调用 `NativeClient.localSpeechAvailable()` 载入语音运行库（返回 true），识别后它仍可用；logcat 里同一进程既有先 OCR 后语音、也有先语音后 OCR 的顺序，没有崩溃或 SIGILL。模块的 `libonnxruntime4j_jni.so` 从 `msime-ocr-v1.apk!/lib/arm64-v8a` 载入独立命名空间 `clns-8`。
- 网络：评测宿主（有 `INTERNET` 权限）跑一轮 41 张识别后再停 65 秒，73 个每秒采样里该 UID 在 `/proc/net/{tcp,tcp6,udp,udp6}` 没有任何套接字；同时段模拟器抓包只有两条虚拟路由器发出的 IPv6 路由通告，没有设备发出的包。模块清单合并后没有权限和 Provider。
- 这次没有跑共享 crate 的设备单测和 `smoke.sh` 基础冒烟（本 PR 不改 Rust 与基础键盘），也没有跑 API 28、x86_64 和物理真机。

## 已发布模块的真机验收（2026-10-25）

设备是维护者的 Pixel 3 XL（Snapdragon 845，Android 12 / API 31，arm64-v8a）。产品包是固定了正式锁的 `MSIME_ANDROID_ABIS=arm64-v8a` 开发包，在 Mac Studio 构建、本机用发布密钥重签后 `install -r` 覆盖安装，用户数据保留。模块是从 `ocr-module-android-v1` 下载、核对长度和 SHA-256 后的 `msime-ocr-v1.apk` 原件，不是本地构建。

- 导入：模块推到手机的 Download 目录，在拍照识字设置页点「选择已下载的配套模块」，经系统 DocumentsUI 选中后一秒内显示「配套模块已导入。」。导入前页面显示「尚未导入模块…」，即宿主认出了锁里固定的产物（旧锁的包显示「配套模块尚未提供」）。同长度、改了一个字节的副本走同一路径，显示「导入失败：下载的文件校验未通过，请重试」。
- 隔离探针：`ocr-runtime-probe.sh` 只接受模拟器，设备阶段用仓库外的临时脚本在真机上逐项复现，探针包内嵌的模块换成已发布的 release 原件。结果 `MSIME_OCR_PROBE_PASSED`；直接安装源码 debug 模块的 `DirectOcrProbe` 为 `MSIME_OCR_DIRECT_PASSED: complete_cases=3/3,vertical_right_to_left=true`，四张合成图逐字节一致，两边识别结果 JSON 逐字节一致。
- 输入法进程内的测量：仓库外的临时 instrumentation 用 `android:targetProcesses="app.msime.android:ime"` 跑在 `:ime` 进程里，直接调用产品的 `OcrRuntime.open`（全模块 SHA-256 复核、隔离类加载器、建两个会话）和 `recognize`，输入是上文引擎选型用的 41 张合成图。

| 项目 | Pixel 3 XL 结果 |
| --- | --- |
| 新进程首次 `OcrRuntime.open`（含 81 MB 摘要复核、类加载、建两个会话） | 7 次 328–391 ms，其余 6 次都在 328–343 ms |
| 同进程再次 `open`（每次拍照都会走一遍） | 270–316 ms |
| 新进程首张图识别 | 314–341 ms（每秒 `dumpsys meminfo` 采样时 736 ms） |
| 常温识别，41 张 × 3 轮 | 中位 280.4 ms，p90 541.7 ms；带内存与网络采样那轮 266.0 / 516.5 ms |
| 中文 CER / 竖排全对 / 英文 CER | 0.68% / 3/3 / 1.07%，逐图输出与模拟器相同 |
| `:ime` 峰值 PSS | `dumpsys meminfo` 每秒采样最大 136.7 MB（42 个采样）；键盘内拍照那次最大 150.4 MB，键盘常驻约 105 MB |

- 键盘内拍照：在真实键盘「更多 → 拍照识字」里经系统对话框授权相机、预览、拍照，拍照后相机立即释放，模块对真实相机 JPEG 跑完识别回到面板。手机平放在桌上，后置摄像头对着桌面，照片全黑，结果是「未识别到文字，请调整距离或光线。」。所以真机上没有验证识别出文字之后的整行、部分上屏；这段宿主代码与引擎无关，已在 arm64 API 35 模拟器上通过。也没有真实照片的识别数据。
- 网络：每 0.5 秒读一次 `/proc/net/{tcp,tcp6,udp,udp6}` 里属于水杉应用 UID 的行，前后各 `dumpsys netstats --poll` 一次。只跑 `:ime` 测量、不开界面的那轮（识别 55 秒加其后 60 秒，207 个采样）没有任何套接字，该 UID 的收发字节不变。键盘内拍照那轮，打开拍照面板前就有三条到 `104.21.44.153:443` 的已建立连接，识别后约 55 秒又建了一条，地址都是项目自己的 `api.msime.app`；`/proc/net` 分不出是哪个进程开的，主进程和 `:ime` 共用这个 UID。两轮都没有到其他地址的连接，没有到 Microsoft 遥测端点的连接。
- 拍照那轮的「之后 60 秒」没跑满：识别后约 63 秒，有不属于本次验收的操作经 adb 推送并 `pm install -r` 了另一个 `app.msime.android` 包，进程被强停。

## 产品接入边界

以下各节到「产品设备证据」为止记录的是 ML Kit 试验模块阶段的接入与设备运行；宿主一侧的路径换引擎后没有改动，ONNX Runtime 模块的验收见上一节。

实际输入法共存专项复用现有 `HandwritingDeviceSmoke` 和原始触摸桥，在已准备模型且断网的专用 AVD 中，通过手写控件识别完整合成「中」字、打开键盘内相机识别固定图片、取消结果后再识别「中」。前后核对 `app.msime.android:ime` 的 PID，防止重启进程后通过被误当作同进程共存；取消 OCR 后编辑器必须保持此前的手写提交。触摸桥抽为共用设备脚本，普通手写验收与 OCR 专项使用同一条真实输入路径，不在两个脚本各维护一份。该路径在下述 `b78178a731` 两版设备运行中通过，仍不代表真机和照片质量通过。

[第十四轮 API 28](https://github.com/metasequoiaime/msime/actions/runs/40103254491)的模块清理、系统选择器、原生相机、撤权和同一宿主手写识别器前后推理通过；实际 `:ime` 界面专项尚未开始，触摸桥因写死 `virtio_input_multi_touch_1` 而找不到设备。触屏选择改为读取 [AOSP `getevent -lp` 能力表](https://source.android.com/docs/core/interaction/input/getevent)，要求同时具有 slot、tracking ID 和 X/Y 坐标；仍优先既有主显示触屏名，排除其他编号的虚拟显示触屏。没有该名称时只接受唯一的能力匹配设备，多个匹配、缺少能力或只有副显示触屏均失败，不猜编号。专项保存专用 AVD 的能力元数据，不记录实时输入事件。八份合成能力表验证旧名称、主副显示、CRLF、缺能力和歧义拒绝；新选择器仍待两版设备验证。

[第十五轮固定合并源 `13931a`](https://github.com/metasequoiaime/msime/actions/runs/40103451241)复现相同结果：API 28 的四个共享 crate、系统选择器、相机、撤权与主进程手写前后推理通过，真实输入法专项仍未进入 instrumentation；API 35 的四个共享 crate、模型准备与相机通过，选择器显示空 Downloads，撤权后主进程没有重新启动，手写方案名已更新但键区仍为 QWERTY。两版基础冒烟均停在十八键窄布局宽度断言；重新核对确认验证提交 `9659ced22c` 与固定合并源 `13931a` 全树仅差专用 workflow。它与第十八轮当前产品验证使用相同 workflow、几何断言、模拟器启动脚本及临时 OCR 模块摘要，且两版基点与当前 API 35 的三个 `Rect` 完全相同，宽度均为 `69/66/69px`。这只证明本 PR 相对 `13931a` 的后续八个路径未新增这项失败；`13931a` 已包含此前 OCR 实现，不能据此断言整个功能没有回归，该布局失败仍未修复。完整 quick 仍有相同的 91 条失败，无新增，不等于门禁通过。

偏好更新的布局比较改用 `ImeLetterRows.drawnTouchLayout()` 与目标快照：模式切换可能先更新会话 `view`，比较两份逻辑快照无法保证屏幕键区跟上；与按键响应路径的既有规则一致，不用重启输入法绕过共存要求。API 35 失败截图证明了方案名与键区不一致，但尚未证明这条时序是唯一成因，修正以设备重跑为准。系统选择器改点控件或同包可点击父节点的 `ACTION_CLICK`，仍实走系统 URI 授权；不以坐标点击后返回成功替代合成文档实际出现。撤权后的显式启动使用 `NEW_TASK | CLEAR_TASK` 清掉专项旧活动栈，保留主进程 PID 检查；[Android Intent 文档](https://developer.android.com/reference/android/content/Intent#FLAG_ACTIVITY_CLEAR_TASK)规定它与 `NEW_TASK` 配合使用。不延长等待或放行未启动的进程。这些修正均待 API 28/35 设备验证。

[第十六轮 API 35](https://github.com/metasequoiaime/msime/actions/runs/40106018014)的系统选择器导入、取消和坏模块替换、相机、独立撤权与同一宿主手写识别器前后推理均通过，证明点击与任务重建修正已进入设备路径。真实输入法专项已出现手写画布，失败于等待首选候选；失败截图的合成编辑器其实已收到「中」。`ImeLayoutRows.onHandwritingRecognized` 和 `AndroidLocalSettings` 的默认叠写模式会自动提交并清空画布，旧测试仍按单字模式等待候选，前提不成立。夹具改为通过原生设置页明确选中单字模式，重新打开页面确认保存，在测试结束后恢复原模式；不改产品默认值，不以自动提交的截图代替原先的精确候选、手动上屏和同进程共存断言。[第十七轮 API 35](https://github.com/metasequoiaime/msime/actions/runs/40108526752)确认单字前提成立，真实 `:ime` 在相机 OCR 前后均识别出「中」，手动空格、回车、硬件空格上屏及退格撤销也通过，PID 未改变；系统选择器、相机、撤权和宿主识别器共存通过。专项随后在符号入口失败：手写底行是长按 `123` 打开面板，旧夹具却查找只属于横排数字层的「切换符号键盘」。夹具改走与现有标点设备套件一致的 `ACTION_LONG_CLICK`，继续检查面板打开、关闭及手写画布恢复，不放宽候选、上屏或 PID 断言；此入口修正仍待设备重跑。[第十八轮 API 35](https://github.com/metasequoiaime/msime/actions/runs/40111440626)的四个共享 crate、系统选择器、相机、独立撤权、宿主推理共存，以及同一个真实 `:ime` 的手写、上屏、撤销、符号层与画布恢复均通过；十八键宽度断言仍失败，不能称完整门禁通过。第十七轮 API 28 已被取消，不计通过；此前记录的运行状态不代表最终成功，第十八轮 API 28 仍在运行。

第十七轮 API 28 的系统选择器已显示合成模块，但文档节点与同包父节点均不提供可点击动作，夹具停在 `choose synthetic document`。固定 `13931a` 的 API 28 通过真实坐标触摸完成过该专项；当前 API 35 的动作点击也已通过。因此只在 API 28 的合成文档条目恢复触摸：严格匹配系统选择器包、固定文件名、启用状态及稳定的非零可见边界；根抽屉和提供方仍走动作点击，避免重引入此前抽屉动画误点。仍要求实际 URI 导入、损坏拒绝、取消和旧模块推理的全部断言，修正尚待设备验收。

文件选择器专项从原生导入页打开系统 `ACTION_OPEN_DOCUMENT`，通过独立测试 UID 的只读 `DocumentsProvider` 选择有效模块或同长度损坏模块。该提供方只读专项 APK 自带的固定资产，普通测试包不显示此根，不接触用户外部存储。分别检查初次取消、损坏导入、有效导入后实际 OCR、取消替换和坏模块替换后原模块仍可推理；不直接调用活动回调或用内部安装接口冒充系统 URI 授权。专项失败独立保留，不阻止相机、撤权和真实手写共存记录。此路径尚待两版设备运行，不能据源码或编译宣称验收。

独立共存专项单独准备宿主已有的中文手写模型，准备阶段允许联网，随后关闭模拟器网络。专项保留同一个宿主手写识别器，在可选 OCR 模块导入、初始化、重开和篡改拒绝前后识别同一组合成「中」字笔画，要求包含完整目标字且两次候选一致。测试通过反射调用产品的公开手写适配接口，不向测试 APK 再捆绑一份 Google SDK，不以伪造模型回包替代推理。模型准备和共存各自记录状态，不能覆盖原生相机、撤权或基础冒烟的失败。这验证目标应用进程内两个 SDK 容器的实际推理共存，仍不能替代 `:ime` 手写界面与真机验收。

第十轮设备运行中，API 28 的真实手写模型准备、相机 OCR 和撤权专项通过，共存专项在清理上次合成模块时发现 ART 自动生成的 `oat/x86_64`，尚未进入手写推理。夹具仅把该模块对应的 `odex`、`vdex`、`art` 加入固定路径白名单，逐级用 `lstat` 拒绝符号链接及其他条目；不放开任意目录删除。该修正后的第十一轮 API 28 已重跑，结果见下段；API 35 尚未收齐。

第十一轮 API 28 的四个共享 crate 设备套件、完整 `--core` 冒烟、模型准备、相机 OCR 和撤权均通过，共存仍在未知清理条目断言失败，不能称已推理。AOSP 的 [Android 9 `DexLoadReporter`](https://android.googlesource.com/platform/frameworks/base/+/android-9.0.0_r20/core/java/android/app/DexLoadReporter.java) 与 [Android 15 同一入口](https://android.googlesource.com/platform/frameworks/base/+/android-15.0.0_r1/core/java/android/app/DexLoadReporter.java)还定义了 `oat/<APK>.cur.prof`，Android 15 另定义 `oat/<APK>.prof`；[Android 9 `OatFileAssistant::Lock`](https://android.googlesource.com/platform/art/+/android-9.0.0_r20/runtime/oat_file_assistant.cc)定义 `<APK>.x86_64.flock`。夹具额外接受当前合成模块对应的这三个精确路径，仍要求普通文件；拒绝时只显示受限的合成模块派生文件名，其他名字隐藏。源码证明这些是平台合法产物，尚未由该轮设备目录清单证明具体未知条目是哪一个，后续以重跑结果为准。

[第十三轮产品专项](https://github.com/metasequoiaime/msime/actions/runs/40101164998)中，API 28 的系统文件选择器完整导入、取消与损坏替换均通过；相机与主进程共存仍在清理未知 ART 条目时失败。API 35 的模型准备、原生相机 OCR 通过，文件选择器失败于损坏导入步骤，撤权后的 `--no-restart` 又触发系统 `ProcessRecord` 空指针，尚未执行后续专项。脚本现在在每次附着前启动并确认主进程，撤权后先显式停止旧进程，instrumentation 命令异常也写入失败标记并继续其他专项；选择器操作细分阶段，便于下一轮定位。两版四个共享 crate 设备套件通过，基础冒烟均停在十八键窄布局的三排字母键宽度断言；不能称完整门禁通过。这些新夹具修正仍待设备验证。第十一轮 API 28 的剩余原图已逐张检查：部分选择只插入 `2468`，恢复预览显示完整源图，撤权后无预览；`hidden` 与 `switched` 是 HOME 检查点，不是最终跨编辑器截图。

`resources/ocr-module-android.lock.json` 的 `artifact` 是宿主是否提供这个功能的唯一开关。为 `null` 时共享层不列出、不安装、不导入模块，设置页显示「配套模块尚未提供」，不能选择任意 APK 绕过检查。现在锁固定的是 `publish-android-ocr-module.yml` 从提交 `8e4c92657de3b2163297c122668059fbb2c746a9` 构建并发布到 `ocr-module-android-v1` 的 `msime-ocr-v1.apk`（81,146,926 字节，SHA-256 `1e08af23485a1abccfebe1ac1b9289ea621dae62d47690d1c52cca17b8eef8cf`）；这之后构建的 Android 包列出这个资源包、只接受这一份字节，共享层的 `install` 仍拒绝自动下载它。资源包测试 `the_published_ocr_module_is_offered_for_manual_import_only` 覆盖已固定的状态（列出、导入文件清单、拒绝下载、同长度错误字节不发布），`an_unpinned_pack_cannot_be_listed_imported_or_verified` 用永远不固定的白霜语法模型覆盖未固定的状态。

导入复用 `resource_packs::import` 的固定长度、完整 SHA-256、限长暂存和整体替换。正式锁由源码审查固定本项目 GitHub Release 的已审计产物，完整摘要才是运行时的信任依据；接口版本为 1，仅支持宿主已有的 `arm64-v8a`、`x86_64`。主进程导入后将 APK 封为只读；输入法加载前通过新增的 `resource_pack_verify` 再读全量摘要，并拒绝有写权限的 APK。暂存清单和文件选择器提供的名字均不能决定信任。Play 分发不提供外部可执行模块，Java 安装入口和运行库入口也拒绝该渠道，不能只隐藏按钮。

原生库从只读、未压缩且已对齐的 APK 内部路径映射，不在输入法进程中解压或写文件。这个路径仍需 API 28/35 产品验收，若任一 API 无法加载，不能以扩大动态代码写入权限绕过。隔离类加载器在进程内复用，识别器按任务关闭；不触碰原宿主的手写单例。

共享资源包枚举也由桌面设置编译。桌面的 `offered_by` 明确拒绝 Android OCR 模块，三个桌面平台的既有资源包筛选回归同时覆盖它，避免新增枚举造成桌面编译遗漏或错误提供模块入口。

面板使用专用相机线程，拍照后释放相机，图片仅在内存中按尺寸限制解码并处理 EXIF 方向。识别结果绑定面板代次和输入连接；`onStartInput` 的保留组字早返也先作废 OCR。关闭面板、窗口隐藏、`onFinishInputView`、停止会话和销毁都会释放相机并清空结果。用户可以勾选识别行或长按选取部分文字，确认后才经现有上屏入口插入。

设备验证使用当次构建模块的完整摘要生成临时源码锁，随后构建原生产品；这个测试清单不提交到产品锁文件，不把合成模块伪称为已发布资产，也不在运行时添加任意路径或跳过校验的测试开关。测试清单里的保留域名 `example.test` 仅标识合成夹具，共享层和 Java 层都拒绝 OCR 自动下载，不会访问该地址。模拟器通过官方 `-camera-back imagefile:<path>` 输入固定合成图，实走相机拍照回调，而不以预设的识别文本代替图片推理。专项另经可访问性接口选择识别文本中的固定数字并只上屏所选部分，保留独立截图；预览期间隐藏键盘后重新打开相机，以区分面板关闭释放和拍照完成释放。撤回相机权限会终止目标进程，因此脚本在两次 instrumentation 之间撤权，再检查原生入口显示授权提示且不创建相机预览。这些检查均以设备实际结果为准，源码存在不等于已经通过。

## 产品设备证据

[API 28/35 产品专项](https://github.com/metasequoiaime/msime/actions/runs/40091988383)使用验证提交 `a3335cee26`，与修正 PR 的 `335988b93d` 完整源码树只差专用工作流。两版均通过四个共享 crate 设备单测、固定摘要导入、只读 APK 原生库映射、完整中英文静态合成图识别、重开、错误替换和篡改拒绝。模拟器经相机 API 拍摄固定合成图，完整识别 `SYNTHETIC OCR 2468` 并整行上屏，另只选择 `2468` 四字符精确上屏。隐藏键盘后重新取得相机并收到预览帧，离开输入框后迟到结果既不能插入新编辑器，也不能恢复旧 OCR 面板；独立撤权专项也通过，入口显示授权提示且没有相机预览。两版的照片、预览、结果、整行与部分上屏、隐藏、恢复及撤权八张截图均已逐张检查。

空编辑器的无障碍 `getText()` 可能返回提示词，跨框断言因此同时检查 `isShowingHintText()`、固定 `getHintText()` 和返回文本，不放行真实输入。HOME 转场中无障碍窗口先于退场动画更新，必须等待桌面与键盘状态稳定再截图、重开编辑器。隐藏和切框分别写截图；本轮产物漏收 `switched`，已补宿主导出清单并通过 Bash 语法与 ShellCheck，但未将缺失图片当作已审阅证据，跨框结论依据实际通过的设备断言。此次补收清单不改变设备断言或产品源码。

这些是 `x86_64` 模拟器证据，不是物理真机或真实中文照片质量验收。OCR 前后只验证手写客户端创建，没有手写模型实际推理证据；正式模块、许可分发审计、文件选择器完整导入和真机兼容仍未完成。

设备截图发现的两个产品缺陷已经修正：公用 `s.button` 使用横排零宽权重，OCR 的单项动作必须各自放入横排；SurfaceView 的独立 Surface 不能依赖父容器裁剪，预览必须保持比例完整容纳在框内。相机选择与照片比例匹配的预览尺寸，首帧到达后才启用拍照。[第五轮](https://github.com/metasequoiaime/msime/actions/runs/40086972668)同类相机 JPEG 已在 OCR 前截断测试图右侧，夹具因此把同一完整字符串放入已确认可见的源图区域，完整期望与精确断言不变，原图和失败照片仍保留。

本轮设备任务整体失败，不能把 OCR 专项通过报告成完整门禁通过。API 28 的基础冒烟在 `deferred composition length 1` 未观察到 IME 控件；API 35 在 OCR 专项前后的两次基础冒烟均停在拼写放大编辑的中间插入/删除断言。它们与[旧基点已复现的 `phrase composition` 失败](https://github.com/metasequoiaime/msime/actions/runs/40082820419)不同，尚无这两项具体失败的干净基点对照，不归为已证明的旧问题。修正提交 `335988b93d` 的常规 PR CI 为 25 项成功、6 项按范围跳过。完整 quick 的生成文档、符号表、语音目录与三个 Windows portable 链接失败均已在同版本干净基点复现；Rust workspace 编译和 providers Clippy 通过。

先前 `29bbfc36ef` 的[两版设备运行](https://github.com/metasequoiaime/msime/actions/runs/40118436084)均已结束。两版共享设备套件、模型准备、选择器完整导入／取消／损坏替换拒绝、相机识别及撤权、主进程手写共存通过。API 35 的同一真实离线 `:ime` 手写专项也通过；API 28 已完成前后识别、上屏、退格与符号画布恢复，但最后恢复原书写模式时未观察到预期设置控件，`ime_handwriting=1`，不能计为专项通过。两版整体仍有相同十八键三排宽度断言失败。

恢复设置失败的日志没有区分启动、打开选项或读取选项，本次细分这些固定阶段，并在失败时补收同一合成设备截图。设置页面跳转完成后先等待无障碍空闲、重新取得行节点，避免点击仍在转场的旧行；[后续产品 `b78178a731` 的 API 28/35 设备运行](https://github.com/metasequoiaime/msime/actions/runs/40121220880)已全部结束：两版共享设备套件、模型准备、选择器完整导入／取消／坏替换拒绝、相机识别／上屏／恢复／撤权、主进程手写推理，以及同一个真实离线 `:ime` 的前后识别、手动与硬件上屏、撤销、符号层和画布恢复均通过，原书写模式恢复也通过。验证提交 `4c9dd8837e` 与产品完整树仅专用 workflow 不同；六张前后／相机结果合成截图已逐张核对。两版结果都是 `core=0 baseline=1 coexistence=0 picker=0 ime_handwriting=0`，唯一失败仍是前述十八键三排 `69/66/69px` 宽度断言，整轮设备门禁仍未通过。这次运行消除了已观察到的恢复失败，不据单次通过声称已经证明唯一根因。恢复失败仍让专项失败，原模式回读和真实推理断言均保留。

## 卸载验证（#10235）

同步 `ccc237197b` 前的原生瘦包已构建，quick 与 JNI／宿主导出、arm64 ELF 依赖和 16KB 对齐检查通过。API 28、35 的专用 arm64 AVD 均从系统文件选择器导入正式锁固定的 81,146,926 字节模块：取消卸载仍可识别；损坏替换失败后确认卸载，状态回到未导入且按钮禁用；旧类加载器缓存无法重新打开；再次通过系统选择器导入后合成中英文图识别成功。两版卸载后页面截图均已目测。

两版 `build-core-test.sh arm64-v8a` 均通过：client-base 40 项、client-core 710 项、client-providers 285 项、client-services 主套件 1404 项，保留原有忽略与 runner 的硬链接排除；引擎维护租约 11 项通过。首次 quick 发现确认框未接统一截屏保护入口，以及纯 Java 资源包测试被前台服务依赖阻断，均已修正：确认框经 `ScreenCaptureProtection.show`，Java 安装队列的忙状态在 Activity 操作线程检查，共享资源包门面继续独立于 Service。

标准原生 APK 的 API 28 `smoke.sh --core` 通过；API 35 两轮完整 `smoke.sh` 均退出 1。首轮失败涉及触控板放大镜／实体候选共存、英文纠错模式、十四键五笔键面和五笔查码列表；第二轮失败涉及触控板 WebView 光标、模糊拼音和英文纠错模式。定向通过不代替整套通过，也没有用后续独立成功断言上述失败的原因已经解决。

同步 `ccc237197b` 时保留下载地址、系统选择器无匹配应用的提示与独立拍照窗口；卸载确认、互斥与成功后清理失败状态的行为不变。同源原生瘦包、arm64 ELF 依赖及 16 KB 对齐检查通过；宿主检查通过 316 项 JVM 冒烟与 258 份设备源码。API 28／35 再次通过系统文件选择器导入、取消卸载、损坏替换后卸载、缓存拒绝重开和重新导入识别，两版卸载后页面截图均已核对。模块仍是生产锁固定的公开发布文件，没有改锁或重建模型。临时验收脚本首次在 API 28 因 macOS Bash 3 的空数组配合 `set -u` 退出，修正临时参数展开后重跑通过，首次退出不计为通过。本次合并的完整 quick 通过：Rust workspace、Android／WASM、Linux 设置壳及原生 112 项测试、macOS、共享 Apple bridge 与 x86_64／i686 pipe 配置均通过。Windows 原生交叉构建因另一个验证持有共享依赖锁而按脚本跳过，HarmonyOS HAP 缺暂存原生库／资源而跳过，均不计为构建通过；没有重新运行上述 API 35 整套。未操作日用设备或触发发布。

## Related notes audit

卸载入口随后补上了结果语义：共享层返回 `removed: false` 时表示目标目录本来不存在，Android 设置页必须显示失败而不能清理“已完成”状态或提示卸载成功；Java 门面现在保留并返回这个布尔值。

卸载与安装的互斥随后收进进程级和共享层两处。原先只在设置页的线程上先读一次 `ResourcePackService.status(...).running()` 再删，检查与删除之间不是原子的；设置页没有声明 `configChanges`，卸载中旋转屏幕会重建出 `removing` 为假的新页面，导入按钮重新可点，与仍在进行的删除并行，旧页面 `onDestroy` 的 `shutdownNow` 还可能丢掉排队未执行的卸载。现在 `msime_client_resource_pack_remove` 与安装、校验、收编共用同一张登记表，同一进程里同一资源包忙时双方都返回 `resource_pack_busy`；Android 的卸载交给 `ResourcePackService.remove`，在主线程与 `claim` 一起判断并记入进程级的卸载集合，在不随页面结束的命名线程上执行，结束时经监听器通知并用提示报告结果，下载、导入请求在卸载期间被拒绝，卸载开始后才到达的请求记为失败原因「正在卸载」。备选是给 `Status` 加一个卸载阶段，但 `runningPacks()` 会把它算作下载中，牵动前台服务的停止条件和进度通知，所以用独立的集合。共享层登记表只在一个进程内有效，`:ime` 进程打开拍照识字前的校验不与主进程的卸载互斥，那一路仍依靠加载前重新校验安装状态。

[手写会话清理](../../implemented/bug-fix/2026-10-17-android-handwriting-session-reset.md)部分相关，继续持有手写生命周期；OCR 不得重置它的单例或复用它的请求代次。[按需语音运行库](../../implemented/feature/2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)部分相关，说明可选原生资源分发，不提供 OCR 推理接口；Android 的 sherpa-onnx 语音运行库也带一份 ONNX Runtime，与 OCR 模块那份在同一进程里共存的验证见下文。[Google Play 分发](../../implemented/process/2026-10-23-android-google-play-distribution.md)部分相关：Play 渠道不提供外部可执行模块，OCR 入口在该渠道拒绝。现有活跃笔记没有相机 OCR 的决定。

## Probe invocation

只在专用的 `arm64-v8a` 或 `x86_64` API 28/35 AVD 中运行 `bash platforms/android/tests/device/ocr-runtime-probe.sh <serial>`；模块带两套原生库，探针取设备首选的那套。脚本用仓库已有的 Gradle wrapper 显式启用 `-PmsimeOcrProbe=true`，分别构建、检查两个试验 APK，导入的模块文件放在标准 `target/android/ocr-probe-assets` 下。该脚本只清理两个试验应用的数据，不安装或修改产品 APK，也没有相机或编辑器验收。`ocr-product-smoke.sh` 同样按设备 ABI 构建产品包与共享层单测，接受 `msime-client-test` 前缀的专用 AVD。

普通安装方式的对照（`DirectOcrProbe`，同一个 `OcrModule` 由系统类加载器加载）只进模块的 `debug` 构建。除三种字号的中英文图外，两边都识别一张竖排两列图（右列「春眠不觉晓」、左列「处处闻啼鸟」），只认两列逐字正确且右列在前。两种方式使用同一份生成夹具；探针产物允许导出它自己生成的固定图片及其合成识别对照，不接受外部文件或用户图片，不在运行库中记录识别文本，也不把任意异常消息写入日志。这样能分别判断模型的识别偏差和动态模块加载问题，而不会把生产输入泄露到测试产物中。

以下三段是 ML Kit 试验模块的记录。原始 88 px 文字夹具在 API 28/35 均存在模型误识别；普通捆绑与动态模块的结果逐字一致，图片也逐字节一致，因此不能把该偏差归因于类加载器。保留这张原图及其失败结果，并增加内容不变、字号为 44 px 和 22 px 的两个用例。运行库探针要求所有图片、所有识别结果均与普通捆绑方式一致，且至少一张图片同时正确识别完整英文单词、数字与中文；不把几张图的局部成功拼成完整成功，不把运行库等价解释为模型精度通过。产品的识别质量、手写实际推理及真实拍照仍需独立验收。

API 35 的普通捆绑与动态模块均在三个字号输出同词的 `測/测` 字形差异，较小字号的完整英文均匹配；API 28 则在大字号出现 `文/丈` 的实质误识别。内容可用门禁接受「测试」的规范简繁字形组合，仍拒绝 `文/丈`、`O/0` 这类不同字词；逐图另报是否与简体原文完全一致，保留原始合成输出。这只调整探针对同义字形的判断，不转换、改写或纠正运行库返回的文本，也不把简繁混用视为产品精度已经验收。

API 28/35 的[最终隔离探针](https://github.com/metasequoiaime/msime/actions/runs/40074617788)均通过。每个 API 先用固定的旧模块源码复现关闭后重开时的 `IllegalStateException`，再恢复当前源码，断言没有残留源码差异，运行修正后的完整探针。三张合成图片逐字节一致、全部识别结果与普通捆绑 SDK 对照逐字一致，OCR 前后的手写客户端创建也通过。API 28 的 44 px 用例与简体原文完全一致，API 35 的较小字号识别内容匹配但含 `測/测` 字形差异；原始大字号与其他字号的误识别仍保留。首次宿主重复初始化和后续模块重开失败都是已复现并修正的探针问题，不是产品验收。

## 独立窗口交互的后续决定

#10377 撤回键盘内预览方案；替换交互由[独立拍照窗口](2026-10-31-android-ocr-workspace.md)持有。模块加载、固定摘要、离线与许可决定继续有效，未用新窗口验收结果替代旧模块的验证边界。

## Alternatives considered

- **Google ML Kit 文字识别（`com.google.mlkit:text-recognition-chinese:16.0.1`）**：最强的理由是它最先跑通，模块已经在 API 28/35 通过了导入、隔离加载、相机和手写共存的全部产品专项；常温延迟最低（中位 15.9 ms），峰值 PSS 111.5 MB 也最小，宿主手写本来就依赖 ML Kit。放弃的原因有两个。一是精度：41 张合成图上中文 CER 8.64%（tiny 4.07%），形近字成批出错（侯/候、丙/两、大/太、輪人/输入、己/已），竖排三张全错，其中一张整列为空。二是隐私：有网络权限的进程里，识别时 ML Kit 经 Google datatransport 记下 `FIREBASE_ML_SDK` 事件，约 50 秒后连到 `firebaselogging.googleapis.com` 上传，并在应用数据里持久保存 `ml_sdk_instance_id`；事件内容按 ML Kit 的数据披露还包括设备型号、MCC/MNC、语言区域和时区；这是 SDK 条款允许的行为，模块无法在不碰私有状态的前提下关掉，而输入法进程有网络权限。
- **PP-OCRv6 small（同一引擎，换大一号的模型）**：精度最好，中文 CER 3.39%、横排 0%、英文 0.40%。放弃是因为成本：关掉内存池后峰值 PSS 仍有 185 MB（开着 442 MB），常温中位 104–118 ms，双 ABI 模块 102 MiB，接近 128 MiB 上限；tiny 横排中文 CER 0.71% 已经够用。
- **NCNN 或 MNN 跑 PP-OCR**：运行库通常只有几 MB，比 ONNX Runtime 每 ABI 33–39 MB 小得多，模块能小一半以上。放弃是因为模型要自己转换（官方只发 Paddle、ONNX 和 safetensors 格式），转换后的精度要重新验证，还要自己写并维护一层 JNI；ONNX Runtime 有官方 Android AAR 和 Java API，模型原样使用。
- **Paddle Lite**：PaddleOCR 过去的移动端部署路线，模型和推理同出一家。放弃是因为它最近的发布是 2024-07 的 `v2.14-rc`，PP-OCRv6 没有现成的 `.nb` 模型，要用它的 opt 工具自己转换并重新验证精度，同样要自己写 JNI。
- **Tesseract（tesseract4android 等）**：成熟、完全离线、Apache-2.0。没有纳入本次评测；放弃是因为它面向扫描件，拍照场景要自己补去噪、倾斜校正和版面切分，中文竖排还要另一份 `chi_sim_vert` 训练数据，而 PP-OCR 的检测加识别管线本来就是为拍照场景设计的。
- **复用语音运行库资源包里的 `libonnxruntime.so`**：模块可以省掉 33–39 MB 的运行库。放弃是因为那份 ONNX Runtime 的版本由 sherpa-onnx 决定、只有 arm64、不带 Java 绑定，OCR 要自己写 JNI，还会让 OCR 依赖语音资源包的安装状态。
- **用同发行者 APK 签名放行多个模块版本**：可避免宿主每次固定模块摘要，也能复用系统验包；但还需确认最低 API 的验包语义、签名轮换和接口兼容。本次选择共享层已有的固定完整摘要机制，信任边界随宿主源码审查，尚未提供产物时明确拒绝。
- **在基础包内捆绑 OCR**：接入和设备兼容验证最简单；但违反报告人明确的可选模块与基础包体积要求。
- **原样加载参考插件**：已经包含多语言模型和 APK 加载流程；但接口示例与实现不一致，且会操作私有 ML Kit 状态、记录识别文本，不能直接采用。
- **安装另一个 OCR 应用并绑定服务**：SDK 初始化和权限可按应用隔离；但增加系统安装流程，与用户要求的手动导入资源模块有差异，尚未作为实现方案。

## Consequences

- 不装模块时基础输入法照常可用，基础 APK 不包含 OCR 模型、OCR 原生库和识别依赖，也不自动下载模块。用户从 `ocr-module-android-v1` 下载 `msime-ocr-v1.apk`，在拍照识字设置里经系统文件选择器导入，之后在键盘内授权相机、拍照、离线识别中文和英文，勾选整行或长按选部分文字上屏。
- 信任只来自编译进宿主的锁：同一接口版本的模块只发一次，换模块就要改锁、重新发宿主。已经发出去的宿主只认这份字节，标签下的资产不能替换。
- 引擎、模型和全部依赖的固定来源、许可与通知已记录：ONNX Runtime 的 MIT 与第三方通知、PaddleOCR 的 Apache-2.0 全文随模块分发，来源和摘要固定在 `resources/ocr-models-android.lock.json`。
- 已验证的设备覆盖：ML Kit 试验模块阶段在 API 28/35 模拟器上验证了宿主一侧的导入、损坏与错误替换拒绝、撤权、相机释放、会话切换、迟到结果和手写共存；ONNX Runtime 模块在 arm64 API 35 模拟器上通过隔离探针与产品专项；已发布的模块在 Pixel 3 XL 真机上通过系统选择器导入、篡改拒绝、隔离探针、`:ime` 进程内识别和键盘内拍照。
- 还没有验证的：ONNX Runtime 模块在 API 28 和 x86_64 AVD 上的探针与产品专项；真机上识别出文字后的上屏（测试手机的摄像头对不到文字）；真实照片的识别质量。评测图是合成的，带模拟拍照的退化。tiny 的已知错误集中在形近字（巳/已）和标点（……、句末的点），倒置或大角度旋转的文字没有方向分类，会识别失败；横竖混排只按多数方向排序。识别结果仍需用户核对，界面保留了提示。
- 真机延迟明显高于模拟器：Snapdragon 845 上常温识别中位 280 ms、p90 542 ms，模拟器是 46.5 / 91.5 ms；每次拍照还要重新 `open`（复核 81 MB 摘要并建两个会话）约 0.3 s，从按下拍照到出结果约 0.6–0.7 s 加相机出图时间。每次拍照都复核整个模块是有意的，不能为省这 0.3 s 改成只信安装清单。
- `mlas.disable_kleidiai=1` 对所有设备生效，真机上放弃了 KleidiAI 的 ARM 优化内核；上面的真机延迟是关着它测的，没有打开它的对照数据。若要在真机上重新启用，必须先能可靠识别「报告 SME 却不支持」的环境，不能让一次 SIGILL 带走整个输入法进程。
- 动态模块在 Android 14 起要求代码文件只读，宿主导入后封为只读，输入法进程拒绝有写权限的 APK。
- 遥测的处置依赖两层：清单删掉 `TelemetryInitializer`，以及 `setTelemetry(false)`。升级 ONNX Runtime 时要重新检查 AAR 清单和遥测入口，1.31.0 里通道初始化失败这一缺陷修好后，任何一层缺了都可能开始上报。升级同时要重跑网络抽样。
- 同一个输入法进程里可能有两份 ONNX Runtime：sherpa-onnx 语音运行库带的那份在宿主的链接器命名空间，模块这份在模块类加载器的命名空间。产品专项在识别前先加载宿主的语音运行库，识别后检查语音运行库仍可用；没有在同一进程里交替跑真实的语音识别和 OCR。
- 模块双 ABI 约 77.4 MiB，用户手动下载；单 ABI 拆包能省一半，但要宿主按 ABI 选锁条目，本次没做。

# Agent Note: Android 设备信息与一键复制

Status: implemented

## Problem

#5662：用户反馈问题时常常漏填机型、系统版本这些排错必需的信息，而且一个问题可能只在某个版本的 Android System WebView 上出现。应用里没有地方列出这些信息，更没法一键复制；「帮助与反馈」的「附带诊断信息」只随站内反馈提交，贴到 GitHub Issues 或 QQ 群时用不上。

## Decision

- 「关于」页新增「设备信息」组：应用版本（版本名、版本号、产品版本 id）、品牌、型号、系统版本（Android 版本与 API 级别）、系统构建（`Build.DISPLAY`）、处理器架构（`Build.SUPPORTED_ABIS`，首选在前）、屏幕分辨率（窗口最大尺寸的真实像素与 dpi）、存储空间（数据分区可用/总量）、运行内存、Android System WebView（`WebView.getCurrentWebViewPackage()` 的版本与提供方包名）、键盘是否已启用和是否为默认。组末「复制设备信息」把整段文本放进剪贴板；「帮助与反馈」的「其他渠道」里也有同一个复制按钮。
- 格式化与复制文本在纯 Java 的 `DeviceInfoReport`（JVM 冒烟覆盖），读系统的一半在 `home/DeviceInfo`：屏幕尺寸在主线程从 Activity 的窗口量，其余在 `HostTask` 的工作线程读。存储与内存用十进制单位，与系统设置的显示一致；读不到的项写「未知」。
- 只在用户点「复制」时进剪贴板，什么都不上传；不读账号、输入内容、已安装的其他应用或任何能标识设备的 id。站内反馈的诊断字段白名单（`FeedbackApi.DIAGNOSTIC_KEYS`）不变。

## Alternatives considered

- **把这些字段并进站内反馈的「附带诊断信息」** — 一处收集、自动附带；但白名单是服务端定的，加字段要改服务端，而且贴到 GitHub Issues、群里的场景照样拿不到。
- **单独做一个「设备信息」详情页** — 和截图里的视频压缩应用一样有独立页面；但「关于」本来就显示版本，信息只有十来行，再加一层页面只是多点一次。

## Consequences

- **收益**：反馈时一次复制就带齐排错信息，WebView 版本问题有了对照。
- **代价**：「关于」页变长；厂商系统版本（如 OriginOS）没有统一的 API，只能靠「系统构建」间接看出。

## Verification

`platforms/android/tests/settings/DeviceInfoReportSmoke.java`（`bash platforms/android/check-host.sh`）；页面代码经 Gradle `compileFullDebugJavaWithJavac` 编译，没有在真机上验收。

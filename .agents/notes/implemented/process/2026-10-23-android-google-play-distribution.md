# Agent Note: Android full 版上架 Google Play 的渠道构建

Status: implemented

## Problem

Android 此前只经 GitHub release 分发 APK，靠应用内更新（`UpdateApi`、`UpdateJobService`）升级。full 版（`app.msime.android`）要上架 Google Play，现有的包有四处过不了 Play：

- Play 只接受 AAB，发布流水线只产出 APK。
- 清单声明了 `REQUEST_INSTALL_PACKAGES`。它是 Play 的受限权限，自更新不是被认可的用途；Play 的政策也不允许上架的应用绕过 Play 更新自己。宿主原本只在安装来源是 `com.android.vending` 时才隐藏更新入口，包本身仍带着权限和整条自更新路径。
- full 版是瘦包，本地语音运行库 `libonnxruntime.so`、`libsherpa-onnx-c-api.so` 作为资源包 `voice-runtime` 在运行时下载后 `System.load`。Play 禁止从 Play 以外的来源下载并执行代码，.so 也算。
- targetSdk 是 35，Play 从 2026-08-31 起要求新应用和更新以 API 36 为目标。

## Decision

渠道是 gradle-app 的一个属性，不是 flavor：`-PmsimeDistribution=github|play`，缺省 github，由 `build-apk.sh` 在 `MSIME_ANDROID_DISTRIBUTION=play` 时转交。play 时：

- BuildConfig 的 `DISTRIBUTION` 写成 `play`，宿主经 `AppVersionPolicy.playDistribution()` 反射读（理由与 `RELEASE_DATE` 相同，`check-host.sh` 的 javac 编译没有 BuildConfig）。
- release 叠加 `platforms/android/distribution/play/AndroidManifest.xml`，用 `tools:node="remove"` 删掉 `REQUEST_INSTALL_PACKAGES`。`RECEIVE_BOOT_COMPLETED` 不删：它不受限，ML Kit 带进来的 WorkManager 也声明并依赖它。
- `UpdateJobService.updatedByPlay`（原 `installedFromPlay`）对 Play 渠道的包一律为真，关于页不显示检查更新、自动更新和更新通道，每日任务不运行，不论这个包经什么途径装上。
- `build-native.sh` 不再收到 `--omit-voice-runtime`，两个语音运行库随包；词典、语言词库、离线释义这些数据照旧按需下载（数据不受那条政策约束）。`ResourcePackService` 不接 `voice-runtime` 请求，`LocalAsrRecognizer` 不从 files 目录载入 .so，Play 版只走包内自带的那一份。
- `bundle<Flavor>Release` 输出 AAB，`jarsigner` 用 msime-release 签名，打包后的条目检查剥掉 `base/` 前缀后沿用 APK 的那一套，另要求两个语音运行库在包里；135 MB 的体积上限只量 GitHub 渠道的 APK。

签名：Play 应用签名登记的是现有的 msime-release（Play Console 里选「使用现有应用签名密钥」，用 PEPK 导出上传），上传密钥也是它。这样 Play 版和 GitHub 版证书相同（SHA-256 `3a889e43…8e4132`），两边可以互相覆盖安装，Google 登录的 OAuth 客户端也不用为 Play 另登记一个证书指纹。

发布：`release-android.yml` 只为发布的、不带后缀的正式版（main 上触发、`publish` 为真）在 full 那一格另打一份 AAB，放进 `google-play-msime-android-<版本>` artifact，名字避开 publish 收集 APK 用的 `msime-android-*-<版本>`。`play` job 排在 GitHub 发布之后，用 `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` 经 `platforms/android/scripts/upload_play.py`（Google 官方的 `google-api-python-client` 2.201.0 与 `google-auth` 2.62.0，装进临时 venv）上传：一次 edit 里上传 bundle、把这个 versionCode 设成轨道上唯一的版本、提交。手动触发默认 `play_track: alpha`（封闭测试）、`play_status: draft`；nightly 经 workflow_call 调用，`play_track` 缺省 `none`。beta、alpha 发布不进 Play：它们的 versionCode 与随后的正式版相同，Play 不接受重复的 versionCode。

targetSdk 升到 36，所有渠道一起升。`<application>` 显式写 `android:enableOnBackInvokedCallback="false"`：36 起不写就默认开启预测式返回，而 `:ime` 进程里的输入法服务靠 KEYCODE_BACK 收起面板；HomeActivity 上原有的 `true` 覆盖它，行为与 35 时相同。

## Alternatives considered

- **加一个 distribution flavor 维度**：Gradle 原生的做法，渠道差异可以放进各自的源集。但变体从 6 个变成 12 个，所有任务名都要改（CI 的 `compileFullReleaseJavaWithJavac`、`lintFullRelease`、`build-apk.sh` 的 `assemble<Flavor>Release`、设备测试脚本），而两个渠道之间只差一个 BuildConfig 字段和一行清单。属性与已有的 `msimeVersion`、`msimeAbis` 是同一种写法。
- **保留 `REQUEST_INSTALL_PACKAGES`，只靠运行时判断安装来源**：不用改清单和构建，宿主本来就会在 Play 安装时隐藏更新入口。但受限权限在审核时要申报用途，自更新不在允许之列；而且经内部应用分享或 adb 装上的 Play 版，安装来源不是 Play，仍会露出自更新。
- **让 Google 生成新的应用签名密钥**：Play 推荐的默认选项，密钥由 Google 托管。代价是 Play 版和 GitHub 版证书不同，两边装过的用户只能卸载重装、丢掉本地词库和设置，还要为 Play 的证书另登记 Google 登录的 OAuth 客户端。
- **第三方 Action `r0adkll/upload-google-play`**：workflow 里不用写脚本。但仓库目前只用官方和少数大厂的 Action，多一个第三方 Action 就要在 publish 阶段接触 Play 凭据；仓库已有的 iOS 送审也是官方 API 加一个小脚本（`submit_testflight_review.py`），这里沿用同一写法。
- **默认 `completed` 直接推给轨道用户**：少一次在 Console 里手动发布。但应用还没通过审核发布过时 Play 只接受 draft，而且现在没有任何 Play 上的回滚手段比「先别发」更便宜。

## Consequences

- Play Console 只接受手动上传的第一个包，API 在那之前找不到这个应用。上架前的第一次 Android 正式发版要带 `-f play_track=none`，从 workflow artifact 下载 AAB 手动上传；之后每次正式版自动以 draft 出现在 alpha 轨道，要有人在 Console 里点发布。确认流程稳定后可以把默认值改成 `completed`。
- 应用建在 YLDM Co., Ltd. 的组织开发者账号下，不受个人账号「封闭测试 12 名测试者连续 14 天」的限制。
- 建应用之前，包名先要在 Play Console「Android 开发者验证」里登记：选中 msime-release 的证书指纹，上传一个 assets 里带 `adi-registration.properties`（账号标识）、用这把密钥签名的 APK 证明持有私钥。这个 APK 只用于验证，可以是不含代码的空壳。2026-10-23 已提交，审核最长 48 小时。这份登记同样覆盖 GitHub 渠道：从 2027 年起，Play 之外分发的应用也要登记才能装在经过认证的设备上。
- `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` 没配时 `play` job 失败，GitHub 发布不受影响（它排在前面）。
- targetSdk 36 影响所有渠道的包。`android-device.yml` 仍在 API 28 和 35 上验收，没有在 Android 16 上跑过；设备测试脚本里有不少按 `emulator-5580`（API 35）写的假设，接 API 36 镜像需要单独处理。
- Play 版比 GitHub 版大约多 27 MB（两个语音运行库），在 Play 按设备拆分后的下载上限之内。

## Related notes audit

按 `REQUEST_INSTALL_PACKAGES`、`UpdateJobService`、`installedFromPlay`、`com.android.vending`、`OMIT_ON_DEMAND`、`omit-voice-runtime`、`Google Play` 检索活跃笔记：`2026-10-18-android-update-job-cancellation.md` 讲的是更新任务的取消，与渠道无关；`2026-10-20-android-release-page-sha256sums.md` 讲的是 GitHub 发布页的附件，Play 的 AAB 不进发布页，与它不冲突；`2026-10-20-macos-on-demand-voice-runtime-and-wubi.md`、`2026-10-13-ios-language-packs.md`、`2026-10-19-eval-resource-integrity.md` 是其他平台的按需资源或资源校验，原地保留。

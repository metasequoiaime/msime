# Agent Note: Android 五笔码表按需下载

Status: implemented

## Problem

PR #10068 把词库发布锁推进到 dict-v2.0.39。dict-v2.0.29 起 `msime-wubi.db` 并入派生的五笔多字词，从 14,536,704 字节涨到 94,982,144 字节，在 APK 里按 deflate 压缩后约 45 MB。Android full 和拼音版的瘦包（`MSIME_ANDROID_OMIT_ON_DEMAND=1`，GitHub 的 APK 与 Play 的 AAB 同一个开关）照旧随包带着它。android-v0.6.0 的瘦包是 133,833,900 字节，换锁后会到约 172 MB，超过 `build-apk.sh` 135,000,000 字节的上限，发版直接失败。

macOS 在 [macOS 语音运行库与五笔码表按需下载](2026-10-20-macos-on-demand-voice-runtime-and-wubi.md) 里已经把五笔码表改成资源包 `wubi`，只有五笔版随包带它。五笔版的 `EngineResources` 预算是所有版本共用的 126000 KiB，换锁后五笔版约 194,784 KiB，同样打不出包。

## Decision

维护者的决定：Android full 和拼音版跟 macOS 一样，五笔码表改为资源包 `wubi`，135,000,000 字节的上限不动；macOS 五笔版照旧随包带码表，只给它单独的预算。

### 共享规则

- `on_demand_artifacts("android")` 与 macOS 相同，是 `[日文词典三件套, msime-wubi.db]`；iOS 仍只有日文那一组。host-api 的 `ON_DEMAND_ARTIFACTS` 按编译目标取值，所以 Android 上自动得到 macOS 已有的全部行为：资源目录整体缺码表时校验放行；`wubi_dictionary` 改读词库更新或 `wubi` 资源包里的那份；两处都没有时 `wubi_dictionary_missing` 为真，五笔像缺词库的粤拼一样回退；代次准备时日志里的五笔行跳过，码表到达后 `merge_downloaded_wubi` 并入并补写；词库自动更新只给本机已有的码表下载新版本。
- 86 和 98 两版五笔读同一个文件（`msime-wubi.db` 的 `wubi86`、`wubi98` 两张表），一个资源包同时服务两者。五笔反查和五笔查码读的也是这份码表；全拼的 `wubi86` 辅助码是仓库自带的 `helpcodes/wubi86_helpcode.txt`，与码表无关，照旧随包。

### 打包

- `build-apk.sh` 在瘦包里以 `verify_resources --omit-on-demand --platform android` 暂存，码表不进 `assets/dictionary/`，随包的 `desktop-dictionary.lock.json` 也去掉这一项。签名后的包里出现 `msime-wubi.db` 即失败。开关只对 full 和拼音版生效，五笔版的默认方案离不开码表，照旧随包，开发构建照旧全带。
- `release-android.yml` 发版前的资源包检查给 full 加上 `wubi`，锁里的地址失效时停在发版之前。

### 宿主

- `KeyboardScheme.WUBI.resourcePack()` 是 `wubi`。`availablePacks` 在资源包已装、或资源目录里还带着 `msime-wubi.db` 时算作具备；后者覆盖五笔版、开发构建和尚未收编的旧安装。键盘只列具备码表的五笔，设置页沿用「已添加、词典还没到」的那一行：显示大小或进度，可下载、取消、重试、移除。设置页的「五笔查码」一组只在五笔启用且码表在本机时出现。
- **升级收编**：Bootstrap 原来只收编日文词典那一组，现在按组处理日文词典和五笔码表。本安装包的锁里没有某一组时，把 `bootstrap/resources` 里上一个安装包解出的那组用 `ResourcePacks.adopt` 收编为资源包：同一文件系统内改名，按编译进来的长度和 SHA-256 核对。字节对不上或缺文件时删掉，暂时性失败时挪进 `bootstrap/wubi-pending` 等下次启动。拼音版不提供五笔，旧码表直接删掉，不收编；收编了反而会让词库自动更新每次都替它下载新版本。
- **启动补齐**：Android 原来的规则是启动时从不自动下载，这里有一个例外。设置应用回到前台、首次准备已经 `READY`、开屏和首启引导都已过去时，`KeyboardScheme.wubiTableWanted` 判断三件事：本版本提供五笔、码表不在本机、偏好里的 `scheme` 或 `last_chinese_scheme` 是 `wubi`。成立就调用 `ResourcePackService.request`，计费网络先弹窗说明大小，没联网提示一句，每个进程最多请求一次。判断等到 `READY` 之后：准备途中收编正把码表从资源目录挪进资源包，这时两处都看不到它。默认启用列表里的五笔不算，新装 full 的列表本来就有五笔，按它判断会让每个新用户一打开应用就下载约 95 MB。

### macOS 预算

`check_app` 对五笔版（`wubi_bundled=1`）按 205000 KiB 核对，其余版本仍是 126000 KiB。推算方法与脚本测量的方式一致：`resources/editions/wubi.lock.json` 的八个文件按锁里的大小逐个向上取整到 4 KiB，合计 194096 KiB；`helpcodes/` 十个文件同样取整，合计 688 KiB；总共 194784 KiB。码表还在随词库发布变大，所以余量留约 10 MB（约 5%）。用 dict-v2.0.24 按同样方法算得 116036 KiB，也与 macOS 笔记里的实测相同。

## Alternatives considered

- **提高瘦包上限，码表留在包里**：五笔用户升级后什么都不用做，宿主也不用改。没有采用：维护者明确保留 135,000,000 字节的上限。全功能版大多数用户不用五笔，却要为它多下载约 45 MB，拼音版则根本不提供五笔。
- **只靠「添加语言」下载，不做启动补齐**：这样能守住 Android 原来「启动时从不自动下载」的规则。没有采用：换锁的同一个版本里旧码表与新锁对不上，收编必然失败，在用五笔的人升级后键盘里五笔会消失。他得自己想到去设置页找那一行。首启引导里选了五笔的新用户也一样，键盘打不出五笔。例外收得很窄：只认偏好里的当前方案和上一次的中文方案，先在计费网络上确认，每个进程只问一次，并且只在设置应用里发起，输入法进程照旧从不联网。
- **按启用列表判断要不要补齐**：实现最简单。没有采用：新装 full 的默认启用列表（`TouchKeyboardScheme::DEFAULT_ENABLED`）包含五笔，这会变成人人都下载。
- **从 Android 的默认启用列表里去掉五笔**：新装用户就不会看到「五笔 · 需下载」那一行。没有采用：`DEFAULT_ENABLED` 是 iOS、HarmonyOS 共用的契约，那两端照旧随包带着码表，改它会无故改动它们的出厂列表，超出这次的范围。代价见 Consequences。
- **收编时不论版本一律收编（完全照搬日文词典）**：代码少一个分支。没有采用：拼音版以前随包带着码表却从来用不到，收编成资源包后，词库自动更新会继续为它下载每个新版本的码表。

## Consequences

- **体积**（arm64 GitHub 瘦包，开发密钥签名，带内置词书，在编译机上实测）：
  - develop 现在的锁（dict-v2.0.24）：128,062,671 字节，比上限少约 6.9 MB。按 gzip 估算，旧码表在包里约占 7.4 MB。
  - 临时套上 #10068 的锁（dict-v2.0.39）：full 是 128,427,215 字节，拼音版也是 128,427,215 字节（内容不同，对齐后长度碰巧相同），离上限还有约 6.6 MB。码表留在包里时，按 gzip 估算还要多约 44.9 MB，约 173 MB。
  - 随包的锁里没有 `msime-wubi.db`。
  - macOS `MSIME_MACOS_OMIT_ON_DEMAND=1` 暂存后实测：五笔版 194784 KiB，全功能版和拼音版都是 106412 KiB，仍在 126000 KiB 以内。
- **升级窗口**：
  - 同一词库锁下升级的 full 用户，旧码表被收编为资源包，不联网，五笔照常可用。
  - 升级同时换了词库锁（与 #10068 同一个版本发出时就是这样）时，旧码表对不上新锁，被删掉，新代次里没有五笔表，日志里的五笔行先跳过。在用五笔的人下次打开设置应用时会被请求下载约 95 MB，下载完、下一次建会话时码表并入代次，并补写这些五笔行。
  - 用户不打开设置应用之前（Play 静默更新后尤其如此），键盘不列出五笔，方案回退到上一次的中文方案或全拼。这与日文、粤拼、注音、笔画改为按需下载时相同。
  - 词库自动更新目录里那份已经对上新锁的码表，这里不拿来收编；收编只看 `bootstrap/resources`。
- **新装**：full 的默认启用列表里有五笔，所以设置页的五笔一行显示「需下载 95.0 MB」，键盘先不列出五笔。如果想让新装用户完全看不到这一行，要改共享的默认列表，那是一个跨三端的产品决定。
- **离线**：没有网络时，五笔要等联网后下载。资源包本来就支持离线导入，但 Android 设置页没有为 `wubi` 提供导入入口，这里没有加。
- **iOS 和 HarmonyOS 不变**：
  - iOS 的 `on_demand_artifacts("ios")` 仍只有日文那一组，`MSIME_IOS_OMIT_ON_DEMAND=1` 的发布包照旧带着码表。键盘扩展没有体积门禁，App Store 的上限也远高于此，代价是安装体积多约 80 MB。要跟进时，把 `ios` 加进五笔那一组，让 iOS 主应用在选五笔和启动时下载，键盘按 `wubi_dictionary_missing` 不列出五笔即可，共享层不用再改。
  - HarmonyOS 的 `stage-resources.sh` 不省略任何按需文件，也没有词库资源包的下载入口，码表照旧随 HAP。HAP 体积同样多约 80 MB，但没有门禁。要跟进得先给鸿蒙做资源包下载，工作量大于 iOS。
- **验证**：
  - `cargo test -p msime-client-core -p msime-client-providers -p msime-host-api` 中与本改动相关的测试全部通过：按组裁剪的 Android 分支、旧码表收编被拒、`wubi_dictionary_missing` 在 Android 为真、校验标记的子集重验。
  - 另有两项失败与本改动无关，出在这次没改动的文件里：`ffi::pack_downloads` 的期望列表缺 `pronunciations`；`script_plugins` 的本地时间用例。
  - `platforms/android/check-host.sh` 通过：288 个 JVM 冒烟测试（`KeyboardSchemeSmoke` 新增五笔资源包、`availablePacks`、`wubiTableWanted` 的用例），248 个设备测试源编译通过。
  - 上面的 APK 和 macOS 暂存实测都是在编译机上做的。
  - 设备上的行为没有验证：收编、启动补齐的弹窗、下载后五笔出现在键盘里、码表并入后五笔学习的补写。要在 API 35 模拟器上，先装 android-v0.6.0 并用五笔打字，再覆盖安装新瘦包来验收。

## Related notes audit

- [macOS 语音运行库与五笔码表按需下载](2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)：本篇把它的五笔码表规则搬到 Android。那篇里「Android 仍只有日文那一组」的事实已就地改为指向本篇。部分重叠，互链。
- [五笔反查](2026-10-11-wubi-reverse-lookup.md)：Android「五笔查码」出现的条件就地补上「码表在本机」。决定不变，互链。
- [Android 候选显示英文音标](2026-10-26-android-candidate-pronunciation.md)、[内置词书打包](../bug-fix/2026-10-20-bundled-wordbooks-packaging.md)：同样为守住 135,000,000 字节的上限做取舍，前者也把数据改成资源包。决定不变，无需改动。
- [Google Play 分发](../process/2026-10-23-android-google-play-distribution.md)：Play 的 AAB 与 GitHub 的 APK 用同一个瘦包开关，五笔码表是数据而不是可执行代码，Play 版同样按需下载。决定不变。
- [iOS 语言词库按需下载](2026-10-13-ios-language-packs.md)：同一套 `on_demand_artifacts` 规则，iOS 的取值不变，跟进方式写在上面。无需改动。

# Agent Note: Android 发版与设备作业的 Gradle 依赖下载重试

Status: implemented

## Problem

2026-11-17 21:48 到 11-18 06:40（UTC），GitHub 托管 runner 上有四次 Android 构建在 Gradle 的配置阶段失败，原因相同：解析根工程的构建脚本 `classpath`（AGP 8.13.2 及其传递依赖 kotlin-stdlib、grpc、asm、httpmime、tensorflow-lite-metadata 等，多数只在 Maven Central 上）时，`repo.maven.apache.org` 回了 21 次 `429 Too Many Requests`，日志随后报 `Repository MavenRepo is disabled due to earlier error`，构建在 Gradle 守护进程启动约 20 秒后失败。

- Native Platform CI 的 `android-device` 作业三次：run `44181008755`（API 35）、`44215264656`（API 35）、`44243953507`（API 28）。
- Release Android 0.8.3（run `44252182195`）的 `release (wubi)`。`fail-fast: true` 随即取消了其余五个版本，什么都没发布。

同一时间窗里，GitHub 托管 runner 上 43 次 Gradle 构建步骤失败了这 4 次；11-16 到 11-17 21:00 的约 110 次，以及 10-07 以来所有失败的 CI、nightly 和发版运行里，都没有这种错误。每一次都有同一分钟起跑的兄弟作业顺利下载了同样的依赖：0.8.3 里 tibetan、full、pinyin、japanese 被取消时已经过了配置阶段，两个设备作业也都打出了 APK。限流落在个别 runner（共享的出口 IP）上，不是本仓库自己的并发造成的。

这些作业都不缓存 `~/.gradle`，每次从零解析。发版一次有八个这样的冷构建（六个版本加两个设备作业）同时开始。Gradle 9.5.1 遇到 429 只再试两次，用本地一直回 429 的 HTTP 服务实测：三次请求分别在 0、1、3 秒，随后在这次构建里停用该仓库，之后的每个依赖都直接失败。

## Decision

两处改动，只动 workflow 和一个新脚本，`build-apk.sh` 和本地构建不变。

**放宽 Gradle 自己的重试。** `release-android.yml` 的 `release` 作业和 `android-device.yml` 的 `device` 作业在作业级 `env` 里设

```
GRADLE_OPTS: -Dorg.gradle.internal.repository.max.tentatives=6 -Dorg.gradle.internal.repository.initial.backoff=5000
```

即同一个文件最多请求六次，第一次重试前等 5 秒、之后每次翻倍（5、10、20、40、80 秒，合计 155 秒）。`GRADLE_OPTS` 里的 `-D` 会传到构建所在的守护进程，`build-apk.sh` 里的 Gradle 调用也照样生效，不用改脚本。

只设 `org.gradle.internal.repository.*` 这一组。Gradle 还有一组 `org.gradle.internal.network.retry.max.attempts` / `initial.backOff`，字节码里两组的缺省值相同（3 次、1000 毫秒），但实测对 POM 和 JAR 下载回 429 时都只有 repository 这一组起作用，network 这一组不改变请求次数和间隔。两组都是 Gradle 的内部属性，没有文档承诺。

**构建前先单独解析构建脚本 classpath。** 新增 `platforms/android/resolve-gradle-classpath.sh`：在 `platforms/android/gradle-app` 上执行 `gradlew --console=plain help`。`help` 本身不产出什么，但要先配置根工程，也就必须下载并加载构建脚本 classpath 上的全部插件依赖，正是上面四次失败的那一段。失败时只在输出里有下载类错误才重来：`Received status code (408|429|5xx) from server`、wrapper 下载发行包时 Java 写的 `Server returned HTTP response code: (408|429|5xx)`、读写或连接超时、`Connection reset`、域名解析失败。重来前等 60、120 秒，一共三次。其他失败（构建脚本错误、依赖不存在、403）按 Gradle 的退出码立即结束，不重试，免得掩盖真正的问题。

这一步在两个工作流里都排在真正的构建之前：`release-android.yml` 里在「Build native Android host」之前，那一步才从 secret 解出发版签名密钥，所以重来时密钥不在磁盘上；`android-device.yml` 里在「Build the APK」之前。

不改的：发版矩阵的 `fail-fast: true`；Native Platform CI 的「Android host」作业（跑在 Depot 上，出口 IP 不同，这段时间没有出现 429）；不加跨运行的 Gradle 缓存，也不加镜像。

## Alternatives considered

- **只靠放宽 Gradle 的重试，不加预取步骤**：改动最小，一行环境变量；但退避用完后整个仓库在这次构建里停用，限流若持续几分钟，构建照样失败。而在发版作业里，这时原生库已经编完、签名密钥已经解出，要重来只能整个作业重跑。预取步骤在密钥解出之前把最常出事的那一段单独拿出来，可以多等几分钟。两者一起用。
- **在构建步骤外整体重试 `build-apk.sh`**：同样能接住执行阶段（AAR、aapt2、R8）的下载失败；但每次重来都要重编原生库，发版作业里还要让解出的密钥在多轮之间一直留在磁盘上。不采用。
- **把重试属性写进仓库的 `gradle.properties`**（`systemProp.` 前缀，实测同样生效）：本地构建也受益；但断网的开发者要等两分多钟才看到失败，而本地没有出过这种问题。只在 CI 上设。
- **跨运行缓存 `~/.gradle`**（`actions/cache` 或 `gradle/actions/setup-gradle`，发版里只读）：能把 Maven Central 的请求降到接近零；但发版作业持有不可更换的签名密钥，`release-macos.yml` 对恢复的缓存每次都核对摘要，因为缓存由更早的运行写入，可信度不比直接下载高。Gradle 缓存要达到同样的要求，得先引入依赖校验（`gradle/verification-metadata.xml`），每次升级依赖都要重新生成，Dependabot 不会替我们做。留给单独的供应链讨论。
- **同一次运行里先由一个不碰密钥的作业解析一次依赖，再把 `~/.gradle` 作为 artifact 交给矩阵**：八个冷构建降为一个，来源与直接下载等价；但要多一个作业、几百 MB 的 artifact 和关键路径上几分钟的等待。限流若在本改动之后仍然出现，再做这一步。
- **在 `mavenCentral()` 前加 Google 的 Maven Central 镜像**（`maven-central.storage-download.googleapis.com`）：两行改动，流量离开被限流的主机；但发版作业因此多信任一个构建输入的来源，而这个镜像自己的限流情况不明。由维护者决定，不在本改动里。
- **调整仓库顺序**：`settings.gradle.kts` 里 `google()` 已经排在 `mavenCentral()` 之前，失败的依赖只在 Maven Central 上，顺序改不了什么。
- **把矩阵错开（`max-parallel`）**：被限流的是个别 runner，同一分钟的兄弟作业都正常，本仓库的并发不是诱因；错开只会让发版多等一轮。不采用。

## Consequences

- **收益**：构建脚本 classpath 在配置阶段遇到的 429 和 5xx 有两层退避：Gradle 内每个文件约 155 秒，脚本层再给两次各等 1、2 分钟的整轮重来。执行阶段的下载由 Gradle 内的退避覆盖。本地从空的 Gradle 目录跑一次预取约 25 秒，下载约 110 MB（发行包另 151 MB），0.8.3 失败日志里列出的 22 个模块全部取到；之后的构建直接用这些文件。
- **代价与已知上限**：Maven Central 真的长时间不可用时，作业要多等十分钟左右才失败（三轮，每轮 Gradle 内最多约 155 秒，加上两次等待），在 `release` 作业 45 分钟、`device` 作业 120 分钟的上限之内。下载类错误按输出里的文字判断，Gradle 改了错误措辞就会漏判，表现为不重试，不会误把真正的错误当成下载错误重来。两个属性是内部实现：**升级 Gradle wrapper 时要复核它们仍然生效**，做法见下文 Verification。执行阶段的下载失败仍然没有整轮重来；若之后仍有发版因此失败，按上面的「同一次运行里先解析一次依赖」扩展。
- **相关**：同类的有限重试见 [发版工作流的 rustup 安装重试](2026-11-02-ci-rustup-install-retry.md) 和 [Android nightly 设备套件失败](../bug-fix/2026-10-28-android-nightly-device-failures.md) 里给 NDK 安装加的重试；资源下载的重试见 `scripts/download_retry.py`。

## Verification

- `scripts/test-android-resolve-gradle-classpath.py` 用合成的 `gradlew` 和 `sleep` 覆盖六种情况：一次成功；429 一次后成功（等 60 秒）；wrapper 的 503 一次后成功；一直 429 时三次后放弃（等 60、120 秒）；403 不重试；其他失败保留退出码且不重试。由 `scripts/run-checks.sh` 自动发现并执行。
- 复核属性是否生效：起一个对任何请求都回 429 的本地 HTTP 服务，建一个只从它解析一个依赖的小工程，用同一个 Gradle 发行包分别不带和带上面的 `GRADLE_OPTS` 解析，数服务收到的请求。Gradle 9.5.1 上，缺省是三次（0、1、3 秒）；带 `max.tentatives=4`、`initial.backoff=3000` 是四次（0、3、9、21 秒），写进 Gradle 用户目录 `gradle.properties` 的 `systemProp.` 形式结果相同。只设 network.retry 那组时仍是三次（0、1、3 秒），POM 和 JAR 都一样。
- 在真实工程上，以空的 Gradle 用户目录加上同样的 `GRADLE_OPTS` 跑 `resolve-gradle-classpath.sh`，一次成功。没有在 CI 上真的遇到 429 时跑过，真实效果要看之后的发版和设备作业。

import groovy.json.JsonSlurper
import java.time.LocalDate
import java.util.Properties

plugins { id("com.android.application") }

// The sources stay where they are: platforms/android is the module's real home, and this project is
// only the build. Nothing is copied or generated into it.
val hostRoot = rootDir.parentFile

// The release version: platforms/android/version.txt, or -PmsimeVersion from build-apk.sh when the release workflow is dispatched with another one. Usage reports, 关于 and 反馈 all read versionName, so a fixed value here made every release report the same version.
val releaseVersion = (findProperty("msimeVersion") as String?)?.trim()?.takeIf { it.isNotEmpty() }
    ?: hostRoot.resolve("version.txt").readText().trim()
val releaseVersionParts = Regex("""^(\d{1,3})\.(\d{1,3})\.(\d{1,3})$""").matchEntire(releaseVersion)
    ?.groupValues?.drop(1)?.map { it.toInt() }
    ?: error("Android release version must be major.minor.patch, got '$releaseVersion'")

// 发布日期（`YYYY-MM-DD`），写进 BuildConfig 的 `RELEASE_DATE`，只给关于页显示在版本号后面；versionName 不带它，反馈和使用统计报的仍是纯版本号。依次取：构建时的环境变量 MSIME_RELEASE_DATE；与 version.txt 并列的 release-date.txt（只在源码包里有）；构建所在提交的日期（发版从 main 上 release 分支的合并提交构建，这就是发版当天）；都没有时为空，关于页不显示日期。build-apk.sh 经 Gradle 打包，环境变量原样传到这里，不需要另外转交。
val releaseDatePattern = Regex("""^\d{4}-\d{2}-\d{2}$""")
fun checkedReleaseDate(value: String, source: String): String {
    check(releaseDatePattern.matches(value) && runCatching { LocalDate.parse(value) }.isSuccess) {
        "Android release date from $source must be YYYY-MM-DD, got '$value'"
    }
    return value
}
val releaseDate = providers.environmentVariable("MSIME_RELEASE_DATE").orNull?.trim()?.takeIf { it.isNotEmpty() }
    ?.let { checkedReleaseDate(it, "MSIME_RELEASE_DATE") }
    ?: hostRoot.resolve("release-date.txt").takeIf { it.isFile }?.readText()?.trim()?.takeIf { it.isNotEmpty() }
        ?.let { checkedReleaseDate(it, "platforms/android/release-date.txt") }
    // 不在 git 仓库里或没有 git 时没有日期，不算错误。
    ?: runCatching {
        providers.exec {
            commandLine("git", "log", "-1", "--format=%cs")
            workingDir = hostRoot
            isIgnoreExitValue = true
        }.standardOutput.asText.get().trim()
    }.getOrNull()?.takeIf { releaseDatePattern.matches(it) }
    ?: ""

// 产品版本（edition）：版本表 shared/contracts/editions.json 里每个有 Android 段的版本是一个同名的 productFlavor（full、pinyin、wubi、japanese、vietnamese、tibetan），可以同时装在一台设备上。full 的 applicationId、清单和资源与引入版本之前相同；其他版本的 applicationId 加上 `.<id>`，ContentProvider 的 authority 写成 `${applicationId}.<名字>`，随之各不相同。应用名、输入法子类型名和子类型语言（日文、越南文、藏文版登记在 ja_JP、vi_VN、bo 下）取自 platforms/android/editions/<id>/res，图标所有版本相同。
data class AndroidEdition(
    val id: String,
    val applicationId: String,
    val inputSchemes: List<String>,
    val defaultScheme: String,
    val temporaryJapanese: Boolean,
)

val baseApplicationId = "app.msime.android"
@Suppress("UNCHECKED_CAST")
val androidEditions = (JsonSlurper().parse(hostRoot.resolve("../../shared/contracts/editions.json")) as Map<String, Any?>)
    .let { table -> table["editions"] as List<Map<String, Any?>> }
    .mapNotNull { entry ->
        val android = (entry["platforms"] as Map<String, Any?>)["android"] as Map<String, Any?>? ?: return@mapNotNull null
        AndroidEdition(
            id = entry["id"] as String,
            applicationId = android["application_id"] as String,
            inputSchemes = entry["input_schemes"] as List<String>,
            defaultScheme = entry["default_scheme"] as String,
            temporaryJapanese = (entry["features"] as Map<String, Any?>)["temporary_japanese"] as Boolean,
        )
    }
check(androidEditions.firstOrNull()?.let { it.id == "full" && it.applicationId == baseApplicationId } == true) {
    "shared/contracts/editions.json must list full first, with application_id $baseApplicationId"
}

// APK 携带的 ABI：默认 arm64-v8a，或者 build-apk.sh 从 MSIME_ANDROID_ABIS 转交的逗号分隔的 -PmsimeAbis（x86_64 模拟器用 x86_64）。abiFilters 负责把第三方原生库（ML Kit 还带 x86、x86_64 和 armeabi-v7a）限制在已构建 msime 库的那些 ABI 上；没有它，APK 会装到这些设备上，然后因为缺少宿主库而崩溃。
val nativeAbis = (findProperty("msimeAbis") as String?)
    ?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }?.takeIf { it.isNotEmpty() }
    ?: listOf("arm64-v8a")
check(nativeAbis.all { it == "arm64-v8a" || it == "x86_64" }) {
    "msimeAbis supports arm64-v8a and x86_64, got $nativeAbis"
}

// 分发渠道：默认 github（GitHub release 上的 APK，靠应用内更新），build-apk.sh 在 MSIME_ANDROID_DISTRIBUTION=play 时转交 -PmsimeDistribution=play，打上传 Google Play 的 AAB。Play 渠道写进 BuildConfig 的 `DISTRIBUTION`，宿主据此关掉自更新、不下载可执行的资源包；release 再叠加 platforms/android/distribution/play/AndroidManifest.xml，删掉只为自更新声明的权限。Play 的政策不允许上架的应用绕过 Play 更新自己，也不允许从别处下载可执行代码，见 .agents/notes/implemented/process/2026-10-23-android-google-play-distribution.md。
val distribution = (findProperty("msimeDistribution") as String?)?.trim()?.takeIf { it.isNotEmpty() } ?: "github"
check(distribution == "github" || distribution == "play") {
    "msimeDistribution supports github and play, got '$distribution'"
}

android {
    namespace = "app.msime.android"
    compileSdk = 36
    // 与 build-native.sh 固定的 NDK 相同。AGP 用这个 NDK 的 llvm-strip 处理打包进去的 .so；不配置 NDK 时它会把未 strip 的 .so 原样打包。
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "app.msime.android"
        minSdk = 28
        // Google Play 自 2026-08-31 起要求新应用和更新以 API 36 为目标。
        targetSdk = 36
        // Grows with every release, as the platform requires for an update to install: 0.1.0 is 1000, 1.2.3 is 1002003.
        versionCode = releaseVersionParts[0] * 1_000_000 + releaseVersionParts[1] * 1_000 + releaseVersionParts[2]
        versionName = releaseVersion
        // 宿主经反射读它（AppVersionPolicy.releaseDate），理由同下面 productFlavors 里的 EDITION。
        buildConfigField("String", "RELEASE_DATE", "\"$releaseDate\"")
        // 宿主经反射读它（AppVersionPolicy.playDistribution），理由同上。
        buildConfigField("String", "DISTRIBUTION", "\"$distribution\"")
        ndk { abiFilters += nativeAbis }
    }

    sourceSets.getByName("main") {
        manifest.srcFile(hostRoot.resolve("AndroidManifest.xml"))
        java.setSrcDirs(listOf(hostRoot.resolve("java")))
        res.setSrcDirs(listOf(hostRoot.resolve("res")))
        // The verified dictionary and the native licence notices are staged by build-apk.sh.
        assets.setSrcDirs(listOf(hostRoot.resolve("../../target/android/host-assets")))
        jniLibs.setSrcDirs(listOf(hostRoot.resolve("../../target/android/jniLibs")))
    }
    if (distribution == "play") {
        sourceSets.getByName("release") { manifest.srcFile(hostRoot.resolve("distribution/play/AndroidManifest.xml")) }
    }

    flavorDimensions += "edition"
    productFlavors {
        for (edition in androidEditions) {
            create(edition.id) {
                dimension = "edition"
                if (edition.id != "full") {
                    check(edition.applicationId.startsWith("$baseApplicationId.")) {
                        "edition ${edition.id}: application_id must be $baseApplicationId.<id>"
                    }
                    applicationIdSuffix = edition.applicationId.removePrefix(baseApplicationId)
                }
                // 宿主运行时由 AppEdition 读这几项（javac 直接编译的那部分代码引用不到生成的 BuildConfig，所以经反射读）：版本 id、本版本的输入方案（逗号分隔，顺序同版本表）、默认方案和是否带临时日语。
                buildConfigField("String", "EDITION", "\"${edition.id}\"")
                buildConfigField("String", "EDITION_INPUT_SCHEMES", "\"${edition.inputSchemes.joinToString(",")}\"")
                buildConfigField("String", "EDITION_DEFAULT_SCHEME", "\"${edition.defaultScheme}\"")
                buildConfigField("boolean", "EDITION_TEMPORARY_JAPANESE", edition.temporaryJapanese.toString())
            }
        }
    }

    for (edition in androidEditions) {
        if (edition.id == "full") continue
        sourceSets.getByName(edition.id) {
            res.setSrcDirs(listOf(hostRoot.resolve("editions/${edition.id}/res")))
        }
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            // 由 build-apk.sh 事后签名：发布时用 release-android.yml 从仓库 secrets 解出的发布密钥，本机开发构建用所有 worktree 共用的开发密钥。
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        viewBinding = false
        buildConfig = true
    }

    packaging {
        jniLibs.useLegacyPackaging = false
    }

    // lint 在这里只做 API 级别的门禁：CI 编译完各版本后跑 lintFullRelease，宿主调用了高于 minSdk 的 API 时 NewApi 让它失败。javac 按 compileSdk 编译看不出来，D8 也只补齐其中一部分调用；0.2.0 就这样带上了 Path.of（API 34），Android 9 到 12 的设备准备词库时全部 NoSuchMethodError。其余 lint 检查不在这里把关。各版本只有资源不同，Java 全在 main，lint full 一个版本即可。
    lint {
        checkOnly += "NewApi"
        abortOnError = true
    }
}

dependencies {
    implementation("androidx.autofill:autofill:1.3.0")
    implementation("androidx.appcompat:appcompat:1.8.0")
    // Held at 1.13.1: androidx.core 1.19 declares a minimum of AGP 9.1, and AGP cannot move here without the Gradle wrapper, which belongs to the Tauri bundle. The other four in this group carry no such floor.
    implementation("androidx.core:core:1.13.1")
    implementation("androidx.activity:activity:1.10.1")
    implementation("androidx.biometric:biometric:1.1.0")
    implementation("androidx.constraintlayout:constraintlayout:2.2.2")
    implementation("androidx.recyclerview:recyclerview:1.4.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
    implementation("com.google.android.material:material:1.14.0")
    implementation("com.google.mlkit:digital-ink-recognition:19.0.0")
    // Notice bodies on the 设置 tab are simple Markdown. Core only: no HTML plugin, so raw HTML in a notice is never interpreted, and no image loader.
    implementation("io.noties.markwon:core:4.6.2")
    // Google 登录。Credential Manager 是 Google 现在的官方入口，旧的 GoogleSignInClient 已弃用；googleid 提供「通过 Google 登录」按钮用的 GetSignInWithGoogleOption（不是底部弹出一键登录用的 GetGoogleIdOption，见 GoogleSignInFlow），play-services-auth 那件是它在设备上的实现。googleid 1.2.1 依赖 credentials 1.6.0，androidx.credentials 又是一个版本一致的原子组，所以原来写的 1.3.0 实际都解析成 1.6.0；这里直接写解析出来的版本，免得读的人按 1.3.0 去查行为。
    implementation("androidx.credentials:credentials:1.6.0")
    implementation("androidx.credentials:credentials-play-services-auth:1.6.0")
    implementation("com.google.android.libraries.identity.googleid:googleid:1.2.1")
}

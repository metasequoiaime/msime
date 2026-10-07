import groovy.json.JsonSlurper
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

android {
    namespace = "app.msime.android"
    compileSdk = 36
    // 与 build-native.sh 固定的 NDK 相同。AGP 用这个 NDK 的 llvm-strip 处理打包进去的 .so；不配置 NDK 时它会把未 strip 的 .so 原样打包。
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "app.msime.android"
        minSdk = 28
        targetSdk = 35
        // Grows with every release, as the platform requires for an update to install: 0.1.0 is 1000, 1.2.3 is 1002003.
        versionCode = releaseVersionParts[0] * 1_000_000 + releaseVersionParts[1] * 1_000 + releaseVersionParts[2]
        versionName = releaseVersion
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
    implementation("androidx.appcompat:appcompat:1.8.0")
    // Held at 1.13.1: androidx.core 1.19 declares a minimum of AGP 9.1, and AGP cannot move here without the Gradle wrapper, which belongs to the Tauri bundle. The other four in this group carry no such floor.
    implementation("androidx.core:core:1.13.1")
    implementation("androidx.activity:activity:1.10.1")
    implementation("androidx.constraintlayout:constraintlayout:2.2.2")
    implementation("androidx.recyclerview:recyclerview:1.4.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
    implementation("com.google.android.material:material:1.14.0")
    implementation("com.google.mlkit:digital-ink-recognition:19.0.0")
    // Notice bodies on the 设置 tab are simple Markdown. Core only: no HTML plugin, so raw HTML in a notice is never interpreted, and no image loader.
    implementation("io.noties.markwon:core:4.6.2")
    // Google 登录。Credential Manager 是 Google 现在的官方入口，旧的 GoogleSignInClient 已弃用；
    // googleid 提供那颗按钮要的 GetGoogleIdOption，play-services-auth 那件是它在设备上的实现。
    implementation("androidx.credentials:credentials:1.3.0")
    implementation("androidx.credentials:credentials-play-services-auth:1.3.0")
    implementation("com.google.android.libraries.identity.googleid:googleid:1.2.1")
}

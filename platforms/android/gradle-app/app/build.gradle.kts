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

android {
    namespace = "app.msime.android"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.msime.android"
        minSdk = 28
        targetSdk = 35
        // Grows with every release, as the platform requires for an update to install: 0.1.0 is 1000, 1.2.3 is 1002003.
        versionCode = releaseVersionParts[0] * 1_000_000 + releaseVersionParts[1] * 1_000 + releaseVersionParts[2]
        versionName = releaseVersion
    }

    sourceSets.getByName("main") {
        manifest.srcFile(hostRoot.resolve("AndroidManifest.xml"))
        java.setSrcDirs(listOf(hostRoot.resolve("java")))
        res.setSrcDirs(listOf(hostRoot.resolve("res")))
        // The verified dictionary and the native licence notices are staged by build-apk.sh.
        assets.setSrcDirs(listOf(hostRoot.resolve("../../target/android/host-assets")))
        jniLibs.setSrcDirs(listOf(hostRoot.resolve("../../target/android/jniLibs")))
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            // Signed afterwards by build-apk.sh with the development key, as the aapt2 path was.
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures { viewBinding = false }

    packaging {
        jniLibs.useLegacyPackaging = false
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.8.0")
    // Held at 1.13.1: androidx.core 1.19 declares a minimum of AGP 9.1, and AGP cannot move here without the Gradle wrapper, which belongs to the Tauri bundle. The other four in this group carry no such floor.
    implementation("androidx.core:core:1.19.1")
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

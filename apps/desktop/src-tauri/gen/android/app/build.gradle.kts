import groovy.json.JsonSlurper
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val clientRoot = rootProject.file("../../../../..")

// 产品版本：platforms/android/build-client-apk.sh 以 ORG_GRADLE_PROJECT_msimeEdition=<id> 传入，缺省是 full。这个工程的 abi 维度由 Tauri 的 Rust 插件占用，所以版本不做成 flavor，而是直接写进 defaultConfig：applicationId 加上版本后缀、BuildConfig 写入 AppEdition 读的那几项，应用名从 platforms/android/editions/<id>/res 覆盖（放在 debug、release 两个 buildType 的源集里，它们的优先级高于 main）。取值与 platforms/android/gradle-app 的同名 flavor 相同，都来自 shared/contracts/editions.json。
val msimeEditionId = (findProperty("msimeEdition") as String?)?.trim()?.takeIf { it.isNotEmpty() } ?: "full"
@Suppress("UNCHECKED_CAST")
val msimeEdition = ((JsonSlurper().parse(clientRoot.resolve("shared/contracts/editions.json")) as Map<String, Any?>)["editions"] as List<Map<String, Any?>>)
    .firstOrNull { it["id"] == msimeEditionId && (it["platforms"] as Map<String, Any?>)["android"] != null }
    ?: error("edition $msimeEditionId has no Android identifiers in shared/contracts/editions.json")
@Suppress("UNCHECKED_CAST")
val msimeEditionApplicationId = ((msimeEdition["platforms"] as Map<String, Any?>)["android"] as Map<String, Any?>)["application_id"] as String

android {
    compileSdk = 36
    // Use the native client package for generated Tauri Kotlin and Android resources.
    namespace = "app.msime.android"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        // Keep the installed package stable for the Android client.
        applicationId = "app.msime.android"
        minSdk = 28
        targetSdk = 35
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        if (msimeEditionId != "full") {
            check(msimeEditionApplicationId.startsWith("app.msime.android.")) { "edition $msimeEditionId: application_id must be app.msime.android.<id>" }
            applicationIdSuffix = msimeEditionApplicationId.removePrefix("app.msime.android")
        }
        @Suppress("UNCHECKED_CAST")
        val schemes = (msimeEdition["input_schemes"] as List<String>).joinToString(",")
        buildConfigField("String", "EDITION", "\"$msimeEditionId\"")
        buildConfigField("String", "EDITION_INPUT_SCHEMES", "\"$schemes\"")
        buildConfigField("String", "EDITION_DEFAULT_SCHEME", "\"${msimeEdition["default_scheme"]}\"")
        @Suppress("UNCHECKED_CAST")
        val temporaryJapanese = (msimeEdition["features"] as Map<String, Any?>)["temporary_japanese"] as Boolean
        buildConfigField("boolean", "EDITION_TEMPORARY_JAPANESE", temporaryJapanese.toString())
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets.getByName("main") {
        java.srcDir(clientRoot.resolve("platforms/android/java"))
        res.setSrcDirs(listOf("src/main/res-msime", clientRoot.resolve("platforms/android/res"), clientRoot.resolve("apps/desktop/src-tauri/icons/android")))
        assets.srcDir(clientRoot.resolve("target/android/tauri-assets"))
        // Tauri supplies libc++_shared from the pinned NDK; this directory contains only MSIME libs.
        jniLibs.srcDir(clientRoot.resolve("target/android/tauri-jniLibs"))
    }
    if (msimeEditionId != "full") {
        for (profile in listOf("debug", "release")) {
            sourceSets.getByName(profile).res.srcDir(clientRoot.resolve("platforms/android/editions/$msimeEditionId/res"))
        }
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("com.google.mlkit:digital-ink-recognition:19.0.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    implementation("androidx.credentials:credentials:1.3.0")
    implementation("androidx.credentials:credentials-play-services-auth:1.3.0")
    implementation("com.google.android.libraries.identity.googleid:googleid:1.1.1")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
    implementation(project(":tauri-android"))
}

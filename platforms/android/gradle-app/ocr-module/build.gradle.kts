plugins { id("com.android.application") }

val repoRoot = rootDir.resolve("../../..")
// 模型和字典由 scripts/fetch_ocr_models.py 按 resources/ocr-models-android.lock.json 下载校验后放进这个目录（只含 ocr/ 下三份文件）；下载缓存在 target/ocr-models，不进 APK。
val modelAssets = repoRoot.resolve("target/android/ocr-module-assets")
// 许可证全文随模块分发：onnxruntime 的 MIT 和第三方通知、PaddleOCR 的 Apache-2.0，与 resources/licenses 逐字节相同。
val licenseAssets = layout.buildDirectory.dir("generated/ocr-licenses")

val fetchOcrModels by tasks.registering(Exec::class) {
    description = "Fetches and verifies the pinned PP-OCRv6 tiny models and CTC dictionary."
    workingDir = repoRoot
    commandLine(providers.environmentVariable("MSIME_PYTHON").orElse("python3").get(),
        "scripts/fetch_ocr_models.py", "--out", modelAssets.absolutePath)
    inputs.file(repoRoot.resolve("resources/ocr-models-android.lock.json"))
    inputs.file(repoRoot.resolve("scripts/fetch_ocr_models.py"))
    outputs.dir(modelAssets)
}

val stageOcrLicenses by tasks.registering(Sync::class) {
    from(repoRoot.resolve("resources/licenses")) {
        include("onnxruntime-MIT.txt", "onnxruntime-ThirdPartyNotices.txt", "PaddleOCR-Apache-2.0.txt")
    }
    into(licenseAssets.map { it.dir("licenses") })
}

android {
    namespace = "app.msime.ocr.module"
    compileSdk = 36
    ndkVersion = "28.2.13676358"
    defaultConfig {
        applicationId = "app.msime.ocr.module"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // 原生库以 STORED 方式按 16 KB 对齐放进 APK，宿主直接从只读 APK 的 lib/<abi> 映射，输入法进程不解压可执行文件。
    packaging { jniLibs.useLegacyPackaging = false }
    sourceSets.getByName("main") {
        assets.srcDir(modelAssets)
        assets.srcDir(licenseAssets)
    }
    sourceSets.getByName("debug").java.srcDir(rootDir.resolve("ocr-fixture"))
    lint { checkOnly += "NewApi"; abortOnError = true }
}

tasks.named("preBuild") { dependsOn(fetchOcrModels, stageOcrLicenses) }

dependencies {
    // MIT。只进这个可选模块，基础 APK 不依赖它；版本与 resources/ocr-models-android.lock.json 的 runtime 一致。
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.31.0")
}

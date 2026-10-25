plugins { id("com.android.application") }

android {
    namespace = "app.msime.ocr.probe"
    compileSdk = 36
    ndkVersion = "28.2.13676358"
    defaultConfig {
        applicationId = "app.msime.ocr.probe"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets.getByName("main") {
        assets.srcDir(rootDir.resolve("../../../target/android/ocr-probe-assets"))
        java.srcDir(rootDir.resolve("ocr-fixture"))
    }
    lint { checkOnly += "NewApi"; abortOnError = true }
}

dependencies {
    // 模拟产品已有的手写依赖；OCR 依赖只在另一个 APK 内。
    implementation("com.google.mlkit:digital-ink-recognition:19.0.0")
}

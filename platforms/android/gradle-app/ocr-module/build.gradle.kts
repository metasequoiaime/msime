plugins { id("com.android.application") }

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
    sourceSets.getByName("debug").java.srcDir(rootDir.resolve("ocr-fixture"))
    lint { checkOnly += "NewApi"; abortOnError = true }
}

dependencies {
    implementation("com.google.mlkit:text-recognition-chinese:16.0.1")
}

// 原生宿主的 Gradle 工程，只依赖 AndroidX 和 Material；构建入口是本目录自带的 Gradle wrapper（gradlew）。
pluginManagement {
    repositories { google(); mavenCentral() }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "msime-android-host"
include(":app")

// OCR 模块与隔离探针只由专项显式构建，不进入基础输入法的依赖图。
val ocrProbe = providers.gradleProperty("msimeOcrProbe").orNull == "true"
if (ocrProbe || providers.gradleProperty("msimeOcrModule").orNull == "true") include(":ocr-module")
if (ocrProbe) include(":ocr-probe")

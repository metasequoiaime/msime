plugins {
    // AGP 9.x 要求 Gradle 9.6 以上，本工程的 Gradle wrapper（gradle/wrapper/gradle-wrapper.properties）是 8.14.3，两者要一起升：只升 AGP 会让 build-apk.sh 直接失败（2026-10-02 Dependabot 升到 9.4.1 后 release-android 编不出 APK）。
    id("com.android.application") version "8.13.2" apply false
}

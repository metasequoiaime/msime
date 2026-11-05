plugins {
    // AGP 与本工程的 Gradle wrapper（gradle/wrapper/gradle-wrapper.properties，9.5.1）要一起升：AGP 9.x 要求 Gradle 9.6 以上，只升 AGP 会让 build-apk.sh 直接失败（2026-10-02 Dependabot 升到 9.4.1 后 release-android 编不出 APK）；反过来 AGP 8.x 用了 Gradle 9.6 删掉的内部 API（org.gradle.api.problems.internal.InternalProblems），wrapper 单独升到 9.6 以上同样构建不了（#10769 的 9.8.2）。
    id("com.android.application") version "8.13.2" apply false
}

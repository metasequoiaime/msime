plugins {
    // Pinned to the version the Tauri bundle resolves (apps/desktop/src-tauri/gen/android/build.gradle.kts), because both projects are driven by that bundle's Gradle wrapper. AGP 9 requires Gradle 9.6, the wrapper ships 8.14.3, and the generated project is regenerated rather than bumped -- so this cannot move on its own.
    // 与 apps/desktop/src-tauri/gen/android 的 AGP 保持一致：build-apk.sh 用 Tauri 工程的 Gradle 8.14.3 wrapper 构建本工程，AGP 9.x 要求 Gradle 9.6 以上，会直接失败（2026-10-02 Dependabot 升到 9.4.1 后 release-android 编不出 APK）。
    id("com.android.application") version "8.13.2" apply false
}

plugins {
    // Pinned to the version the Tauri bundle resolves (apps/desktop/src-tauri/gen/android/build.gradle.kts), because both projects are driven by that bundle's Gradle wrapper. AGP 9 requires Gradle 9.6, the wrapper ships 8.14.3, and the generated project is regenerated rather than bumped -- so this cannot move on its own.
    id("com.android.application") version "9.4.1" apply false
}

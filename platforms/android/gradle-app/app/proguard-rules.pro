# 只裁剪未引用代码；保留诊断中的原始名字和方法行为。
-dontobfuscate
-dontoptimize

# 宿主含反射构造、BuildConfig 字段与 JNI 回调，第一步完整保留该动态契约。
-keep class app.msime.android.** { *; }
-keep interface app.msime.android.** { *; }

# ClipboardImageCache 经反射调用 FileProvider 的静态方法。
-keep class androidx.core.content.FileProvider { *; }

# 只裁剪未引用代码；保留诊断中的原始名字和方法行为。
-dontobfuscate
-dontoptimize

# 宿主含反射构造、BuildConfig 字段与 JNI 回调，第一步完整保留该动态契约。
-keep class app.msime.android.** { *; }
-keep interface app.msime.android.** { *; }

# ClipboardImageCache 经反射调用 FileProvider 的静态方法。
-keep class androidx.core.content.FileProvider { *; }

# 设备回归经反射重建 Fragment 视图，保留与未裁剪包一致的生命周期入口。
-keepclassmembers class androidx.fragment.app.FragmentTransaction {
    public androidx.fragment.app.FragmentTransaction attach(androidx.fragment.app.Fragment);
    public androidx.fragment.app.FragmentTransaction detach(androidx.fragment.app.Fragment);
}

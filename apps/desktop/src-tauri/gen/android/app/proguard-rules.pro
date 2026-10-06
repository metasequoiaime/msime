# Add project specific ProGuard rules here.
-keep class app.msime.android.NativeClient { *; }
-keep class app.msime.android.MlKitHandwritingRecognizer { public <init>(android.content.Context); }
# AppEdition 经反射读版本声明；不留下这几个字段，R8 会把它们当作没人用的常量删掉，五笔版就会被当成 full 运行。
-keep class app.msime.android.BuildConfig { public static final *** EDITION*; }
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

#!/usr/bin/env python3
"""Android 的 JNI 名字在三处必须一致，host 导出清单里的符号必须在头文件里声明。

`NativeClient.java` 的 `native` 方法、`client_jni.cpp` 的 `Java_app_msime_android_NativeClient_*` 导出和 `verify-native.sh` 的 JNI 导出清单各写一份名字。check-host 只对 `client_jni.cpp` 做语法编译，不链接，也不比较 Java 与 C++ 的名字：Java 声明了却没有导出的方法要到设备上第一次调用时才以 `UnsatisfiedLinkError` 暴露，清单漏掉的导出则让 `build-native.sh` 的检查形同虚设。

第二个问题是 `verify-native.sh` 的 host 导出清单：其中每个 `msime_client_*` 都必须在 `crates/host-api/include/msime_client.h` 里声明，否则清单在检查一个不存在的契约。

失败时打印各集合之间的差集。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
JAVA = ROOT / "platforms/android/java/app/msime/android/core/NativeClient.java"
CPP = ROOT / "platforms/android/native/client_jni.cpp"
VERIFY = ROOT / "platforms/android/verify-native.sh"
HEADER = ROOT / "crates/host-api/include/msime_client.h"

# `private static native byte[] nameRaw(` 之类的声明，参数可以跨行。
JAVA_NATIVE = re.compile(r"\bnative\s+[\w\[\]<>.]+\s+(\w+)\s*\(")
CPP_EXPORT = re.compile(r"\bJava_app_msime_android_NativeClient_(\w+)\s*\(")
VERIFY_METHODS = re.compile(r"for method in ([^;]+); do")
VERIFY_SYMBOLS = re.compile(r"for symbol in ([^;]+); do")
HEADER_DECL = re.compile(r"\b(msime_client_\w+)\s*\(")


def strip_comments(text: str) -> str:
    """去掉 C/C++/Java 注释，避免注释里提到的名字被当成声明。"""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def report(name_a: str, set_a: set, name_b: str, set_b: set) -> bool:
    ok = True
    for label, missing in ((f"{name_a} 有、{name_b} 没有", set_a - set_b), (f"{name_b} 有、{name_a} 没有", set_b - set_a)):
        if missing:
            ok = False
            print(f"{label}: {', '.join(sorted(missing))}", file=sys.stderr)
    return ok


def main() -> int:
    java = set(JAVA_NATIVE.findall(strip_comments(JAVA.read_text(encoding="utf-8"))))
    cpp = set(CPP_EXPORT.findall(strip_comments(CPP.read_text(encoding="utf-8"))))
    verify_text = VERIFY.read_text(encoding="utf-8")
    methods = VERIFY_METHODS.search(verify_text)
    symbols = VERIFY_SYMBOLS.search(verify_text)
    if not java or not cpp or methods is None or symbols is None:
        print("没有读到 NativeClient 的 native 方法、client_jni.cpp 的导出或 verify-native.sh 的清单", file=sys.stderr)
        return 1
    listed = set(methods.group(1).split())
    host_symbols = set(symbols.group(1).split())
    declared = set(HEADER_DECL.findall(strip_comments(HEADER.read_text(encoding="utf-8"))))

    ok = report("NativeClient.java", java, "client_jni.cpp", cpp)
    ok = report("client_jni.cpp", cpp, "verify-native.sh JNI 清单", listed) and ok
    undeclared = host_symbols - declared
    if undeclared:
        ok = False
        print(f"verify-native.sh host 清单里没有在 msime_client.h 声明的符号: {', '.join(sorted(undeclared))}", file=sys.stderr)
    if not ok:
        return 1
    print(f"JNI names: {len(java)} native methods match client_jni.cpp and verify-native.sh; {len(host_symbols)} host exports are declared in msime_client.h")
    return 0


if __name__ == "__main__":
    sys.exit(main())

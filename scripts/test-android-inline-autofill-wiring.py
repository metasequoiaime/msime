#!/usr/bin/env python3
"""检查原生包与开发合包的内联填充声明、依赖和反射入口不会漂移。"""
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[1]
native = root / "platforms/android"
tauri = root / "apps/desktop/src-tauri/gen/android/app"
attribute = "{http://schemas.android.com/apk/res/android}supportsInlineSuggestions"
methods = [native / "res/xml/method.xml", tauri / "src/main/res-msime/xml/tauri_method.xml"]
methods += list((native / "editions").glob("*/res/xml/method.xml"))
methods += list((tauri / "src/editions").glob("*/res-msime/xml/tauri_method.xml"))
for method in methods:
    assert ET.parse(method).getroot().get(attribute) == "true", f"Missing inline capability: {method.relative_to(root)}"
for gradle in [native / "gradle-app/app/build.gradle.kts", tauri / "build.gradle.kts"]:
    assert 'implementation("androidx.autofill:autofill:1.3.0")' in gradle.read_text(), f"Missing official style dependency: {gradle.relative_to(root)}"
rules = (tauri / "proguard-rules.pro").read_text()
assert "-keep class app.msime.android.AndroidXInlineAutofillStyles { public static android.os.Bundle create(android.os.Bundle, int, int); }" in rules, "R8 must preserve the reflected style entry"
print(f"Android inline autofill: {len(methods)} metadata declarations, both Gradle dependencies and R8 entry passed")

#!/usr/bin/env python3
"""检查原生包的内联填充声明和依赖不会漂移。"""
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[1]
native = root / "platforms/android"
attribute = "{http://schemas.android.com/apk/res/android}supportsInlineSuggestions"
methods = [native / "res/xml/method.xml"]
methods += list((native / "editions").glob("*/res/xml/method.xml"))
for method in methods:
    assert ET.parse(method).getroot().get(attribute) == "true", f"Missing inline capability: {method.relative_to(root)}"
gradle = native / "gradle-app/app/build.gradle.kts"
assert 'implementation("androidx.autofill:autofill:1.3.0")' in gradle.read_text(), f"Missing official style dependency: {gradle.relative_to(root)}"
print(f"Android inline autofill: {len(methods)} metadata declarations and the Gradle dependency passed")

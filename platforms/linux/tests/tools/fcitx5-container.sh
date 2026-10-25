#!/usr/bin/env bash
set -euo pipefail
[[ ${MSIME_ISOLATED_LINUX_TEST:-} == 1 && -d /resources && -d /build ]] || exit 2

cmake -S platforms/linux -B /build/fcitx5 -G Ninja \
  -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so \
  -DMSIME_ENABLE_FCITX5=ON -DMSIME_ENABLE_PACKAGING=OFF \
  -DMSIME_FCITX5_TEST_RESOURCES=/resources
cmake --build /build/fcitx5 --target msime-fcitx5 fcitx5-native-test linux-candidate-fcitx-canvas-test linux-candidate-native-canvas-test linux-candidate-native-canvas-chrome-test
# 用例名逐个列出、只用一组括号：CMake 的正则最多容纳 10 组括号，原先按前缀嵌套的写法超出上限，ctest 报「Too many parentheses」后一个用例也不跑。
names=(linux-candidate-fcitx-canvas linux-candidate-native-canvas linux-candidate-native-canvas-chrome
  fcitx5-custom-voice-hotkey fcitx5-helpcode-trigger fcitx5-candidate-rows fcitx5-native-context fcitx5-native-ai fcitx5-native-smart-abc fcitx5-daemon-frontend
  fcitx5-gtk-editor fcitx5-ibus-gtk-editor fcitx5-mode-hint-preferences
  fcitx5-shuangpin-helpcode-editor-on fcitx5-shuangpin-helpcode-editor-off)
for layout in horizontal vertical; do
  names+=("fcitx5-candidate-font-pixels-$layout")
  names+=("fcitx5-translation-below-$layout" "fcitx5-gtk-canvas-$layout" "fcitx5-gtk-canvas-$layout-gif"
    "fcitx5-gtk-canvas-$layout-webp" "fcitx5-native-canvas-$layout" "fcitx5-native-canvas-$layout-scale2"
    "fcitx5-native-canvas-$layout-composited" "fcitx5-native-chrome-$layout" "fcitx5-native-chrome-$layout-composited"
    "fcitx5-wayland-canvas-$layout-png"
    "fcitx5-wayland-canvas-$layout-gif" "fcitx5-wayland-canvas-$layout-webp")
done
pattern="^($(IFS='|'; echo "${names[*]}"))\$"
ctest --test-dir /build/fcitx5 -R "$pattern" \
  --output-on-failure --output-junit /build/fcitx5-acceptance.xml
# 专用镜像提供全部依赖；X11 与 Wayland 的二十四项真实窗口用例不能靠跳过得到验收通过。
python3 - /build/fcitx5-acceptance.xml <<'PY'
import sys
import subprocess
import xml.etree.ElementTree as ET

report = ET.parse(sys.argv[1]).getroot()
for state in ("on", "off"):
    name = f"fcitx5-shuangpin-helpcode-editor-{state}"
    tests = [test for test in report.findall("testcase") if test.get("name") == name]
    assert len(tests) == 1 and tests[0].get("status") == "run" and tests[0].find("skipped") is None, f"{name}: 未执行"

mode_hints = [test for test in report.findall("testcase") if test.get("name") == "fcitx5-mode-hint-preferences"]
if subprocess.run(["pkg-config", "--atleast-version=5.1.11", "Fcitx5Core"], check=False).returncode == 0:
    assert len(mode_hints) == 1 and mode_hints[0].get("status") == "run" and mode_hints[0].find("skipped") is None, "聚焦提示验收未执行"

for layout in ("horizontal", "vertical"):
    names = [f"fcitx5-candidate-font-pixels-{layout}"]
    names += [f"fcitx5-gtk-canvas-{layout}{suffix}" for suffix in ("", "-gif", "-webp")]
    names += [f"fcitx5-wayland-canvas-{layout}-{format}" for format in ("png", "gif", "webp")]
    names += [f"fcitx5-native-canvas-{layout}{suffix}" for suffix in ("", "-scale2", "-composited")]
    names += [f"fcitx5-native-chrome-{layout}{suffix}" for suffix in ("", "-composited")]
    for name in names:
        tests = [test for test in report.findall("testcase") if test.get("name") == name]
        assert len(tests) == 1, f"{name}: 未注册"
        assert tests[0].get("status") == "run" and tests[0].find("skipped") is None, f"{name}: 被跳过"
PY
echo "Fcitx5 native addon, daemon, and GTK editor acceptance passed"

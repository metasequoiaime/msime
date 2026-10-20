#!/usr/bin/env bash
set -euo pipefail
[[ ${MSIME_ISOLATED_LINUX_TEST:-} == 1 && -d /resources && -d /build ]] || exit 2

cmake -S platforms/linux -B /build/fcitx5 -G Ninja \
  -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so \
  -DMSIME_ENABLE_FCITX5=ON -DMSIME_ENABLE_PACKAGING=OFF \
  -DMSIME_FCITX5_TEST_RESOURCES=/resources
cmake --build /build/fcitx5 --target msime-fcitx5 fcitx5-native-test linux-candidate-fcitx-canvas-test linux-candidate-native-canvas-test
ctest --test-dir /build/fcitx5 -R '^(linux-candidate-(fcitx-canvas|native-canvas)|fcitx5-(candidate-rows|native-context|native-ai|native-smart-abc|daemon-frontend|gtk-editor|translation-below-(horizontal|vertical)|gtk-canvas-(horizontal|vertical)(-(gif|webp))?|native-canvas-(horizontal|vertical)(-(scale2|composited))?|wayland-canvas-(horizontal|vertical)-(png|gif|webp)|ibus-gtk-editor))$' \
  --output-on-failure --output-junit /build/fcitx5-acceptance.xml
# 专用镜像提供全部依赖；X11 与 Wayland 的十八项真实窗口用例不能靠跳过得到验收通过。
python3 - /build/fcitx5-acceptance.xml <<'PY'
import sys
import xml.etree.ElementTree as ET

report = ET.parse(sys.argv[1]).getroot()
for layout in ("horizontal", "vertical"):
    names = [f"fcitx5-gtk-canvas-{layout}{suffix}" for suffix in ("", "-gif", "-webp")]
    names += [f"fcitx5-wayland-canvas-{layout}-{format}" for format in ("png", "gif", "webp")]
    names += [f"fcitx5-native-canvas-{layout}{suffix}" for suffix in ("", "-scale2", "-composited")]
    for name in names:
        tests = [test for test in report.findall("testcase") if test.get("name") == name]
        assert len(tests) == 1, f"{name}: 未注册"
        assert tests[0].get("status") == "run" and tests[0].find("skipped") is None, f"{name}: 被跳过"
PY
echo "Fcitx5 native addon, daemon, and GTK editor acceptance passed"

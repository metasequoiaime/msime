#!/usr/bin/env python3
"""用合成 SDK 验证模拟器依赖安装的重试与完整性判断。"""
from pathlib import Path
import os
import platform
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
INSTALLER = ROOT / "platforms/android/install-sdk-packages.sh"


@unittest.skipUnless(platform.system() in ("Darwin", "Linux"), "仅支持 macOS/Linux 的 SDK 安装")
class InstallSdkPackagesTests(unittest.TestCase):
    api = "35"

    def run_installer(self, mode, installed=False, api=None):
        api = self.api if api is None else api
        image_api = "37.0" if api == "37" else api
        with tempfile.TemporaryDirectory(prefix="msime-sdk-test-") as directory:
            sdk = Path(directory)
            manager = sdk / "cmdline-tools/latest/bin/sdkmanager"
            manager.parent.mkdir(parents=True)
            manager.write_text(
                r'''#!/usr/bin/env bash
set -eu
test "$1" = --install
shift
test "$#" -ge 1
found=false
for package in "$@"; do
  if [[ "$package" = "system-images;android-${SDK_API};default;x86_64" ]]; then found=true; fi
done
[[ "$found" = true ]]
count=0
if [[ -f "$ANDROID_SDK_ROOT/count" ]]; then read -r count < "$ANDROID_SDK_ROOT/count"; fi
echo "$((count + 1))" > "$ANDROID_SDK_ROOT/count"
if [[ "$MODE" = fail || ( "$MODE" = retry && "$count" = 0 ) ]]; then
  if [[ "$MODE" = retry ]]; then
    mkdir -p "$ANDROID_SDK_ROOT/.downloadIntermediates" "$ANDROID_SDK_ROOT/.temp/PackageOperation01"
    echo partial > "$ANDROID_SDK_ROOT/.downloadIntermediates/emulator.zip"
    touch "$ANDROID_SDK_ROOT/.temp/PackageOperation01/.prepareComplete"
  fi
  exit 1
fi
if [[ "$MODE" = corrupt && "$count" = 0 ]]; then
  mkdir -p "$ANDROID_SDK_ROOT/.downloadIntermediates" "$ANDROID_SDK_ROOT/.temp/PackageOperation01"
  echo partial > "$ANDROID_SDK_ROOT/.downloadIntermediates/system-image.zip"
  touch "$ANDROID_SDK_ROOT/.temp/PackageOperation01/.prepareComplete"
  mkdir -p "$ANDROID_SDK_ROOT/system-images/android-${SDK_API}/default/x86_64"
  echo partial > "$ANDROID_SDK_ROOT/system-images/android-${SDK_API}/default/x86_64/package.xml"
  exit 1
fi
if [[ "$MODE" = corrupt && ( -e "$ANDROID_SDK_ROOT/.downloadIntermediates" || -e "$ANDROID_SDK_ROOT/.temp" ) ]]; then
  exit 1
fi
mkdir -p "$ANDROID_SDK_ROOT/emulator" "$ANDROID_SDK_ROOT/platform-tools"
mkdir -p "$ANDROID_SDK_ROOT/build-tools/35.0.0" "$ANDROID_SDK_ROOT/platforms/android-35"
mkdir -p "$ANDROID_SDK_ROOT/platforms/android-36"
mkdir -p "$ANDROID_SDK_ROOT/system-images/android-${SDK_API}/default/x86_64"
for package in emulator platform-tools build-tools/35.0.0 platforms/android-35 platforms/android-36 system-images/android-${SDK_API}/default/x86_64; do
  echo package > "$ANDROID_SDK_ROOT/$package/package.xml"
done
touch "$ANDROID_SDK_ROOT/emulator/emulator" "$ANDROID_SDK_ROOT/platform-tools/adb"
chmod +x "$ANDROID_SDK_ROOT/emulator/emulator" "$ANDROID_SDK_ROOT/platform-tools/adb"
'''
            )
            manager.chmod(0o755)
            fake_sleep = sdk / "sleep"
            fake_sleep.write_text("#!/usr/bin/env bash\nexit 0\n")
            fake_sleep.chmod(0o755)
            if installed:
                for package in (
                    "emulator",
                    "platform-tools",
                    "build-tools/35.0.0",
                    "platforms/android-35",
                    "platforms/android-36",
                    f"system-images/android-{image_api}/default/x86_64",
                ):
                    package_dir = sdk / package
                    package_dir.mkdir(parents=True)
                    (package_dir / "package.xml").write_text("package\n")
                for binary in (sdk / "emulator/emulator", sdk / "platform-tools/adb"):
                    binary.touch(mode=0o755)
            result = subprocess.run(
                ["bash", str(INSTALLER), api],
                env={
                    **os.environ,
                    "ANDROID_SDK_ROOT": str(sdk),
                    "SDK_API": image_api,
                    "MODE": mode,
                    "PATH": str(sdk) + os.pathsep + os.environ["PATH"],
                },
                capture_output=True,
                text=True,
                timeout=15,
            )
            count = int((sdk / "count").read_text()) if (sdk / "count").exists() else 0
            return result.returncode, count

    def test_download_failure_then_success(self):
        self.assertEqual(self.run_installer("retry"), (0, 2))

    def test_partial_download_is_cleared_before_retry(self):
        self.assertEqual(self.run_installer("corrupt"), (0, 2))

    def test_persistent_failure_stops_after_three_attempts(self):
        self.assertEqual(self.run_installer("fail"), (1, 3))

    def test_existing_complete_sdk_is_reused(self):
        self.assertEqual(self.run_installer("fail", installed=True), (0, 0))

    def test_android17_uses_versioned_image_package(self):
        self.assertEqual(self.run_installer("corrupt", api="37"), (0, 2))
        self.assertEqual(self.run_installer("fail", installed=True, api="37"), (0, 0))


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""用合成 SDK 验证 NDK 安装重试与完整性判断，不访问网络或本机 SDK。"""
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(platform.system() in ("Darwin", "Linux"), "仅支持 macOS/Linux 的 NDK 安装")
class InstallNdkTests(unittest.TestCase):
    def run_installer(self, mode, installed=False):
        with tempfile.TemporaryDirectory(prefix="msime-ndk-test-") as directory:
            sdk = Path(directory)
            manager = sdk / "cmdline-tools/latest/bin/sdkmanager"
            manager.parent.mkdir(parents=True)
            manager.write_text('''#!/usr/bin/env bash
set -eu
test "$1" = --install
test "$2" = 'ndk;28.2.13676358'
count=0
if [[ -f "$ANDROID_SDK_ROOT/count" ]]; then read -r count < "$ANDROID_SDK_ROOT/count"; fi
echo "$((count + 1))" > "$ANDROID_SDK_ROOT/count"
if [[ "$MODE" = fail || ( "$MODE" = retry && "$count" = 0 ) ]]; then exit 1; fi
if [[ "$MODE" = corrupt ]]; then
  # 第一次留下坏掉的压缩包、准备完成的暂存操作和没装完的 NDK 目录后失败；这些还在时重来，就像真实 sdkmanager 复用它们一样再次失败。
  if [[ "$count" = 0 ]]; then
    mkdir -p "$ANDROID_SDK_ROOT/.downloadIntermediates" "$ANDROID_SDK_ROOT/.temp/PackageOperation01" "$ANDROID_SDK_ROOT/ndk/28.2.13676358"
    echo partial > "$ANDROID_SDK_ROOT/.downloadIntermediates/ndk.zip"
    touch "$ANDROID_SDK_ROOT/.temp/PackageOperation01/.prepareComplete"
    echo partial > "$ANDROID_SDK_ROOT/ndk/28.2.13676358/package.xml"
    exit 1
  fi
  if [[ -e "$ANDROID_SDK_ROOT/.downloadIntermediates" || -e "$ANDROID_SDK_ROOT/.temp" || -e "$ANDROID_SDK_ROOT/ndk/28.2.13676358" ]]; then exit 1; fi
fi
ndk="$ANDROID_SDK_ROOT/ndk/28.2.13676358"
mkdir -p "$ndk/toolchains/llvm/prebuilt/$NDK_HOST/bin"
echo 'Pkg.Revision = 28.2.13676358' > "$ndk/source.properties"
if [[ "$MODE" != incomplete ]]; then
  touch "$ndk/toolchains/llvm/prebuilt/$NDK_HOST/bin/clang"
  chmod +x "$ndk/toolchains/llvm/prebuilt/$NDK_HOST/bin/clang"
fi
''')
            manager.chmod(0o755)
            host = "darwin-x86_64" if platform.system() == "Darwin" else "linux-x86_64"
            if installed:
                ndk = sdk / "ndk/28.2.13676358"
                compiler = ndk / f"toolchains/llvm/prebuilt/{host}/bin/clang"
                compiler.parent.mkdir(parents=True)
                compiler.touch(mode=0o755)
                (ndk / "source.properties").write_text("Pkg.Revision = 28.2.13676358\n")
            # 只替代等待，保留真实 bash 管道及 yes 的 SIGPIPE 行为。
            (sdk / "sleep").write_text("#!/usr/bin/env bash\nexit 0\n")
            (sdk / "sleep").chmod(0o755)
            result = subprocess.run(
                ["bash", str(ROOT / "platforms/android/install-ndk.sh")],
                env={**os.environ, "ANDROID_SDK_ROOT": str(sdk), "MODE": mode,
                     "NDK_HOST": host, "PATH": str(sdk) + os.pathsep + os.environ["PATH"]},
                capture_output=True, text=True, timeout=15,
            )
            count = int((sdk / "count").read_text()) if (sdk / "count").exists() else 0
            return result.returncode, count

    def test_download_failure_then_success(self):
        self.assertEqual(self.run_installer("retry"), (0, 2))

    def test_partial_download_is_cleared_before_retry(self):
        self.assertEqual(self.run_installer("corrupt"), (0, 2))

    def test_persistent_failure_stops_after_three_attempts(self):
        self.assertEqual(self.run_installer("fail"), (1, 3))

    def test_missing_compiler_is_not_success(self):
        self.assertEqual(self.run_installer("incomplete"), (1, 3))

    def test_existing_complete_ndk_is_reused(self):
        self.assertEqual(self.run_installer("fail", installed=True), (0, 0))


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""用合成 rustup 验证 scripts/rustup-install.sh 的重试次数、等待和参数传递，不访问网络、不碰本机工具链。"""
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/rustup-install.sh"

# 合成 rustup：按子命令计数，前 FAIL_<子命令> 次调用失败（-1 表示一直失败），每次调用把工作目录和参数记进日志。
FAKE_RUSTUP = '''#!/usr/bin/env bash
set -eu
key="$1"
count_file="$FAKE_DIR/count-$key"
count=0
if [[ -f "$count_file" ]]; then read -r count < "$count_file"; fi
echo "$((count + 1))" > "$count_file"
echo "$PWD|$*" >> "$FAKE_DIR/calls"
var="FAIL_$key"
fail=${!var:-0}
if [[ "$fail" = -1 || "$count" -lt "$fail" ]]; then
  echo "error: could not download file: dns error" >&2
  exit 1
fi
'''


@unittest.skipUnless(platform.system() in ("Darwin", "Linux") and shutil.which("bash"), "需要 bash")
class RustupInstallTests(unittest.TestCase):
    def run_script(self, *targets, fail_toolchain=0, fail_target=0):
        with tempfile.TemporaryDirectory(prefix="msime-rustup-test-") as directory:
            fake = Path(directory)
            (fake / "rustup").write_text(FAKE_RUSTUP)
            (fake / "rustup").chmod(0o755)
            # 只替代等待，记下每次等多久。
            (fake / "sleep").write_text('#!/usr/bin/env bash\necho "$1" >> "$FAKE_DIR/sleeps"\n')
            (fake / "sleep").chmod(0o755)
            env = dict(os.environ, PATH=f"{fake}{os.pathsep}{os.environ['PATH']}", FAKE_DIR=str(fake),
                       FAIL_toolchain=str(fail_toolchain), FAIL_target=str(fail_target))
            # 从仓库外的目录调用，确认脚本自己回到仓库根目录读 rust-toolchain.toml。
            result = subprocess.run(["bash", str(SCRIPT), *targets], cwd=directory, env=env,
                                    capture_output=True, text=True, timeout=60)
            calls = (fake / "calls").read_text().splitlines() if (fake / "calls").exists() else []
            sleeps = (fake / "sleeps").read_text().split() if (fake / "sleeps").exists() else []
            return result, calls, sleeps

    def test_installs_toolchain_and_targets_once_when_nothing_fails(self):
        result, calls, sleeps = self.run_script("wasm32-unknown-unknown")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [f"{ROOT}|toolchain install", f"{ROOT}|target add wasm32-unknown-unknown"])
        self.assertEqual(sleeps, [])

    def test_without_targets_only_installs_the_toolchain(self):
        result, calls, sleeps = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [f"{ROOT}|toolchain install"])
        self.assertEqual(sleeps, [])

    def test_retries_a_transient_toolchain_failure(self):
        result, calls, sleeps = self.run_script("aarch64-apple-darwin", "x86_64-apple-darwin", fail_toolchain=2)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call.split("|")[1] for call in calls],
                         ["toolchain install"] * 3 + ["target add aarch64-apple-darwin x86_64-apple-darwin"])
        self.assertEqual(sleeps, ["5", "15"])
        self.assertIn("attempt 2/4", result.stderr)

    def test_retries_a_transient_target_failure(self):
        result, calls, sleeps = self.run_script("x86_64-linux-android", fail_target=1)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call.split("|")[1] for call in calls],
                         ["toolchain install", "target add x86_64-linux-android"] * 2)
        self.assertEqual(sleeps, ["5"])

    def test_gives_up_after_four_attempts(self):
        result, calls, sleeps = self.run_script("wasm32-unknown-unknown", fail_toolchain=-1)
        self.assertEqual(result.returncode, 1)
        self.assertEqual([call.split("|")[1] for call in calls], ["toolchain install"] * 4)
        self.assertEqual(sleeps, ["5", "15", "45"])
        self.assertIn("after 4 attempts", result.stderr)


if __name__ == "__main__":
    unittest.main()

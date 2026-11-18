#!/usr/bin/env python3
"""用合成的 gradlew 和 sleep 验证 Gradle classpath 预取只在下载出错时重试，不访问网络或本机 SDK。"""
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "platforms/android/resolve-gradle-classpath.sh"

# 错误文本取自真实日志：依赖解析的 429 来自 Release Android 0.8.3（run 44252182195），wrapper 的 503 是 Java URLConnection 的写法。
MAVEN_429 = ("Could not GET 'https://repo.maven.apache.org/maven2/org/ow2/asm/asm/9.8/asm-9.8.pom'. "
             "Received status code 429 from server: Too Many Requests")
WRAPPER_503 = ("Exception in thread \"main\" java.io.IOException: Server returned HTTP response code: 503 "
               "for URL: https://services.gradle.org/distributions/gradle-9.5.1-bin.zip")
MAVEN_403 = ("Could not GET 'https://repo.maven.apache.org/maven2/org/ow2/asm/asm/9.8/asm-9.8.pom'. "
             "Received status code 403 from server: Forbidden")
MISSING = "Could not find com.example:missing:1.0."


@unittest.skipUnless(platform.system() in ("Darwin", "Linux"), "仅支持 macOS/Linux 的 bash")
class ResolveGradleClasspathTests(unittest.TestCase):
    def run_script(self, *outcomes):
        """outcomes 依次是每次调用 gradlew 时的（退出码, 输出）；用完后一直重复最后一个。"""
        with tempfile.TemporaryDirectory(prefix="msime-gradle-prefetch-test-") as directory:
            work = Path(directory)
            project = work / "gradle-app"
            project.mkdir()
            for index, (code, text) in enumerate(outcomes):
                (work / f"outcome-{index}").write_text(f"{code}\n{text}\n")
            gradlew = project / "gradlew"
            gradlew.write_text('''#!/usr/bin/env bash
set -eu
echo "$ANDROID_HOME $*" >> "$WORK/calls"
count=$(wc -l < "$WORK/calls")
index=$((count - 1))
last=$(ls "$WORK" | grep -c '^outcome-')
if ((index >= last)); then index=$((last - 1)); fi
{ read -r code; text=$(cat); } < "$WORK/outcome-$index"
echo "$text"
exit "$code"
''')
            gradlew.chmod(0o755)
            (work / "sleep").write_text('#!/usr/bin/env bash\necho "$1" >> "$WORK/sleeps"\n')
            (work / "sleep").chmod(0o755)
            result = subprocess.run(
                ["bash", str(SCRIPT)],
                env={**os.environ, "WORK": str(work), "MSIME_GRADLE_PROJECT_DIR": str(project),
                     "ANDROID_SDK_ROOT": "/synthetic/sdk",
                     "PATH": str(work) + os.pathsep + os.environ["PATH"]},
                capture_output=True, text=True, timeout=30,
            )
            calls = (work / "calls").read_text().replace(str(project), "<project>").splitlines() if (work / "calls").exists() else []
            sleeps = (work / "sleeps").read_text().split() if (work / "sleeps").exists() else []
            return result, calls, sleeps

    def test_success_needs_one_call(self):
        result, calls, sleeps = self.run_script((0, "BUILD SUCCESSFUL"))
        self.assertEqual(result.returncode, 0)
        self.assertEqual(calls, ["/synthetic/sdk --project-dir <project> --console=plain help"])
        self.assertEqual(sleeps, [])

    def test_rate_limit_then_success_retries_after_a_minute(self):
        result, calls, sleeps = self.run_script((1, MAVEN_429), (0, "BUILD SUCCESSFUL"))
        self.assertEqual((result.returncode, len(calls), sleeps), (0, 2, ["60"]))
        self.assertIn("Received status code 429", result.stderr)

    def test_wrapper_download_error_is_retried(self):
        result, calls, sleeps = self.run_script((1, WRAPPER_503), (0, "BUILD SUCCESSFUL"))
        self.assertEqual((result.returncode, len(calls), sleeps), (0, 2, ["60"]))

    def test_persistent_rate_limit_stops_after_three_attempts(self):
        result, calls, sleeps = self.run_script((1, MAVEN_429))
        self.assertEqual((result.returncode, len(calls), sleeps), (1, 3, ["60", "120"]))
        self.assertIn("after 3 attempts", result.stderr)

    def test_forbidden_is_not_retried(self):
        result, calls, sleeps = self.run_script((1, MAVEN_403), (0, "BUILD SUCCESSFUL"))
        self.assertEqual((result.returncode, len(calls), sleeps), (1, 1, []))

    def test_other_failure_keeps_its_status_and_is_not_retried(self):
        result, calls, sleeps = self.run_script((3, MISSING), (0, "BUILD SUCCESSFUL"))
        self.assertEqual((result.returncode, len(calls), sleeps), (3, 1, []))
        self.assertIn("not retrying", result.stderr)


if __name__ == "__main__":
    unittest.main()

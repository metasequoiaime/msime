#!/usr/bin/env python3
"""检查 Android 发布链（含 nightly 与 publish=false 的构建）一律用正式发布密钥签名，且密钥不经 workflow artifact 或缓存在 job 之间传递。"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release-android.yml"
NIGHTLY = ROOT / ".github/workflows/nightly.yml"
EDITION_ANDROID = ROOT / "platforms/android/scripts/edition_android.py"
BUILD_STEP = "      - name: Build native Android host\n"
SECRET_ENV = {
    "RELEASE_KEYSTORE_BASE64": "${{ secrets.ANDROID_RELEASE_KEYSTORE_BASE64 }}",
    "MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD": "${{ secrets.ANDROID_RELEASE_KEYSTORE_PASSWORD }}",
}
MISSING_SECRET_CHECK = '[[ -n "$RELEASE_KEYSTORE_BASE64" && -n "$MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD" ]] || {'
# 密钥文件、解出目录或 secret 名出现在 artifact / 缓存步骤的参数里，就说明密钥被当成中间产物在 job 之间传递了。
KEY_MATERIAL = re.compile(r"keystore|signing|\.p12\b|\.jks\b|secrets\.", re.IGNORECASE)
TRANSFER_ACTIONS = re.compile(r"uses: actions/(upload-artifact|download-artifact|cache)(/save|/restore)?@")


def fail(message: str) -> int:
    print(f"FAIL: {message}", file=sys.stderr)
    return 1


def jobs(text: str) -> dict[str, str]:
    """按两格缩进的 job 名切分 jobs: 段。"""
    body = text[text.index("\njobs:\n") + len("\njobs:\n"):]
    starts = [(m.start(), m.group(1)) for m in re.finditer(r"^  ([A-Za-z0-9_-]+):\n", body, re.MULTILINE)]
    return {name: body[start:(starts[i + 1][0] if i + 1 < len(starts) else len(body))] for i, (start, name) in enumerate(starts)}


def steps(job: str) -> list[str]:
    """按六格缩进的 `- ` 切分一个 job 的步骤。"""
    starts = [m.start() for m in re.finditer(r"^      - ", job, re.MULTILINE)]
    return [job[start:(starts[i + 1] if i + 1 < len(starts) else len(job))] for i, start in enumerate(starts)]


def main() -> int:
    if not WORKFLOW.exists():
        return 0
    text = WORKFLOW.read_text(encoding="utf-8")

    if "ANDROID_NIGHTLY_KEYSTORE" in text:
        return fail("nightly must not require a dedicated ANDROID_NIGHTLY_KEYSTORE_* secret; it signs with the release key")
    all_jobs = jobs(text)
    if "development-signing-key" in text:
        return fail("the development-signing-key job must be gone; each build job decodes the key from the secret itself")

    # 任何 job 都不得把密钥材料上传、下载或缓存成 artifact。
    for name, job in all_jobs.items():
        for step in steps(job):
            if TRANSFER_ACTIONS.search(step) and KEY_MATERIAL.search(step):
                return fail(f"job {name} moves key material through an artifact or cache step:\n{step}")

    release = all_jobs.get("release")
    if release is None:
        return fail("release job is missing")
    # 矩阵要与版本表里有 Android 段的版本一一对应（publish 也读同一张表核对 APK），不写死个数，版本表增删版本时这里跟着走。
    editions = re.search(r"^        edition: \[([^\]]*)\]$", release, re.MULTILINE)
    expected = subprocess.run(
        [sys.executable, str(EDITION_ANDROID), "editions"], check=True, capture_output=True, text=True
    ).stdout.strip().split(",")
    if editions is None:
        return fail("release job has no edition matrix")
    matrix = [e.strip() for e in editions.group(1).split(",") if e.strip()]
    if sorted(matrix) != sorted(expected) or len(matrix) != len(set(matrix)):
        return fail(f"release matrix {matrix} must build exactly the Android editions in shared/contracts/editions.json: {expected}")

    build = next((step for step in steps(release) if step.startswith(BUILD_STEP)), None)
    if build is None:
        return fail("release job has no 'Build native Android host' step")
    if re.search(r"^        if:", build, re.MULTILINE):
        return fail("the signing build step must run for every edition and every publish value")
    for name, value in SECRET_ENV.items():
        if f'          {name}: "{value}"' not in build:
            return fail(f"build step must read {name} from {value}")

    script = build[build.index("        run: |\n"):]
    if 'if [[ "$PUBLISH" == true ]]' in script:
        return fail("release-key decoding must not depend on publish; nightly and publish=false builds sign with the same key")
    if script.count(MISSING_SECRET_CHECK) != 1 or text.count(MISSING_SECRET_CHECK) != 1:
        return fail("a missing release secret must fail explicitly, in exactly one place")
    decode = 'printf \'%s\' "$RELEASE_KEYSTORE_BASE64" | base64 --decode > "$MSIME_ANDROID_RELEASE_KEYSTORE"'
    # 资源先用 cargo 生成，再解出密钥：密钥文件只在 build-apk.sh 运行期间存在，install_resources 那一轮完整 cargo 构建（含第三方 build.rs）读不到它。
    ordered = [
        'resource_dir="$(cargo run --quiet -p msime-client-core --example install_resources -- target/resources)"',
        MISSING_SECRET_CHECK,
        'export MSIME_ANDROID_RELEASE_KEYSTORE="$RUNNER_TEMP/msime-release.p12"',
        "trap 'rm -f \"$MSIME_ANDROID_RELEASE_KEYSTORE\"' EXIT",
        decode,
        'bash platforms/android/build-apk.sh "$resource_dir"',
    ]
    positions = [script.find(fragment) for fragment in ordered]
    if -1 in positions:
        return fail(f"build step is missing: {ordered[positions.index(-1)]}")
    if positions != sorted(positions):
        return fail("build step must generate resources first, then check the secret, decode it into $RUNNER_TEMP with a cleanup trap, then build")

    # 密钥只在这一步出现，而且不打印、不开 shell 跟踪。
    if text.count("secrets.ANDROID_RELEASE_KEYSTORE_BASE64") != 1:
        return fail("ANDROID_RELEASE_KEYSTORE_BASE64 must be read only by the signing build step")
    if re.search(r"set -[a-z]*x|echo[^\n]*\$\{?(RELEASE_KEYSTORE_BASE64|MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD)", script):
        return fail("build step must not trace or echo the keystore or its password")

    if NIGHTLY.exists():
        nightly = NIGHTLY.read_text(encoding="utf-8")
        call = nightly[nightly.index("uses: ./.github/workflows/release-android.yml"):]
        call = call[:call.index("\n\n")] if "\n\n" in call else call
        if "publish: false" not in call or "secrets: inherit" not in call:
            return fail("nightly must call release-android.yml with publish: false and inherit the repository secrets")

    print("Android release-key signing workflow contract passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

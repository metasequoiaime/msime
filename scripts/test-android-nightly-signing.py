#!/usr/bin/env python3
"""检查 Android nightly 的矩阵 runner 是否共享同一份临时开发签名。"""

from __future__ import annotations

import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release-android.yml"


def fail(message: str) -> int:
    print(f"FAIL: {message}", file=sys.stderr)
    return 1


def main() -> int:
    if not WORKFLOW.exists():
        return 0
    text = WORKFLOW.read_text(encoding="utf-8")

    required = {
        "shared key job": "  development-signing-key:\n    needs: source",
        "nightly key preparation guard": "        if: ${{ !inputs.publish }}\n        env:\n          NIGHTLY_KEYSTORE_BASE64: \"${{ secrets.ANDROID_NIGHTLY_KEYSTORE_BASE64 }}\"\n          NIGHTLY_KEYSTORE_PASSWORD: \"${{ secrets.ANDROID_NIGHTLY_KEYSTORE_PASSWORD }}\"\n        run: |\n          set -euo pipefail\n          [[ -n \"$NIGHTLY_KEYSTORE_BASE64\" && -n \"$NIGHTLY_KEYSTORE_PASSWORD\" ]]",
        "stable key alias": "-alias msime-release",
        "shared key upload": "          name: msime-android-nightly-signing-${{ github.run_id }}-${{ github.run_attempt }}",
        "short artifact retention": "          retention-days: 1",
        "release depends on shared key": "    needs: [source, development-signing-key]",
        "nightly key download": "          name: msime-android-nightly-signing-${{ github.run_id }}-${{ github.run_attempt }}\n          path: ${{ runner.temp }}/android-nightly-signing",
        "nightly keystore path": '            export MSIME_ANDROID_RELEASE_KEYSTORE="$RUNNER_TEMP/android-nightly-signing/msime-nightly.p12"',
        "nightly keystore password": '            export MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD="$NIGHTLY_KEYSTORE_PASSWORD"',
        "missing key is fatal": '            [[ -f "$MSIME_ANDROID_RELEASE_KEYSTORE" ]] || { echo "nightly shared development keystore is missing" >&2; exit 1; }',
    }
    for label, fragment in required.items():
        if fragment not in text:
            return fail(f"{label} contract is missing")

    generation = text.index("  development-signing-key:")
    release = text.index("  release:")
    download = text.index("      - name: 下载 nightly 共享开发密钥")
    build = text.index("      - name: Build native Android host")
    if not generation < release:
        return fail("shared key job must be declared before release")
    if not download < build:
        return fail("nightly key must be downloaded before the Android build")

    build_block = text[build : text.index("      - uses: actions/upload-artifact", build)]
    if not re.search(r'if \[\[ "\$PUBLISH" == true \]\]; then', build_block):
        return fail("formal signing branch is missing")
    if 'else\n            export MSIME_ANDROID_RELEASE_KEYSTORE=' not in build_block:
        return fail("nightly signing branch is missing")

    print("Android nightly shared-signing workflow contract passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

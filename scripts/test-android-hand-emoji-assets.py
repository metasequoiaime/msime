#!/usr/bin/env python3
"""校验候选手势资源的完整集合、尺寸与固定来源哈希，不依赖 Android 工具链。"""
import hashlib
import json
from pathlib import Path
import struct

root = Path(__file__).resolve().parent.parent
notices = root / "platforms/android/third_party/noto-emoji"
manifest = json.loads((notices / "sources.json").read_text())
expected = {
    f"emoji_u{code}{tone}"
    for code in ("1faf1", "1faf2", "1faf7", "1faf8")
    for tone in ("", "_1f3fb", "_1f3fc", "_1f3fd", "_1f3fe", "_1f3ff")
}
assert manifest["commit"] == "e20cbc2bbec1926686be9f9bee7d1d2cfa1fea0e"
assert manifest["source_license"] == "2D/svg/LICENSE"
assert len(manifest["assets"]) == 24
assert {Path(row["source"]).stem for row in manifest["assets"]} == expected
for row in manifest["assets"]:
    name = Path(row["source"]).stem
    assert row["source"] == f"2D/svg/{name}.svg"
    assert row["output"] == f"platforms/android/res/drawable-nodpi/noto_{name}.png"
    assert len(bytes.fromhex(row["source_sha256"])) == 32
    data = (root / row["output"]).read_bytes()
    assert hashlib.sha256(data).hexdigest() == row["output_sha256"]
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    assert struct.unpack(">II", data[16:24]) == (128, 128)
for name in ("LICENSE.txt", "UPSTREAM-LICENSE.txt", "NOTICE.txt"):
    assert (notices / name).stat().st_size > 100
for name in ("build-apk.sh", "build-client-apk.sh"):
    script = (root / "platforms/android" / name).read_text()
    assert '"$assets/native-notices/noto-emoji/"' in script
print("android hand emoji: 24 fixed-source images, hashes, 128px dimensions and both notice paths verified")

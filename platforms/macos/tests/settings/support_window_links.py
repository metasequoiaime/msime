"""Keep the native fallback support window on client-owned destinations."""

import sys
from pathlib import Path


source = Path(sys.argv[1]).read_text(encoding="utf-8")
expected = {
    "issues": "https://github.com/metasequoiaime/msime/issues",
    "license": "https://github.com/metasequoiaime/msime/blob/main/LICENSE",
    "privacy": "https://msime.app/privacy/",
}

for name, url in expected.items():
    assert source.count(f'@"{url}"') == 1, f"native support {name} URL is not pinned"

assert "MSIME-Windows" not in source, "native macOS support must not open Windows project pages"

print("native macOS support links use the shared client destinations")

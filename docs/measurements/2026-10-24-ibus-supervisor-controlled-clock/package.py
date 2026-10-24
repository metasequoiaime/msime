"""封存本片的测试与门禁记录，保留失败，清除本机路径。"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--resources", type=Path, required=True)
parser.add_argument("--main-checkout", type=Path, required=True)
args = parser.parse_args()
root = Path.cwd()
source = root / "target/page-compaction-evidence"
destination = Path(__file__).resolve().parent
replacements = (
    (str(root), "<WORKTREE>"),
    (str(args.resources.resolve()), "<RESOURCES>"),
    (str(args.main_checkout.resolve()), "<MAIN_CHECKOUT>"),
    (str(Path.home()), "<HOME>"),
)
names = (
    "push-attempt-1.log", "push-attempt-1-linux-native.log", "theme-hint-rerun.log",
    "push-attempt-2.log", "push-attempt-2-linux-native.log",
    "launcher-jitter-red.log", "launcher-jitter-green.log", "launcher-mutations.log",
    "quick-after-launcher.log", "quick-after-launcher-status.json",
    "quick-after-launcher-linux-native.log",
)
for name in names:
    content = (source / name).read_text()
    for before, after in replacements:
        content = content.replace(before, after)
    (destination / f"{name}.gz").write_bytes(gzip.compress(content.encode(), mtime=0))
manifest = {"engine_commit": "13ceb2186b9d5478d1857bf5fc805ecfb9683a0b", "files": {}}
for path in sorted(destination.iterdir()):
    if path.is_file() and path.name != "manifest.json":
        payload = path.read_bytes()
        manifest["files"][path.name] = {
            "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest(),
        }
(destination / "manifest.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
)
print("packaged", len(manifest["files"]), "files")

#!/usr/bin/env python3
"""打引擎的最小全拼、双拼数据包 msime-engine-data-quanpin-<词库版本>，`.github/workflows/release-engine-capi.yml` 调用它。

    python3 crates/engine-capi/packaging/data-pack.py <输出目录>

数据包是 `docs/data-formats.md` §20「全拼、双拼、九键」一行的必需加产品同等文件：`msime-pinyin.db`、`msime-english.db`、`msime-bigram.bin`、`msime-trigram.bin`，加上分发 `msime-english.db` 时必须同带的 `msime-scowl_Copyright.txt`。五个文件都按 `resources/desktop-dictionary.lock.json` 下载，逐个核对大小和 SHA-256，必须来自同一个 msime-dictionary `dict-v*` 发布，包名里的版本号就是它的。

包里的顶层目录直接就是引擎的 `resources` 目录：五个数据文件、说明数据许可证的 `README.md`（模板是同目录的 `data-pack-README.md`）和 `licenses/`（GPL-3.0 全文、行政区划名称的 WTFPL、词库的来源与授权说明）。输出 `.tar.gz` 和给 Windows 用的 `.zip`，内容相同。

下载复用 `scripts/download_retry.py` 的重试；已经下载过、摘要对得上的文件不再下载（缓存在 `target/engine-capi/data-download/`）。
"""
import hashlib
import json
import re
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import download_retry  # noqa: E402

LOCK = ROOT / "resources/desktop-dictionary.lock.json"
TEMPLATE = Path(__file__).resolve().parent / "data-pack-README.md"
CACHE = ROOT / "target/engine-capi/data-download"

# docs/data-formats.md §20：全拼、双拼的必需文件、产品同等再加的 n-gram 表，以及随 msime-english.db 必须携带的 SCOWL 声明。
FILES = ("msime-pinyin.db", "msime-english.db", "msime-scowl_Copyright.txt", "msime-bigram.bin", "msime-trigram.bin")

# 数据许可证要求随附的全文与声明（README 模板逐个说明）：msime-pinyin.db、msime-english.db 的 GPL-3.0 全文是仓库根目录的 LICENSE；行政区划名称是 WTFPL；msime-pinyin.db 的单字读音按 pinyin-data 校正、msime-english.db 的中英释义取自 ECDICT，两者都是 MIT，MIT 要求随副本附上版权与许可声明全文；词库每个来源的上游与条款在 msime-dictionary 的 NOTICE 里，这份是它在本仓库的副本。
LICENCES = {
    "GPL-3.0.txt": ROOT / "LICENSE",
    "Administrative-divisions-of-China-WTFPL.txt": ROOT / "resources/licenses/Administrative-divisions-of-China-WTFPL.txt",
    "pinyin-data-MIT.txt": ROOT / "resources/licenses/pinyin-data-MIT.txt",
    "ECDICT-MIT.txt": ROOT / "resources/licenses/ECDICT-MIT.txt",
    "msime-engine-dictionary-NOTICE.md": ROOT / "resources/licenses/msime-engine-dictionary-NOTICE.md",
}

RELEASE_URL = re.compile(r"^https://github\.com/metasequoiaime/msime-dictionary/releases/download/dict-v(\d+\.\d+\.\d+)/([^/]+)$")


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            sha.update(chunk)
    return sha.hexdigest()


def fetch(artifact: dict) -> Path:
    """下载一个锁定的文件到缓存，核对大小和摘要；缓存里已有且摘要对得上就直接用。"""
    CACHE.mkdir(parents=True, exist_ok=True)
    target = CACHE / artifact["name"]
    if target.is_file() and target.stat().st_size == artifact["size"] and digest(target) == artifact["sha256"]:
        print(f"{artifact['name']}: cached", file=sys.stderr)
        return target
    print(f"{artifact['name']}: downloading {artifact['url']}", file=sys.stderr)
    with tempfile.NamedTemporaryFile(dir=CACHE, delete=False) as handle:
        temporary = Path(handle.name)
    try:
        size = 0
        with download_retry.urlopen(artifact["url"], timeout=300) as response, temporary.open("wb") as out:
            while chunk := response.read(1024 * 1024):
                size += len(chunk)
                # 比锁里记的大就停下，不先把整个响应写完。
                if size > artifact["size"]:
                    raise SystemExit(f"{artifact['name']} is larger than the {artifact['size']} bytes the lock records")
                out.write(chunk)
        if size != artifact["size"]:
            raise SystemExit(f"{artifact['name']} is {size} bytes; the lock records {artifact['size']}")
        found = digest(temporary)
        if found != artifact["sha256"]:
            raise SystemExit(f"{artifact['name']} has SHA-256 {found}; the lock records {artifact['sha256']}")
        temporary.replace(target)
        return target
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <output directory>", file=sys.stderr)
        return 2
    out = Path(sys.argv[1]).resolve()
    artifacts = {artifact["name"]: artifact for artifact in json.loads(LOCK.read_text(encoding="utf-8"))["artifacts"]}

    versions = set()
    for name in FILES:
        artifact = artifacts.get(name)
        if artifact is None:
            raise SystemExit(f"{LOCK.relative_to(ROOT)} does not pin {name}")
        match = RELEASE_URL.match(artifact["url"])
        if match is None or match.group(2) != name:
            raise SystemExit(f"{name} is not pinned to a msime-dictionary dict-v release asset of that name: {artifact['url']}")
        versions.add(match.group(1))
    if len(versions) != 1:
        raise SystemExit(f"the data pack files come from different dictionary releases: {sorted(versions)}")
    version = versions.pop()
    name = f"msime-engine-data-quanpin-{version}"

    staging = ROOT / "target/engine-capi/data-pack"
    pack = staging / name
    shutil.rmtree(staging, ignore_errors=True)
    (pack / "licenses").mkdir(parents=True)
    for file in FILES:
        shutil.copyfile(fetch(artifacts[file]), pack / file)
    for target, source in LICENCES.items():
        shutil.copyfile(source, pack / "licenses" / target)
    readme = TEMPLATE.read_text(encoding="utf-8").replace("{{version}}", version).replace("{{version_id}}", version.replace(".", "_"))
    for file in FILES:
        artifact = artifacts[file]
        readme = readme.replace(f"{{{{size:{file}}}}}", str(artifact["size"])).replace(f"{{{{sha256:{file}}}}}", artifact["sha256"])
    leftover = re.findall(r"\{\{[^}]*\}\}", readme)
    if leftover:
        raise SystemExit(f"{TEMPLATE.name} has placeholders this script does not fill: {leftover}")
    (pack / "README.md").write_text(readme, encoding="utf-8")

    out.mkdir(parents=True, exist_ok=True)
    # 归档里的条目按名字排序，属主一律记成 0，不把构建机的用户名带进去。
    entries = sorted(path for path in pack.rglob("*"))

    def anonymous(info: tarfile.TarInfo) -> tarfile.TarInfo:
        info.uid = info.gid = 0
        info.uname = info.gname = ""
        return info

    tar_path = out / f"{name}.tar.gz"
    with tarfile.open(tar_path, "w:gz") as archive:
        archive.add(pack, arcname=name, recursive=False, filter=anonymous)
        for path in entries:
            archive.add(path, arcname=f"{name}/{path.relative_to(pack).as_posix()}", recursive=False, filter=anonymous)
    zip_path = out / f"{name}.zip"
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in entries:
            archive.write(path, arcname=f"{name}/{path.relative_to(pack).as_posix()}")
    print(tar_path)
    print(zip_path)
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""验证 macOS 发布页瘦身的脚本：sparkle_feed.py 的验签与比较、publish-sparkle-feed.sh 的上传顺序与清理、relocate-release-extras.sh 只在全部核对通过后才删除版本发布页上的资产。

不访问 GitHub，不用发布私钥：`gh` 和 `curl` 是在临时目录里模拟 release 的替身，签名用每次新生成的一次性 Ed25519 密钥。`generate_appcast` 默认也是替身，按 Sparkle 2.9.6 的输出格式生成并签名；设置 `MSIME_SPARKLE_ROOT` 指向解压好的 Sparkle 2.9.6 时改用真的 `generate_appcast`。需要支持 Ed25519 的 OpenSSL 3（`MSIME_OPENSSL`、Homebrew 的 openssl@3 或 PATH 里的 openssl），没有时跳过。
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "platforms/macos/scripts"
FEED_TOOL = SCRIPTS / "sparkle_feed.py"
DOWNLOAD = "https://github.com/metasequoiaime/msime/releases/download/"
APPCAST_PREFIX = DOWNLOAD + "macos-appcast/"
SEED_PREFIX = bytes.fromhex("302e020100300506032b657004220420")
SPARKLE_ROOT = os.environ.get("MSIME_SPARKLE_ROOT", "")
REAL_SPARKLE = bool(SPARKLE_ROOT) and Path(SPARKLE_ROOT, "bin/generate_appcast").is_file()


def find_openssl() -> str | None:
    candidates = [os.environ.get("MSIME_OPENSSL", "")]
    if shutil.which("brew"):
        prefix = subprocess.run(["brew", "--prefix", "openssl@3"], capture_output=True, text=True).stdout.strip()
        candidates.append(str(Path(prefix) / "bin/openssl"))
    candidates.append(shutil.which("openssl") or "")
    with tempfile.TemporaryDirectory() as directory:
        key = Path(directory) / "key.der"
        key.write_bytes(SEED_PREFIX + bytes(32))
        for candidate in filter(None, candidates):
            probe = subprocess.run([candidate, "pkey", "-inform", "DER", "-in", str(key), "-pubout", "-outform", "DER"], capture_output=True)
            if probe.returncode == 0 and len(probe.stdout) == 44:
                return candidate
    return None


OPENSSL = find_openssl()

# 模拟 gh release 的替身：每个 release 是 state 下的一个目录，调用依次记进 calls.jsonl。
FAKE_GH = r'''
import fnmatch, hashlib, json, os, shutil, sys
from pathlib import Path
state = Path(os.environ["MSIME_TEST_RELEASES"])
with open(state / "calls.jsonl", "a", encoding="utf-8") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
args = sys.argv[1:]
assert args[0] == "release", args
command, tag, rest = args[1], args[2], args[3:]
release = state / "releases" / tag
def option(name):
    return [rest[i + 1] for i, value in enumerate(rest) if value == name]
if command == "view":
    if not release.is_dir():
        print(f"release not found: {tag}", file=sys.stderr); sys.exit(1)
    query = option("--jq")
    if not query:
        sys.exit(0)
    names = sorted(path.name for path in release.iterdir())
    if query[0] == ".assets[].name":
        print("\n".join(names))
    elif 'select(.name == "' in query[0]:
        name = query[0].split('select(.name == "', 1)[1].split('"', 1)[0]
        if name in names:
            print("sha256:" + hashlib.sha256((release / name).read_bytes()).hexdigest())
    else:
        sys.exit(f"unsupported --jq {query[0]}")
elif command == "create":
    release.mkdir(parents=True)
elif command == "upload":
    if not release.is_dir():
        sys.exit(1)
    for path in (value for value in rest if not value.startswith("--")):
        target = release / Path(path).name
        if target.exists() and "--clobber" not in rest:
            sys.exit(f"{target.name} exists")
        shutil.copyfile(path, target)
elif command == "download":
    patterns, directory = option("--pattern"), Path(option("--dir")[0])
    directory.mkdir(parents=True, exist_ok=True)
    matched = [path for path in sorted(release.iterdir()) if any(fnmatch.fnmatch(path.name, pattern) for pattern in patterns)] if release.is_dir() else []
    if not matched:
        print("no assets match the file pattern", file=sys.stderr); sys.exit(1)
    for path in matched:
        shutil.copyfile(path, directory / path.name)
elif command == "delete-asset":
    (release / rest[0]).unlink()
else:
    sys.exit(f"unsupported gh release {command}")
'''

# 模拟 curl：把 releases/download/<tag>/<name> 换成 state 里的文件；MSIME_TEST_CORRUPT 列出的文件名下载到的内容被改掉。
FAKE_CURL = r'''
import os, shutil, sys
from pathlib import Path
state = Path(os.environ["MSIME_TEST_RELEASES"])
args = sys.argv[1:]
output = Path(args[args.index("--output") + 1])
url = args[-1]
prefix = "https://github.com/metasequoiaime/msime/releases/download/"
if not url.startswith(prefix):
    sys.exit(6)
tag, name = url[len(prefix):].split("/", 1)
source = state / "releases" / tag / name
if not source.is_file():
    print(f"curl: (22) The requested URL returned error: 404", file=sys.stderr); sys.exit(22)
shutil.copyfile(source, output)
if name in os.environ.get("MSIME_TEST_CORRUPT", "").split(","):
    with open(output, "ab") as handle:
        handle.write(b"tampered")
'''

# 按 Sparkle 2.9.6 的 generate_appcast 的输出格式生成一份签名的 appcast（见 package-release.sh 产出的样子）。
FAKE_GENERATE_APPCAST = r'''
import base64, datetime, email.utils, os, plistlib, subprocess, sys, tempfile, zipfile
from pathlib import Path
args = sys.argv[1:]
prefix = args[args.index("--download-url-prefix") + 1]
output = Path(args[args.index("-o") + 1])
archives = [path for path in Path(args[-1]).iterdir() if path.suffix == ".zip"]
assert len(archives) == 1, archives
archive = archives[0]
seed = base64.b64decode(sys.stdin.read().strip())
with zipfile.ZipFile(archive) as bundle:
    info = plistlib.loads(next(bundle.read(name) for name in bundle.namelist() if name.count("/") == 2 and name.endswith("/Contents/Info.plist")))
def sign(data):
    with tempfile.TemporaryDirectory() as directory:
        folder = Path(directory)
        (folder / "key.der").write_bytes(bytes.fromhex("302e020100300506032b657004220420") + seed)
        (folder / "data").write_bytes(data)
        subprocess.run([os.environ["MSIME_OPENSSL"], "pkeyutl", "-sign", "-keyform", "DER", "-inkey", str(folder / "key.der"), "-rawin", "-in", str(folder / "data"), "-out", str(folder / "signature")], check=True)
        return base64.b64encode((folder / "signature").read_bytes()).decode()
published = email.utils.format_datetime(datetime.datetime.fromtimestamp(int(archive.stat().st_mtime), datetime.timezone.utc))
body = (
    '<?xml version="1.0" standalone="yes"?><!-- sparkle-sign-warning:\nIMPORTANT: synthetic\n-->'
    '<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0">\n'
    f'    <channel>\n        <title>{info["CFBundleName"]}</title>\n        <item>\n'
    f'            <title>{info["CFBundleShortVersionString"]}</title>\n'
    f'            <pubDate>{published}</pubDate>\n'
    f'            <sparkle:version>{info["CFBundleVersion"]}</sparkle:version>\n'
    f'            <sparkle:shortVersionString>{info["CFBundleShortVersionString"]}</sparkle:shortVersionString>\n'
    f'            <sparkle:minimumSystemVersion>{info["LSMinimumSystemVersion"]}</sparkle:minimumSystemVersion>\n'
    f'            <enclosure url="{prefix}{archive.name}" length="{archive.stat().st_size}" type="application/octet-stream" sparkle:edSignature="{sign(archive.read_bytes())}"></enclosure>\n'
    '        </item>\n    </channel>\n</rss>'
).encode()
output.write_bytes(body + f"<!-- sparkle-signatures:\nedSignature: {sign(body)}\nlength: {len(body)}\n-->\n".encode())
'''


@unittest.skipUnless(os.name == "posix" and shutil.which("bash") and shutil.which("shasum") and OPENSSL, "需要 POSIX bash、shasum 和支持 Ed25519 的 OpenSSL 3")
class ReleaseExtrasTests(unittest.TestCase):
    editions = ("full", "wubi")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="msime-release-extras-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.state = self.directory / "state"
        (self.state / "releases").mkdir(parents=True)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        for name, source in (("gh", FAKE_GH), ("curl", FAKE_CURL), ("generate_appcast", FAKE_GENERATE_APPCAST)):
            script = self.directory / f"fake_{name}.py"
            script.write_text(source, encoding="utf-8")
            tool = self.bin / name
            tool.write_text(f"#!/usr/bin/env bash\nexec {shlex.quote(sys.executable)} -I {shlex.quote(str(script))} \"$@\"\n", encoding="utf-8")
            tool.chmod(0o755)
        sparkle = self.directory / "sparkle"
        (sparkle / "bin").mkdir(parents=True)
        if REAL_SPARKLE:
            self.sparkle_root = Path(SPARKLE_ROOT)
        else:
            (sparkle / "bin/generate_appcast").symlink_to(self.bin / "generate_appcast")
            self.sparkle_root = sparkle
        seed = os.urandom(32)
        self.private_key = base64.b64encode(seed).decode()
        public = subprocess.run([OPENSSL, "pkey", "-inform", "DER", "-pubout", "-outform", "DER"], input=SEED_PREFIX + seed, capture_output=True, check=True).stdout[-32:]
        self.public_key = base64.b64encode(public).decode()
        self.public_key_file = self.directory / "sparkle-public-key.txt"
        self.public_key_file.write_text(self.public_key + "\n", encoding="utf-8")
        self.env = {
            **os.environ,
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "MSIME_TEST_RELEASES": str(self.state),
            "MSIME_OPENSSL": OPENSSL,
            "MSIME_SPARKLE_PUBLIC_KEY_FILE": str(self.public_key_file),
            "MSIME_SPARKLE_ROOT": str(self.sparkle_root),
            "MSIME_SPARKLE_PRIVATE_KEY": self.private_key,
            "RUNNER_TEMP": str(self.directory),
            "TMPDIR": str(self.directory),
        }
        self.env.pop("MSIME_TEST_CORRUPT", None)

    # ---- 构造发布状态 ----

    def release(self, tag: str) -> Path:
        path = self.state / "releases" / tag
        path.mkdir(parents=True, exist_ok=True)
        return path

    def assets(self, tag: str) -> list[str]:
        path = self.state / "releases" / tag
        return sorted(item.name for item in path.iterdir()) if path.is_dir() else []

    def calls(self) -> list[list[str]]:
        log = self.state / "calls.jsonl"
        return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()] if log.exists() else []

    def mutating_calls(self) -> list[list[str]]:
        return [call for call in self.calls() if call[1] in ("upload", "delete-asset", "create")]

    def make_archive(self, path: Path, name: str, version: str, build: int) -> None:
        info = {
            "CFBundleName": name, "CFBundleIdentifier": f"app.example.{path.stem}", "CFBundleExecutable": "Synthetic",
            "CFBundleVersion": str(build), "CFBundleShortVersionString": version, "LSMinimumSystemVersion": "13.0",
            "SUPublicEDKey": self.public_key, "SURequireSignedFeed": True,
        }
        with zipfile.ZipFile(path, "w") as bundle:
            bundle.writestr(f"{name}.app/Contents/Info.plist", plistlib.dumps(info))
            bundle.writestr(f"{name}.app/Contents/MacOS/Synthetic", f"#!/bin/sh\n# {path.name}\n")
        os.utime(path, (1792427849, 1792427849))

    def generate(self, archive: Path, prefix: str, output: Path) -> None:
        source = self.directory / "generate" / output.name
        shutil.rmtree(source, ignore_errors=True)
        source.mkdir(parents=True)
        shutil.copy2(archive, source / archive.name)
        subprocess.run([str(self.sparkle_root / "bin/generate_appcast"), "--ed-key-file", "-", "--download-url-prefix", prefix, "-o", str(output), str(source)],
                       input=self.private_key, text=True, env={**self.env, "TZ": "UTC"}, check=True, capture_output=True)

    def archives_for(self, version: str) -> dict[str, str]:
        """appcast 名到更新包名。"""
        mapping = {}
        for edition in self.editions:
            prefix = "msime-macos" if edition == "full" else f"msime-macos-{edition}"
            mapping[f"appcast-{edition}.xml"] = f"{prefix}-{version}-input-method.zip"
            mapping[f"appcast-settings-{edition}.xml"] = f"{prefix}-{version}-settings.zip"
        return mapping

    def publish_version(self, version: str, build: int, archives_on: str, dsym: bool = True) -> None:
        """像旧的 release-macos.yml 那样发布一个版本：DMG、dSYM 和更新包都在 macos-vVERSION 上，appcast 指向 archives_on。"""
        release = self.release(f"macos-v{version}")
        feed = self.release("macos-appcast")
        archive_release = self.release(archives_on)
        names = []
        for edition in self.editions:
            prefix = "msime-macos" if edition == "full" else f"msime-macos-{edition}"
            dmg = release / f"{prefix}-{version}-universal.dmg"
            dmg.write_bytes(f"synthetic dmg {edition} {version}".encode())
            names.append(dmg.name)
        if dsym:
            (release / f"msime-macos-{version}-dsym.zip").write_bytes(b"synthetic dsym " + version.encode())
            names.append(f"msime-macos-{version}-dsym.zip")
        for appcast, archive in self.archives_for(version).items():
            path = archive_release / archive
            self.make_archive(path, "水杉输入法" if "settings" not in appcast else "MSIME", version, build)
            if archives_on == f"macos-v{version}":
                names.append(archive)
            self.generate(path, DOWNLOAD + archives_on + "/", feed / appcast)
        lines = [f"{hashlib.sha256((release / name).read_bytes()).hexdigest()}  {name}" for name in names]
        (release / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def run_script(self, *args: str, check: bool = True) -> subprocess.CompletedProcess:
        result = subprocess.run(["bash", *args], capture_output=True, text=True, env=self.env, cwd=self.directory)
        if check and result.returncode != 0:
            self.fail(f"{args[0]} 失败（{result.returncode}）：\n{result.stdout}\n{result.stderr}")
        return result

    def feed_tool(self, *args: str, check: bool = True) -> subprocess.CompletedProcess:
        result = subprocess.run([sys.executable, "-I", str(FEED_TOOL), *args], capture_output=True, text=True, env=self.env)
        if check and result.returncode != 0:
            self.fail(f"sparkle_feed.py {args[0]} 失败：\n{result.stdout}\n{result.stderr}")
        return result

    # ---- sparkle_feed.py ----

    def test_verify_rejects_tampered_feed_and_archive(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        feed = self.state / "releases/macos-appcast/appcast-full.xml"
        archives = self.state / "releases/macos-v1.2.0"
        self.feed_tool("verify", "--archives", str(archives), "--url-prefix", DOWNLOAD + "macos-v1.2.0/", str(feed))
        wrong_prefix = self.feed_tool("verify", "--url-prefix", APPCAST_PREFIX, str(feed), check=False)
        self.assertNotEqual(wrong_prefix.returncode, 0)
        tampered = self.directory / "appcast-full.xml"
        tampered.write_bytes(feed.read_bytes().replace(b"macos-v1.2.0/", b"macos-appcast/", 1))
        self.assertIn("签名与公钥不符", self.feed_tool("verify", str(tampered), check=False).stderr)
        archive = archives / "msime-macos-1.2.0-input-method.zip"
        content = bytearray(archive.read_bytes())
        content[len(content) // 2] ^= 0xFF
        archive.write_bytes(bytes(content))
        self.assertIn("与 appcast-full.xml", self.feed_tool("verify", "--archives", str(archives), str(feed), check=False).stderr)

    def test_compare_allows_only_the_url_to_change(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        old = self.state / "releases/macos-appcast/appcast-wubi.xml"
        archive = self.state / "releases/macos-v1.2.0/msime-macos-wubi-1.2.0-input-method.zip"
        moved = self.directory / "moved.xml"
        self.generate(archive, APPCAST_PREFIX, moved)
        self.feed_tool("compare", str(old), str(moved))
        os.utime(archive, (1792427900, 1792427900))
        later = self.directory / "later.xml"
        self.generate(archive, APPCAST_PREFIX, later)
        self.assertNotEqual(self.feed_tool("compare", str(old), str(later), check=False).returncode, 0)

    # ---- relocate-release-extras.sh ----

    def test_dry_run_changes_nothing(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        before = {tag: self.assets(tag) for tag in ("macos-v1.2.0", "macos-appcast")}
        result = self.run_script(str(SCRIPTS / "relocate-release-extras.sh"), "--dry-run", "1.2.0", str(self.directory / "work"))
        self.assertIn("没有上传或删除任何东西", result.stdout)
        self.assertEqual(self.mutating_calls(), [])
        self.assertEqual({tag: self.assets(tag) for tag in before}, before)

    def test_relocation_leaves_only_dmgs_and_signed_feeds(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        old_sums = (self.state / "releases/macos-v1.2.0/SHA256SUMS").read_text(encoding="utf-8")
        old_feeds = {path.name: path.read_bytes() for path in (self.state / "releases/macos-appcast").iterdir()}
        self.run_script(str(SCRIPTS / "relocate-release-extras.sh"), "1.2.0", str(self.directory / "work"))

        self.assertEqual(self.assets("macos-v1.2.0"), ["SHA256SUMS", "msime-macos-1.2.0-universal.dmg", "msime-macos-wubi-1.2.0-universal.dmg"])
        sums = (self.state / "releases/macos-v1.2.0/SHA256SUMS").read_text(encoding="utf-8").splitlines()
        self.assertEqual([line for line in old_sums.splitlines() if line.endswith(".dmg")], sums)
        self.assertEqual(self.assets("macos-symbols"), ["msime-macos-1.2.0-dsym.zip"])
        feed = self.state / "releases/macos-appcast"
        archives = sorted(self.archives_for("1.2.0").values())
        self.assertEqual(self.assets("macos-appcast"), sorted([*old_feeds, *archives, "sparkle-SHA256SUMS"]))
        for name in archives:
            digest = hashlib.sha256((feed / name).read_bytes()).hexdigest()
            self.assertIn(f"{digest}  {name}", old_sums)
        for appcast in old_feeds:
            self.feed_tool("verify", "--archives", str(feed), "--url-prefix", APPCAST_PREFIX, str(feed / appcast))
            old = self.directory / f"old-{appcast}"
            old.write_bytes(old_feeds[appcast])
            self.feed_tool("compare", str(old), str(feed / appcast))

        # 删除只在所有上传和核对之后：第一次 delete-asset macos-v1.2.0 之前，appcast 已经上传、dSYM 已经在 macos-symbols 上。
        calls = self.calls()
        first_delete = next(index for index, call in enumerate(calls) if call[1] == "delete-asset" and call[2] == "macos-v1.2.0")
        uploads = [index for index, call in enumerate(calls) if call[1] == "upload"]
        appcast_upload = next(index for index in uploads if any(Path(arg).name.startswith("appcast-") for arg in calls[index][3:]))
        archive_upload = next(index for index in uploads if any(arg.endswith("-input-method.zip") for arg in calls[index][3:]))
        dsym_upload = next(index for index in uploads if calls[index][2] == "macos-symbols")
        self.assertLess(archive_upload, appcast_upload)
        self.assertLess(max(appcast_upload, dsym_upload), first_delete)

        again = self.run_script(str(SCRIPTS / "relocate-release-extras.sh"), "1.2.0", str(self.directory / "work"))
        self.assertIn("已经只有 DMG 和 SHA256SUMS", again.stdout)

    def test_failed_download_check_deletes_nothing(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        old_feeds = {path.name: path.read_bytes() for path in (self.state / "releases/macos-appcast").iterdir()}
        release_before = {name: (self.state / "releases/macos-v1.2.0" / name).read_bytes() for name in self.assets("macos-v1.2.0")}
        self.env["MSIME_TEST_CORRUPT"] = "msime-macos-wubi-1.2.0-settings.zip"
        result = self.run_script(str(SCRIPTS / "relocate-release-extras.sh"), "1.2.0", str(self.directory / "work"), check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("下载到的内容与 sparkle-SHA256SUMS 不符", result.stderr)
        self.assertEqual({name: (self.state / "releases/macos-v1.2.0" / name).read_bytes() for name in self.assets("macos-v1.2.0")}, release_before)
        for appcast, content in old_feeds.items():
            self.assertEqual((self.state / "releases/macos-appcast" / appcast).read_bytes(), content)
        self.assertFalse([call for call in self.calls() if call[1] == "delete-asset"])

    def test_mismatched_checksum_stops_before_any_upload(self):
        self.publish_version("1.2.0", 1002000001, "macos-v1.2.0")
        archive = self.state / "releases/macos-v1.2.0/msime-macos-1.2.0-settings.zip"
        archive.write_bytes(archive.read_bytes() + b"x")
        result = self.run_script(str(SCRIPTS / "relocate-release-extras.sh"), "1.2.0", str(self.directory / "work"), check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.mutating_calls(), [])

    # ---- publish-sparkle-feed.sh ----

    def stage_new_version(self, version: str, build: int) -> Path:
        dist = self.directory / f"dist-{version}"
        dist.mkdir()
        lines = []
        for appcast, archive in self.archives_for(version).items():
            self.make_archive(dist / archive, "水杉输入法", version, build)
            self.generate(dist / archive, APPCAST_PREFIX, dist / appcast)
            lines.append(f"{hashlib.sha256((dist / archive).read_bytes()).hexdigest()}  {archive}")
        (dist / "sparkle-SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")
        return dist

    def test_publish_prunes_only_unreferenced_archives(self):
        self.publish_version("1.2.0", 1002000001, "macos-appcast", dsym=False)
        # 另一个不在这次发布里的版本仍指向它自己的更新包，不能被删。
        feed = self.release("macos-appcast")
        self.make_archive(feed / "msime-macos-tibetan-1.2.0-input-method.zip", "水杉藏文", "1.2.0", 1002000001)
        self.generate(feed / "msime-macos-tibetan-1.2.0-input-method.zip", APPCAST_PREFIX, feed / "appcast-tibetan.xml")
        dist = self.stage_new_version("1.3.0", 1003000001)
        self.run_script(str(SCRIPTS / "publish-sparkle-feed.sh"), str(dist), "1.3.0")
        remaining = [name for name in self.assets("macos-appcast") if name.endswith(".zip")]
        self.assertEqual(remaining, sorted([*self.archives_for("1.3.0").values(), "msime-macos-tibetan-1.2.0-input-method.zip"]))
        for appcast in self.archives_for("1.3.0"):
            self.assertEqual((feed / appcast).read_bytes(), (dist / appcast).read_bytes())

    def test_publish_refuses_to_go_backwards(self):
        self.publish_version("1.3.0", 1003000001, "macos-appcast", dsym=False)
        self.env.pop("MSIME_TEST_CORRUPT", None)
        dist = self.stage_new_version("1.2.0", 1002000001)
        before = len(self.mutating_calls())
        result = self.run_script(str(SCRIPTS / "publish-sparkle-feed.sh"), str(dist), "1.2.0", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("newer than", result.stderr)
        self.assertEqual(len(self.mutating_calls()), before)

    # ---- publish-dsym.sh ----

    def test_dsym_goes_to_symbols_under_the_release_name(self):
        dsym = self.directory / "msime-macos-1.3.0-dsym.zip"
        dsym.write_bytes(b"synthetic dsym")
        self.run_script(str(SCRIPTS / "publish-dsym.sh"), str(dsym), "msime-macos-1.3.0-beta-dsym.zip")
        self.run_script(str(SCRIPTS / "publish-dsym.sh"), str(dsym))
        self.assertEqual(self.assets("macos-symbols"), ["msime-macos-1.3.0-beta-dsym.zip", "msime-macos-1.3.0-dsym.zip"])
        self.assertNotEqual(self.run_script(str(SCRIPTS / "publish-dsym.sh"), str(dsym), "notes.zip", check=False).returncode, 0)


class DownloadGuideTests(unittest.TestCase):
    def guide(self, *extra: str) -> str:
        return subprocess.run(
            [sys.executable, "-I", str(SCRIPTS / "edition_bundle.py"), "download-guide", "--editions", "wubi,full,japanese", "--version", "1.2.0",
             "--download-base", DOWNLOAD + "macos-v1.2.0/", *extra],
            capture_output=True, text=True, check=True,
        ).stdout

    def test_full_edition_comes_first_with_names_from_the_edition_table(self):
        table = json.loads((ROOT / "shared/contracts/editions.json").read_text(encoding="utf-8"))
        names = {entry["id"]: entry["display_name"]["zh-Hans"] for entry in table["editions"]}
        rows = [line for line in self.guide("--homebrew").splitlines() if line.startswith("| ") and "dmg" in line]
        self.assertEqual(len(rows), 3)
        self.assertIn(f"**{names['full']}**", rows[0])
        self.assertIn(f"({DOWNLOAD}macos-v1.2.0/msime-macos-1.2.0-universal.dmg)", rows[0])
        self.assertIn("`brew install --cask metasequoiaime/tap/msime`", rows[0])
        self.assertIn(names["wubi"], rows[1])
        self.assertIn("msime-macos-japanese-1.2.0-universal.dmg", rows[2])

    def test_prerelease_guide_has_no_homebrew_column(self):
        guide = self.guide()
        self.assertNotIn("brew install", guide)
        self.assertIn("预览版", guide)
        self.assertTrue(guide.startswith("<!-- msime-download-guide -->\n"))


if __name__ == "__main__":
    unittest.main()

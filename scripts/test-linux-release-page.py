#!/usr/bin/env python3
"""验证 Linux 版本发布页只放安装包：下载指引的生成、源码包传到 linux-sources 与从那里取回，以及各处包定义都按 linux-sources 的地址引用源码包。

- `edition_linux.py download-guide` 为版本表里每个有 Linux 段的版本、两个架构各链接 deb、rpm 与 tar.gz，完整版排第一并标为推荐；legacy 包只在这次构建了它时出现；少一个安装包就失败；软件源一行只在正式版里出现。
- `packaging/publish-sources.sh` 在 linux-sources 不存在时创建它（预发布、不设为 latest），上传三个 tarball 与 msime-VERSION-SHA256SUMS 并按摘要核对；校验文件对不上或多列了文件时什么都不上传。
- `packaging/download-sources.sh` 优先从 linux-sources 取，没有这个版本时退回它自己的 linux-vVERSION 发布页；被篡改的文件核对不过。
- `packaging/render-definitions.sh` 对着 msime-VERSION-SHA256SUMS 核对源码包，对不上时在进任何容器之前失败。
- `release-linux.yml` 只把安装包和 SHA256SUMS 交给 publish-release.sh，RPM 规格、debian/watch 与 Gentoo 的 ebuild 模板都从 linux-sources 取源码包。

不访问 GitHub：`gh`、`curl` 与 `docker` 是在临时目录里模拟的替身。需要 bash 与 GNU 的 sha256sum，没有时跳过。
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINUX = ROOT / "platforms/linux"
PACKAGING = LINUX / "packaging"
GUIDE = LINUX / "scripts/edition_linux.py"
DOWNLOAD = "https://github.com/metasequoiaime/msime/releases/download/"
VERSION = "12.34.56"

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
    names = sorted(path.name for path in release.iterdir() if path.name != ".flags")
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
    (release / ".flags").write_text(json.dumps(rest), encoding="utf-8")
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
    matched = [path for path in sorted(release.iterdir()) if path.name != ".flags" and any(fnmatch.fnmatch(path.name, pattern) for pattern in patterns)] if release.is_dir() else []
    if not matched:
        print("no assets match the file pattern", file=sys.stderr); sys.exit(1)
    for path in matched:
        target = directory / path.name
        if target.exists() and "--clobber" not in rest:
            sys.exit(f"{target} exists")
        shutil.copyfile(path, target)
        if path.name in os.environ.get("MSIME_TEST_CORRUPT", "").split(","):
            with open(target, "ab") as handle:
                handle.write(b"tampered")
else:
    sys.exit(f"unsupported gh release {command}")
'''

# 模拟 curl：把 releases/download/<tag>/<name> 换成 state 里的文件。
FAKE_CURL = r'''
import os, shutil, sys
from pathlib import Path
state = Path(os.environ["MSIME_TEST_RELEASES"])
args = sys.argv[1:]
output = Path(args[args.index("-o") + 1])
url = args[-1]
prefix = "https://github.com/metasequoiaime/msime/releases/download/"
if not url.startswith(prefix):
    sys.exit(6)
tag, name = url[len(prefix):].split("/", 1)
source = state / "releases" / tag / name
if not source.is_file():
    print("curl: (22) The requested URL returned error: 404", file=sys.stderr); sys.exit(22)
shutil.copyfile(source, output)
'''

# 模拟 docker：只记下调用，render-definitions.sh 的核对在任何一次 docker run 之前完成。
FAKE_DOCKER = r'''
import json, os, sys
from pathlib import Path
with open(Path(os.environ["MSIME_TEST_RELEASES"]) / "docker.jsonl", "a", encoding="utf-8") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
'''


def available() -> str | None:
    if shutil.which("bash") is None:
        return "bash is not installed"
    sha = shutil.which("sha256sum")
    if sha is None or "GNU" not in subprocess.run([sha, "--version"], capture_output=True, text=True).stdout:
        return "GNU sha256sum is not installed"
    return None


def linux_editions() -> list[dict]:
    table = json.loads((ROOT / "shared/contracts/editions.json").read_text(encoding="utf-8"))
    return [entry for entry in table["editions"] if entry["platforms"].get("linux") is not None]


def installer_names(version: str, legacy: bool = True) -> list[str]:
    names = []
    for entry in linux_editions():
        package = entry["platforms"]["linux"]["package"]
        for arch, deb_arch in (("x86_64", "amd64"), ("aarch64", "arm64")):
            names += [f"{package}_{version}_{deb_arch}.deb", f"{package}-{version}-1.{arch}.rpm", f"{package}-{version}-linux-{arch}.tar.gz"]
    if legacy:
        names += [f"msime-linux-legacy_{version}_amd64.deb", f"msime-linux-legacy_{version}_arm64.deb"]
    return names


def source_names(version: str) -> list[str]:
    return [f"msime-{version}.tar.xz", f"msime-{version}-vendor.tar.xz", f"msime-{version}-frontend.tar.xz"]


def write_sums(directory: Path, names: list[str], sums: str) -> None:
    lines = [f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}" for name in names]
    (directory / sums).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


class Harness(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="msime-linux-release-page-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.state = self.directory / "state"
        (self.state / "releases").mkdir(parents=True)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        for name, source in (("gh", FAKE_GH), ("curl", FAKE_CURL), ("docker", FAKE_DOCKER)):
            script = self.directory / f"fake_{name}.py"
            script.write_text(source, encoding="utf-8")
            tool = self.bin / name
            tool.write_text(f"#!/usr/bin/env bash\nexec {shlex.quote(sys.executable)} -I {shlex.quote(str(script))} \"$@\"\n", encoding="utf-8")
            tool.chmod(0o755)
        if os.name == "nt":
            # 原生 Windows 可能只有 python.exe；渲染脚本也使用本次测试的解释器。
            python = self.bin / "python3"
            python.write_text(f"#!/usr/bin/env bash\nexec {shlex.quote(sys.executable)} \"$@\"\n", encoding="utf-8", newline="\n")
            python.chmod(0o755)
        self.env = {**os.environ, "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}", "MSIME_TEST_RELEASES": str(self.state)}
        self.env.pop("MSIME_RELEASE_DIR", None)
        self.env.pop("MSIME_DEFINITIONS", None)
        # Bash 脚本仍需把 POSIX 路径交给原生 git，不继承父门禁的 CMake 参数规则。
        self.env.pop("MSYS_NO_PATHCONV", None)
        self.env.pop("MSYS2_ARG_CONV_EXCL", None)

    def run_script(self, *args: str, env: dict | None = None, cwd: Path | None = None) -> subprocess.CompletedProcess:
        environment = {**self.env, **(env or {})}
        if os.name == "nt":
            args = tuple(Path(arg).as_posix() if Path(arg).is_absolute() else arg for arg in args)
            if "MSIME_RELEASE_DIR" in environment:
                environment["MSIME_RELEASE_DIR"] = Path(environment["MSIME_RELEASE_DIR"]).as_posix()
            # Git 的 bin/bash.exe 启动器会把 mingw64/bin 前置，盖过合成 curl。
            # 在实际 Bash 中恢复替身优先级，再直接用该解释器执行，避免再次经过启动器。
            command = ["bash", "-c", 'export PATH="$(cygpath -u "$1"):$PATH"; shift; exec "$BASH" "$@"',
                       "msime-linux-release-page", self.bin.as_posix(), *args]
        else:
            command = ["bash", *args]
        return subprocess.run(command, capture_output=True, text=True, errors="replace", env=environment, cwd=cwd or self.directory)

    def release(self, tag: str) -> Path:
        return self.state / "releases" / tag

    def make_sources(self, directory: Path, version: str = VERSION) -> list[str]:
        directory.mkdir(parents=True, exist_ok=True)
        names = source_names(version)
        for name in names:
            (directory / name).write_bytes(os.urandom(64) + name.encode())
        write_sums(directory, names, f"msime-{version}-SHA256SUMS")
        return names

    def gh_calls(self) -> list[list[str]]:
        log = self.state / "calls.jsonl"
        return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()] if log.is_file() else []


class DownloadGuideTest(unittest.TestCase):
    def guide(self, names: list[str], repositories: bool = True) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory(prefix="msime-linux-guide-") as directory:
            for name in names + ["SHA256SUMS"]:
                (Path(directory) / name).touch()
            command = [sys.executable, str(GUIDE), "download-guide", "--version", VERSION, "--dist", directory, "--download-base", f"{DOWNLOAD}linux-v{VERSION}/"]
            return subprocess.run(command + (["--repositories"] if repositories else []), capture_output=True, text=True)

    def test_every_edition_and_architecture_links_each_installer(self):
        result = self.guide(installer_names(VERSION))
        self.assertEqual(result.returncode, 0, result.stderr)
        text = result.stdout
        self.assertTrue(text.startswith("<!-- msime-download-guide -->\n"))
        self.assertTrue(text.endswith("<!-- /msime-download-guide -->\n"))
        for name in installer_names(VERSION):
            self.assertIn(f"]({DOWNLOAD}linux-v{VERSION}/{name})", text, name)
        rows = [line for line in text.splitlines() if line.startswith("| ") and "](" in line]
        editions = linux_editions()
        self.assertEqual(len(rows), len(editions) + 1, "one row per edition plus the legacy row")
        self.assertIn(f"**{editions[0]['display_name']['zh-Hans']}**（完整版，推荐）", rows[0])
        self.assertIn(f"{editions[0]['platforms']['linux']['package']}_{VERSION}_amd64.deb", rows[0])
        for row, entry in zip(rows[1:], editions[1:]):
            self.assertIn(f"| {entry['display_name']['zh-Hans']} |", row)
            self.assertIn(f"{entry['platforms']['linux']['package']}_{VERSION}_arm64.deb", row)
        self.assertIn("legacy", rows[-1])
        self.assertIn("uname -m", text)
        self.assertIn("#包管理器", text)
        self.assertNotIn("预览版", text)

    def test_legacy_row_only_when_it_was_built(self):
        result = self.guide(installer_names(VERSION, legacy=False))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("legacy", result.stdout)
        half = [name for name in installer_names(VERSION) if not name.endswith("_arm64.deb") or "legacy" not in name]
        self.assertNotEqual(self.guide(half).returncode, 0, "legacy packages for only one architecture")

    def test_each_format_names_the_systems_it_installs_on(self):
        # deb 与 tar.gz 是同一棵 bookworm 构建树（glibc 2.35、libstdc++ 12），rpm 只在 Fedora 44 上构建和验证，tar.gz 是 Debian 多架构布局，不是通用包。
        result = self.guide(installer_names(VERSION))
        self.assertEqual(result.returncode, 0, result.stderr)
        formats = {line.split("：")[0]: line for line in result.stdout.splitlines() if line.startswith("- ")}
        deb = next(line for line in formats.values() if "：`.deb`" in line)
        rpm = next(line for line in formats.values() if "：`.rpm`" in line)
        tarball = next(line for line in formats.values() if line.startswith("- `.tar.gz`"))
        self.assertIn("Debian 12、Ubuntu 22.04", deb)
        self.assertTrue(rpm.startswith("- Fedora 44："), rpm)
        self.assertIn("Debian 12、Ubuntu 22.04", tarball)
        self.assertIn("Arch、Fedora 上的 Fcitx5 找不到它的插件", tarball)
        self.assertNotIn("通用", tarball)
        legacy = next(line for line in formats.values() if "legacy `.deb`" in line)
        self.assertIn("glibc 2.28 到 2.34", legacy)

    def test_a_missing_installer_fails_rather_than_linking_nothing(self):
        for missing in (f"msime-linux-wubi-{VERSION}-1.aarch64.rpm", f"msime-linux_{VERSION}_amd64.deb", f"msime-linux-tibetan-{VERSION}-linux-x86_64.tar.gz"):
            names = [name for name in installer_names(VERSION) if name != missing]
            result = self.guide(names)
            self.assertNotEqual(result.returncode, 0, missing)
            self.assertIn("download guide", result.stderr)

    def test_a_prerelease_does_not_point_at_the_repositories(self):
        result = self.guide(installer_names(VERSION), repositories=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("#包管理器", result.stdout)
        self.assertIn("预览版", result.stdout)


class PublishSourcesTest(Harness):
    def test_creates_the_rolling_release_uploads_and_checks_digests(self):
        dist = self.directory / "dist"
        names = self.make_sources(dist)
        result = self.run_script(str(PACKAGING / "publish-sources.sh"), str(dist), VERSION)
        self.assertEqual(result.returncode, 0, result.stderr)
        release = self.release("linux-sources")
        flags = json.loads((release / ".flags").read_text(encoding="utf-8"))
        self.assertIn("--prerelease", flags)
        self.assertIn("--latest=false", flags)
        uploaded = sorted(path.name for path in release.iterdir() if path.name != ".flags")
        self.assertEqual(uploaded, sorted(names + [f"msime-{VERSION}-SHA256SUMS"]))
        for name in uploaded:
            self.assertEqual((release / name).read_bytes(), (dist / name).read_bytes())
        # 第二个版本加进同一个 release，不动第一个版本的文件。
        other = self.directory / "other"
        self.make_sources(other, "12.34.57")
        result = self.run_script(str(PACKAGING / "publish-sources.sh"), str(other), "12.34.57")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(all((release / name).read_bytes() == (dist / name).read_bytes() for name in names))
        self.assertEqual(len([path for path in release.iterdir() if path.name != ".flags"]), 8)

    def test_uploads_nothing_when_the_checksums_do_not_match(self):
        dist = self.directory / "dist"
        names = self.make_sources(dist)
        with open(dist / names[1], "ab") as handle:
            handle.write(b"x")
        result = self.run_script(str(PACKAGING / "publish-sources.sh"), str(dist), VERSION)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.release("linux-sources").exists())
        self.assertEqual(self.gh_calls(), [])

    def test_uploads_nothing_when_the_checksum_file_lists_something_else(self):
        dist = self.directory / "dist"
        names = self.make_sources(dist)
        (dist / "extra.deb").write_bytes(b"deb")
        write_sums(dist, names + ["extra.deb"], f"msime-{VERSION}-SHA256SUMS")
        result = self.run_script(str(PACKAGING / "publish-sources.sh"), str(dist), VERSION)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.gh_calls(), [])


class DownloadSourcesTest(Harness):
    def test_takes_the_sources_from_linux_sources(self):
        self.make_sources(self.release("linux-sources"))
        result = self.run_script(str(PACKAGING / "download-sources.sh"), VERSION, str(self.directory / "out"))
        self.assertEqual(result.returncode, 0, result.stderr)
        got = sorted(path.name for path in (self.directory / "out").iterdir())
        self.assertEqual(got, sorted([f"msime-{VERSION}.tar.xz", f"msime-{VERSION}-vendor.tar.xz", f"msime-{VERSION}-SHA256SUMS"]))
        self.assertTrue(all(call[2] == "linux-sources" for call in self.gh_calls()))

    def test_falls_back_to_the_version_page_for_releases_before_linux_sources(self):
        old = "0.12.1"
        page = self.release(f"linux-v{old}")
        page.mkdir(parents=True)
        names = source_names(old) + [f"msime-linux_{old}_amd64.deb"]
        for name in names:
            (page / name).write_bytes(os.urandom(32))
        write_sums(page, names, "SHA256SUMS")
        self.make_sources(self.release("linux-sources"))
        result = self.run_script(str(PACKAGING / "download-sources.sh"), old, str(self.directory / "out"))
        self.assertEqual(result.returncode, 0, result.stderr)
        got = sorted(path.name for path in (self.directory / "out").iterdir())
        self.assertEqual(got, sorted([f"msime-{old}.tar.xz", f"msime-{old}-vendor.tar.xz", "SHA256SUMS"]))

    def test_a_tampered_tarball_fails(self):
        self.make_sources(self.release("linux-sources"))
        result = self.run_script(str(PACKAGING / "download-sources.sh"), VERSION, str(self.directory / "out"), env={"MSIME_TEST_CORRUPT": f"msime-{VERSION}-vendor.tar.xz"})
        self.assertNotEqual(result.returncode, 0)

    def test_a_new_version_missing_everywhere_fails(self):
        self.make_sources(self.release("linux-sources"))
        self.release(f"linux-v{VERSION}").mkdir(parents=True)
        result = self.run_script(str(PACKAGING / "download-sources.sh"), "12.34.99", str(self.directory / "out"))
        self.assertNotEqual(result.returncode, 0)


class RenderDefinitionsTest(Harness):
    def render(self, env: dict) -> subprocess.CompletedProcess:
        return self.run_script(str(PACKAGING / "render-definitions.sh"), VERSION, str(self.directory / "defs"), env={"MSIME_DEFINITIONS": "rpm", **env}, cwd=ROOT)

    def test_downloads_the_sources_from_linux_sources_and_checks_them(self):
        self.make_sources(self.release("linux-sources"))
        result = self.render({})
        self.assertEqual(result.returncode, 0, result.stderr)
        release = self.directory / "defs/.release"
        self.assertFalse(release.exists(), "the download directory is removed afterwards")
        docker = (self.state / "docker.jsonl").read_text(encoding="utf-8")
        self.assertIn("fedora:44", docker)

    def test_a_tampered_tarball_fails_before_any_container(self):
        local = self.directory / "local"
        names = self.make_sources(local)
        with open(local / names[1], "ab") as handle:
            handle.write(b"x")
        result = self.render({"MSIME_RELEASE_DIR": str(local)})
        self.assertNotEqual(result.returncode, 0)
        docker = self.state / "docker.jsonl"
        runs = [json.loads(line) for line in docker.read_text(encoding="utf-8").splitlines()] if docker.is_file() else []
        self.assertFalse([run for run in runs if "fedora:44" in run], "no rpmbuild container after a failed check")


class DefinitionsPointAtLinuxSourcesTest(unittest.TestCase):
    def test_release_page_gets_only_installers(self):
        workflow = (ROOT / ".github/workflows/release-linux.yml").read_text(encoding="utf-8")
        publish = [line for line in workflow.splitlines() if "scripts/publish-release.sh" in line and "bash " in line]
        self.assertEqual(len(publish), 1, publish)
        self.assertNotIn(".tar.xz", publish[0])
        self.assertTrue(publish[0].endswith(" release-notes.md dist/*.deb dist/*.rpm dist/*.tar.gz dist/SHA256SUMS"), publish[0])
        # 发布页的全部内容在上传源码包之前备齐，源码包在版本发布页之前上线。
        steps = [workflow.index(f"- name: {name}") for name in ("Prepare the release page", "Upload the distro source tarballs to linux-sources", "Publish independent Linux release")]
        self.assertEqual(steps, sorted(steps))
        sums = [line for line in workflow.splitlines() if "> SHA256SUMS" in line]
        self.assertEqual(len(sums), 1, sums)
        self.assertNotIn(".tar.xz", sums[0])
        self.assertIn("packaging/publish-sources.sh dist", workflow)
        self.assertIn("edition_linux.py download-guide", workflow)

    def test_definitions_fetch_sources_from_linux_sources(self):
        base = re.escape("releases/download/linux-sources/")
        spec = (PACKAGING / "rpm/msime.spec").read_text(encoding="utf-8")
        self.assertRegex(spec, rf"(?m)^Source0:\s+%{{url}}/{base}msime-%{{version}}\.tar\.xz$")
        self.assertRegex(spec, rf"(?m)^Source1:\s+%{{url}}/{base}msime-%{{version}}-vendor\.tar\.xz$")
        watch = (PACKAGING / "debian/watch").read_text(encoding="utf-8")
        self.assertEqual(watch.count("releases/tags/linux-sources"), 2)
        self.assertEqual(watch.count("releases/download/linux-sources/msime-"), 2)
        ebuild = (PACKAGING / "gentoo/app-i18n/msime/msime.ebuild.in").read_text(encoding="utf-8")
        self.assertIn("releases/download/linux-sources/${P}-frontend.tar.xz", ebuild)
        # 源码包不再按版本发布页的地址引用：任何一处 linux-vVERSION 下的 .tar.xz 都是漏改的消费方。
        for path in sorted(PACKAGING.rglob("*")) + sorted((ROOT / ".github/workflows").glob("*linux*.yml")):
            if path.is_file():
                text = path.read_text(encoding="utf-8", errors="replace")
                self.assertIsNone(re.search(r"releases/download/linux-v[^\s\"']*\.tar\.xz", text), path)
        for workflow in ("publish-linux-copr.yml", "publish-linux-obs.yml", "publish-linux-ppa.yml"):
            text = (ROOT / ".github/workflows" / workflow).read_text(encoding="utf-8")
            self.assertIn("tools/platforms/linux/packaging/download-sources.sh", text, workflow)
            self.assertNotRegex(text, r"--pattern \"msime-\$VERSION(-vendor)?\.tar\.xz\"", workflow)


if __name__ == "__main__":
    reason = available()
    if reason:
        print(f"skipped: {reason}")
        sys.exit(0)
    unittest.main()

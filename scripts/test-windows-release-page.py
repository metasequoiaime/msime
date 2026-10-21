#!/usr/bin/env python3
"""Windows 版本发布页的契约：release-windows.yml 的 publish 步骤只上传安装程序、只列安装程序的 SHA256SUMS 和包管理器清单要的 `<安装程序>.sha256`，发布说明开头是 edition_windows.py 从版本表生成的下载指引；符号包由 publish-symbols.sh 传到滚动的 windows-symbols release，不上版本发布页。

这里把 workflow 里那一步的脚本原样取出来，放进一个临时目录里跑：版本表和 edition_windows.py 用仓库里的，生成发布说明和发布的两个脚本换成只记录参数的假脚本；publish-symbols.sh 对着一个假的 gh 跑。不联网、不碰任何 release。见 .agents/notes/implemented/process/2026-10-20-windows-release-page-symbols.md。
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release-windows.yml"
EDITION_WINDOWS = ROOT / "platforms/windows/scripts/edition_windows.py"
PUBLISH_SYMBOLS = ROOT / "platforms/windows/scripts/publish-symbols.sh"
EDITIONS = ROOT / "shared/contracts/editions.json"
DOWNLOAD = "https://github.com/metasequoiaime/msime/releases/download/"
MARKER = "<!-- msime-download-guide -->"

# 假的 gh：release view/create/upload 只在 $GH_STORE 下的目录里读写，view --jq 只认 publish-symbols.sh 用的那种按名字取 digest 的写法。
FAKE_GH = r'''
import hashlib, os, pathlib, re, shutil, sys
store = pathlib.Path(os.environ["GH_STORE"])
args = sys.argv[1:]
assert args[0] == "release", args
command, tag = args[1], args[2]
release = store / tag
if command == "view":
    if not release.is_dir():
        sys.exit(1)
    if "--jq" in args:
        name = re.search(r'\.name == "([^"]+)"', args[args.index("--jq") + 1]).group(1)
        path = release / name
        if path.is_file():
            print("sha256:" + hashlib.sha256(path.read_bytes()).hexdigest())
elif command == "create":
    release.mkdir()
    (store / "created.txt").write_text(" ".join(args[3:]), encoding="utf-8")
elif command == "upload":
    for path in args[3:]:
        if path != "--clobber":
            shutil.copy(path, release / pathlib.Path(path).name)
else:
    sys.exit(f"fake gh: unexpected {args}")
'''


def step_script(name: str) -> str:
    """取出 workflow 里名为 name 的那一步的 run 脚本，去掉 YAML 的缩进。"""
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    start = next(index for index, line in enumerate(lines) if line.strip() == f"- name: {name}")
    run = next(index for index in range(start, len(lines)) if lines[index].strip().startswith("run:"))
    inline = lines[run].strip()[len("run:"):].strip()
    if inline != "|":
        return inline + "\n"
    body = []
    indent = None
    for line in lines[run + 1:]:
        if line.strip() and indent is None:
            indent = len(line) - len(line.lstrip())
        if line.strip() and len(line) - len(line.lstrip()) < indent:
            break
        body.append(line[indent:] if line.strip() else "")
    return "\n".join(body) + "\n"


def windows_table() -> list[dict]:
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    return [entry for entry in table["editions"] if entry["platforms"].get("windows") is not None]


def installer_names(version: str) -> list[str]:
    return [f"{entry['platforms']['windows']['installer_base_name']}_v{version}.exe" for entry in windows_table()]


def symbol_names(version: str) -> list[str]:
    return [f"msime-windows-{entry['id']}-{version}-symbols.zip" for entry in windows_table()]


def sha256_line(path: pathlib.Path) -> str:
    return f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}"


class Workspace(unittest.TestCase):
    """一个临时的仓库检出：版本表、edition_windows.py 和 publish-symbols.sh 是仓库里的，dist 里是 publish 任务下载到的合成产物。"""

    def setUp(self):
        for tool in ("git", "sha256sum", "bash"):
            if shutil.which(tool) is None:
                self.skipTest(f"{tool} is not installed")
        self.work = pathlib.Path(tempfile.mkdtemp(prefix="windows-release-page-"))
        self.addCleanup(shutil.rmtree, self.work)
        (self.work / "platforms/windows/scripts").mkdir(parents=True)
        shutil.copy(EDITION_WINDOWS, self.work / "platforms/windows/scripts/edition_windows.py")
        shutil.copy(PUBLISH_SYMBOLS, self.work / "platforms/windows/scripts/publish-symbols.sh")
        (self.work / "shared/contracts").mkdir(parents=True)
        shutil.copy(EDITIONS, self.work / "shared/contracts/editions.json")
        bin_dir = self.work / "fake-bin"
        bin_dir.mkdir()
        (bin_dir / "gh.py").write_text(FAKE_GH, encoding="utf-8")
        (bin_dir / "gh").write_text(f'#!/bin/sh\nexec "{sys.executable}" -I "{bin_dir / "gh.py"}" "$@"\n', encoding="utf-8")
        (bin_dir / "gh").chmod(0o755)
        self.store = self.work / "fake-releases"
        self.store.mkdir()
        self.path = f"{bin_dir}{os.pathsep}{os.environ['PATH']}"
        self.dist = self.work / "dist"
        self.dist.mkdir()
        for index, name in enumerate(installer_names("1.1.0")):
            installer = self.dist / name
            installer.write_bytes(f"synthetic installer {index}".encode())
            # package 任务（PowerShell）写的 .sha256：一行 sha256sum 格式，结尾一个换行。
            (self.dist / f"{name}.sha256").write_text(sha256_line(installer) + "\n", encoding="utf-8")
        for index, name in enumerate(symbol_names("1.1.0")):
            (self.dist / name).write_bytes(f"synthetic symbols {index}".encode())

    def env(self, release: str) -> dict:
        # GITHUB_REPOSITORY 在 runner 上总有；GH_STORE 只给假的 gh 用。
        return {
            "PATH": self.path,
            "HOME": str(self.work),
            "GH_TOKEN": "synthetic",
            "GH_STORE": str(self.store),
            "VERSION": "1.1.0",
            "RELEASE": release,
            "PRERELEASE": "false",
            "GITHUB_REPOSITORY": "metasequoiaime/msime",
        }

    def run_step(self, name: str, release: str = "1.1.0") -> subprocess.CompletedProcess:
        # publish 任务的步骤写了 shell: bash，GitHub 用 `bash --noprofile --norc -eo pipefail {0}` 运行。
        return subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", step_script(name)],
                              cwd=self.work, env=self.env(release), capture_output=True, text=True)


class PublishStepTests(Workspace):
    STEP = "Publish independent Windows release"

    def setUp(self):
        super().setUp()
        scripts = self.work / "scripts"
        scripts.mkdir()
        (scripts / "generate-release-notes.py").write_text(
            "import sys\n"
            "output = sys.argv[sys.argv.index('--output') + 1]\n"
            "open(output, 'w', encoding='utf-8').write('## 更新内容\\n\\n- 合成的更新条目\\n')\n",
            encoding="utf-8",
        )
        # 假的 publish-release.sh：把 tag、标题和资产名逐行记下，发布说明另存一份。
        (scripts / "publish-release.sh").write_text(
            "set -eu\n"
            "{ echo \"$1\"; echo \"$3\"; shift 4; for asset in \"$@\"; do basename -- \"$asset\"; done; } > published.txt\n"
            "cp release-notes.md published-notes.md\n",
            encoding="utf-8",
        )
        git = ["git", "-c", "user.name=test", "-c", "user.email=test@example.invalid"]
        subprocess.run(["git", "init", "-q"], cwd=self.work, check=True)
        subprocess.run([*git, "commit", "-q", "--allow-empty", "-m", "synthetic"], cwd=self.work, check=True)
        subprocess.run([*git, "tag", "windows-v1.0.0"], cwd=self.work, check=True)

    def publish(self, release: str = "1.1.0") -> subprocess.CompletedProcess:
        return self.run_step(self.STEP, release)

    def published(self) -> list[str]:
        return (self.work / "published.txt").read_text(encoding="utf-8").splitlines()

    def test_release_page_has_installers_one_sha256sums_and_sidecars_but_no_symbols(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        published = self.published()
        self.assertEqual(published[:2], ["windows-v1.1.0", "Windows 1.1.0"])
        installers = installer_names("1.1.0")
        self.assertEqual(sorted(published[2:]), sorted(installers + ["SHA256SUMS"] + [f"{name}.sha256" for name in installers]))
        self.assertFalse([name for name in published if name.endswith("-symbols.zip")])
        sums = (self.dist / "SHA256SUMS").read_text(encoding="utf-8").splitlines()
        self.assertEqual(sorted(line.split("  ", 1)[1] for line in sums), sorted(installers), "SHA256SUMS lists exactly the installers")
        for line in sums:
            self.assertEqual(line, sha256_line(self.dist / line.split("  ", 1)[1]))

    def test_notes_start_with_the_download_guide(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        notes = (self.work / "published-notes.md").read_text(encoding="utf-8")
        self.assertTrue(notes.startswith(MARKER + "\n"))
        guide, changes = notes.split("<!-- /msime-download-guide -->\n", 1)
        self.assertIn(f"({DOWNLOAD}windows-v1.1.0/MetasequoiaIME-Full_Setup_v1.1.0.exe)", guide)
        self.assertNotIn("预览版", guide)
        self.assertEqual(changes, "\n## 更新内容\n\n- 合成的更新条目\n")

    def test_beta_notes_link_the_beta_tag_and_say_it_is_a_preview(self):
        result = self.publish("1.1.0-beta")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.published()[0], "windows-v1.1.0-beta")
        notes = (self.work / "published-notes.md").read_text(encoding="utf-8")
        # 安装程序名只带版本号，不带 -beta；链接指向 beta 的 tag。
        self.assertIn(f"({DOWNLOAD}windows-v1.1.0-beta/MetasequoiaIME-Full_Setup_v1.1.0.exe)", notes)
        self.assertIn("这是预览版", notes)

    def test_missing_installer_publishes_nothing(self):
        (self.dist / "MetasequoiaIME-Wubi_Setup_v1.1.0.exe").unlink()
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("MetasequoiaIME-Wubi_Setup_v1.1.0.exe", result.stderr)
        self.assertFalse((self.work / "published.txt").exists())

    def test_missing_symbols_publish_nothing(self):
        (self.dist / "msime-windows-tibetan-1.1.0-symbols.zip").unlink()
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("msime-windows-tibetan-1.1.0-symbols.zip", result.stderr)
        self.assertFalse((self.work / "published.txt").exists())

    def test_stray_installer_publishes_nothing(self):
        (self.dist / "MetasequoiaIME-Debug_Setup_v1.1.0.exe").write_bytes(b"stray")
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.work / "published.txt").exists())

    def test_sidecar_that_disagrees_with_the_installer_publishes_nothing(self):
        name = "MetasequoiaIME-Full_Setup_v1.1.0.exe"
        (self.dist / f"{name}.sha256").write_text(f"{'0' * 64}  {name}\n", encoding="utf-8")
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"{name}.sha256", result.stderr)
        self.assertFalse((self.work / "published.txt").exists())


class SymbolsStepTests(Workspace):
    STEP = "Upload the symbols to windows-symbols"

    def assets(self) -> dict[str, bytes]:
        release = self.store / "windows-symbols"
        return {path.name: path.read_bytes() for path in release.iterdir()} if release.is_dir() else {}

    def test_symbols_go_to_the_rolling_release_under_the_release_name(self):
        result = self.run_step(self.STEP)
        self.assertEqual(result.returncode, 0, result.stderr)
        assets = self.assets()
        self.assertEqual(sorted(assets), sorted(symbol_names("1.1.0")))
        for name, content in assets.items():
            self.assertEqual(content, (self.dist / name).read_bytes())
        self.assertIn("--prerelease --latest=false", (self.store / "created.txt").read_text(encoding="utf-8"))

    def test_beta_symbols_carry_the_suffix_and_keep_the_final_release_ones(self):
        self.assertEqual(self.run_step(self.STEP).returncode, 0)
        result = self.run_step(self.STEP, "1.1.0-beta")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(sorted(self.assets()), sorted(symbol_names("1.1.0") + symbol_names("1.1.0-beta")))
        self.assertEqual(self.assets()["msime-windows-full-1.1.0-beta-symbols.zip"],
                         (self.dist / "msime-windows-full-1.1.0-symbols.zip").read_bytes())

    def test_archive_of_another_version_is_refused_before_uploading(self):
        (self.dist / "msime-windows-full-1.0.9-symbols.zip").write_bytes(b"old")
        result = self.run_step(self.STEP)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("msime-windows-full-1.0.9-symbols.zip", result.stderr)
        self.assertEqual(self.assets(), {})


class DownloadGuideTests(unittest.TestCase):
    def guide(self, editions: str, *extra: str, check: bool = True) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, "-I", str(EDITION_WINDOWS), "download-guide", "--editions", editions, "--version", "1.2.0",
             "--download-base", DOWNLOAD + "windows-v1.2.0/", *extra],
            capture_output=True, text=True, check=check,
        )

    def test_full_edition_comes_first_with_names_from_the_edition_table(self):
        names = {entry["id"]: entry["display_name"]["zh-Hans"] for entry in windows_table()}
        guide = self.guide("wubi,full,japanese").stdout
        rows = [line for line in guide.splitlines() if line.startswith("| ") and ".exe" in line]
        self.assertEqual(len(rows), 3)
        self.assertIn(f"**{names['full']}**（完整版，推荐）", rows[0])
        self.assertIn(f"[MetasequoiaIME-Full_Setup_v1.2.0.exe]({DOWNLOAD}windows-v1.2.0/MetasequoiaIME-Full_Setup_v1.2.0.exe)", rows[0])
        self.assertIn(names["wubi"], rows[1])
        self.assertIn(f"({DOWNLOAD}windows-v1.2.0/MetasequoiaIME-Japanese_Setup_v1.2.0.exe)", rows[2])
        self.assertIn("大多数人下载第一行", guide)
        self.assertIn("`MetasequoiaIME-Full_Setup_v1.2.0.exe`", guide)
        self.assertTrue(guide.startswith(MARKER + "\n"))
        self.assertTrue(guide.endswith("<!-- /msime-download-guide -->\n"))

    def test_every_windows_edition_gets_a_row(self):
        guide = self.guide(",".join(entry["id"] for entry in windows_table())).stdout
        for name in installer_names("1.2.0"):
            self.assertIn(f"[{name}]", guide)

    def test_prerelease_guide_says_update_check_skips_it(self):
        self.assertIn("「检查更新」只提供正式版", self.guide("full", "--prerelease").stdout)

    def test_guide_without_the_full_edition_is_refused(self):
        self.assertNotEqual(self.guide("wubi", check=False).returncode, 0)


if __name__ == "__main__":
    unittest.main()

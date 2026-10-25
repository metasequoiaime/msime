#!/usr/bin/env python3
"""Android 版本发布页的契约：release-android.yml 的 publish 步骤只上传 APK、只列 APK 的 SHA256SUMS 和旧版本应用内更新要的 `<apk>.sha256`，发布说明开头是 edition_android.py 从版本表生成的下载指引。

这里把 workflow 里那一步的脚本原样取出来，放进一个临时目录里跑：版本表和 edition_android.py 用仓库里的，生成发布说明和发布的两个脚本换成只记录参数的假脚本，不联网、不碰任何 release。见 .agents/notes/implemented/process/2026-10-20-android-release-page-sha256sums.md。
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import shutil
import shlex
import stat
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release-android.yml"
EDITION_ANDROID = ROOT / "platforms/android/scripts/edition_android.py"
EDITIONS = ROOT / "shared/contracts/editions.json"
DOWNLOAD = "https://github.com/metasequoiaime/msime/releases/download/"
MARKER = "<!-- msime-download-guide -->"


def publish_script() -> str:
    """取出「Publish independent Android release」这一步的 run 脚本，去掉 YAML 的缩进。"""
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    start = next(index for index, line in enumerate(lines) if line.strip() == "- name: Publish independent Android release")
    run = next(index for index in range(start, len(lines)) if lines[index].strip() == "run: |")
    body = []
    indent = None
    for line in lines[run + 1:]:
        if line.strip() and indent is None:
            indent = len(line) - len(line.lstrip())
        if line.strip() and len(line) - len(line.lstrip()) < indent:
            break
        body.append(line[indent:] if line.strip() else "")
    return "\n".join(body) + "\n"


def android_table() -> list[dict]:
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    return [entry for entry in table["editions"] if entry["platforms"].get("android") is not None]


def apk_names() -> list[str]:
    return [entry["platforms"]["android"]["apk_name"] + ".apk" for entry in android_table()]


def remove_temporary_repository(path):
    def make_writable(function, name, error):
        if os.name != "nt" or not isinstance(error[1], PermissionError):
            raise error[1]
        os.chmod(name, stat.S_IWRITE)
        function(name)

    # Windows Git 的对象只读；只恢复本测试临时仓库中的删除权限。
    shutil.rmtree(path, onerror=make_writable)


class PublishStepTests(unittest.TestCase):
    def setUp(self):
        for tool in ("git", "sha256sum", "bash"):
            if shutil.which(tool) is None:
                self.skipTest(f"{tool} is not installed")
        self.work = pathlib.Path(tempfile.mkdtemp(prefix="android-release-page-"))
        self.addCleanup(remove_temporary_repository, self.work)
        (self.work / "platforms/android/scripts").mkdir(parents=True)
        shutil.copy(EDITION_ANDROID, self.work / "platforms/android/scripts/edition_android.py")
        (self.work / "shared/contracts").mkdir(parents=True)
        shutil.copy(EDITIONS, self.work / "shared/contracts/editions.json")
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
        subprocess.run([*git, "tag", "android-v1.0.0"], cwd=self.work, check=True)
        self.dist = self.work / "dist"
        self.dist.mkdir()
        for index, name in enumerate(apk_names()):
            (self.dist / name).write_bytes(f"synthetic apk {index}".encode())

    def publish(self, release: str = "1.1.0") -> subprocess.CompletedProcess:
        env = {
            "PATH": os.environ["PATH"],
            "HOME": str(self.work),
            "GH_TOKEN": "synthetic",
            "VERSION": "1.1.0",
            "RELEASE": release,
            "PRERELEASE": "false",
            "GITHUB_REPOSITORY": "metasequoiaime/msime",
        }
        # GitHub 托管 runner 上 run 的默认 shell 是 `bash -e {0}`。
        script = f'python3() {{ {shlex.quote(sys.executable)} -X utf8 "$@"; }}\n' + publish_script()
        return subprocess.run(["bash", "-e", "-c", script], cwd=self.work, env=env, capture_output=True, text=True, encoding="utf-8")

    def published(self) -> list[str]:
        return (self.work / "published.txt").read_text(encoding="utf-8").splitlines()

    def test_release_page_has_apks_one_sha256sums_and_legacy_sidecars(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        published = self.published()
        self.assertEqual(published[:2], ["android-v1.1.0", "Android 1.1.0"])
        apks = apk_names()
        self.assertEqual(sorted(published[2:]), sorted(apks + ["SHA256SUMS"] + [f"{apk}.sha256" for apk in apks]))
        sums = (self.dist / "SHA256SUMS").read_text(encoding="utf-8").splitlines()
        self.assertTrue(all(line[64:66] in ("  ", " *") for line in sums))
        self.assertEqual(sorted(line[66:] for line in sums), sorted(apks), "SHA256SUMS lists exactly the APKs")
        for line in sums:
            digest, name = line[:64], line[66:]
            self.assertEqual(digest, hashlib.sha256((self.dist / name).read_bytes()).hexdigest())
            # 旧版本应用内更新读的 `<apk>.sha256` 与 SHA256SUMS 里那一行相同。
            self.assertEqual((self.dist / f"{name}.sha256").read_text(encoding="utf-8").strip(), line)

    def test_notes_start_with_the_download_guide(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        notes = (self.work / "published-notes.md").read_text(encoding="utf-8")
        self.assertTrue(notes.startswith(MARKER + "\n"))
        guide, changes = notes.split("<!-- /msime-download-guide -->\n", 1)
        self.assertIn(f"({DOWNLOAD}android-v1.1.0/msime-android.apk)", guide)
        self.assertNotIn("预览版", guide)
        self.assertEqual(changes, "\n## 更新内容\n\n- 合成的更新条目\n")

    def test_beta_notes_say_it_is_a_preview(self):
        result = self.publish("1.1.0-beta")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.published()[0], "android-v1.1.0-beta")
        notes = (self.work / "published-notes.md").read_text(encoding="utf-8")
        self.assertIn(f"({DOWNLOAD}android-v1.1.0-beta/msime-android.apk)", notes)
        self.assertIn("这是预览版", notes)

    def test_missing_apk_publishes_nothing(self):
        (self.dist / "msime-android-wubi.apk").unlink()
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("msime-android-wubi.apk", result.stderr)
        self.assertFalse((self.work / "published.txt").exists())

    def test_stray_apk_publishes_nothing(self):
        (self.dist / "msime-android-debug.apk").write_bytes(b"stray")
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.work / "published.txt").exists())


class DownloadGuideTests(unittest.TestCase):
    def guide(self, editions: str, *extra: str, check: bool = True) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, "-I", "-X", "utf8", str(EDITION_ANDROID), "download-guide", "--editions", editions,
             "--download-base", DOWNLOAD + "android-v1.2.0/", *extra],
            capture_output=True, text=True, encoding="utf-8", check=check,
        )

    def test_full_edition_comes_first_with_names_from_the_edition_table(self):
        names = {entry["id"]: entry["display_name"]["zh-Hans"] for entry in android_table()}
        guide = self.guide("wubi,full,japanese").stdout
        rows = [line for line in guide.splitlines() if line.startswith("| ") and ".apk" in line]
        self.assertEqual(len(rows), 3)
        self.assertIn(f"**{names['full']}**（完整版，推荐）", rows[0])
        self.assertIn(f"[msime-android.apk]({DOWNLOAD}android-v1.2.0/msime-android.apk)", rows[0])
        self.assertIn(names["wubi"], rows[1])
        self.assertIn(f"({DOWNLOAD}android-v1.2.0/msime-android-japanese.apk)", rows[2])
        self.assertIn("大多数人下载第一行", guide)
        self.assertIn("`msime-android.apk`", guide)
        self.assertTrue(guide.startswith(MARKER + "\n"))
        self.assertTrue(guide.endswith("<!-- /msime-download-guide -->\n"))

    def test_every_android_edition_gets_a_row(self):
        ids = [entry["id"] for entry in android_table()]
        guide = self.guide(",".join(ids)).stdout
        for apk in apk_names():
            self.assertIn(f"[{apk}]", guide)

    def test_prerelease_guide_names_the_preview_channel(self):
        self.assertIn("「预览版」通道", self.guide("full", "--prerelease").stdout)

    def test_guide_without_the_full_edition_is_refused(self):
        self.assertNotEqual(self.guide("wubi", check=False).returncode, 0)


if __name__ == "__main__":
    unittest.main()

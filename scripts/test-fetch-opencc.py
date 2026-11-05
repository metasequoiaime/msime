#!/usr/bin/env python3
"""用合成输入验证 OpenCC 下载的完整性、限长、失败清理和离线拒绝。"""
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import fetch_opencc


class OpenCCInputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.cache = Path(self.temporary.name)
        self.data = b"synthetic\tfixture\n"
        self.entry = {"path": "sources/opencc/STCharacters.txt", "size": len(self.data),
                      "sha256": hashlib.sha256(self.data).hexdigest()}
        self.lock = {"commit": "a" * 40, "files": [self.entry]}
        self.target = self.cache / "STCharacters.txt"

    def test_cache_and_offline_source_never_access_network(self):
        self.target.write_bytes(self.data)
        with patch.object(fetch_opencc.download_retry, "urlopen", side_effect=AssertionError("不得联网")):
            for source in (False, True):
                self.assertEqual(fetch_opencc.prepare(self.lock, self.cache, offline=True, source=source), self.cache.resolve())
            self.target.write_bytes(b"x" * len(self.data))
            with self.assertRaises(ValueError):
                fetch_opencc.prepare(self.lock, self.cache, offline=True)
            self.target.unlink()
            with self.assertRaises(FileNotFoundError):
                fetch_opencc.prepare(self.lock, self.cache, offline=True)

    def test_download_uses_fixed_commit_and_replaces_corrupt_cache(self):
        self.target.write_bytes(b"corrupt")
        with patch.object(fetch_opencc.download_retry, "urlopen", return_value=io.BytesIO(self.data)) as request:
            fetch_opencc.prepare(self.lock, self.cache, offline=False)
        self.assertEqual(self.target.read_bytes(), self.data)
        self.assertIn("/" + "a" * 40 + "/sources/opencc/STCharacters.txt", request.call_args.args[0])
        self.assertEqual(list(self.cache.iterdir()), [self.target])

    def test_bad_downloads_preserve_previous_file_and_clean_staging(self):
        for data in (self.data[:-1], self.data + b"extra", b"x" * len(self.data)):
            with self.subTest(data=data):
                self.target.write_bytes(b"previous")
                with patch.object(fetch_opencc.download_retry, "urlopen", return_value=io.BytesIO(data)):
                    with self.assertRaises(ValueError):
                        fetch_opencc.prepare(self.lock, self.cache, offline=False)
                self.assertEqual(self.target.read_bytes(), b"previous")
                self.assertEqual(list(self.cache.iterdir()), [self.target])

    def test_network_failure_cleans_staging(self):
        with patch.object(fetch_opencc.download_retry, "urlopen", side_effect=OSError("synthetic failure")):
            with self.assertRaises(OSError):
                fetch_opencc.prepare(self.lock, self.cache, offline=False)
        self.assertEqual(list(self.cache.iterdir()), [])

    def test_source_mismatch_is_rejected_even_when_online(self):
        self.target.write_bytes(b"corrupt")
        with patch.object(fetch_opencc.download_retry, "urlopen", side_effect=AssertionError("不得联网")):
            with self.assertRaises(ValueError):
                fetch_opencc.prepare(self.lock, self.cache, offline=False, source=True)

    def test_repository_lock_and_licence_agree(self):
        lock = fetch_opencc.read_lock()
        self.assertEqual({Path(entry["path"]).name for entry in lock["files"]}, {
            "CJK_Compatibility_Ideographs.txt", "STCharacters.txt", "STPhrases.txt",
            "STPhrases_GeneratedFromRegionalPhrases.txt", "TSCharacters.txt", "TSPhrases.txt", "TWPhrases.txt",
            "TWVariants.txt", "TWVariantsPhrases.txt", "LICENSE"})
        references = json.loads((fetch_opencc.ROOT / "resources/dictionary-sources.lock.json").read_text())["references"]
        self.assertEqual(lock["upstream"], references["OpenCC"])
        licence = next(entry for entry in lock["files"] if entry["path"].endswith("/LICENSE"))
        self.assertTrue(fetch_opencc.matches(fetch_opencc.ROOT / "resources/licenses/OpenCC-Apache-2.0.txt", licence))
        for consumer in ("platforms/windows/Collect-Notices.ps1", "platforms/linux/CMakeLists.txt",
                         "platforms/macos/CMakeLists.txt", "platforms/ios/project.yml",
                         "platforms/android/build-native.sh", "platforms/harmony/stage-resources.sh"):
            with self.subTest(consumer=consumer):
                text = (fetch_opencc.ROOT / consumer).read_text(encoding="utf-8")
                live = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
                self.assertIn("resources/licenses/OpenCC-Apache-2.0.txt", live)

    def test_linux_source_packages_stage_opencc_for_offline_cargo(self):
        source_tarballs = (fetch_opencc.ROOT / "platforms/linux/packaging/make-source-tarballs.sh").read_text()
        rpm = (fetch_opencc.ROOT / "platforms/linux/packaging/rpm/msime.spec").read_text()
        debian = (fetch_opencc.ROOT / "platforms/linux/packaging/debian/rules").read_text()
        self.assertIn('mkdir -p "$vendor/frontend" "$vendor/voice-runtime" "$vendor/opencc"', source_tarballs)
        self.assertIn('opencc_dir=$(python3 scripts/fetch_opencc.py)', source_tarballs)
        self.assertIn('cp -a "$opencc_dir/." "$vendor/opencc/"', source_tarballs)
        self.assertIn('MSIME_OPENCC_SOURCE_DIR="$PWD/vendor/opencc"', rpm)
        self.assertIn('export MSIME_OPENCC_SOURCE_DIR := $(CURDIR)/vendor/opencc', debian)

    def test_lock_rejects_traversal_and_unpinned_revision(self):
        original = json.loads(fetch_opencc.LOCK.read_text())
        for commit, path in (("main", "sources/opencc/LICENSE"),
                             ("a" * 40, "sources/opencc/.."),
                             ("a" * 40, "sources/opencc/../../outside")):
            lock = dict(original, commit=commit, files=[dict(self.entry, path=path)])
            record = self.cache / "lock.json"
            record.write_text(json.dumps(lock))
            with self.subTest(commit=commit, path=path), patch.object(fetch_opencc, "LOCK", record):
                with self.assertRaises(ValueError):
                    fetch_opencc.read_lock()

    @unittest.skipUnless(shutil.which("git"), "需要 Git 验证 Windows 检出")
    def test_windows_checkout_preserves_locked_licence_bytes(self):
        checkout = self.cache / "checkout"
        checkout.mkdir()
        relative = "resources/licenses/OpenCC-Apache-2.0.txt"
        licence = checkout / relative
        licence.parent.mkdir(parents=True)
        original = (fetch_opencc.ROOT / relative).read_bytes()
        licence.write_bytes(original)
        (checkout / ".gitattributes").write_bytes((fetch_opencc.ROOT / ".gitattributes").read_bytes())

        def git(*arguments):
            return subprocess.run(
                ["git", "-c", "core.autocrlf=true", "-c", "core.eol=crlf", *arguments],
                cwd=checkout, check=True, capture_output=True,
            )

        git("init", "--quiet")
        git("add", "--", ".gitattributes", relative)
        licence.unlink()
        git("checkout-index", "--force", "--", relative)
        self.assertEqual(licence.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()

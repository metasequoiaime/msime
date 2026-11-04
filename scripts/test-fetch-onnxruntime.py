#!/usr/bin/env python3
"""用合成的压缩包验证精简版 ONNX Runtime 的下载核对和解压，并核对锁文件的形状、它与算子清单的对应。不联网。"""
import gzip
import hashlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import fetch_onnxruntime  # noqa: E402

CONFIG = fetch_onnxruntime.ROOT / "tools/onnxruntime-minimal/required_operators_and_types.config"


def artifact_for(library: bytes, url: str = "https://example.invalid/libonnxruntime-test.a.gz") -> tuple[dict, bytes]:
    packed = gzip.compress(library, mtime=0)
    return {
        "name": "libonnxruntime-test.a.gz",
        "url": url,
        "sha256": hashlib.sha256(packed).hexdigest(),
        "size": len(packed),
        "library": "libonnxruntime.a",
        "library_sha256": hashlib.sha256(library).hexdigest(),
        "library_size": len(library),
    }, packed


class Response(io.BytesIO):
    headers = {}

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def serve(packed: bytes):
    return patch.object(fetch_onnxruntime.download_retry, "urlopen", side_effect=lambda *_args, **_kwargs: Response(packed))


class InstallTests(unittest.TestCase):
    def test_a_matching_archive_is_unpacked_and_cached(self):
        library = b"static library bytes" * 100
        artifact, packed = artifact_for(library)
        with tempfile.TemporaryDirectory() as out, serve(packed) as fetch:
            out = Path(out)
            self.assertEqual(fetch_onnxruntime.install(artifact, out), out)
            self.assertEqual((out / "libonnxruntime.a").read_bytes(), library)
            self.assertTrue((out / ".archive" / artifact["name"]).is_file())
            # 库已经是锁里那一份：不下载。
            fetch_onnxruntime.install(artifact, out)
            # 库被换掉了：从缓存的压缩包恢复，也不下载。
            (out / "libonnxruntime.a").write_bytes(b"some other build")
            fetch_onnxruntime.install(artifact, out)
            self.assertEqual((out / "libonnxruntime.a").read_bytes(), library)
            self.assertEqual(fetch.call_count, 1)

    def test_the_library_takes_the_name_the_lock_gives(self):
        artifact, packed = artifact_for(b"coff archive")
        artifact["library"] = "onnxruntime.lib"
        with tempfile.TemporaryDirectory() as out, serve(packed):
            fetch_onnxruntime.install(artifact, Path(out))
            self.assertEqual(sorted(path.name for path in Path(out).iterdir()), [".archive", "onnxruntime.lib"])

    def test_a_tampered_download_is_discarded(self):
        artifact, _ = artifact_for(b"the real library")
        tampered = gzip.compress(b"something else!!", mtime=0)
        artifact["size"] = len(tampered)
        with tempfile.TemporaryDirectory() as out, serve(tampered):
            out = Path(out)
            with self.assertRaises(SystemExit):
                fetch_onnxruntime.install(artifact, out)
            self.assertFalse((out / "libonnxruntime.a").exists())
            self.assertEqual(list((out / ".archive").iterdir()), [])

    def test_an_archive_that_unpacks_to_another_library_leaves_nothing(self):
        artifact, packed = artifact_for(b"the real library")
        other = b"another library!"
        artifact["library_sha256"] = hashlib.sha256(other).hexdigest()
        with tempfile.TemporaryDirectory() as out, serve(packed):
            out = Path(out)
            (out / "libonnxruntime.a").write_bytes(b"previous build")
            with self.assertRaises(SystemExit):
                fetch_onnxruntime.install(artifact, out)
            # 原来的库不被半成品替换，目录里也不留暂存文件。
            self.assertEqual((out / "libonnxruntime.a").read_bytes(), b"previous build")
            self.assertEqual(sorted(path.name for path in out.iterdir()), [".archive", "libonnxruntime.a"])

    def test_an_oversized_response_stops_early(self):
        artifact, packed = artifact_for(b"x" * 64)
        with tempfile.TemporaryDirectory() as out, serve(packed + b"tail"):
            with self.assertRaisesRegex(SystemExit, "larger than the lock"):
                fetch_onnxruntime.install(artifact, Path(out))

    def test_plain_http_is_refused(self):
        artifact, _ = artifact_for(b"x", url="http://example.invalid/libonnxruntime-test.a.gz")
        with tempfile.TemporaryDirectory() as out, self.assertRaisesRegex(SystemExit, "non-HTTPS"):
            fetch_onnxruntime.install(artifact, Path(out))


class LockTests(unittest.TestCase):
    lock = json.loads(fetch_onnxruntime.LOCK.read_text(encoding="utf-8"))

    def test_every_target_is_pinned_to_the_release(self):
        lock = self.lock
        self.assertEqual(lock["license"], "MIT")
        self.assertRegex(lock["source_commit"], r"^[0-9a-f]{40}$")
        self.assertTrue(lock["release_tag"].startswith(f"onnxruntime-minimal-{lock['onnxruntime_version']}-r"))
        base = f"https://github.com/metasequoiaime/msime/releases/download/{lock['release_tag']}/"
        self.assertTrue(lock["targets"])
        for target, artifact in lock["targets"].items():
            with self.subTest(target=target):
                windows = target.endswith("-pc-windows-msvc")
                self.assertEqual(artifact["library"], "onnxruntime.lib" if windows else "libonnxruntime.a")
                self.assertEqual(artifact["name"], f"onnxruntime-{target}.lib.gz" if windows else f"libonnxruntime-{target}.a.gz")
                self.assertEqual(artifact["url"], base + artifact["name"])
                for key in ("sha256", "library_sha256"):
                    self.assertRegex(artifact[key], r"^[0-9a-f]{64}$")
                self.assertGreater(artifact["size"], 0)
                self.assertGreater(artifact["library_size"], artifact["size"])

    def test_no_32_bit_target(self):
        # 32 位设备不跑 ONNX 重排，照旧用随包模型；见 tools/onnxruntime-minimal/README.md。
        for target in self.lock["targets"]:
            self.assertFalse(target.startswith(("armv7", "i686", "thumbv7")), target)

    def test_the_operator_list_is_the_one_the_libraries_were_built_from(self):
        # 改了算子清单就要重编并重新发布全部平台，再更新锁；只改清单不重编，这里失败。
        self.assertEqual(hashlib.sha256(CONFIG.read_bytes()).hexdigest(), self.lock["operators_config_sha256"])


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""验证 nightly 直接提供原生分发文件，过滤源码和调试符号。"""

import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "packaging", Path(__file__).with_name("package-nightly.py"))
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class NightlyPackagingTests(unittest.TestCase):
    def prepare(self, source):
        expected = {}
        for platform, suffixes in packaging.PLATFORM_SUFFIXES.items():
            directory = source / platform / "artifact"
            directory.mkdir(parents=True)
            for suffix in suffixes:
                name = f"msime-{platform}{suffix}"
                payload = f"synthetic {platform} {suffix}".encode()
                (directory / name).write_bytes(payload)
                expected[name] = payload
            (directory / "SHA256SUMS").write_text("ignored manifest")
            (directory / "symbols-dsym.zip").write_bytes(b"ignored symbols")
            (directory / "source.tar.xz").write_bytes(b"ignored sources")
        simulator = source / "ios"
        simulator.mkdir()
        (simulator / "msime-ios-simulator.tar.gz").write_bytes(b"simulator only")
        return expected

    def test_native_files_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / "source", root / "dist"
            expected = self.prepare(source)
            packaging.package(source, output, "a" * 40)
            self.assertEqual({path.name for path in output.iterdir()},
                             set(expected) | {"SHA256SUMS", "README.txt"})
            for name, payload in expected.items():
                self.assertEqual((output / name).read_bytes(), payload)
            entries = (output / "SHA256SUMS").read_text().splitlines()
            self.assertEqual(len(entries), len(expected))
            for line in entries:
                digest, name = line.split("  ")
                self.assertEqual(hashlib.sha256(expected[name]).hexdigest(), digest)
            self.assertIn("TestFlight", (output / "README.txt").read_text())
            self.assertIn("unsigned HAP", (output / "README.txt").read_text())

    def test_arch_binary_package_is_an_installer(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            directory = source / "linux" / "msime-linux-arch-1.2.3"
            directory.mkdir(parents=True)
            package = directory / "msime-bin-1.2.3-1-x86_64.pkg.tar.zst"
            package.write_bytes(b"synthetic Arch package")
            (directory / "source.tar.zst").write_bytes(b"not an installer")
            self.assertEqual(packaging.platform_files(source, "linux"), [package])

    def test_oversized_and_duplicate_files(self):
        for case in ("oversized", "duplicate"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source, output = root / "source", root / "dist"
                self.prepare(source)
                if case == "oversized":
                    with self.assertRaisesRegex(ValueError, "超出附件大小限制"):
                        packaging.package(source, output, "a" * 40, limit=1)
                else:
                    extra = source / "android" / "extra"
                    extra.mkdir()
                    (extra / "msime-android.apk").write_bytes(b"different edition")
                    with self.assertRaisesRegex(ValueError, "分发文件重名"):
                        packaging.package(source, output, "a" * 40)
                self.assertFalse(output.exists())

    def test_missing_platform_and_stale_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, output = Path(temporary) / "source", Path(temporary) / "dist"
            self.prepare(source)
            next((source / "android").rglob("*.apk")).unlink()
            with self.assertRaisesRegex(ValueError, "缺少平台分发文件"):
                packaging.package(source, output, "a" * 40)
            self.assertFalse(output.exists())
            output.mkdir()
            (output / "stale.apk").touch()
            with self.assertRaisesRegex(ValueError, "输出目录必须为空"):
                packaging.package(source, output, "a" * 40)


if __name__ == "__main__":
    unittest.main()

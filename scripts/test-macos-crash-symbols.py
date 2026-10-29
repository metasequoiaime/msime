#!/usr/bin/env python3
"""用真实 Clang/Swift Mach-O 验证行表、架构和 UUID，避免空 dSYM 通过发布。"""

from __future__ import annotations

import importlib.util
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if "--require-tools" in sys.argv:
    sys.argv.remove("--require-tools")
    if platform.system() != "Darwin" or not shutil.which("xcrun") or not shutil.which("rustc"):
        raise SystemExit("macOS 符号回归需要 macOS、Xcode 与 Rust 工具链")
SPEC = importlib.util.spec_from_file_location("macos_crash_symbols", ROOT / "platforms/macos/scripts/crash_symbols.py")
symbols = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(symbols)


@unittest.skipUnless(platform.system() == "Darwin" and shutil.which("xcrun"), "需要 macOS 与 Xcode 命令行工具")
class CrashSymbolsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="msime-crash-symbols-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "synthetic.c"
        self.source.write_text("__attribute__((noinline)) int synthetic_crash(int x) { return x + 1; }\nint main(void) { return synthetic_crash(1); }\n")

    def compile(self, name="fixture", arch="arm64", debug=True):
        binary = self.root / name
        subprocess.run(["xcrun", "clang", "-arch", arch, *(["-gline-tables-only"] if debug else []),
                        str(self.source), "-o", str(binary)], check=True)
        return binary, Path(str(binary) + ".dSYM")

    def test_source_roundtrip_after_strip(self):
        binary, dsym = self.compile(arch=platform.machine())
        symbols.collect(binary, self.root / "archive", dsym, "1.2.3")
        archived = self.root / "archive/fixture.dSYM"
        subprocess.run(["xcrun", "strip", "-x", str(binary)], check=True)
        symbols.validate(binary, archived)
        nm = subprocess.check_output(["xcrun", "nm", str(symbols.dwarf_file(archived))], text=True)
        address = next(row.split()[0] for row in nm.splitlines() if row.endswith(" _synthetic_crash"))
        frame = subprocess.check_output(["xcrun", "atos", "-arch", platform.machine(),
                                         "-o", str(symbols.dwarf_file(archived)), "0x" + address], text=True)
        self.assertIn("synthetic.c:1", frame)
        manifest = json.loads((self.root / "archive/manifest.json").read_text())
        self.assertEqual(manifest["version"], "1.2.3")
        self.assertEqual(manifest["modules"][0]["uuids"], symbols.identities(binary))
        arch = platform.machine()
        uuid = symbols.identities(binary)[arch]
        result = symbols.symbolize(self.root / "archive", "fixture", uuid.replace("-", "").lower(), arch,
                                   int(address, 16) - 0x100000000)
        self.assertIn("synthetic.c:1", result)
        with self.assertRaisesRegex(ValueError, "找不到唯一匹配"):
            symbols.symbolize(self.root / "archive", "fixture", "00" * 16, arch, 0)
        with symbols.dwarf_file(archived).open("ab") as stream:
            stream.write(b"synthetic-tamper")
        with self.assertRaisesRegex(ValueError, "哈希或 UUID 不匹配"):
            symbols.symbolize(self.root / "archive", "fixture", uuid, arch, 0)

    def test_reject_empty_dsym_even_when_dsymutil_succeeds(self):
        binary, dsym = self.compile(debug=False)
        subprocess.run(["xcrun", "dsymutil", str(binary), "-o", str(dsym)], check=True, capture_output=True)
        with self.assertRaisesRegex(ValueError, "没有可还原的源码行"):
            symbols.validate(binary, dsym)

    def test_unicode_zip_roundtrip_restores_source_from_standard_extraction(self):
        binary, dsym = self.compile(name="水杉输入法", arch=platform.machine())
        source = self.root / "dsym"
        symbols.collect(binary, source, dsym, "1.2.3")
        packed = self.root / "symbols.zip"
        symbols.pack(source, packed)
        dwarf = symbols.dwarf_file(source / "水杉输入法.dSYM")
        relative = str(Path(source.name) / dwarf.relative_to(source))
        with zipfile.ZipFile(packed) as archive:
            self.assertTrue(archive.getinfo(relative).flag_bits & 0x800)
            self.assertEqual(archive.read(relative), dwarf.read_bytes())
            archive.extractall(self.root / "unpacked")
        extracted = self.root / "unpacked/dsym"
        module = json.loads((extracted / "manifest.json").read_text())["modules"][0]
        nm = symbols.run("xcrun", "nm", str(symbols.dwarf_file(extracted / module["dsym"])))
        address = next(row.split()[0] for row in nm.splitlines() if row.endswith(" _synthetic_crash"))
        frame = symbols.symbolize(extracted, "水杉输入法", module["uuids"][platform.machine()],
                                  platform.machine(), int(address, 16) - 0x100000000)
        self.assertIn("synthetic.c:1", frame)
        original = packed.read_bytes()
        outside = self.root / "synthetic-outside"
        outside.write_text("synthetic")
        (source / "unsafe-link").symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "实际目录与普通文件"):
            symbols.pack(source, packed)
        self.assertEqual(packed.read_bytes(), original)

    def test_manifest_uses_the_capture_build_identity(self):
        binary, dsym = self.compile(arch=platform.machine())
        identity = {"MSIME_BUILD_REVISION": "1" * 40, "MSIME_BUILD_RUN": "synthetic-local-run",
                    "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2"}
        with patch.dict(os.environ, identity):
            symbols.collect(binary, self.root / "explicit", dsym, "1.2.3")
            manifest = json.loads((self.root / "explicit/manifest.json").read_text())
            self.assertEqual(manifest["source_revision"], identity["MSIME_BUILD_REVISION"])
            self.assertEqual(manifest["build_run"], identity["MSIME_BUILD_RUN"])
            self.assertEqual(manifest["build_attempt"], "2")
            del os.environ["MSIME_BUILD_RUN"]
            symbols.collect(binary, self.root / "github", dsym, "1.2.3")
            manifest = json.loads((self.root / "github/manifest.json").read_text())
            self.assertEqual(manifest["build_run"], "123")

    def test_reject_mixed_identity_before_replacing_archived_symbols(self):
        binary, dsym = self.compile(arch=platform.machine())
        archive = self.root / "archive"
        identity = {"MSIME_BUILD_REVISION": "1" * 40, "MSIME_BUILD_RUN": "synthetic-run",
                    "GITHUB_RUN_ATTEMPT": "1"}
        with patch.dict(os.environ, identity):
            symbols.collect(binary, archive, dsym, "1.2.3")
        original_manifest = (archive / "manifest.json").read_bytes()
        original_dwarf = symbols.dwarf_file(archive / "fixture.dSYM").read_bytes()
        self.source.write_text("int main(void) { return 8; }\n")
        binary, dsym = self.compile(arch=platform.machine())
        cases = [("version", {}, "1.2.4"), ("revision", {"MSIME_BUILD_REVISION": "2" * 40}, "1.2.3"),
                 ("run", {"MSIME_BUILD_RUN": "other-run"}, "1.2.3"),
                 ("attempt", {"GITHUB_RUN_ATTEMPT": "2"}, "1.2.3")]
        for name, changed, version in cases:
            with self.subTest(name=name), patch.dict(os.environ, {**identity, **changed}):
                with self.assertRaisesRegex(ValueError, "构建身份"):
                    symbols.collect(binary, archive, dsym, version)
                self.assertEqual((archive / "manifest.json").read_bytes(), original_manifest)
                self.assertEqual(symbols.dwarf_file(archive / "fixture.dSYM").read_bytes(), original_dwarf)

    def test_reject_other_build_and_missing_architecture(self):
        arm, arm_dsym = self.compile("arm", "arm64")
        intel, intel_dsym = self.compile("intel", "x86_64")
        binary = self.root / "universal"
        subprocess.run(["xcrun", "lipo", "-create", str(arm), str(intel), "-output", str(binary)], check=True)
        with self.assertRaisesRegex(ValueError, "未覆盖全部架构"):
            symbols.merge(binary, self.root / "missing.dSYM", [arm_dsym])
        self.source.write_text("int main(void) { return 7; }\n")
        _, other = self.compile("other", "arm64")
        with self.assertRaisesRegex(ValueError, "不属于本次构建"):
            symbols.merge(binary, self.root / "wrong.dSYM", [other, intel_dsym])
        output = self.root / "universal.dSYM"
        symbols.merge(binary, output, [arm_dsym, intel_dsym])
        self.assertEqual(set(symbols.validate(binary, output)), {"arm64", "x86_64"})

    def test_preserve_swift_automatic_dsym(self):
        source = self.root / "synthetic.swift"
        source.write_text('@_cdecl("synthetic_crash") public func syntheticCrash(_ x: Int32) -> Int32 { x + 1 }\n')
        binaries, dsyms = [], []
        for arch in ["arm64", "x86_64"]:
            binary = self.root / (arch + ".dylib")
            subprocess.run(["xcrun", "swiftc", "-emit-library", "-parse-as-library", "-Osize", "-wmo",
                            "-gline-tables-only", "-target", arch + "-apple-macosx13.0", str(source),
                            "-o", str(binary)], check=True)
            binaries.append(binary)
            dsyms.append(Path(str(binary) + ".dSYM"))
        merged = self.root / "backend.dylib"
        subprocess.run(["xcrun", "lipo", "-create", *map(str, binaries), "-output", str(merged)], check=True)
        combined = Path(str(merged) + ".dSYM")
        symbols.merge(merged, combined, dsyms)
        subprocess.run(["xcrun", "strip", "-x", str(merged)], check=True)
        symbols.collect(merged, self.root / "archive", combined, "1.2.3")
        self.assertEqual(set(symbols.validate(merged, self.root / "archive/backend.dylib.dSYM")), {"arm64", "x86_64"})
        for arch, uuid in symbols.identities(merged).items():
            nm = subprocess.check_output(["xcrun", "nm", "-arch", arch, str(merged)], text=True)
            # @_cdecl 入口是编译器生成的桥接桩，源码行为零；取实际 Swift 函数的地址。
            address = next(row.split()[0] for row in nm.splitlines() if "syntheticCrash" in row)
            frame = symbols.symbolize(self.root / "archive", "backend.dylib", uuid, arch, int(address, 16))
            self.assertIn("synthetic.swift:1", frame)

    def test_preserve_rust_packed_dsym(self):
        if not shutil.which("rustc"):
            self.skipTest("需要 Rust 工具链")
        source = self.root / "synthetic.rs"
        source.write_text('fn main() { std::hint::black_box(7); }\n')
        binary = self.root / "rust-fixture"
        subprocess.run(["rustc", "-C", "debuginfo=1", "-C", "split-debuginfo=packed", "-C", "strip=none",
                        str(source), "-o", str(binary)], check=True)
        symbols.collect(binary, self.root / "archive", Path(str(binary) + ".dSYM"), "1.2.3")
        symbols.validate(binary, self.root / "archive/rust-fixture.dSYM")


if __name__ == "__main__":
    unittest.main()

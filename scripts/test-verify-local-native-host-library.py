#!/usr/bin/env python3
"""原生门禁按 CMake 的 Rust 目标/profile 构建，并且只分发成功产出的运行库。"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BASH = shutil.which("bash")
source = (ROOT / "scripts/verify-local.sh").read_text(encoding="utf-8")
start = source.index("build_windows_host_library() {")
end = source.index('\nnote "compile: native host"', start)
HELPER = source[start:end]


@unittest.skipUnless(BASH, "需要 Bash 执行原生库构建夹具")
class NativeHostLibraryTests(unittest.TestCase):
    def run_fixture(self, target="x86_64-pc-windows-msvc", profile="debug", *, cargo_exit=0, runtime=True, external=False, fake_msvc=False, missing_linker=False):
        with tempfile.TemporaryDirectory(prefix="msime-native-host-") as directory:
            root = Path(directory)
            cargo_root = root / ("other-checkout/target" if external else "target")
            profile_dir = cargo_root / target / profile if target else cargo_root / profile
            profile_dir.mkdir(parents=True)
            library = profile_dir / "msime_host_api.dll.lib"
            library.write_bytes(b"synthetic-import")
            if runtime:
                (profile_dir / "msime_host_api.dll").write_bytes(b"synthetic-current-runtime")
            build = root / "target/native"
            staged = build / "Debug/msime_host_api.dll"
            staged.parent.mkdir(parents=True)
            staged.write_bytes(b"synthetic-old-runtime")
            cached_linker = root / "configured MSVC/link.exe"
            if not missing_linker:
                cached_linker.parent.mkdir()
                cached_linker.write_bytes(b"synthetic-linker")
            (build / "CMakeCache.txt").write_text("MSIME_HOST_LIBRARY:FILEPATH=" + library.as_posix() + "\nCMAKE_LINKER:FILEPATH=" + cached_linker.as_posix() + "\n", encoding="utf-8")
            commands = root / "bin"
            commands.mkdir()
            cargo = commands / "cargo"
            cargo.write_text('#!/usr/bin/env bash\nprintf "%s\\n" "$@" > cargo-args.txt\nprintf "fixture_linker=%s\\n" "$(command -v link.exe)"\nprintf "fixture_explicit_linker=%s\\n" "${CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER:-}"\nif [ "${MSIME_NATIVE_TEST_CARGO_EXIT:-0}" != 0 ]; then echo synthetic-cargo-failure >&2; fi\nexit "${MSIME_NATIVE_TEST_CARGO_EXIT:-0}"\n', encoding="utf-8", newline="\n")
            cargo.chmod(0o755)
            env = dict(os.environ, MSIME_NATIVE_TEST_CARGO_EXIT=str(cargo_exit))
            if fake_msvc:
                tools = root / "MSVC tools"
                msvc_linker = tools / "bin/Hostx64/x64/link.exe"
                msvc_linker.parent.mkdir(parents=True)
                for linker in (commands / "link.exe", msvc_linker):
                    linker.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8", newline="\n")
                    linker.chmod(0o755)
                env.update(VCToolsInstallDir=tools.as_posix(), VSCMD_ARG_HOST_ARCH="x64", VSCMD_ARG_TGT_ARCH="x64")
            script = 'set -uo pipefail\nexport PATH="$PWD/bin:$PATH"\nMSIME_NATIVE_BUILD=target/native\n' + HELPER + "\nbuild_windows_host_library\n"
            result = subprocess.run([BASH, "-s"], input=script, text=True, encoding="utf-8", cwd=root,
                env=env, capture_output=True, timeout=60 if os.name == "nt" else 20)
            arguments = (root / "cargo-args.txt").read_text().splitlines() if (root / "cargo-args.txt").exists() else None
            return result.returncode, arguments, staged.read_bytes(), result.stdout + result.stderr

    def test_explicit_msvc_target_and_debug_profile_match_the_import_library(self):
        code, arguments, staged, log = self.run_fixture()
        self.assertEqual(code, 0, log)
        self.assertEqual(arguments, ["build", "-p", "msime-host-api", "--locked", "--profile", "dev", "--target", "x86_64-pc-windows-msvc"])
        self.assertEqual(staged, b"synthetic-current-runtime")

    def test_default_target_does_not_add_an_explicit_target_directory(self):
        code, arguments, staged, log = self.run_fixture(target="")
        self.assertEqual(code, 0, log)
        self.assertNotIn("--target", arguments)
        self.assertEqual(staged, b"synthetic-current-runtime")

    def test_release_keeps_the_configured_profile(self):
        code, arguments, _, log = self.run_fixture(target="x86_64-pc-windows-gnu", profile="release")
        self.assertEqual(code, 0, log)
        self.assertEqual(arguments[arguments.index("--profile") + 1], "release")
        self.assertEqual(arguments[arguments.index("--target") + 1], "x86_64-pc-windows-gnu")

    def test_failed_cargo_does_not_stage_a_stale_runtime(self):
        code, _, staged, log = self.run_fixture(cargo_exit=7)
        self.assertNotEqual(code, 0)
        self.assertEqual(staged, b"synthetic-old-runtime")
        self.assertIn("synthetic-cargo-failure", log)

    def test_missing_runtime_does_not_stage_an_unrelated_default_build(self):
        code, _, staged, _ = self.run_fixture(runtime=False)
        self.assertNotEqual(code, 0)
        self.assertEqual(staged, b"synthetic-old-runtime")

    def test_another_checkout_is_not_overwritten(self):
        code, arguments, staged, _ = self.run_fixture(external=True)
        self.assertNotEqual(code, 0)
        self.assertIsNone(arguments)
        self.assertEqual(staged, b"synthetic-old-runtime")

    def test_msvc_linker_precedes_the_git_bash_link_utility(self):
        code, _, _, log = self.run_fixture(fake_msvc=True)
        self.assertEqual(code, 0, log)
        self.assertIn("/MSVC tools/bin/Hostx64/x64/link.exe", log)

    def test_cmake_linker_is_passed_explicitly_to_the_native_rust_target(self):
        code, _, _, log = self.run_fixture()
        self.assertEqual(code, 0, log)
        self.assertRegex(log, r"fixture_explicit_linker=.*configured MSVC[/\\]link.exe")

    def test_missing_configured_msvc_linker_does_not_fall_back_to_git(self):
        code, arguments, staged, log = self.run_fixture(missing_linker=True)
        self.assertNotEqual(code, 0, log)
        self.assertIsNone(arguments)
        self.assertEqual(staged, b"synthetic-old-runtime")


if __name__ == "__main__":
    if not BASH:
        print("skipped: 原生库构建夹具需要 Bash")
    else:
        unittest.main()

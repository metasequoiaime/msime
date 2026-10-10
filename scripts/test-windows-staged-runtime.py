#!/usr/bin/env python3
"""验证构建方暂存匹配的 MinGW 运行时，Wine 复用它而不再准备编译镜像。"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class StagedRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.windows = self.root / "platforms/windows"
        self.windows.mkdir(parents=True)
        for name in ("stage-runtime.sh", "run-tests-wine.sh"):
            shutil.copyfile(ROOT / "platforms/windows" / name, self.windows / name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.library = self.root / "toolchain"
        self.library.mkdir()
        self.log = self.root / "docker.jsonl"
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                        MSIME_TEST_LIBRARY=str(self.library),
                        MSIME_TEST_DOCKER_LOG=str(self.log))
        self.env.pop("MSIME_WINE_RESOURCES", None)
        self.env.pop("MSIME_MINGW_RUNTIME_DIR", None)
        self.write_command("synthetic-compiler", '''#!/usr/bin/env python3
import os, pathlib, sys
arg = sys.argv[1]
if arg.startswith("-print-file-name="):
    name = arg.split("=", 1)[1]
    path = pathlib.Path(os.environ["MSIME_TEST_LIBRARY"]) / name
    print(path if path.is_file() else name)
else:
    sys.exit(1)
''')
        self.write_command("synthetic-objdump", '''#!/usr/bin/env python3
import os, pathlib, sys
if sys.argv[1] == "-f":
    print("fixture: file format " + pathlib.Path(sys.argv[2]).read_text().strip())
elif sys.argv[1] == "-p":
    for dependency in os.environ.get("MSIME_TEST_IMPORTS", "").split():
        print("DLL Name: " + dependency)
else:
    sys.exit(1)
''')
        for prefix in ("i686", "x86_64"):
            (self.bin / f"{prefix}-w64-mingw32-g++").symlink_to("synthetic-compiler")
            (self.bin / f"{prefix}-w64-mingw32-objdump").symlink_to("synthetic-objdump")
        self.write_command("cargo", "#!/bin/sh\nexit 0\n")
        self.write_command("docker", '''#!/usr/bin/env python3
import json, os, pathlib, sys
call = sys.argv[1:]
entry = {"args": call}
if call[0] == "run":
    for arg in call:
        if arg.endswith(":/rt:ro"):
            folder = pathlib.Path(arg[:-len(":/rt:ro")])
            entry["runtime"] = {p.name: p.read_text() for p in folder.glob("*.dll")}
        if arg.endswith(":/repo"):
            entry["repo"] = arg[:-len(":/repo")]
with open(os.environ["MSIME_TEST_DOCKER_LOG"], "a") as log:
    log.write(json.dumps(entry) + "\\n")
if "--message-format=json" in " ".join(call):
    if os.environ.get("MSIME_TEST_CARGO_FAIL"):
        print("error: synthetic Cargo failure", file=sys.stderr)
        sys.exit(1)
    for target in ("msime_host_windows", "paste_policy", "msime_engine", "golden"):
        exe = pathlib.Path(entry["repo"]) / f"target/x86_64-pc-windows-gnu/debug/deps/{target}-0123456789abcdef.exe"
        exe.parent.mkdir(parents=True, exist_ok=True)
        exe.write_text("synthetic Rust test executable")
        print(json.dumps({"executable": "/repo/" + str(exe.relative_to(entry["repo"])),
                          "profile": {"test": True}, "target": {"name": target}}))
if "runtime" in entry:
    print("PASS windows-synthetic-runtime")
    rust_stage = next((pathlib.Path(arg[:-len(":/bin-rust:ro")]) for arg in call
                       if arg.endswith(":/bin-rust:ro")), None)
    if rust_stage is not None and (rust_stage / "rust-msime_host_windows.exe").is_file():
        print("PASS rust-msime_host_windows")
if call[:2] == ["info", "--format"]:
    print("linux/arm64")
''')

    def write_command(self, name, text):
        path = self.bin / name
        path.write_text(text, encoding="utf-8")
        path.chmod(0o755)

    def names(self, arch):
        return ("libstdc++-6.dll", "libwinpthread-1.dll",
                "libgcc_s_dw2-1.dll" if arch == "x86" else "libgcc_s_seh-1.dll")

    def prepare_library(self, arch):
        for name in self.names(arch):
            (self.library / name).write_text("pei-i386" if arch == "x86" else "pei-x86-64")

    def stage(self, arch):
        output = self.root / "target" / "windows-full" / arch
        return output, subprocess.run(
            ["bash", str(self.windows / "stage-runtime.sh"), arch, "--runtime-only", str(output)],
            env=self.env, capture_output=True, text=True)

    def test_runtime_only_stages_both_architectures_without_product_executables(self):
        for arch in ("x86", "x64"):
            with self.subTest(arch=arch):
                self.prepare_library(arch)
                output, result = self.stage(arch)
                self.assertEqual(result.returncode, 0, result.stderr)
                for name in self.names(arch):
                    self.assertEqual((output / name).read_bytes(), (self.library / name).read_bytes())
                self.assertFalse((output / "run-smoke.ps1").exists())

    def test_runtime_only_rejects_missing_dependency(self):
        self.prepare_library("x86")
        (self.library / "libwinpthread-1.dll").unlink()
        _, result = self.stage("x86")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing matching runtime", result.stderr)

    def test_runtime_only_rejects_wrong_pe_architecture(self):
        self.prepare_library("x86")
        (self.library / "libstdc++-6.dll").write_text("pei-x86-64")
        _, result = self.stage("x86")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Wrong runtime architecture", result.stderr)

    def test_default_mode_still_requires_full_build(self):
        self.prepare_library("x86")
        result = subprocess.run(["bash", str(self.windows / "stage-runtime.sh"), "x86"],
                                env=self.env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Run build-cross.sh first", result.stderr)

    def prepare_full_build(self):
        self.prepare_library("x86")
        build = self.root / "target/windows-full/x86"
        build.mkdir(parents=True)
        for name in (
            "windows-registration-inbox.exe", "windows-focus-router.exe", "windows-main-frame.exe",
            "windows-focus-gate.exe", "windows-input-queue.exe", "windows-session-smoke.exe",
            "windows-reply-codec.exe", "windows-reply-composer.exe", "windows-server-smoke.exe",
            "windows-preview-config.exe", "MetasequoiaImeServer.exe", "msime_host_api.dll",
            "tests/native-pipe/windows-pipe-io.exe",
        ):
            path = build / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("pei-i386")
        runner = self.windows / "tests/tools/run-smoke.ps1"
        runner.parent.mkdir(parents=True)
        runner.write_text("synthetic runner")
        return build

    def test_full_mode_accepts_system_dlls_and_still_rejects_unknown_dependencies(self):
        build = self.prepare_full_build()
        self.env["MSIME_TEST_IMPORTS"] = (
            "CRYPT32.dll WINHTTP.dll DWrite.dll combase.dll d2d1.dll d3d11.dll dcomp.dll "
            "dwmapi.dll mmdevapi.dll oleaut32.dll propsys.dll rpcrt4.dll"
        )
        result = subprocess.run(["bash", str(self.windows / "stage-runtime.sh"), "x86"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((build / "run-smoke.ps1").read_text(), "synthetic runner")
        self.env["MSIME_TEST_IMPORTS"] = "synthetic-unknown.dll"
        result = subprocess.run(["bash", str(self.windows / "stage-runtime.sh"), "x86"],
                                env=self.env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unclassified dependency: synthetic-unknown.dll", result.stderr)

    def test_wine_uses_staged_dlls_without_querying_host_or_cross_compiler(self):
        (self.bin / "x86_64-w64-mingw32-gcc").symlink_to("synthetic-compiler")
        (self.bin / "i686-w64-mingw32-gcc").symlink_to("synthetic-compiler")
        for arch in ("x86", "x64"):
            with self.subTest(arch=arch):
                self.log.unlink(missing_ok=True)
                build = self.root / "target/windows-full" / arch
                build.mkdir(parents=True)
                for name in self.names(arch):
                    (build / name).write_text(f"synthetic-{arch}-{name}")
                # 故意移走生产工具链：读取已有构建产物不应需要它。
                for name in self.library.glob("*.dll"):
                    name.unlink()
                result = subprocess.run(["bash", str(self.windows / "run-tests-wine.sh"), arch],
                                        env=self.env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = [json.loads(line) for line in self.log.read_text().splitlines()]
                builds = [call["args"] for call in calls if call["args"][0] == "build"]
                self.assertEqual(len(builds), 1, calls)
                self.assertTrue(builds[0][-1].endswith("/platforms/windows/wine"), calls)
                run = next((call for call in calls if "runtime" in call), None)
                self.assertIsNotNone(run, calls)
                self.assertEqual(run["runtime"], {name: f"synthetic-{arch}-{name}" for name in self.names(arch)})
                self.assertEqual(run["args"][run["args"].index("--platform") + 1], "linux/amd64")
                self.assertIn("PASS windows-synthetic-runtime", result.stdout)

    def test_wine_stages_rust_tests_with_cross_container_when_host_lacks_mingw(self):
        # 这条测的是主机没有 MinGW 的路径：装了 mingw-w64 的开发机（Homebrew 放在 /opt/homebrew/bin）会让脚本改用本机 cargo，所以把含有 MinGW 编译器的 PATH 目录挡在外面。
        self.env["PATH"] = os.pathsep.join(
            entry for entry in self.env["PATH"].split(os.pathsep)
            if not (Path(entry) / "x86_64-w64-mingw32-gcc").exists()
            or Path(entry) == self.bin
        )
        build = self.root / "target/windows-full/x64"
        build.mkdir(parents=True)
        for name in self.names("x64"):
            (build / name).write_text(name)
        result = subprocess.run(["bash", str(self.windows / "run-tests-wine.sh"), "x64"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        staged = self.root / "target/wine-rust-tests/x64/rust-msime_host_windows.exe"
        self.assertTrue(staged.is_file(), result.stdout)
        self.assertIn("PASS rust-msime_host_windows", result.stdout)
        self.assertTrue((staged.parent / "rust-paste_policy.exe").is_file())
        self.assertFalse((staged.parent / "rust-msime_engine.exe").exists())
        self.assertFalse((staged.parent / "rust-golden.exe").exists())
        calls = [json.loads(line)["args"] for line in self.log.read_text().splitlines()]
        cargo_run = next((args for args in calls if "--message-format=json" in " ".join(args)), None)
        self.assertIsNotNone(cargo_run, calls)
        self.assertIn("msime-cross:local-arm64", cargo_run)
        self.assertIn("CARGO_HOME=/repo/target/windows-cross/cargo-home", cargo_run)
        self.assertIn("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc", cargo_run)

    def test_wine_reports_rust_build_failure_alongside_native_results(self):
        build = self.root / "target/windows-full/x64"
        build.mkdir(parents=True)
        for name in self.names("x64"):
            (build / name).write_text(name)
        self.env["MSIME_TEST_CARGO_FAIL"] = "1"
        result = subprocess.run(["bash", str(self.windows / "run-tests-wine.sh"), "x64"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("FAIL rust-test-build", result.stdout)
        self.assertIn("PASS windows-synthetic-runtime", result.stdout)

    def test_wine_supplies_dictionary_fixtures_to_stroke_and_zhuyin_suites(self):
        build = self.root / "target/windows-full/x64"
        build.mkdir(parents=True)
        for name in self.names("x64"):
            (build / name).write_text(name)
        for name in ("windows-stroke-keys.exe", "windows-zhuyin-keys.exe"):
            (build / name).write_text("synthetic test executable")
        fixtures = self.windows / "tests/input/fixtures"
        fixtures.mkdir(parents=True)
        for name in ("msime-stroke.db", "msime-zhuyin.db"):
            (fixtures / name).write_text("synthetic dictionary")
        result = subprocess.run(["bash", str(self.windows / "run-tests-wine.sh"), "x64"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        wine = next(call["args"] for call in calls if "runtime" in call)
        self.assertIn(f"{fixtures}:/fixtures:ro", wine)
        command = "\n".join(wine)
        self.assertIn("msime-stroke.db", command)
        self.assertIn("msime-zhuyin.db", command)

    def test_wine_runs_tsf_subdirectory_tests_with_source_for_wiring_checks(self):
        build = self.root / "target/windows-full/x64"
        tsf_build = build / "tsf"
        tsf_build.mkdir(parents=True)
        for name in self.names("x64"):
            (build / name).write_text(name)
        for name in ("msime-tsf-paired-punctuation-wiring-test.exe",
                     "msime-tsf-smart-punctuation-focus-wiring-test.exe"):
            (tsf_build / name).write_text("synthetic test executable")
        for directory, name in (("registration_categories", "msime-tsf-category-registration-test.exe"),
                                ("registration_profiles", "msime-tsf-profile-registration-test.exe")):
            nested = tsf_build / "tests" / directory
            nested.mkdir(parents=True)
            (nested / name).write_text("synthetic test executable")
        source = self.windows / "tsf"
        source.mkdir()
        (tsf_build / "libMetasequoiaImeTsf.dll").write_text("synthetic TSF DLL")

        result = subprocess.run(["bash", str(self.windows / "run-tests-wine.sh"), "x64"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        wine = next(call["args"] for call in calls if "runtime" in call)
        command = "\n".join(wine)
        self.assertIn("/bin-win/tsf/msime-tsf-*.exe", command)
        self.assertIn("/bin-win/tsf/tests/registration_categories/msime-tsf-*.exe", command)
        self.assertIn("/bin-win/tsf/tests/registration_profiles/msime-tsf-*.exe", command)
        self.assertIn("cp /bin-win/tsf/*MetasequoiaImeTsf.dll /run/t/", command)
        self.assertIn(f"{source}:/tsf-source:ro", wine)
        self.assertIn("MSIME_TSF_SOURCE=Z:\\\\tsf-source", command)
        for name in ("msime-tsf-paired-punctuation-wiring-test",
                     "msime-tsf-smart-punctuation-focus-wiring-test"):
            self.assertIn(f'"$name" = {name} ] && argument="$MSIME_TSF_SOURCE"', command)


if __name__ == "__main__":
    if os.name != "posix" or not shutil.which("bash") or not shutil.which("cmake"):
        print("skipped: 运行时暂存回归需要 POSIX、bash 和 cmake")
    else:
        unittest.main()

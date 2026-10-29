#!/usr/bin/env python3
"""用生产脚本的 Rust 链接参数生成 Android ELF，验证符号归档与源码还原。"""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-tools", action="store_true")
    parser.add_argument("--target", choices=("aarch64-linux-android", "x86_64-linux-android"))
    arguments = parser.parse_args()
    sdk = Path(os.environ.get("ANDROID_SDK_ROOT", str(Path.home() / "Library/Android/sdk")))
    ndk = Path(os.environ.get("MSIME_ANDROID_NDK", str(sdk / "ndk/28.2.13676358")))
    tag = "darwin-x86_64" if sys.platform == "darwin" else "linux-x86_64"
    tools = ndk / "toolchains/llvm/prebuilt" / tag / "bin"
    prefix = str(tools / "llvm-")
    if not shutil.which("rustc") or not shutil.which("rustup") or not all(
        (tools / name).is_file() for name in ("llvm-readelf", "llvm-nm", "llvm-addr2line")
    ):
        if arguments.require_tools:
            raise SystemExit("Android 符号回归需要 Rust 与固定版本 NDK 工具链")
        print("skipped: Android 符号回归需要 Rust 与固定版本 NDK 工具链")
        return
    installed = subprocess.check_output(["rustup", "target", "list", "--installed"], text=True).splitlines()
    targets = [("aarch64-linux-android", "aarch64-linux-android"), ("x86_64-linux-android", "x86_64-linux-android")]
    if arguments.target:
        targets = [pair for pair in targets if pair[0] == arguments.target]
    targets = [(rust, clang) for rust, clang in targets if rust in installed and (tools / (clang + "28-clang")).is_file()]
    if not targets:
        if arguments.require_tools:
            raise SystemExit("没有已安装的 Android Rust target 或 NDK 编译器")
        print("skipped: 没有已安装的 Android Rust target")
        return
    build = (ROOT / "platforms/android/build-native.sh").read_text()
    flags = shlex.split(re.search(r'RUSTFLAGS="([^"\n]+)"', build)[1])
    with tempfile.TemporaryDirectory(prefix="msime-android-symbols-") as temporary:
        directory = Path(temporary)
        source = directory / "synthetic.rs"
        source.write_text(
            "#![no_std]\n"
            "#[unsafe(no_mangle)]\n"
            'pub extern "C" fn synthetic_crash_symbol(value: u32) -> u32 { value.wrapping_add(7) }\n'
            "#[panic_handler]\n"
            "fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }\n"
        )
        for rust, clang in targets:
            output = directory / rust
            output.mkdir()
            binary = output / "libmsime-synthetic.so"
            subprocess.run([
                "rustc", "--edition=2021", "--crate-type=cdylib", "--target", rust,
                "-C", "panic=abort", "-C", "debuginfo=line-tables-only",
                "-C", "linker=" + str(tools / (clang + "28-clang")),
                *flags, str(source), "-o", str(binary),
            ], check=True)
            archive = directory / (rust + ".zip")
            subprocess.run([
                sys.executable, str(ROOT / "scripts/crash-symbols.py"), "collect",
                "--platform", "android", "--version", "0.0.1", "--root", str(output),
                "--tool-prefix", prefix, "--output", str(archive),
            ], check=True, stdout=subprocess.DEVNULL)
            with zipfile.ZipFile(archive) as saved:
                [module] = json.loads(saved.read("manifest.json"))["modules"]
                assert len(module["build_id"]) == 40 and module["name"] == binary.name
            symbols = subprocess.check_output([prefix + "nm", str(binary)], text=True)
            offset = "0x" + next(line.split()[0] for line in symbols.splitlines() if line.endswith(" synthetic_crash_symbol"))
            resolved = subprocess.check_output([
                sys.executable, str(ROOT / "scripts/crash-symbols.py"), "symbolize",
                str(archive), binary.name, module["build_id"], offset, "--tool-prefix", prefix,
            ], text=True)
            assert "synthetic_crash_symbol" in resolved and "synthetic.rs:3" in resolved, resolved
            segments = subprocess.check_output([prefix + "readelf", "-l", "-W", str(binary)], text=True)
            loads = [line.split() for line in segments.splitlines() if line.lstrip().startswith("LOAD ")]
            assert loads and all(int(line[-1], 16) >= 16384 for line in loads), segments
            print(rust + ": Rust ELF build-id、16KB 对齐、归档及源码行还原通过")


if __name__ == "__main__":
    main()

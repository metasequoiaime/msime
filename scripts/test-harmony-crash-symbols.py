#!/usr/bin/env python3
"""用生产脚本的最终 Rust 链接参数验证鸿蒙 ELF 身份、符号归档与源码还原。"""
import argparse
import json
import os
from pathlib import Path
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
    parser.add_argument("--target", choices=("aarch64-unknown-linux-ohos", "armv7-unknown-linux-ohos", "x86_64-unknown-linux-ohos"))
    arguments = parser.parse_args()
    candidates = [Path.home() / "command-line-tools/sdk/default/openharmony/native",
                  Path("/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native")]
    ndk = Path(os.environ["MSIME_OHOS_NDK"]) if os.environ.get("MSIME_OHOS_NDK") else next(
        (path for path in candidates if (path / "llvm/bin/llvm-readelf").is_file()), candidates[0])
    tools = ndk / "llvm/bin"
    prefix = str(tools / "llvm-")
    if not shutil.which("rustc") or not shutil.which("rustup") or not all(
        (tools / name).is_file() for name in ("llvm-readelf", "llvm-nm", "llvm-addr2line")
    ):
        if arguments.require_tools:
            raise SystemExit("鸿蒙符号回归需要 Rust 与 OpenHarmony NDK")
        print("skipped: 鸿蒙符号回归需要 Rust 与 OpenHarmony NDK")
        return
    installed = subprocess.check_output(["rustup", "target", "list", "--installed"], text=True).splitlines()
    targets = [target for target in ("aarch64-unknown-linux-ohos", "armv7-unknown-linux-ohos",
                                   "x86_64-unknown-linux-ohos")
               if target in installed and (tools / (target + "-clang")).is_file()]
    if arguments.target:
        targets = [target for target in targets if target == arguments.target]
    if not targets:
        if arguments.require_tools:
            raise SystemExit("没有已安装的鸿蒙 Rust target 或 NDK 编译器")
        print("skipped: 没有已安装的鸿蒙 Rust target")
        return
    build = (ROOT / "platforms/harmony/build-native.sh").read_text()
    # 最终 crate 的参数与生产一致，不改变依赖指纹，也不放宽归档器的身份要求。
    invocation = build.split("cargo rustc ", 1)[1].replace("\\\n", " ").splitlines()[0]
    flags = shlex.split(invocation.split(" -- ", 1)[1])
    with tempfile.TemporaryDirectory(prefix="msime-harmony-symbols-") as temporary:
        directory = Path(temporary)
        source = directory / "synthetic.rs"
        source.write_text(
            "#![no_std]\n"
            "#[unsafe(no_mangle)]\n"
            'pub extern "C" fn synthetic_crash_symbol(value: u32) -> u32 { value.wrapping_add(7) }\n'
            "#[panic_handler]\n"
            "fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }\n"
        )
        for rust in targets:
            output = directory / rust
            output.mkdir()
            binary = output / "libmsime-synthetic.so"
            subprocess.run([
                "rustc", "--edition=2021", "--crate-type=cdylib", "--target", rust,
                "-C", "panic=abort", "-C", "debuginfo=line-tables-only",
                "-C", "linker=" + str(tools / (rust + "-clang")),
                *flags, str(source), "-o", str(binary),
            ], check=True)
            archive = directory / (rust + ".zip")
            subprocess.run([
                sys.executable, str(ROOT / "scripts/crash-symbols.py"), "collect",
                "--platform", "harmony", "--version", "0.0.1", "--root", str(output),
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
            print(rust + ": Rust ELF build-id、符号归档及源码行还原通过")


if __name__ == "__main__":
    main()

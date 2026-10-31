#!/usr/bin/env python3
"""用合成 ELF 核对符号还原、构建拒配与归档完整性。"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts/crash-symbols.py"


def main():
    if not all(shutil.which(tool) for tool in ("g++", "readelf", "addr2line", "nm")) or sys.platform != "linux":
        print("skipped: ELF 符号回归需要 Linux 和 g++、binutils")
        return
    with tempfile.TemporaryDirectory(prefix="msime-crash-symbols-") as temporary:
        root = Path(temporary)
        source = root / "synthetic.cpp"
        source.write_text("int synthetic_symbol(int x) { return x + 7; }\nint main() { return synthetic_symbol(2); }\n")
        binary = root / "msime-synthetic"
        subprocess.run(["g++", "-g", "-Wl,--build-id=sha1", str(source), "-o", str(binary)], check=True)
        archive = root / "symbols.zip"
        subprocess.run([sys.executable, str(SCRIPT), "collect", "--platform", "linux", "--version", "0.0.1", "--root", str(root), "--output", str(archive)], check=True)
        with zipfile.ZipFile(archive) as file:
            manifest = json.loads(file.read("manifest.json"))
            module = manifest["modules"][0]
            payload = file.read(module["file"])
        assert hashlib.sha256(payload).hexdigest() == module["sha256"]
        symbols = subprocess.check_output(["nm", str(binary)], text=True)
        offset = "0x" + next(line.split()[0] for line in symbols.splitlines() if "synthetic_symbol" in line)
        command = [sys.executable, str(SCRIPT), "symbolize", str(archive), binary.name, module["build_id"], offset]
        result = subprocess.check_output(command, text=True)
        assert "synthetic_symbol(int)" in result and "synthetic.cpp:1" in result
        executable = command.copy()
        executable[-3] = "<executable>"
        assert subprocess.check_output(executable, text=True) == result
        wrong = command.copy()
        wrong[-2] = "00"
        result = subprocess.run(wrong, capture_output=True, text=True)
        assert result.returncode != 0 and "未唯一命中" in result.stderr
        duplicate = dict(module, name="msime-synthetic-other")
        manifest["modules"].append(duplicate)
        with zipfile.ZipFile(archive, "w") as file:
            file.writestr("manifest.json", json.dumps(manifest))
            file.writestr(module["file"], payload)
        result = subprocess.run(executable, capture_output=True, text=True)
        assert result.returncode != 0 and "未唯一命中" in result.stderr
        manifest["modules"].pop()
        module["sha256"] = "0" * 64
        with zipfile.ZipFile(archive, "w") as file:
            file.writestr("manifest.json", json.dumps(manifest))
            file.writestr(module["file"], payload)
        result = subprocess.run(command, capture_output=True, text=True)
        assert result.returncode != 0 and "SHA-256 不匹配" in result.stderr
    print("crash symbols: 合成源码还原、错误构建拒配和摘要校验通过")


if __name__ == "__main__":
    main()

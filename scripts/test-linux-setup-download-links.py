#!/usr/bin/env python3
"""运行真实安装器导出手动下载清单，确认只读、分版本与原有校验边界。"""
from __future__ import annotations

import hashlib
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

if os.name != "posix":
    print("skipped: Linux 安装器检查需要 POSIX 的 fcntl 模块")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent.parent
SOURCE = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "platforms/linux/scripts/msime-linux-setup"


def load(path: Path, name: str):
    spec = importlib.util.spec_from_loader(name, importlib.machinery.SourceFileLoader(name, str(path)))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    setup = load(SOURCE, "setup_download_links")
    editions = load(ROOT / "platforms/linux/scripts/edition_linux.py", "download_link_editions")
    payloads = {"synthetic.db": b"synthetic dictionary", "COPYING.txt": b"synthetic license"}
    lock = {"artifacts": [
        {"name": name, "url": "https://example.invalid/locked/" + name,
         "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        for name, data in payloads.items()
    ]}
    with tempfile.TemporaryDirectory(prefix="msime-setup-links-") as temporary:
        root = Path(temporary)
        guard = root / "guard"
        guard.mkdir()
        (guard / "sitecustomize.py").write_text(
            "import urllib.request, subprocess\n"
            "def blocked(*args, **kwargs):\n"
            "    raise AssertionError('listing must not download or launch setup')\n"
            "urllib.request.urlopen = blocked\nsubprocess.Popen = blocked\n"
        )
        for edition in ("full", "wubi"):
            identity = editions.identity(editions.edition_entry(edition))
            prefix = root / edition
            program = prefix / "bin" / (identity["package"] + "-setup")
            program.parent.mkdir(parents=True)
            program.write_text(editions.render_text("scripts/msime-linux-setup", SOURCE.read_text(), identity))
            share = prefix / "share" / identity["client_directory"]
            share.mkdir(parents=True)
            lock_file = share / "desktop-dictionary.lock.json"
            lock_file.write_text(json.dumps(lock))
            config = root / (edition + "-config")
            state = config / identity["client_directory"]
            state.mkdir(parents=True)
            (state / "occupied.txt").write_text("synthetic existing state")
            data = root / (edition + "-data")
            env = {**os.environ, "XDG_CONFIG_HOME": str(config), "XDG_DATA_HOME": str(data),
                   "XDG_DATA_DIRS": str(root / "no-other-lock"), "MSIME_DICTIONARY_LOCK": str(lock_file),
                   "PYTHONPATH": str(guard), "PYTHONDONTWRITEBYTECODE": "1"}

            def run(*args, mirror="https://mirror.example.invalid/"):
                return subprocess.run([sys.executable, str(program), *args],
                                      env={**env, "MSIME_DOWNLOAD_MIRROR": mirror},
                                      text=True, capture_output=True, timeout=15)

            before = sorted(str(path.relative_to(root)) for path in root.rglob("*"))
            for mirror in ("https://mirror.example.invalid/", ""):
                result = run("--list-downloads", "--json", mirror=mirror)
                assert result.returncode == 0, result.stderr
                manifest = json.loads(result.stdout)
                assert manifest["directory"] == str(data / identity["client_directory"] / "resources"), manifest
                for actual, expected in zip(manifest["artifacts"], lock["artifacts"], strict=True):
                    assert {key: actual[key] for key in expected} == expected, actual
                    assert actual["mirrorUrl"] == (mirror + expected["url"] if mirror else None), actual
            readable = run("--list-downloads")
            assert readable.returncode == 0 and "--resources" in readable.stdout, readable
            for artifact in lock["artifacts"]:
                assert artifact["url"] in readable.stdout and artifact["sha256"] in readable.stdout
            for args in (("--json",), ("--list-downloads", "--download"),
                         ("--list-downloads", "--register"), ("--list-downloads", "--unregister"),
                         ("--list-downloads", "--update")):
                assert run(*args).returncode == 2, args
            assert sorted(str(path.relative_to(root)) for path in root.rglob("*")) == before
            assert not data.exists()

            missing_url = {"artifacts": [{key: value for key, value in lock["artifacts"][0].items() if key != "url"}]}
            lock_file.write_text(json.dumps(missing_url))
            result = run("--list-downloads", "--json")
            assert result.returncode == 0, result.stderr
            missing = json.loads(result.stdout)["artifacts"][0]
            assert missing["url"] is None and missing["mirrorUrl"] is None
            assert "未提供下载地址" in run("--list-downloads").stdout
            lock_file.unlink()
            result = run("--list-downloads", "--json")
            assert result.returncode != 0 and "找不到 desktop-dictionary.lock.json" in result.stderr, result

        # 外部工具保存的文件仍由原安装器核对；大小相同的坏内容也不能通过。
        resources = root / "manual-files"
        resources.mkdir()
        for name, data in payloads.items():
            (resources / name).write_bytes(data)
        assert setup.verify_directory(resources, lock) == []
        (resources / "synthetic.db").write_bytes(b"x" * len(payloads["synthetic.db"]))
        assert "SHA-256" in " ".join(setup.verify_directory(resources, lock))
    print("Linux manual download links: read-only CLI, editions, mirrors and checksum rejection passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

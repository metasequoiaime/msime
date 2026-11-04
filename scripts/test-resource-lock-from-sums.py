#!/usr/bin/env python3
"""检查 scripts/resource_lock_from_sums.py：临时锁保持模板的文件清单和顺序，换上 artifact 的 SHA-256 和长度，模型可去可留，文件与 SUMS 不符时拒绝，staged 目录恰好只含锁里的文件。

用合成的 artifact，不需要真实词库。
"""
import errno
import hashlib
import importlib.util
import json
import os
import stat
import tempfile
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("lock_from_sums", ROOT / "scripts/resource_lock_from_sums.py")
lock_from_sums = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lock_from_sums)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def expect_exit(call, message):
    try:
        call()
    except SystemExit as error:
        assert message in str(error), (message, str(error))
    else:
        raise AssertionError(f"expected a failure mentioning {message!r}")


with tempfile.TemporaryDirectory(prefix="msime-lock-from-sums-") as temporary:
    root = Path(temporary)
    artifact = root / "artifact"
    artifact.mkdir()
    files = {"msime-pinyin.db": b"pinyin", "msime-english.db": b"english", "msime-cantonese.db": b"cantonese"}
    commit = "a" * 40
    files["msime-dictionary-manifest.json"] = json.dumps({"source": {"commit": commit}}).encode()
    for name, data in files.items():
        (artifact / name).write_bytes(data)
    (artifact / "msime-SHA256SUMS.txt").write_text("".join(f"{digest(data)}  {name}\n" for name, data in files.items() if name != "msime-cantonese.db") + f"{digest(b'cantonese')} *msime-cantonese.db\n", encoding="utf-8")
    model = root / "sentence-model.safetensors"
    model.write_bytes(b"model")
    base = {"source_commit": "b" * 40, "artifacts": [
        {"name": "msime-pinyin.db", "url": "https://example.invalid/old/msime-pinyin.db", "sha256": "0" * 64, "size": 1},
        {"name": "msime-english.db", "url": "https://example.invalid/old/msime-english.db", "sha256": "0" * 64, "size": 1},
        {"name": "msime-dictionary-manifest.json", "url": "https://example.invalid/old/msime-dictionary-manifest.json", "sha256": "0" * 64, "size": 1},
        {"name": "sentence-model.safetensors", "url": "https://example.invalid/model/sentence-model.safetensors", "sha256": digest(b"model"), "size": 5},
    ]}

    lock, extra = lock_from_sums.build_lock(artifact, base, "https://dry-run.invalid", False, None)
    assert lock["source_commit"] == commit, "source_commit comes from the artifact's manifest"
    assert [entry["name"] for entry in lock["artifacts"]] == [entry["name"] for entry in base["artifacts"]], "keeps the template's list and order"
    pinyin = lock["artifacts"][0]
    assert pinyin == {"name": "msime-pinyin.db", "url": "https://dry-run.invalid/msime-pinyin.db", "sha256": digest(b"pinyin"), "size": 6}
    assert lock["artifacts"][3] == base["artifacts"][3], "the model entry is kept as it is"
    assert extra == ["msime-cantonese.db"]

    without, _ = lock_from_sums.build_lock(artifact, base, "https://dry-run.invalid/", True, None)
    assert [entry["name"] for entry in without["artifacts"]] == ["msime-pinyin.db", "msime-english.db", "msime-dictionary-manifest.json"]

    # 校验：URL 必须 https；文件与 SUMS 不符、模板里的文件不在 SUMS 里、提交号不是 40 位都拒绝。
    expect_exit(lambda: lock_from_sums.build_lock(artifact, base, "http://dry-run.invalid/", False, None), "https://")
    expect_exit(lambda: lock_from_sums.build_lock(artifact, base, "https://dry-run.invalid/", False, "abc"), "40-digit")
    missing = json.loads(json.dumps(base))
    missing["artifacts"].insert(0, {"name": "msime-wubi.db", "url": "https://x/msime-wubi.db", "sha256": "0" * 64, "size": 1})
    expect_exit(lambda: lock_from_sums.build_lock(artifact, missing, "https://dry-run.invalid/", False, None), "not in msime-SHA256SUMS.txt")
    (artifact / "msime-english.db").write_bytes(b"tampered")
    expect_exit(lambda: lock_from_sums.build_lock(artifact, base, "https://dry-run.invalid/", False, None), "msime-SHA256SUMS.txt says")
    (artifact / "msime-english.db").write_bytes(b"english")
    bad_sums = root / "bad.txt"
    bad_sums.write_text("nothex  msime-pinyin.db\n", encoding="utf-8")
    expect_exit(lambda: lock_from_sums.parse_sums(bad_sums), "not a sha256sum line")
    bad_sums.write_text(f"{'0' * 64}  dir/msime-pinyin.db\n", encoding="utf-8")
    expect_exit(lambda: lock_from_sums.parse_sums(bad_sums), "not a sha256sum line")

    # staged 目录恰好只含锁里的文件，模型从 --model 取并核对。
    staged = root / "staged"
    sources = [artifact / entry["name"] if entry["name"] != lock_from_sums.MODEL else model for entry in lock["artifacts"]]
    for source in sources:
        source.chmod(0o644)
    source_modes = {source: stat.S_IMODE(source.stat().st_mode) for source in sources}
    lock_from_sums.stage(lock, artifact, model, staged)
    assert all(stat.S_IMODE(source.stat().st_mode) == source_modes[source] for source in sources), "硬链接暂存不得改变词库或模型源文件的权限"
    assert sorted(path.name for path in staged.iterdir()) == sorted(entry["name"] for entry in lock["artifacts"])
    for entry in lock["artifacts"]:
        data = (staged / entry["name"]).read_bytes()
        assert digest(data) == entry["sha256"] and len(data) == entry["size"]
    # 跨盘回退仍复制真实内容，只收紧副本的权限。
    copied = root / "copied"
    with patch.object(lock_from_sums.os, "link", side_effect=OSError(errno.EXDEV, "合成跨盘错误")):
        lock_from_sums.stage(lock, artifact, model, copied)
    for entry, source in zip(lock["artifacts"], sources):
        target = copied / entry["name"]
        assert target.read_bytes() == source.read_bytes()
        if os.name != "nt":
            assert stat.S_IMODE(target.stat().st_mode) == 0o600
        assert stat.S_IMODE(source.stat().st_mode) == source_modes[source]
    expect_exit(lambda: lock_from_sums.stage(lock, artifact, model, staged), "not empty")
    expect_exit(lambda: lock_from_sums.stage(lock, artifact, None, root / "staged-no-model"), "--model")
    wrong_model = root / "wrong.safetensors"
    wrong_model.write_bytes(b"other")
    expect_exit(lambda: lock_from_sums.stage(lock, artifact, wrong_model, root / "staged-wrong"), "does not match")

    # 命令行写出的锁是合法 JSON，与提交的锁同样缩进两格。
    out, base_lock = root / "lock.json", root / "base.json"
    base_lock.write_text(json.dumps(base), encoding="utf-8")
    assert lock_from_sums.main(["--artifact", str(artifact), "--out", str(out), "--without-model", "--base-lock", str(base_lock)]) == 0
    assert json.loads(out.read_text(encoding="utf-8")) == without
    assert out.read_text(encoding="utf-8").startswith('{\n  "source_commit"')

# 真实模板：提交的桌面锁里除了模型，每个文件都是 dict-v 发布的 msime-SHA256SUMS.txt 会列出的（manifest 也在里面）。
desktop = json.loads((ROOT / "resources/desktop-dictionary.lock.json").read_text(encoding="utf-8"))
assert any(entry["name"] == lock_from_sums.MODEL for entry in desktop["artifacts"])
assert any(entry["name"] == lock_from_sums.MANIFEST for entry in desktop["artifacts"])

print("resource lock from sums: ok")

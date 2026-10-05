#!/usr/bin/env python3
"""msime-linux-setup --update：升级之后只取回过期的那几项词库，再让宿主库切换代次。

词库从本机的一个 HTTP 桩取，记下被请求的路径；msime-linux-prepare 换成桩，--refresh 时把配置指向新代次，记下调用以及那一刻输入会话是否已被请走（租约有效、会话锁被独占）。不联网，不需要真实词库。给出已构建的 msime-linux-prepare 时，另外核对它的 --refresh 在词库过期时以 3 退出且不改写配置，这是 setup 与两个宿主区分「词库过期」的依据。
"""
from __future__ import annotations

import fcntl
import hashlib
import http.server
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/msime-linux-setup"

# Stands in for msime-linux-prepare. --refresh points the options at a new generation, the observable effect of the real command, unless STUB_REFRESH_EXIT asks for a failure. It also records what the hosts would see at that moment: whether the quiesce lease is live and whether the session lock is held exclusively, and which resource directory it was asked to prepare.
# 首行用跑测试的同一个解释器，不经 /usr/bin/env：没有 FHS 布局的环境（Nix 构建沙箱）里没有它。
PREPARE_STUB = f"#!{sys.executable}\n" + r'''import fcntl, json, os, sys, time
from pathlib import Path

with open(os.environ["STUB_LOG"], "a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
if sys.argv[1:2] == ["--refresh"]:
    document = json.loads(Path(sys.argv[2]).read_text())
    user = Path(document["user_data"])
    try:
        remaining = int((user / ".msime-dictionary-quiesce").read_text()) / 1000 - time.time()
    except (OSError, ValueError):
        remaining = 0
    descriptor = os.open(user / ".msime-dictionary-access.lock", os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_SH | fcntl.LOCK_NB)
        locked = False
    except BlockingIOError:
        locked = True
    os.close(descriptor)
    with open(os.environ["STUB_LOG"] + ".refresh", "a") as log:
        log.write(json.dumps({"lease": 0 < remaining <= 30, "locked": locked, "resources": document["resources"]}) + "\n")
    code = int(os.environ.get("STUB_REFRESH_EXIT", "0"))
    if code:
        print("The recorded dictionaries do not match this version; runtime options were left unchanged", file=sys.stderr)
        sys.exit(code)
    options = Path(sys.argv[2])
    document = json.loads(options.read_text())
    document["dictionaries"] = str(Path(document["user_data"]) / "dictionaries/next")
    options.write_text(json.dumps(document))
    print("refreshed")
    sys.exit(0)
Path(sys.argv[-1]).mkdir(parents=True)
'''


class Artifacts(http.server.BaseHTTPRequestHandler):
    payloads: dict = {}
    requested: list = []

    def do_GET(self):
        Artifacts.requested.append(self.path)
        payload = Artifacts.payloads.get(self.path)
        if payload is None:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *arguments):
        pass


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


class Harness:
    def __init__(self, scratch: Path, port: int):
        self.scratch = scratch
        self.prefix = scratch / "prefix"
        (self.prefix / "bin").mkdir(parents=True)
        self.setup = self.prefix / "bin/msime-linux-setup"
        self.setup.write_text(SCRIPT.read_text())
        self.setup.chmod(0o755)
        prepare = self.prefix / "bin/msime-linux-prepare"
        prepare.write_text(PREPARE_STUB)
        prepare.chmod(0o755)
        self.current = {"a.db": b"dictionary a, unchanged", "b.db": b"dictionary b, next release"}
        self.previous_b = b"dictionary b, previous release"
        self.lock = scratch / "desktop-dictionary.lock.json"
        self.lock.write_text(json.dumps({"artifacts": [
            {"name": name, "size": len(payload), "sha256": sha256(payload), "url": f"http://127.0.0.1:{port}/{name}"}
            for name, payload in self.current.items()
        ]}))
        self.log = scratch / "prepare.log"
        self.environment = {
            key: value for key, value in os.environ.items()
            if not key.startswith(("MSIME_", "XDG_")) and "proxy" not in key.lower()
        }
        self.environment.update(
            HOME=str(scratch / "home"),
            XDG_CONFIG_HOME=str(scratch / "config"),
            XDG_DATA_HOME=str(scratch / "data"),
            MSIME_DICTIONARY_LOCK=str(self.lock),
            STUB_LOG=str(self.log),
        )

    def installed(self, name: str, resources: Path | None = None) -> Path:
        """A state directory prepared by an earlier release: its downloaded b.db predates the installed lock."""
        state = self.scratch / name
        # Each fixture gets its own parent: an update stages the new dictionaries beside the recorded directory.
        resources = resources or self.scratch / f"{name}-data/resources"
        resources.mkdir(parents=True)
        (resources / "a.db").write_bytes(self.current["a.db"])
        (resources / "b.db").write_bytes(self.previous_b)
        (state / "user/dictionaries/previous").mkdir(parents=True)
        (state / "runtime-options.json").write_text(json.dumps({
            "api_version": 1,
            "resources": str(resources),
            "user_data": str(state / "user"),
            "cache": str(state / "cache"),
            "dictionaries": str(state / "user/dictionaries/previous"),
            "preferences_directory": str(state),
            "preferences": {},
        }))
        return state

    def run(self, *arguments: str, refresh_exit: int = 0, **overrides: str) -> subprocess.CompletedProcess:
        Artifacts.requested.clear()
        self.log.write_text("")
        Path(f"{self.log}.refresh").write_text("")
        environment = dict(self.environment, STUB_REFRESH_EXIT=str(refresh_exit), **overrides)
        return subprocess.run(
            [sys.executable, str(self.setup), *arguments],
            env=environment, capture_output=True, text=True, timeout=60,
        )

    def prepare_calls(self) -> list:
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def refreshes(self) -> list:
        return [json.loads(line) for line in Path(f"{self.log}.refresh").read_text().splitlines()]

    def staged(self, resources: Path) -> Path:
        """Where an update stages the dictionaries of the installed lock, beside the recorded directory."""
        lock = json.loads(self.lock.read_text())
        tag = hashlib.sha256(
            "\n".join(f"{artifact['name']}:{artifact['sha256']}" for artifact in lock["artifacts"]).encode()
        ).hexdigest()[:16]
        return resources.parent / f"resources-{tag}"


def options(state: Path) -> dict:
    return json.loads((state / "runtime-options.json").read_text())


def leftovers(state: Path) -> list[str]:
    """What an update may leave in the state or user directory besides the options: a staged options copy or the quiesce lease would be a bug."""
    names = [path.name for path in state.iterdir()] + [path.name for path in (state / "user").iterdir()]
    return sorted(name for name in names if name.startswith((".runtime-options-", ".msime-dictionary-quiesce")))


def check_setup(harness: Harness) -> None:
    # --download over an existing state is an update: only the stale artifact is fetched, into a new directory beside the one in use, then the host library switches the generation with the input sessions closed.
    state = harness.installed("state-download")
    resources = Path(options(state)["resources"])
    staged = harness.staged(resources)
    result = harness.run("--state", str(state), "--download")
    assert result.returncode == 0, result
    assert Artifacts.requested == ["/b.db"], Artifacts.requested
    # The directory the previous generation reads is never written: it stays exactly as it was, for the sessions still on it and for rollback.
    assert (resources / "b.db").read_bytes() == harness.previous_b
    assert (staged / "b.db").read_bytes() == harness.current["b.db"]
    assert (staged / "a.db").stat().st_ino == (resources / "a.db").stat().st_ino, "an artifact that already matched is linked, not fetched"
    calls = harness.prepare_calls()
    assert len(calls) == 1 and calls[0][0] == "--refresh", calls
    assert Path(calls[0][1]).parent == state and Path(calls[0][1]).name.startswith(".runtime-options-update-"), calls
    assert harness.refreshes() == [{"lease": True, "locked": True, "resources": str(staged)}], harness.refreshes()
    assert options(state)["resources"] == str(staged), options(state)
    assert options(state)["dictionaries"] == str(state / "user/dictionaries/next"), options(state)
    assert leftovers(state) == [], leftovers(state)
    assert "已切换到新版本的词库" in result.stdout and str(resources) in result.stdout, result.stdout

    # Explicit --update behaves the same; once current, nothing is fetched and the refresh still runs on the file itself, with the sessions closed.
    result = harness.run("--update", "--download", "--state", str(state))
    assert result.returncode == 0, result
    assert Artifacts.requested == [], Artifacts.requested
    assert harness.prepare_calls() == [["--refresh", str(state / "runtime-options.json")]]
    assert harness.refreshes() == [{"lease": True, "locked": True, "resources": str(staged)}], harness.refreshes()
    assert leftovers(state) == [], leftovers(state)

    # An input session that does not let go keeps the switch from happening: nothing is refreshed and the lease comes down again. The staged dictionaries stay for the next attempt.
    state = harness.installed("state-busy")
    before = (state / "runtime-options.json").read_bytes()
    descriptor = os.open(state / "user/.msime-dictionary-access.lock", os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_SH)
        result = harness.run("--update", "--download", "--state", str(state))
    finally:
        os.close(descriptor)
    assert result.returncode == 1, result
    assert "占用词库" in result.stderr, result.stderr
    assert harness.prepare_calls() == [], harness.prepare_calls()
    assert (state / "runtime-options.json").read_bytes() == before
    assert leftovers(state) == [], leftovers(state)
    assert (harness.staged(Path(options(state)["resources"])) / "b.db").read_bytes() == harness.current["b.db"]

    # A replaced access lock must not redirect the update's exclusive lock to an
    # unrelated file. The host and setup script share this lock, so accepting a
    # symlink here would let a hostile state directory make the refresh wait on
    # or lock an external inode.
    state = harness.installed("state-linked-access-lock")
    access_lock = state / "user/.msime-dictionary-access.lock"
    outside_lock = harness.scratch / "outside-access.lock"
    outside_lock.write_bytes(b"keep")
    access_lock.symlink_to(outside_lock)
    result = harness.run("--update", "--download", "--state", str(state))
    assert result.returncode == 1, result
    assert "切换词库失败" in result.stderr, result.stderr
    assert outside_lock.read_bytes() == b"keep"
    assert harness.prepare_calls() == []

    # Without --download an outdated dictionary is reported and left alone, and nothing switches.
    state = harness.installed("state-no-download")
    before = (state / "runtime-options.json").read_bytes()
    result = harness.run("--update", "--state", str(state))
    assert result.returncode == 1, result
    assert "b.db" in result.stderr and "--download" in result.stderr, result.stderr
    assert Artifacts.requested == [] and harness.prepare_calls() == []
    assert (state / "runtime-options.json").read_bytes() == before

    # A failed download publishes nothing, not even the artifacts that arrived before it: the previous generation keeps reading the dictionaries it was built from.
    state = harness.installed("state-offline")
    resources = Path(options(state)["resources"])
    (resources / "a.db").write_bytes(b"dictionary a, previous release")
    before = (state / "runtime-options.json").read_bytes()
    served = Artifacts.payloads.pop("/b.db")
    result = harness.run("--update", "--download", "--state", str(state))
    Artifacts.payloads["/b.db"] = served
    assert result.returncode != 0, result
    assert "词库目录未改动" in result.stderr, result.stderr
    assert Artifacts.requested == ["/a.db", "/b.db"], Artifacts.requested
    assert (resources / "a.db").read_bytes() == b"dictionary a, previous release"
    assert (resources / "b.db").read_bytes() == harness.previous_b
    assert sorted(path.name for path in resources.iterdir()) == ["a.db", "b.db"]
    assert harness.prepare_calls() == []
    assert (state / "runtime-options.json").read_bytes() == before

    # A file the lock no longer names would make the host reject the whole directory. Without --download it is reported; the update stages a directory holding only the lock's entries, so the file stays behind with the previous generation.
    state = harness.installed("state-extra")
    resources = Path(options(state)["resources"])
    (resources / "retired.db").write_bytes(b"dropped by the new lock")
    result = harness.run("--update", "--state", str(state))
    assert result.returncode == 1 and "retired.db" in result.stderr, result
    result = harness.run("--update", "--download", "--state", str(state))
    assert result.returncode == 0, result
    assert Artifacts.requested == ["/b.db"], Artifacts.requested
    assert sorted(path.name for path in harness.staged(resources).iterdir()) == ["a.db", "b.db"]
    assert (resources / "retired.db").is_file() and (resources / "b.db").read_bytes() == harness.previous_b

    # A refresh that fails leaves the options naming the previous directory, which was never written, and says where the new dictionaries wait.
    state = harness.installed("state-refresh-failed")
    resources = Path(options(state)["resources"])
    before = (state / "runtime-options.json").read_bytes()
    result = harness.run("--update", "--download", "--state", str(state), refresh_exit=1)
    assert result.returncode == 1, result
    assert "未改动" in result.stderr and str(harness.staged(resources)) in result.stderr, result.stderr
    assert (state / "runtime-options.json").read_bytes() == before
    assert (resources / "b.db").read_bytes() == harness.previous_b
    assert leftovers(state) == [], leftovers(state)

    # A prefix such as ~/.local puts the packaged location on the download directory of a fresh setup: those dictionaries are the user's own, and an update brings them up to date.
    state = harness.installed("state-local-prefix", harness.prefix / "share/msime-client/resources")
    result = harness.run("--update", "--download", "--state", str(state), XDG_DATA_HOME=str(harness.prefix / "share"))
    assert result.returncode == 0, result
    assert options(state)["resources"] == str(harness.staged(harness.prefix / "share/msime-client/resources")), options(state)
    shutil.rmtree(harness.prefix / "share/msime-client")

    # Dictionaries shipped with the package belong to the package manager, even under a prefix the user can write.
    state = harness.installed("state-packaged", harness.prefix / "share/msime-client/resources")
    before = (state / "runtime-options.json").read_bytes()
    result = harness.run("--update", "--download", "--state", str(state))
    assert result.returncode == 1, result
    assert "包管理器" in result.stderr, result.stderr
    assert Artifacts.requested == [] and harness.prepare_calls() == []
    assert (harness.prefix / "share/msime-client/resources/b.db").read_bytes() == harness.previous_b
    assert (state / "runtime-options.json").read_bytes() == before

    # The host library disagreeing with the installed lock is a broken install; the options stay as they were.
    state = harness.installed("state-mismatched-install")
    before = (state / "runtime-options.json").read_bytes()
    result = harness.run("--update", "--download", "--state", str(state), refresh_exit=3)
    assert result.returncode == 1, result
    assert "重新安装" in result.stderr and "do not match this version" in result.stderr, result.stderr
    assert (state / "runtime-options.json").read_bytes() == before
    assert leftovers(state) == [], leftovers(state)

    # A fresh setup still refuses an existing directory and points at --update instead.
    state = harness.installed("state-fresh")
    resources = Path(options(state)["resources"])
    (resources / "b.db").write_bytes(harness.current["b.db"])
    result = harness.run("--resources", str(resources), "--state", str(state))
    assert result.returncode != 0, result
    assert "状态目录已存在" in result.stderr and "--update --download" in result.stderr, result.stderr
    assert harness.prepare_calls() == []

    # --update reads the directory the state recorded, so another one cannot be named, and it needs a prepared state.
    result = harness.run("--update", "--resources", str(resources), "--state", str(state))
    assert result.returncode != 0 and "--resources" in result.stderr, result
    empty = harness.scratch / "state-empty"
    empty.mkdir()
    result = harness.run("--update", "--state", str(empty))
    assert result.returncode != 0 and "runtime-options.json" in result.stderr, result
    assert harness.prepare_calls() == []

    # The hosts and refresh API cap runtime-options.json at 16 KiB. The installer must reject a
    # damaged or hostile oversized document before loading it, rather than handing an unbounded
    # file to json.loads or attempting a dictionary update from its contents.
    state = harness.installed("state-oversized-options")
    options_file = state / "runtime-options.json"
    options_file.write_bytes(b"{" + b"x" * (16 * 1024) + b"}")
    result = harness.run("--update", "--state", str(state))
    assert result.returncode != 0 and "runtime-options.json" in result.stderr, result
    assert "限制" in result.stderr and harness.prepare_calls() == [], result


def check_prepare(prepare: Path, harness: Harness) -> None:
    """The built msime-linux-prepare against the lock it was compiled with: the fixture dictionaries match none of it."""
    result = subprocess.run([str(prepare), "--refresh", "runtime-options.json"], capture_output=True, text=True, timeout=30)
    assert result.returncode == 2, result
    state = harness.installed("state-real")
    before = (state / "runtime-options.json").read_bytes()
    result = subprocess.run(
        [str(prepare), "--refresh", str(state / "runtime-options.json")], capture_output=True, text=True, timeout=60
    )
    assert result.returncode == 3, result
    assert "do not match this version" in result.stderr, result.stderr
    assert (state / "runtime-options.json").read_bytes() == before
    assert sorted(path.name for path in state.iterdir()) == ["runtime-options.json", "user"]


def main() -> int:
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Artifacts)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory() as name:
            harness = Harness(Path(name), server.server_address[1])
            Artifacts.payloads = {f"/{key}": value for key, value in harness.current.items()}
            check_setup(harness)
            if len(sys.argv) > 1:
                check_prepare(Path(sys.argv[1]), harness)
    finally:
        server.shutdown()
        server.server_close()
    print("setup update tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

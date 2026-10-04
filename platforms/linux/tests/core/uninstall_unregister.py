#!/usr/bin/env python3
"""The CMake uninstall takes the input method out of the uninstalling user's input method lists, the counterpart of the Windows uninstaller unregistering the TSF profile.

The installed msime-linux-setup is a stub that records how it was called and whether it still existed at that moment, and `id` is a stub so the non-root path runs under the root build-gate container too. The uninstall runs against a synthetic manifest in a scratch prefix, without DESTDIR unless a case asks for it.

Usage: uninstall_unregister.py <cmake> <configured uninstall.cmake>
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

CMAKE = sys.argv[1]
UNINSTALL = Path(sys.argv[2])

# Records its arguments, and that the files it belongs to were still installed when it ran.
SETUP = """#!/bin/sh
other="$(dirname "$0")/../share/msime-client/other.txt"
printf '%s other=%s\\n' "$*" "$([ -e "$other" ] && echo present || echo removed)" >> "$STUB_LOG"
exit "${STUB_SETUP_STATUS:-0}"
"""

ID = """#!/bin/sh
echo "$STUB_UID"
"""

MANUAL = "remove it there: Metasequoia 水杉输入法 from the desktop input sources (IBus), MSIME from the current group in fcitx5-configtool (Fcitx5)"


def uninstall(temp: Path, uid: str = "1000", staged: bool = False, setup_status: str = "0", with_setup: bool = True):
    prefix = temp / "prefix"
    stage = temp / "stage" if staged else None
    root = Path(str(stage) + str(prefix)) if stage else prefix
    installed = [prefix / "share/msime-client/other.txt"]
    if with_setup:
        installed.append(prefix / "bin/msime-linux-setup")
    for path in installed:
        target = root / path.relative_to(prefix)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(SETUP if path.name == "msime-linux-setup" else "installed")
        target.chmod(0o755)
    script, count = re.subn(r'set\(manifest "[^"]*"\)', lambda _: f'set(manifest "{temp / "manifest.txt"}")', UNINSTALL.read_text())
    assert count == 1, "uninstall.cmake no longer names its manifest"
    (temp / "uninstall.cmake").write_text(script)
    (temp / "manifest.txt").write_text("".join(f"{path}\n" for path in installed))
    tools = temp / "tools"
    tools.mkdir(exist_ok=True)
    (tools / "id").write_text(ID)
    (tools / "id").chmod(0o755)
    log = temp / "setup.log"
    log.write_text("")
    environment = {
        # 桩目录在前，其余沿用真实 PATH，与同目录的其他测试一致；写死 /usr/bin:/bin 在没有 FHS 布局的环境里找不到 dirname。
        "PATH": f"{tools}:{os.environ.get('PATH', '/usr/bin:/bin')}",
        "STUB_UID": uid,
        "STUB_LOG": str(log),
        "STUB_SETUP_STATUS": setup_status,
    }
    if stage:
        environment["DESTDIR"] = str(stage)
    result = subprocess.run(
        [CMAKE, f"-DMSIME_UNINSTALL_PREFIX={prefix}", "-P", str(temp / "uninstall.cmake")],
        env=environment, capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, (result.stdout, result.stderr)
    assert not any((root / path.relative_to(prefix)).exists() for path in installed), result.stdout
    return result, log.read_text().splitlines()


def main() -> None:
    # A user uninstalling their own installation: setup runs once with --unregister, before any file is removed.
    with tempfile.TemporaryDirectory() as name:
        result, calls = uninstall(Path(name))
        assert calls == ["--unregister other=present"], calls
        assert MANUAL not in result.stdout, result.stdout

    # Its failure does not stop the uninstall; the user is told where to remove the input method by hand.
    with tempfile.TemporaryDirectory() as name:
        result, calls = uninstall(Path(name), setup_status="3")
        assert calls == ["--unregister other=present"], calls
        assert "Could not remove MSIME from this user's input method lists" in result.stdout, result.stdout

    # A root or staged uninstall cannot reach anyone's session, and the program is deleted right after, so it names what to remove by hand rather than a command.
    for kwargs in ({"uid": "0"}, {"staged": True}):
        with tempfile.TemporaryDirectory() as name:
            result, calls = uninstall(Path(name), **kwargs)
            assert calls == [], (kwargs, calls)
            assert MANUAL in result.stdout, (kwargs, result.stdout)
            assert "msime-linux-setup --unregister" not in result.stdout, result.stdout

    # An installation without the setup script has nothing to run.
    with tempfile.TemporaryDirectory() as name:
        result, calls = uninstall(Path(name), with_setup=False)
        assert calls == [] and MANUAL in result.stdout, result.stdout

    print("The CMake uninstall removes the input method from the uninstalling user's lists before deleting the program")


if __name__ == "__main__":
    main()

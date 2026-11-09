#!/usr/bin/env python3
"""在没有 graphical-session.target 的会话里，登录时排队启动剪贴板服务的 XDG 自启动项。

It checks where `cmake --install` puts the entry (relocated by --prefix unless the configured prefix maps it to /etc), that the CMake uninstall removes it in both layouts, and runs its Exec line against a stub systemctl: nothing happens for a user who never enabled the service or where graphical-session.target already started it; otherwise the session's display reaches the user manager before the service is restarted.

Usage: clipboard_autostart.py <cmake> <build-dir> <configured autostart directory> <configured uninstall.cmake>
"""
import configparser
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

CMAKE, BUILD, AUTOSTART_DIR, UNINSTALL = sys.argv[1:5]
SOURCE = Path(__file__).resolve().parents[2] / "data/msime-linux-clipboard.desktop"
NAME = "msime-linux-clipboard.desktop"
UNIT = "msime-linux-clipboard.service"

SYSTEMCTL = """#!/bin/sh
printf '%s\\n' "$*" >> "$STUB_LOG"
case " $* " in
  *" is-enabled "*) exit "$STUB_ENABLED" ;;
  *" is-active "*) exit "$STUB_ACTIVE" ;;
esac
exit 0
"""


def entry() -> configparser.SectionProxy:
    parser = configparser.ConfigParser(interpolation=None)
    parser.optionxform = str
    parser.read(SOURCE, encoding="utf-8")
    return parser["Desktop Entry"]


def installed() -> None:
    with tempfile.TemporaryDirectory() as temp:
        stage = Path(temp) / "stage"
        prefix = Path(temp) / "prefix"
        result = subprocess.run(
            [CMAKE, "--install", BUILD, "--component", "clipboard-autostart", "--prefix", str(prefix)],
            env={**os.environ, "DESTDIR": str(stage)}, capture_output=True, text=True, check=False,
        )
        assert result.returncode == 0, result.stderr
        if os.path.isabs(AUTOSTART_DIR):
            # A /usr prefix maps it to /etc/xdg/autostart, which --prefix does not move.
            expected = Path(str(stage) + AUTOSTART_DIR) / NAME
        else:
            # Any other prefix keeps it under the prefix, so --prefix relocates it with everything else.
            expected = Path(str(stage) + str(prefix)) / AUTOSTART_DIR / NAME
        assert expected.is_file(), (expected, sorted(str(path) for path in stage.rglob("*")))
        assert expected.read_bytes() == SOURCE.read_bytes()
        # The component holds this one file.
        assert [path for path in stage.rglob("*") if path.is_file()] == [expected], list(stage.rglob("*"))
        assert AUTOSTART_DIR.rstrip("/").endswith("xdg/autostart"), AUTOSTART_DIR


def uninstall(autostart_dir: str, prefix: str, manifest: list, stage: Path, temp: Path):
    """Runs the configured uninstall script as if configured with another autostart directory, against a synthetic manifest."""
    script = Path(UNINSTALL).read_text()
    script, count = re.subn(r'set\(autostart_destination "[^"]*"\)', lambda _: f'set(autostart_destination "{autostart_dir}")', script)
    assert count == 1, "uninstall.cmake no longer configures the autostart directory"
    script, count = re.subn(r'set\(manifest "[^"]*"\)', lambda _: f'set(manifest "{temp / "manifest.txt"}")', script)
    assert count == 1, "uninstall.cmake no longer names its manifest"
    (temp / "uninstall.cmake").write_text(script)
    (temp / "manifest.txt").write_text("".join(f"{path}\n" for path in manifest))
    for path in manifest:
        target = Path(str(stage) + path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("installed")
    return subprocess.run(
        [CMAKE, f"-DMSIME_UNINSTALL_PREFIX={prefix}", "-P", str(temp / "uninstall.cmake")],
        env={**os.environ, "DESTDIR": str(stage)}, capture_output=True, text=True, check=False,
    )


def uninstalled() -> None:
    # A /usr prefix: the entry sits under /etc, outside the prefix, and is still removed with the programs.
    with tempfile.TemporaryDirectory() as name:
        temp = Path(name)
        stage = temp / "stage"
        manifest = ["/usr/bin/msime-linux-ibus", f"/etc/xdg/autostart/{NAME}"]
        result = uninstall("/etc/xdg/autostart", "/usr", manifest, stage, temp)
        assert result.returncode == 0, result.stderr
        assert not any(Path(str(stage) + path).exists() for path in manifest), list(stage.rglob("*"))
        # Only that directory is accepted outside the prefix; anything else under /etc still aborts before removing.
        manifest = ["/usr/bin/msime-linux-ibus", "/etc/other/file"]
        result = uninstall("/etc/xdg/autostart", "/usr", manifest, stage, temp)
        assert result.returncode != 0 and "outside the uninstall prefix" in result.stderr, result.stderr
        assert Path(str(stage) + "/usr/bin/msime-linux-ibus").exists()
    # Any other prefix: the entry moved with the prefix, including one overridden at uninstall time as after `cmake --install --prefix`.
    with tempfile.TemporaryDirectory() as name:
        temp = Path(name)
        stage = temp / "stage"
        manifest = ["/opt/msime/bin/msime-linux-ibus", f"/opt/msime/etc/xdg/autostart/{NAME}"]
        result = uninstall("etc/xdg/autostart", "/opt/msime", manifest, stage, temp)
        assert result.returncode == 0, result.stderr
        assert not any(Path(str(stage) + path).exists() for path in manifest), list(stage.rglob("*"))
        # A relative directory adds no root: /etc stays outside.
        manifest = ["/opt/msime/bin/msime-linux-ibus", f"/etc/xdg/autostart/{NAME}"]
        result = uninstall("etc/xdg/autostart", "/opt/msime", manifest, stage, temp)
        assert result.returncode != 0 and "outside the uninstall prefix" in result.stderr, result.stderr


def run_exec(enabled: bool, target_active: bool) -> list:
    command = shlex.split(entry()["Exec"])
    with tempfile.TemporaryDirectory() as temp:
        bin_dir = Path(temp) / "bin"
        bin_dir.mkdir()
        (bin_dir / "systemctl").write_text(SYSTEMCTL)
        (bin_dir / "systemctl").chmod(0o755)
        (bin_dir / "sh").symlink_to(shutil.which("sh"))
        log = Path(temp) / "systemctl.log"
        result = subprocess.run(
            command,
            env={
                "PATH": str(bin_dir),
                "STUB_LOG": str(log),
                "STUB_ENABLED": "0" if enabled else "1",
                "STUB_ACTIVE": "0" if target_active else "3",
                "WAYLAND_DISPLAY": "wayland-1",
            },
            capture_output=True, text=True, check=False,
        )
        assert result.returncode == 0, (result.returncode, result.stderr)
        return log.read_text().splitlines() if log.exists() else []


def main() -> None:
    desktop = entry()
    assert desktop["Type"] == "Application"
    assert desktop["NoDisplay"] == "true"
    # Gated on the unit being enabled, so users who never enabled the service are unaffected.
    assert "is-enabled" in desktop["Exec"] and UNIT in desktop["Exec"], desktop["Exec"]
    # systemctl 默认等待重启任务完成；登录会话不能在这里等待 graphical-session.target。
    assert "--no-block restart" in desktop["Exec"], desktop["Exec"]
    # systemd-xdg-autostart-generator skips entries with a GNOME startup phase; sessions it serves are left to the Exec line's own check instead.
    assert "X-GNOME-Autostart-Phase" not in desktop
    # Reserved characters only inside one double-quoted argument, as the Desktop Entry spec requires.
    assert "'" not in desktop["Exec"] and "$" not in desktop["Exec"] and "`" not in desktop["Exec"], desktop["Exec"]

    installed()
    uninstalled()

    is_enabled = f"--user -q is-enabled {UNIT}"
    is_active = "--user -q is-active graphical-session.target"
    assert run_exec(enabled=False, target_active=False) == [is_enabled]
    assert run_exec(enabled=False, target_active=True) == [is_enabled]
    assert run_exec(enabled=True, target_active=True) == [is_enabled, is_active]
    assert run_exec(enabled=True, target_active=False) == [
        is_enabled,
        is_active,
        "--user import-environment WAYLAND_DISPLAY DISPLAY XAUTHORITY",
        f"--user --no-block restart {UNIT}",
    ]
    print("Clipboard autostart entry is installed and starts only an enabled service outside graphical-session.target")


if __name__ == "__main__":
    main()

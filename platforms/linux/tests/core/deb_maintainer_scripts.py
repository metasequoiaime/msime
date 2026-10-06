#!/usr/bin/env python3
"""The Debian prerm and postinst: which systemctl and systemd-run calls they make for which dpkg action, including the prerm's msime-linux-setup --unregister.

systemctl, systemd-run and loginctl are stubs that record their arguments, and PATH holds nothing else, so the scripts also prove they need no other program. A real systemd user manager is not started here; the build-gate container runs under an init that is not systemd.

Usage: deb_maintainer_scripts.py <configured-debian-dir> <configured-uninstall.cmake> <unit list> <installed msime-linux-setup>
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

DEBIAN = Path(sys.argv[1])
UNINSTALL = Path(sys.argv[2])
UNITS = sys.argv[3].split()
SERVICES = [unit for unit in UNITS if unit.endswith(".service")]
SETUP = sys.argv[4]
PROVIDER = SETUP.rsplit("/", 1)[0] + "/msime-linux-online-provider"
# Removing the input method from each user's lists, the counterpart of the Windows uninstaller unregistering the TSF profile.
MANUAL_LISTS = "remove the input method from its lists: Metasequoia 水杉输入法 from the desktop input sources (IBus), MSIME from the current group in fcitx5-configtool (Fcitx5)"

# systemd 252 prints UID USER LINGER, newer releases add STATE and indent the UID column; both must parse.
USERS = "1000 alice no\n   1001 bob yes active\n"

SYSTEMCTL = """#!/bin/sh
printf '%s\\n' "$*" >> "$STUB_LOG"
case " $* " in
  *" -M $STUB_UNREACHABLE@ "*) exit 1 ;;
  *" $STUB_FAILING_UNIT "*) exit 5 ;;
  *" is-active "*) [ -n "$STUB_ACTIVE_UNIT" ] && case " $* " in *" $STUB_ACTIVE_UNIT "*) exit 0 ;; esac; exit 3 ;;
esac
exit 0
"""

# Logged to the same file so the order relative to the systemctl calls is checked too.
SYSTEMD_RUN = """#!/bin/sh
printf 'systemd-run %s\\n' "$*" >> "$STUB_LOG"
[ -z "$STUB_SYSTEMD_RUN_FAILS" ] || exit 1
exit 0
"""

WITH_SYSTEMD_RUN = ("systemctl", "loginctl", "systemd-run")

LOGINCTL = """#!/bin/sh
[ -z "$STUB_LOGINCTL_FAILS" ] || exit 1
printf '%s' "$STUB_USERS"
"""


def run(script: str, *args: str, tools=("systemctl", "loginctl"), **env: str):
    with tempfile.TemporaryDirectory() as temp:
        bin_dir = Path(temp) / "bin"
        bin_dir.mkdir()
        log = Path(temp) / "systemctl.log"
        for name, body in (("systemctl", SYSTEMCTL), ("loginctl", LOGINCTL), ("systemd-run", SYSTEMD_RUN)):
            if name in tools:
                (bin_dir / name).write_text(body)
                (bin_dir / name).chmod(0o755)
        environment = {
            "PATH": str(bin_dir),
            "STUB_LOG": str(log),
            "STUB_USERS": USERS,
            "STUB_UNREACHABLE": "",
            "STUB_FAILING_UNIT": "",
            "STUB_LOGINCTL_FAILS": "",
            "STUB_SYSTEMD_RUN_FAILS": "",
            "STUB_ACTIVE_UNIT": "",
            **env,
        }
        result = subprocess.run(
            [str(DEBIAN / script), *args], env=environment, capture_output=True, text=True, check=False
        )
        calls = log.read_text().splitlines() if log.exists() else []
        return result, calls


def expect(result, calls, expected_calls, stderr_lines=0):
    assert result.returncode == 0, (result.returncode, result.stderr)
    assert result.stdout == "", result.stdout
    assert calls == expected_calls, calls
    assert len(result.stderr.splitlines()) == stderr_lines, result.stderr


def disable_calls(uid: str) -> list:
    return [f"--user -M {uid}@ show --property=Version"] + [f"--user -M {uid}@ disable --now {unit}" for unit in UNITS]


def unregister_call(uid: str) -> list:
    return [f"systemd-run --user -M {uid}@ --wait --collect --quiet {SETUP} --unregister"]


def removal_calls(uid: str) -> list:
    return disable_calls(uid) + unregister_call(uid)


def restart_calls(uid: str) -> list:
    return [f"--user -M {uid}@ daemon-reload"] + [f"--user -M {uid}@ try-restart {service}" for service in SERVICES]


def account_call(uid: str) -> list:
    return [f"systemd-run --user -M {uid}@ --wait --collect --quiet {PROVIDER} --ensure-anonymous-account"]


def configure_calls(uid: str, account: bool = True) -> list:
    calls = restart_calls(uid)
    return calls[:1] + (account_call(uid) if account else []) + calls[1:]


def main() -> None:
    assert UNITS and SERVICES and len(SERVICES) < len(UNITS), UNITS
    assert SETUP.startswith("/") and SETUP.endswith("/bin/msime-linux-setup"), SETUP
    # Both removal paths stop the same units: the CMake uninstall is configured from the same list.
    assert f"set(user_units {' '.join(UNITS)})" in UNINSTALL.read_text()

    for script in ("prerm", "postinst"):
        path = DEBIAN / script
        assert os.access(path, os.X_OK), path
        assert path.read_text().startswith("#!/bin/sh\n"), path
        assert "@" + "MSIME_" not in path.read_text(), f"{script} has an unconfigured placeholder"

    # Removal stops and disables every unit for every logged-in user, sockets before their services.
    result, calls = run("prerm", "remove")
    expect(result, calls, disable_calls("1000") + disable_calls("1001"))

    # An upgrade, or a failed one, leaves the services to postinst. A deconfigure is temporary and keeps the package installed, and neither the configure nor the abort-deconfigure that follows enables units again, so it must not disable any.
    for args in (
        ("upgrade", "1.0.1"),
        ("failed-upgrade", "1.0.0"),
        ("deconfigure", "in-favour", "breaker", "2.0", "removing", "dep", "1.0"),
        ("deconfigure", "in-favour", "breaker", "2.0"),
    ):
        expect(*run("prerm", *args), [])

    # One unit failing to disable does not stop the rest, and does not fail the removal, wherever it is in the list.
    for failing in (UNITS[0], UNITS[-1]):
        result, calls = run("prerm", "remove", STUB_FAILING_UNIT=failing)
        expect(result, calls, disable_calls("1000") + disable_calls("1001"))

    # A user manager that cannot be reached gets the command the CMake uninstall prints; the other user is still handled.
    result, calls = run("prerm", "remove", STUB_UNREACHABLE="1000")
    expect(result, calls, disable_calls("1000")[:1] + disable_calls("1001"), stderr_lines=1)
    assert "alice" in result.stderr and f"systemctl --user disable --now {' '.join(UNITS)}" in result.stderr, result.stderr

    # With systemd-run, removal also takes the input method out of each user's input method lists, in that user's manager so it reaches the session bus, after the units are disabled and while the program still exists. prerm waits for it.
    result, calls = run("prerm", "remove", tools=WITH_SYSTEMD_RUN)
    expect(result, calls, removal_calls("1000") + removal_calls("1001"))

    # A failure to unregister does not fail the removal or skip the next user.
    result, calls = run("prerm", "remove", tools=WITH_SYSTEMD_RUN, STUB_SYSTEMD_RUN_FAILS="1")
    expect(result, calls, removal_calls("1000") + removal_calls("1001"))

    # An unreachable user is told what to remove by hand: the program that would do it is deleted right after prerm, so naming it would be no help.
    result, calls = run("prerm", "remove", tools=WITH_SYSTEMD_RUN, STUB_UNREACHABLE="1000")
    expect(result, calls, disable_calls("1000")[:1] + removal_calls("1001"), stderr_lines=1)
    assert "alice" in result.stderr and MANUAL_LISTS in result.stderr, result.stderr
    assert "msime-linux-setup --unregister" not in result.stderr, result.stderr

    # An upgrade keeps the input method, and a deconfigure keeps the package installed: neither unregisters.
    for args in (("upgrade", "1.0.1"), ("failed-upgrade", "1.0.0"), ("deconfigure", "in-favour", "breaker", "2.0")):
        expect(*run("prerm", *args, tools=WITH_SYSTEMD_RUN), [])

    # Every configure (including a first install) asks each reachable user manager to register its anonymous account, then reloads the manager and restarts only running services; sockets are left listening.
    result, calls = run("postinst", "configure", "1.0.0")
    expect(result, calls, restart_calls("1000") + restart_calls("1001"))

    for failing in (SERVICES[0], SERVICES[-1]):
        result, calls = run("postinst", "configure", "1.0.0", STUB_FAILING_UNIT=failing)
        expect(result, calls, restart_calls("1000") + restart_calls("1001"))

    result, calls = run("postinst", "configure", "1.0.0", STUB_UNREACHABLE="1001")
    expect(result, calls, restart_calls("1000") + restart_calls("1001")[:1], stderr_lines=1)
    assert "bob" in result.stderr and f"systemctl --user try-restart {' '.join(SERVICES)}" in result.stderr, result.stderr

    # With systemd-run, every configure (an upgrade or a first installation) also registers each reachable user's anonymous account in that user's manager, after the reload and before the services restart.
    for args in (("configure", "1.0.0"), ("configure", ""), ("configure",)):
        expect(*run("postinst", *args, tools=WITH_SYSTEMD_RUN), configure_calls("1000") + configure_calls("1001"))

    # A registration that fails does not fail the configure or skip the next user.
    result, calls = run("postinst", "configure", "1.0.0", tools=WITH_SYSTEMD_RUN, STUB_SYSTEMD_RUN_FAILS="1")
    expect(result, calls, configure_calls("1000") + configure_calls("1001"))

    # A user manager that cannot be reached is skipped with the commands to run, and the other user is still handled.
    result, calls = run("postinst", "configure", "1.0.0", tools=WITH_SYSTEMD_RUN, STUB_UNREACHABLE="1000")
    expect(result, calls, restart_calls("1000")[:1] + configure_calls("1001"), stderr_lines=1)
    assert "alice" in result.stderr and f"systemctl --user try-restart {' '.join(SERVICES)}" in result.stderr, result.stderr

    # Removal does not register anything, and the abort paths have nothing to undo.
    expect(*run("prerm", "remove", tools=WITH_SYSTEMD_RUN), removal_calls("1000") + removal_calls("1001"))
    for args in (("abort-upgrade", "1.0.1"), ("abort-remove",)):
        expect(*run("postinst", *args, tools=WITH_SYSTEMD_RUN), [])

    # A first installation has nothing running; the abort paths have nothing to undo. Without systemd-run, account registration is deferred to msime-linux-setup.
    for args in (("configure", ""), ("configure",), ("abort-upgrade", "1.0.1"), ("abort-remove",), ("abort-deconfigure", "in-favour", "breaker", "2.0")):
        expected = restart_calls("1000") + restart_calls("1001") if args[0] == "configure" else []
        expect(*run("postinst", *args), expected)

    # Containers and systems without systemd: nothing to do, nothing printed, never a failure.
    for script, args in (("prerm", ("remove",)), ("postinst", ("configure", "1.0.0"))):
        expect(*run(script, *args, tools=("loginctl",)), [])
        expect(*run(script, *args, tools=("systemctl",)), [])
        expect(*run(script, *args, STUB_LOGINCTL_FAILS="1"), [])
        expect(*run(script, *args, STUB_USERS=""), [])

    print("Debian maintainer scripts stop units and unregister the input method on removal, and restart services on upgrade")


if __name__ == "__main__":
    main()

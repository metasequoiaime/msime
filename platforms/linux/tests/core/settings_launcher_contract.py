#!/usr/bin/env python3
"""Static contract checks for desktop settings panel aliases."""

# The legacy .deb runs this on Debian buster's Python 3.7, where list[str] in an annotation is a TypeError unless annotations are postponed.
from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[2]
launcher = (root / "data/msime-linux-settings.in").read_text()
desktop = (root / "data/msime-linux.desktop.in").read_text()
engine = (root / "src/core/ClientEngine.cpp").read_text()
assert "about|help|feedback|dictionary) route=settings:$2" in launcher
assert "Desktop Action Dictionary" in desktop
assert "--panel dictionary" in desktop
# The route travels only as the desktop shell's --route argument. A settings section must travel as "settings:<category>": the bare section name is not a route head, so the shared parser would reject it and the desktop shell would fall back to its default page.
assert '"--route=$route"' in launcher
assert '"--route="' in engine
# The host builds the route rather than spelling each one out, so the literal to look for is the prefix it prepends.
assert 'std::string("settings:")' in engine
for retired in (
    "MSIME_CLIENT_PANEL",
    "MSIME_CLIENT_SETTINGS_PAGE",
    "MSIME_CLIENT_ROUTE",
):
    assert retired not in launcher and retired not in engine, retired
for property_name in (
    "Learning",
    "FrequencyMode",
    "FrequencyTriggerCount",
    "FrequencyLinearStep",
):
    assert f'"{property_name}"' in engine
assert "MenuPreference::Learning" in engine
assert "MenuPreference::FrequencyTriggerCount" in engine
assert "MenuPreference::FrequencyLinearStep" in engine
assert '"ShuangpinPreedit"' in engine
assert "MenuPreference::ShuangpinPreedit" in engine
assert "shuangpin_preedit_uses_raw" in engine
assert '"WubiCodeHint"' in engine
assert "wubi_code_hint" in engine
assert "MenuPreference::WubiCodeHint" in engine
assert '"WubiAutoCommitUnique"' in engine
assert "wubi_auto_commit_unique" in engine
assert "MenuPreference::WubiAutoCommitUnique" in engine

# Without a prepared file the launcher still opens the window, which shows the first-run page for the user locator; an installed system configuration keeps precedence over that page.
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    system_config = scratch / "system/runtime-options.json"
    script = scratch / "bin/msime-linux-settings"
    script.parent.mkdir()
    # 首行换成 /bin/sh：模板的 `#!/usr/bin/env sh` 在没有 FHS 布局的环境（Nix 构建沙箱）里找不到 env，
    # 装出去的那份由打包时的 patchShebangs 改写；这里测的是脚本内容。
    body = launcher.replace("@MSIME_SETTINGS_SYSTEM_CONFIG@", str(system_config))
    # NVIDIA 驱动的两个探测位置换成临时目录里的路径，测试按需创建它们来模拟装了或没装驱动，不依赖运行测试的机器有没有 NVIDIA。
    nvidia_markers = "/proc/driver/nvidia/version /sys/module/nvidia"
    assert f"for nvidia_marker in {nvidia_markers}; do" in body
    nvidia_version = scratch / "proc/driver/nvidia/version"
    nvidia_module = scratch / "sys/module/nvidia"
    body = body.replace(nvidia_markers, f"{nvidia_version} {nvidia_module}")
    assert body.startswith("#!/usr/bin/env sh\n"), body.splitlines()[0]
    script.write_text("#!/bin/sh\n" + body.split("\n", 1)[1])
    desktop_binary = scratch / "bin/msime-linux-desktop"
    desktop_binary.write_text('#!/bin/sh\nprintf %s "$MSIME_CLIENT_HOST_OPTIONS"\n')
    for path in (script, desktop_binary):
        path.chmod(0o755)
    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in ("MSIME_CLIENT_HOST_OPTIONS", "MSIME_IBUS_OPTIONS")
    }
    environment["XDG_CONFIG_HOME"] = str(scratch / "config")

    def launched() -> str:
        return subprocess.run(
            [str(script)], env=environment, check=True, capture_output=True, text=True
        ).stdout

    assert launched() == str(scratch / "config/msime-client/runtime-options.json")
    system_config.parent.mkdir()
    system_config.write_text("{}")
    assert launched() == str(system_config)

    # About, Help and Feedback each open their own settings section. Unlisted in the --panel case they exit 2, and passed through bare they would not parse as a route, leaving the window on its home page.
    desktop_binary.write_text('#!/bin/sh\nprintf "%s" "$*"\n')

    def launched_panel(panel: str) -> str:
        return subprocess.run(
            [str(script), "--panel", panel],
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        ).stdout

    for page in ("about", "help", "feedback", "dictionary"):
        assert launched_panel(page) == f"--route=settings:{page}", page
    assert launched_panel("handwriting") == "--route=handwriting"
    # A host's own --route argument reaches the shell untouched.
    assert (
        subprocess.run(
            [str(script), "--route=emoji"],
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        == "--route=emoji"
    )
    usage = subprocess.run(
        [str(script), "--help"],
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    assert "|help|feedback|" in usage

    # 装了 NVIDIA 专有驱动时，所有窗口入口默认禁用 WebKit 合成与 DMA-BUF 渲染器；没装时两个变量都不设，交回 WebKit 自己的默认（#6847）。两种情况都不改变调用方的 GTK 后端，调用方显式给出的非空值都原样保留，用于排查上游问题。
    # 假的桌面程序把未设置的变量打成 `<unset>`，好把「没设」和「设成空串」分开：WebKit 把空串也当作禁用。
    desktop_binary.write_text(
        '#!/bin/sh\nprintf "%s\\n" "${WEBKIT_DISABLE_COMPOSITING_MODE-<unset>}" "${WEBKIT_DISABLE_DMABUF_RENDERER-<unset>}" "$GDK_BACKEND" "$@"\n'
    )
    environment.pop("WEBKIT_DISABLE_COMPOSITING_MODE", None)
    environment.pop("WEBKIT_DISABLE_DMABUF_RENDERER", None)
    environment["GDK_BACKEND"] = "wayland"
    entries = (
        [],
        ["--panel", "settings"],
        ["--panel", "voice"],
        ["--route=settings:about"],
    )

    def launched_webkit(arguments: list[str]) -> list[str]:
        output = subprocess.run(
            [str(script), *arguments],
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        lines = output.splitlines()
        assert lines[2] == "wayland", (arguments, output)
        return lines[:2]

    def check_overrides(default: str) -> None:
        for arguments in entries:
            assert launched_webkit(arguments) == [default, default], arguments
        for name, position in (
            ("WEBKIT_DISABLE_COMPOSITING_MODE", 0),
            ("WEBKIT_DISABLE_DMABUF_RENDERER", 1),
        ):
            # 空串按未设置处理，与另一个变量一样取默认值；`0` 和 `1` 原样传下去。
            for value, expected in (("", default), ("0", "0"), ("1", "1")):
                environment[name] = value
                lines = launched_webkit([])
                assert lines[position] == expected, (name, value, default, lines)
                assert lines[1 - position] == default, (name, value, default, lines)
            environment.pop(name)

    # 没有 NVIDIA 驱动：两个变量都不设。
    check_overrides("<unset>")
    # /proc/driver/nvidia/version 与 /sys/module/nvidia 任一存在都算装了驱动，各自单独检查一遍。
    nvidia_version.parent.mkdir(parents=True)
    nvidia_version.write_text("NVRM version: synthetic\n")
    check_overrides("1")
    nvidia_version.unlink()
    check_overrides("<unset>")
    nvidia_module.mkdir(parents=True)
    check_overrides("1")
    nvidia_module.rmdir()
    check_overrides("<unset>")

print("settings launcher contract: ok")

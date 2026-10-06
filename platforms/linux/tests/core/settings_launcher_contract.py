#!/usr/bin/env python3
"""Static contract checks for desktop settings panel aliases."""
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
for retired in ("MSIME_CLIENT_PANEL", "MSIME_CLIENT_SETTINGS_PAGE", "MSIME_CLIENT_ROUTE"):
    assert retired not in launcher and retired not in engine, retired
for property_name in ("Learning", "FrequencyMode", "FrequencyTriggerCount", "FrequencyLinearStep"):
    assert f'"{property_name}"' in engine
assert "MenuPreference::Learning" in engine
assert "MenuPreference::FrequencyTriggerCount" in engine
assert "MenuPreference::FrequencyLinearStep" in engine
assert '"ShuangpinPreedit"' in engine
assert "MenuPreference::ShuangpinPreedit" in engine
assert 'shuangpin_preedit_uses_raw' in engine
assert '"WubiCodeHint"' in engine
assert 'wubi_code_hint' in engine
assert "MenuPreference::WubiCodeHint" in engine

# Without a prepared file the launcher still opens the window, which shows the first-run page for the user locator; an installed system configuration keeps precedence over that page.
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    system_config = scratch / "system/runtime-options.json"
    script = scratch / "bin/msime-linux-settings"
    script.parent.mkdir()
    # 首行换成 /bin/sh：模板的 `#!/usr/bin/env sh` 在没有 FHS 布局的环境（Nix 构建沙箱）里找不到 env，
    # 装出去的那份由打包时的 patchShebangs 改写；这里测的是脚本内容。
    body = launcher.replace("@MSIME_SETTINGS_SYSTEM_CONFIG@", str(system_config))
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
        return subprocess.run([str(script)], env=environment, check=True, capture_output=True, text=True).stdout

    assert launched() == str(scratch / "config/msime-client/runtime-options.json")
    system_config.parent.mkdir()
    system_config.write_text("{}")
    assert launched() == str(system_config)

    # About, Help and Feedback each open their own settings section. Unlisted in the --panel case they exit 2, and passed through bare they would not parse as a route, leaving the window on its home page.
    desktop_binary.write_text('#!/bin/sh\nprintf "%s" "$*"\n')

    def launched_panel(panel: str) -> str:
        return subprocess.run([str(script), "--panel", panel], env=environment, check=True, capture_output=True, text=True).stdout

    for page in ("about", "help", "feedback", "dictionary"):
        assert launched_panel(page) == f"--route=settings:{page}", page
    assert launched_panel("handwriting") == "--route=handwriting"
    # A host's own --route argument reaches the shell untouched.
    assert subprocess.run([str(script), "--route=emoji"], env=environment, check=True, capture_output=True, text=True).stdout == "--route=emoji"
    usage = subprocess.run([str(script), "--help"], env=environment, check=True, capture_output=True, text=True).stdout
    assert "|help|feedback|" in usage

    # 所有窗口入口默认禁用 WebKit 合成，不改变调用方的 GTK 后端；显式值仍可用于排查上游问题。
    desktop_binary.write_text('#!/bin/sh\nprintf "%s\\n" "$WEBKIT_DISABLE_COMPOSITING_MODE" "$GDK_BACKEND" "$@"\n')
    environment.pop("WEBKIT_DISABLE_COMPOSITING_MODE", None)
    environment["GDK_BACKEND"] = "wayland"
    for arguments in ([], ["--panel", "settings"], ["--panel", "voice"], ["--route=settings:about"]):
        output = subprocess.run([str(script), *arguments], env=environment, check=True, capture_output=True, text=True).stdout
        assert output.splitlines()[:2] == ["1", "wayland"], (arguments, output)
    for value, expected in (("", "1"), ("0", "0"), ("1", "1")):
        environment["WEBKIT_DISABLE_COMPOSITING_MODE"] = value
        output = launched()
        assert output.splitlines()[:2] == [expected, "wayland"], output

print("settings launcher contract: ok")

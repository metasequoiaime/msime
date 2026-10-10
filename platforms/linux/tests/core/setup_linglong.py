#!/usr/bin/env python3
"""msime-linux-setup 在带玲珑（Linglong）的系统上注册进 IBus 时提示改用 Fcitx5：玲珑运行时只带 Fcitx5 的输入法模块，IBus 用户在玲珑应用里只能打出字母（#6410）。只在注册进 IBus、本安装带 Fcitx5 插件、且系统是 deepin/UOS 或装了玲珑时提示，其他系统的输出不变。纯函数，不碰真实的系统。
"""
import importlib.machinery
import importlib.util
from pathlib import Path
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/msime-linux-setup"


def load_setup():
    spec = importlib.util.spec_from_loader(
        "msime_client_setup", importlib.machinery.SourceFileLoader("msime_client_setup", str(SCRIPT))
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    setup = load_setup()
    note = setup.linglong_ibus_note
    deepin = {"ID": "deepin", "NAME": "Deepin"}

    # deepin 上用 IBus：提示，写明改用 Fcitx5 的命令并指向 README 的那一节。
    message = note("ibus", deepin, False, True)
    assert message and "Fcitx5" in message and "im-config -n fcitx5" in message, message
    assert "msime-linux-setup --register" in message and "deepin 玲珑应用里只能输入字母" in message, message
    # 统信 UOS、以 deepin 为基础的发行版（ID_LIKE）同样提示。
    assert note("ibus", {"ID": "uos"}, False, True) == message
    assert note("ibus", {"ID": "example", "ID_LIKE": "debian deepin"}, False, True) == message
    # 别的发行版上另装了玲珑：同样提示。
    assert note("ibus", {"ID": "ubuntu", "ID_LIKE": "debian"}, True, True) == message

    # 注册进 Fcitx5：玲珑应用能用，不提示。
    assert note("fcitx5", deepin, True, True) is None
    # 没有在运行的框架：没注册进 IBus，不提示。
    assert note(None, deepin, True, True) is None
    # 没有玲珑的其他发行版用 IBus：输出不变。
    assert note("ibus", {"ID": "ubuntu", "ID_LIKE": "debian"}, False, True) is None
    assert note("ibus", {}, False, True) is None

    # 不带 Fcitx5 插件的安装（legacy 包，-DMSIME_ENABLE_FCITX5=OFF）：切到 Fcitx5 后那里没有水杉，所以不提示，deepin/UOS 和装了玲珑都一样。
    assert note("ibus", {"ID": "uos"}, False, False) is None
    assert note("ibus", deepin, True, False) is None

    # 插件条目的查找：前缀的 share/fcitx5/addon 下有本版本的条目才算带插件。/usr/share 下也找，所以这台机器自己装着水杉的 Fcitx5 插件时跳过「没有」那一条。
    with tempfile.TemporaryDirectory(prefix="msime-fcitx5-addon-") as directory:
        prefix = Path(directory)
        entry = Path("fcitx5/addon") / f"{setup.FCITX5_INPUT_METHOD}.conf"
        if not (Path("/usr/share") / entry).is_file():
            assert not setup.fcitx5_addon_installed(prefix)
        (prefix / "share" / entry).parent.mkdir(parents=True)
        (prefix / "share" / entry).write_text("[Addon]\n")
        assert setup.fcitx5_addon_installed(prefix)

    # os-release 的解析：带引号的值去掉引号，缺文件时为空。
    with tempfile.TemporaryDirectory(prefix="msime-os-release-") as directory:
        release = Path(directory) / "os-release"
        release.write_text('PRETTY_NAME="Deepin 25"\nID=deepin\nID_LIKE="debian"\n# comment\n')
        missing = Path(directory) / "missing"
        assert setup.os_release((missing, release)) == {"PRETTY_NAME": "Deepin 25", "ID": "deepin", "ID_LIKE": "debian"}
        assert setup.os_release((missing,)) == {}
    print("setup Linglong note: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

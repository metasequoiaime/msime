#!/usr/bin/env python3
"""msime-linux-setup 在 GNOME 的 Wayland 会话里提示不要为 GTK 设 GTK_IM_MODULE=fcitx：那样 GTK 程序的候选窗由 fcitx5-gtk 的客户端面板绘制，会不停闪烁、不用水杉主题。只在注册进 Fcitx5、会话是 GNOME Wayland、本进程或用户级 systemd 的 GTK_IM_MODULE 是 fcitx 时提示。纯函数，不碰真实的会话。
"""
import importlib.machinery
import importlib.util
from pathlib import Path

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
    note = setup.gnome_wayland_gtk_note
    gnome_wayland = {"XDG_SESSION_TYPE": "wayland", "XDG_CURRENT_DESKTOP": "ubuntu:GNOME"}

    # GNOME Wayland、Fcitx5、本进程设了 fcitx：提示，并指向 README 的那一节。
    message = note("fcitx5", {**gnome_wayland, "GTK_IM_MODULE": "fcitx"}, {})
    assert message and "GTK_IM_MODULE" in message and "GNOME Wayland 下候选窗闪烁" in message
    # 只有用户级 systemd 留着 fcitx（开了 Linger、D-Bus 激活的终端用它）：同样提示。
    assert note("fcitx5", dict(gnome_wayland), {"GTK_IM_MODULE": "fcitx"}) == message
    # 安装包以临时单元运行：本进程没有会话变量，按 systemd 的判断。
    assert note("fcitx5", {}, {**gnome_wayland, "GTK_IM_MODULE": "fcitx"}) == message
    # 纯 GNOME（XDG_CURRENT_DESKTOP=GNOME）也算。
    assert note("fcitx5", {"XDG_SESSION_TYPE": "wayland", "XDG_CURRENT_DESKTOP": "GNOME", "GTK_IM_MODULE": "fcitx"}, {})

    # X11：经典界面能直接画，不提示。
    assert note("fcitx5", {"XDG_SESSION_TYPE": "x11", "XDG_CURRENT_DESKTOP": "ubuntu:GNOME", "GTK_IM_MODULE": "fcitx"}, {}) is None
    # KDE Plasma 的 Wayland 支持输入法弹窗协议，不提示。
    assert note("fcitx5", {"XDG_SESSION_TYPE": "wayland", "XDG_CURRENT_DESKTOP": "KDE", "GTK_IM_MODULE": "fcitx"}, {}) is None
    # 名字里带 GNOME 的别的桌面（例如 GNOME-Flashback）不算。
    assert note("fcitx5", {"XDG_SESSION_TYPE": "wayland", "XDG_CURRENT_DESKTOP": "GNOME-Flashback", "GTK_IM_MODULE": "fcitx"}, {}) is None
    # 已经不设，或设的不是 fcitx：不提示。
    assert note("fcitx5", dict(gnome_wayland), {}) is None
    assert note("fcitx5", {**gnome_wayland, "GTK_IM_MODULE": "ibus"}, {"GTK_IM_MODULE": "ibus"}) is None
    # 注册进 IBus 或没有在运行的框架：不提示。
    assert note("ibus", {**gnome_wayland, "GTK_IM_MODULE": "fcitx"}, {}) is None
    assert note(None, {**gnome_wayland, "GTK_IM_MODULE": "fcitx"}, {}) is None
    print("setup GNOME Wayland note: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

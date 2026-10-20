#!/usr/bin/env python3
"""由真实 IBus daemon 从安装组件首次激活宿主，不提前启动输入法进程。"""

import argparse
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host")
    parser.add_argument("component")
    parser.add_argument("--options")
    parser.add_argument("--session", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not all(shutil.which(tool) for tool in ("ibus-daemon", "dbus-run-session")):
        print("skipped: ibus-daemon and dbus-run-session are required")
        return 77
    try:
        import gi
        gi.require_version("IBus", "1.0")
        from gi.repository import Gio, GLib, IBus
    except (ImportError, ValueError):
        print("skipped: Python IBus introspection is required")
        return 77
    if not args.session:
        return subprocess.call([
            "dbus-run-session", "--", sys.executable, __file__, "--session", *sys.argv[1:]
        ])

    with tempfile.TemporaryDirectory(prefix="msime-ibus-activation-") as temporary:
        root = Path(temporary)
        components = root / "components"
        components.mkdir()
        options = root / "runtime-options.json"
        options.write_text(Path(args.options).read_text() if args.options else '{"preferences":{}}')
        tree = ET.parse(args.component)
        component = tree.getroot()
        engine = component.findtext("engines/engine/name")
        assert engine, "组件没有引擎名"
        host = str(Path(args.host).resolve())
        launcher = Path(host).parent / "msime-linux-ibus-launcher"
        # 暂存安装仅重定位可执行文件，组件名和引擎描述沿用真正的安装产物。
        component.find("exec").text = " ".join(map(shlex.quote, [str(launcher) if launcher.is_file() else host, str(options)]))
        tree.write(components / "msime.xml", encoding="utf-8")
        os.environ.update(
            IBUS_COMPONENT_PATH=str(components),
            XDG_CONFIG_HOME=str(root / "config"), XDG_CACHE_HOME=str(root / "cache"),
            XDG_STATE_HOME=str(root / "state"), MSIME_IBUS_OPTIONS=str(options),
        )
        # 传入的验收配置只使用合成目录；基础门禁不需要词库或用户配置。
        if args.options:
            assert os.environ.get("MSIME_ISOLATED_LINUX_TEST") == "1", "真实词库验收仅在隔离容器中运行"
        os.environ.pop("IBUS_ADDRESS", None)
        daemon = subprocess.Popen([
            "ibus-daemon", "--single", "--panel=disable", "--config=disable",
            "--emoji-extension=disable", "--cache=none", "--address", f"unix:tmpdir={root}",
        ], start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            deadline = time.monotonic() + 5
            while True:
                assert daemon.poll() is None, "IBus daemon 提前退出"
                assert time.monotonic() < deadline, "IBus daemon 未创建套接字"
                address = subprocess.run(["ibus", "address"], stdout=subprocess.PIPE,
                                         stderr=subprocess.DEVNULL, text=True, timeout=2).stdout.strip()
                if address and address != "(null)":
                    os.environ["IBUS_ADDRESS"] = address
                    break
                time.sleep(.02)
            IBus.init()
            bus = IBus.Bus.new()
            assert bus.is_connected(), "无法连接隔离 IBus daemon"
            assert engine in [entry.get_name() for entry in bus.list_engines()]
            for selection in range(2):
                # 第一次选择必须完成原始静态组件等待，不能靠第二次重试选中动态注册的副本。
                bus.get_connection().call_sync(
                    "org.freedesktop.IBus", "/org/freedesktop/IBus", "org.freedesktop.IBus",
                    "SetGlobalEngine", GLib.Variant("(s)", (engine,)), None,
                    Gio.DBusCallFlags.NONE, 5000, None,
                )
                assert bus.get_global_engine().get_name() == engine
                print(f"IBus selection {selection + 1} succeeded", flush=True)
            if args.options:
                context = bus.create_input_context("msime-activation-test")
                context.set_capabilities(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS)
                committed = []
                context.connect("commit-text", lambda _, text: committed.append(text.get_text()))
                context.focus_in()
                for letter in "nihao":
                    assert context.process_key_event(ord(letter), 0, 0)
                assert context.process_key_event(IBus.KEY_space, 0, 0)
                deadline = time.monotonic() + 5
                while not committed and time.monotonic() < deadline:
                    while GLib.MainContext.default().iteration(False):
                        pass
                    time.sleep(.01)
                assert committed == ["你好"], "没有提交预期的合成中文候选"
                context.destroy()
                print("IBus first activation accepts and commits synthetic pinyin", flush=True)
        finally:
            try:
                os.killpg(daemon.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            output, _ = daemon.communicate(timeout=5)
            if sys.exc_info()[0] is not None:
                print(output, file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

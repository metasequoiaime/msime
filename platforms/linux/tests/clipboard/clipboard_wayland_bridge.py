#!/usr/bin/env python3
"""在独立无窗口 Sway 中验证真实 data-control 选区和双向云同步。"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from msime_clipboard_cloud import CloudAutoSync, NativeWaylandClipboard

parser = argparse.ArgumentParser()
parser.add_argument("binary")
parser.add_argument("--owner", required=True)
parser.add_argument("--crash-controller")
parser.add_argument("--version", default="synthetic")
parser.add_argument("--client-directory", default="msime-client")
parser.add_argument("--telemetry-directory", default="msime")
OPTIONS, remaining = parser.parse_known_args()
sys.argv = [sys.argv[0], *remaining]
BINARY = str(Path(OPTIONS.binary).resolve())


def wait_for(predicate, message, timeout=5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.01)
    raise AssertionError(message)


class WaylandBridge(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="msime-wayland-cloud-")
        cls.root = Path(cls.temporary.name)
        cls.env = {key: value for key, value in os.environ.items()
                   if key not in ("DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK")}
        cls.env.update(XDG_RUNTIME_DIR=str(cls.root), WLR_BACKENDS="headless",
                       WLR_LIBINPUT_NO_DEVICES="1", WLR_RENDERER="pixman")
        cls.errors = (cls.root / "sway.log").open("wb")
        cls.sway = subprocess.Popen(["sway", "-c", "/dev/null"], env=cls.env,
                                    stdout=cls.errors, stderr=cls.errors)
        try:
            wait_for(lambda: any(path.is_socket() for path in cls.root.glob("wayland-*")), "独立合成器未启动")
            cls.env["WAYLAND_DISPLAY"] = next(path.name for path in cls.root.glob("wayland-*")
                                              if not path.name.endswith(".lock"))
        except BaseException:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        cls.sway.terminate()
        cls.sway.wait(timeout=5)
        cls.errors.close()
        cls.temporary.cleanup()

    def setUp(self):
        self.owners = []
        self.process = subprocess.Popen([BINARY], env=self.env, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.bridge = NativeWaylandClipboard(self.process)
        self.addCleanup(self.stop)
        wait_for(lambda: self.ready(), "原生通道未交付初始选区")
        self.local("合成本机基线")
        self.page = {"enabled": True, "items": []}
        self.calls = []
        self.during_list = lambda: None
        self.binding = {"source": "account", "login": {"user_id": "synthetic", "session_id": "one"}}
        self.sync = CloudAutoSync(self.request)
        self.poll()

    def ready(self):
        self.bridge.read()
        self.assertIsNone(self.process.poll(), "原生通道提前退出")
        return self.bridge.generation is not None and not self.bridge.pending

    def stop(self):
        self.process.terminate()
        self.process.wait(timeout=5)
        self.process.stdin.close()
        self.process.stdout.close()
        for owner in self.owners:
            if owner.poll() is None:
                owner.terminate()
            owner.wait(timeout=5)

    def local(self, text, mime="text/plain;charset=utf-8"):
        before = self.bridge.generation
        owner = subprocess.Popen(["wl-copy", "--foreground", "--type", mime], env=self.env,
                                 stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.owners.append(owner)
        owner.stdin.write(text.encode())
        owner.stdin.close()
        wait_for(lambda: self.ready() and self.bridge.generation != before,
                 "本机选区未改变")

    def request(self, action):
        self.calls.append(action)
        if action["operation"] == "binding":
            return self.binding.copy()
        if action["operation"] == "list":
            self.during_list()
            return self.page
        return {}

    def poll(self):
        self.sync.poll(self.bridge.read, self.bridge.write,
                       revision=lambda: self.bridge.generation, remote=lambda: self.bridge.remote)

    def remote(self, second, text="合成远端新复制"):
        self.page["items"] = [{"text": text, "updated_at": f"2026-11-14T00:00:{second:02d}Z"}]

    def uploads(self):
        return [call for call in self.calls if call["operation"] == "add"]

    def system_text(self):
        return subprocess.check_output(["wl-paste", "--no-newline", "--type", "text"], env=self.env).decode()

    def test_remote_write_is_confirmed_and_never_uploaded_back(self):
        self.remote(1)
        self.poll()
        self.assertTrue(self.bridge.remote)
        self.assertEqual(self.system_text(), "合成远端新复制")
        self.poll()
        self.assertEqual(self.uploads(), [])

    def test_local_copy_during_list_wins(self):
        self.remote(1)
        self.during_list = lambda: self.local("合成并发本机复制")
        self.poll()
        self.assertEqual(self.system_text(), "合成并发本机复制")
        self.assertEqual([call["text"] for call in self.uploads()], ["合成并发本机复制"])

    def test_same_text_copied_again_during_list_preserves_local_selection(self):
        self.remote(1)
        self.during_list = lambda: self.local("合成本机基线")
        self.poll()
        self.assertFalse(self.bridge.remote)
        self.assertEqual(self.system_text(), "合成本机基线")

    def test_image_invalidates_text_snapshot_and_blocks_receive(self):
        self.remote(1)
        self.during_list = lambda: self.local("synthetic-image-data", "image/png")
        self.poll()
        self.assertIsNone(self.bridge.read())
        self.assertEqual(self.uploads(), [])
        self.assertIn(b"image/png", subprocess.check_output(["wl-paste", "--list-types"], env=self.env))

    def test_stale_generation_is_rejected_even_when_text_matches(self):
        old = self.bridge.generation
        self.local("合成本机基线")
        command = {"request": 99, "generation": old, "text": "合成不应写入"}
        self.process.stdin.write((json.dumps(command) + "\n").encode())
        self.process.stdin.flush()
        wait_for(lambda: self.ready() and self.bridge.receipt == (99, False), "缺少过期代次回执")
        self.assertEqual(self.system_text(), "合成本机基线")

    def test_local_same_text_after_receive_is_a_new_selection(self):
        self.remote(1)
        self.poll()
        self.local("合成远端新复制")
        self.assertFalse(self.bridge.remote)
        self.remote(2, "合成第二条远端")
        self.poll()
        self.assertEqual(self.system_text(), "合成远端新复制")
        self.remote(3, "合成第三条远端")
        self.poll()
        self.assertEqual(self.system_text(), "合成第三条远端")

    def test_full_size_text_is_served_without_pipe_truncation(self):
        text = "杉" * 4000
        self.remote(1, text)
        self.poll()
        self.assertEqual(self.system_text(), text)
        self.assertEqual(self.bridge.read(), text)

    def test_oversize_and_sensitive_selection_do_not_reuse_previous_text(self):
        for text, mime in [("杉" * 4001, "text/plain"),
                           ("合成敏感内容", "x-kde-passwordManagerHint")]:
            self.local(text, mime)
            self.assertIsNone(self.bridge.read())
            self.assertFalse(self.bridge.write("合成不应覆盖"))

    def test_sensitive_mime_with_plain_text_is_never_read_or_overwritten(self):
        before = self.bridge.generation
        owner = subprocess.Popen([str(Path(OPTIONS.owner).resolve()), "text/plain;charset=utf-8",
                                  "x-kde-passwordManagerHint"], env=self.env, stdin=subprocess.PIPE,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.owners.append(owner)
        owner.stdin.write("合成敏感文本".encode())
        owner.stdin.close()
        wait_for(lambda: self.ready() and self.bridge.generation != before, "敏感选区未交付")
        self.assertIsNone(self.bridge.read())
        self.assertFalse(self.bridge.write("合成不应覆盖"))
        self.assertEqual(self.system_text(), "合成敏感文本")

    def test_parent_monitor_uses_native_receive_and_upload_without_echo(self):
        directory = self.root / self.id().rsplit(".", 1)[-1]
        directory.mkdir()
        for name in ("msime-linux-clipboard-monitor", "msime_clipboard_cloud.py", "msime_crash_capture.py"):
            shutil.copyfile(ROOT / "scripts" / name, directory / name)
        (directory / "msime-linux-clipboard-watch-wayland").symlink_to(BINARY)
        preferences = directory / "preferences.json"
        preferences.write_text(json.dumps({"format_version": 1, "preferences": {"clipboard_history": False}}))
        options = directory / "runtime.json"
        options.write_text(json.dumps({"preferences_directory": str(directory)}))
        page = directory / "page.json"
        page.write_text(json.dumps({"enabled": True, "items": []}))
        calls = directory / "calls.jsonl"
        helper = directory / "msime-linux-cloud-clipboard-sync"
        helper.write_text("#!" + sys.executable + "\n" +
            "import json,sys\nfrom pathlib import Path\n" +
            "action=json.load(sys.stdin)\n" +
            f"with Path({str(calls)!r}).open('a') as output: output.write(json.dumps(action)+'\\n')\n" +
            "binding={'source':'account','login':{'user_id':'synthetic','session_id':'one'}}\n" +
            f"print(json.dumps(binding if action['operation']=='binding' else json.loads(Path({str(page)!r}).read_text())))\n")
        helper.chmod(0o700)
        monitor = subprocess.Popen([sys.executable, str(directory / "msime-linux-clipboard-monitor"), str(options)],
                                   env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        def operations():
            return [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
        try:
            wait_for(lambda: sum(action["operation"] == "binding" for action in operations()) >= 4,
                     "父监视器未建立绑定基线")
            page.write_text(json.dumps({"enabled": True, "items": [{"text": "合成父进程接收",
                                         "updated_at": "2026-11-14T00:00:01Z"}]}))
            wait_for(lambda: self.system_text() == "合成父进程接收", "父监视器没有接收云复制", timeout=8)
            self.local("合成父进程本机新复制")
            wait_for(lambda: any(action["operation"] == "add" for action in operations()),
                     "父监视器没有上传本机复制", timeout=8)
            self.assertEqual([action["text"] for action in operations() if action["operation"] == "add"],
                             ["合成父进程本机新复制"])
        finally:
            monitor.terminate()
            monitor.wait(timeout=5)

    @unittest.skipUnless(OPTIONS.crash_controller, "未提供真实崩溃采集控制器")
    def test_native_helper_keeps_crash_opt_in_and_private_text_out_of_reports(self):
        subprocess.run([sys.executable, str(ROOT / "tests/core/auxiliary_crash_capture.py"),
                        "--controller", str(Path(OPTIONS.crash_controller).resolve()),
                        "--version", OPTIONS.version, "--client-directory", OPTIONS.client_directory,
                        "--telemetry-directory", OPTIONS.telemetry_directory,
                        "--binary", "msime-linux-clipboard-watch-wayland", BINARY],
                       env=self.env, check=True, timeout=45)


if __name__ == "__main__":
    if os.getuid() == 0:
        # Sway 拒绝以 root 启动；整个测试降权，既不接入登录桌面也不改它的剪贴板。
        os.setgroups([])
        os.setgid(65534)
        os.setuid(65534)
    if not shutil.which("sway") or not shutil.which("wl-copy"):
        raise SystemExit("测试需要 sway 与 wl-clipboard")
    unittest.main()

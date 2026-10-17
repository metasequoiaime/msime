#!/usr/bin/env python3
"""在线 provider 与 host 库之间的契约：按安装路径找库、要的入口都在 msime_client.h 里且是同一个签名、注册和释义带同一个 Linux client_agent。

不依赖构建产物，列在 tests/tools/python-contracts.list 里，legacy 包在 Python 3.7 上也跑。设了 MSIME_HOST_LIBRARY 时再用真的库核对 provider 发出的请求形状被 Rust 接受（只走不联网的分支）。
"""
import ctypes
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import re
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader("online_provider_host_library", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
loader.exec_module(provider)

HEADER = (REPOSITORY / "crates" / "host-api" / "include" / "msime_client.h").read_text(encoding="utf-8")


class HeaderContract(unittest.TestCase):
    def test_every_required_entry_point_takes_a_request_and_its_length(self):
        # ctypes 按 (c_char_p, c_size_t) -> c_void_p 调它们；头文件里换了签名，Python 这边不会报错，只会把指针或长度传错。
        self.assertIn("msime_client_account", provider.HOST_LIBRARY_SYMBOLS)
        for name in provider.HOST_LIBRARY_SYMBOLS:
            with self.subTest(symbol=name):
                self.assertRegex(HEADER, r"(?m)^char \*" + re.escape(name) +
                                 r"\(const uint8_t \*\w+, size_t \w+\);$")
        self.assertRegex(HEADER, r"(?m)^void msime_client_string_free\(char \*\w+\);$")

    def test_the_provider_no_longer_sends_account_requests_itself(self):
        # 释义的请求、会话和令牌都归 Rust：provider 不再取令牌、不再自己拼或解读 /v1/translate。
        source = Path(provider.__file__).read_text(encoding="utf-8")
        for retired in ("msime_client_account_access_token", "msime_client_account_gloss",
                        "https://api.msime.app", "/v1/translate\""):
            with self.subTest(retired=retired):
                self.assertNotIn(retired, source)

    def test_the_installed_paths_module_carries_library_version_and_edition(self):
        # provider 按 msime_voice_paths.py 找库、取版本号和版本 ID，两者都装进 bin。
        template = (ROOT / "data" / "msime_voice_paths.py.in").read_text(encoding="utf-8")
        for name, variable in (("HOST_LIBRARY", "MSIME_HOST_LIBRARY_RELATIVE"),
                               ("VERSION", "MSIME_LINUX_VERSION"), ("EDITION", "MSIME_EDITION")):
            with self.subTest(name=name):
                self.assertIn(name + ' = r"@' + variable + '@"', template)
        cmake = (ROOT / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertIn("file(RELATIVE_PATH MSIME_HOST_LIBRARY_RELATIVE \"${CMAKE_INSTALL_FULL_BINDIR}\"", cmake)
        self.assertIn("configure_file(data/msime_voice_paths.py.in msime_voice_paths.py @ONLY)", cmake)
        self.assertIn('install(FILES "${CMAKE_CURRENT_BINARY_DIR}/msime_voice_paths.py" DESTINATION "${CMAKE_INSTALL_BINDIR}")', cmake)
        self.assertRegex(cmake, r"foreach\(MSIME_EDITION_SCRIPT [^)]*msime-linux-online-provider")


class FakeFunction:
    pass


class LibraryLoading(unittest.TestCase):
    def fake_library(self, missing=None):
        library = SimpleNamespace(msime_client_string_free=FakeFunction())
        for name in provider.HOST_LIBRARY_SYMBOLS:
            if name != missing:
                setattr(library, name, FakeFunction())
        return library

    def test_a_library_without_msime_client_account_is_not_used(self):
        with mock.patch.object(provider.ctypes, "CDLL", return_value=self.fake_library("msime_client_account")):
            self.assertIsNone(provider.RustAccountApi("/synthetic/account")._load())
        library = self.fake_library()
        with mock.patch.object(provider.ctypes, "CDLL", return_value=library):
            self.assertIs(provider.RustAccountApi("/synthetic/account")._load(), library)
        self.assertEqual(library.msime_client_account.argtypes, [ctypes.c_char_p, ctypes.c_size_t])
        self.assertIs(library.msime_client_account.restype, ctypes.c_void_p)

class HostLibraryAndClientAgent(unittest.TestCase):
    def test_the_installed_library_path_is_tried_first(self):
        paths = SimpleNamespace(HOST_LIBRARY="../lib64/msime-client/libmsime_host_api.so",
                                VERSION="1.4.0", EDITION="wubi")
        tried = []

        def refuse(path):
            tried.append(str(path))
            raise OSError("synthetic missing library")

        with mock.patch.dict(sys.modules, {"msime_voice_paths": paths}), \
                mock.patch.dict(provider.os.environ, {"MSIME_HOST_LIBRARY": "/synthetic/configured.so"}), \
                mock.patch.object(provider.ctypes, "CDLL", side_effect=refuse):
            self.assertIsNone(provider.RustAccountApi("/synthetic/account")._load())
        installed = Path(provider.__file__).resolve().parent / "../lib64/msime-client/libmsime_host_api.so"
        self.assertEqual(tried[:2], [str(installed), "/synthetic/configured.so"])

    def test_registration_reports_the_linux_client_agent(self):
        api = provider.RustAccountApi("/synthetic/account")
        calls = []

        def call(name, payload):
            calls.append((name, payload))
            if name == "msime_client_client_agent":
                return {"ok": True, "value": "msime-linux/1.4.0 (unknown; Synthetic 1; edition=wubi)"}
            return {"ok": True, "value": True}

        paths = SimpleNamespace(HOST_LIBRARY="x", VERSION="1.4.0", EDITION="wubi")
        with mock.patch.dict(sys.modules, {"msime_voice_paths": paths}), \
                mock.patch.object(provider, "linux_system", return_value="Synthetic 1"), \
                mock.patch.object(api, "_call", side_effect=call):
            self.assertTrue(api.ensure_anonymous())
            self.assertTrue(api.ensure_anonymous())
        self.assertEqual(calls[0], ("msime_client_client_agent", {
            "platform": "linux", "version": "1.4.0", "device": None, "system": "Synthetic 1", "edition": "wubi"}))
        registration = {"directory": "/synthetic/account",
                        "client_agent": "msime-linux/1.4.0 (unknown; Synthetic 1; edition=wubi)"}
        # 拼一次就记住，之后每次惰性注册都带同一个 User-Agent。
        self.assertEqual(calls[1:], [("msime_client_ensure_anonymous_account_with_agent", registration)] * 2)

    def test_registration_without_an_agent_still_registers(self):
        api = provider.RustAccountApi("/synthetic/account")
        responses = {"msime_client_client_agent": {"ok": False, "error": "account_invalid"},
                     "msime_client_ensure_anonymous_account_with_agent": {"ok": True, "value": True}}
        with mock.patch.object(api, "_call", side_effect=lambda name, payload: responses[name]) as call:
            self.assertTrue(api.ensure_anonymous())
        self.assertEqual(call.call_args_list[-1].args,
                         ("msime_client_ensure_anonymous_account_with_agent", {"directory": "/synthetic/account"}))

    def test_system_comes_from_os_release(self):
        with tempfile.TemporaryDirectory() as directory:
            release = Path(directory) / "os-release"
            release.write_text('NAME="Synthetic Linux"\nVERSION_ID=24.04\nID=synthetic\n')
            real_open = open
            with mock.patch("builtins.open", side_effect=lambda path, *args, **kwargs: real_open(
                    release if path == "/etc/os-release" else path, *args, **kwargs)):
                self.assertEqual(provider.linux_system(), "Synthetic Linux 24.04")


    def test_translation_carries_the_registration_agent(self):
        # 头文件要求同一个宿主的各次 msime_client_account 调用都带同一个 client_agent，注册与释义用的是同一次拼出的那个。
        api = provider.RustAccountApi("/synthetic/account")
        calls = []
        agent = "msime-linux/1.4.0 (unknown; Synthetic 1; edition=wubi)"

        def call(name, payload):
            calls.append((name, payload))
            if name == "msime_client_client_agent":
                return {"ok": True, "value": agent}
            if name == "msime_client_account":
                return {"ok": False, "error": "account_unauthorized"} if len(calls) == 2 else \
                    {"ok": True, "value": {"translations": ["sequoia"]}}
            return {"ok": True, "value": True}

        with mock.patch.object(api, "_call", side_effect=call):
            self.assertEqual(api.translate(["水杉"], "zh", "en", provider.time.monotonic() + 6), ["sequoia"])
        self.assertEqual([name for name, _ in calls], [
            "msime_client_client_agent", "msime_client_account",
            "msime_client_ensure_anonymous_account_with_agent", "msime_client_account"])
        self.assertEqual({payload["client_agent"] for _, payload in calls[1:]}, {agent})


@unittest.skipUnless(os.environ.get("MSIME_HOST_LIBRARY"), "MSIME_HOST_LIBRARY is not set")
class RealLibrary(unittest.TestCase):
    """用真的 host 库核对 provider 发出的请求形状。只走不联网的分支：拼 client_agent，以及空目录里的 translate（没有会话，Rust 不联网就答 account_unauthorized）。"""

    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="msime-host-library-contract-")
        self.addCleanup(directory.cleanup)
        self.api = provider.RustAccountApi(directory.name)
        # 只认环境变量指的那个库，不让安装路径或源码树里别的库顶上。
        with mock.patch.object(provider, "installed_host_library", return_value=None):
            self.assertIsNotNone(self.api._load(), os.environ["MSIME_HOST_LIBRARY"])

    def test_rust_builds_the_linux_agent_from_the_provider_request(self):
        agent = self.api.client_agent()
        self.assertRegex(agent, r"^msime-linux/[0-9A-Za-z.+_-]+ \(unknown; [^;()]+; edition=[a-z0-9-]+\)$")

    def test_rust_accepts_the_translate_request_and_answers_without_a_session(self):
        replies = []
        real_call = self.api._call

        def record(name, payload):
            reply = real_call(name, payload)
            replies.append((name, reply))
            return reply

        # 让补注册这一步什么也不做：它会联网。
        with mock.patch.object(self.api, "_call", side_effect=record), \
                mock.patch.object(self.api, "_may_register", return_value=False):
            self.assertIsNone(self.api.translate(["水杉", "测试"], "zh", "en", provider.time.monotonic() + 6))
        translate = [reply for name, reply in replies if name == "msime_client_account"]
        # account_invalid 说明 provider 拼的请求 Rust 不认；这里只能是没有会话。
        self.assertEqual(translate, [{"ok": False, "error": "account_unauthorized"}])


if __name__ == "__main__":
    unittest.main()

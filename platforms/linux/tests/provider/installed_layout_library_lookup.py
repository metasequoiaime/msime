#!/usr/bin/env python3
"""The provider finds its host library when it is installed in /usr/bin, not only in the source tree."""
import importlib.machinery
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "installed_layout_online_provider", str(ROOT / "scripts" / "msime-linux-online-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
online = importlib.util.module_from_spec(spec)
loader.exec_module(online)


class InstalledLayoutLibraryLookup(unittest.TestCase):
    def test_usr_bin_install_builds_the_candidate_list_without_a_source_tree(self):
        # 装到 /usr/bin 时 Path(__file__).resolve().parents 只有三层；以前按第四层拼开发构建的路径，候选表还没建完就抛 IndexError，postinst 的 --ensure-anonymous-account 跟着崩溃。
        tried = []

        def refuse(path, *args, **kwargs):
            tried.append(str(path))
            raise OSError("synthetic: no library here")

        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.dict(online.__dict__, {"__file__": "/usr/bin/msime-linux-online-provider"}), \
                mock.patch.dict(os.environ, {}, clear=False), \
                mock.patch.object(online.ctypes, "CDLL", side_effect=refuse):
            os.environ.pop("MSIME_HOST_LIBRARY", None)
            api = online.RustAccountApi(directory)
            self.assertIsNone(api._load())
        # 已安装布局的两个库路径仍按顺序尝试过，只是这里都加载不到。
        self.assertEqual(tried[:2], [
            "/usr/lib/msime-client/libmsime_host_api.so",
            "/usr/lib/msime-client/libmsime_host_api_full.so",
        ])
        self.assertFalse(any("target" in path for path in tried))

    def test_usr_bin_install_tries_the_debian_multiarch_library_directory(self):
        # 旧版安装包没有 msime_voice_paths.py 时，Debian 的 host 库仍在 /usr/lib/<multiarch>/msime-client。
        tried = []

        def refuse(path, *args, **kwargs):
            tried.append(str(path))
            raise OSError("synthetic: no library here")

        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.dict(online.__dict__, {"__file__": "/usr/bin/msime-linux-online-provider"}), \
                mock.patch.dict(os.environ, {}, clear=False), \
                mock.patch.object(online, "installed_host_library", return_value=None), \
                mock.patch.object(online.sysconfig, "get_config_var", return_value="x86_64-linux-gnu"), \
                mock.patch.object(online.ctypes, "CDLL", side_effect=refuse):
            os.environ.pop("MSIME_HOST_LIBRARY", None)
            api = online.RustAccountApi(directory)
            self.assertIsNone(api._load())
        self.assertEqual(tried[:4], [
            "/usr/lib/msime-client/libmsime_host_api.so",
            "/usr/lib/msime-client/libmsime_host_api_full.so",
            "/usr/lib/x86_64-linux-gnu/msime-client/libmsime_host_api.so",
            "/usr/lib/x86_64-linux-gnu/msime-client/libmsime_host_api_full.so",
        ])


if __name__ == "__main__":
    unittest.main()

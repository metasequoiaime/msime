import importlib.metadata
import importlib.util
import inspect
from importlib.machinery import SourceFileLoader
import json
import pathlib
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = SourceFileLoader("msime_voice_provider", str(ROOT / "scripts" / "msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)
from msime_voice_doubao import doubao_headers, normalize_doubao_auth_mode, websocket_dependency


def websockets_stub(installed, arguments=None, receive_timeout=True):
    """Install a synthetic websockets distribution whose sync client takes the given keywords."""
    if arguments is None:
        arguments = ("additional_headers", "user_agent_header", "open_timeout", "close_timeout",
                     "ping_interval", "ping_timeout", "max_size", "max_queue", "compression",
                     "logger", "create_connection")
    keyword = lambda name: inspect.Parameter(name, inspect.Parameter.KEYWORD_ONLY, default=None)
    positional = lambda name: inspect.Parameter(name, inspect.Parameter.POSITIONAL_OR_KEYWORD)
    connect = lambda *args, **kwargs: None
    connect.__signature__ = inspect.Signature([positional("uri")] + [keyword(a) for a in arguments])
    recv = lambda *args, **kwargs: None
    recv.__signature__ = inspect.Signature(
        [positional("self")] + ([positional("timeout")] if receive_timeout else []))
    client = types.ModuleType("websockets.sync.client")
    client.connect = connect
    client.ClientConnection = type("ClientConnection", (), {"recv": recv})
    sync = types.ModuleType("websockets.sync")
    sync.client = client
    package = types.ModuleType("websockets")
    package.sync = sync

    def version(name):
        if name != "websockets" or installed is None:
            raise importlib.metadata.PackageNotFoundError(name)
        return installed

    modules = {"websockets": package, "websockets.sync": sync, "websockets.sync.client": client}
    return mock.patch.dict(sys.modules, modules), mock.patch("importlib.metadata.version", version)


class WebsocketDependency(unittest.TestCase):
    def check(self, *args, **kwargs):
        modules, version = websockets_stub(*args, **kwargs)
        with modules, version:
            return websocket_dependency()

    def test_accepts_any_release_with_the_sync_client_features(self):
        for installed in ("15.0", "15.0.1", "16.0"):
            with self.subTest(installed=installed):
                connection, connect = self.check(installed)
                self.assertEqual(connection.__name__, "ClientConnection")
                self.assertTrue(callable(connect))

    def test_rejects_releases_before_sync_keepalive(self):
        # 14.x has a sync client and recv(timeout=) but not ping_interval/ping_timeout.
        for installed in ("10.4", "12.0", "14.2"):
            with self.subTest(installed=installed):
                with self.assertRaisesRegex(RuntimeError, "websockets>=15"):
                    self.check(installed)

    def test_rejects_missing_package_or_features(self):
        with self.assertRaisesRegex(RuntimeError, "websockets>=15"):
            self.check(None)
        with self.assertRaises(RuntimeError):
            self.check("15.0.1", arguments=("additional_headers", "open_timeout"))
        with self.assertRaises(RuntimeError):
            self.check("15.0.1", receive_timeout=False)


class DoubaoAuthentication(unittest.TestCase):
    def test_only_a_named_legacy_mode_is_legacy(self):
        self.assertEqual(normalize_doubao_auth_mode("api_key"), "api_key")
        self.assertEqual(normalize_doubao_auth_mode("legacy"), "legacy")
        self.assertEqual(normalize_doubao_auth_mode("unknown"), "api_key")
        self.assertEqual(normalize_doubao_auth_mode(""), "api_key")
        self.assertEqual(normalize_doubao_auth_mode(None), "api_key")

        common = {"resource_id": "fixture-resource", "token": "fixture-token",
                  "app_key": "fixture-app", "doubao_auth_mode": "api_key"}
        headers = doubao_headers(common, "fixture-request")
        self.assertEqual(headers["X-Api-Key"], "fixture-token")
        self.assertNotIn("X-Api-App-Key", headers)
        self.assertNotIn("X-Api-Access-Key", headers)
        common["doubao_auth_mode"] = "legacy"
        headers = doubao_headers(common, "fixture-request")
        self.assertEqual(headers["X-Api-App-Key"], "fixture-app")
        self.assertEqual(headers["X-Api-Access-Key"], "fixture-token")
        self.assertNotIn("X-Api-Key", headers)

    def test_private_config_normalizes_and_validates_modes(self):
        def read_config(asr):
            with tempfile.TemporaryDirectory() as directory:
                path = pathlib.Path(directory) / "voice.json"
                path.write_text(json.dumps({"asr": asr}), encoding="utf-8")
                path.chmod(0o600)
                return provider.load_config(path)["asr"]

        api = read_config({"provider": "doubao", "token": "fixture-token"})
        self.assertEqual(api["doubao_auth_mode"], "api_key")
        # An App ID alone does not select the legacy console.
        absent = read_config({"provider": "doubao", "token": "fixture-token",
                              "app_key": "fixture-app"})
        self.assertEqual(absent["doubao_auth_mode"], "api_key")
        legacy = read_config({"provider": "doubao", "token": "fixture-token",
                              "app_key": "fixture-app", "doubao_auth_mode": "legacy"})
        self.assertEqual(legacy["doubao_auth_mode"], "legacy")
        explicit = read_config({"provider": "doubao", "token": "fixture-token",
                                "app_key": "stale-app", "doubao_auth_mode": "api_key"})
        self.assertEqual(explicit["doubao_auth_mode"], "api_key")
        with self.assertRaises(ValueError):
            read_config({"provider": "doubao", "token": "fixture-token",
                         "doubao_auth_mode": "legacy"})

        normalized = read_config({"provider": "doubao", "token": " fixture-token\n",
                                  "app_key": " fixture-app\t",
                                  "resource_id": " fixture-resource\r\n"})
        self.assertEqual(normalized["token"], "fixture-token")
        self.assertEqual(normalized["app_key"], "fixture-app")
        self.assertEqual(normalized["resource_id"], "fixture-resource")

    def test_doubao_headers_normalize_direct_config_values(self):
        headers = doubao_headers({"resource_id": " fixture-resource\n",
                                  "token": " fixture-token\r\n",
                                  "app_key": " fixture-app\t",
                                  "doubao_auth_mode": "legacy"}, "fixture-request")
        self.assertEqual(headers["X-Api-Resource-Id"], "fixture-resource")
        self.assertEqual(headers["X-Api-App-Key"], "fixture-app")
        self.assertEqual(headers["X-Api-Access-Key"], "fixture-token")


if __name__ == "__main__":
    unittest.main()

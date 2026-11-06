#!/usr/bin/env python3
"""Synthetic credential-test dispatch; no real credentials or network calls."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def load(name, filename):
    loader = importlib.machinery.SourceFileLoader(name, str(ROOT / "scripts" / filename))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


online = load("credential_online_provider", "msime-linux-online-provider")
voice = load("credential_voice_provider", "msime-linux-voice-provider")


class OnlineCredentialTest(unittest.TestCase):
    def setUp(self):
        self.server = SimpleNamespace(ai_config_path=Path("/private/ai.json"),
                                      tencent_config_path=Path("/private/tencent.json"))

    def test_ai_uses_private_credential_bound_to_current_options(self):
        private = {"provider": "deepseek", "endpoint": "https://fixture.invalid/chat",
                   "model": "fixture-model", "token": "fixture-private-token"}
        query = {"service": "ai.assistant", "config": {
            "provider": "deepseek", "endpoint": private["endpoint"], "model": private["model"]}}
        with mock.patch.object(online, "load_ai_config", return_value=private), \
                mock.patch.object(online, "fetch", return_value={"choices": [{"message": {}}]}) as fetch:
            result = online.credential_test(query, self.server)
        self.assertTrue(result["ok"])
        self.assertEqual(fetch.call_args.args[3], "fixture-private-token")
        query["config"]["model"] = "other-model"
        with mock.patch.object(online, "load_ai_config", return_value=private), \
                mock.patch.object(online, "fetch") as fetch:
            result = online.credential_test(query, self.server)
        self.assertFalse(result["ok"])
        fetch.assert_not_called()

    def test_ai_rejects_empty_choices_and_error_objects(self):
        private = {"provider": "openai", "endpoint": "https://fixture.invalid/chat",
                   "model": "fixture-model", "token": "fixture-private-token"}
        query = {"service": "ai.assistant", "config": {
            "provider": "openai", "endpoint": private["endpoint"], "model": private["model"]}}
        for response in ({"choices": []}, {"error": {"message": "invalid"}}):
            with self.subTest(response=response), \
                    mock.patch.object(online, "load_ai_config", return_value=private), \
                    mock.patch.object(online, "fetch", return_value=response):
                self.assertFalse(online.credential_test(query, self.server)["ok"])

    def test_ai_private_tokens_normalize_pasted_whitespace(self):
        configuration = {
            "provider": "deepseek",
            "endpoint": "\thttps://fixture.invalid/chat\r\n",
            "model": " fixture-model \t",
            "token": " \tfixture-private-token\r\n",
            "profiles": {
                "openai": {
                    "endpoint": "https://fixture.invalid/chat",
                    "model": "fixture-model",
                    "token": " fixture-profile-token\n",
                }
            },
        }
        with tempfile.TemporaryDirectory(prefix="msime-ai-") as directory:
            path = Path(directory) / "ai.json"
            path.write_text(json.dumps(configuration), encoding="utf-8")
            path.chmod(0o600)
            loaded = online.load_ai_config(path)
        self.assertEqual(loaded["token"], "fixture-private-token")
        self.assertEqual(loaded["endpoint"], "https://fixture.invalid/chat")
        self.assertEqual(loaded["model"], "fixture-model")
        self.assertEqual(loaded["profiles"]["openai"]["token"], "fixture-profile-token")

    def test_ai_retired_ark_model_bindings_read_as_their_successors(self):
        ark = "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
        gateway = "https://gateway.example/v1/chat/completions"
        configuration = {"profiles": {
            "volcengine": {"endpoint": ark, "model": " doubao-seed-1.6-flash ", "token": "fixture-ark-token"},
            "custom": {"endpoint": gateway, "model": "doubao-seed-1.6", "token": "fixture-gateway-token"},
        }}
        with tempfile.TemporaryDirectory(prefix="msime-ai-") as directory:
            path = Path(directory) / "ai.json"

            def write(document):
                path.write_text(json.dumps(document), encoding="utf-8")
                path.chmod(0o600)

            write(configuration)
            loaded = online.load_ai_config(path)
            self.assertEqual(loaded["profiles"]["volcengine"]["model"], "doubao-seed-2-0-mini-260428")
            self.assertEqual(loaded["profiles"]["volcengine"]["token"], "fixture-ark-token")
            # 别的服务商下的同名值不动。
            self.assertEqual(loaded["profiles"]["custom"]["model"], "doubao-seed-1.6")
            # 偏好读出来的接替模型和凭据对得上，请求发的也是接替模型，不用重新保存。
            options = {"provider": "volcengine", "endpoint": ark, "model": "doubao-seed-2-0-mini-260428"}
            with mock.patch.object(online, "fetch", return_value={"choices": [{"message": {}}]}) as fetch:
                self.assertTrue(online.ai_credential_test(options, path))
            self.assertEqual(fetch.call_args.args[2]["model"], "doubao-seed-2-0-mini-260428")
            self.assertEqual(fetch.call_args.args[2]["thinking"], {"type": "disabled"})
            self.assertEqual(fetch.call_args.args[3], "fixture-ark-token")
            # 火山方舟槽位指向别家网关时，那边可能真有同名模型，不换。
            write({"profiles": {"volcengine": {
                "endpoint": gateway, "model": "doubao-seed-1.6", "token": "fixture-ark-token"}}})
            self.assertEqual(online.load_ai_config(path)["profiles"]["volcengine"]["model"], "doubao-seed-1.6")
            # 主机只是以 volces.com 开头的别家地址也不换。
            write({"profiles": {"volcengine": {
                "endpoint": "https://volces.com.example/v1/chat/completions",
                "model": "doubao-seed-1.6", "token": "fixture-ark-token"}}})
            self.assertEqual(online.load_ai_config(path)["profiles"]["volcengine"]["model"], "doubao-seed-1.6")
        # 文件本身不改写。
        self.assertEqual(configuration["profiles"]["volcengine"]["model"], " doubao-seed-1.6-flash ")

    def test_ark_seed_requests_turn_off_thinking(self):
        ark = "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
        for provider, endpoint, model, disabled in (
                ("volcengine", ark, "doubao-seed-2-0-mini-260428", True),
                ("custom", "https://ark.cn-shanghai.volces.com/api/v3/chat/completions",
                 "Doubao-Seed-2-0-lite-260428", True),
                ("volcengine", ark, "glm-5-3-flash-260828", False),
                ("volcengine", ark, "ep-20261105123456-abcde", False),
                ("volcengine", "https://gateway.example/v1/chat/completions", "doubao-seed-2-0-mini-260428", False),
                ("custom", "https://volces.com.example/v1/chat/completions", "doubao-seed-2-0-mini-260428", False)):
            with self.subTest(endpoint=endpoint, model=model):
                config = {"provider": provider, "endpoint": endpoint, "model": model, "token": "fixture-token"}
                options = {"enabled": True, "provider": provider, "endpoint": endpoint, "model": model}
                query = {"ai_eligible": True, "ai_assistant": options, "pinyin_segments": ["ni"]}
                expected = {"type": "disabled"} if disabled else None
                reply = {"choices": [{"message": {"content": '{"candidates": []}'}}]}
                with mock.patch.object(online, "fetch", return_value=reply) as fetch:
                    online.ai(query, config)
                self.assertEqual(fetch.call_args.args[2].get("thinking"), expected)
                gloss = {"choices": [{"message": {"content": '{"translation": "hello"}'}}]}
                with mock.patch.object(online, "load_ai_config", return_value=dict(config)), \
                        mock.patch.object(online, "fetch", return_value=gloss) as fetch:
                    online.ai_translation("你好", "en", options, Path("/private/ai.json"))
                self.assertEqual(fetch.call_args.args[2].get("thinking"), expected)
                with mock.patch.object(online, "select_ai_profile", return_value=config), \
                        mock.patch.object(online, "fetch", return_value={"choices": []}) as fetch:
                    online.ai_polish_test({"model": model, "prompt": "润色", "text": "合成"}, self.server)
                self.assertEqual(fetch.call_args.args[2].get("thinking"), expected)
                with mock.patch.object(online, "load_ai_config", return_value=dict(config)), \
                        mock.patch.object(online, "fetch", return_value={"choices": []}) as fetch:
                    online.ai_credential_test(options, Path("/private/ai.json"))
                self.assertEqual(fetch.call_args.args[2].get("thinking"), expected)
                with mock.patch.object(voice, "polish_request", return_value={"choices": []}) as post:
                    voice.polish_credential_test(config)
                    voice.polish("合成", config, {"polish_enabled": True, "polish_provider": provider,
                                                  "polish_model": model})
                self.assertEqual(post.call_count, 2)
                for call in post.call_args_list:
                    self.assertEqual(call.args[1].get("thinking"), expected)

    def test_ai_private_tokens_still_reject_control_characters(self):
        configuration = {
            "provider": "deepseek",
            "endpoint": "https://fixture.invalid/chat",
            "model": "fixture-model",
            "token": "fixture-\x01-token",
        }
        with tempfile.TemporaryDirectory(prefix="msime-ai-") as directory:
            path = Path(directory) / "ai.json"
            path.write_text(json.dumps(configuration), encoding="utf-8")
            path.chmod(0o600)
            with self.assertRaises(ValueError):
                online.load_ai_config(path)

        for key in ("endpoint", "model"):
            configuration[key] = configuration[key] + "\x01"
            with tempfile.TemporaryDirectory(prefix="msime-ai-") as directory:
                path = Path(directory) / "ai.json"
                path.write_text(json.dumps(configuration), encoding="utf-8")
                path.chmod(0o600)
                with self.assertRaises(ValueError):
                    online.load_ai_config(path)
            configuration[key] = configuration[key][:-1]

    def test_ai_private_config_rejects_symlinks(self):
        configuration = {
            "provider": "deepseek",
            "endpoint": "https://fixture.invalid/chat",
            "model": "fixture-model",
            "token": "fixture-token",
        }
        with tempfile.TemporaryDirectory(prefix="msime-ai-") as directory:
            root = Path(directory)
            target = root / "target.json"
            link = root / "ai.json"
            target.write_text(json.dumps(configuration), encoding="utf-8")
            target.chmod(0o600)
            link.symlink_to(target.name)
            with self.assertRaises(OSError):
                online.load_ai_config(link)

    def test_translation_services_return_only_bounded_status(self):
        with mock.patch.object(online, "load_tencent_config", return_value={
                "secret_id": "fixture", "secret_key": "fixture", "region": "fixture"}), \
                mock.patch.object(online, "tencent_translation", return_value=["合成结果"]):
            result = online.credential_test(
                {"service": "translation.tencent", "config": {}}, self.server)
        self.assertEqual(result, {"ok": True, "message": "连接成功，当前配置有效。"})
        with mock.patch.object(online, "custom_translation", side_effect=RuntimeError("private detail")):
            result = online.credential_test({"service": "translation.custom", "config": {
                "endpoint": "https://fixture.invalid/translate", "api_key": "fixture"}}, self.server)
        self.assertFalse(result["ok"])
        self.assertNotIn("private detail", result["message"])


class VoiceCredentialTest(unittest.TestCase):
    def setUp(self):
        self.server = SimpleNamespace(config_path=Path("/private/voice.json"))

    def test_asr_and_polish_select_matching_private_profiles(self):
        configuration = {
            "asr": {"provider": "openai", "model": "whisper-1", "token": "private",
                    "endpoint": "https://fixture.invalid/asr"},
            "polish": {"provider": "deepseek", "model": "fixture-chat", "token": "private",
                       "endpoint": "https://fixture.invalid/chat"},
            "asr_profiles": {}, "polish_profiles": {},
        }
        with mock.patch.object(voice, "load_config", return_value=configuration), \
                mock.patch.object(voice, "batch_asr_credential_test", return_value=True) as batch:
            result = voice.credential_test({"service": "voice.asr", "config": {
                "asr_provider": "openai", "asr_model": "whisper-1"}}, self.server)
        self.assertTrue(result["ok"])
        batch.assert_called_once_with(configuration["asr"])
        with mock.patch.object(voice, "load_config", return_value=configuration), \
                mock.patch.object(voice, "polish_credential_test", return_value=True) as polish:
            result = voice.credential_test({"service": "voice.polish", "config": {
                "polish_provider": "deepseek", "polish_model": "fixture-chat"}}, self.server)
        self.assertTrue(result["ok"])
        polish.assert_called_once_with(configuration["polish"])

    def test_voice_private_tokens_normalize_pasted_whitespace(self):
        configuration = {
            "asr": {
                "provider": "openai",
                "endpoint": " https://fixture.invalid/asr\n",
                "model": " whisper-1 \t",
                "token": " \tfixture-asr-token\r\n",
            },
            "polish": {
                "provider": "deepseek",
                "endpoint": "\thttps://fixture.invalid/chat\r",
                "model": " fixture-model \n",
                "token": " fixture-polish-token\n",
            },
        }
        with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
            path = Path(directory) / "voice.json"
            path.write_text(json.dumps(configuration), encoding="utf-8")
            path.chmod(0o600)
            loaded = voice.load_config(path)
        self.assertEqual(loaded["asr"]["token"], "fixture-asr-token")
        self.assertEqual(loaded["asr"]["endpoint"], "https://fixture.invalid/asr")
        self.assertEqual(loaded["asr"]["model"], "whisper-1")
        self.assertEqual(loaded["polish"]["token"], "fixture-polish-token")
        self.assertEqual(loaded["polish"]["endpoint"], "https://fixture.invalid/chat")
        self.assertEqual(loaded["polish"]["model"], "fixture-model")

    def test_voice_retired_ark_polish_model_reads_as_its_successor(self):
        with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
            path = Path(directory) / "voice.json"

            def read(entry):
                path.write_text(json.dumps({"polish": entry}), encoding="utf-8")
                path.chmod(0o600)
                return voice.load_config(path)

            loaded = read({"provider": "volcengine", "model": "doubao-seed-1.6", "token": "fixture-ark-token"})
            self.assertEqual(loaded["polish"]["model"], "doubao-seed-2-0-lite-260428")
            options = {"polish_provider": "volcengine", "polish_model": "doubao-seed-2-0-lite-260428"}
            self.assertIs(voice.select_profile(loaded, options, "polish"), loaded["polish"])
            loaded = read({"provider": "volcengine", "model": "doubao-seed-1.6-flash", "token": "fixture-ark-token",
                           "endpoint": "https://ark.ap-southeast.bytepluses.com/api/v3/chat/completions"})
            self.assertEqual(loaded["polish"]["model"], "doubao-seed-1.6-flash")
            loaded = read({"provider": "deepseek", "model": "doubao-seed-1.6", "token": "fixture-token"})
            self.assertEqual(loaded["polish"]["model"], "doubao-seed-1.6")

    def test_voice_private_tokens_still_reject_control_characters(self):
        configuration = {
            "asr": {
                "provider": "openai",
                "endpoint": "https://fixture.invalid/asr",
                "model": "whisper-1",
                "token": "fixture-\x01-token",
            },
        }
        with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
            path = Path(directory) / "voice.json"
            path.write_text(json.dumps(configuration), encoding="utf-8")
            path.chmod(0o600)
            with self.assertRaises(ValueError):
                voice.load_config(path)

        for key in ("endpoint", "model"):
            configuration["asr"][key] = configuration["asr"][key] + "\x01"
            with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
                path = Path(directory) / "voice.json"
                path.write_text(json.dumps(configuration), encoding="utf-8")
                path.chmod(0o600)
                with self.assertRaises(ValueError):
                    voice.load_config(path)
            configuration["asr"][key] = configuration["asr"][key][:-1]

    def test_voice_private_config_rejects_symlinks(self):
        configuration = {
            "asr": {
                "provider": "openai",
                "endpoint": "https://fixture.invalid/asr",
                "model": "whisper-1",
                "token": "fixture-token",
            },
        }
        with tempfile.TemporaryDirectory(prefix="msime-voice-") as directory:
            root = Path(directory)
            target = root / "target.json"
            link = root / "voice.json"
            target.write_text(json.dumps(configuration), encoding="utf-8")
            target.chmod(0o600)
            link.symlink_to(target.name)
            with self.assertRaises(OSError):
                voice.load_config(link)

    def test_invalid_or_failed_requests_do_not_expose_details(self):
        result = voice.credential_test({"service": "voice.asr", "config": {"bad": 1}}, self.server)
        self.assertEqual(result, {"ok": False, "message": "当前语音选项无效。"})
        with mock.patch.object(voice, "load_config", side_effect=RuntimeError("private detail")):
            result = voice.credential_test(
                {"service": "voice.asr", "config": {"asr_provider": "openai"}}, self.server)
        self.assertFalse(result["ok"])
        self.assertNotIn("private detail", result["message"])


if __name__ == "__main__":
    unittest.main()

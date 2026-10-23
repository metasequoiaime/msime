#!/usr/bin/env python3
"""固定迁移前的语音目录，并验证实际消费者读取生成模块。"""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
PROVIDER = ROOT / "scripts/msime-linux-voice-provider"
if len(sys.argv) > 2 and sys.argv[1] == "--provider":
    PROVIDER = Path(sys.argv[2]).resolve()
    del sys.argv[1:3]
sys.path.insert(0, str(PROVIDER.parent))


def load_provider(name):
    loader = importlib.machinery.SourceFileLoader(name, str(PROVIDER))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


voice = load_provider("voice_provider_catalog_original")
# 固定真实宿主原有值；不从生成器读取期望，防止两边一起漂移。
EXPECTED = {
    "asr": [
        "bailian",
        "custom",
        "doubao",
        "everyapi",
        "groq",
        "minimax",
        "mistral",
        "openai",
        "siliconflow",
        "zhipu"
    ],
    "polish": [
        "custom",
        "deepseek",
        "groq",
        "kimi",
        "minimax",
        "openai",
        "openrouter",
        "qwen",
        "siliconflow",
        "volcengine",
        "zhipu"
    ],
    "models": {
        "asr": {
            "openai": "whisper-1",
            "groq": "whisper-large-v3-turbo",
            "siliconflow": "FunAudioLLM/SenseVoiceSmall",
            "doubao": "",
            "everyapi": "volc.seedasr.sauc.duration",
            "mistral": "voxtral-mini-latest",
            "bailian": "qwen3-asr-flash",
            "zhipu": "glm-asr",
            "minimax": "asr-1.0",
            "custom": ""
        },
        "polish": {
            "openai": "gpt-4o-mini",
            "groq": "llama-3.3-70b-versatile",
            "siliconflow": "Qwen/Qwen3-8B",
            "deepseek": "deepseek-v4-flash",
            "minimax": "MiniMax-M3.1-Flash",
            "kimi": "kimi-k2-0905-preview",
            "zhipu": "glm-4-flash",
            "qwen": "qwen-plus",
            "volcengine": "doubao-seed-1.6-flash",
            "openrouter": "openrouter/auto",
            "custom": ""
        }
    },
    "endpoints": {
        "asr": {
            "openai": "https://api.openai.com/v1/audio/transcriptions",
            "groq": "https://api.groq.com/openai/v1/audio/transcriptions",
            "siliconflow": "https://api.siliconflow.cn/v1/audio/transcriptions",
            "everyapi": "https://api.everyapi.ai/v1/audio/transcriptions",
            "mistral": "https://api.mistral.ai/v1/audio/transcriptions",
            "bailian": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "doubao": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
            "zhipu": "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions",
            "minimax": "https://api.minimax.cn/v1/speech_to_text",
            "custom": ""
        },
        "polish": {
            "openai": "https://api.openai.com/v1/chat/completions",
            "groq": "https://api.groq.com/openai/v1/chat/completions",
            "siliconflow": "https://api.siliconflow.cn/v1/chat/completions",
            "deepseek": "https://api.deepseek.com/chat/completions",
            "minimax": "https://api.minimax.cn/v1/chat/completions",
            "kimi": "https://api.moonshot.cn/v1/chat/completions",
            "zhipu": "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "qwen": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "volcengine": "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
            "openrouter": "https://openrouter.ai/api/v1/chat/completions",
            "custom": ""
        }
    },
    "formats": {
        "openai": "multipart",
        "groq": "multipart",
        "siliconflow": "multipart",
        "everyapi": "multipart",
        "mistral": "multipart",
        "zhipu": "multipart",
        "minimax": "multipart",
        "custom": "multipart",
        "bailian": "chat_audio"
    }
}


class ProviderCatalog(unittest.TestCase):
    def test_existing_public_catalog_is_unchanged(self):
        actual = {
            "asr": sorted(voice.ASR_PROVIDERS),
            "polish": sorted(voice.POLISH_PROVIDERS),
            "models": voice.DEFAULT_MODELS,
            "endpoints": voice.DEFAULT_ENDPOINTS,
            "formats": voice.REQUEST_FORMATS,
        }
        self.assertEqual(actual, EXPECTED)
        self.assertNotIn("local", voice.ASR_PROVIDERS)
        self.assertNotIn("doubao", voice.REQUEST_FORMATS)
        self.assertNotIn("local", voice.REQUEST_FORMATS)

    def test_real_configuration_and_requests_use_the_generated_catalog(self):
        catalog = types.ModuleType("msime_voice_provider_catalog")
        catalog.ASR_PROVIDERS = {"synthetic-asr"}
        catalog.POLISH_PROVIDERS = {"synthetic-polish"}
        catalog.DEFAULT_ENDPOINTS = {
            "asr": {"synthetic-asr": "https://synthetic.example.test/asr"},
            "polish": {"synthetic-polish": "https://synthetic.example.test/polish"},
        }
        catalog.DEFAULT_MODELS = {
            "asr": {"synthetic-asr": "合成识别模型😀"},
            "polish": {"synthetic-polish": "合成润色模型😀"},
        }
        catalog.REQUEST_FORMATS = {"synthetic-asr": "chat_audio"}
        with patch.dict(sys.modules, {"msime_voice_provider_catalog": catalog}):
            consumer = load_provider("voice_provider_catalog_synthetic")
        self.assertEqual(consumer.ASR_PROVIDERS, catalog.ASR_PROVIDERS,
                         "识别服务集合仍是宿主副本")
        self.assertEqual(consumer.POLISH_PROVIDERS, catalog.POLISH_PROVIDERS)
        self.assertEqual(consumer.REQUEST_FORMATS, catalog.REQUEST_FORMATS)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.json"

            def read(config):
                path.write_text(json.dumps(config), encoding="utf-8")
                path.chmod(0o600)
                return consumer.load_config(path)

            config = read({
                "asr": {"provider": "synthetic-asr", "token": "synthetic-token"},
                "polish": {"provider": "synthetic-polish", "token": "synthetic-token"},
            })
            for kind in ("asr", "polish"):
                with self.subTest(kind=kind):
                    provider = config[kind]["provider"]
                    self.assertEqual(config[kind]["endpoint"],
                                     catalog.DEFAULT_ENDPOINTS[kind][provider])
                    self.assertEqual(config[kind]["model"],
                                     catalog.DEFAULT_MODELS[kind][provider])
            body, content_type = consumer.request_body(bytes(320), config["asr"], "auto")
            self.assertEqual(content_type, "application/json")
            self.assertEqual(json.loads(body)["model"], "合成识别模型😀")
            self.assertEqual(json.loads(body)["messages"][0]["content"][0]["type"], "input_audio")

            config = read({
                "asr": {"provider": "synthetic-asr", "token": "synthetic-token",
                        "endpoint": " https://custom.example.test/asr ", "model": " 自定义模型😀 "},
                "asr_profiles": {"local": {}},
                "polish_profiles": {"SYNTHETIC-POLISH": {"token": "synthetic-token"}},
            })
            self.assertEqual(config["asr"]["endpoint"], "https://custom.example.test/asr")
            self.assertEqual(config["asr"]["model"], "自定义模型😀")
            self.assertEqual(config["asr_profiles"]["local"]["provider"], "local")
            self.assertEqual(config["polish_profiles"]["synthetic-polish"]["endpoint"],
                             catalog.DEFAULT_ENDPOINTS["polish"]["synthetic-polish"])
            with self.assertRaises(ValueError):
                read({"asr": {"provider": "openai", "token": "synthetic-token"}})


if __name__ == "__main__":
    unittest.main()

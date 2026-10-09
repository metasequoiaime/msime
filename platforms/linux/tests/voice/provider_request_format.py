#!/usr/bin/env python3
"""每个整句识别服务按请求格式拼请求体：multipart 或阿里云百炼的 chat_audio（#6017）。"""
import base64
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
loader = importlib.machinery.SourceFileLoader(
    "voice_request_format_provider", str(ROOT / "scripts" / "msime-linux-voice-provider"))
spec = importlib.util.spec_from_loader(loader.name, loader)
voice = importlib.util.module_from_spec(spec)
loader.exec_module(voice)


class ProviderRequestFormat(unittest.TestCase):
    def test_bailian_is_a_recognition_service_with_its_own_defaults(self):
        self.assertIn("bailian", voice.ASR_PROVIDERS)
        self.assertNotIn("bailian", voice.POLISH_PROVIDERS)
        self.assertEqual(voice.DEFAULT_ENDPOINTS["asr"]["bailian"],
                         "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
        self.assertEqual(voice.DEFAULT_MODELS["asr"]["bailian"], "qwen3-asr-flash")
        self.assertEqual(voice.REQUEST_FORMATS["bailian"], "chat_audio")
        for provider in ("openai", "groq", "siliconflow", "everyapi", "mistral"):
            self.assertEqual(voice.REQUEST_FORMATS[provider], "multipart")

    def test_chat_audio_body_carries_the_recording_as_a_data_url(self):
        body, content_type = voice.request_body(
            b"\0\0" * 160, {"provider": "bailian", "model": "qwen3-asr-flash"}, "zh-cn")
        self.assertEqual(content_type, "application/json")
        request = json.loads(body)
        self.assertEqual(request["model"], "qwen3-asr-flash")
        self.assertIs(request["stream"], False)
        self.assertEqual(len(request["messages"]), 1)
        part = request["messages"][0]["content"][0]
        self.assertEqual(part["type"], "input_audio")
        data = part["input_audio"]["data"]
        self.assertTrue(data.startswith("data:audio/wav;base64,"))
        wav = base64.b64decode(data.split(",", 1)[1])
        self.assertEqual(wav[:4], b"RIFF")
        self.assertEqual(len(wav), 44 + 320)

    def test_chat_audio_refuses_a_recording_over_the_upload_limit(self):
        with self.assertRaises(ValueError):
            voice.request_body(b"\0" * voice.CHAT_AUDIO_MAX_WAV,
                               {"provider": "bailian", "model": "qwen3-asr-flash"}, "auto")

    def test_bailian_recordings_stop_at_what_one_upload_can_carry(self):
        limit = voice.capture_byte_limit("bailian")
        self.assertEqual(limit, voice.CHAT_AUDIO_MAX_WAV - 44)
        voice.request_body(b"\0" * limit, {"provider": "bailian", "model": "qwen3-asr-flash"}, "auto")
        self.assertEqual(voice.capture_byte_limit("openai"), voice.MAX_AUDIO - 44)
        self.assertEqual(voice.capture_byte_limit("local"), voice.MAX_AUDIO - 44)

    def test_multipart_providers_keep_their_upload(self):
        _, content_type = voice.request_body(
            b"\0\0" * 160, {"provider": "openai", "model": "whisper-1"}, "zh-cn")
        self.assertTrue(content_type.startswith("multipart/form-data; boundary="))

    def test_transcript_is_read_from_either_answer_shape(self):
        self.assertEqual(voice.transcript({"text": " 水杉 "}), "水杉")
        self.assertEqual(
            voice.transcript({"choices": [{"message": {"role": "assistant", "content": "百炼"}}]}),
            "百炼")
        self.assertEqual(voice.transcript({"choices": []}), "")
        self.assertEqual(voice.transcript({"choices": ["not an object"]}), "")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""用合成输入验证 Android OCR 模型的下载校验、字典抽取和检测参数核对，并核对锁文件、许可证副本与 Java 常量一致。不联网。"""
import hashlib
import io
from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import fetch_ocr_models

ROOT = fetch_ocr_models.ROOT
PIPELINE = ROOT / "platforms/android/gradle-app/ocr-module/src/main/java/app/msime/ocr/module/OcrPipeline.java"
GRADLE = ROOT / "platforms/android/gradle-app/ocr-module/build.gradle.kts"

DETECTION_YML = """Global:
  model_name: synthetic_det
PostProcess:
  box_thresh: 0.4
  max_candidates: 3000
  name: DBPostProcess
  thresh: 0.2
  unclip_ratio: 1.4
PreProcess:
  transform_ops:
  - DecodeImage:
      img_mode: BGR
"""

RECOGNITION_YML = """Global:
  model_name: synthetic_rec
PreProcess:
  transform_ops:
  - DecodeImage:
      img_mode: BGR
PostProcess:
  name: CTCLabelDecode
  character_dict:
  - '!'
  - ''''
  - '0'
  - A
  - 中
  - \u3000
  - '～'
"""


class DictionaryTests(unittest.TestCase):
    def test_entries_keep_order_quotes_and_full_width_space(self):
        self.assertEqual(fetch_ocr_models.dictionary(RECOGNITION_YML), ["!", "'", "0", "A", "中", "　", "～"])

    def test_unexpected_shapes_are_rejected(self):
        for bad in ("  - ab\n", "  - 1\n", "  - '\n", "  - ~\n", "  - - x\n", "  - 'a'b'\n"):
            with self.subTest(bad=bad):
                with self.assertRaises(ValueError):
                    fetch_ocr_models.dictionary(RECOGNITION_YML + bad)
        with self.assertRaises(ValueError):
            fetch_ocr_models.dictionary(RECOGNITION_YML.replace("CTCLabelDecode", "AttnLabelDecode"))
        with self.assertRaises(ValueError):
            fetch_ocr_models.dictionary(RECOGNITION_YML + "    nested: true\n")

    def test_detection_parameters_must_match_the_lock(self):
        expected = {"thresh": 0.2, "box_thresh": 0.4, "unclip_ratio": 1.4}
        fetch_ocr_models.check_detection(DETECTION_YML, expected)
        with self.assertRaises(ValueError):
            fetch_ocr_models.check_detection(DETECTION_YML.replace("box_thresh: 0.4", "box_thresh: 0.6"), expected)
        with self.assertRaises(ValueError):
            fetch_ocr_models.check_detection(DETECTION_YML.replace("DBPostProcess", "EASTPostProcess"), expected)


class DownloadTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.data = b"synthetic model bytes"
        self.entry = {"name": "det.onnx", "repository": "PaddlePaddle/synthetic", "commit": "a" * 40,
                      "path": "inference.onnx", "size": len(self.data),
                      "sha256": hashlib.sha256(self.data).hexdigest(), "license": "Apache-2.0"}
        self.target = self.directory / "det.onnx"

    def test_download_uses_the_pinned_commit(self):
        with patch.object(fetch_ocr_models.download_retry, "urlopen", return_value=io.BytesIO(self.data)) as request:
            fetch_ocr_models.download(self.entry, self.target)
        self.assertEqual(self.target.read_bytes(), self.data)
        self.assertEqual(request.call_args.args[0],
                         "https://huggingface.co/PaddlePaddle/synthetic/resolve/" + "a" * 40 + "/inference.onnx")
        self.assertEqual(list(self.directory.iterdir()), [self.target])

    def test_bad_downloads_keep_the_previous_file_and_clean_staging(self):
        for data in (self.data[:-1], self.data + b"x", b"x" * len(self.data)):
            with self.subTest(data=data):
                self.target.write_bytes(b"previous")
                with patch.object(fetch_ocr_models.download_retry, "urlopen", return_value=io.BytesIO(data)):
                    with self.assertRaises(ValueError):
                        fetch_ocr_models.download(self.entry, self.target)
                self.assertEqual(self.target.read_bytes(), b"previous")
                self.assertEqual(list(self.directory.iterdir()), [self.target])

    def test_offline_never_downloads(self):
        lock = fetch_ocr_models.read_lock()
        with patch.object(fetch_ocr_models.download_retry, "urlopen", side_effect=AssertionError("不得联网")):
            with self.assertRaises(FileNotFoundError):
                fetch_ocr_models.prepare(lock, self.directory / "out", self.directory / "cache", offline=True)


class RepositoryTests(unittest.TestCase):
    def test_lock_licences_and_java_constants_agree(self):
        lock = fetch_ocr_models.read_lock()
        for entry in lock["licenses"]:
            path = ROOT / entry["path"]
            data = path.read_bytes()
            self.assertEqual((len(data), hashlib.sha256(data).hexdigest()), (entry["size"], entry["sha256"]), entry["path"])
        java = PIPELINE.read_text(encoding="utf-8")
        constants = {"thresh": "DET_THRESH", "box_thresh": "DET_BOX_THRESH", "unclip_ratio": "DET_UNCLIP_RATIO"}
        for key, name in constants.items():
            match = re.search(rf"static final float {name} = ([0-9.]+)f;", java)
            self.assertIsNotNone(match, name)
            self.assertEqual(float(match.group(1)), lock["detection"][key], name)
        maven = lock["runtime"]["maven"]
        self.assertIn(f'implementation("{maven}")', GRADLE.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()

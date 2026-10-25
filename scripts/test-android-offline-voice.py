#!/usr/bin/env python3
"""守住外部语音入口与内部输入法队列、网络识别之间的边界。"""

from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android"
NS = "{http://schemas.android.com/apk/res/android}"
tree = ET.parse(ANDROID / "AndroidManifest.xml")
activities = {item.get(NS + "name"): item for item in tree.findall("application/activity")}
internal = activities["app.msime.android.VoiceRecognitionActivity"]
assert internal.get(NS + "exported") == "false", "内部带服务参数的语音窗口不能导出"
external = activities.get("app.msime.android.OfflineVoiceRecognitionActivity")
assert external is not None, "缺少标准外部离线语音入口"
assert external.get(NS + "exported") == "true", "离线语音入口必须可被外部调用"
assert any(
    {action.get(NS + "name") for action in item.findall("action")}
    == {"android.speech.action.RECOGNIZE_SPEECH"}
    and "android.intent.category.DEFAULT"
    in {category.get(NS + "name") for category in item.findall("category")}
    for item in external.findall("intent-filter")
), "离线语音入口必须提供标准识别 action"
source = (ANDROID / "java/app/msime/android/voice/OfflineVoiceRecognitionActivity.java").read_text()
for forbidden in (
    "HttpAsrRecognizer", "DoubaoRecognizer", "SpeechRecognizer", "VoiceResultStore",
    "ClipboardManager", "VoicePolish", "Log",
):
    assert not re.search(r"\b" + forbidden + r"\b", source), f"外部离线结果越过归属边界：{forbidden}"
assert "LocalAsrRecognizer" in source, "外部入口必须复用本地识别器"
assert "RecognizerIntent.EXTRA_RESULTS" in source, "结果必须遵循 Android 标准识别契约"
print("Android offline voice entry: manifest and result ownership boundaries pass")

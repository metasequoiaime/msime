#!/usr/bin/env python3
"""检查 Windows 原生语音在哪一步按「语音标点」处理最终文本（#10597）。

VoicePunctuation.h 的纯函数由 platforms/windows/tests/voice/voice_punctuation.cpp 覆盖，这里只看 VoiceInputSession::finish 里的接线顺序：润色之后处理，处理完再判空（只剩标点时提示没听到，审阅录音和取消不提示），之后才做繁体转换和投递。挪到润色之前会让润色补回的标点上屏，挪到繁体转换或投递之后则不起作用。
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "platforms/windows/src/voice/VoiceInputSession.cpp"

source = SOURCE.read_text(encoding="utf-8")
start = source.index("void VoiceInputSession::finish(")
finish = source[start:]

polish = finish.index("polish_cloud_text(")
punctuation = finish.index("final_text = voice_punctuated(final_text, config.punctuation_mode,")
empty_check = finish.index("final_text.empty()) {", punctuation)
no_speech = finish.index(
    "if (final_text.empty() && !review && session_.load() == session &&", punctuation
)
traditional = finish.index("simplified_to_traditional(final_text, true)")
deliver = finish.index("deliver_voice_result(review, final_text,")

assert polish < punctuation, "语音标点要在润色之后处理"
assert punctuation < empty_check < no_speech < traditional, "处理完先判空并提示没听到，再做繁体转换"
assert traditional < deliver, "繁体转换之后才投递"
assert "voice_failure_notice(VoiceFailure::NoSpeech)" in finish[no_speech : no_speech + 300]
# 处理写在函数体最外层，不在 review 或繁体输出的分支里：面板的审阅录音拿到的也是处理后的文字。
assert finish[: punctuation].endswith("\n  "), "语音标点不能放进条件分支"

print("Windows voice punctuation order passed")

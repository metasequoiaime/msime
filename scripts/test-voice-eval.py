#!/usr/bin/env python3
"""检查 scripts/voice_eval.py 的打分规则：混合错误率按汉字一字、拉丁词一词切分，标点、空白和大小写不计。

打分规则一变，resources/eval/voice-baseline.json 里的基线就和新跑出来的数不可比了，所以把切分和编辑距离钉在这里。不需要模型、运行库或数据集。
"""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("voice_eval", ROOT / "scripts/voice_eval.py")
voice_eval = importlib.util.module_from_spec(spec)
spec.loader.exec_module(voice_eval)

assert voice_eval.tokens("昨天是 Monday，today is 礼拜二。") == ["昨", "天", "是", "monday", "today", "is", "礼", "拜", "二"]
assert voice_eval.tokens("what's your major") == ["what's", "your", "major"]
assert voice_eval.tokens("桥下垂直净空 15 米。") == ["桥", "下", "垂", "直", "净", "空", "15", "米"]
# 全角字母数字按 NFKC 归一，和半角算同一个词。
assert voice_eval.tokens("ＧＰＵ 用了 ３ 张") == ["gpu", "用", "了", "3", "张"]
assert voice_eval.tokens("“……”") == []

assert voice_eval.edit_distance(list("abc"), list("abc")) == 0
assert voice_eval.edit_distance(list("abc"), list("axc")) == 1
assert voice_eval.edit_distance(list("abc"), list("ab")) == 1
assert voice_eval.edit_distance([], list("ab")) == 2

# 标点和大小写的差别不算错；一个字错、一个英文词错各算一个。
assert voice_eval.score([("今天天气很好。", "今天天气很好")]) == (0, 6)
assert voice_eval.score([("Today is Monday", "today is monday.")]) == (0, 3)
assert voice_eval.score([("代码写好了", "代马写好了"), ("push the branch", "push a branch")]) == (2, 8)

# 汇总按模型、测试集分组，MER 与错误数、单位数一致。
rows = [
    {"model": "m", "kind": "offline_transducer", "set": "a", "id": "1", "seconds": 1.0, "finish_seconds": 0.2, "reference": "代码写好了", "hypothesis": "代马写好了"},
    {"model": "m", "kind": "offline_transducer", "set": "a", "id": "2", "seconds": 2.0, "finish_seconds": 0.4, "reference": "push the branch", "hypothesis": "push a branch"},
]
summary = voice_eval.summarize(rows, None)
assert summary == [{"model": "m", "kind": "offline_transducer", "sets": {"a": {"utterances": 2, "audio_seconds": 3.0, "units": 8, "errors": 2, "mer": 25.0, "finish_seconds_median": 0.4}}}], summary

baseline = ROOT / "resources/eval/voice-baseline.json"
if baseline.exists():
    document = json.loads(baseline.read_text(encoding="utf-8"))
    for run in document["runs"]:
        for name, result in run["sets"].items():
            assert result["errors"] <= result["units"] * 2, (run["model"], name)
            assert abs(result["mer"] - round(100 * result["errors"] / result["units"], 2)) < 0.005, (run["model"], name)
            if "mer_numbers" in result:
                assert abs(result["mer_numbers"] - round(100 * result["errors_numbers"] / result["units_numbers"], 2)) < 0.005, (run["model"], name)

print("voice eval scoring: ok")

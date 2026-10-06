#!/usr/bin/env python3
"""随包的离线手写模型能被构建出的 `msime-linux-handwriting --local` 加载，并认出两笔合成的「十」。

CMake 只按锁文件核对模型的大小和 SHA-256；这里确认它真的是识别器读得懂的模型。笔画坐标归一化到 [0, 1]，与手写面板发出的一致。

Usage: handwriting_local_model.py <built msime-linux-handwriting> <handwriting-zh_CN.model>
"""
import json
import subprocess
import sys

# 一横一竖。
STROKES = [
    [{"x": 0.1, "y": 0.5}, {"x": 0.9, "y": 0.5}],
    [{"x": 0.5, "y": 0.1}, {"x": 0.5, "y": 0.9}],
]


def main():
    tool, model = sys.argv[1:]
    result = subprocess.run(
        [tool, "--local", model],
        input=json.dumps({"strokes": STROKES}),
        capture_output=True,
        text=True,
        timeout=30,
    )
    print(result.stdout.strip())
    if result.returncode != 0:
        sys.exit("msime-linux-handwriting --local exited with %d: %s" % (result.returncode, result.stderr.strip()))
    candidates = json.loads(result.stdout)["value"]["candidates"]
    if not candidates or candidates[0] != "十":
        sys.exit("expected 十 as the first candidate, got %r" % candidates)


if __name__ == "__main__":
    main()

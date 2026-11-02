"""用真实运行库与 Fun-ASR-Nano 模型检查目录型分词器；不采集或发送音频。"""

import json
import os
from pathlib import Path
import queue
import threading
import subprocess
import sys
import tempfile

helper, runtime, installed_model = sys.argv[1:4]
model = Path(installed_model)
if json.loads((model / "msime-model.json").read_text())["kind"] != "offline_funasr_nano":
    print("跳过：需要已安装的 Fun-ASR-Nano 模型")
    sys.exit(77)


def start(model_path, expected_missing=None):
    process = subprocess.Popen(
        [helper, "--runtime", runtime, "--idle-exit", "120"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True,
    )
    # Windows 的 select 只支持 socket，匿名管道统一由读取线程交给带超时的队列。
    responses = queue.Queue()

    def read_responses():
        try:
            for line in process.stdout:
                responses.put(line)
        finally:
            responses.put(None)

    reader = threading.Thread(target=read_responses, daemon=True)
    reader.start()

    def receive():
        try:
            line = responses.get(timeout=120)
        except queue.Empty:
            raise AssertionError("等待辅助进程响应超时") from None
        assert line, "辅助进程提前退出"
        return json.loads(line)

    def send(message):
        process.stdin.write(json.dumps(message) + "\n")
        process.stdin.flush()

    try:
        hello = receive()
        assert hello["type"] == "hello" and hello["available"], hello
        send({"op": "start", "id": 1, "model": str(model_path), "language": "zh-CN", "threads": 2})
        reply = receive()
        if expected_missing is None:
            assert reply == {"type": "started", "id": 1}, reply
            send({"op": "cancel"})
            assert receive() == {"type": "cancelled", "id": 1}
        else:
            assert reply["type"] == "error", reply
            assert reply["message"] == "Local model is missing " + expected_missing, reply
        process.stdin.close()
        assert process.wait(timeout=15) == 0
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        reader.join(timeout=5)
        process.stdout.close()
        if not process.stdin.closed:
            process.stdin.close()


# 正常安装必须经过真正的加载入口成功建会话，取消时不发送任何音频。
start(model)
print("真实 Fun-ASR-Nano 分词器目录：加载与取消通过")

with tempfile.TemporaryDirectory(prefix="msime-tokenizer-") as temporary:
    root = Path(temporary).resolve()
    outside = root / "outside"
    outside.mkdir()
    (outside / "tokenizer.json").write_text("synthetic")

    def fixture(name):
        directory = root / name
        directory.mkdir()
        files = {"encoder_adaptor": "adaptor.onnx", "llm": "llm.onnx",
                 "embedding": "embedding.onnx", "vad": "vad.onnx", "tokenizer": "Qwen3-0.6B"}
        for role, filename in files.items():
            if role != "tokenizer":
                (directory / filename).write_text("synthetic")
        tokenizer = directory / files["tokenizer"]
        tokenizer.mkdir()
        for filename in ["vocab.json", "merges.txt", "tokenizer.json"]:
            (tokenizer / filename).write_text("synthetic")
        (directory / "msime-model.json").write_text(json.dumps({"kind": "offline_funasr_nano", "files": files}))
        return directory, tokenizer

    directory, tokenizer = fixture("regular-file")
    for child in tokenizer.iterdir():
        child.unlink()
    tokenizer.rmdir()
    tokenizer.write_text("synthetic")
    start(directory, "Qwen3-0.6B")

    directory, tokenizer = fixture("empty-directory")
    for child in tokenizer.iterdir():
        child.unlink()
    start(directory, "Qwen3-0.6B/vocab.json")

    directory, tokenizer = fixture("escaped-directory")
    manifest = json.loads((directory / "msime-model.json").read_text())
    manifest["files"]["tokenizer"] = "../outside"
    (directory / "msime-model.json").write_text(json.dumps(manifest))
    start(directory, "../outside")

    directory, tokenizer = fixture("directory-child")
    (tokenizer / "tokenizer.json").unlink()
    (tokenizer / "tokenizer.json").mkdir()
    start(directory, "Qwen3-0.6B/tokenizer.json")

    directory, tokenizer = fixture("hardlink-child")
    (tokenizer / "tokenizer.json").unlink()
    os.link(outside / "tokenizer.json", tokenizer / "tokenizer.json")
    start(directory, "Qwen3-0.6B/tokenizer.json")

    if os.name != "nt":
        directory, tokenizer = fixture("symlink-directory")
        real = directory / "real-tokenizer"
        tokenizer.rename(real)
        tokenizer.symlink_to(real, target_is_directory=True)
        start(directory, "Qwen3-0.6B")

        directory, tokenizer = fixture("external-directory")
        for child in tokenizer.iterdir():
            child.unlink()
        tokenizer.rmdir()
        tokenizer.symlink_to(outside, target_is_directory=True)
        start(directory, "Qwen3-0.6B")

        directory, tokenizer = fixture("symlink-child")
        (tokenizer / "tokenizer.json").unlink()
        (tokenizer / "tokenizer.json").symlink_to(outside / "tokenizer.json")
        start(directory, "Qwen3-0.6B/tokenizer.json")

        directory, tokenizer = fixture("fifo-child")
        (tokenizer / "tokenizer.json").unlink()
        os.mkfifo(tokenizer / "tokenizer.json")
        start(directory, "Qwen3-0.6B/tokenizer.json")

print("分词器错误类型、缺文件、符号链接、硬链接与特殊文件拒绝检查通过")

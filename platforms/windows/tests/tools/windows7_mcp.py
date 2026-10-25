#!/usr/bin/env python3
"""用最终收集的 MCP 程序验收协议，所有状态和词库均为临时合成数据。"""
import argparse
from contextlib import closing
import json
import os
from pathlib import Path
import queue
import sqlite3
import subprocess
import tempfile
import threading


def fixture(root):
    tables = """
        CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
        CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
        CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
    """
    for name in ("resources", "dictionaries", "user", "cache"):
        (root / name).mkdir()
    for name in ("resources/msime-pinyin.db", "resources/msime-wubi.db", "dictionaries/msime-pinyin.db"):
        with closing(sqlite3.connect(root / name)) as connection:
            connection.executescript(tables)
    options = root / "runtime-options.json"
    options.write_text(json.dumps({
        "api_version": 1, "resources": str(root / "resources"),
        "dictionaries": str(root / "dictionaries"), "user_data": str(root / "user"),
        "cache": str(root / "cache"), "preferences_directory": str(root), "preferences": {},
    }), encoding="utf-8")
    return options


def verify(program):
    with tempfile.TemporaryDirectory(prefix="msime-win7-mcp-") as temporary:
        root = Path(temporary)
        options = fixture(root)
        environment = os.environ.copy()
        for key in ("MSIME_CLIENT_HOST_OPTIONS", "MSIME_IBUS_OPTIONS", "MSIME_CLIENT_STATE_DIR"):
            environment.pop(key, None)
        base = [str(program), "--options", str(options), "--state-dir", str(root)]
        # 经共享服务写出完整偏好，避免在 Python 复制 Rust 的默认值和文档格式。
        seed = subprocess.run(base + ["--allow-write", "call", "update_preferences",
            json.dumps({"expected_revision": 0, "candidate_page_size": 7})],
            env=environment, capture_output=True, timeout=30, check=True)
        assert json.loads(seed.stdout)["candidate_page_size"] == 7
        cli = subprocess.run(base + ["tools"], env=environment, capture_output=True, timeout=30, check=True)
        cli_tools = {tool["name"] for tool in json.loads(cli.stdout)}
        assert "get_preferences" in cli_tools and "update_preferences" not in cli_tools
        for flags in ([], ["--allow-write", "--allow-dictionary-read"]):
            process = subprocess.Popen(base + flags, env=environment, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            lines = queue.Queue()
            def receive():
                for line in process.stdout:
                    lines.put(line)
                lines.put(None)
            reader = threading.Thread(target=receive, daemon=True)
            reader.start()
            sequence = 0
            def send(method, params, notification=False):
                nonlocal sequence
                sequence += 1
                request = {"jsonrpc": "2.0", "method": method, "params": params}
                if not notification:
                    request["id"] = sequence
                process.stdin.write(json.dumps(request).encode() + b"\n")
                process.stdin.flush()
                if notification:
                    return None
                while True:
                    line = lines.get(timeout=20)
                    assert line is not None, "MCP closed before response"
                    response = json.loads(line)
                    if response.get("id") == sequence:
                        assert "error" not in response, response.get("error")
                        return response["result"]
            try:
                initialized = send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "synthetic-win7-verifier", "version": "1"}})
                assert initialized["serverInfo"]["name"] == "msime"
                send("notifications/initialized", {}, notification=True)
                names = {tool["name"] for tool in send("tools/list", {})["tools"]}
                if flags:
                    assert {"get_preferences", "update_preferences", "lookup_candidates", "edit_dictionary_words"} <= names
                else:
                    assert names == cli_tools
                result = send("tools/call", {"name": "get_preferences", "arguments": {}})
                assert not result.get("isError", False) and isinstance(result.get("structuredContent"), dict)
                result = send("tools/call", {"name": "list_quick_phrases", "arguments": {}})
                assert not result.get("isError", False) and result["structuredContent"]["phrases"] == [], result
            finally:
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                reader.join(timeout=5)
                process.stdout.close()
                process.stderr.close()
        print(f"{program.parent.name}: MCP CLI, initialize, tools/list, permissions and synthetic read calls passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("program", type=Path, nargs="+")
    arguments = parser.parse_args()
    for binary in arguments.program:
        verify(binary.resolve(strict=True))

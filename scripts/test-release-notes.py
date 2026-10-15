#!/usr/bin/env python3
"""验证发布说明只选择目标平台和纯共享改动。"""

from __future__ import annotations

import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release_notes", ROOT / "scripts/generate-release-notes.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def check(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def check_model_request_user_agent() -> None:
    # api.everyapi.ai sits behind Cloudflare, which answers urllib's default `Python-urllib/3.x` with 403 (error 1010) before the request reaches the API; every release from android-v0.2.0 to android-v0.3.0 fell back to the raw commit list because of it.
    sent: list[object] = []

    def capture(request: object, timeout: float) -> object:
        sent.append(request)
        raise OSError("captured")

    original = MODULE.urllib.request.urlopen
    MODULE.urllib.request.urlopen = capture
    MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"] = "test-token"
    try:
        MODULE.model_notes("Android", "1.2.3", [{"pr": None, "commits": [{"subject": "修复键盘崩溃", "commit": "1234567890", "paths": ["platforms/android/java/app.kt"], "body": ""}]}], {})
    finally:
        MODULE.urllib.request.urlopen = original
        del MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"]
    check(len(sent) == 1, "模型请求没有发出")
    agent = sent[0].get_header("User-agent") or ""
    check(bool(agent) and not agent.startswith("Python-urllib"), f"模型请求用了会被 Cloudflare 拒绝的 User-Agent：{agent!r}")
    # GPT-5 系列只接受默认 temperature，带了就是 HTTP 400（windows-v0.2.1）。
    payload = MODULE.json.loads(sent[0].data)
    check("temperature" not in payload, "模型请求带了 temperature，GPT-5 系列会以 HTTP 400 拒绝")
    check(int(payload.get("max_tokens", 0)) >= 16000, "模型请求没有给足输出上限，长说明会被截断")
    prompt = payload["messages"][-1]["content"]
    check("## v0.10.0 更新内容" in prompt and "## v1.2.3 更新内容" in prompt, "模型请求没有带上格式样例和目标标题")


def check_model_configuration() -> None:
    groups = [{"pr": None, "commits": [{"subject": "fix: 合成修复", "commit": "1234567890", "paths": [], "body": ""}]}]
    for configured, expected in [(None, "claude-sonnet-5-5"), ("", "claude-sonnet-5-5"), (" \t ", "claude-sonnet-5-5"), (" custom-model ", "custom-model")]:
        sent: list[object] = []

        def respond(request: object, timeout: float) -> object:
            sent.append(request)
            return io.BytesIO(MODULE.json.dumps({"choices": [{"message": {"content": "## v1.2.3 更新内容\n\n- 合成修复"}, "finish_reason": "stop"}]}).encode("utf-8"))

        environment = {"EVERYAPI_RELEASE_NOTES_TOKEN": "test-token"}
        if configured is not None:
            environment["EVERYAPI_RELEASE_NOTES_MODEL"] = configured
        with patch.dict(MODULE.os.environ, environment, clear=True), patch.object(MODULE.urllib.request, "urlopen", respond):
            check(MODULE.model_notes("Android", "1.2.3", groups, {}) is not None, "合成模型回复没有被接受")
        check(len(sent) == 1, "模型请求没有发出")
        payload = MODULE.json.loads(sent[0].data)
        check(payload["model"] == expected, f"模型配置 {configured!r} 没有选择预期的模型")


def check_model_reply_heading() -> None:
    # 标题不对的回复说明模型没照样例写，宁可退回确定性版本，也不把它发出去。
    # 截断的回复同样不能发：android-v0.3.0 曾停在半句话上，标题却是对的。
    replies = {
        ("## v1.2.3 更新内容\n\n### 🐛 问题修复\n\n- 修好了。(#1)", "stop"): True,
        ("## Android 1.2.3\n\n### 修复\n\n- 修好了。", "stop"): False,
        ("## v1.2.3 更新内容\n\n### 🐛 问题修复\n\n- 修好", "length"): False,
    }
    for (reply, finish), accepted in replies.items():
        def respond(request: object, timeout: float, reply: str = reply, finish: str = finish) -> object:
            return io.BytesIO(MODULE.json.dumps({"choices": [{"message": {"content": reply}, "finish_reason": finish}]}).encode("utf-8"))

        original = MODULE.urllib.request.urlopen
        MODULE.urllib.request.urlopen = respond
        MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"] = "test-token"
        try:
            notes = MODULE.model_notes("Android", "1.2.3", [{"pr": 1, "commits": [{"subject": "fix: 修好了", "commit": "1234567890", "paths": [], "body": ""}]}], {})
        finally:
            MODULE.urllib.request.urlopen = original
            del MODULE.os.environ["EVERYAPI_RELEASE_NOTES_TOKEN"]
        check((notes is not None) == accepted, f"模型回复的标题校验不对：{reply.splitlines()[0]!r}")


def check_pull_request_assignment() -> None:
    # develop 上两种合并方式并存：GitHub 的 merge commit（标题是 Merge pull request #N），和标题末尾带 (#N) 的 squash 提交。
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)

        def run(*args: str) -> None:
            subprocess.run(["git", *args], cwd=root, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def commit(name: str, subject: str) -> None:
            (root / "crates").mkdir(exist_ok=True)
            (root / "crates" / name).write_text(name, encoding="utf-8")
            run("add", ".")
            run("commit", "-q", "-m", subject)

        run("init", "-q", "-b", "develop")
        run("config", "user.name", "test")
        run("config", "user.email", "test@example.com")
        commit("base", "chore: base")
        run("tag", "start")
        run("checkout", "-q", "-b", "feature")
        commit("a", "fix(engine): 第一处")
        commit("b", "test(engine): 覆盖第一处")
        run("checkout", "-q", "develop")
        run("merge", "-q", "--no-ff", "feature", "-m", "Merge pull request #12 from org/feature")
        commit("c", "feat(engine): 新功能 (#13)")
        commit("d", "chore: 直接推送")
        # release PR 把 develop 带进 main，它的合并提交不能认领前面那些提交。
        run("checkout", "-q", "-b", "release/1.0.0")
        commit("e", "chore(release): set version to 1.0.0")
        run("checkout", "-q", "develop")
        run("checkout", "-q", "-b", "main", "start")
        run("merge", "-q", "--no-ff", "release/1.0.0", "-m", "Merge pull request #14 from org/release/1.0.0")
        records = MODULE.collect_commits(root, "start", "HEAD", "android")
    numbers = {str(record["subject"]): record["pr"] for record in records}
    check(numbers == {"fix(engine): 第一处": 12, "test(engine): 覆盖第一处": 12, "feat(engine): 新功能 (#13)": 13, "chore: 直接推送": None, "chore(release): set version to 1.0.0": None}, f"提交没有对上 PR：{numbers}")
    groups = MODULE.group_by_pull_request(records)
    check([group["pr"] for group in groups] == [12, 13, None, None], f"按 PR 分组不对：{[group['pr'] for group in groups]}")


def main() -> int:
    check(MODULE.platform_relevant(["platforms/android/java/app.kt"], "android"), "Android 路径没有被选中")
    check(MODULE.platform_relevant(["crates/engine/src/lib.rs"], "ios"), "纯共享改动没有被所有平台选中")
    check(MODULE.platform_relevant(["platforms/common/src/host.rs"], "windows"), "平台公共改动没有被所有平台选中")
    check(MODULE.platform_relevant(["apps/desktop/src-tauri/src/shared/voice.rs"], "android"), "Tauri 公共改动没有被所有平台选中")
    check(MODULE.platform_relevant(["platforms/android/java/app.kt", "packages/ui/src/index.tsx"], "android"), "平台与共享改动没有被选中")
    check(not MODULE.platform_relevant(["platforms/ios/App/App.swift"], "android"), "iOS 改动混入 Android 说明")
    check(not MODULE.platform_relevant(["platforms/ios/App/App.swift", "packages/ui/src/index.tsx"], "android"), "带 iOS 路径的共享改动混入 Android 说明")
    groups = [
        {"pr": 5, "commits": [{"subject": "fix(android): 修复键盘崩溃", "commit": "1234567890"}]},
        {"pr": 6, "commits": [{"subject": "feat(ui): 新增设置页", "commit": "2234567890"}]},
        {"pr": None, "commits": [{"subject": "ci: 缓存编译产物", "commit": "3234567890"}]},
    ]
    pulls = {
        5: {"title": "fix(android): 修复键盘崩溃", "body": "", "author": "someone", "issues": [{"number": 4, "author": "reporter"}]},
        6: {"title": "feat(ui): 新增设置页", "body": "", "author": "houko", "issues": []},
    }
    notes = MODULE.fallback_notes("1.2.3", groups, pulls)
    check(notes.startswith("## v1.2.3 更新内容\n"), "确定性说明的标题不是样例格式")
    check(notes.index("### ✨ 新功能") < notes.index("### 🐛 问题修复") < notes.index("### 🔧 开发与构建"), f"确定性说明的分组顺序不对：{notes}")
    check("- 修复键盘崩溃 (感谢 @someone，#5，修复 #4)" in notes, f"外部贡献者的条目没有致谢和 issue：{notes}")
    check("- 新增设置页 (#6)" in notes, f"维护者的条目不该带致谢：{notes}")
    check("- 缓存编译产物 (提交 `32345678`)" in notes, f"不属于 PR 的提交没有标出提交：{notes}")
    check_model_request_user_agent()
    check_model_configuration()
    check_model_reply_heading()
    check_pull_request_assignment()
    print("release notes platform filtering: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""门禁脚本里不许把输出用管道交给 `rg -q` / `grep -q`。

`rg -q` 和 `grep -q` 读到第一处命中就退出。上游的 printf、sed、rg 或别的命令若还有没写完的输出，再写就收到 SIGPIPE（`printf: write error: Broken pipe`），而门禁脚本都开着 pipefail，整条管道于是返回失败：同一份源码时过时不过，取决于上游分几次写、下游何时退出。`platforms/android/check-host.sh` 的「android host java」守卫就这样让推送和 CI 偶发失败。

变量改用 here-string（`rg -q pattern <<< "$var"`），命令的输出先整段取进来再判断（`rg -q pattern <<< "$(command)"`，或 check-host.sh 里先读完标准输入的 `rg_q_stdin`）。

扫描范围是门禁会跑到的 shell：`scripts/verify-local.sh`、`scripts/run-checks.sh`、`.githooks/*`、`platforms/*/check-*.sh`、`platforms/*/tests/run.sh`，加上它们和 PR 上的 CI workflow 用路径点名的 `.sh`（逐层展开）。只在手动触发的发版 workflow 里用到的脚本不在门禁上，不扫。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# PR 上运行的 workflow；release-*、publish-* 等手动触发的不算门禁。
GATE_WORKFLOWS = ("ci*.yml", "contracts.yml", "quality.yml")
ENTRY_GLOBS = (
    "scripts/verify-local.sh",
    "scripts/run-checks.sh",
    ".githooks/*",
    "platforms/*/check-*.sh",
    "platforms/*/tests/run.sh",
)
SCRIPT_PATH = re.compile(r"(?<![\w./-])((?:\.githooks|scripts|platforms)/[\w./-]+\.sh)\b")
# 单个 `|`（不是 `||`）后面紧跟 rg 或 grep，参数里带 -q 或 --quiet。
PIPE_TO_QUIET = re.compile(r"(?<!\|)\|(?!\|)\s*(rg|grep)\s+([^|;&]*)")
QUIET_FLAG = re.compile(r"(?:^|\s)(?:-[A-Za-z]*q[A-Za-z]*|--quiet)(?=\s|$)")


def strip_comment(line: str) -> str:
    """去掉行尾注释；引号里的 `#` 和 `$#`、`${#x}` 之类保留。"""
    quote = ""
    for index, char in enumerate(line):
        if quote:
            if char == quote:
                quote = ""
        elif char in "'\"":
            quote = char
        elif char == "#" and (index == 0 or line[index - 1] in " \t"):
            return line[:index]
    return line


def logical_lines(text: str):
    """把续行（行尾 `\\`）和以 `|` 结尾、命令接在下一行的管道并成一行，返回 (起始行号, 内容)。"""
    pending = ""
    start = 0
    for number, raw in enumerate(text.splitlines(), 1):
        line = strip_comment(raw).rstrip()
        if not pending:
            start = number
        if line.endswith("\\"):
            pending += line[:-1] + " "
            continue
        if line.endswith("|") and not line.endswith("||"):
            pending += line + " "
            continue
        yield start, pending + line
        pending = ""
    if pending:
        yield start, pending


def gate_scripts() -> list[pathlib.Path]:
    queue: list[pathlib.Path] = []
    for pattern in ENTRY_GLOBS:
        queue.extend(sorted(path for path in ROOT.glob(pattern) if path.is_file()))
    workflows = ROOT / ".github" / "workflows"
    for pattern in GATE_WORKFLOWS:
        for workflow in sorted(workflows.glob(pattern)):
            for match in SCRIPT_PATH.finditer(workflow.read_text(encoding="utf-8")):
                queue.append(ROOT / match.group(1))
    seen: dict[pathlib.Path, None] = {}
    while queue:
        path = queue.pop(0)
        if path in seen or not path.is_file():
            continue
        seen[path] = None
        for _, line in logical_lines(path.read_text(encoding="utf-8")):
            for match in SCRIPT_PATH.finditer(line):
                queue.append(ROOT / match.group(1))
    return list(seen)


def offenders(text: str) -> list[tuple[int, str]]:
    found = []
    for number, line in logical_lines(text):
        if any(QUIET_FLAG.search(match.group(2)) for match in PIPE_TO_QUIET.finditer(line)):
            found.append((number, line.strip()))
    return found


def self_test() -> None:
    """守住扫描本身：该抓的抓到，不该抓的放过。"""
    caught = [
        "printf '%s\\n' \"$body\" | rg -q 'x'",
        "sed -n '1p' f | grep -qF 'x'",
        "rustup target list | grep -Fxq x",
        "a \\\n  | rg -q 'x'",
        "a |\n  rg --quiet x",
    ]
    allowed = [
        "rg -q 'x' <<< \"$body\"",
        "rg -q 'x' file || rg -q 'y' file",
        "a | rg_q_stdin 'x'",
        "a | rg -n 'x' | head -n 1",
        "# a | rg -q 'x'",
        "a | grep -c 'q'",
    ]
    for text in caught:
        assert offenders(text), f"扫描漏掉了 {text!r}"
    for text in allowed:
        assert not offenders(text), f"扫描误报了 {text!r}"


def main() -> int:
    self_test()
    scripts = gate_scripts()
    failed = False
    for path in scripts:
        for number, line in offenders(path.read_text(encoding="utf-8")):
            failed = True
            print(f"{path.relative_to(ROOT)}:{number}: 管道交给了 rg -q / grep -q，pipefail 下上游会偶发 SIGPIPE: {line}")
    if failed:
        print("改用 here-string（`rg -q pattern <<< \"$var\"`、`<<< \"$(command)\"`），或 check-host.sh 的 rg_q_stdin")
        return 1
    print(f"gate pipe early exit: {len(scripts)} 个门禁 shell 脚本里没有交给 rg -q / grep -q 的管道")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""为发布 PR 生成预算内的提交摘要，完整历史保留在固定提交的对比链接中。"""

from __future__ import annotations

import argparse
from collections.abc import Iterable
from pathlib import Path
import subprocess


BODY_BYTE_BUDGET = 60000
MAX_COMMITS = 100


def render_body(prefix: str, subjects: Iterable[str], commit_count: int, compare_url: str) -> str:
    prefix = prefix.rstrip() + "\n\n## 新增提交\n\n"
    suffix = f"\n</details>\n\n[查看全部提交与改动]({compare_url})\n"
    selected: list[str] = []

    def compose() -> str:
        summary = f"共 {commit_count} 个提交，展示最近 {len(selected)} 项"
        rows = "\n".join(selected)
        return prefix + f"<details><summary>{summary}</summary>\n\n" + rows + "\n" + suffix

    if len(compose().encode("utf-8")) > BODY_BYTE_BUDGET:
        raise ValueError("发布 PR 的版本表和合并说明已超过正文预算，未推送分支")
    for subject in subjects:
        if len(selected) == MAX_COMMITS:
            break
        selected.append(f"- {subject}")
        if len(compose().encode("utf-8")) > BODY_BYTE_BUDGET:
            selected.pop()
            break
    return compose()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix-file", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--since", required=True)
    parser.add_argument("--develop", required=True)
    parser.add_argument("--commit-count", type=int, required=True)
    parser.add_argument("--repo-url", required=True)
    args = parser.parse_args()
    subjects = subprocess.check_output(
        ["git", "log", "--first-parent", f"--max-count={MAX_COMMITS}", "--encoding=UTF-8", "--format=%s",
         f"{args.since}..{args.develop}"],
        encoding="utf-8",
    ).splitlines()
    compare_url = f"{args.repo_url.rstrip('/')}/compare/{args.since}...{args.develop}"
    try:
        body = render_body(args.prefix_file.read_text(encoding="utf-8"), subjects,
                           args.commit_count, compare_url)
    except ValueError as error:
        parser.error(str(error))
    args.output.write_text(body, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

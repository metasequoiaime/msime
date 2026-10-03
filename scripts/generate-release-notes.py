#!/usr/bin/env python3
"""为一个独立平台生成只包含该平台改动的发布说明。"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import urllib.error
import urllib.request


COMMON_PREFIXES = (
    "crates/",
    "packages/ui/",
    "platforms/common/",
    "shared/",
    "apps/desktop/src-tauri/src/shared/",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "package.json",
    "pnpm-lock.yaml",
)

PLATFORMS = {
    "android": {"label": "Android", "prefixes": ("platforms/android/", "apps/desktop/src-tauri/src/platform/android/")},
    "harmony": {"label": "HarmonyOS", "prefixes": ("platforms/harmony/",)},
    "ios": {"label": "iOS", "prefixes": ("platforms/ios/", "apps/desktop/src-tauri/src/platform/ios/")},
    "linux": {"label": "Linux", "prefixes": ("platforms/linux/", "apps/desktop/src-tauri/src/platform/linux/")},
    "macos": {"label": "macOS", "prefixes": ("platforms/macos/", "apps/desktop/src-tauri/src/platform/macos/")},
    "windows": {"label": "Windows", "prefixes": ("platforms/windows/", "apps/desktop/src-tauri/src/platform/windows/")},
}


def git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout


def platform_relevant(paths: list[str], platform: str) -> bool:
    """只接受当前平台的路径，或没有任何平台专属路径的共享改动。"""
    own = PLATFORMS[platform]["prefixes"]
    other_platform_prefixes = tuple(
        prefix
        for name, config in PLATFORMS.items()
        if name != platform
        for prefix in config["prefixes"]
    )
    has_own = any(path.startswith(prefix) for path in paths for prefix in own)
    has_other = any(path.startswith(prefix) for path in paths for prefix in other_platform_prefixes)
    has_common = any(path.startswith(prefix) or path == prefix for path in paths for prefix in COMMON_PREFIXES)
    return has_own or (has_common and not has_other)


def collect_commits(root: Path, start_tag: str | None, end_ref: str, platform: str) -> list[dict[str, object]]:
    revision_range = f"{start_tag}..{end_ref}" if start_tag else end_ref
    # 一次 git 调用读出提交和路径，避免首个版本没有平台 tag 时对整个历史逐提交启动子进程。
    output = git(root, "log", "--reverse", "--no-merges", "--format=__COMMIT__%H%x00%s", "--name-only", revision_range)
    records: list[dict[str, object]] = []
    current: dict[str, object] | None = None
    for line in output.splitlines():
        if line.startswith("__COMMIT__"):
            if current and platform_relevant(list(current["paths"]), platform):
                records.append(current)
            commit, subject = line.removeprefix("__COMMIT__").split("\x00", 1)
            current = {"commit": commit, "subject": subject, "body": "", "paths": []}
        elif current and line:
            current["paths"].append(line)
    if current and platform_relevant(list(current["paths"]), platform):
        records.append(current)
    return records


def commit_context(records: list[dict[str, object]], limit: int = 180) -> str:
    selected = records[-limit:]
    lines: list[str] = []
    if len(records) > limit:
        lines.append(f"（提交过多，只提供最近 {limit} 条；不要猜测未提供的提交。）")
    for record in selected:
        paths = ", ".join(str(path) for path in list(record["paths"])[:12])
        suffix = " …" if len(list(record["paths"])) > 12 else ""
        lines.append(f"[{str(record['commit'])[:8]}] {record['subject']}\n路径：{paths}{suffix}")
        body = str(record["body"])
        if body:
            lines.append(f"正文：{body[:800]}")
    return "\n".join(lines)


def fallback_notes(label: str, version: str, records: list[dict[str, object]]) -> str:
    lines = [f"## {label} {version}", ""]
    if not records:
        lines.append("本次版本没有检测到该平台相关的代码改动。")
        return "\n".join(lines) + "\n"
    if len(records) > 180:
        lines.append(f"本次版本的改动（最近 180 条）：")
        records = records[-180:]
    else:
        lines.append("本次版本的改动：")
    for record in records:
        lines.append(f"- {record['subject']}（提交 `{str(record['commit'])[:8]}`）")
    return "\n".join(lines) + "\n"


def model_notes(label: str, version: str, records: list[dict[str, object]]) -> str | None:
    token = next(
        (os.environ.get(name, "").strip() for name in ("EVERYAPI_RELEASE_NOTES_TOKEN", "EVERYAPI_API_KEY", "EVERYAPI_TOKEN") if os.environ.get(name, "").strip()),
        "",
    )
    if not token or not records:
        return None
    endpoint = os.environ.get("EVERYAPI_RELEASE_NOTES_ENDPOINT", "https://api.everyapi.ai/v1/chat/completions")
    model = os.environ.get("EVERYAPI_RELEASE_NOTES_MODEL", "gpt-5.5")
    prompt = f"""你是水杉输入法的发行说明编辑。请为 {label} {version} 生成简洁的简体中文 Markdown 发布说明。

只根据下面的提交生成内容。只写会影响 {label} 用户的改动；纯共享代码改动只有在确实会进入 {label} 产品时才写。绝对不要列出其他平台的功能，不要发明提交中没有的行为，不要写测试过程、内部路径或凭据。

输出要求：第一行是 `## {label} {version}`；后面按“新增”“修复”“改进”分组（没有内容的分组省略），每个条目一句话。不要输出代码围栏、前言或结语。

提交：
{commit_context(records)}"""
    payload = json.dumps(
        {
            "model": model,
            "temperature": 0.2,
            "messages": [
                {"role": "system", "content": "你只输出符合要求的 Markdown，不要输出分析。"},
                {"role": "user", "content": prompt},
            ],
        },
        ensure_ascii=False,
    ).encode("utf-8")
    request = urllib.request.Request(
        endpoint,
        data=payload,
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            document = json.load(response)
        content = document["choices"][0]["message"]["content"]
        if not isinstance(content, str) or not content.strip():
            raise ValueError("模型返回了空内容")
        content = content.strip()
        if content.startswith("```") and content.endswith("```"):
            content = content.split("\n", 1)[1].rsplit("\n", 1)[0].strip()
        if not content.startswith("## ") or len(content) > 12000:
            raise ValueError("模型返回的发布说明格式不符合约束")
        return content + "\n"
    except (OSError, ValueError, KeyError, IndexError, TypeError, json.JSONDecodeError, urllib.error.URLError) as error:
        print(f"EveryAPI 发布说明生成失败，使用确定性平台说明：{error}", file=sys.stderr)
        return None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=sorted(PLATFORMS), required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--start-tag", default="")
    parser.add_argument("--end-ref", default="HEAD")
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    records = collect_commits(root, args.start_tag or None, args.end_ref, args.platform)
    notes = model_notes(PLATFORMS[args.platform]["label"], args.version, records)
    if notes is None:
        notes = fallback_notes(PLATFORMS[args.platform]["label"], args.version, records)
    args.output.write_text(notes, encoding="utf-8")
    print(f"generated {args.output} with {len(records)} {args.platform} commits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

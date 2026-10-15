#!/usr/bin/env python3
"""为一个独立平台生成只包含该平台改动的发布说明。"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
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

# 维护者自己的 PR 和自己开的 issue 不进「感谢」，否则几乎每一条都会挂上同一个名字。
MAINTAINERS = frozenset({"houko"})

# 发布说明的格式以这份样例为准，模型按它的结构和措辞写，兜底版本也用同一套分组标题。
EXAMPLE_PATH = Path(__file__).with_name("release-notes-example.md")

MERGE_SUBJECT = re.compile(r"^Merge pull request #(\d+) from [^/\s]+/(\S+)")
SQUASH_SUBJECT = re.compile(r"\(#(\d+)\)$")
CONVENTIONAL_SUBJECT = re.compile(r"^(\w+)(?:\([^)]*\))?!?:\s*")

FALLBACK_SECTIONS = (
    ("feat", "### ✨ 新功能"),
    ("perf", "### ⚡ 性能"),
    ("fix", "### 🐛 问题修复"),
    ("other", "### 🔧 开发与构建"),
)


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
    # 一次 git 调用读出提交、正文和路径，避免首个版本没有平台 tag 时对整个历史逐提交启动子进程。
    output = git(root, "log", "--reverse", "--no-merges", "--format=%x1e%H%x00%s%x00%b%x1f", "--name-only", revision_range)
    records: list[dict[str, object]] = []
    for chunk in output.split("\x1e")[1:]:
        header, _, names = chunk.partition("\x1f")
        commit, subject, body = header.split("\x00", 2)
        paths = [line for line in names.splitlines() if line]
        if platform_relevant(paths, platform):
            records.append({"commit": commit, "subject": subject, "body": body.strip(), "paths": paths, "pr": None})
    assign_pull_requests(root, revision_range, records)
    return records


def assign_pull_requests(root: Path, revision_range: str, records: list[dict[str, object]]) -> None:
    """给每个提交标上它所属的 PR：squash 进来的看标题末尾的 (#N)，merge commit 合进来的看合并提交。

    release/* 分支的 PR 是把整个 develop 带进 main，不是一项改动，跳过；否则它会认领区间里所有还没归属的提交，android-v0.2.0 有 355 个 squash 提交就这样被算到了 release PR 名下。
    """
    owner: dict[str, int] = {}
    merges = git(root, "log", "--merges", "--reverse", "--format=%H %P%x00%s", revision_range)
    for line in merges.splitlines():
        parents, _, subject = line.partition("\x00")
        match = MERGE_SUBJECT.match(subject)
        hashes = parents.split()
        if not match or len(hashes) != 3 or match.group(2).startswith("release/"):
            continue
        for commit in git(root, "rev-list", "--no-merges", f"{hashes[1]}..{hashes[2]}").split():
            owner.setdefault(commit, int(match.group(1)))
    for record in records:
        squash = SQUASH_SUBJECT.search(str(record["subject"]))
        record["pr"] = int(squash.group(1)) if squash else owner.get(str(record["commit"]))


def fetch_pull_requests(numbers: list[int]) -> dict[int, dict[str, object]]:
    """经 gh 读 PR 的标题、正文、作者和它关闭的 issue；拿不到时返回空表，说明照样生成，只是少了致谢和 issue 编号。"""
    repository = os.environ.get("GITHUB_REPOSITORY", "metasequoiaime/msime")
    owner, name = repository.split("/", 1)
    result: dict[int, dict[str, object]] = {}
    for start in range(0, len(numbers), 40):
        fields = " ".join(
            f"pr{number}: pullRequest(number: {number}) {{ title body author {{ login }} closingIssuesReferences(first: 10) {{ nodes {{ number author {{ login }} }} }} }}"
            for number in numbers[start : start + 40]
        )
        query = f'query {{ repository(owner: "{owner}", name: "{name}") {{ {fields} }} }}'
        try:
            completed = subprocess.run(["gh", "api", "graphql", "-f", f"query={query}"], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=60)
            repository_data = json.loads(completed.stdout)["data"]["repository"]
        except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError) as error:
            stderr = getattr(error, "stderr", "") or ""
            print(f"读取 PR 信息失败，发布说明不带致谢和 issue 编号：{error} {stderr[:300]}", file=sys.stderr)
            return {}
        for key, pull in repository_data.items():
            if not pull:
                continue
            issues = [
                {"number": node["number"], "author": (node.get("author") or {}).get("login", "")}
                for node in pull["closingIssuesReferences"]["nodes"]
            ]
            result[int(key.removeprefix("pr"))] = {
                "title": pull["title"],
                "body": pull["body"] or "",
                "author": (pull.get("author") or {}).get("login", ""),
                "issues": issues,
            }
    return result


def credited(login: str) -> bool:
    return bool(login) and login not in MAINTAINERS and not login.endswith("[bot]")


def group_by_pull_request(records: list[dict[str, object]]) -> list[dict[str, object]]:
    """按 PR 合并提交，保持首次出现的顺序；不属于任何 PR 的提交各自成一组。"""
    groups: list[dict[str, object]] = []
    by_number: dict[int, dict[str, object]] = {}
    for record in records:
        number = record.get("pr")
        if number is None:
            groups.append({"pr": None, "commits": [record]})
        elif number in by_number:
            by_number[number]["commits"].append(record)
        else:
            by_number[number] = {"pr": number, "commits": [record]}
            groups.append(by_number[number])
    return groups


def change_context(groups: list[dict[str, object]], pulls: dict[int, dict[str, object]], limit: int = 1500) -> str:
    selected = groups[-limit:]
    # 大版本动辄上千个 PR（android-v0.2.1 有 1242 组），只给最近一部分会把前面的改动整个漏掉；改为组数越多每组给得越少，总量压在十万 token 上下。
    detail = "full" if len(selected) <= 180 else "brief" if len(selected) <= 500 else "title"
    lines: list[str] = []
    if len(groups) > limit:
        lines.append(f"（改动过多，只提供最近 {limit} 组；不要猜测未提供的改动。）")
    for group in selected:
        commits = list(group["commits"])
        number = group["pr"]
        pull = pulls.get(number) if number is not None else None
        if number is None:
            lines.append(f"[提交 {str(commits[0]['commit'])[:8]}，不属于任何 PR]")
        elif pull is None:
            lines.append(f"[PR #{number}]")
        else:
            author = f"@{pull['author']}" if credited(str(pull["author"])) else "维护者（不致谢）"
            issues = "、".join(
                f"#{issue['number']}（{'@' + issue['author'] if credited(str(issue['author'])) else '维护者自提'}）"
                for issue in pull["issues"]
            )
            lines.append(f"[PR #{number}] 作者：{author}" + (f"；关闭 issue：{issues}" if issues else ""))
            lines.append(f"PR 标题：{pull['title']}")
            body = str(pull["body"]).strip()
            if body and detail != "title":
                lines.append(f"PR 正文：{body[:800 if detail == 'full' else 200]}")
            if detail != "full" and len(commits) == 1:
                lines.append("")
                continue
        for record in commits[:20 if detail == "full" else 5]:
            lines.append(f"- 提交：{record['subject']}")
            body = str(record.get("body", ""))
            if body and pull is None and detail != "title":
                lines.append(f"  正文：{body[:400 if detail == 'full' else 200]}")
        shown = 20 if detail == "full" else 5
        if len(commits) > shown:
            lines.append(f"- ……另有 {len(commits) - shown} 个提交")
        if detail == "full":
            paths = sorted({str(path) for record in commits for path in list(record.get("paths", []))})
            lines.append(f"路径：{', '.join(paths[:12])}" + (" …" if len(paths) > 12 else ""))
        lines.append("")
    return "\n".join(lines)


def fallback_notes(version: str, groups: list[dict[str, object]], pulls: dict[int, dict[str, object]]) -> str:
    lines = [f"## v{version} 更新内容", ""]
    if not groups:
        lines.append("本次版本没有检测到该平台相关的代码改动。")
        return "\n".join(lines) + "\n"
    if len(groups) > 180:
        lines += ["只列出最近 180 项改动。", ""]
        groups = groups[-180:]
    sections: dict[str, list[str]] = {key: [] for key, _ in FALLBACK_SECTIONS}
    for group in groups:
        commits = list(group["commits"])
        number = group["pr"]
        pull = pulls.get(number) if number is not None else None
        subject = str(pull["title"]) if pull else str(commits[0]["subject"])
        if number is not None:
            subject = SQUASH_SUBJECT.sub("", subject).rstrip()
        match = CONVENTIONAL_SUBJECT.match(subject)
        kind = match.group(1) if match and match.group(1) in ("feat", "perf", "fix") else "other"
        text = subject[match.end():] if match else subject
        refs: list[str] = []
        if pull and credited(str(pull["author"])):
            refs.append(f"感谢 @{pull['author']}")
        refs.append(f"#{number}" if number is not None else f"提交 `{str(commits[0]['commit'])[:8]}`")
        if pull and pull["issues"]:
            refs.append("修复 " + ", ".join(f"#{issue['number']}" for issue in pull["issues"]))
        sections[kind].append(f"- {text} ({'，'.join(refs)})")
    for key, heading in FALLBACK_SECTIONS:
        if sections[key]:
            lines += [heading, "", *sections[key], ""]
    return "\n".join(lines).rstrip() + "\n"


def model_notes(label: str, version: str, groups: list[dict[str, object]], pulls: dict[int, dict[str, object]]) -> str | None:
    token = next(
        (os.environ.get(name, "").strip() for name in ("EVERYAPI_RELEASE_NOTES_TOKEN", "EVERYAPI_API_KEY", "EVERYAPI_TOKEN") if os.environ.get(name, "").strip()),
        "",
    )
    if not token or not groups:
        return None
    endpoint = os.environ.get("EVERYAPI_RELEASE_NOTES_ENDPOINT", "https://api.everyapi.ai/v1/chat/completions")
    model = os.environ.get("EVERYAPI_RELEASE_NOTES_MODEL", "").strip() or "claude-sonnet-5-5"
    heading = f"## v{version} 更新内容"
    prompt = f"""你是水杉输入法的发行说明编辑。请为 {label} {version} 写简体中文 Markdown 发布说明，格式和措辞照下面的样例。

<样例>
{EXAMPLE_PATH.read_text(encoding="utf-8").strip()}
</样例>

样例只用来示范格式和写法，里面的功能、编号、人名一个都不要照抄，内容只能来自后面列出的改动。

内容范围：
- 只写会影响 {label} 用户的改动；纯共享代码改动只有在确实会进入 {label} 产品时才写。绝对不要列出其他平台的功能，不要发明改动里没有的行为，不要写测试过程、内部路径、函数名或凭据。
- 纯内部的重构、测试、注释和文档不写。同一件事的多个 PR 合成一条；一批细碎的同类修复（比如十几处文件读写加固）合成一条讲清楚用户得到了什么。

结构：
- 第一行固定是 `{heading}`，前面不要有任何内容。
- 二级分组按这个顺序：`### ✨ 新功能`、`### ⚡ 性能`、`### 🐛 问题修复`、`### 🔧 开发与构建`、`### ⚠️ 升级须知`、`### 🙏 感谢`。没有内容的分组整段省略。
- 只有当一批改动围绕同一个主题、而且是这一版最大的看点时，才在「新功能」之前加一个专题分组，标题写成 `### <合适的 emoji> <主题>：<一句话概括>`，像样例的「📚 词库：中译英释义大更新」。
- 「新功能」和「问题修复」里涉及两个以上领域时，用单独一行的粗体领域名分小组（如 `**输入**`、`**设置与界面**`），小组之间空一行；只改了新安装默认值的单独放进 `**新安装的默认配置**（老用户升级不受影响）`。
- 「性能」写用户感受得到的提升，有数字就写数字。「开发与构建」只写贡献者和打包者关心的变化：签名、CI、构建方式、诊断埋点。
- 「升级须知」只在升级后需要用户自己处理、或降级会出问题时才写，每条以粗体短句开头。

条目写法：
- 每条一到两句，说用户看到了什么变化，不要用 feat/fix 这类前缀，不要写提交哈希。能举例就举例，按键和输入用反引号，比如「`nihao` 选了「你」再按逗号」。
- 重要的新能力可以用 `**名称**：` 开头。
- 每条末尾用半角括号标注来源，括号内用全角逗号分隔：先写「感谢 @作者」（只有改动里标了 @作者 的才写，标着「维护者（不致谢）」的不写），再写 PR 编号（多个用 `#1, #2`），最后写 issue 关系：修好了 issue 写「修复 #N」，只是相关写「相关 #N」，按 issue 的要求实现写「对应 #N」。比如 `(感谢 @someone，#12, #15，修复 #10)`、`(#20)`。不属于任何 PR 的提交不写编号。

「感谢」分组：
- 「贡献者」下每人一行 `@作者：做了什么`，按贡献多少排序；只列改动里标了 @作者 的人。
- 「问题反馈」列出由 @用户 提出的 issue 编号（标着「维护者自提」的不列），写成「#1、#2 的各位报告者」。
- 两项都没有就省略整个分组。

不要输出代码围栏、前言或结语。

改动（按 PR 分组，按合并顺序）：
{change_context(groups, pulls)}"""
    payload = json.dumps(
        {
            # 不带 temperature：GPT-5 系列的推理模型只接受默认值，带别的值会被直接拒绝（HTTP 400），windows-v0.2.1 的发布说明就是这样退回了确定性版本。
            "model": model,
            # 不写的话由网关给默认上限，按样例写的长说明会被截断：android-v0.3.0 停在半句话上，改动多的版本整段返回空内容。
            "max_tokens": 32000,
            "messages": [
                {"role": "system", "content": "你只输出符合要求的 Markdown，不要输出分析。"},
                {"role": "user", "content": prompt},
            ],
        },
        ensure_ascii=False,
    ).encode("utf-8")
    # Cloudflare in front of api.everyapi.ai rejects urllib's default `Python-urllib/3.x` with 403 (error 1010) before the API sees the request, so name the client explicitly.
    request = urllib.request.Request(
        endpoint,
        data=payload,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "User-Agent": "msime-release-notes (+https://github.com/metasequoiaime/msime)",
        },
        method="POST",
    )
    try:
        # 按样例写一整份说明比原来的三段式长得多，推理模型常要一两分钟，45 秒会在出结果前超时。
        with urllib.request.urlopen(request, timeout=300) as response:
            document = json.load(response)
        choice = document["choices"][0]
        if choice.get("finish_reason") == "length":
            raise ValueError("模型输出达到长度上限被截断")
        content = choice["message"]["content"]
        if not isinstance(content, str) or not content.strip():
            raise ValueError("模型返回了空内容")
        content = content.strip()
        if content.startswith("```") and content.endswith("```"):
            content = content.split("\n", 1)[1].rsplit("\n", 1)[0].strip()
        if not content.startswith(heading + "\n") or len(content) > 30000:
            raise ValueError("模型返回的发布说明格式不符合约束")
        return content + "\n"
    except (OSError, ValueError, KeyError, IndexError, TypeError, json.JSONDecodeError, urllib.error.URLError) as error:
        # HTTPError 本身只说状态码，拒绝的原因在响应正文里；带上它，下次失败从日志就能看出是哪个参数。
        detail = ""
        if isinstance(error, urllib.error.HTTPError):
            try:
                detail = f"：{error.read().decode('utf-8', 'replace')[:500]}"
            except OSError:
                pass
        print(f"EveryAPI 发布说明生成失败，使用确定性平台说明：{error}{detail}", file=sys.stderr)
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
    groups = group_by_pull_request(records)
    pulls = fetch_pull_requests(sorted({int(group["pr"]) for group in groups if group["pr"] is not None}))
    notes = model_notes(PLATFORMS[args.platform]["label"], args.version, groups, pulls)
    if notes is None:
        notes = fallback_notes(args.version, groups, pulls)
    args.output.write_text(notes, encoding="utf-8")
    print(f"generated {args.output} with {len(records)} {args.platform} commits in {len(groups)} changes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

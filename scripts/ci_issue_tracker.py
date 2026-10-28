#!/usr/bin/env python3
"""develop 上的红灯跟踪 issue：CI 失败（ci-red-tracker.yml）和 Android APK 体积预警（nightly.yml 的 android-size）。

两类 issue 都是「一件事一张单」：同一个 workflow 的连续失败只开一张、之后追加评论，转绿时自动关闭；APK 体积同理，任一版本越过预警线时开单或追加，全部回到线下时关闭。哪张单属于谁靠正文里的 HTML 注释标记认，不靠标题，标题可以被人改。

所有 GitHub 调用都经 `gh` 进行，集中在 GitHub 类里；scripts/test-ci-issue-tracker.py 换成假的实现来测判定逻辑。写进 issue 的只有 job 名称、链接、提交号和提交标题，不放任何日志内容。
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ASSIGNEE = "houko"
BRANCH = "develop"
# 只跟踪合进 develop 之后的运行；pull_request 的红是 PR 自己的事，不进跟踪单。
TRACKED_EVENTS = ("push", "schedule", "workflow_dispatch")
# startup_failure 是运行根本没能启动（workflow 文件写错、复用的 workflow 引用失效、权限不匹配），nightly「悄悄不跑」就是这种，算红。action_required 是运行在等人批准、一个 job 都没跑，只出现在外部贡献者的 PR 上，这里不跟踪 PR，也说明不了代码红绿，和取消、跳过一样不算。
RED = ("failure", "timed_out", "startup_failure")
DECISIVE = RED + ("success",)
RED_LABEL = ("ci-red", "B60205", "develop 或 nightly 的 CI 失败，先修红再合功能 PR")
SIZE_LABEL = ("apk-size", "FBCA04", "Android 瘦包体积越过预警线")
SIZE_MARKER = "<!-- apk-size-tracker -->"
SIZE_DATA = re.compile(r"<!-- apk-size-data (\{.*?\}) -->")


class GitHub:
    """经 gh CLI 访问当前仓库（GITHUB_REPOSITORY）；令牌来自 GH_TOKEN。"""

    def __init__(self, repository):
        self.repository = repository

    def _gh(self, *args, stdin=None):
        return subprocess.run(["gh", *args], check=True, capture_output=True, text=True, input=stdin).stdout

    def _api(self, path, paginate=False):
        args = ["api", "-H", "Accept: application/vnd.github+json"]
        if paginate:
            args += ["--paginate", "--slurp"]
        return json.loads(self._gh(*args, f"repos/{self.repository}/{path}"))

    def run(self, run_id):
        return self._api(f"actions/runs/{run_id}")

    def workflow_runs(self, workflow_id):
        # 一页 100 次足够找到上一次成功；更长的连续失败在正文里写明超出了查询窗口。
        return self._api(f"actions/workflows/{workflow_id}/runs?branch={BRANCH}&status=completed&per_page=100")["workflow_runs"]

    def jobs(self, run_id, attempt):
        pages = self._api(f"actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100", paginate=True)
        return [job for page in pages for job in page["jobs"]]

    def ensure_label(self, label):
        name, color, description = label
        # --force 让已存在与并发创建都算成功，见 2026-10-27-conflict-label-idempotent-create.md。
        self._gh("label", "create", name, "--repo", self.repository, "--color", color, "--description", description, "--force")

    def open_issues(self, label):
        return json.loads(self._gh("issue", "list", "--repo", self.repository, "--label", label, "--state", "open",
                                   "--limit", "100", "--json", "number,title,body"))

    def comments(self, number):
        pages = self._api(f"issues/{number}/comments?per_page=100", paginate=True)
        return [comment["body"] for page in pages for comment in page]

    def create_issue(self, title, body, label):
        url = self._gh("issue", "create", "--repo", self.repository, "--title", title, "--body-file", "-",
                       "--label", label, "--assignee", ASSIGNEE, stdin=body).strip()
        return int(url.rstrip("/").rsplit("/", 1)[-1])

    def comment(self, number, body):
        self._gh("issue", "comment", str(number), "--repo", self.repository, "--body-file", "-", stdin=body)

    def edit_body(self, number, body):
        self._gh("issue", "edit", str(number), "--repo", self.repository, "--body-file", "-", stdin=body)

    def close(self, number, body):
        self.comment(number, body)
        self._gh("issue", "close", str(number), "--repo", self.repository, "--reason", "completed")


def has_marker(text, marker):
    # 标记只认独占一行的：提交标题这类外来文字和标记写在同一行里，就算原样含有一个标记，也冒充不了另一个 workflow 的单或「已经报过」的运行。
    return any(line.strip() == marker for line in (text or "").splitlines())


def find_issue(issues, marker):
    # 理论上每个标记只有一张开着的单；万一有人手工复制出第二张，跟踪最早的那张。
    matches = sorted((issue for issue in issues if has_marker(issue.get("body"), marker)), key=lambda issue: issue["number"])
    return matches[0] if matches else None


def already_reported(api, number, body, marker):
    return has_marker(body, marker) or any(has_marker(comment, marker) for comment in api.comments(number))


def inert(text):
    """外来文字（提交标题）写进 issue 前的处理：反引号换掉，好整段放进代码样式；HTML 注释的起止符拆开，渲染时藏不住后面的内容，也不会被当成标记。"""
    return text.replace("`", "'").replace("<!--", "<!- -").replace("-->", "- ->")


def commit_line(server, repository, run):
    sha = run["head_sha"]
    message = ((run.get("head_commit") or {}).get("message") or "").splitlines()
    # 提交标题放进代码样式里，免得里面的 @ 提及或 #编号在 issue 里通知人、链出去。
    subject = inert((message[0] if message else "").strip()[:200])
    link = f"[`{sha[:10]}`]({server}/{repository}/commit/{sha})"
    return f"{link} `{subject}`" if subject else link


def run_link(run):
    return f"[#{run['run_number']}]({run['html_url']})"


def red_streak(runs, current):
    """从当前运行往前数连续失败的运行，返回（连续失败列表，上一次成功或 None）。"""
    ordered = sorted({run["id"]: run for run in [*runs, current]}.values(), key=lambda run: run["run_number"], reverse=True)
    streak = []
    for run in ordered:
        if run["run_number"] > current["run_number"]:
            continue
        if run["conclusion"] == "success":
            return streak, run
        streak.append(run)
    return streak, None


def job_lines(jobs):
    failed = [job for job in jobs if job.get("conclusion") in RED]
    if not failed:
        return ["- 没有找到失败的 job（可能是 workflow 文件出错或运行没能启动），请看运行页。"]
    labels = {"failure": "失败", "timed_out": "超时", "startup_failure": "没能启动"}
    return [f"- [{job['name']}]({job['html_url']})（{labels[job['conclusion']]}）" for job in failed]


def track_red_build(api, run_id, server="https://github.com"):
    run = api.run(run_id)
    name, conclusion = run["name"], run["conclusion"]
    if run["head_branch"] != BRANCH or run["event"] not in TRACKED_EVENTS:
        print(f"{name} #{run['run_number']}: {run['event']} on {run['head_branch']} is not tracked")
        return
    if conclusion not in DECISIVE:
        print(f"{name} #{run['run_number']}: conclusion {conclusion} is neither red nor green; nothing to do")
        return
    runs = [other for other in api.workflow_runs(run["workflow_id"])
            if other["event"] in TRACKED_EVENTS and other["head_branch"] == BRANCH and other["conclusion"] in DECISIVE]
    # 事件可能乱序到达，旧运行也可能被重跑：只让最新一次有结论的运行决定开单还是关单，否则旧的失败会重开已经转绿的单，旧的成功会关掉仍然红着的单。触发这次的是更旧的运行时改按最新那次判定，而不是跳过：最新那次自己的跟踪运行可能在并发组里排队时被这次顶掉了，跳过就没人替它开单或关单。重复处理同一次运行由下面的运行标记挡住。
    latest = max([run, *runs], key=lambda other: other["run_number"])
    if latest["id"] != run["id"]:
        print(f"{name} #{run['run_number']}: newer run #{latest['run_number']} concluded {latest['conclusion']}; deciding from it")
        run, conclusion = latest, latest["conclusion"]

    marker = f"<!-- ci-red-tracker workflow={run['path']} -->"
    run_marker = f"<!-- ci-red-tracker run={run['id']} attempt={run['run_attempt']} -->"
    repository = api.repository
    # 先确保标签存在：之后按它列出开着的单，新开的单也要带它。
    api.ensure_label(RED_LABEL)
    issue = find_issue(api.open_issues(RED_LABEL[0]), marker)

    if conclusion == "success":
        if issue is None:
            print(f"{name} #{run['run_number']} is green and no tracking issue is open")
            return
        api.close(issue["number"], "\n".join([
            run_marker,
            f"已转绿：{run_link(run)}，提交 {commit_line(server, repository, run)}。自动关闭；同一个 workflow 再红时会开新的跟踪单。",
        ]))
        print(f"closed #{issue['number']} for {name}")
        return

    if conclusion == "startup_failure":
        # 没能启动的运行一个 job 都没有，原因只在运行页顶部的报错里。
        jobs = ["- 运行没能启动（startup_failure），通常是 workflow 文件写错、复用的 workflow 引用失效或权限配置不对，请看运行页顶部的报错。"]
    else:
        jobs = job_lines(api.jobs(run["id"], run["run_attempt"]))
    attempt = f"（第 {run['run_attempt']} 次尝试）" if run["run_attempt"] > 1 else ""
    if issue is not None:
        if already_reported(api, issue["number"], issue["body"], run_marker):
            print(f"{name} #{run['run_number']} is already reported on #{issue['number']}")
            return
        api.comment(issue["number"], "\n".join([
            run_marker,
            f"又失败一次：{run_link(run)}{attempt}，{run['event']}，提交 {commit_line(server, repository, run)}。",
            "",
            "失败的 job：",
            *jobs,
        ]))
        print(f"commented on #{issue['number']} for {name} #{run['run_number']}")
        return

    streak, green = red_streak(runs, run)
    first = streak[-1]
    if green is None:
        history = f"最近 {len(runs)} 次有结论的运行里没有成功的，连续失败至少从 {run_link(first)}（提交 `{first['head_sha'][:10]}`）开始。"
    elif first["id"] == run["id"]:
        history = f"这是上一次成功（{run_link(green)}，提交 `{green['head_sha'][:10]}`）之后的第一次失败。"
    else:
        history = (f"上一次成功之后的第一次失败是 {run_link(first)}（提交 `{first['head_sha'][:10]}`，{first['created_at']}），"
                   f"到这次已连续失败 {len(streak)} 次；上一次成功是 {run_link(green)}（提交 `{green['head_sha'][:10]}`）。")
    body = "\n".join([
        marker,
        run_marker,
        f"`{name}` 在 `{BRANCH}` 上失败了。这张单跟踪它直到转绿：之后每次失败追加一条评论，转绿时由 `ci-red-tracker.yml` 自动关闭。",
        "",
        f"- 运行：{run_link(run)}{attempt}，{run['event']}，{run['created_at']}",
        f"- 提交：{commit_line(server, repository, run)}",
        f"- 起点：{history}",
        "",
        "失败的 job：",
        *jobs,
        "",
        "按 `AGENTS.md` 的约定，develop 的推送 CI 或 nightly 红着时先修红，修好之前不合并功能 PR；暂时修不了的，在这里写明原因和计划。",
    ])
    number = api.create_issue(f"{name} 失败：{run['created_at'][:10]}", body, RED_LABEL[0])
    print(f"opened #{number} for {name} #{run['run_number']}")


def megabytes(value):
    return f"{value / 1_000_000:.1f} MB"


def size_rows(reports, previous):
    rows = ["| 版本 | APK | 字节 | 较上次 | 距上限 | 状态 |", "|---|---|---:|---:|---:|---|"]
    for report in sorted(reports, key=lambda report: report["edition"]):
        before = previous.get(report["edition"])
        delta = f"{report['bytes'] - before:+,}" if before is not None else "—"
        if not report["checked"]:
            headroom, state = "—", "不设上限（非瘦包）"
        else:
            headroom = megabytes(report["max_bytes"] - report["bytes"])
            if report["bytes"] > report["max_bytes"]:
                state = "**超过上限**"
            elif report["bytes"] > report["warn_bytes"]:
                state = "**超过预警线**"
            else:
                state = "正常"
        rows.append(f"| {report['edition']} | `{report['apk']}` | {report['bytes']:,} | {delta} | {headroom} | {state} |")
    return rows


def load_reports(directory):
    reports = {}
    for path in sorted(Path(directory).rglob("*.json")) if Path(directory).is_dir() else []:
        report = json.loads(path.read_text(encoding="utf-8"))
        reports[report["edition"]] = report
    return list(reports.values())


def track_apk_size(api, directory, editions, run_url, sha, now=None):
    reports = load_reports(directory)
    if not reports:
        print("no APK size reports; nothing to decide")
        return
    checked = [report for report in reports if report["checked"]]
    over = [report for report in checked if report["bytes"] > report["warn_bytes"]]
    api.ensure_label(SIZE_LABEL)
    issue = find_issue(api.open_issues(SIZE_LABEL[0]), SIZE_MARKER)
    issue_number, issue_body = (issue["number"], issue["body"]) if issue else (None, "")
    found = SIZE_DATA.search(issue_body)
    previous = json.loads(found.group(1)) if found else {}
    run_marker = f"<!-- apk-size-tracker run={run_url} -->"
    # 两条线由 build-apk.sh 写进每份报告，同一次运行的各版本取值相同。
    threshold_line = (f"预警线 {checked[0]['warn_bytes']:,} 字节（`MSIME_ANDROID_WARN_APK_BYTES`），上限 {checked[0]['max_bytes']:,} 字节（`MSIME_ANDROID_MAX_APK_BYTES`），"
                      f"只对 GitHub 瘦包（{'、'.join(sorted(report['edition'] for report in checked))}）生效。"
                      if checked else "这次没有量到瘦包。")
    table = size_rows(reports, previous)
    data = f"<!-- apk-size-data {json.dumps({report['edition']: report['bytes'] for report in reports}, sort_keys=True)} -->"

    if not over:
        missing = sorted(set(editions) - {report["edition"] for report in reports})
        if missing:
            print(f"every measured slim APK is under the warning line, but {', '.join(missing)} reported no size; not closing anything")
            return
        if issue_number is None:
            print("every slim APK is under the warning line")
            return
        api.close(issue_number, "\n".join([run_marker, f"[这次 nightly]({run_url})（提交 `{sha[:10]}`）所有瘦包都回到预警线以下，自动关闭。", "", threshold_line, "", *table]))
        print(f"closed #{issue_number}")
        return

    hint = ("趋势提示：「较上次」是和这张单上一次记录的差值，「距上限」是离硬上限还剩多少。越过上限后发版直接失败。"
            "常见来源是词库锁更新、新随包数据或原生库增长；用 `unzip -lv` 比较两次构建的 APK 条目，能省的数据改成资源包按需下载，"
            "确需提高上限是产品决定，先找维护者确认。")
    body = "\n".join([
        SIZE_MARKER,
        run_marker,
        data,
        f"Android 瘦包越过了体积预警线。最近一次：[nightly]({run_url})，提交 `{sha[:10]}`。这张单在所有瘦包回到预警线以下时由 nightly 自动关闭。",
        "",
        threshold_line,
        "",
        *table,
        "",
        hint,
    ])
    if issue_number is None:
        stamp = (now or datetime.now(timezone.utc)).strftime("%Y-%m-%d")
        number = api.create_issue(f"Android APK 体积超过预警线：{stamp}", body, SIZE_LABEL[0])
        print(f"opened #{number}")
        return
    if already_reported(api, issue_number, issue_body, run_marker):
        print(f"this run is already reported on #{issue_number}")
        return
    api.comment(issue_number, "\n".join([run_marker, f"[这次 nightly]({run_url})（提交 `{sha[:10]}`）：", "", threshold_line, "", *table]))
    # 正文始终是最新一次的数字，下一次的「较上次」以它为准。
    api.edit_body(issue_number, body)
    print(f"updated #{issue_number}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    red = commands.add_parser("red-build", help="open, update or close the tracking issue of one completed workflow run")
    red.add_argument("--run-id", required=True)
    size = commands.add_parser("apk-size", help="open, update or close the APK size tracking issue from the size reports")
    size.add_argument("--reports", required=True, help="directory holding the build-apk.sh size reports (*.json)")
    size.add_argument("--editions", required=True, help="comma-separated editions the run was expected to measure")
    size.add_argument("--run-url", required=True)
    size.add_argument("--sha", required=True)
    args = parser.parse_args(argv)
    repository = os.environ.get("GITHUB_REPOSITORY")
    if not repository:
        parser.error("GITHUB_REPOSITORY must name the repository (owner/name)")
    api = GitHub(repository)
    if args.command == "red-build":
        track_red_build(api, args.run_id, os.environ.get("GITHUB_SERVER_URL", "https://github.com"))
    else:
        track_apk_size(api, args.reports, [edition for edition in args.editions.split(",") if edition], args.run_url, args.sha)
    return 0


if __name__ == "__main__":
    sys.exit(main())

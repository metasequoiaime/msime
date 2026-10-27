#!/usr/bin/env python3
"""scripts/ci_issue_tracker.py 的判定逻辑：什么时候开单、追加、关单、什么都不做。

GitHub 换成内存里的假实现，不调用 gh、不联网，所以作为契约检查在任何机器上都能跑。真实的 gh 调用只能在合并后由 ci-red-tracker.yml 和 nightly 的 android-size 跑到。
"""

from contextlib import redirect_stdout
from datetime import datetime, timezone
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("ci_issue_tracker", Path(__file__).with_name("ci_issue_tracker.py"))
TRACKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TRACKER)


class FakeGitHub:
    repository = "synthetic/ime"

    def __init__(self):
        self.runs = {}
        self.jobs_by_run = {}
        self.issues = {}
        self.labels = []

    def add_run(self, run_id, number, conclusion, workflow="Nightly builds", event="schedule", branch="develop", attempt=1):
        path = {"Nightly builds": ".github/workflows/nightly.yml", "Core CI": ".github/workflows/ci.yml"}[workflow]
        self.runs[run_id] = {
            "id": run_id, "run_number": number, "run_attempt": attempt, "conclusion": conclusion, "name": workflow,
            "path": path, "workflow_id": path, "event": event, "head_branch": branch, "head_sha": f"{number:040x}",
            "head_commit": {"message": f"fix(core): 第 {number} 次 @someone #12 `x`\n\n正文"},
            "html_url": f"https://github.com/synthetic/ime/actions/runs/{run_id}",
            "created_at": f"2026-10-{number:02d}T18:00:00Z",
        }
        self.jobs_by_run[run_id] = [
            {"name": "Android nightly / release (pinyin)", "conclusion": "failure", "html_url": f"https://example.invalid/job/{run_id}/1"},
            {"name": "Android nightly / release (full)", "conclusion": "cancelled", "html_url": f"https://example.invalid/job/{run_id}/2"},
            {"name": "Linux nightly / rpm (x86_64)", "conclusion": "timed_out", "html_url": f"https://example.invalid/job/{run_id}/3"},
            {"name": "Windows nightly / build", "conclusion": "success", "html_url": f"https://example.invalid/job/{run_id}/4"},
        ]
        return self.runs[run_id]

    def run(self, run_id):
        return self.runs[run_id]

    def workflow_runs(self, workflow_id):
        return sorted((run for run in self.runs.values() if run["workflow_id"] == workflow_id),
                      key=lambda run: run["run_number"], reverse=True)

    def jobs(self, run_id, attempt):
        return self.jobs_by_run[run_id]

    def ensure_label(self, label):
        self.labels.append(label[0])

    def open_issues(self, label):
        return [{"number": number, "title": issue["title"], "body": issue["body"]}
                for number, issue in self.issues.items() if issue["state"] == "open" and label in issue["labels"]]

    def comments(self, number):
        return list(self.issues[number]["comments"])

    def create_issue(self, title, body, label):
        assert label in self.labels, f"label {label} was not ensured before use"
        number = len(self.issues) + 1
        self.issues[number] = {"title": title, "body": body, "labels": [label], "state": "open", "comments": [], "assignee": TRACKER.ASSIGNEE}
        return number

    def comment(self, number, body):
        self.issues[number]["comments"].append(body)

    def edit_body(self, number, body):
        self.issues[number]["body"] = body

    def close(self, number, body):
        self.comment(number, body)
        self.issues[number]["state"] = "closed"


def quiet(function, *args, **kwargs):
    with redirect_stdout(io.StringIO()):
        return function(*args, **kwargs)


class RedBuild(unittest.TestCase):
    def setUp(self):
        self.api = FakeGitHub()

    def track(self, run_id):
        quiet(TRACKER.track_red_build, self.api, run_id)

    def test_first_failure_opens_one_issue_with_jobs_and_red_start(self):
        self.api.add_run(10, 20, "success")
        self.api.add_run(11, 21, "failure")
        self.api.add_run(12, 22, "cancelled")
        self.api.add_run(13, 23, "failure")
        self.track(13)
        self.assertEqual(len(self.api.issues), 1)
        issue = self.api.issues[1]
        self.assertEqual(issue["title"], "Nightly builds 失败：2026-10-23")
        self.assertEqual(issue["assignee"], "houko")
        self.assertEqual(issue["labels"], ["ci-red"])
        body = issue["body"]
        self.assertIn("<!-- ci-red-tracker workflow=.github/workflows/nightly.yml -->", body)
        self.assertIn("[Android nightly / release (pinyin)](https://example.invalid/job/13/1)（失败）", body)
        self.assertIn("[Linux nightly / rpm (x86_64)](https://example.invalid/job/13/3)（超时）", body)
        self.assertNotIn("release (full)", body, "cancelled sibling jobs are noise, not failures")
        self.assertNotIn("Windows nightly", body)
        # 起点是上次成功（#20）之后的第一次失败 #21，取消的 #22 不打断也不计入。
        self.assertIn("第一次失败是 [#21]", body)
        self.assertIn("连续失败 2 次", body)
        self.assertIn("上一次成功是 [#20]", body)
        # 提交标题里的反引号被换掉，整个标题在代码样式里，@ 和 # 不会通知人或链出去。
        self.assertIn("`fix(core): 第 23 次 @someone #12 'x'`", body)
        self.assertNotIn("正文", body)

    def test_later_failure_comments_once_and_success_closes(self):
        self.api.add_run(10, 20, "success")
        self.api.add_run(11, 21, "failure")
        self.track(11)
        self.assertIn("之后的第一次失败", self.api.issues[1]["body"])
        self.track(11)
        self.assertEqual(self.api.issues[1]["comments"], [], "the run that opened the issue is not reported twice")
        self.api.add_run(12, 22, "timed_out")
        self.track(12)
        self.track(12)
        self.assertEqual(len(self.api.issues[1]["comments"]), 1, "a redelivered event must not comment twice")
        self.assertIn("又失败一次：[#22]", self.api.issues[1]["comments"][0])
        self.api.add_run(13, 23, "success")
        self.track(13)
        self.assertEqual(self.api.issues[1]["state"], "closed")
        self.assertIn("已转绿：[#23]", self.api.issues[1]["comments"][-1])
        self.api.add_run(14, 24, "failure")
        self.track(14)
        self.assertEqual(len(self.api.issues), 2, "a new red after a close opens a fresh issue")

    def test_rerun_attempt_is_reported_separately(self):
        self.api.add_run(10, 20, "success")
        self.api.add_run(11, 21, "failure")
        self.track(11)
        self.api.runs[11]["run_attempt"] = 2
        self.track(11)
        self.assertEqual(len(self.api.issues[1]["comments"]), 1)
        self.assertIn("（第 2 次尝试）", self.api.issues[1]["comments"][0])

    def test_out_of_order_events_do_not_flip_state(self):
        self.api.add_run(11, 21, "failure")
        self.api.add_run(12, 22, "success")
        self.track(12)
        self.track(11)
        self.assertEqual(self.api.issues, {}, "an older failure must not open an issue once a newer run is green")
        self.api.add_run(13, 23, "failure")
        self.track(13)
        self.api.add_run(14, 24, "failure")
        self.api.runs[12]["conclusion"] = "success"
        self.track(12)
        self.assertEqual(self.api.issues[1]["state"], "open", "an older success must not close an issue a newer failure opened")

    def test_success_without_issue_and_untracked_runs_do_nothing(self):
        self.api.add_run(11, 21, "cancelled")
        self.track(11)
        self.api.add_run(12, 22, "skipped")
        self.track(12)
        self.api.add_run(13, 23, "failure", event="pull_request")
        self.track(13)
        self.api.add_run(14, 24, "failure", event="push", branch="feature/x")
        self.track(14)
        self.assertEqual(self.api.labels, [], "untracked runs do not touch the repository at all")
        self.api.add_run(10, 25, "success")
        self.track(10)
        self.assertEqual(self.api.issues, {})

    def test_streak_without_green_in_window_says_so(self):
        self.api.add_run(11, 21, "failure")
        self.api.add_run(12, 22, "failure")
        self.track(12)
        self.assertIn("没有成功的", self.api.issues[1]["body"])
        self.assertIn("至少从 [#21]", self.api.issues[1]["body"])

    def test_each_workflow_has_its_own_issue(self):
        self.api.add_run(11, 21, "failure")
        self.track(11)
        self.api.add_run(31, 5, "failure", workflow="Core CI", event="push")
        self.track(31)
        self.assertEqual([issue["title"] for issue in self.api.issues.values()], ["Nightly builds 失败：2026-10-21", "Core CI 失败：2026-10-05"])
        self.api.add_run(32, 6, "success", workflow="Core CI", event="push")
        self.track(32)
        self.assertEqual([issue["state"] for issue in self.api.issues.values()], ["open", "closed"])

    def test_run_without_failed_jobs_still_points_at_the_run(self):
        self.api.add_run(11, 21, "failure")
        self.api.jobs_by_run[11] = []
        self.track(11)
        self.assertIn("没有找到失败的 job", self.api.issues[1]["body"])


class ApkSize(unittest.TestCase):
    EDITIONS = ["full", "wubi", "pinyin", "japanese", "vietnamese", "tibetan"]

    def setUp(self):
        self.api = FakeGitHub()
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)

    def write(self, sizes):
        root = Path(self.directory.name)
        for path in root.rglob("*.json"):
            path.unlink()
        for edition, size in sizes.items():
            checked = edition in ("full", "pinyin")
            report = {"edition": edition, "apk": f"msime-android-{edition}.apk", "bytes": size, "checked": checked,
                      "warn_bytes": 132000000 if checked else None, "max_bytes": 135000000 if checked else None}
            # download-artifact 不合并时每个 artifact 一个子目录，两种布局都要读得到。
            (root / edition).mkdir(exist_ok=True)
            (root / edition / f"{edition}.json").write_text(json.dumps(report), encoding="utf-8")

    def track(self, run):
        quiet(TRACKER.track_apk_size, self.api, self.directory.name, self.EDITIONS,
              f"https://github.com/synthetic/ime/actions/runs/{run}", f"{run:040x}", now=datetime(2026, 10, 28, tzinfo=timezone.utc))

    def sizes(self, full, pinyin):
        return {"full": full, "pinyin": pinyin, "wubi": 170000000, "japanese": 150000000, "vietnamese": 90000000, "tibetan": 90000000}

    def test_over_warning_opens_updates_and_closes(self):
        self.write(self.sizes(128435407, 128435407))
        self.track(1)
        self.assertEqual(self.api.issues, {}, "under the line nothing is opened")
        self.write(self.sizes(128435407, 133000000))
        self.track(2)
        issue = self.api.issues[1]
        self.assertEqual(issue["title"], "Android APK 体积超过预警线：2026-10-28")
        self.assertEqual(issue["labels"], ["apk-size"])
        self.assertIn("| pinyin | `msime-android-pinyin.apk` | 133,000,000 | — | 2.0 MB | **超过预警线** |", issue["body"])
        self.assertIn("| full | `msime-android-full.apk` | 128,435,407 | — | 6.6 MB | 正常 |", issue["body"])
        self.assertIn("| wubi | `msime-android-wubi.apk` | 170,000,000 | — | — | 不设上限（非瘦包） |", issue["body"])
        self.assertIn("预警线 132,000,000 字节", issue["body"])
        self.assertIn("上限 135,000,000 字节", issue["body"])
        self.assertIn("full、pinyin", issue["body"])
        self.write(self.sizes(128435407, 135623964))
        self.track(3)
        self.track(3)
        self.assertEqual(len(issue["comments"]), 1, "the same nightly is reported once")
        self.assertIn("| pinyin | `msime-android-pinyin.apk` | 135,623,964 | +2,623,964 | -0.6 MB | **超过上限** |", issue["comments"][0])
        self.assertIn("135,623,964 | +2,623,964", issue["body"], "the body carries the latest numbers")
        self.write({"pinyin": 128000000})
        self.track(4)
        self.assertEqual(issue["state"], "open", "a nightly that measured only some editions cannot prove they are all back under")
        self.write(self.sizes(128435407, 128000000))
        self.track(5)
        self.assertEqual(issue["state"], "closed")
        self.assertIn("回到预警线以下", issue["comments"][-1])

    def test_no_reports_does_nothing(self):
        self.track(1)
        self.assertEqual(self.api.issues, {})
        self.assertEqual(self.api.labels, [])


if __name__ == "__main__":
    unittest.main()

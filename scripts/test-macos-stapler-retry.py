#!/usr/bin/env python3
"""验证公证票据装订的有限重试，不访问 Apple 服务或使用签名凭据。"""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "platforms/macos/scripts/staple-ticket.sh"
DISCONNECT = "Error Domain=NSURLErrorDomain Code=-1005 synthetic disconnect"
# stapler 原文的拼写就是 expections。
CLOUDKIT = "CloudKit's response is inconsistent with expections: (null)"
DELAYS = [["15"], ["30"], ["60"], ["90"], ["120"]]


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "需要 POSIX bash")
class StaplerRetryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="msime-stapler-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        self.state = self.directory / "state.json"
        self.plan = self.directory / "plan.json"
        self.dmg = self.directory / "合成 安装包.pkg"
        self.dmg.write_bytes(b"synthetic package")
        runner = self.directory / "tools.py"
        runner.write_text(
            """import json, os, sys
from pathlib import Path
state_file = Path(os.environ["MSIME_TEST_STAPLER_STATE"])
state = json.loads(state_file.read_text()) if state_file.exists() else {"calls": [], "delays": []}
if sys.argv[1] == "sleep":
    state["delays"].append(sys.argv[2:])
    state_file.write_text(json.dumps(state))
    sys.exit(0)
plan = json.loads(Path(os.environ["MSIME_TEST_STAPLER_PLAN"]).read_text())
index = len(state["calls"])
state["calls"].append(sys.argv[2:])
state_file.write_text(json.dumps(state))
if index >= len(plan):
    sys.exit(90)
reply = plan[index]
print(reply["message"], file=sys.stderr)
sys.exit(reply["status"])
""",
            encoding="utf-8",
        )
        for name in ("xcrun", "sleep"):
            tool = self.bin / name
            tool.write_text(
                "#!/usr/bin/env bash\nexec "
                + shlex.quote(sys.executable)
                + " "
                + shlex.quote(str(runner))
                + " "
                + name
                + ' "$@"\n',
                encoding="utf-8",
            )
            tool.chmod(0o755)

    def run_helper(self, replies, action="staple"):
        self.plan.write_text(json.dumps(replies), encoding="utf-8")
        environment = os.environ.copy()
        environment.update(
            PATH=str(self.bin) + os.pathsep + environment.get("PATH", ""),
            MSIME_TEST_STAPLER_STATE=str(self.state),
            MSIME_TEST_STAPLER_PLAN=str(self.plan),
        )
        result = subprocess.run(
            ["bash", str(HELPER)] + (["--validate"] if action == "validate" else []) + [str(self.dmg)],
            env=environment,
            capture_output=True,
            text=True,
            timeout=20,
        )
        state = json.loads(self.state.read_text())
        self.assertTrue(
            all(call == ["stapler", action, str(self.dmg)] for call in state["calls"])
        )
        return result, state

    def test_success_does_not_retry(self):
        result, state = self.run_helper([{"status": 0, "message": "synthetic success"}])
        self.assertEqual(result.returncode, 0)
        self.assertIn("synthetic success", result.stdout)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])

    def test_disconnection_retries_the_same_package(self):
        result, state = self.run_helper(
            [{"status": 68, "message": DISCONNECT}] * 2
            + [{"status": 0, "message": "synthetic success"}]
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(state["calls"]), 3)
        self.assertEqual(state["delays"], [["15"], ["30"]])

    def test_cloudkit_inconsistency_alone_retries(self):
        result, state = self.run_helper(
            [{"status": 68, "message": CLOUDKIT}] * 3
            + [{"status": 0, "message": "synthetic success"}]
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(state["calls"]), 4)
        self.assertEqual(state["delays"], [["15"], ["30"], ["60"]])

    def test_exhausted_disconnections_keep_the_failure(self):
        result, state = self.run_helper([{"status": 68, "message": DISCONNECT}] * 6)
        self.assertEqual(result.returncode, 68)
        self.assertEqual(len(state["calls"]), 6)
        self.assertEqual(state["delays"], DELAYS)

    def test_cloudkit_inconsistency_with_other_status_does_not_retry(self):
        result, state = self.run_helper([{"status": 65, "message": CLOUDKIT}])
        self.assertEqual(result.returncode, 65)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])

    def test_other_ticket_failures_do_not_retry(self):
        result, state = self.run_helper([{"status": 68, "message": "synthetic missing ticket"}])
        self.assertEqual(result.returncode, 68)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])

    def test_other_exit_status_is_preserved(self):
        result, state = self.run_helper([{"status": 65, "message": DISCONNECT}])
        self.assertEqual(result.returncode, 65)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])

    def test_validate_success_does_not_retry(self):
        result, state = self.run_helper([{"status": 0, "message": "synthetic valid"}], action="validate")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])

    def test_validate_disconnection_retries_with_the_same_window(self):
        result, state = self.run_helper(
            [{"status": 68, "message": DISCONNECT + "\n" + CLOUDKIT}] + [{"status": 0, "message": "synthetic valid"}],
            action="validate",
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(state["calls"]), 2)
        self.assertEqual(state["delays"], [["15"]])

    def test_validate_exhausted_disconnections_report_a_temporary_failure(self):
        result, state = self.run_helper([{"status": 68, "message": CLOUDKIT}] * 6, action="validate")
        self.assertEqual(result.returncode, 75)
        self.assertEqual(len(state["calls"]), 6)
        self.assertEqual(state["delays"], DELAYS)

    def test_validate_missing_ticket_keeps_its_status(self):
        result, state = self.run_helper([{"status": 68, "message": "synthetic does not have a ticket stapled"}], action="validate")
        self.assertEqual(result.returncode, 68)
        self.assertEqual(len(state["calls"]), 1)
        self.assertEqual(state["delays"], [])


if __name__ == "__main__":
    unittest.main()

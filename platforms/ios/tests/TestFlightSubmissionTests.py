#!/usr/bin/env python3
"""提交 TestFlight 外部审核那段逻辑的测试。

这段代码只在发布当天跑,而且跑在拿得到 App Store Connect 凭据的作业里 —— 想靠"下次发版看看对不对"
来验证它,代价是一次失败的发布。所以真正会出错的几处都在这里用桩走一遍:找不到 build、build 处理
失败、处理超时、找不到测试组、以及重复提交。
"""
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"


def load_module():
    """脚本依赖 pyjwt,而它只在发布作业里装。缺了就跳过,而不是把整个套件拖红。"""
    if importlib.util.find_spec("jwt") is None:
        raise unittest.SkipTest("pyjwt is not installed; the release job installs it")
    spec = importlib.util.spec_from_file_location("submit_testflight_review",
                                                  SCRIPTS / "submit_testflight_review.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TestFlightSubmissionTests(unittest.TestCase):
    def setUp(self):
        self.module = load_module()

    def test_a_build_that_has_not_appeared_yet_is_waited_for(self):
        """altool 一把字节交给 Apple 就返回,build 要几分钟后才出现在 API 里。

        不等的后果今天见过:0.48.6 的 1002.68.1 上传成功,下一步立刻去查、查不到,整个发布报红,而包已经
        在 Apple 手里 —— 看起来像发布失败,实际只差分发,测试者干等。这条用例钉住「先等再放弃」。
        """
        attempts = []

        def answer(method, path, auth, body=None):
            if "sort=-uploadedDate" in path:
                return {"data": []}
            attempts.append(path)
            if len(attempts) < 3:
                return {"data": []}
            return {"data": [{"id": "late", "attributes": {"version": "1002.68.1"}}]}

        with mock.patch.object(self.module, "request", side_effect=answer), \
                mock.patch.object(self.module.time, "sleep"):
            found = self.module.find_build("t", "1", "1002.68.1", 600)
        self.assertEqual(found["id"], "late")
        self.assertEqual(len(attempts), 3, "没有重试,第一次查不到就放弃了")

    def test_waiting_still_gives_up_and_says_what_it_saw(self):
        """等不是无限等:超时之后仍要报出它看到的版本,否则发布当天只知道「没找到」。"""
        def answer(method, path, auth, body=None):
            if "sort=-uploadedDate" in path:
                return {"data": [{"attributes": {"version": "1002.71.1"}}]}
            return {"data": []}

        with mock.patch.object(self.module, "request", side_effect=answer), \
                mock.patch.object(self.module.time, "sleep"):
            with self.assertRaises(self.module.Failure) as raised:
                self.module.find_build("t", "1", "1002.68.1", 0)
        self.assertIn("1002.68.1", str(raised.exception))
        self.assertIn("1002.71.1", str(raised.exception))

    def test_the_exact_build_is_chosen_and_a_miss_names_what_is_there(self):
        builds = {"data": [{"id": "b1", "attributes": {"version": "1003.1.1"}}]}
        with mock.patch.object(self.module, "request", return_value=builds):
            self.assertEqual(self.module.find_build("t", "1", "1003.1.1", 0)["id"], "b1")

        # 过滤是服务端做的,但返回里混进别的版本时不能将就着用 —— 提交错一个 build 比失败更难发现。
        def answer(method, path, auth, body=None):
            if "sort=-uploadedDate" in path:
                return {"data": [{"attributes": {"version": "1002.71.1"}},
                                 {"attributes": {"version": "1002.69.1"}}]}
            return {"data": [{"id": "other", "attributes": {"version": "1002.71.1"}}]}

        with mock.patch.object(self.module, "request", side_effect=answer):
            with self.assertRaises(self.module.Failure) as raised:
                self.module.find_build("t", "1", "1003.1.1", 0)
        # 报错要说出它看到了什么,否则发布当天只知道"没找到"。
        self.assertIn("1003.1.1", str(raised.exception))
        self.assertIn("1002.71.1", str(raised.exception))

    def test_processing_is_awaited_until_valid(self):
        states = iter(["PROCESSING", "PROCESSING", "VALID"])
        answer = lambda *a, **k: {"data": {"attributes": {"processingState": next(states)}}}
        with mock.patch.object(self.module, "request", side_effect=answer), \
             mock.patch.object(self.module.time, "sleep"):
            self.module.await_processing("t", "b1", timeout=600)

    def test_a_build_that_fails_processing_stops_the_step(self):
        for state in ("INVALID", "FAILED"):
            answer = lambda *a, **k: {"data": {"attributes": {"processingState": state}}}
            with self.subTest(state=state), \
                 mock.patch.object(self.module, "request", side_effect=answer), \
                 mock.patch.object(self.module.time, "sleep"):
                with self.assertRaises(self.module.Failure) as raised:
                    self.module.await_processing("t", "b1", timeout=600)
                self.assertIn(state, str(raised.exception))

    def test_waiting_forever_is_a_failure_not_a_hang(self):
        answer = lambda *a, **k: {"data": {"attributes": {"processingState": "PROCESSING"}}}
        clock = iter([0, 1, 2_000, 2_000])
        with mock.patch.object(self.module, "request", side_effect=answer), \
             mock.patch.object(self.module.time, "sleep"), \
             mock.patch.object(self.module.time, "time", side_effect=lambda: next(clock)):
            with self.assertRaises(self.module.Failure):
                self.module.await_processing("t", "b1", timeout=1)

    def test_the_group_is_found_by_name_with_its_kind(self):
        groups = {"data": [{"id": "g1", "attributes": {"name": "外部测试", "isInternalGroup": False}},
                           {"id": "g2", "attributes": {"name": "internal", "isInternalGroup": True}}]}
        with mock.patch.object(self.module, "request", return_value=groups):
            self.assertEqual(self.module.find_group("t", "1", "外部测试"), ("g1", False))
            self.assertEqual(self.module.find_group("t", "1", "internal"), ("g2", True))
            with self.assertRaises(self.module.Failure) as raised:
                self.module.find_group("t", "1", "没有这个组")
        self.assertIn("外部测试", str(raised.exception))

    def test_an_internal_group_is_never_asked_to_take_a_build(self):
        """内部组自动拥有每一个 build,Apple 对显式添加回 422。

        做了必错:每一次合并到 main 的发布都会在最后一步染红,而内测其实已经拿到了 build。
        这条用例钉住「内部组只等处理完成,不发那个 POST」。
        """
        posts = []

        def answer(method, path, auth, body=None):
            if method == "POST":
                posts.append(path)
                return {}
            if "/betaGroups" in path:
                return {"data": [{"id": "g2", "attributes": {"name": "internal", "isInternalGroup": True}}]}
            if "/builds/" in path:
                return {"data": {"attributes": {"processingState": "VALID"}}}
            return {"data": [{"id": "b1", "attributes": {"version": "9"}}]}

        with mock.patch.object(self.module, "request", side_effect=answer), \
                mock.patch.object(self.module, "token", return_value="t"), \
                mock.patch.object(self.module.time, "sleep"), \
                mock.patch.object(self.module.sys, "argv", [
                    "x", "--app", "1", "--build-version", "9", "--group", "internal",
                    "--key-id", "k", "--issuer-id", "i", "--key-path", "/dev/null"]):
            self.assertEqual(self.module.main(), 0)
        self.assertEqual(posts, [], f"内部组不该收到任何 POST,却发了 {posts}")

    def test_an_internal_handover_does_not_ask_apple_for_anything(self):
        """合进 main 的构建进内部组。内部测试不需要审核,提交它只会白占一个名额。"""
        calls = []

        def answer(method, path, auth, body=None):
            calls.append((method, path))
            if path.startswith("/builds?filter[app]"):
                return {"data": [{"id": "b1", "attributes": {"version": "1003.1.1"}}]}
            if path.startswith("/builds/"):
                return {"data": {"attributes": {"processingState": "VALID"}}}
            if path.startswith("/betaGroups?"):
                return {"data": [{"id": "gi", "attributes": {"name": "internal"}}]}
            return {}

        argv = ["submit", "--app", "1", "--build-version", "1003.1.1", "--group", "internal",
                "--key-id", "K", "--issuer-id", "I", "--key-path", str(SCRIPTS)]
        with mock.patch.object(self.module, "request", side_effect=answer), \
             mock.patch.object(self.module, "token", return_value="t"), \
             mock.patch.object(sys, "argv", argv):
            self.assertEqual(self.module.main(), 0)
        posts = [path for method, path in calls if method == "POST"]
        self.assertEqual(posts, ["/betaGroups/gi/relationships/builds"])
        self.assertNotIn("/betaAppReviewSubmissions", [p for _, p in calls])

    def test_a_rerun_of_a_submitted_build_is_not_a_failure(self):
        """发布重跑一次不该因为"活已经干完了"而变红。"""
        calls = []

        def answer(method, path, auth, body=None):
            calls.append((method, path))
            if path == "/betaAppReviewSubmissions":
                raise self.module.Failure("POST -> 409\nENTITY_ERROR.ATTRIBUTE.INVALID already exists")
            if path.startswith("/builds?filter[app]"):
                return {"data": [{"id": "b1", "attributes": {"version": "1003.1.1"}}]}
            if path.startswith("/builds/"):
                return {"data": {"attributes": {"processingState": "VALID"}}}
            if path.startswith("/betaGroups?"):
                return {"data": [{"id": "g1", "attributes": {"name": "外部测试"}}]}
            return {}

        argv = ["submit", "--app", "1", "--build-version", "1003.1.1", "--group", "外部测试",
                "--submit-review", "--key-id", "K", "--issuer-id", "I", "--key-path", str(SCRIPTS)]
        with mock.patch.object(self.module, "request", side_effect=answer), \
             mock.patch.object(self.module, "token", return_value="t"), \
             mock.patch.object(sys, "argv", argv):
            self.assertEqual(self.module.main(), 0)
        # 分组在提交之前 —— 顺序反了的话,审核提交的是一个还没分发给任何人的 build。
        posts = [path for method, path in calls if method == "POST"]
        self.assertEqual(posts, ["/betaGroups/g1/relationships/builds", "/betaAppReviewSubmissions"])


if __name__ == "__main__":
    unittest.main()

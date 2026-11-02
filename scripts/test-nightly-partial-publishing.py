#!/usr/bin/env python3
"""用合成安装包模拟连续发布与中断，确认失败平台不会拖住其他平台。"""

import copy
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "nightly_publishing", Path(__file__).with_name("publish-nightly.py"))
PUBLISHING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISHING)
OLD_SHA, NEW_SHA = "a" * 40, "b" * 40


class FakeGitHub:
    repository = "synthetic/ime"

    def __init__(self, legacy=True):
        self.current = ({"id": 1, "body": f"nightly source: {OLD_SHA}\n",
                         "published_at": "2026-01-01T00:00:00Z", "draft": False}
                        if legacy else None)
        self.files = {}
        self.events = []
        self.failed_platform = None
        self.corrupt_platform = None
        self.fail_commit = False
        self.fail_checksum = False
        self.fail_cleanup = False
        if legacy:
            for platform, suffixes in PUBLISHING.PACKAGING.PLATFORM_SUFFIXES.items():
                for suffix in suffixes:
                    self.files[f"old-{platform}{suffix}"] = f"old {platform} {suffix}".encode()
            self.files["README.txt"] = b"old instructions"
            self.files["SHA256SUMS"] = b"old manifest"

    def release(self):
        return copy.deepcopy(self.current)

    def assets(self, release):
        return {name: {"id": name, "name": name,
                       "digest": "sha256:" + hashlib.sha256(payload).hexdigest()}
                for name, payload in self.files.items()}

    def create(self, source_sha):
        self.events.append("create")
        self.current = {"id": 1, "body": "", "draft": True, "published_at": None}
        return self.release()

    def upload(self, paths):
        self.events.append("upload")
        for index, path in enumerate(paths):
            if self.fail_checksum and path.name.startswith("SHA256SUMS"):
                raise RuntimeError("合成校验清单上传失败")
            if self.failed_platform and self.failed_platform in path.name and index == len(paths) - 1:
                raise RuntimeError("合成平台上传中断")
            if path.name in self.files:
                raise RuntimeError("禁止覆盖同名旧附件")
            self.files[path.name] = (b"corrupted" if self.corrupt_platform and self.corrupt_platform in path.name
                                     else path.read_bytes())

    def commit(self, release, body, source_sha):
        self.events.append("commit")
        if self.fail_commit:
            raise RuntimeError("合成正文提交失败")
        self.current.update(body=body, draft=False, target_commitish=source_sha)

    def delete(self, asset):
        self.events.append("delete")
        if self.fail_cleanup:
            raise RuntimeError("合成清理失败")
        del self.files[asset["name"]]


class PartialPublishingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="msime-nightly-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        for platform, suffixes in PUBLISHING.PACKAGING.PLATFORM_SUFFIXES.items():
            # 复用的 release-*.yml 把包内版本号写在 artifact 名末尾，publish-nightly.py 从这里取 version。
            directory = self.source / platform / f"msime-{platform}-1.2.3"
            directory.mkdir(parents=True)
            for suffix in suffixes:
                (directory / f"new-{platform}{suffix}").write_bytes(f"new {platform} {suffix}".encode())
            (directory / "source.zip").write_bytes(b"not an installer")
        self.env = patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""})
        self.env.start()
        self.addCleanup(self.env.stop)
        for redirect in (redirect_stdout, redirect_stderr):
            capture = redirect(io.StringIO())
            capture.__enter__()
            self.addCleanup(capture.__exit__, None, None, None)
        self.client = FakeGitHub()

    def publish(self, successful, *, build_id="10.1", sha=NEW_SHA, downloads=None, ios_group="external"):
        results = {platform: "success" if platform in successful else "failure"
                   for platform in PUBLISHING.PLATFORMS}
        downloads = downloads if downloads is not None else {
            platform: {"outcome": "success"} for platform in successful if platform != "ios"}
        return PUBLISHING.publish(self.client, self.source, self.root / build_id,
                                  results, downloads, sha, build_id,
                                  f"https://github.com/synthetic/ime/actions/runs/{build_id.split('.')[0]}", ios_group)

    def state(self):
        return PUBLISHING.previous_state(self.client.release(), self.client.assets(self.client.release()))

    def assert_manifest_matches(self):
        state = self.state()
        lines = self.client.files[state["checksum"]].decode().splitlines()
        expected = {file["name"] for build in state["platforms"].values() for file in build["files"]}
        self.assertEqual({line.split("  ")[1] for line in lines}, expected)
        for line in lines:
            digest, name = line.split("  ")
            self.assertEqual(digest, hashlib.sha256(self.client.files[name]).hexdigest())

    def test_three_failed_platforms_keep_their_bytes(self):
        original = copy.deepcopy(self.client.files)
        state = self.publish({"windows", "linux", "ios"})
        for platform in ("android", "macos", "harmony"):
            self.assertEqual(state["platforms"][platform]["source"], OLD_SHA)
            for file in state["platforms"][platform]["files"]:
                self.assertEqual(self.client.files[file["name"]], original[file["name"]])
        for platform in ("windows", "linux", "ios"):
            self.assertEqual(state["platforms"][platform]["source"], NEW_SHA)
        self.assertNotIn("old-windows.exe", self.client.files)
        self.assertNotIn("old-linux.deb", self.client.files)
        self.assert_manifest_matches()
        self.assertIn("保留上次成功构建", self.client.current["body"])
        self.assertIn("待 Apple 审核", self.client.current["body"])
        self.assertLess(self.client.events.index("commit"), self.client.events.index("delete"))

    def test_updated_platforms_record_their_version_and_run(self):
        state = self.publish({"windows", "linux", "ios"}, build_id="900.2")
        for platform in ("windows", "linux"):
            self.assertEqual(state["platforms"][platform]["version"], "1.2.3")
            self.assertEqual(state["platforms"][platform]["run"], 900)
        # iOS 没有附件，也就没有 artifact 名可取版本号；运行标识照样记。
        self.assertNotIn("version", state["platforms"]["ios"])
        self.assertEqual(state["platforms"]["ios"]["run"], 900)
        # 保留下来的旧条目没有这两个字段，状态照常解析。
        self.assertNotIn("version", state["platforms"]["android"])
        self.assertNotIn("run", state["platforms"]["android"])
        self.assertEqual(self.state(), state)
        self.assertIn('"version": "1.2.3"', self.client.current["body"])
        # 下一轮只更新 Linux：Windows 的版本号和运行标识原样留着。
        state = self.publish({"linux"}, build_id="901.1")
        self.assertEqual(state["platforms"]["windows"]["run"], 900)
        self.assertEqual(state["platforms"]["linux"]["run"], 901)

    def test_arch_package_keeps_its_extension_and_replaces_the_previous_build(self):
        state = self.publish({"linux"})
        name = "new-linux-nightly.10.1.pkg.tar.zst"
        self.assertIn(name, self.client.files)
        self.assertIn(name, {file["name"] for file in state["platforms"]["linux"]["files"]})
        self.assertEqual(self.client.files[name], b"new linux .pkg.tar.zst")
        self.assert_manifest_matches()
        self.publish({"linux"}, build_id="11.1")
        self.assertNotIn(name, self.client.files)
        self.assertIn("new-linux-nightly.11.1.pkg.tar.zst", self.client.files)
        self.assert_manifest_matches()

    def test_artifacts_without_one_version_record_no_version(self):
        other = self.source / "linux" / "msime-linux-rpm-x86_64-1.2.4"
        other.mkdir()
        next((self.source / "linux").rglob("*.rpm")).rename(other / "new-linux.rpm")
        unversioned = self.source / "windows" / "artifact"
        (self.source / "windows" / "msime-windows-1.2.3").rename(unversioned)
        state = self.publish({"linux", "windows"})
        self.assertNotIn("version", state["platforms"]["linux"])
        self.assertNotIn("version", state["platforms"]["windows"])
        self.assertEqual(state["platforms"]["linux"]["run"], 10)

    def test_invalid_recorded_version_or_run_prevents_mutations(self):
        import json
        self.publish({"windows"})
        valid = self.state()
        body = self.client.current["body"].split(PUBLISHING.MARKER, 1)[0]
        for field, value in (("version", "1.2"), ("version", 3), ("run", 0), ("run", "10"), ("run", True)):
            state = copy.deepcopy(valid)
            state["platforms"]["windows"][field] = value
            self.client.current["body"] = body + PUBLISHING.MARKER + json.dumps(state) + "\n-->\n"
            self.client.events.clear()
            with self.assertRaisesRegex(ValueError, "无效"):
                self.publish({"linux"}, build_id="11.1")
            self.assertEqual(self.client.events, [], (field, value))

    def test_first_run_partial_success_is_published(self):
        self.client = FakeGitHub(legacy=False)
        state = self.publish({"windows"})
        self.assertEqual(set(state["platforms"]), {"windows"})
        self.assertFalse(self.client.current["draft"])
        self.assertIn("暂无成功构建", self.client.current["body"])
        self.assert_manifest_matches()

    def test_all_failed_keep_every_native_file(self):
        original = copy.deepcopy(self.client.files)
        state = self.publish(set())
        for platform in PUBLISHING.PACKAGING.PLATFORM_SUFFIXES:
            self.assertEqual(state["platforms"][platform]["source"], OLD_SHA)
            for file in state["platforms"][platform]["files"]:
                self.assertEqual(self.client.files[file["name"]], original[file["name"]])
        self.assert_manifest_matches()

    def test_first_run_all_failed_does_not_create_empty_release(self):
        self.client = FakeGitHub(legacy=False)
        self.assertIsNone(self.publish(set()))
        self.assertIsNone(self.client.current)
        self.assertEqual(self.client.events, [])

    def test_second_run_preserves_each_platform_source(self):
        first = self.publish({"windows", "ios"})
        windows_files = copy.deepcopy(first["platforms"]["windows"]["files"])
        state = self.publish({"linux"}, build_id="11.1", sha="c" * 40)
        self.assertEqual(state["platforms"]["windows"]["files"], windows_files)
        self.assertEqual(state["platforms"]["windows"]["source"], NEW_SHA)
        self.assertEqual(state["platforms"]["linux"]["source"], "c" * 40)
        self.assertEqual(state["platforms"]["android"]["source"], OLD_SHA)
        self.assert_manifest_matches()

    def test_partial_upload_failure_keeps_old_platform_and_publishes_others(self):
        self.client.failed_platform = "linux"
        original = copy.deepcopy(self.client.files)
        with self.assertRaisesRegex(RuntimeError, "其他平台已发布"):
            self.publish({"linux", "windows"})
        state = self.state()
        self.assertEqual(state["platforms"]["windows"]["source"], NEW_SHA)
        self.assertEqual(state["platforms"]["linux"]["source"], OLD_SHA)
        for suffix in (".deb", ".rpm", ".pkg.tar.zst"):
            self.assertEqual(self.client.files["old-linux" + suffix], original["old-linux" + suffix])
        self.assertNotIn("new-linux-nightly.10.1.deb", self.client.files)
        self.assert_manifest_matches()

    def test_macos_pkg_name_and_retired_dmg_cleanup(self):
        # macOS 从 DMG 改成 pkg：新包按官网认的名字发（…-universal-nightly.<运行>.<尝试>.pkg），nightly 上没人再指向的旧 DMG 被清掉。
        artifact = self.source / "macos" / "msime-macos-1.2.3"
        for path in artifact.iterdir():
            path.unlink()
        (artifact / "msime-macos-1.2.3-universal.pkg").write_bytes(b"new macos pkg")
        (artifact / "msime-macos-wubi-1.2.3-universal.pkg").write_bytes(b"new macos wubi pkg")
        (artifact / "msime-macos-1.2.3-input-method.zip").write_bytes(b"sparkle archive, not an installer")
        self.client.files["msime-macos-1.2.2-universal-nightly.9.1.dmg"] = b"old macos dmg"
        state = self.publish(set(PUBLISHING.PLATFORMS))
        self.assertEqual([file["name"] for file in state["platforms"]["macos"]["files"]],
                         ["msime-macos-1.2.3-universal-nightly.10.1.pkg", "msime-macos-wubi-1.2.3-universal-nightly.10.1.pkg"])
        self.assertNotIn("msime-macos-1.2.2-universal-nightly.9.1.dmg", self.client.files)
        self.assert_manifest_matches()

    def test_dmg_still_referenced_by_macos_state_is_kept(self):
        # 上一轮发的还是 DMG、这一轮 macOS 失败时，状态仍指向那个 DMG，它不能被当成退役的后缀删掉。
        artifact = self.source / "macos" / "msime-macos-1.2.3"
        (artifact / "msime-macos-1.2.3-universal.dmg").write_bytes(b"previous macos dmg")
        with patch.dict(PUBLISHING.PACKAGING.PLATFORM_SUFFIXES, {"macos": (".dmg",)}):
            first = self.publish({"macos"})
        dmg = first["platforms"]["macos"]["files"][0]["name"]
        self.assertTrue(dmg.endswith(".dmg"))
        state = self.publish({"windows"}, build_id="11.1", sha="c" * 40)
        self.assertEqual(state["platforms"]["macos"]["files"][0]["name"], dmg)
        self.assertIn(dmg, self.client.files)
        self.assert_manifest_matches()

    def test_digest_mismatch_keeps_old_platform(self):
        self.client.corrupt_platform = "android"
        with self.assertRaisesRegex(RuntimeError, "android"):
            self.publish({"android", "windows"})
        self.assertEqual(self.state()["platforms"]["android"]["source"], OLD_SHA)
        self.assertEqual(self.state()["platforms"]["windows"]["source"], NEW_SHA)
        self.assert_manifest_matches()

    def test_failed_download_does_not_use_partial_files(self):
        with self.assertRaisesRegex(RuntimeError, "android"):
            self.publish({"android", "windows"}, downloads={
                "android": {"outcome": "failure"}, "windows": {"outcome": "success"}})
        self.assertEqual(self.state()["platforms"]["android"]["source"], OLD_SHA)
        self.assertEqual(self.state()["platforms"]["windows"]["source"], NEW_SHA)

    def test_missing_successful_artifact_only_blocks_its_platform(self):
        next((self.source / "android").rglob("*.apk")).unlink()
        with self.assertRaisesRegex(RuntimeError, "android"):
            self.publish({"android", "windows"})
        self.assertEqual(self.state()["platforms"]["windows"]["source"], NEW_SHA)
        self.assertEqual(self.state()["platforms"]["android"]["source"], OLD_SHA)

    def test_commit_failure_does_not_delete_previous_downloads(self):
        original = copy.deepcopy(self.client.files)
        body = self.client.current["body"]
        self.client.fail_commit = True
        with self.assertRaisesRegex(RuntimeError, "正文提交失败"):
            self.publish({"windows"})
        self.assertEqual(self.client.current["body"], body)
        self.assertNotIn("delete", self.client.events)
        for name, payload in original.items():
            self.assertEqual(self.client.files[name], payload)
        self.client.fail_commit = False
        self.publish({"windows"}, build_id="10.2")
        self.assertNotIn("new-windows-nightly.10.1.exe", self.client.files)
        self.assert_manifest_matches()

    def test_checksum_upload_failure_keeps_old_body_and_downloads(self):
        original = copy.deepcopy(self.client.files)
        body = self.client.current["body"]
        self.client.fail_checksum = True
        with self.assertRaisesRegex(RuntimeError, "校验清单"):
            self.publish({"windows"})
        self.assertEqual(self.client.current["body"], body)
        for name, payload in original.items():
            self.assertEqual(self.client.files[name], payload)
        self.assertNotIn("delete", self.client.events)

    def test_cleanup_failure_leaves_new_downloads_available(self):
        self.client.fail_cleanup = True
        state = self.publish({"windows"})
        self.assertEqual(state["platforms"]["windows"]["source"], NEW_SHA)
        self.assert_manifest_matches()

    def test_cleanup_list_failure_does_not_fail_completed_publication(self):
        assets = self.client.assets

        def list_assets(release):
            if "commit" in self.client.events:
                raise RuntimeError("合成清理列表网络中断")
            return assets(release)

        with patch.object(self.client, "assets", side_effect=list_assets):
            state = self.publish({"windows"})
        self.assertEqual(state["platforms"]["windows"]["source"], NEW_SHA)
        self.assert_manifest_matches()

    def test_ios_only_does_not_publish_simulator_archive(self):
        self.client = FakeGitHub(legacy=False)
        state = self.publish({"ios"})
        self.assertEqual(set(state["platforms"]), {"ios"})
        self.assertEqual(state["platforms"]["ios"]["files"], [])
        self.assertEqual(self.client.files, {})
        self.assertIn("已提交 TestFlight，待 Apple 审核", self.client.current["body"])

    def test_unreleased_ios_version_is_reported_as_internal_only(self):
        self.client = FakeGitHub(legacy=False)
        self.publish({"ios"}, ios_group="internal")
        body = self.client.current["body"]
        self.assertIn("仅上传 TestFlight 内部组：这个版本号尚未发布", body)
        self.assertNotIn("已提交 TestFlight，待 Apple 审核", body)

    def test_unknown_previous_body_prevents_mutations(self):
        self.client.current["body"] = "unknown layout"
        original = copy.deepcopy(self.client.files)
        with self.assertRaisesRegex(ValueError, "无法确认"):
            self.publish({"windows"})
        self.assertEqual(self.client.files, original)
        self.assertEqual(self.client.events, [])

    def test_metadata_digest_mismatch_prevents_mutations(self):
        self.publish({"windows"})
        self.client.files["new-windows-nightly.10.1.exe"] = b"changed outside publisher"
        self.client.events.clear()
        with self.assertRaisesRegex(ValueError, "摘要不符"):
            self.publish({"linux"}, build_id="11.1")
        self.assertEqual(self.client.events, [])

    def test_legacy_names_are_identified_by_native_format(self):
        self.client.files["entry-default-unsigned.hap"] = self.client.files.pop("old-harmony.hap")
        self.client.files["MetasequoiaIME-Full_Setup_v0.1.0.exe"] = self.client.files.pop("old-windows.exe")
        state = self.publish({"android"})
        self.assertEqual(state["platforms"]["harmony"]["files"][0]["name"], "entry-default-unsigned.hap")
        self.assertEqual(state["platforms"]["windows"]["files"][0]["name"], "MetasequoiaIME-Full_Setup_v0.1.0.exe")
        self.assert_manifest_matches()

    def test_workflow_environment_reaches_the_incremental_publisher(self):
        import json
        env = {"NEEDS_RESULTS": json.dumps({
                   platform: {"result": "success" if platform in ("windows", "ios") else "failure",
                              "outputs": {"testflight_group": "internal"} if platform == "ios" else {}}
                   for platform in PUBLISHING.PLATFORMS}),
               "DOWNLOAD_RESULTS": json.dumps({"windows": {"outcome": "success"}}),
               "GITHUB_REPOSITORY": "synthetic/ime", "SOURCE_SHA": NEW_SHA,
               "GITHUB_RUN_ID": "12", "GITHUB_RUN_ATTEMPT": "2",
               "GITHUB_SERVER_URL": "https://github.com"}
        with patch.dict(os.environ, env), patch.object(PUBLISHING, "GitHub", return_value=self.client), \
                patch.object(PUBLISHING.sys, "argv", ["publish-nightly.py", str(self.source), str(self.root / "cli")]):
            PUBLISHING.main()
        self.assertEqual(self.state()["platforms"]["windows"]["source"], NEW_SHA)
        self.assertIn("new-windows-nightly.12.2.exe", self.client.files)
        self.assertEqual(self.state()["platforms"]["android"]["source"], OLD_SHA)
        self.assertIn("仅上传 TestFlight 内部组", self.client.current["body"])


class GitHubClientTests(unittest.TestCase):
    def test_only_404_is_treated_as_missing(self):
        client = PUBLISHING.GitHub("synthetic/ime")
        with patch.object(PUBLISHING.subprocess, "run", side_effect=[
                subprocess.CompletedProcess([], 1, "", "HTTP 404"),
                subprocess.CompletedProcess([], 0, "[]", "")]):
            self.assertIsNone(client.release())
        with patch.object(PUBLISHING.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "HTTP 401")):
            with self.assertRaisesRegex(RuntimeError, "401"):
                client.release()

    def test_all_asset_pages_are_used(self):
        client = PUBLISHING.GitHub("synthetic/ime")
        with patch.object(client, "api", return_value=[[{"name": "a"}], [{"name": "b"}]]) as api:
            self.assertEqual(set(client.assets({"id": 1})), {"a", "b"})
            self.assertTrue(api.call_args.kwargs["paginate"])

    def test_interrupted_initial_draft_is_reused(self):
        client = PUBLISHING.GitHub("synthetic/ime")
        draft = {"id": 2, "tag_name": "nightly", "draft": True}
        with patch.object(client, "api", side_effect=[None, [[], [draft]]]):
            self.assertEqual(client.release(), draft)

    def test_initial_draft_is_published_before_ref_is_moved(self):
        client = PUBLISHING.GitHub("synthetic/ime")
        with patch.object(client, "api") as api:
            client.commit({"id": 1}, "synthetic notes", NEW_SHA)
        self.assertEqual(api.call_args_list[0].args[1], "releases/1")
        self.assertFalse(api.call_args_list[0].args[2]["draft"])
        self.assertEqual(api.call_args_list[1].args[1], "git/refs/tags/nightly")


if __name__ == "__main__":
    unittest.main()

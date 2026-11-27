#!/usr/bin/env python3
"""release-ref.py 的契约：推 tag 发版，正式版和 -beta.N 测试版的发布名、预发布标记，版本号与 main 祖先关系的核对，以及非 tag 运行只构建不发布。另核对每个 Release 工作流都由自己前缀的 tag 触发、经它决定是否发布，不再按分支名加后缀。"""

import importlib.util
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/release-ref.py"
SPEC = importlib.util.spec_from_file_location("release_ref", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
# 每个 Release 工作流的 tag 前缀，与 update_check.rs 认的 <平台>-v 前缀和已经发出去的 tag 相同。
PREFIXES = {
    "release-android.yml": ["android-v"],
    "release-ios.yml": ["ios-v"],
    "release-macos.yml": ["macos-v"],
    "release-linux.yml": ["linux-v"],
    "release-harmony.yml": ["harmony-v"],
    "release-windows.yml": ["windows-v"],
    "release-windows7.yml": ["windows7-v"],
    "release-web-engine.yml": ["web-engine-v"],
    "release-pack-tool.yml": ["pack-tool-v"],
    "release-engine-capi.yml": ["msime-engine-capi-v"],
    "release-engine-crates.yml": ["msime-engine-v", "msime-engine-capi-v"],
    "release-script-sdk.yml": ["script-sdk-v"],
}


def resolve(ref, tree="1.2.3", requested="", publish="", stable_only=False, on_main=True):
    return MODULE.resolve(ref, "android-v", tree, requested, publish, stable_only, on_main=lambda: on_main)


class ResolveTests(unittest.TestCase):
    def test_a_plain_tag_is_a_published_final_release(self):
        self.assertEqual(resolve("refs/tags/android-v1.2.3"),
                         {"version": "1.2.3", "release": "1.2.3", "tag": "android-v1.2.3", "prerelease": "false", "publish": "true"})

    def test_beta_tags_are_published_prereleases_with_the_suffix(self):
        for suffix in ("-beta.1", "-beta.2", "-beta.10", "-beta"):
            out = resolve(f"refs/tags/android-v1.2.3{suffix}")
            self.assertEqual((out["version"], out["release"], out["prerelease"], out["publish"]),
                             ("1.2.3", f"1.2.3{suffix}", "true", "true"), suffix)

    def test_other_suffixes_and_prefixes_are_refused(self):
        for tag in ("android-v1.2.3-alpha", "android-v1.2.3-beta.0", "android-v1.2.3-beta.01", "android-v1.2.3-beta.",
                    "android-v1.2.3-beta.1.2", "android-v1.2.3-beta1", "android-v1.2.3-rc.1", "android-v1.2",
                    "android-1.2.3", "ios-v1.2.3", "nightly"):
            with self.assertRaises(MODULE.ReleaseRefError, msg=tag):
                resolve(f"refs/tags/{tag}")

    def test_stable_only_workflows_refuse_betas(self):
        self.assertEqual(resolve("refs/tags/android-v1.2.3", stable_only=True)["publish"], "true")
        with self.assertRaises(MODULE.ReleaseRefError):
            resolve("refs/tags/android-v1.2.3-beta.1", stable_only=True)

    def test_the_tag_must_match_the_version_in_the_commit(self):
        with self.assertRaisesRegex(MODULE.ReleaseRefError, "1.2.4"):
            resolve("refs/tags/android-v1.2.4")
        with self.assertRaises(MODULE.ReleaseRefError):
            resolve("refs/tags/android-v1.2.3", requested="1.2.4")
        self.assertEqual(resolve("refs/tags/android-v1.2.3", requested="1.2.3")["release"], "1.2.3")
        # 没有仓库内版本号的工作流（网页引擎、打包工具）只核对祖先关系。
        self.assertEqual(resolve("refs/tags/android-v9.9.9", tree=None)["version"], "9.9.9")

    def test_the_tagged_commit_must_be_on_main(self):
        with self.assertRaisesRegex(MODULE.ReleaseRefError, "origin/main"):
            resolve("refs/tags/android-v1.2.3", on_main=False)

    def test_a_tag_dispatch_with_publish_off_only_builds(self):
        self.assertEqual(resolve("refs/tags/android-v1.2.3", publish="false")["publish"], "false")
        self.assertEqual(resolve("refs/tags/android-v1.2.3", publish="true")["publish"], "true")

    def test_branches_only_build(self):
        for ref in ("refs/heads/main", "refs/heads/feature", "refs/pull/1/merge", ""):
            for publish in ("", "true", "false"):
                self.assertEqual(resolve(ref, publish=publish),
                                 {"version": "1.2.3", "release": "1.2.3", "tag": "", "prerelease": "false", "publish": "false"})
        self.assertEqual(resolve("refs/heads/main", requested="2.0.0")["version"], "2.0.0")
        self.assertEqual(resolve("refs/heads/main", tree=None)["version"], "")

    def test_malformed_versions_are_refused(self):
        with self.assertRaises(MODULE.ReleaseRefError):
            resolve("refs/heads/main", requested="1.2")
        with self.assertRaises(MODULE.ReleaseRefError):
            resolve("refs/heads/main", tree="1.2.3-beta")


@unittest.skipUnless(shutil.which("git"), "git is not installed")
class GitTests(unittest.TestCase):
    """在真的仓库里跑命令行：origin/main 的祖先关系由 git 判断，浅克隆也要判断对。"""

    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_") and k not in ("GITHUB_OUTPUT", "PUBLISH_INPUT")}
        self.env.update({"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.com", "GIT_COMMITTER_NAME": "t",
                         "GIT_COMMITTER_EMAIL": "t@example.com", "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"})
        origin = self.root / "origin"
        self.git(self.root, "init", "-q", "-b", "main", str(origin))
        (origin / "version.txt").write_text("1.2.3\n")
        self.git(origin, "add", "version.txt")
        self.git(origin, "commit", "-q", "-m", "1.2.3")
        self.git(origin, "checkout", "-q", "-b", "feature")
        (origin / "version.txt").write_text("1.2.4\n")
        self.git(origin, "commit", "-q", "-am", "1.2.4 off main")
        self.feature = self.git(origin, "rev-parse", "HEAD").strip()
        self.git(origin, "checkout", "-q", "main")
        (origin / "other.txt").write_text("x\n")
        self.git(origin, "add", "other.txt")
        self.git(origin, "commit", "-q", "-m", "second")
        self.origin = origin

    def git(self, cwd, *args):
        return subprocess.run(["git", *args], cwd=cwd, env=self.env, check=True, capture_output=True, text=True).stdout

    def run_script(self, clone, ref, *extra):
        output = clone / "github-output"
        env = dict(self.env, GITHUB_REF=ref, GITHUB_OUTPUT=str(output))
        done = subprocess.run([sys.executable, str(SCRIPT), "--prefix", "android-v", "--version-file", "version.txt", *extra],
                              cwd=clone, env=env, capture_output=True, text=True, timeout=60)
        written = output.read_text() if output.exists() else ""
        return done, written

    def clone(self, name, commit, *args):
        clone = self.root / name
        self.git(self.root, "clone", "-q", "--no-local", *args, self.origin.as_uri(), str(clone))
        self.git(clone, "fetch", "-q", "origin", commit)
        self.git(clone, "checkout", "-q", "--detach", commit)
        return clone

    def test_a_tag_on_main_publishes_and_writes_the_outputs(self):
        first = self.git(self.origin, "rev-list", "--max-parents=0", "main").strip()
        clone = self.clone("full", first)
        done, written = self.run_script(clone, "refs/tags/android-v1.2.3-beta.2")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout, written)
        self.assertIn("release=1.2.3-beta.2\n", written)
        self.assertIn("publish=true\n", written)

    def test_a_tag_off_main_fails_even_from_a_shallow_clone(self):
        for name, args in (("full-off", ()), ("shallow-off", ("--depth", "1"))):
            clone = self.clone(name, self.feature, *args)
            done, written = self.run_script(clone, "refs/tags/android-v1.2.4")
            self.assertNotEqual(done.returncode, 0, name)
            self.assertIn("origin/main", done.stderr, name)
            self.assertEqual(written, "", name)

    def test_a_shallow_clone_of_main_still_passes(self):
        clone = self.clone("shallow-on", "main", "--depth", "1")
        done, _ = self.run_script(clone, "refs/tags/android-v1.2.3")
        self.assertEqual(done.returncode, 0, done.stderr)


class WorkflowTests(unittest.TestCase):
    def test_every_release_workflow_is_tag_triggered_and_resolves_through_the_script(self):
        workflows = sorted(p.name for p in (ROOT / ".github/workflows").glob("release-*.yml"))
        self.assertEqual(workflows, sorted(PREFIXES), "a release workflow was added or removed; list its tag prefix here")
        for name, prefixes in PREFIXES.items():
            text = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            trigger = re.search(r"(?m)^on:\n  push:\n    tags: \[(?P<tags>[^\]]*)\]\n", text)
            self.assertIsNotNone(trigger, f"{name} is not triggered by pushing a tag")
            self.assertEqual([t.strip().strip("'\"") for t in trigger.group("tags").split(",")], [p + "*" for p in prefixes], name)
            self.assertTrue("workflow_dispatch:" in text, name)
            # release-engine-crates.yml 按 tag 选前缀（--prefix "$prefix"），其余直接写死自己的前缀。
            for prefix in prefixes:
                self.assertTrue(f"scripts/release-ref.py --prefix {prefix}" in text or (len(prefixes) > 1 and f"prefix={prefix}" in text), f"{name} does not resolve {prefix} tags through release-ref.py")
            # 以前按分支名决定后缀和能不能发布；现在只看 tag。
            for stale in ("develop", "-alpha", "suffix=-beta"):
                self.assertFalse(stale in text, f"{name} still mentions {stale}")

    def test_release_creation_only_targets_the_pushed_tag(self):
        for name in PREFIXES:
            text = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            for line in text.splitlines():
                if "gh release create" in line:
                    self.assertIn("--verify-tag", line, f"{name}: gh release create must publish the existing tag, not create one")


if __name__ == "__main__":
    unittest.main()

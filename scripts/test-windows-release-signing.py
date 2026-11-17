#!/usr/bin/env python3
"""执行发布签名策略，验证无凭据构建、分支限制和显式配置失败。"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "platforms/windows/scripts/select-release-signing.sh"


class SigningPolicyTests(unittest.TestCase):
    def select(self, success=True, **overrides):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "outputs"
            env = {**os.environ, "PROVIDER": "simplysign", "PUBLISH": "true", "TEST": "false", "GITHUB_REF_NAME": "develop", "GITHUB_REF_TYPE": "branch", "GITHUB_REPOSITORY": "metasequoiaime/msime", "SSIGN_SHA256": "a" * 64, "CERTIFICATE_THUMBPRINT": "b" * 40, "TOKEN_SET": "true", "ORGANIZATION": "synthetic-org", "PROJECT": "synthetic-project", "POLICY": "release-signing", "GITHUB_OUTPUT": str(output), **overrides}
            result = subprocess.run(["bash", str(SCRIPT)], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text().splitlines()) if output.exists() else {}
            if not success:
                self.assertEqual(values, {}, "配置错误不能产生可继续发布的输出")
            return values

    def test_simplysign_trusted_branches(self):
        for branch in ("main", "develop"):
            with self.subTest(branch=branch):
                self.assertEqual(self.select(GITHUB_REF_NAME=branch), {"simplysign": "true", "signpath": "false", "policy": ""})

    def test_simplysign_rejects_untrusted_source(self):
        for change in ({"GITHUB_REF_NAME": "feature/test"}, {"GITHUB_REF_TYPE": "tag"}, {"GITHUB_REPOSITORY": "synthetic/fork"}):
            with self.subTest(change=change):
                self.select(success=False, **change)

    def test_simplysign_requires_pins(self):
        for change in ({"SSIGN_SHA256": ""}, {"SSIGN_SHA256": "not-a-hash"}, {"CERTIFICATE_THUMBPRINT": ""}, {"CERTIFICATE_THUMBPRINT": "bad"}):
            with self.subTest(change=change):
                self.select(success=False, **change)

    def test_unsigned_build_needs_no_signing_configuration(self):
        self.assertEqual(self.select(PUBLISH="false", SSIGN_SHA256="", CERTIFICATE_THUMBPRINT="", TOKEN_SET="false", GITHUB_REF_NAME="feature/test"), {"simplysign": "false", "signpath": "false", "policy": ""})

    def test_unconfigured_provider_preserves_unsigned_release(self):
        self.assertEqual(self.select(PROVIDER="")["simplysign"], "false")

    def test_signpath_preserves_policy_and_alpha_behavior(self):
        self.assertEqual(self.select(PROVIDER="signpath"), {"simplysign": "false", "signpath": "true", "policy": "release-signing"})
        self.assertEqual(self.select(PROVIDER="signpath", GITHUB_REF_NAME="feature/test")["signpath"], "false")
        for change in ({"TOKEN_SET": "false"}, {"ORGANIZATION": ""}, {"PROJECT": ""}, {"POLICY": ""}):
            with self.subTest(change=change):
                self.select(success=False, PROVIDER="signpath", **change)

    def test_signpath_test_overrides_provider_and_publish(self):
        self.assertEqual(self.select(TEST="true", PUBLISH="false", GITHUB_REF_NAME="feature/test", SSIGN_SHA256=""), {"simplysign": "false", "signpath": "true", "policy": "test-signing"})

    def test_unknown_provider_fails(self):
        self.select(success=False, PROVIDER="unknown")


if __name__ == "__main__":
    unittest.main()

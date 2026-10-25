#!/usr/bin/env python3
"""用合成证书和包验证鸿蒙发布签名的拒绝条件，不访问真实签名资料。"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("harmony_signing", ROOT / "platforms/harmony/sign-release.py")
SIGNING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNING)


@unittest.skipUnless(shutil.which("openssl"), "缺少 openssl")
class ReleaseSigning(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="msime-harmony-signing-test-")
        cls.directory = Path(cls.temporary.name)
        cls.certificates = []
        for index in range(2):
            cert = cls.directory / f"synthetic-{index}.pem"
            subprocess.run([
                "openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                "-nodes", "-keyout", str(cls.directory / f"synthetic-{index}.key"), "-out", str(cert),
                "-days", "2", "-subj", f"/CN=Synthetic Release Test {index}",
            ], check=True, capture_output=True)
            cls.certificates.append(cert.read_bytes())

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def profile(self):
        return {
            "type": "release", "app-distribution-type": "app_gallery",
            "validity": {"not-before": 100, "not-after": 300},
            "bundle-info": {"bundle-name": SIGNING.BUNDLE, "data-group-ids": [SIGNING.DATA_GROUP],
                            "distribution-certificate": self.certificates[0].decode()},
            "acls": {"allowed-acls": ["ohos.permission.READ_PASTEBOARD"]},
        }

    def test_valid_profile_with_certificate_chain(self):
        SIGNING.validate_profile(self.profile(), self.certificates[1] + self.certificates[0], now=200)

    def test_debug_profile_rejected(self):
        profile = self.profile()
        profile["type"] = "debug"
        with self.assertRaisesRegex(ValueError, "release Profile"):
            SIGNING.validate_profile(profile, self.certificates[0], now=200)

    def test_wrong_channel_rejected(self):
        profile = self.profile()
        profile["app-distribution-type"] = "enterprise"
        with self.assertRaisesRegex(ValueError, "应用市场"):
            SIGNING.validate_profile(profile, self.certificates[0], now=200)

    def test_expired_and_future_profiles_rejected(self):
        for now in [50, 300, 400]:
            with self.subTest(now=now), self.assertRaisesRegex(ValueError, "生效|过期"):
                SIGNING.validate_profile(self.profile(), self.certificates[0], now=now)

    def test_wrong_bundle_and_missing_group_rejected(self):
        for field, value in [("bundle-name", "app.synthetic.other"), ("data-group-ids", [])]:
            profile = self.profile()
            profile["bundle-info"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                SIGNING.validate_profile(profile, self.certificates[0], now=200)

    def test_missing_acl_and_device_limited_profile_rejected(self):
        for field, value in [("acls", {}), ("debug-info", {"device-ids": ["synthetic-device"]})]:
            profile = self.profile()
            profile[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                SIGNING.validate_profile(profile, self.certificates[0], now=200)

    def test_certificate_mismatch_rejected(self):
        with self.assertRaisesRegex(ValueError, "证书不匹配"):
            SIGNING.validate_profile(self.profile(), self.certificates[1], now=200)

    def test_tool_errors_do_not_disclose_output_or_arguments(self):
        secret = "synthetic-secret-not-for-logs"
        with self.assertRaises(ValueError) as failure:
            SIGNING.run([sys.executable, "-c", f"import sys;sys.exit({secret!r})", secret])
        self.assertNotIn(secret, str(failure.exception))

    def hap(self, version="0.2.0", omitted=None):
        path = self.directory / "synthetic.hap"
        entries = {
            "module.json": json.dumps({"app": {"bundleName": SIGNING.BUNDLE, "versionName": version, "versionCode": 2000, "debug": False}}),
            "pack.info": "{}", "libs/arm64-v8a/libmsime_host_api.so": "synthetic",
            "libs/arm64-v8a/libmsimeclient.so": "synthetic", "resources/rawfile/settings/index.html": "synthetic",
            "resources/resfile/handwriting/handwriting-zh_CN.model": "synthetic",
            "resources/resfile/handwriting/HandwritingModel-LICENSE.txt": "synthetic",
            "resources/resfile/handwriting/Zinnia-LICENSE.txt": "synthetic",
            "resources/resfile/engine/synthetic.dat": "synthetic", "resources/resfile/licenses/synthetic.txt": "synthetic",
            "resources/resfile/wordbooks/synthetic.json": "synthetic",
        }
        with zipfile.ZipFile(path, "w") as package:
            for name, content in entries.items():
                if name != omitted:
                    package.writestr(name, content)
        return path

    def test_complete_hap_accepted(self):
        self.assertEqual(SIGNING.validate_hap(self.hap(), "0.2.0"), b"{}")

    def test_wrong_version_rejected(self):
        with self.assertRaisesRegex(ValueError, "版本不匹配"):
            SIGNING.validate_hap(self.hap(version="0.1.4"), "0.2.0")

    def test_debug_build_rejected(self):
        path = self.hap()
        debug_path = self.directory / "debug.hap"
        with zipfile.ZipFile(path) as original, zipfile.ZipFile(debug_path, "w") as result:
            for name in original.namelist():
                data = original.read(name)
                if name == "module.json":
                    module = json.loads(data)
                    module["app"]["debug"] = True
                    data = json.dumps(module).encode()
                result.writestr(name, data)
        with self.assertRaisesRegex(ValueError, "调试标记"):
            SIGNING.validate_hap(debug_path, "0.2.0")

    def test_incomplete_resources_rejected(self):
        for omitted in ["libs/arm64-v8a/libmsime_host_api.so", "resources/resfile/licenses/synthetic.txt",
                        "resources/resfile/wordbooks/synthetic.json", "resources/resfile/handwriting/Zinnia-LICENSE.txt"]:
            with self.subTest(omitted=omitted), self.assertRaises(ValueError):
                SIGNING.validate_hap(self.hap(omitted=omitted), "0.2.0")

    def test_public_release_uses_signed_outputs(self):
        workflow = (ROOT / ".github/workflows/release-harmony.yml").read_text()
        publish = workflow.split("name: Publish independent HarmonyOS release", 1)[1]
        self.assertIn('steps.signing.outputs.hap', publish)
        self.assertNotIn('steps.hap.outputs.path', publish)
        self.assertIn('"$HAP" "$SUMS"', publish)
        self.assertIn('appgallery-msime-harmony-', workflow)
        self.assertNotIn("find platforms/harmony -type f -name '*.hap'", workflow)

    def test_market_packer_preserves_hap_signature(self):
        source = (ROOT / "platforms/harmony/sign-release.py").read_text()
        self.assertIn('"--replace-pack-info", "false"', source)


if __name__ == "__main__":
    unittest.main()

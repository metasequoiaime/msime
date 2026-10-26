#!/usr/bin/env python3
"""用合成证书和包验证鸿蒙发布签名与拒绝条件，不访问真实签名资料。"""

from __future__ import annotations

import argparse
import base64
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import zipfile
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("harmony_signing", ROOT / "platforms/harmony/sign-release.py")
SIGNING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNING)
COMPRESS_SPEC = importlib.util.spec_from_file_location("harmony_compression", ROOT / "scripts/compress_harmony_hap.py")
COMPRESSION = importlib.util.module_from_spec(COMPRESS_SPEC)
COMPRESS_SPEC.loader.exec_module(COMPRESSION)
TOOL_DIR = Path(os.environ.get("DEVECO_SDK_HOME", "/missing-harmony-sdk")) / "default/openharmony/toolchains/lib"


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
            SIGNING.run([
                sys.executable,
                "-c",
                f"import sys;sys.stderr.write('synthetic diagnostic');sys.exit({secret!r})",
                secret,
            ])
        self.assertIn("synthetic diagnostic", str(failure.exception))
        self.assertNotIn(secret, str(failure.exception))

    def test_tool_failure_identifies_stage(self):
        with self.assertRaisesRegex(ValueError, "验证 HAP 签名 执行失败"):
            SIGNING.run([sys.executable, "-c", "raise SystemExit(1)"], stage="验证 HAP 签名")

    def hap(self, version="0.2.0", omitted=None):
        path = self.directory / "synthetic.hap"
        major, minor, patch = map(int, version.split("."))
        code = major * 1_000_000 + minor * 1_000 + patch
        entries = {
            "module.json": json.dumps({"app": {"bundleName": SIGNING.BUNDLE, "versionName": version, "versionCode": code, "debug": False}}),
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
        locate = workflow.split("name: Locate the packaged HAP", 1)[1].split("- id: signing", 1)[0]
        self.assertIn("compress_harmony_hap.py", locate)
        self.assertIn('echo "path=$hap" >> "$GITHUB_OUTPUT"', locate)
        self.assertIn("UNSIGNED_HAP: ${{ steps.hap.outputs.path }}", workflow)

    def test_market_packer_preserves_hap_signature(self):
        source = (ROOT / "platforms/harmony/sign-release.py").read_text()
        self.assertIn('"--replace-pack-info", "false"', source)

    @unittest.skipUnless(
        (TOOL_DIR / "hap-sign-tool.jar").is_file()
        and (TOOL_DIR / "app_packing_tool.jar").is_file()
        and shutil.which("java") and shutil.which("keytool"),
        "缺少 HarmonyOS SDK 或 Java；配置 DEVECO_SDK_HOME 后验证真实签名工具",
    )
    def test_compressed_hap_signed_and_packed_by_real_tools(self):
        # 全部证书和授权数据在测试中生成，不读取发布密钥或本机调试资料。
        with tempfile.TemporaryDirectory(prefix="msime-harmony-signing-tool-test-") as temporary:
            directory = Path(temporary)
            self.create_signing_chain(directory)
            profile = self.profile()
            profile.update({
                "version-name": "2.0.0", "version-code": 2,
                "uuid": "00000000-0000-0000-0000-000000000000", "issuer": "pki_internal",
                "validity": {"not-before": int(time.time()) - 60, "not-after": int(time.time()) + 3600},
                "permissions": {"restricted-permissions": []},
            })
            profile["bundle-info"].update({
                "distribution-certificate": (directory / "leaf.pem").read_text(),
                "developer-id": "Synthetic", "apl": "normal", "app-feature": "hos_normal_app",
            })
            (directory / "profile.json").write_text(json.dumps(profile))
            self.tool_command([
                "openssl", "cms", "-sign", "-binary", "-nodetach", "-in", str(directory / "profile.json"),
                "-signer", str(directory / "leaf.pem"), "-inkey", str(directory / "leaf.key"),
                "-outform", "DER", "-out", str(directory / "profile.p7b"),
            ])
            source = self.hap(version="0.2.1")
            complete = directory / "complete.hap"
            compressed = directory / "compressed.hap"
            pack_info = {
                "summary": {"app": {"bundleName": SIGNING.BUNDLE, "bundleType": "app",
                                    "version": {"code": 2001, "name": "0.2.1"}},
                            "modules": [{"deviceType": ["phone"],
                                         "distro": {"moduleName": "entry", "moduleType": "entry",
                                                    "installationFree": False, "deliveryWithInstall": True},
                                         "apiVersion": {"compatible": 21, "target": 21, "releaseType": "Release"}}]},
                "packages": [{"deviceType": ["phone"], "moduleType": "entry",
                              "deliveryWithInstall": True, "name": "entry-default"}],
            }
            with zipfile.ZipFile(source) as original, zipfile.ZipFile(complete, "w") as result:
                for entry in original.infolist():
                    data = original.read(entry)
                    if entry.filename == "pack.info":
                        data = json.dumps(pack_info).encode()
                    elif entry.filename == "module.json":
                        module = json.loads(data)
                        module["module"] = {"name": "entry", "type": "entry", "packageName": "entry",
                                            "deviceTypes": ["phone"], "deliveryWithInstall": True,
                                            "installationFree": False}
                        data = json.dumps(module).encode()
                    result.writestr(entry, data)
                result.writestr("resources/resfile/synthetic-large.txt", b"synthetic-resource-" * 4096)
                result.writestr("ets/modules.abc", b"synthetic-code")
            COMPRESSION.compress_resources(complete, compressed)
            environment = {f"HARMONY_RELEASE_{key}_BASE64": base64.b64encode((directory / filename).read_bytes()).decode()
                           for key, filename in [("KEYSTORE", "store.p12"), ("PROFILE", "profile.p7b"),
                                                 ("CERTIFICATE", "chain.pem")]}
            environment.update({"HARMONY_RELEASE_KEYSTORE_PASSWORD": "synthetic-password",
                                "HARMONY_RELEASE_KEY_PASSWORD": "synthetic-password", "HARMONY_RELEASE_KEY_ALIAS": "msime"})
            output = directory / "release"
            with mock.patch.dict(os.environ, environment):
                SIGNING.sign(argparse.Namespace(input=compressed, output_dir=output, version="0.2.1",
                                                tool_dir=TOOL_DIR, check_only=False, signed_hap=False))
            signed = output / "msime-harmony-0.2.1.hap"
            # 独立复验最终文件，防止脚本内部验签后又改写发布产物。
            self.tool_command(["java", "-jar", str(TOOL_DIR / "hap-sign-tool.jar"), "verify-app", "-inFile", str(signed),
                               "-outCertChain", str(directory / "verified.cer"), "-outProfile", str(directory / "verified.p7b")])
            self.assertEqual((directory / "verified.p7b").read_bytes(), (directory / "profile.p7b").read_bytes())
            self.tool_command(["java", "-jar", str(TOOL_DIR / "hap-sign-tool.jar"), "verify-app",
                               "-inFile", str(output / "msime-harmony-0.2.1.app"),
                               "-outCertChain", str(directory / "app-verified.cer"),
                               "-outProfile", str(directory / "app-verified.p7b")])
            self.assertEqual((directory / "app-verified.p7b").read_bytes(), (directory / "profile.p7b").read_bytes())
            with zipfile.ZipFile(signed) as package:
                self.assertEqual(package.getinfo("resources/resfile/synthetic-large.txt").compress_type, zipfile.ZIP_DEFLATED)
                self.assertEqual(package.getinfo("libs/arm64-v8a/libmsime_host_api.so").compress_type, zipfile.ZIP_STORED)
            with zipfile.ZipFile(output / "msime-harmony-0.2.1.app") as package:
                self.assertEqual(package.read("entry-default.hap"), signed.read_bytes())
            repacked = directory / "repacked"
            with mock.patch.dict(os.environ, environment):
                SIGNING.sign(argparse.Namespace(input=signed, output_dir=repacked, version="0.2.1",
                                                tool_dir=TOOL_DIR, check_only=False, signed_hap=True))
            self.assertEqual((repacked / signed.name).read_bytes(), signed.read_bytes())
            self.tool_command(["java", "-jar", str(TOOL_DIR / "hap-sign-tool.jar"), "verify-app",
                               "-inFile", str(repacked / "msime-harmony-0.2.1.app"),
                               "-outCertChain", str(directory / "repacked.cer"),
                               "-outProfile", str(directory / "repacked.p7b")])
            with self.assertRaisesRegex(ValueError, "必须在签名前"):
                COMPRESSION.compress_resources(signed, directory / "invalid.hap")
            self.assertFalse((directory / "invalid.hap").exists())

    def tool_command(self, command):
        result = subprocess.run(command, capture_output=True)
        self.assertEqual(result.returncode, 0, (result.stdout + result.stderr).decode(errors="replace"))

    def create_signing_chain(self, directory):
        for name, parent in [("root", None), ("intermediate", "root"), ("leaf", "intermediate")]:
            subject = f"/CN=Synthetic Harmony {name}"
            if parent is None:
                self.tool_command([
                    "openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes",
                    "-keyout", str(directory / "root.key"), "-out", str(directory / "root.pem"), "-days", "2", "-subj", subject,
                    "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign",
                ])
                continue
            self.tool_command([
                "openssl", "req", "-new", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes",
                "-keyout", str(directory / f"{name}.key"), "-out", str(directory / f"{name}.csr"), "-subj", subject,
            ])
            extensions = ("basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign" if name == "intermediate"
                          else "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=codeSigning")
            (directory / f"{name}.ext").write_text(extensions + "\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid,issuer\n")
            self.tool_command([
                "openssl", "x509", "-req", "-in", str(directory / f"{name}.csr"), "-CA", str(directory / f"{parent}.pem"),
                "-CAkey", str(directory / f"{parent}.key"), "-CAcreateserial", "-out", str(directory / f"{name}.pem"),
                "-days", "2", "-extfile", str(directory / f"{name}.ext"),
            ])
        (directory / "chain.pem").write_bytes(b"".join((directory / f"{name}.pem").read_bytes()
                                                     for name in ["leaf", "intermediate", "root"]))
        self.tool_command(["openssl", "pkcs12", "-export", "-inkey", str(directory / "leaf.key"), "-in", str(directory / "leaf.pem"),
                           "-name", "msime", "-out", str(directory / "store.p12"), "-passout", "pass:synthetic-password"])


if __name__ == "__main__":
    unittest.main()

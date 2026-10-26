#!/usr/bin/env python3
"""离线验证华为上传、解析、公开测试提交与敏感日志边界。"""

import contextlib
import http.server
import importlib.util
import io
import json
import os
import shutil
import subprocess
from pathlib import Path
import tempfile
import threading
import unittest
from unittest import mock
import urllib.parse


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("appgallery", ROOT / "platforms/harmony/appgallery-release.py")
AGC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AGC)


def ok(**fields):
    return {"ret": {"code": 0}, **fields}


def state(status=0, pkg="pkg-1"):
    return ok(pkgStateList=[{"pkgId": pkg, "successStatus": status}])


class FakeTransport:
    def __init__(self, replies):
        self.replies = iter([{"access_token": "synthetic-token"}, *replies])
        self.calls = []

    def request(self, method, url, headers, data=None, *, binary=False):
        data = data.read() if binary else json.loads(data) if data else None
        self.calls.append((method, url, dict(headers), data))
        reply = next(self.replies)
        if isinstance(reply, Exception):
            raise reply
        return reply


class AppGalleryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "synthetic.app"
        self.path.write_bytes(b"synthetic signed package")
        self.output = io.StringIO()
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.stack.enter_context(mock.patch.dict(os.environ, {}, clear=True))
        self.stack.enter_context(contextlib.redirect_stdout(self.output))

    def client(self, replies):
        self.transport = FakeTransport(replies)
        return AGC.Client("123456", "synthetic-client", "synthetic-secret", self.transport)

    def upload_replies(self):
        return [ok(urlInfo={"url": "https://obs.example.invalid/file?signature=synthetic",
                            "method": "PUT", "headers": {"Authorization": "synthetic-obs"},
                            "objectId": "CN/synthetic.app"}), None, ok(pkgVersion=["pkg-1"])]

    def run_release(self, client, **options):
        return AGC.release(client, **{"path": self.path, "mode": "public-test",
                                     "config": AGC.test_config("0.2.1", ""), **options})

    def test_full_flow_uses_v3_and_correct_public_test_contract(self):
        client = self.client([*self.upload_replies(), state(1), state(), ok(versionId="draft-1"), ok(), ok()])
        with mock.patch.object(AGC.time, "sleep"):
            self.run_release(client)
        calls = self.transport.calls
        self.assertIn("/publish/v3/package/compile/status?", calls[4][1])
        self.assertEqual(calls[6][3]["testType"], 4)
        self.assertEqual(calls[6][3]["releaseType"], 6)
        self.assertEqual(calls[7][3]["pkgId"], "pkg-1")
        self.assertEqual(calls[8][3], {"versionId": "draft-1"})
        self.assertIn("review_status=submitted", self.output.getvalue())
        self.assertEqual(calls[2][2]["Authorization"], "synthetic-obs")
        self.assertNotIn("client_id", calls[2][2])
        self.assertEqual(calls[2][3], self.path.read_bytes())
        self.assertEqual(int(calls[2][2]["Content-Length"]), self.path.stat().st_size)
        query = urllib.parse.parse_qs(urllib.parse.urlsplit(calls[1][1]).query)
        self.assertEqual(query["sha256"], [AGC.hashlib.sha256(self.path.read_bytes()).hexdigest()])
        for secret in ("synthetic-secret", "synthetic-token", "synthetic-obs", "signature="):
            self.assertNotIn(secret, self.output.getvalue())

    def test_upload_mode_never_creates_or_submits_test(self):
        self.run_release(self.client([*self.upload_replies(), state()]), mode="upload")
        self.assertEqual(len(self.transport.calls), 5)

    def test_resume_package_and_draft_avoids_duplicate_creation(self):
        client = self.client([state(), ok(), ok()])
        self.run_release(client, package_id="pkg-1", version_id="draft-1")
        self.assertEqual(len(self.transport.calls), 4)
        self.assertEqual(self.transport.calls[2][0], "PUT")

    def test_resume_draft_still_uploads_when_no_package_given(self):
        self.run_release(self.client([*self.upload_replies(), state(), ok(), ok()]), version_id="draft-1")
        self.assertEqual(self.transport.calls[-2][3]["versionId"], "draft-1")
        self.assertFalse(any(call[0] == "POST" and "/test/app/version?" in call[1] for call in self.transport.calls))

    def test_upload_failure_never_adds_package_or_creates_test(self):
        replies = self.upload_replies()
        replies[1] = AGC.ReleaseError("模拟上传失败")
        with self.assertRaises(AGC.ReleaseError):
            self.run_release(self.client(replies))
        self.assertEqual(len(self.transport.calls), 3)

    def test_parse_failure_unknown_and_mismatched_status_never_submit(self):
        for status in (2, 99, None, "0", False):
            with self.subTest(status=status), self.assertRaises(AGC.ReleaseError):
                self.run_release(self.client([state(status)]), package_id="pkg-1")
            self.assertEqual(len(self.transport.calls), 2)
        with self.assertRaises(AGC.ReleaseError):
            self.run_release(self.client([state(pkg="other")]), package_id="pkg-1")

    def test_poll_timeout_keeps_package_id_and_never_creates_draft(self):
        client = self.client([state(1)])
        with mock.patch.object(AGC.time, "monotonic", side_effect=[0, 10]):
            with self.assertRaisesRegex(AGC.ReleaseError, "超时"):
                self.run_release(client, package_id="pkg-1", timeout=1)
        self.assertIn("package_id=pkg-1", self.output.getvalue())
        self.assertEqual(len(self.transport.calls), 2)

    def test_update_failure_records_recovery_id_and_does_not_submit(self):
        client = self.client([state(), ok(versionId="draft-1"), AGC.ReleaseError("资料缺失")])
        with self.assertRaises(AGC.ReleaseError):
            self.run_release(client, package_id="pkg-1")
        self.assertIn("test_version_id=draft-1", self.output.getvalue())
        self.assertEqual(len(self.transport.calls), 4)

    def test_submit_failure_is_not_retried_or_reported_as_success(self):
        client = self.client([state(), ok(), AGC.ReleaseError("审核拒绝")])
        with self.assertRaises(AGC.ReleaseError):
            self.run_release(client, package_id="pkg-1", version_id="draft-1")
        self.assertNotIn("review_status=submitted", self.output.getvalue())
        self.assertEqual(len(self.transport.calls), 4)

    def test_ret_accepts_documented_object_and_json_string(self):
        for ret in ({"code": 0}, '{"code":"0"}'):
            self.assertEqual(AGC.check_result({"ret": ret})["ret"], ret)
        for ret in (None, {"code": False}, {"code": "private contact"}, "private contact", {"code": 991}):
            with self.subTest(ret=ret), self.assertRaises(AGC.ReleaseError) as error:
                AGC.check_result({"ret": ret})
            self.assertNotIn("private contact", str(error.exception))

    def test_missing_token_or_credentials_fail(self):
        with self.assertRaises(AGC.ReleaseError):
            AGC.Client("123", "", "")
        transport = FakeTransport([])
        transport.replies = iter([ok()])
        with self.assertRaises(AGC.ReleaseError):
            AGC.Client("123", "client", "secret", transport)

    @unittest.skipUnless(shutil.which("openssl"), "缺少 openssl")
    def test_service_account_ps256_signature_and_no_token_exchange(self):
        key = Path(self.temp.name) / "synthetic.pem"
        public = Path(self.temp.name) / "public.pem"
        subprocess.run(["openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048",
                        "-out", str(key)], capture_output=True, check=True)
        subprocess.run(["openssl", "pkey", "-in", str(key), "-pubout", "-out", str(public)],
                       capture_output=True, check=True)
        transport = FakeTransport([])
        raw = json.dumps({"key_id": "synthetic-kid", "sub_account": "synthetic-account",
                          "private_key": key.read_text()})
        client = AGC.Client("123", transport=transport, service_account=raw)
        self.assertEqual(transport.calls, [])
        self.assertNotIn("client_id", client.headers)
        token = client.headers["Authorization"].removeprefix("Bearer ")
        header, payload, signature = token.split(".")
        decode = lambda value: AGC.base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))
        self.assertEqual(json.loads(decode(header)), {"kid": "synthetic-kid", "typ": "JWT", "alg": "PS256"})
        claims = json.loads(decode(payload))
        self.assertEqual(claims["aud"], "https://oauth-login.cloud.huawei.com/oauth2/v3/token")
        self.assertEqual(claims["iss"], "synthetic-account")
        self.assertEqual(claims["exp"] - claims["iat"], 3600)
        signature_file = Path(self.temp.name) / "signature.bin"
        signature_file.write_bytes(decode(signature))
        result = subprocess.run([
            "openssl", "dgst", "-sha256", "-verify", str(public), "-signature", str(signature_file),
            "-sigopt", "rsa_padding_mode:pss", "-sigopt", "rsa_pss_saltlen:32",
            "-sigopt", "rsa_mgf1_md:sha256",
        ], input=f"{header}.{payload}".encode(), capture_output=True)
        self.assertEqual(result.returncode, 0)
        self.assertNotIn("PRIVATE KEY", self.output.getvalue())

    def test_service_account_invalid_or_signing_failure_is_sanitized(self):
        for raw in ("{", "[]", "{}", '{"key_id":"kid","sub_account":"account","private_key":"bad"}'):
            with self.assertRaises(AGC.ReleaseError):
                AGC.service_account_headers(raw)
        raw = json.dumps({"key_id": "kid", "sub_account": "account",
                          "private_key": "-----BEGIN PRIVATE KEY-----synthetic"})
        with mock.patch.object(AGC.subprocess, "run", side_effect=subprocess.CalledProcessError(
                1, "openssl", stderr=b"private-key-must-not-appear")):
            with self.assertRaises(AGC.ReleaseError) as error:
                AGC.service_account_headers(raw)
        self.assertNotIn("private-key-must-not-appear", str(error.exception))

    def test_preflight_reads_version_list_with_app_id_header(self):
        client = self.client([ok(versionList=[])])
        client.check_access()
        method, url, headers, data = self.transport.calls[-1]
        self.assertEqual(method, "POST")
        self.assertTrue(url.endswith("/publish/v3/version/brief-info/list"))
        self.assertEqual(headers["appId"], "123456")
        self.assertEqual(data, {"packageName": "app.msime.hmos"})

    def test_config_inherits_information_and_uses_bounded_future_dates(self):
        config = AGC.test_config("0.2.1", "", now=100)
        info = config["openTestInfo"]
        self.assertEqual(info["startTime"], 100 + AGC.DAY)
        self.assertEqual(info["endTime"] - info["startTime"], 28 * AGC.DAY)
        self.assertNotIn("extInfo", config)
        self.assertNotIn("languages", config)
        self.assertEqual(info["testTaskInfo"]["needShareLink"], 1)
        self.assertNotIn("groupInfos", info["testTaskInfo"])

    def test_config_overrides_real_materials_without_printing(self):
        config = AGC.test_config("0.2.1", json.dumps({"extInfo": {"privacyPolicy": "https://example.invalid"},
                                                     "languages": [{"language": "zh-CN", "newFeatures": "合成测试"}]}))
        self.assertIn("languages", config)
        self.assertEqual(self.output.getvalue(), "")

    def test_invalid_config_fails_before_remote_mutation(self):
        cases = ["{", "[]", '{"pkgId":"other"}', '{"openTestInfo":[]}',
                 '{"openTestInfo":{"startTime":0}}', '{"openTestInfo":{"startTime":"bad"}}',
                 json.dumps({"openTestInfo": {"startTime": AGC.DAY, "endTime": 32 * AGC.DAY}}),
                 '{"openTestInfo":{"testTaskInfo":{"groupInfos":[]}}}']
        for raw in cases:
            with self.subTest(raw=raw), self.assertRaises(AGC.ReleaseError):
                AGC.test_config("0.2.1", raw, now=100)

    def test_malformed_package_ids_cannot_inject_outputs(self):
        for value in ("pkg\nother=1", "../pkg", None):
            with self.assertRaises(AGC.ReleaseError):
                AGC.record("package_id", value)
        with self.assertRaises(AGC.ReleaseError):
            self.run_release(self.client([]), version_id="bad\nid")
        self.assertEqual(len(self.transport.calls), 1)

    def test_outputs_are_saved_before_later_failure(self):
        output = Path(self.temp.name) / "output"
        summary = Path(self.temp.name) / "summary"
        with mock.patch.dict(os.environ, GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(summary)):
            with self.assertRaises(AGC.ReleaseError):
                self.run_release(self.client([state(2)]), package_id="pkg-1")
        self.assertIn("package_id=pkg-1", output.read_text())
        self.assertIn("`package_id`: `pkg-1`", summary.read_text())

    def test_unsafe_obs_urls_and_wrong_methods_are_rejected(self):
        for field, value in (("url", "http://example.invalid"), ("method", "POST")):
            replies = self.upload_replies()
            replies[0]["urlInfo"][field] = value
            with self.assertRaises(AGC.ReleaseError):
                self.client(replies).upload(self.path)
            self.assertEqual(len(self.transport.calls), 2)

    def test_workflow_defaults_and_signing_gates(self):
        workflow = (ROOT / ".github/workflows/release-harmony.yml").read_text()
        self.assertIn("options: [none, upload, public-test]", workflow)
        self.assertIn("default: none, type: string", workflow)
        self.assertEqual(workflow.count("if: ${{ (inputs.publish || inputs.sign || inputs.appgallery != 'none') && !inputs.test_package_id }}"), 5)
        self.assertLess(workflow.index("--check-only --mode"), workflow.index("Build the HarmonyOS settings page"))
        self.assertLess(workflow.index("name: appgallery-msime-harmony"), workflow.index("id: appgallery"))
        self.assertIn('--package-id "$TEST_PACKAGE_ID"', workflow)
        self.assertIn('"$PUBLISH" == false && -z "$SOURCE_RELEASE"', workflow)
        self.assertIn("if: ${{ !inputs.source_release && !inputs.test_package_id }}", workflow)

    def test_workflow_input_validation_runs_before_expensive_steps(self):
        workflow = (ROOT / ".github/workflows/release-harmony.yml").read_text()
        block = workflow.split("      - id: version\n", 1)[1].split("      - name:", 1)[0]
        script = block.split("        run: |\n", 1)[1]
        script = "\n".join(line[10:] for line in script.splitlines())
        defaults = {"REQUESTED": "0.2.1", "SOURCE_RELEASE": "", "SIGN": "false", "PUBLISH": "false",
                    "APPGALLERY": "none", "TEST_VERSION_ID": "", "TEST_PACKAGE_ID": "",
                    "GITHUB_REF_NAME": "develop", "GITHUB_OUTPUT": str(Path(self.temp.name) / "output")}
        cases = [({}, True), ({"APPGALLERY": "unknown"}, False),
                 ({"TEST_VERSION_ID": "draft-1"}, False), ({"TEST_PACKAGE_ID": "pkg-1"}, False),
                 ({"APPGALLERY": "upload", "SOURCE_RELEASE": "harmony-v0.2.1-beta"}, True),
                 ({"APPGALLERY": "upload", "TEST_PACKAGE_ID": "pkg-1"}, True),
                 ({"APPGALLERY": "public-test", "TEST_VERSION_ID": "draft-1", "TEST_PACKAGE_ID": "pkg-1"}, True),
                 ({"APPGALLERY": "upload", "TEST_PACKAGE_ID": "pkg-1", "PUBLISH": "true"}, False),
                 ({"APPGALLERY": "upload", "SOURCE_RELEASE": "harmony-v9.9.9-beta"}, False)]
        for changes, success in cases:
            with self.subTest(changes=changes):
                result = subprocess.run(["bash", "-e", "-c", script], env={**defaults, **changes},
                                        cwd=ROOT, capture_output=True)
                self.assertEqual(result.returncode == 0, success)

    def test_real_transport_streams_bytes_and_refuses_redirects(self):
        received = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_PUT(self):
                received.append(self.rfile.read(int(self.headers["Content-Length"])))
                self.send_response(200)
                self.end_headers()

            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(302)
                self.send_header("Location", "/must-not-follow")
                self.end_headers()

            def log_message(self, *args):
                pass

        with http.server.HTTPServer(("127.0.0.1", 0), Handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                url = f"http://127.0.0.1:{server.server_port}/upload"
                with self.path.open("rb") as source:
                    AGC.Transport().request("PUT", url, {"Content-Length": str(self.path.stat().st_size)}, source, binary=True)
                self.assertEqual(received, [self.path.read_bytes()])
                with self.assertRaisesRegex(AGC.ReleaseError, "HTTP 302"):
                    AGC.Transport().request("POST", url, {}, b"{}")
            finally:
                server.shutdown()
                thread.join()


if __name__ == "__main__":
    unittest.main()

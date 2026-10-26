#!/usr/bin/env python3
"""把已签名 APP 上传到华为测试专区，可选绑定公开测试版本并提交审核。"""

from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import sys
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


BASE = "https://connect-api.cloud.huawei.com/api"
DAY = 86400000


class ReleaseError(Exception):
    pass


def identifier(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", value):
        raise ReleaseError(f"{label} 无效")
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # 不把认证头转给重定向目标，也不自动重复有副作用的请求。
        return None


class Transport:
    def __init__(self):
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, method, url, headers, data=None, *, binary=False):
        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=120 if binary else 60) as response:
                if binary:
                    return None
                result = json.loads(response.read(4 * 1024 * 1024))
        except urllib.error.HTTPError as error:
            error.close()
            raise ReleaseError(f"华为请求失败（HTTP {error.code}）；请在控制台核对权限和资料") from None
        except (OSError, ValueError, urllib.error.URLError, http.client.HTTPException):
            # 原始响应、URL 和异常可能携带 token、签名地址或联系人资料。
            raise ReleaseError("华为请求或响应解析失败；未自动重试，请核对控制台状态") from None
        if not isinstance(result, dict):
            raise ReleaseError("华为响应不是 JSON 对象")
        return result


def check_result(result, *, token=False):
    ret = result.get("ret")
    if token and ret is None and isinstance(result.get("access_token"), str) and result["access_token"]:
        return result
    if isinstance(ret, str):
        try:
            ret = json.loads(ret)
        except ValueError:
            ret = None
    code = ret.get("code") if isinstance(ret, dict) else None
    if type(code) not in (int, str) or str(code) != "0":
        safe_code = str(code) if re.fullmatch(r"[0-9-]{1,16}", str(code)) else "unknown"
        raise ReleaseError(f"华为拒绝请求（code={safe_code}）；请在控制台核对权限和资料")
    return result


def service_account_headers(raw):
    try:
        account = json.loads(raw)
        key_id = identifier(account["key_id"], "服务账号 key_id")
        sub_account = identifier(account["sub_account"], "服务账号 sub_account")
        private_key = account["private_key"]
        if not isinstance(private_key, str) or not private_key.startswith("-----BEGIN PRIVATE KEY-----"):
            raise ValueError()
    except (ValueError, KeyError, TypeError):
        raise ReleaseError("HARMONY_AGC_SERVICE_ACCOUNT_JSON 格式无效") from None

    def encode(value):
        return base64.urlsafe_b64encode(json.dumps(value, separators=(",", ":")).encode()).rstrip(b"=")

    now = int(time.time())
    content = encode({"kid": key_id, "typ": "JWT", "alg": "PS256"}) + b"." + encode({
        "aud": "https://oauth-login.cloud.huawei.com/oauth2/v3/token",
        "iss": sub_account, "iat": now, "exp": now + 3600,
    })
    # 密钥只在私有临时目录内落盘；OpenSSL 的输出与异常不进入发布日志。
    with tempfile.TemporaryDirectory(prefix="msime-agc-") as temporary:
        key_path = Path(temporary) / "private.pem"
        descriptor = os.open(key_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w") as key_file:
            key_file.write(private_key)
        try:
            signed = subprocess.run([
                "openssl", "dgst", "-sha256", "-sign", str(key_path),
                "-sigopt", "rsa_padding_mode:pss", "-sigopt", "rsa_pss_saltlen:32",
                "-sigopt", "rsa_mgf1_md:sha256",
            ], input=content, capture_output=True, timeout=30, check=True).stdout
        except (OSError, subprocess.SubprocessError):
            raise ReleaseError("服务账号 JWT 签名失败，请检查私钥与 OpenSSL") from None
    token = (content + b"." + base64.urlsafe_b64encode(signed).rstrip(b"=")).decode()
    # 华为服务账号直接使用 JWT，不把它发送到 OAuth token 换取接口。
    return {"Authorization": "Bearer " + token, "Content-Type": "application/json"}


class Client:
    def __init__(self, app_id, client_id="", secret="", transport=None, *, service_account=""):
        self.app_id = identifier(app_id, "HARMONY_AGC_APP_ID")
        self.transport = transport or Transport()
        if service_account:
            self.headers = service_account_headers(service_account)
            return
        if not client_id or not secret:
            raise ReleaseError("缺少 HARMONY_AGC_SERVICE_ACCOUNT_JSON，或兼容的 HARMONY_AGC_CLIENT_ID / HARMONY_AGC_CLIENT_SECRET")
        result = check_result(self.transport.request(
            "POST", BASE + "/oauth2/v1/token", {"Content-Type": "application/json"},
            json.dumps({"grant_type": "client_credentials", "client_id": client_id,
                        "client_secret": secret}).encode()), token=True)
        token = result.get("access_token")
        if not isinstance(token, str) or not token or "\n" in token or "\r" in token:
            raise ReleaseError("华为没有返回有效的 access_token")
        self.headers = {"client_id": client_id, "Authorization": "Bearer " + token,
                        "Content-Type": "application/json"}

    def api(self, method, path, body=None, **query):
        query = {"appId": self.app_id, **query}
        return check_result(self.transport.request(
            method, BASE + path + "?" + urllib.parse.urlencode(query), self.headers,
            None if body is None else json.dumps(body).encode()))

    def check_access(self):
        # 此只读列表允许尚无测试版本的应用；app-info 的测试查询必须提供 versionId。
        check_result(self.transport.request(
            "POST", BASE + "/publish/v3/version/brief-info/list",
            {**self.headers, "appId": self.app_id},
            json.dumps({"packageName": "app.msime.hmos"}).encode()))

    def upload(self, path):
        size = path.stat().st_size
        if path.suffix != ".app" or not 0 < size <= 4 * 1024**3:
            raise ReleaseError("必须提供非空且不超过 4GB 的已签名 .app")
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
        result = self.api("GET", "/publish/v2/upload-url/for-obs", fileName=path.name,
                          contentLength=size, sha256=digest.hexdigest(), chineseMainlandFlag=1)
        info = result.get("urlInfo")
        if not isinstance(info, dict):
            raise ReleaseError("华为没有返回上传地址")
        url = info.get("url", "")
        parsed = urllib.parse.urlsplit(url)
        if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
            raise ReleaseError("华为返回的上传地址无效")
        headers = info.get("headers")
        object_id = info.get("objectId")
        if (info.get("method") != "PUT" or not isinstance(headers, dict)
                or not isinstance(object_id, str) or not object_id):
            raise ReleaseError("华为返回的上传参数不完整")
        # OBS 使用华为返回的独立认证头；流式发送，禁止附加 AGC Bearer。
        headers = {key: value for key, value in headers.items() if key.lower() != "content-length"}
        headers["Content-Length"] = str(size)
        with path.open("rb") as source:
            self.transport.request("PUT", url, headers, source, binary=True)
        packages = self.api("POST", "/publish/v2/test/version/pkg", {
            "distributeMode": 1, "file": {"fileName": path.name, "objectId": object_id},
        }).get("pkgVersion")
        if not isinstance(packages, list) or len(packages) != 1:
            raise ReleaseError("华为没有返回唯一的软件包 ID")
        return identifier(packages[0], "软件包 ID")

    def wait_package(self, package_id, timeout, interval=15):
        deadline = time.monotonic() + timeout
        while True:
            states = self.api("GET", "/publish/v3/package/compile/status", pkgIds=package_id).get("pkgStateList")
            if not isinstance(states, list):
                raise ReleaseError("华为没有返回软件包状态")
            matches = [item for item in states if isinstance(item, dict) and item.get("pkgId") == package_id]
            status = matches[0].get("successStatus") if len(matches) == 1 else None
            if type(status) is not int or status not in (0, 1, 2):
                raise ReleaseError("软件包状态未知，停止后续提交")
            if status == 0:
                return
            if status == 2:
                raise ReleaseError("华为软件包解析失败，停止后续提交")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ReleaseError("等待华为软件包解析超时，可使用软件包 ID 继续")
            time.sleep(min(interval, remaining))


def test_config(version, raw, now=None):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ReleaseError("版本号须为 X.Y.Z")
    now = int(time.time() * 1000) if now is None else now
    try:
        config = json.loads(raw or "{}")
    except ValueError:
        raise ReleaseError("HARMONY_AGC_TEST_CONFIG_JSON 不是有效 JSON") from None
    allowed = {"languages", "deviceTypes", "distCountryList", "extInfo", "openTestInfo",
               "packagePermissionIntroVideoList", "versionVocReq"}
    if not isinstance(config, dict) or config.keys() - allowed:
        raise ReleaseError("测试配置含不支持的顶层字段")
    info = config.setdefault("openTestInfo", {})
    if not isinstance(info, dict):
        raise ReleaseError("openTestInfo 必须是对象")
    info.setdefault("startTime", now + DAY)
    info.setdefault("endTime", info["startTime"] + 28 * DAY if type(info["startTime"]) is int else 0)
    info.setdefault("testDesc", f"{version} Beta 公开测试")
    start, end = info["startTime"], info["endTime"]
    if (type(start) is not int or type(end) is not int or start <= now
            or not 0 < end - start <= 30 * DAY):
        raise ReleaseError("公开测试开始时间须在未来，持续时间须大于 0 且不超过 30 天")
    if not isinstance(info["testDesc"], str) or not 1 <= len(info["testDesc"]) <= 50:
        raise ReleaseError("公开测试描述须为 1 至 50 字")
    task = info.setdefault("testTaskInfo", {})
    if not isinstance(task, dict) or "groupInfos" in task:
        raise ReleaseError("公开测试不能修改 groupInfos")
    for key, value in {"needShareLink": 1, "displayArea": "1", "needNotify": 0,
                       "publicTestInstallLimit": 10000}.items():
        task.setdefault(key, value)
    return config


def record(key, value):
    value = identifier(value, key)
    print(f"{key}={value}", flush=True)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"{key}={value}\n")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
            summary.write(f"- `{key}`: `{value}`\n")


def release(client, *, path, mode, version_id="", package_id="", config=None, timeout=900):
    if version_id:
        identifier(version_id, "测试版本 ID")
        if mode != "public-test":
            raise ReleaseError("测试版本 ID 仅用于 public-test 模式")
    if package_id:
        identifier(package_id, "软件包 ID")
    if mode == "public-test" and config is None:
        raise ReleaseError("缺少已验证的公开测试配置")
    if version_id:
        record("test_version_id", version_id)
    package_id = package_id or client.upload(path)
    record("package_id", package_id)
    client.wait_package(package_id, timeout)
    record("package_status", "parsed")
    if mode == "upload":
        return
    if not version_id:
        result = client.api("POST", "/publish/v2/test/app/version", {
            "releaseType": 6, "testType": 4, "testDesc": config["openTestInfo"]["testDesc"],
        })
        version_id = identifier(result.get("versionId"), "测试版本 ID")
        record("test_version_id", version_id)
    client.api("PUT", "/publish/v2/test/app/version", {**config, "versionId": version_id, "pkgId": package_id})
    client.api("POST", "/publish/v2/test/app/version/submit", {"versionId": version_id})
    record("review_status", "submitted")
    print("公开测试已提交，等待华为审核；提交成功不代表已上架。", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--input", type=Path)
    parser.add_argument("--mode", choices=("upload", "public-test"), default="upload")
    parser.add_argument("--version", default="")
    parser.add_argument("--test-version-id", default="")
    parser.add_argument("--package-id", default="")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    try:
        if not 1 <= args.timeout <= 1800:
            raise ReleaseError("轮询超时须为 1 至 1800 秒")
        if args.test_version_id:
            identifier(args.test_version_id, "测试版本 ID")
            if args.mode != "public-test":
                raise ReleaseError("测试版本 ID 仅用于 public-test 模式")
        if args.package_id:
            identifier(args.package_id, "软件包 ID")
        config = test_config(args.version, os.environ.get("HARMONY_AGC_TEST_CONFIG_JSON", "")) if args.mode == "public-test" else None
        if not args.check_only and not args.package_id and not args.input:
            raise ReleaseError("必须提供 --input 或 --package-id")
        client = Client(os.environ.get("HARMONY_AGC_APP_ID", ""),
                        os.environ.get("HARMONY_AGC_CLIENT_ID", ""),
                        os.environ.get("HARMONY_AGC_CLIENT_SECRET", ""),
                        service_account=os.environ.get("HARMONY_AGC_SERVICE_ACCOUNT_JSON", ""))
        if args.check_only:
            # JWT 本地签名成功不等于平台认可，预检还要验证应用读取权限。
            client.check_access()
            print("华为 API 鉴权与本地配置检查通过")
            return 0
        release(client, path=args.input, mode=args.mode,
                version_id=args.test_version_id, package_id=args.package_id,
                config=config, timeout=args.timeout)
        return 0
    except (ReleaseError, OSError, ValueError, TypeError):
        # 仅输出本脚本定义的安全错误；系统异常不携带原始敏感上下文。
        error = sys.exc_info()[1]
        print(str(error) if isinstance(error, ReleaseError) else "本地文件或华为响应无效，已停止", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""使用华为发布证书签名 HAP，并生成应用市场提交包。"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import zipfile


BUNDLE = "app.msime.hmos"
DATA_GROUP = "group.1516753898738609107"


def run(command: list[str], *, stage: str | None = None, **kwargs) -> bytes:
    # 签名工具的输出可能带命令参数，失败时也不将凭据写进日志。
    result = subprocess.run(command, capture_output=True, **kwargs)
    if result.returncode:
        detail = b"\n".join(part for part in (result.stdout, result.stderr) if part).decode(
            "utf-8", errors="replace"
        ).strip()
        for secret_name in (
            "HARMONY_RELEASE_KEYSTORE_PASSWORD",
            "HARMONY_RELEASE_KEY_PASSWORD",
        ):
            secret = os.environ.get(secret_name)
            if secret:
                detail = detail.replace(secret, "<redacted>")
        for argument in command:
            if argument:
                detail = detail.replace(str(argument), "<redacted>")
        suffix = f"：{detail}" if detail else ""
        raise ValueError(
            f"{stage or Path(command[0]).name} 执行失败（退出码 {result.returncode}）{suffix}"
        )
    return result.stdout


def certificate_der(pem: bytes) -> bytes:
    return run(["openssl", "x509", "-outform", "DER"], input=pem)


def public_key(pem: bytes) -> bytes:
    return run(["openssl", "x509", "-pubkey", "-noout"], input=pem)


def validate_profile(profile: dict, certificate: bytes, now: float | None = None) -> None:
    now = time.time() if now is None else now
    if profile.get("type") != "release" or profile.get("app-distribution-type") != "app_gallery":
        raise ValueError("必须使用华为应用市场类型的 release Profile")
    validity = profile.get("validity", {})
    if not validity.get("not-before", now + 1) <= now < validity.get("not-after", 0):
        raise ValueError("发布 Profile 尚未生效或已经过期")
    bundle = profile.get("bundle-info", {})
    if bundle.get("bundle-name") != BUNDLE:
        raise ValueError("发布 Profile 包名不匹配")
    if DATA_GROUP not in bundle.get("data-group-ids", []):
        raise ValueError("发布 Profile 缺少输入法共享沙箱授权")
    if profile.get("debug-info", {}).get("device-ids"):
        raise ValueError("应用市场 Profile 不应限制测试设备")
    if "ohos.permission.READ_PASTEBOARD" not in profile.get("acls", {}).get("allowed-acls", []):
        raise ValueError("发布 Profile 缺少剪贴板受限权限授权")
    leaf = bundle.get("distribution-certificate", "").encode()
    if not leaf:
        raise ValueError("发布 Profile 缺少发布证书")
    chain = re.findall(rb"-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----", certificate, re.S)
    if certificate_der(leaf) not in [certificate_der(item) for item in chain]:
        raise ValueError("发布证书链与 Profile 中的证书不匹配")
    run(["openssl", "x509", "-checkend", "0", "-noout"], input=leaf)


def validate_hap(path: Path, version: str) -> bytes:
    with zipfile.ZipFile(path) as package:
        if package.testzip():
            raise ValueError("HAP 内容损坏")
        names = set(package.namelist())
        module = json.loads(package.read("module.json"))
        app = module["app"]
        if app.get("debug") is not False:
            raise ValueError("正式发布必须使用 release 构建，不能保留调试标记")
        major, minor, patch = map(int, version.split("."))
        if (app["bundleName"], app["versionName"], app["versionCode"]) != (
            BUNDLE, version, major * 1_000_000 + minor * 1_000 + patch
        ):
            raise ValueError("HAP 包名或版本不匹配")
        required = {
            "libs/arm64-v8a/libmsime_host_api.so",
            "libs/arm64-v8a/libmsimeclient.so",
            "resources/rawfile/settings/index.html",
            "resources/resfile/handwriting/handwriting-zh_CN.model",
            "resources/resfile/handwriting/HandwritingModel-LICENSE.txt",
            "resources/resfile/handwriting/Zinnia-LICENSE.txt",
        }
        if not required <= names:
            raise ValueError("HAP 缺少原生库、设置页或手写资源")
        for prefix in ["resources/resfile/engine/", "resources/resfile/licenses/", "resources/resfile/wordbooks/"]:
            if not any(n.startswith(prefix) and not n.endswith("/") for n in names):
                raise ValueError(f"HAP 缺少资源：{prefix}")
        return package.read("pack.info")


def sign(args: argparse.Namespace) -> None:
    with tempfile.TemporaryDirectory(prefix="msime-harmony-signing-") as temporary:
        directory = Path(temporary)
        for key, filename in [("KEYSTORE", "release.p12"), ("PROFILE", "release.p7b"), ("CERTIFICATE", "release.cer")]:
            value = os.environ.get(f"HARMONY_RELEASE_{key}_BASE64", "")
            if not value:
                raise ValueError(f"缺少 HARMONY_RELEASE_{key}_BASE64")
            path = directory / filename
            path.write_bytes(base64.b64decode(value, validate=True))
            path.chmod(0o600)
        profile_path = directory / "release.p7b"
        certificate = directory / "release.cer"
        payload = run(["openssl", "cms", "-verify", "-inform", "DER", "-in", str(profile_path), "-noverify"])
        profile = json.loads(payload)
        validate_profile(profile, certificate.read_bytes())
        if not os.environ.get("HARMONY_RELEASE_KEYSTORE_PASSWORD"):
            raise ValueError("缺少 HARMONY_RELEASE_KEYSTORE_PASSWORD")
        alias = os.environ.get("HARMONY_RELEASE_KEY_ALIAS", "msime")
        exported = run(["keytool", "-exportcert", "-rfc", "-keystore", str(directory / "release.p12"),
                        "-storepass:env", "HARMONY_RELEASE_KEYSTORE_PASSWORD", "-alias", alias])
        if public_key(exported) != public_key(profile["bundle-info"]["distribution-certificate"].encode()):
            raise ValueError("密钥库与发布证书不匹配")
        if args.check_only:
            print("发布证书、密钥库、Profile、共享沙箱与 ACL 授权检查通过")
            return
        if args.input is None or args.output_dir is None or args.tool_dir is None or args.version is None:
            raise ValueError("签名需要 --input、--output-dir、--tool-dir 和 --version")
        pack_info = validate_hap(args.input, args.version)
        tool = ["java", "-jar", str(args.tool_dir / "hap-sign-tool.jar")]
        signed = directory / "entry-default.hap"
        password = os.environ["HARMONY_RELEASE_KEYSTORE_PASSWORD"]
        run(tool + ["sign-app", "-mode", "localSign", "-keyAlias", alias,
                    "-keystoreFile", str(directory / "release.p12"), "-keystorePwd", password,
                    "-keyPwd", os.environ.get("HARMONY_RELEASE_KEY_PASSWORD", password),
                    "-profileFile", str(profile_path), "-appCertFile", str(certificate),
                    "-inFile", str(args.input), "-outFile", str(signed),
                    "-signAlg", "SHA256withECDSA", "-compatibleVersion", "21"], stage="签名 HAP")
        extracted_profile = directory / "verified.p7b"
        run(tool + ["verify-app", "-inFile", str(signed), "-outCertChain", str(directory / "verified.cer"),
                    "-outProfile", str(extracted_profile)], stage="验证 HAP 签名")
        if extracted_profile.read_bytes() != profile_path.read_bytes():
            raise ValueError("签名包中的 Profile 与提交的 Profile 不一致")
        validate_hap(signed, args.version)
        with zipfile.ZipFile(args.input) as original, zipfile.ZipFile(signed) as result:
            if not set(original.namelist()) <= set(result.namelist()) or any(
                original.read(name) != result.read(name) for name in original.namelist()
            ):
                raise ValueError("签名改变了原生库、代码或资源内容")
            if ".pages.info" not in result.namelist():
                raise ValueError("签名包缺少代码完整性签名")
        (directory / "pack.info").write_bytes(pack_info)
        market_app = directory / f"msime-harmony-{args.version}.app"
        run(["java", "-jar", str(args.tool_dir / "app_packing_tool.jar"), "--mode", "app",
             "--hap-path", str(signed), "--pack-info-path", str(directory / "pack.info"),
             "--out-path", str(market_app), "--replace-pack-info", "false", "--force", "true"],
            stage="打包应用市场 APP")
        with zipfile.ZipFile(market_app) as archive:
            haps = [name for name in archive.namelist() if name.endswith(".hap")]
            if len(haps) != 1 or archive.read(haps[0]) != signed.read_bytes():
                raise ValueError("应用市场提交包中的 HAP 不匹配")
        args.output_dir.mkdir(parents=True, exist_ok=True)
        hap = args.output_dir / f"msime-harmony-{args.version}.hap"
        shutil.copyfile(signed, hap)
        shutil.copyfile(market_app, args.output_dir / market_app.name)
        digest = hashlib.sha256()
        with hap.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        (args.output_dir / "SHA256SUMS").write_text(f"{digest.hexdigest()}  {hap.name}\n", encoding="utf-8")
        print("发布签名和内容验证通过，已生成 HAP、应用市场 APP 包及 SHA256SUMS")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--input", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--tool-dir", type=Path)
    parser.add_argument("--version")
    args = parser.parse_args()
    try:
        sign(args)
    except (ValueError, KeyError, OSError, zipfile.BadZipFile) as error:
        parser.exit(1, f"鸿蒙发布签名失败：{error}\n")


if __name__ == "__main__":
    main()

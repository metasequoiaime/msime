#!/usr/bin/env python3
"""从已签名的内嵌输入法配置完整设置应用的 Sparkle 更新通道。"""

from __future__ import annotations

import argparse
import base64
import plistlib
import shutil
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit

UPDATE_KEYS = (
    "SUFeedURL", "SUPublicEDKey", "SURequireSignedFeed", "SUVerifyUpdateBeforeExtraction",
    "SUEnableAutomaticChecks", "SUAutomaticallyUpdate", "SUScheduledCheckInterval",
)


def configure(app: Path, input_method: Path, edition: str) -> bool:
    metadata = app / "Contents/Info.plist"
    native = plistlib.loads((input_method / "Contents/Info.plist").read_bytes())
    settings = plistlib.loads(metadata.read_bytes())
    build = str(native["CFBundleVersion"])
    if not build.isdecimal() or int(build) <= 0:
        raise ValueError("输入法构建号必须是正整数")
    settings["CFBundleVersion"] = build
    for key in UPDATE_KEYS:
        settings.pop(key, None)
    enabled = bool(native.get("SUFeedURL"))
    if enabled:
        feed = urlsplit(native["SUFeedURL"])
        expected = f"appcast-{edition}.xml"
        if feed.scheme != "https" or not feed.netloc or not feed.path.endswith('/' + expected) or feed.query or feed.fragment:
            raise ValueError("输入法更新地址不属于当前版本")
        key = native.get("SUPublicEDKey", "")
        if len(base64.b64decode(key, validate=True)) != 32:
            raise ValueError("缺少有效的 Sparkle 公钥")
        for flag in ("SURequireSignedFeed", "SUVerifyUpdateBeforeExtraction"):
            if native.get(flag) is not True:
                raise ValueError("完整应用更新必须校验 feed 和解压前的归档签名")
        framework = input_method / "Contents/Frameworks/Sparkle.framework"
        destination = app / "Contents/Frameworks/Sparkle.framework"
        if not framework.is_dir():
            raise ValueError("输入法中缺少 Sparkle.framework")
        if destination.exists():
            shutil.rmtree(destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(framework, destination, symlinks=True)
        settings.update({key: native[key] for key in UPDATE_KEYS if key in native})
        settings["SUFeedURL"] = urlunsplit(feed._replace(path=feed.path[:-len(expected)] + f"appcast-settings-{edition}.xml"))
        # 只在用户主动检查时启动设置应用的更新器，输入法仍保持原来的后台更新策略。
        settings["SUEnableAutomaticChecks"] = False
        settings["SUAutomaticallyUpdate"] = False
    metadata.write_bytes(plistlib.dumps(settings, sort_keys=False))
    return enabled


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--input-method", type=Path, required=True)
    parser.add_argument("--edition", required=True)
    args = parser.parse_args()
    enabled = configure(args.app, args.input_method, args.edition)
    print("settings updater: enabled" if enabled else "settings updater: disabled (no feed)")


if __name__ == "__main__":
    main()

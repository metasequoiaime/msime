#!/usr/bin/env python3
"""按发布版本渲染发行版源码包的版本相关文件：RPM 规格文件（msime.spec）和 Debian 的 debian/changelog。

两者的源码地址都由版本号推出（发布页 linux-vVERSION 下的 msime-VERSION.tar.xz 与 msime-VERSION-vendor.tar.xz），校验和在 .src.rpm 和 .dsc 生成时由 rpmbuild、dpkg-source 自己算出，所以这里只改版本、发布号和更新日志，不抄写任何哈希。

  render-sources.py --version 0.9.1 --spec-out OUT.spec --changelog-out OUT/changelog
      [--rpm-release 1] [--debian-revision 1] [--debian-distribution unstable] [--date 2026-10-05]

--debian-distribution 对 PPA 要写 Ubuntu 的代号（例如 resolute）；同一版本传给多个代号时，--debian-revision 用 1~ppa1~ubuntu26.04 这类后缀区分，Launchpad 不接受同名同版本的两次上传。
"""
from __future__ import annotations

import argparse
import datetime as dt
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = HERE / "msime.spec"
MAINTAINER = "Metasequoia IME <metasequoiaime@gmail.com>"


def render_spec(text: str, version: str, release: str, date: dt.date) -> str:
    text, count = re.subn(r"(?m)^Version:(\s+)\S+$", rf"Version:\g<1>{version}", text)
    if count != 1:
        raise SystemExit("msime.spec: expected exactly one Version: line")
    text, count = re.subn(r"(?m)^Release:(\s+)\S+$", rf"Release:\g<1>{release}%{{?dist}}", text)
    if count != 1:
        raise SystemExit("msime.spec: expected exactly one Release: line")
    head, marker, _ = text.partition("\n%changelog\n")
    if not marker:
        raise SystemExit("msime.spec: no %changelog section")
    entry = f"* {date.strftime('%a %b %d %Y')} {MAINTAINER} - {version}-{release}\n- Release {version}\n"
    return f"{head}{marker}{entry}"


def render_changelog(version: str, revision: str, distribution: str, date: dt.date) -> str:
    stamp = dt.datetime.combine(date, dt.time(), tzinfo=dt.timezone.utc).strftime("%a, %d %b %Y %H:%M:%S +0000")
    return f"msime ({version}-{revision}) {distribution}; urgency=medium\n\n  * Release {version}.\n\n -- {MAINTAINER}  {stamp}\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--spec-out", type=Path)
    parser.add_argument("--changelog-out", type=Path)
    parser.add_argument("--rpm-release", default="1")
    parser.add_argument("--debian-revision", default="1")
    parser.add_argument("--debian-distribution", default="unstable")
    parser.add_argument("--date", type=dt.date.fromisoformat, default=dt.datetime.now(dt.timezone.utc).date())
    arguments = parser.parse_args()
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", arguments.version):
        raise SystemExit(f"version must be MAJOR.MINOR.PATCH: {arguments.version}")
    if not re.fullmatch(r"[0-9][0-9A-Za-z._+~]*", arguments.rpm_release):
        raise SystemExit(f"invalid RPM release: {arguments.rpm_release}")
    if not re.fullmatch(r"[0-9A-Za-z.+~]+", arguments.debian_revision):
        raise SystemExit(f"invalid Debian revision: {arguments.debian_revision}")
    if not re.fullmatch(r"[a-z0-9-]+", arguments.debian_distribution):
        raise SystemExit(f"invalid Debian distribution: {arguments.debian_distribution}")
    if not arguments.spec_out and not arguments.changelog_out:
        parser.error("nothing to render: pass --spec-out and/or --changelog-out")
    if arguments.spec_out:
        rendered = render_spec(SPEC.read_text(encoding="utf-8"), arguments.version, arguments.rpm_release, arguments.date)
        arguments.spec_out.write_text(rendered, encoding="utf-8")
    if arguments.changelog_out:
        arguments.changelog_out.write_text(
            render_changelog(arguments.version, arguments.debian_revision, arguments.debian_distribution, arguments.date),
            encoding="utf-8")


if __name__ == "__main__":
    main()

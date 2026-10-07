#!/usr/bin/env python3
"""把一个 Windows 发布填进 winget、Scoop 与 Chocolatey 的包定义模板。

本脚本旁边的模板保存发布之间不变的事实（ProductCode、静默参数、依赖、说明）；每个发布只提供版本号、安装包地址、安装包的 SHA-256、发布日期和发布说明地址，由本脚本填进 @...@ 字段。它们有两种来源：

    render.py --latest --output DIR                      GitHub 上最新一个已发布、非预发布的 windows-v* 发布
    render.py --tag windows-v0.1.0 --output DIR          GitHub 上的指定发布
    render.py --version 0.1.0 --installer FILE --output DIR

走 GitHub 时，GitHub 为每个附件记录的摘要与发布流程放在安装包旁边的 `<安装包>.sha256` 小文件互相核对，再下载安装包本身核对同一个摘要。`--installer` 改为对本地文件求摘要，地址按发布流程发布它的位置推出。

两种来源都要求安装包带有效的 Authenticode 签名，没有就拒绝渲染。release-windows.yml 在 CI 上打出、先行发布的安装包是未签名的：x64 Server 以 uiAccess=true 构建，未签名时系统拒绝启动它，装上之后只能打英文（installer/Sign-InstalledServer-Local.ps1）。能交给包管理器的只有维护者用 installer/Package-SimplySign.ps1 签名后替换上去的那一份，而替换会改变摘要，所以也只能在替换之后渲染。签名用 Windows 自己的 Get-AuthenticodeSignature 核对（状态必须是 Valid），所以本脚本只能在 Windows 上渲染；其他系统上一律失败，改为手动触发 package-definitions-windows.yml。

输出目录按三个包管理器各自的布局：

    winget/manifests/m/Metasequoia/MetasequoiaIME/<version>/*.yaml
    scoop/msime.json
    chocolatey/msime/msime.nuspec, chocolatey/msime/tools/*.ps1

只用 Python 标准库。设置 GH_TOKEN 或 GITHUB_TOKEN 可以提高 GitHub API 限额。
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
import xml.etree.ElementTree as ElementTree

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_REPO = "metasequoiaime/msime"
TAG_PREFIX = "windows-v"
WINGET_ID = "Metasequoia.MetasequoiaIME"
# release-windows.yml 只接受三段数字的版本号，因为 Build-Client.ps1 要把它写进 Tauri 与 Server 的元数据。
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+$")
SHA256_PATTERN = re.compile(r"^[0-9a-fA-F]{64}$")
PLACEHOLDER_PATTERN = re.compile(r"@[A-Z0-9_]+@")
# 摘要小文件的内容是「<hex>  <name>\n」，比这大得多的不是它。
SIDECAR_LIMIT = 4096
API_PAGES = 10


class RenderError(Exception):
    pass


def installer_name(version: str) -> str:
    # msime_setup.iss 的 OutputBaseFilename 是 {#MyEditionInstallerBaseName}_v{#MyAppVersion}，full 的前缀是 MetasequoiaIME-Full_Setup（版本表的 installer_base_name）；release-windows.yml 以这个名字发布。不带版本名的 MetasequoiaIME_Setup 是 msime-windows 的安装包。
    return f"MetasequoiaIME-Full_Setup_v{version}.exe"


def release_download_url(repo: str, version: str) -> str:
    return f"https://github.com/{repo}/releases/download/{TAG_PREFIX}{version}/{installer_name(version)}"


def release_page_url(repo: str, version: str) -> str:
    return f"https://github.com/{repo}/releases/tag/{TAG_PREFIX}{version}"


def check_version(version: str) -> str:
    if not VERSION_PATTERN.match(version):
        raise RenderError(f"version {version!r} is not MAJOR.MINOR.PATCH")
    return version


def check_sha256(value: str) -> str:
    if not SHA256_PATTERN.match(value):
        raise RenderError(f"{value!r} is not a SHA-256 digest")
    return value.lower()


def check_date(value: str) -> str:
    try:
        datetime.date.fromisoformat(value)
    except ValueError as error:
        raise RenderError(f"release date {value!r} is not YYYY-MM-DD") from error
    return value


def values_for(repo: str, version: str, sha256: str, release_date: str, installer_url: str | None = None, notes_url: str | None = None) -> dict[str, str]:
    version = check_version(version)
    sha256 = check_sha256(sha256)
    return {
        "VERSION": version,
        "INSTALLER_URL": installer_url or release_download_url(repo, version),
        "SHA256": sha256,
        "SHA256_UPPER": sha256.upper(),
        "RELEASE_DATE": check_date(release_date),
        "RELEASE_NOTES_URL": notes_url or release_page_url(repo, version),
    }


# ---- GitHub ----


def github_request(url: str, accept: str = "application/vnd.github+json") -> bytes:
    headers = {"Accept": accept, "User-Agent": "msime-windows-packaging-render"}
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token and url.startswith("https://api.github.com/"):
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        raise RenderError(f"GET {url}: HTTP {error.code}") from error


def download(url: str, target: pathlib.Path) -> str:
    """把 url 下载到 target，返回内容的 SHA-256。发布附件的地址会重定向到 GitHub 的对象存储，urllib 自己跟随。"""
    request = urllib.request.Request(url, headers={"Accept": "application/octet-stream", "User-Agent": "msime-windows-packaging-render"})
    digest = hashlib.sha256()
    try:
        with urllib.request.urlopen(request, timeout=300) as response, target.open("wb") as stream:
            for block in iter(lambda: response.read(1 << 20), b""):
                digest.update(block)
                stream.write(block)
    except urllib.error.HTTPError as error:
        raise RenderError(f"GET {url}: HTTP {error.code}") from error
    return digest.hexdigest()


def github_json(url: str):
    return json.loads(github_request(url))


def latest_release(repo: str) -> dict:
    """版本号最高的、已发布且非预发布的 windows-v 发布。"""
    found: list[tuple[tuple[int, ...], dict]] = []
    for page in range(1, API_PAGES + 1):
        releases = github_json(f"https://api.github.com/repos/{repo}/releases?per_page=100&page={page}")
        for release in releases:
            tag = release.get("tag_name", "")
            version = tag[len(TAG_PREFIX):]
            if tag.startswith(TAG_PREFIX) and VERSION_PATTERN.match(version) and not release.get("draft") and not release.get("prerelease"):
                found.append((tuple(int(part) for part in version.split(".")), release))
        if len(releases) < 100:
            break
    if not found:
        raise RenderError(f"{repo} has no published, non-prerelease {TAG_PREFIX}* release")
    return max(found, key=lambda item: item[0])[1]


def tagged_release(repo: str, tag: str) -> dict:
    return github_json(f"https://api.github.com/repos/{repo}/releases/tags/{tag}")


def parse_sidecar(text: str, name: str) -> str:
    fields = text.split()
    if not fields:
        raise RenderError(f"{name}.sha256 is empty")
    digest = check_sha256(fields[0])
    if len(fields) > 1 and fields[1].lstrip("*") != name:
        raise RenderError(f"{name}.sha256 names {fields[1]!r}, not {name}")
    return digest


def values_from_release(repo: str, release: dict, allow_prerelease: bool) -> dict[str, str]:
    tag = release.get("tag_name", "")
    if not tag.startswith(TAG_PREFIX):
        raise RenderError(f"{tag!r} is not a {TAG_PREFIX}* release")
    version = check_version(tag[len(TAG_PREFIX):])
    if release.get("draft"):
        raise RenderError(f"{tag} is a draft")
    if release.get("prerelease") and not allow_prerelease:
        raise RenderError(f"{tag} is a prerelease; package managers should not be offered it (pass --allow-prerelease to render it anyway)")
    assets = {asset["name"]: asset for asset in release.get("assets", [])}
    name = installer_name(version)
    installer = assets.get(name)
    if installer is None:
        raise RenderError(f"{tag} has no {name} asset")

    digests: dict[str, str] = {}
    recorded = installer.get("digest") or ""
    if recorded.startswith("sha256:"):
        digests["GitHub asset digest"] = check_sha256(recorded[len("sha256:"):])
    sidecar = assets.get(f"{name}.sha256")
    if sidecar is not None:
        if sidecar.get("size", 0) > SIDECAR_LIMIT:
            raise RenderError(f"{name}.sha256 is {sidecar['size']} bytes, too large for a digest file")
        text = github_request(sidecar["browser_download_url"], accept="application/octet-stream").decode("utf-8")
        digests[f"{name}.sha256"] = parse_sidecar(text, name)
    if not digests:
        raise RenderError(f"{tag}: neither a GitHub digest nor {name}.sha256 gives the installer's SHA-256")
    if len(set(digests.values())) != 1:
        raise RenderError(f"{tag}: the installer digests disagree: {digests}")

    digest = next(iter(digests.values()))
    # 摘要只说明元数据彼此一致；签名要看文件本身，所以把安装包取下来，先核对它就是摘要说的那一份，再核对签名。
    with tempfile.TemporaryDirectory(prefix="msime-render-") as scratch:
        local = pathlib.Path(scratch) / name
        downloaded = download(installer["browser_download_url"], local)
        if downloaded != digest:
            raise RenderError(f"{tag}: the downloaded {name} has SHA-256 {downloaded}, not the published {digest}")
        verify_signature(local)

    published = release.get("published_at") or release.get("created_at") or ""
    return values_for(
        repo,
        version,
        digest,
        published[:10],
        installer_url=installer["browser_download_url"],
        notes_url=release.get("html_url") or release_page_url(repo, version),
    )


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_signature(path: pathlib.Path) -> None:
    """安装包没有有效的 Authenticode 签名时抛出 RenderError。

    发布页上先出现的是 CI 打的未签名安装包，它的 uiAccess Server 起不来，交给包管理器的用户装上之后只能打英文；签名后的安装包替换它之后才能渲染。这里只看签名是否有效、链到受信任的根，不认具体的证书：证书换代时不需要改这里。
    """
    check_can_verify()
    shell = shutil.which("pwsh") or shutil.which("powershell")
    if shell is None:
        raise RenderError("neither pwsh nor powershell is available to check the installer's Authenticode signature")
    script = "$s = Get-AuthenticodeSignature -LiteralPath $env:MSIME_INSTALLER; Write-Output $s.Status; Write-Output $s.StatusMessage; Write-Output $s.SignerCertificate.Subject"
    result = subprocess.run(
        [shell, "-NoProfile", "-NonInteractive", "-Command", script],
        env={**os.environ, "MSIME_INSTALLER": str(path)}, capture_output=True, text=True, check=False,
    )
    lines = [line.strip() for line in result.stdout.splitlines()]
    if result.returncode != 0 or not lines or lines[0] != "Valid":
        detail = " / ".join(line for line in lines if line) or result.stderr.strip()
        raise RenderError(f"{path.name} has no valid Authenticode signature ({detail}); render only after the SimplySign-signed installer has replaced the CI build on the release")
    print(f"signature ok: {path.name}: {lines[2] if len(lines) > 2 else ''}", file=sys.stderr)


def check_can_verify() -> None:
    """在下载安装包之前就确认这台机器能核对签名。

    其他系统上没有等价的检查：osslsigncode 只能对着 TLS 用的 CA 证书包验证，代码签名证书的根常常不在里面，合法的签名也报失败（python.org 的安装包就是这样），而跳过证书链又只剩「有签名」这一条，挡不住自签名。
    """
    if sys.platform != "win32":
        raise RenderError("the installer's Authenticode signature can only be checked on Windows (Get-AuthenticodeSignature); run render.py there, or dispatch .github/workflows/package-definitions-windows.yml")


# ---- 渲染 ----


def outputs(version: str) -> list[tuple[pathlib.Path, pathlib.Path]]:
    """一个发布要渲染的每个文件的（模板，输出目录里的路径）。"""
    winget_dir = pathlib.Path("winget/manifests/m/Metasequoia/MetasequoiaIME") / version
    pairs = [(template, winget_dir / template.name) for template in sorted((HERE / "winget").glob(f"{WINGET_ID}*.yaml"))]
    pairs.append((HERE / "scoop/msime.json", pathlib.Path("scoop/msime.json")))
    pairs.append((HERE / "chocolatey/msime.nuspec", pathlib.Path("chocolatey/msime/msime.nuspec")))
    for script in sorted((HERE / "chocolatey/tools").glob("*.ps1")):
        pairs.append((script, pathlib.Path("chocolatey/msime/tools") / script.name))
    return pairs


def fill(text: str, values: dict[str, str], source: pathlib.Path) -> str:
    def replace(match: re.Match[str]) -> str:
        key = match.group(0)[1:-1]
        if key not in values:
            raise RenderError(f"{source.relative_to(HERE)}: unknown field {match.group(0)}")
        return values[key]

    return PLACEHOLDER_PATTERN.sub(replace, text)


def check_rendered(path: pathlib.Path, text: str) -> None:
    """填完之后解析不了的模板在这里就报错；按官方 schema 的校验在 scripts/test-windows-package-managers.py 里。"""
    if path.suffix == ".json":
        json.loads(text)
    elif path.suffix == ".nuspec":
        ElementTree.fromstring(text.encode("utf-8"))


def render(values: dict[str, str], output: pathlib.Path) -> list[pathlib.Path]:
    written: list[pathlib.Path] = []
    for template, relative in outputs(values["VERSION"]):
        text = fill(template.read_text(encoding="utf-8"), values, template)
        check_rendered(relative, text)
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write(text)
        written.append(target)
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--latest", action="store_true", help="newest published windows-v* release on GitHub")
    source.add_argument("--tag", help="a windows-v* release on GitHub")
    source.add_argument("--version", help="MAJOR.MINOR.PATCH, with --installer")
    parser.add_argument("--installer", type=pathlib.Path, help="local signed installer to hash (with --version)")
    parser.add_argument("--release-date", help="YYYY-MM-DD (with --version; default: today, UTC)")
    parser.add_argument("--repo", default=DEFAULT_REPO, help=f"GitHub repository (default: {DEFAULT_REPO})")
    parser.add_argument("--allow-prerelease", action="store_true", help="render a prerelease named by --tag")
    parser.add_argument("--output", type=pathlib.Path, required=True, help="directory to write the rendered definitions into")
    args = parser.parse_args(argv)

    try:
        check_can_verify()
        if args.version is not None:
            if args.installer is None:
                parser.error("--version needs --installer")
            if args.installer.name != installer_name(args.version):
                raise RenderError(f"{args.installer.name} is not {installer_name(args.version)}, the name the release publishes")
            verify_signature(args.installer)
            digest = sha256_file(args.installer)
            date = args.release_date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
            values = values_for(args.repo, args.version, digest, date)
        else:
            if args.installer is not None or args.release_date is not None:
                parser.error("--installer and --release-date go with --version")
            release = latest_release(args.repo) if args.latest else tagged_release(args.repo, args.tag)
            values = values_from_release(args.repo, release, args.allow_prerelease)
        written = render(values, args.output)
    except RenderError as error:
        print(f"render.py: {error}", file=sys.stderr)
        return 1

    for key in ("VERSION", "INSTALLER_URL", "SHA256", "RELEASE_DATE", "RELEASE_NOTES_URL"):
        print(f"{key}={values[key]}")
    for path in written:
        print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Render the winget, Scoop and Chocolatey definitions for one Windows release.

The templates next to this script hold every fact that does not change between releases (product code, silent switches, dependencies, descriptions); a release only supplies its version, the installer URL, the installer's SHA-256, its date and its release-notes URL, which this script fills into the @...@ fields. It takes them from one of three places:

    render.py --latest --output DIR                      newest published windows-v* release on GitHub
    render.py --tag windows-v0.1.0 --output DIR          that release on GitHub
    render.py --version 0.1.0 --installer FILE --output DIR
    render.py --version 0.1.0 --sha256 HEX --output DIR

From GitHub it reads the release metadata only: the installer's digest that GitHub records for each asset, cross-checked against the small `<installer>.sha256` file the release workflow uploads beside it. The installer itself is never downloaded. `--installer` hashes a local file instead, for the release job that has just built it; the URL is then the one the release workflow will publish it under.

The output directory receives the three package managers' own layouts:

    winget/manifests/m/Metasequoia/MetasequoiaIME/<version>/*.yaml
    scoop/msime.json
    chocolatey/msime/msime.nuspec, chocolatey/msime/tools/*.ps1

Only the Python standard library is used. Set GH_TOKEN or GITHUB_TOKEN to raise the GitHub API rate limit.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import sys
import urllib.error
import urllib.request
import xml.etree.ElementTree as ElementTree

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_REPO = "metasequoiaime/msime"
TAG_PREFIX = "windows-v"
WINGET_ID = "Metasequoia.MetasequoiaIME"
# release-windows.yml accepts three numeric components only, because Build-Client.ps1 stamps the version into Tauri and Server metadata.
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+$")
SHA256_PATTERN = re.compile(r"^[0-9a-fA-F]{64}$")
PLACEHOLDER_PATTERN = re.compile(r"@[A-Z0-9_]+@")
# The sidecar holds "<hex>  <name>\n"; anything larger is not one.
SIDECAR_LIMIT = 4096
API_PAGES = 10


class RenderError(Exception):
    pass


def installer_name(version: str) -> str:
    # msime_setup.iss: OutputBaseFilename=MetasequoiaIME_Setup_v{#MyAppVersion}; release-windows.yml publishes it under that name.
    return f"MetasequoiaIME_Setup_v{version}.exe"


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


def github_json(url: str):
    return json.loads(github_request(url))


def latest_release(repo: str) -> dict:
    """The highest-versioned published, non-prerelease windows-v release."""
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

    published = release.get("published_at") or release.get("created_at") or ""
    return values_for(
        repo,
        version,
        next(iter(digests.values())),
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


# ---- rendering ----


def outputs(version: str) -> list[tuple[pathlib.Path, pathlib.Path]]:
    """(template, path in the output directory) for every file a release renders."""
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
    """Catch a template that no longer parses once filled; the schema checks live in scripts/test-windows-package-managers.py."""
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
    source.add_argument("--version", help="MAJOR.MINOR.PATCH, with --installer or --sha256")
    parser.add_argument("--installer", type=pathlib.Path, help="local installer to hash (with --version)")
    parser.add_argument("--sha256", help="installer SHA-256 (with --version)")
    parser.add_argument("--release-date", help="YYYY-MM-DD (with --version; default: today, UTC)")
    parser.add_argument("--repo", default=DEFAULT_REPO, help=f"GitHub repository (default: {DEFAULT_REPO})")
    parser.add_argument("--allow-prerelease", action="store_true", help="render a prerelease named by --tag")
    parser.add_argument("--output", type=pathlib.Path, required=True, help="directory to write the rendered definitions into")
    args = parser.parse_args(argv)

    try:
        if args.version is not None:
            if (args.installer is None) == (args.sha256 is None):
                parser.error("--version needs exactly one of --installer and --sha256")
            if args.installer is not None:
                if args.installer.name != installer_name(args.version):
                    raise RenderError(f"{args.installer.name} is not {installer_name(args.version)}, the name the release publishes")
                digest = sha256_file(args.installer)
            else:
                digest = args.sha256
            date = args.release_date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
            values = values_for(args.repo, args.version, digest, date)
        else:
            if args.installer is not None or args.sha256 is not None or args.release_date is not None:
                parser.error("--installer, --sha256 and --release-date go with --version")
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

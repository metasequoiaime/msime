#!/usr/bin/env python3
"""winget、Scoop 与 Chocolatey 的包定义必须描述发布里实际发出的那个安装包。

platforms/windows/packaging/ 下是由 platforms/windows/packaging/render.py 按 windows-v 发布填写的模板。它们重复了安装包的一些事实（AppId 以及由它得到的卸载键、发布者、显示名、架构、权限级别、文件名和静默参数），这些事实的来源是 platforms/windows/installer/msime_setup.iss、editions.iss、版本表 shared/contracts/editions.json 与 release-windows.yml；本检查让两边保持一致，用固定输入渲染全部模板，并在不联网的情况下用一份录制的发布走一遍 render.py 的 GitHub 路径。

不带参数时只用标准库。加 `--schema-dir DIR` 时还按官方 schema 校验渲染结果，需要 PyYAML、jsonschema 和 lxml；DIR 里要有 microsoft/winget-cli schemas/JSON/manifests/v1.12.0 的 manifest.{version,installer,defaultLocale,locale}.1.12.0.json、Scoop 的 schema.json，以及 chocolatey/NuGet.Client src/NuGet.Core/NuGet.Packaging/compiler/resources 的 nuspec.xsd（它的 targetNamespace 是占位符「{0}」，本检查会填上）。下载命令见 platforms/windows/packaging/README.md。
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import pathlib
import re
import sys
import tempfile
import xml.etree.ElementTree as ElementTree

ROOT = pathlib.Path(__file__).resolve().parent.parent
PACKAGING = ROOT / "platforms/windows/packaging"
SETUP = ROOT / "platforms/windows/installer/msime_setup.iss"
EDITIONS_ISS = ROOT / "platforms/windows/installer/editions.iss"
EDITIONS = ROOT / "shared/contracts/editions.json"
SMOKE = ROOT / "platforms/windows/installer/tests/install-smoke.ps1"
WORKFLOW = ROOT / ".github/workflows/release-windows.yml"
NUSPEC_NS = "http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd"
SILENT = ("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART")

failures: list[str] = []


def check(condition: bool, what: str) -> None:
    if not condition:
        failures.append(what)


def load_render():
    spec = importlib.util.spec_from_file_location("msime_windows_packaging_render", PACKAGING / "render.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def setup_value(text: str, key: str) -> str:
    match = re.search(rf"(?m)^{key}=(.*)$", text)
    if not match:
        raise SystemExit(f"{SETUP.relative_to(ROOT)} has no {key}=")
    return match.group(1).strip()


def setup_define(text: str, name: str) -> str:
    match = re.search(rf'(?m)^#define\s+{name}\s+"([^"]*)"', text)
    if not match:
        raise SystemExit(f"{SETUP.relative_to(ROOT)} has no #define {name}")
    return match.group(1)


def yaml_scalar(text: str, key: str) -> list[str]:
    """任意缩进下 `key:` 的每个值，去掉引号；对这里比较的扁平字段够用。"""
    return [value.strip().strip('"') for value in re.findall(rf"(?m)^\s*-?\s*{key}:\s*(.+?)\s*$", text)]


def full_edition() -> dict:
    """版本表里 full 的 Windows 段：各包管理器只发 full 这一个安装包。"""
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    for entry in table["editions"]:
        if entry["id"] == "full":
            return entry["platforms"]["windows"]
    raise SystemExit(f"{EDITIONS.relative_to(ROOT)} has no full edition")


def check_installer_facts(render) -> None:
    setup = SETUP.read_text(encoding="utf-8")
    editions_iss = EDITIONS_ISS.read_text(encoding="utf-8")
    full = full_edition()
    # 安装包的 AppId、显示名和文件名前缀按版本取自 editions.iss，那里的值由版本表生成；包管理器只发 full，所以与版本表 full 的 Windows 段对照。Inno 的卸载键（也就是各包的 ProductCode）是 {GUID}_is1。
    check(setup_value(setup, "AppId") == "{#MyEditionAppId}", "msime_setup.iss AppId no longer comes from editions.iss MyEditionAppId")
    full_branch = re.search(r'(?ms)^#if Edition == "full"$(.*?)^#elif', editions_iss)
    check(full_branch is not None, f"{EDITIONS_ISS.relative_to(ROOT)} has no full branch")
    full_defines = dict(re.findall(r'(?m)^#define\s+(\w+)\s+"([^"]*)"', full_branch.group(1))) if full_branch else {}
    app_id = full["inno_app_id"]
    check(full_defines.get("MyEditionAppId") == "{" + app_id, f"editions.iss full MyEditionAppId is not the escaped form of {app_id}")
    product_code = app_id + "_is1"
    app_name = full["app_name"]
    check(full_defines.get("MyEditionAppName") == app_name, f"editions.iss full MyEditionAppName is not {app_name!r}")
    check(re.search(r"(?m)^#define\s+MyAppName\s+MyEditionAppName\s*$", setup) is not None, "msime_setup.iss MyAppName no longer comes from MyEditionAppName")
    publisher = setup_define(setup, "MyAppPublisher")
    check(setup_value(setup, "AppName") == "{#MyAppName}" and setup_value(setup, "AppPublisher") == "{#MyAppPublisher}", "AppName/AppPublisher no longer come from MyAppName/MyAppPublisher")
    check(setup_value(setup, "ArchitecturesAllowed") == "x64compatible", "the installer is no longer x64 only; add or drop installers in winget, Scoop and Chocolatey to match")
    check(setup_value(setup, "PrivilegesRequired") == "admin", "the installer is no longer per-machine (PrivilegesRequired=admin); winget Scope and the Scoop notes say machine")
    check(setup_value(setup, "OutputBaseFilename") == "{#MyEditionInstallerBaseName}_v{#MyAppVersion}{#MyOutputSuffix}", "the installer file name changed; update render.installer_name and the Scoop autoupdate URL")
    check(full_defines.get("MyEditionInstallerBaseName") == full["installer_base_name"], "editions.iss full MyEditionInstallerBaseName differs from the edition table")
    check(render.installer_name("1.2.3") == f"{full['installer_base_name']}_v1.2.3.exe", "render.installer_name does not follow the full edition's installer_base_name")

    workflow = WORKFLOW.read_text(encoding="utf-8")
    check('tag="windows-v${VERSION}"' in workflow, "release-windows.yml no longer tags windows-v${VERSION}; render.TAG_PREFIX and the Scoop checkver follow it")
    check('"dist/$name.sha256"' in workflow, "release-windows.yml no longer uploads <installer>.sha256; the Scoop autoupdate hash and render.py read it")
    check("'^\\d+\\.\\d+\\.\\d+$'" in workflow and render.VERSION_PATTERN.pattern == r"^\d+\.\d+\.\d+$", "release versions are no longer MAJOR.MINOR.PATCH in both release-windows.yml and render.py")

    smoke = SMOKE.read_text(encoding="utf-8")
    check(all(f"'{switch}'" in smoke for switch in SILENT), "install-smoke.ps1 no longer installs with the switches the packages use")

    installer_yaml = (PACKAGING / f"winget/{render.WINGET_ID}.installer.yaml").read_text(encoding="utf-8")
    check(yaml_scalar(installer_yaml, "ProductCode") == [product_code, product_code], f"winget ProductCode is not {product_code} in both places")
    check(yaml_scalar(installer_yaml, "DisplayName") == [app_name], f"winget DisplayName is not {app_name!r}")
    check(yaml_scalar(installer_yaml, "Publisher") == [publisher], f"winget AppsAndFeaturesEntries Publisher is not {publisher!r}")
    check(yaml_scalar(installer_yaml, "Architecture") == ["x64"], "winget offers an installer other than x64")
    check(yaml_scalar(installer_yaml, "Scope") == ["machine"] and yaml_scalar(installer_yaml, "InstallerType") == ["inno", "inno"], "winget is not a machine-scope Inno installer")
    check(yaml_scalar(installer_yaml, "UpgradeBehavior") == ["install"], "winget must upgrade in place: uninstalling first deletes the user's data directory")
    for mode in ("Silent", "SilentWithProgress"):
        switches = yaml_scalar(installer_yaml, mode)
        check(len(switches) == 1 and all(s in switches[0] for s in SILENT[1:]), f"winget {mode} switches lack {SILENT[1:]}")
    for locale in (PACKAGING / "winget").glob(f"{render.WINGET_ID}.locale.*.yaml"):
        check(yaml_scalar(locale.read_text(encoding="utf-8"), "Publisher") == [publisher], f"{locale.name} Publisher is not {publisher!r}")

    scoop = json.loads((PACKAGING / "scoop/msime.json").read_text(encoding="utf-8"))
    install_script = "\n".join(scoop["installer"]["script"])
    uninstall_script = "\n".join(scoop["uninstaller"]["script"])
    check(all(f"'{s}'" in install_script for s in SILENT), "Scoop installer script lacks the silent switches")
    check(product_code in uninstall_script, f"Scoop uninstaller does not read the {product_code} uninstall key")
    check("$cmd -eq 'update'" in uninstall_script, "Scoop uninstaller must skip 'scoop update': the Inno uninstaller deletes the data directory")
    check(set(scoop["architecture"]) == {"64bit"} and set(scoop["autoupdate"]["architecture"]) == {"64bit"}, "Scoop offers an architecture other than 64bit")
    check(scoop["autoupdate"]["architecture"]["64bit"]["url"] == render.release_download_url(render.DEFAULT_REPO, "$version"), "Scoop autoupdate URL differs from render.release_download_url")

    choco_install = (PACKAGING / "chocolatey/tools/chocolateyinstall.ps1").read_text(encoding="utf-8")
    choco_uninstall = (PACKAGING / "chocolatey/tools/chocolateyuninstall.ps1").read_text(encoding="utf-8")
    check(all(s in choco_install for s in SILENT) and "url64bit" in choco_install and "url " not in choco_install, "Chocolatey install is not a silent x64-only install")
    check(f"'{product_code}'" in choco_uninstall and all(s in choco_uninstall for s in SILENT), "Chocolatey uninstall does not run the registered Inno uninstaller silently")
    # Windows PowerShell 5.1 按 ANSI 代码页读取没有 BOM 的脚本，而 render.py 不写 BOM，所以脚本只能是 ASCII。
    for script in (PACKAGING / "chocolatey/tools").glob("*.ps1"):
        check(script.read_bytes().isascii(), f"{script.name} is not ASCII")


def scoop_checkver(scoop: dict, api_text: str) -> str | None:
    match = re.search(scoop["checkver"]["regex"], api_text)
    return match.group(1) if match else None


def check_scoop_checkver() -> None:
    scoop = json.loads((PACKAGING / "scoop/msime.json").read_text(encoding="utf-8"))
    releases = [
        {"url": "u", "author": {"login": "bot", "events_url": "https://x{/privacy}"}, "tag_name": "linux-v0.9.1", "name": "Linux 0.9.1", "draft": False, "prerelease": False},
        {"url": "u", "author": {"login": "bot"}, "tag_name": "windows-v0.3.0", "target_commitish": "abc", "name": "Windows 0.3.0", "draft": False, "immutable": False, "prerelease": True, "assets": []},
        {"url": "u", "author": {"login": "bot"}, "tag_name": "windows-v0.2.1", "target_commitish": "abc", "name": "Windows 0.2.1", "draft": False, "immutable": False, "prerelease": False, "assets": [{"name": "a"}]},
        {"url": "u", "author": {"login": "bot"}, "tag_name": "windows-v0.2.0", "name": "Windows 0.2.0", "draft": False, "prerelease": False},
    ]
    # GitHub API 的输出带两格缩进；缩进与紧凑两种写法都要能匹配。
    for text in (json.dumps(releases, indent=2), json.dumps(releases, separators=(",", ":"))):
        check(scoop_checkver(scoop, text) == "0.2.1", "Scoop checkver does not pick the newest non-prerelease windows-v release")


def check_render(render, output: pathlib.Path) -> list[pathlib.Path]:
    digest = "0123456789abcdef" * 4
    values = render.values_for(render.DEFAULT_REPO, "1.2.3", digest.upper(), "2026-10-05")
    check(values["SHA256"] == digest and values["SHA256_UPPER"] == digest.upper(), "render.values_for does not normalise the digest's case")
    for bad in (lambda: render.values_for(render.DEFAULT_REPO, "1.2", digest, "2026-10-05"), lambda: render.values_for(render.DEFAULT_REPO, "1.2.3", "abc", "2026-10-05"), lambda: render.values_for(render.DEFAULT_REPO, "1.2.3", digest, "2026-13-01")):
        try:
            bad()
            check(False, "render.values_for accepted a malformed version, digest or date")
        except render.RenderError:
            pass

    written = render.render(values, output)
    names = sorted(str(path.relative_to(output)) for path in written)
    expected = sorted(
        [f"winget/manifests/m/Metasequoia/MetasequoiaIME/1.2.3/{render.WINGET_ID}{suffix}.yaml" for suffix in ("", ".installer", ".locale.en-US", ".locale.zh-CN")]
        + ["scoop/msime.json", "chocolatey/msime/msime.nuspec", "chocolatey/msime/tools/chocolateyinstall.ps1", "chocolatey/msime/tools/chocolateyuninstall.ps1"]
    )
    check(names == expected, f"render.py wrote {names}, expected {expected}")
    url = "https://github.com/metasequoiaime/msime/releases/download/windows-v1.2.3/MetasequoiaIME_Setup_v1.2.3.exe"
    for path in written:
        text = path.read_text(encoding="utf-8")
        check(not render.PLACEHOLDER_PATTERN.search(text), f"{path.name} still has a @...@ field after rendering")
        check(not text.startswith("﻿"), f"{path.name} starts with a BOM")
        if path.suffix == ".yaml":
            check(yaml_scalar(text, "PackageVersion") == ["1.2.3"], f"{path.name} PackageVersion is not 1.2.3")
    installer_yaml = next(path for path in written if path.name.endswith(".installer.yaml")).read_text(encoding="utf-8")
    check(yaml_scalar(installer_yaml, "InstallerUrl") == [url] and yaml_scalar(installer_yaml, "InstallerSha256") == [digest.upper()], "winget installer URL or digest not rendered")
    check(yaml_scalar(installer_yaml, "ReleaseDate") == ["2026-10-05"], "winget ReleaseDate not rendered")
    scoop = json.loads((output / "scoop/msime.json").read_text(encoding="utf-8"))
    check(scoop["version"] == "1.2.3" and scoop["architecture"]["64bit"] == {"url": url, "hash": digest}, "Scoop version, URL or hash not rendered")
    nuspec = ElementTree.parse(output / "chocolatey/msime/msime.nuspec").getroot()
    check(nuspec.tag == f"{{{NUSPEC_NS}}}package", "nuspec root is not a package in the nuspec namespace")
    check(nuspec.findtext(f"{{{NUSPEC_NS}}}metadata/{{{NUSPEC_NS}}}version") == "1.2.3", "nuspec version not rendered")
    choco_install = (output / "chocolatey/msime/tools/chocolateyinstall.ps1").read_text(encoding="utf-8")
    check(f"url64bit       = '{url}'" in choco_install and f"checksum64     = '{digest}'" in choco_install, "Chocolatey URL or checksum not rendered")

    # --installer：对刚构建出的安装包求摘要，文件名必须是发布时用的那个。
    fake = output / "MetasequoiaIME_Setup_v1.2.3.exe"
    fake.write_bytes(b"msime")
    with contextlib.redirect_stdout(io.StringIO()):
        check(render.main(["--version", "1.2.3", "--installer", str(fake), "--release-date", "2026-10-05", "--output", str(output / "local")]) == 0, "render.py --installer failed")
    local = json.loads((output / "local/scoop/msime.json").read_text(encoding="utf-8"))
    check(local["architecture"]["64bit"]["hash"] == hashlib.sha256(b"msime").hexdigest(), "render.py --installer did not hash the file")
    wrong = output / "setup.exe"
    wrong.write_bytes(b"msime")
    with contextlib.redirect_stderr(io.StringIO()):
        check(render.main(["--version", "1.2.3", "--installer", str(wrong), "--output", str(output / "wrong")]) == 1, "render.py accepted an installer under a name the release does not publish")
    return written


def check_github_path(render) -> None:
    """用一份录制的发布跑 values_from_release，摘要小文件的下载在本地应答。"""
    digest = "ab" * 32
    name = "MetasequoiaIME_Setup_v0.1.0.exe"
    sidecars = {"https://example.invalid/sidecar": f"{digest}  {name}\n".encode()}
    original = render.github_request
    render.github_request = lambda url, accept="": sidecars[url]
    try:
        def release(**changes):
            base = {
                "tag_name": "windows-v0.1.0", "draft": False, "prerelease": False, "published_at": "2026-10-03T03:08:00Z",
                "html_url": "https://github.com/metasequoiaime/msime/releases/tag/windows-v0.1.0",
                "assets": [
                    {"name": name, "digest": f"sha256:{digest}", "size": 1, "browser_download_url": f"https://github.com/metasequoiaime/msime/releases/download/windows-v0.1.0/{name}"},
                    {"name": f"{name}.sha256", "size": 100, "browser_download_url": "https://example.invalid/sidecar"},
                ],
            }
            base.update(changes)
            return base

        values = render.values_from_release(render.DEFAULT_REPO, release(), False)
        check(values["VERSION"] == "0.1.0" and values["SHA256"] == digest and values["RELEASE_DATE"] == "2026-10-03", "values_from_release misread a release")

        def rejects(what: str, payload: dict, allow_prerelease: bool = False) -> None:
            try:
                render.values_from_release(render.DEFAULT_REPO, payload, allow_prerelease)
                check(False, f"values_from_release accepted {what}")
            except render.RenderError:
                pass

        rejects("a prerelease", release(prerelease=True))
        rejects("a draft", release(draft=True))
        rejects("a non-Windows tag", release(tag_name="linux-v0.1.0"))
        rejects("a release without the installer", release(assets=[]))
        rejects("disagreeing digests", release(assets=[dict(release()["assets"][0], digest="sha256:" + "cd" * 32), release()["assets"][1]]))
        rejects("a release with no digest at all", release(assets=[dict(release()["assets"][0], digest=None)]))
        check(render.values_from_release(render.DEFAULT_REPO, release(prerelease=True), True)["VERSION"] == "0.1.0", "--allow-prerelease does not render a prerelease")
    finally:
        render.github_request = original


def validate_schemas(written: list[pathlib.Path], schema_dir: pathlib.Path) -> None:
    import jsonschema
    import yaml
    from lxml import etree

    for path in written:
        if path.suffix == ".yaml":
            document = yaml.safe_load(path.read_text(encoding="utf-8"))
            schema = json.loads((schema_dir / f"manifest.{document['ManifestType']}.{document['ManifestVersion']}.json").read_text(encoding="utf-8"))
            errors = [error.message for error in jsonschema.Draft7Validator(schema, format_checker=jsonschema.FormatChecker()).iter_errors(document)]
        elif path.suffix == ".json":
            schema = json.loads((schema_dir / "schema.json").read_text(encoding="utf-8"))
            errors = [error.message for error in jsonschema.Draft7Validator(schema).iter_errors(json.loads(path.read_text(encoding="utf-8")))]
        elif path.suffix == ".nuspec":
            # 这份 schema 是模板，命名空间属性（targetNamespace、xmlns、xmlns:mstns）都写作「{0}」。
            xsd = (schema_dir / "nuspec.xsd").read_text(encoding="utf-8").replace('"{0}"', f'"{NUSPEC_NS}"')
            schema = etree.XMLSchema(etree.fromstring(xsd.encode("utf-8")))
            valid = schema.validate(etree.parse(str(path)))
            errors = [] if valid else [str(error) for error in schema.error_log]
        else:
            continue
        check(not errors, f"{path.name} fails its schema: {errors}")
        print(f"schema ok: {path.name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--schema-dir", type=pathlib.Path, help="also validate against the official schemas in this directory")
    args = parser.parse_args()

    render = load_render()
    check_installer_facts(render)
    check_scoop_checkver()
    check_github_path(render)
    with tempfile.TemporaryDirectory(prefix="msime-packaging-") as output:
        written = check_render(render, pathlib.Path(output))
        if args.schema_dir:
            validate_schemas(written, args.schema_dir)
        else:
            print("test-windows-package-managers: schema validation skipped (no --schema-dir)")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print("test-windows-package-managers: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())

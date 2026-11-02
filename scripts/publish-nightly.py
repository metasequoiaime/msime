#!/usr/bin/env python3
"""按平台增量更新 nightly，失败时保留上次可用的安装文件。"""

import argparse
import copy
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import quote


SPEC = importlib.util.spec_from_file_location(
    "nightly_packaging", Path(__file__).with_name("package-nightly.py"))
PACKAGING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGING)
PLATFORMS = {"android": "Android", "ios": "iOS", "macos": "macOS",
             "linux": "Linux", "harmony": "HarmonyOS", "windows": "Windows"}
MARKER = "<!-- msime-nightly-state\n"
SHA = re.compile(r"[0-9a-f]{40}")
DIGEST = re.compile(r"sha256:([0-9a-f]{64})")
ASSET_NAME = re.compile(r"[A-Za-z0-9_.-]+")
# 不再发布、但 nightly 上可能还挂着的安装包后缀：macOS 从 DMG 改成 pkg 之后，旧 DMG 不在 PLATFORM_SUFFIXES 里，清理时不算进去就永远删不掉。仍被某个平台的状态引用时照样保留（keep），只删没人指向的。
RETIRED_SUFFIXES = (".dmg",)
# 各平台包内的版本号只有 X.Y.Z；复用的 release-*.yml 把它写在 artifact 名的末尾（msime-windows-full-0.3.1、msime-linux-deb-x86_64-0.11.0、msime-macos-0.52.0）。
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
ARTIFACT_VERSION = re.compile(r"-([0-9]+\.[0-9]+\.[0-9]+)$")


class GitHub:
    def __init__(self, repository):
        self.repository = repository
        self.base = f"repos/{repository}"

    def command(self, arguments, *, data=None, missing_ok=False):
        result = subprocess.run(["gh", *arguments], input=data, text=True,
                                capture_output=True, check=False)
        if result.returncode:
            if missing_ok and "HTTP 404" in result.stderr:
                return None
            raise RuntimeError(result.stderr.strip() or "GitHub 请求失败")
        return result.stdout

    def api(self, method, endpoint, payload=None, *, missing_ok=False, paginate=False):
        arguments = ["api", "--method", method, f"{self.base}/{endpoint}"]
        if paginate:
            arguments += ["--paginate", "--slurp"]
        if payload is not None:
            arguments += ["--input", "-"]
        output = self.command(arguments, data=json.dumps(payload) if payload is not None else None,
                              missing_ok=missing_ok)
        return json.loads(output) if output else None

    def assets(self, release):
        pages = self.api("GET", f"releases/{release['id']}/assets?per_page=100", paginate=True)
        return {asset["name"]: asset for page in pages for asset in page}

    def release(self):
        release = self.api("GET", "releases/tags/nightly", missing_ok=True)
        if release:
            return release
        # 按标签查询只保证找到已发布版本；恢复中断的首次发布还要找已有 draft。
        pages = self.api("GET", "releases?per_page=100", paginate=True)
        return next((release for page in pages for release in page
                     if release.get("tag_name") == "nightly"), None)

    def create(self, source_sha):
        return self.api("POST", "releases", {
            "tag_name": "nightly", "target_commitish": source_sha,
            "name": "水杉输入法 nightly", "draft": True, "prerelease": True,
        })

    def upload(self, paths):
        self.command(["release", "upload", "nightly", "--repo", self.repository,
                      *map(str, paths)])

    def commit(self, release, body, source_sha):
        # 首次 draft 发布时 GitHub 才创建标签，不能提前 PATCH 尚不存在的 ref。
        self.api("PATCH", f"releases/{release['id']}", {
            "body": body, "name": "水杉输入法 nightly", "draft": False,
            "prerelease": True, "make_latest": "false", "target_commitish": source_sha,
        })
        # 汇总标签代表本轮尝试；每个平台的真正源码版本由正文单独记录。
        self.api("PATCH", "git/refs/tags/nightly", {"sha": source_sha, "force": True})

    def delete(self, asset):
        self.api("DELETE", f"releases/assets/{asset['id']}")


def previous_state(release, assets):
    if not release:
        return {"version": 1, "platforms": {}, "checksum": None}
    body = release.get("body") or ""
    if MARKER in body:
        state = json.loads(body.split(MARKER, 1)[1].split("\n-->", 1)[0])
        if state.get("version") != 1:
            raise ValueError("不支持的 nightly 状态版本")
    else:
        # 首次迁移只读取旧包的服务端摘要，不下载几 GB 的旧安装文件。
        match = re.search(r"^nightly source: ([0-9a-f]{40})$", body, re.MULTILINE)
        if not match:
            if not body and release.get("draft"):
                return {"version": 1, "platforms": {}, "checksum": None}
            raise ValueError("无法确认旧 nightly 的源码提交，保留现有发布")
        state = {"version": 1, "platforms": {}, "checksum": "SHA256SUMS"}
        for platform, suffixes in PACKAGING.PLATFORM_SUFFIXES.items():
            files = []
            for name, asset in assets.items():
                if name.endswith(suffixes):
                    digest = DIGEST.fullmatch(asset.get("digest") or "")
                    if not digest:
                        raise ValueError(f"旧附件缺少 SHA-256 摘要：{name}")
                    files.append({"name": name, "sha256": digest.group(1)})
            if files:
                state["platforms"][platform] = {
                    "source": match.group(1), "updated_at": release["published_at"],
                    "run_url": None, "files": sorted(files, key=lambda file: file["name"]),
                }
        state["platforms"]["ios"] = {
            "source": match.group(1), "updated_at": release["published_at"],
            "run_url": None, "files": [],
        }
    for platform, build in state["platforms"].items():
        if platform not in PLATFORMS or not SHA.fullmatch(build["source"]):
            raise ValueError("旧 nightly 平台或源码提交无效")
        # version 与 run 是后来加的可选字段（应用内检查更新的每日构建通道读它们），旧状态里没有。
        if "version" in build and not (isinstance(build["version"], str) and VERSION.fullmatch(build["version"])):
            raise ValueError("旧 nightly 平台版本号无效")
        if "run" in build and not (type(build["run"]) is int and build["run"] > 0):
            raise ValueError("旧 nightly 平台运行标识无效")
        for file in build["files"]:
            name = file["name"]
            if not ASSET_NAME.fullmatch(name) or not re.fullmatch(r"[0-9a-f]{64}", file["sha256"]):
                raise ValueError("旧 nightly 附件元数据无效")
            if name not in assets:
                raise ValueError(f"旧 nightly 引用的附件不存在：{name}")
            digest = assets[name].get("digest")
            if digest and digest != f"sha256:{file['sha256']}":
                raise ValueError(f"旧 nightly 附件摘要不符：{name}")
    return state


def artifact_version(source, platform, paths):
    """这批安装文件所在 artifact 名末尾共同的 X.Y.Z；没有或不一致时为 None，该平台的条目就不带版本号，应用内检查更新不提供它。"""
    versions = set()
    for path in paths:
        artifact = path.relative_to(source / platform).parts[0]
        match = ARTIFACT_VERSION.search(artifact)
        versions.add(match.group(1) if match else None)
    if len(versions) == 1 and None not in versions:
        return versions.pop()
    print(f"::warning::{platform}：无法从 artifact 名确定版本号，nightly 状态不记录版本", file=sys.stderr)
    return None


def render(state, results, updated, repository, source_sha, run_url, ios_group):
    lines = ["各平台独立更新；本轮失败的平台保留上次成功包。",
             f"本轮：[构建记录]({run_url}) · 源码 `{source_sha}`。",
             "汇总标签不代表所有安装包来自同一提交，请以各平台的来源为准。", "",
             "| 平台 | 本轮结果 | 当前包的源码 | 最近成功更新（UTC） |",
             "| --- | --- | --- | --- |"]
    for platform, label in PLATFORMS.items():
        build = state["platforms"].get(platform)
        if platform in updated:
            status = "已更新"
            if platform == "ios":
                # release-ios.yml 只把已发布的版本号送外部组；没发布的只上传到内部组，外部测试者拿不到。
                status = "已提交 TestFlight，待 Apple 审核" if ios_group != "internal" else "仅上传 TestFlight 内部组：这个版本号尚未发布"
        else:
            result = {"failure": "构建或验收失败", "cancelled": "已取消", "skipped": "未运行"}.get(
                results[platform], results[platform])
            status = f"{result}；" + ("保留上次成功构建" if build else "暂无成功构建")
        source = (f"[{build['source'][:12]}](https://github.com/{repository}/commit/{build['source']})"
                  if build else "—")
        lines.append(f"| {label} | {status} | {source} | {build['updated_at'] if build else '—'} |")
    lines += ["", "直接下载下列对应平台的原生安装文件。", ""]
    for platform, label in PLATFORMS.items():
        files = state["platforms"].get(platform, {}).get("files", [])
        if files:
            lines.append(f"**{label}**")
            lines += [f"- [{file['name']}](https://github.com/{repository}/releases/download/nightly/{quote(file['name'])})"
                      for file in files]
            lines.append("")
    if state.get("checksum"):
        name = state["checksum"]
        lines += [f"[SHA-256 校验清单](https://github.com/{repository}/releases/download/nightly/{name})；"
                  f"下载后运行 `sha256sum --ignore-missing -c {name}`。", ""]
    lines += ["Android 安装 APK 后在系统设置中启用输入法；Windows 运行 Setup EXE。",
              "macOS 双击 pkg 安装包按提示安装（需要管理员密码）；Linux 按发行版与架构选择 DEB、RPM 或 Arch 的 .pkg.tar.zst。",
              "iOS 通过现有外部 TestFlight 组安装，须等 Apple 审核；模拟器包只在 Actions 中保留。",
              "HarmonyOS 的 unsigned HAP 供模拟器或开发签名使用，不能直接安装到零售设备。"]
    return "\n".join(lines) + "\n\n" + MARKER + json.dumps(state, ensure_ascii=False) + "\n-->\n"


def publish(client, source, output, results, downloads, source_sha, build_id, run_url, ios_group="external"):
    if not SHA.fullmatch(source_sha) or not re.fullmatch(r"[0-9]+\.[0-9]+", build_id):
        raise ValueError("源码提交或运行标识无效")
    if output.exists() and any(output.iterdir()):
        raise ValueError("输出目录必须为空")
    output.mkdir(parents=True, exist_ok=True)
    release = client.release()
    assets = client.assets(release) if release else {}
    state = copy.deepcopy(previous_state(release, assets))
    updated, errors = set(), []
    run = int(build_id.split(".", 1)[0])
    for platform in PLATFORMS:
        if results[platform] != "success":
            continue
        try:
            files = []
            version = None
            if platform != "ios":
                if downloads.get(platform, {}).get("outcome") != "success":
                    raise ValueError("产物下载失败")
                paths = PACKAGING.platform_files(source, platform)
                version = artifact_version(source, platform, paths)
                staged = []
                for path in paths:
                    if not ASSET_NAME.fullmatch(path.name):
                        raise ValueError("安装文件名无效")
                    # 保留完整扩展名，Arch 包仍须以 .pkg.tar.zst 结尾，才能被 pacman 和下一轮发布识别。
                    suffix = next(suffix for suffix in PACKAGING.PLATFORM_SUFFIXES[platform]
                                  if path.name.endswith(suffix))
                    target = output / f"{path.name.removesuffix(suffix)}-nightly.{build_id}{suffix}"
                    shutil.copyfile(path, target)
                    files.append({"name": target.name, "sha256": PACKAGING.checksum(target)})
                    staged.append(target)
                release = release or client.create(source_sha)
                client.upload(staged)
                uploaded = client.assets(release)
                for file in files:
                    if uploaded.get(file["name"], {}).get("digest") != f"sha256:{file['sha256']}":
                        raise ValueError("上传后的附件摘要不符")
            build = {
                "source": source_sha, "updated_at": datetime.now(timezone.utc).isoformat(),
                "run_url": run_url, "files": files,
            }
            if run:
                build["run"] = run
            if version:
                build["version"] = version
            state["platforms"][platform] = build
            updated.add(platform)
        except (ValueError, RuntimeError, OSError) as error:
            results[platform] = "更新失败"
            errors.append(platform)
            print(f"::warning::{platform}：{error}；保留上次成功包", file=sys.stderr)
    if not release and not updated:
        print("本轮没有可发布的平台，保留现有 nightly")
        if errors:
            raise RuntimeError("平台更新失败：" + ", ".join(errors))
        return None
    release = release or client.create(source_sha)
    all_files = [file for build in state["platforms"].values() for file in build["files"]]
    if all_files:
        checksums = output / f"SHA256SUMS-{build_id}"
        checksums.write_text("".join(f"{file['sha256']}  {file['name']}\n"
                                    for file in sorted(all_files, key=lambda file: file["name"])))
        client.upload([checksums])
        uploaded = client.assets(release).get(checksums.name, {})
        if uploaded.get("digest") != f"sha256:{PACKAGING.checksum(checksums)}":
            raise ValueError("校验清单上传后的摘要不符")
        state["checksum"] = checksums.name
    body = render(state, results, updated, client.repository, source_sha, run_url, ios_group)
    client.commit(release, body, source_sha)
    # 正文已指向完整的新文件集合，才清理不再被任何平台引用的旧附件。
    keep = {file["name"] for file in all_files} | {state.get("checksum")}
    suffixes = tuple(suffix for values in PACKAGING.PLATFORM_SUFFIXES.values() for suffix in values) + RETIRED_SUFFIXES
    try:
        cleanup_assets = client.assets(release)
    except RuntimeError:
        print("::warning::暂未读取旧附件清单；新包已发布，稍后再清理", file=sys.stderr)
        cleanup_assets = {}
    for name, asset in cleanup_assets.items():
        if name not in keep and (name.endswith(suffixes) or name == "README.txt" or name.startswith("SHA256SUMS")):
            try:
                client.delete(asset)
            except RuntimeError:
                print(f"::warning::暂未清理旧附件：{name}", file=sys.stderr)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        Path(os.environ["GITHUB_STEP_SUMMARY"]).write_text(body.split(MARKER, 1)[0])
    if errors:
        raise RuntimeError("其他平台已发布；以下平台保留旧包：" + ", ".join(errors))
    return state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    needs = json.loads(os.environ["NEEDS_RESULTS"])
    results = {platform: needs[platform]["result"] for platform in PLATFORMS}
    publish(GitHub(os.environ["GITHUB_REPOSITORY"]), args.source, args.output,
            results, json.loads(os.environ["DOWNLOAD_RESULTS"]), os.environ["SOURCE_SHA"],
            f"{os.environ['GITHUB_RUN_ID']}.{os.environ['GITHUB_RUN_ATTEMPT']}",
            f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}",
            needs["ios"].get("outputs", {}).get("testflight_group") or "external")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError) as error:
        print(f"::error::{error}", file=sys.stderr)
        raise SystemExit(1)

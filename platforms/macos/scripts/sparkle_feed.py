#!/usr/bin/env python3
"""读取、核对和比较 macOS 的 Sparkle appcast，只用公钥，不碰私钥。

publish-sparkle-feed.sh 和 relocate-sparkle-archives.sh 用它：

- `enclosures`：每个 appcast 恰好一项更新，打印 appcast 文件名、构建号、短版本号和更新包地址，以制表符分隔。
- `verify`：用 `sparkle-public-key.txt` 里的公钥核对 appcast 自带的签名（`sparkle-signatures` 注释，签的是注释之前的 `length` 个字节）；给了 `--archives` 时再核对目录里同名更新包的长度和 `sparkle:edSignature`，给了 `--url-prefix` 时要求地址就是这个前缀加文件名。
- `compare`：两份 appcast 除更新包地址之外每一项都相同，用来证明重新生成的 appcast 只换了地址。
- `set-mtime`：把更新包的修改时间设成 appcast 里的 `pubDate`，`generate_appcast` 按修改时间写 `pubDate`，重新生成时才不会变。

Ed25519 验签用 OpenSSL 3（macOS 自带的 LibreSSL 不支持）：依次取 `MSIME_OPENSSL`、Homebrew 的 `openssl@3`、PATH 里的 `openssl`。
"""

from __future__ import annotations

import argparse
import base64
import email.utils
import os
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

SPARKLE_NS = "http://www.andymatuschak.org/xml-namespaces/sparkle"
ED_SIGNATURE = f"{{{SPARKLE_NS}}}edSignature"
VERSION = f"{{{SPARKLE_NS}}}version"
SHORT_VERSION = f"{{{SPARKLE_NS}}}shortVersionString"
# generate_appcast 写在文件末尾的签名注释；签名覆盖的是这段注释之前的 length 个字节。
FEED_SIGNATURE = re.compile(rb"<!-- sparkle-signatures:\nedSignature: ([A-Za-z0-9+/=]+)\nlength: ([0-9]+)\n-->\s*\Z")
# Ed25519 公钥的 SubjectPublicKeyInfo DER 前缀，后面接 32 字节原始公钥。
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
# 缺省是仓库里发布用的公钥；MSIME_SPARKLE_PUBLIC_KEY_FILE 可以换成别的（例如测试用的一次性密钥）。
DEFAULT_PUBLIC_KEY = Path(os.environ.get("MSIME_SPARKLE_PUBLIC_KEY_FILE") or Path(__file__).resolve().parent.parent / "sparkle-public-key.txt")


class FeedError(Exception):
    pass


def openssl() -> str:
    explicit = os.environ.get("MSIME_OPENSSL")
    if explicit:
        return explicit
    brew = shutil.which("brew")
    if brew:
        prefix = subprocess.run([brew, "--prefix", "openssl@3"], capture_output=True, text=True)
        candidate = Path(prefix.stdout.strip()) / "bin/openssl"
        if prefix.returncode == 0 and candidate.is_file():
            return str(candidate)
    found = shutil.which("openssl")
    if not found:
        raise FeedError("找不到 openssl；设置 MSIME_OPENSSL 指向 OpenSSL 3")
    return found


def load_public_key(path: Path) -> bytes:
    raw = base64.b64decode(path.read_text(encoding="utf-8").strip(), validate=True)
    if len(raw) != 32:
        raise FeedError(f"{path} 不是 32 字节的 Ed25519 公钥")
    return raw


def ed25519_verify(public_key: bytes, data: bytes, signature: bytes) -> bool:
    with tempfile.TemporaryDirectory(prefix="msime-sparkle-verify-") as directory:
        folder = Path(directory)
        (folder / "key.der").write_bytes(ED25519_SPKI_PREFIX + public_key)
        (folder / "data").write_bytes(data)
        (folder / "signature").write_bytes(signature)
        result = subprocess.run(
            [openssl(), "pkeyutl", "-verify", "-pubin", "-keyform", "DER", "-inkey", str(folder / "key.der"),
             "-rawin", "-in", str(folder / "data"), "-sigfile", str(folder / "signature")],
            capture_output=True, text=True,
        )
        return result.returncode == 0


def single_item(path: Path) -> ET.Element:
    try:
        root = ET.fromstring(path.read_bytes())
    except ET.ParseError as error:
        raise FeedError(f"{path.name} 不是有效的 XML：{error}") from error
    items = root.findall("./channel/item")
    if len(items) != 1:
        raise FeedError(f"{path.name} 有 {len(items)} 项更新，应当恰好一项")
    enclosures = items[0].findall("enclosure")
    if len(enclosures) != 1:
        raise FeedError(f"{path.name} 的更新有 {len(enclosures)} 个 enclosure，应当恰好一个")
    return items[0]


def item_text(item: ET.Element, tag: str, path: Path) -> str:
    value = item.findtext(tag)
    if not value:
        raise FeedError(f"{path.name} 缺少 {tag}")
    return value.strip()


def enclosure_url(item: ET.Element, path: Path) -> str:
    url = item.find("enclosure").get("url", "")
    if not url.startswith("https://"):
        raise FeedError(f"{path.name} 的更新包地址不是 https：{url!r}")
    return url


def archive_name(url: str) -> str:
    return url.rsplit("/", 1)[-1]


def command_enclosures(args: argparse.Namespace) -> None:
    for path in args.appcasts:
        item = single_item(path)
        fields = (path.name, item_text(item, VERSION, path), item_text(item, SHORT_VERSION, path), enclosure_url(item, path))
        print("\t".join(fields))


def verify_one(path: Path, public_key: bytes, archives: Path | None, url_prefix: str | None) -> str:
    raw = path.read_bytes()
    match = FEED_SIGNATURE.search(raw)
    if not match:
        raise FeedError(f"{path.name} 没有 appcast 签名")
    length = int(match.group(2))
    if length > match.start():
        raise FeedError(f"{path.name} 的签名长度 {length} 超出了签名注释之前的内容")
    if not ed25519_verify(public_key, raw[:length], base64.b64decode(match.group(1))):
        raise FeedError(f"{path.name} 的 appcast 签名与公钥不符")
    if raw[length:match.start()].strip():
        raise FeedError(f"{path.name} 在签名覆盖的内容之后还有未签名的内容")
    item = single_item(path)
    url = enclosure_url(item, path)
    name = archive_name(url)
    if url_prefix is not None and url != url_prefix + name:
        raise FeedError(f"{path.name} 指向 {url}，不在 {url_prefix} 下")
    if archives is not None:
        archive = archives / name
        if not archive.is_file():
            raise FeedError(f"{path.name} 指向的 {name} 不在 {archives}")
        enclosure = item.find("enclosure")
        if enclosure.get("length") != str(archive.stat().st_size):
            raise FeedError(f"{path.name} 记录的长度 {enclosure.get('length')} 与 {name} 的 {archive.stat().st_size} 不符")
        signature = enclosure.get(ED_SIGNATURE)
        if not signature:
            raise FeedError(f"{path.name} 的更新包没有 sparkle:edSignature")
        if not ed25519_verify(public_key, archive.read_bytes(), base64.b64decode(signature)):
            raise FeedError(f"{name} 与 {path.name} 里的 sparkle:edSignature 不符")
    return url


def command_verify(args: argparse.Namespace) -> None:
    public_key = load_public_key(args.public_key)
    for path in args.appcasts:
        url = verify_one(path, public_key, args.archives, args.url_prefix)
        print(f"{path.name}: appcast 签名有效，更新包 {url}" + ("，长度与签名一致" if args.archives else ""))


def comparable(path: Path) -> tuple:
    root = ET.fromstring(path.read_bytes())
    item = single_item(path)
    children = []
    for child in item:
        attributes = dict(child.attrib)
        if child.tag == "enclosure":
            attributes.pop("url", None)
        children.append((child.tag, (child.text or "").strip(), sorted(attributes.items())))
    return (root.findtext("./channel/title"), children)


def command_compare(args: argparse.Namespace) -> None:
    old, new = comparable(args.old), comparable(args.new)
    if old != new:
        raise FeedError(f"{args.new.name} 与 {args.old.name} 除更新包地址外还有不同：\n旧 {old}\n新 {new}")
    old_url = enclosure_url(single_item(args.old), args.old)
    new_url = enclosure_url(single_item(args.new), args.new)
    if archive_name(old_url) != archive_name(new_url):
        raise FeedError(f"{args.new.name} 换了更新包文件：{old_url} -> {new_url}")
    print(f"{args.new.name}: 只有地址不同，{old_url} -> {new_url}")


def command_set_mtime(args: argparse.Namespace) -> None:
    item = single_item(args.appcast)
    published = email.utils.parsedate_to_datetime(item_text(item, "pubDate", args.appcast))
    timestamp = published.timestamp()
    os.utime(args.archive, (timestamp, timestamp))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    enclosures = commands.add_parser("enclosures")
    enclosures.add_argument("appcasts", nargs="+", type=Path)
    enclosures.set_defaults(run=command_enclosures)
    verify = commands.add_parser("verify")
    verify.add_argument("--public-key", type=Path, default=DEFAULT_PUBLIC_KEY)
    verify.add_argument("--archives", type=Path)
    verify.add_argument("--url-prefix")
    verify.add_argument("appcasts", nargs="+", type=Path)
    verify.set_defaults(run=command_verify)
    compare = commands.add_parser("compare")
    compare.add_argument("old", type=Path)
    compare.add_argument("new", type=Path)
    compare.set_defaults(run=command_compare)
    set_mtime = commands.add_parser("set-mtime")
    set_mtime.add_argument("appcast", type=Path)
    set_mtime.add_argument("archive", type=Path)
    set_mtime.set_defaults(run=command_set_mtime)
    args = parser.parse_args()
    try:
        args.run(args)
    except FeedError as error:
        print(f"sparkle_feed.py: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

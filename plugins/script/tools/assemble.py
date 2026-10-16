#!/usr/bin/env python3
"""把编译好的模块和 pack/ 组装成插件包：target/script-plugins/<id>/ 文件夹和 target/script-plugins/<id>-<version>.zip。

由 scripts/build-script-plugins.sh 在 cargo 构建之后调用，不单独使用。检查本目录约定的几件事：

- 目录名、Cargo.toml 的包名和 plugin.toml 的 id 三者相同；
- pack/ 里没有 .wasm（模块只来自构建，不提交）；
- 模块不超过 256 KiB，线性内存的最小页数不超过清单的 memory_pages（不写时 16）；
- pack/ 里按约定命名的第三方许可证和仓库里的原件逐字节相同；
- 许可证义务按 crate 实际编进模块的数据推出来：从 src/ 出发顺着 `#[path]` 找到全部源文件，收集其中 `include_bytes!` 的文件，在 [EMBEDDED_DATA] 里查它要求包里带哪些许可证原文、清单的 license 要含哪些 SPDX 标识、NOTICE.txt 要原文包含哪份来源说明。编进了表里没有的数据文件直接失败，先把它的义务登记进表。插件代码本身的 MIT 也一样：pack/LICENSE.txt 必须是 plugins/script/LICENSE 的副本，license 必须含 MIT。

压缩包是确定的：条目按名字排序、放在顶层、时间固定为 1980-01-01、权限 0644、不压缩（deflate 的输出随 zlib 版本变化，插件包只有几百 KiB 以内，不值得为此失去逐字节可复现）。

usage: assemble.py <plugin-dir> <module.wasm>
"""
import re
import shutil
import sys
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "target/script-plugins"
MAX_MODULE_BYTES = 256 * 1024
DEFAULT_MEMORY_PAGES = 16
# pack/ 里这些文件名必须是仓库里对应原件的逐字节副本，免得许可证被改了一份、忘了另一份。
LICENSE_COPIES = {
    "LICENSE.txt": ROOT / "plugins/script/LICENSE",
    "Unicode-License-V3.txt": ROOT / "resources/licenses/Unicode-License-V3.txt",
    "OpenCC-LICENSE.txt": ROOT / "resources/licenses/OpenCC-Apache-2.0.txt",
}
# 插件代码自己的许可证，每个包都要带。
CODE_LICENSE = ("LICENSE.txt", "MIT")
# 编进模块的第三方数据（相对仓库根）→（包里要带的许可证原文、license 要含的 SPDX 标识、NOTICE.txt 要原文包含的来源说明）。
EMBEDDED_DATA = {
    "plugins/script/data/common-chars.bin": (
        ("Unicode-License-V3.txt", "OpenCC-LICENSE.txt"),
        ("Unicode-3.0", "Apache-2.0"),
        ROOT / "plugins/script/data/common-chars-NOTICE.txt",
    ),
}
PATH_ATTRIBUTE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
INCLUDE_BYTES = re.compile(r'include_bytes!\(\s*"([^"]+)"\s*\)')


def leb128(data: bytes, offset: int) -> tuple[int, int]:
    value = shift = 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if byte < 0x80:
            return value, offset


def minimum_pages(module: bytes) -> int:
    """模块自己定义的那块线性内存的最小页数（memory 段，编号 5）。"""
    if module[:8] != b"\0asm\1\0\0\0":
        raise SystemExit("不是 core wasm 模块")
    offset = 8
    while offset < len(module):
        section = module[offset]
        size, offset = leb128(module, offset + 1)
        if section == 5:
            count, cursor = leb128(module, offset)
            if count != 1:
                raise SystemExit(f"模块定义了 {count} 块线性内存")
            cursor += 1  # limits 的标志字节
            pages, _ = leb128(module, cursor)
            return pages
        offset += size
    raise SystemExit("模块没有定义线性内存")


def embedded_data(plugin: Path) -> set[str]:
    """crate 编进模块的数据文件（相对仓库根）：src/ 下的源文件，加上顺着 `#[path]` 找到的源文件（相对写它的那个文件所在的目录），其中全部 `include_bytes!` 的目标。注释行里举例写的 `#[path]` 不算。"""
    pending = sorted((plugin / "src").rglob("*.rs"))
    seen: set[Path] = set()
    data: set[str] = set()
    while pending:
        source = pending.pop().resolve()
        if source in seen:
            continue
        seen.add(source)
        text = "\n".join(line for line in source.read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("//"))
        pending.extend(source.parent / target for target in PATH_ATTRIBUTE.findall(text))
        for target in INCLUDE_BYTES.findall(text):
            data.add((source.parent / target).resolve().relative_to(ROOT).as_posix())
    return data


def check_obligations(plugin_id: str, plugin: Path, manifest: dict, names: set[str]) -> None:
    """包里带齐插件代码和编进模块的第三方数据要求的许可证原文、SPDX 标识和来源说明。"""
    terms = {term.strip() for term in manifest.get("license", "").split(" AND ")}
    files, spdx, notices = {CODE_LICENSE[0]}, {CODE_LICENSE[1]}, []
    for data in sorted(embedded_data(plugin)):
        if data not in EMBEDDED_DATA:
            raise SystemExit(f"{plugin_id}: 模块编进了 {data}，但 assemble.py 的 EMBEDDED_DATA 没有登记它的许可证义务")
        licenses, identifiers, notice = EMBEDDED_DATA[data]
        files.update(licenses)
        spdx.update(identifiers)
        if notice is not None:
            notices.append(notice)
    if missing := sorted(files - names):
        raise SystemExit(f"{plugin_id}: pack/ 缺少 {'、'.join(missing)}")
    if missing := sorted(spdx - terms):
        raise SystemExit(f"{plugin_id}: plugin.toml 的 license 缺少 {'、'.join(missing)}（现在是 {manifest.get('license')!r}）")
    if notices:
        if "NOTICE.txt" not in names:
            raise SystemExit(f"{plugin_id}: pack/ 缺少 NOTICE.txt")
        text = (plugin / "pack/NOTICE.txt").read_text(encoding="utf-8")
        for notice in notices:
            if notice.read_text(encoding="utf-8").strip() not in text:
                raise SystemExit(f"{plugin_id}: pack/NOTICE.txt 没有原文包含 {notice.relative_to(ROOT)}，重新照录一份")


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    plugin = Path(argv[0]).resolve()
    module_path = Path(argv[1])
    plugin_id = plugin.name
    cargo = tomllib.loads((plugin / "Cargo.toml").read_text(encoding="utf-8"))
    manifest = tomllib.loads((plugin / "pack/plugin.toml").read_text(encoding="utf-8"))
    if cargo["package"]["name"] != plugin_id or manifest.get("id") != plugin_id:
        raise SystemExit(f"{plugin_id}: 目录名、Cargo.toml 的 name 和 plugin.toml 的 id 必须相同")
    script = manifest["script"]
    pack_files = sorted(path for path in (plugin / "pack").iterdir() if not path.name.startswith("."))
    if any(path.suffix.lower() == ".wasm" for path in pack_files):
        raise SystemExit(f"{plugin_id}: pack/ 里不能放 .wasm，模块由构建生成")
    for path in pack_files:
        original = LICENSE_COPIES.get(path.name)
        if original is not None and path.read_bytes() != original.read_bytes():
            raise SystemExit(f"{plugin_id}: pack/{path.name} 和 {original.relative_to(ROOT)} 不一致，重新复制一份")
    check_obligations(plugin_id, plugin, manifest, {path.name for path in pack_files})

    module = module_path.read_bytes()
    if len(module) > MAX_MODULE_BYTES:
        raise SystemExit(f"{plugin_id}: 模块 {len(module)} 字节，超过 {MAX_MODULE_BYTES}")
    pages = minimum_pages(module)
    allowed = script.get("memory_pages", DEFAULT_MEMORY_PAGES)
    if pages > allowed:
        raise SystemExit(f"{plugin_id}: 模块至少要 {pages} 页内存，清单的 memory_pages 是 {allowed}")

    folder = OUT / plugin_id
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    entries = {path.name: path.read_bytes() for path in pack_files}
    entries[script["module"]] = module
    for name, content in entries.items():
        (folder / name).write_bytes(content)

    archive = OUT / f"{plugin_id}-{manifest['version']}.zip"
    archive.unlink(missing_ok=True)
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_STORED) as bundle:
        for name in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            bundle.writestr(info, entries[name])
    print(f"{plugin_id} {manifest['version']}: module {len(module)} bytes, memory {pages}/{allowed} pages -> {folder.relative_to(ROOT)}, {archive.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

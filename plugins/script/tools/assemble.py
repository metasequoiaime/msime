#!/usr/bin/env python3
"""把编译好的模块和 pack/ 组装成插件包：target/script-plugins/<id>/ 文件夹和 target/script-plugins/<id>-<version>.zip。

由 scripts/build-script-plugins.sh 在 cargo 构建之后调用，不单独使用。检查本目录约定的几件事：

- 目录名、Cargo.toml 的包名和 plugin.toml 的 id 三者相同；
- pack/ 里没有 .wasm（模块只来自构建，不提交）；
- 模块不超过 256 KiB，线性内存的最小页数不超过清单的 memory_pages（不写时 16）；
- pack/ 里按约定命名的第三方许可证和仓库里的原件逐字节相同。

压缩包是确定的：条目按名字排序、放在顶层、时间固定为 1980-01-01、权限 0644、不压缩（deflate 的输出随 zlib 版本变化，插件包只有几百 KiB 以内，不值得为此失去逐字节可复现）。

usage: assemble.py <plugin-dir> <module.wasm>
"""
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
    "Unicode-License-V3.txt": ROOT / "resources/licenses/Unicode-License-V3.txt",
    "OpenCC-LICENSE.txt": ROOT / "crates/client-core/data/opencc/LICENSE",
}


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

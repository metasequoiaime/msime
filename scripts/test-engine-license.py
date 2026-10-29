#!/usr/bin/env python3
"""Check that msime-engine and msime-path-trust stay MPL-2.0 and that what that licence and the data inside the engine require travels through every notice channel.

The two crates are licensed under MPL-2.0 without Exhibit B while the rest of the repository is GPL-3.0-only (crates/engine/README.md, the 许可证 section). So:

- each crate's LICENSE is the Exhibit A notice followed by resources/licenses/MPL-2.0.txt verbatim, says Exhibit B is not attached, and its src/lib.rs opens with the same notice;
- the license field of crates/path-trust/Cargo.toml is MPL-2.0, and that of crates/engine/Cargo.toml is the SPDX expression of the code and the data compiled into the engine: MPL-2.0 for the code plus the licences of the data files and adapted code listed in EMBEDDED and ADAPTED below, no more and no fewer, and no GPL or LGPL;
- every data file the engine compiles in with include_str! or include_bytes! from inside its own directory is either in EMBEDDED or is test data, every EMBEDDED and ADAPTED file is named in the crate README's table, and nothing compiled in reaches outside the crate (cargo package cannot carry such a file);
- the licence texts those files need ship in every channel that lists notices by hand, and the inbound "GPL-3.0-only OR MPL-2.0" rule is stated where contributors read it;
- the distribution metadata lists the engine's licences;
- the package published to crates.io carries what those licences ask for: the engine's licenses/ holds the licence texts the README table names, each a byte-for-byte copy of the file of the same name in resources/licenses/, and the include whitelist of crates/engine/Cargo.toml takes in every compiled-in data file, licenses/, LICENSE and README.md while leaving out tests/, examples/ and the test data; both crates carry a version of their own, the engine's path dependency on msime-path-trust names that version, and the READMEs link outside their crate only by absolute URL, since crates.io renders them away from the repository.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENGINE = ROOT / "crates/engine"
MPL_TEXT = ROOT / "resources/licenses/MPL-2.0.txt"
EXHIBIT_A = "This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/."
INBOUND = "GPL-3.0-only OR MPL-2.0"

# 编进引擎、保留各自许可证的数据文件（相对 crates/engine/src）→ 它们带进包的 SPDX 标识。
EMBEDDED = {
    "korean/hanja.tsv": {"BSD-3-Clause"},
    "local/places.tsv": {"WTFPL"},
    "local/chaizi.tsv": {"CC-BY-3.0", "Unicode-3.0", "Apache-2.0"},
    "local/chaizi-chars.tsv": {"CC-BY-3.0", "Unicode-3.0", "Apache-2.0"},
    "local/symbol_groups.tsv": {"Unicode-3.0"},
    "local/latex_symbols.tsv": {"MIT"},
    # 本项目自写，与代码同为 MPL-2.0。
    "local/symbol_candidates.tsv": {"MPL-2.0"},
    "lattice/offensive_words.txt": {"MPL-2.0"},
    # 聚合统计事实，不带许可证（维护者 2026-10-29 裁定，见 docs/third-party.md）。
    "nine_key/syllable_frequency.tsv": set(),
}
# 改编自第三方代码的源文件 → 它们带进包的 SPDX 标识（本项目的改动是 MPL-2.0）。
ADAPTED = {
    "handwriting/model.rs": {"BSD-3-Clause"},
    "handwriting/features.rs": {"BSD-3-Clause"},
    "lattice/octagram.rs": {"BSD-3-Clause", "BSD-2-Clause"},
    "lattice/cxx_sort.rs": {"Apache-2.0 WITH LLVM-exception"},
}
# 只给测试读的数据：目录名或文件名带这些片段的不算编进产品。
TEST_DATA = ("/testdata/", "_fixture")

# 手工列声明的渠道必须在非注释行写出的许可证文件。Linux 和 Windows 的 MPL-2.0 全文走 collect-notices.py 收集的 rust-crates-NOTICES.txt（见 COLLECTOR），所以那几个渠道只要求 libc++ 的文本。
LLVM_LICENCE = "libcxx-Apache-2.0-WITH-LLVM-exception.txt"
# 引擎编进的 LaTeX 表是 MIT，不带官方短语表包的构建也要带上它的全文；Windows 与网页引擎直接收 plugins/phrase_table/LICENSE。
LATEX_LICENCE = "latex-symbols-MIT.txt"
CHANNELS = {
    "platforms/macos/CMakeLists.txt": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/macos/tests/settings/bundle_contents.py": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/linux/CMakeLists.txt": (LLVM_LICENCE, LATEX_LICENCE),
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt": (LLVM_LICENCE, LATEX_LICENCE),
    "platforms/windows/Collect-Notices.ps1": (LLVM_LICENCE, "plugins/phrase_table/LICENSE"),
    "platforms/windows/tests/tools/collect_notices.ps1": (LLVM_LICENCE,),
    "platforms/windows/Notices.md": (LLVM_LICENCE,),
    "platforms/android/build-native.sh": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/harmony/stage-resources.sh": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/ios/project.yml": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/ios/tests/settings/ProjectConfigurationTests.py": ("MPL-2.0.txt", LLVM_LICENCE, LATEX_LICENCE),
    "platforms/ios/App/Sources/settings/AboutAndDownloadViews.swift": ('"MPL-2.0"', '"libcxx-Apache-2.0-WITH-LLVM-exception"', '"latex-symbols-MIT"'),
    "scripts/web-engine-notice.sh": ("crates/engine/LICENSE", LLVM_LICENCE, "librime-octagram-BSD-3-Clause.txt", "Unicode-License-V3.txt", "plugins/phrase_table/LICENSE"),
    "docs/third-party.md": (LLVM_LICENCE, "crates/engine/README.md"),
}
COLLECTOR = "platforms/linux/collect-notices.py"
# 随 crates.io 上的引擎包发布的许可证全文：README 表里「全文」一列指向的文件，逐字节复制自 resources/licenses/。
PACKAGED_LICENCES = {
    "libhangul-hanja-BSD-3-Clause.txt",
    "Administrative-divisions-of-China-WTFPL.txt",
    "kfcd-chaizi-CC-BY-3.0.txt",
    "Unicode-License-V3.txt",
    "OpenCC-Apache-2.0.txt",
    "latex-symbols-MIT.txt",
    "Zinnia-LICENSE.txt",
    "librime-octagram-BSD-3-Clause.txt",
    LLVM_LICENCE,
}
# 一定不能进包的路径（相对 crates/engine）：录自 GPL 词库的金样、手写参考输出、自动纠错表的转储，以及读仓库里词库的示例。
UNPACKAGED = ("tests/golden.rs", "tests/golden/real/example.json", "src/handwriting/testdata/strokes.tsv", "src/pinyin/autocorrect_table_fixture.txt", "examples/query_dictionary.rs")
INBOUND_DOCS = ("crates/engine/README.md", "crates/path-trust/README.md", ".github/CONTRIBUTING.md", ".github/pull_request_template.md", "README.md")
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def live_lines(path: Path) -> list[str]:
    return [line for line in path.read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith(("#", "//"))]


def production_code(text: str) -> str:
    """Drop the items marked #[cfg(test)] from rustfmt-formatted Rust source: a block item (a tests module, a test-only fixture module, helper fn or impl) up to the closing brace at the attribute's indent, any other item up to the line that ends it. Everything else, including production code that follows a test-only helper, stays."""
    lines = text.splitlines()
    kept = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.strip() != "#[cfg(test)]":
            kept.append(line)
            index += 1
            continue
        indent = line[: len(line) - len(line.lstrip())]
        header = index + 1
        while header < len(lines) and lines[header].lstrip().startswith(("#[", "//")):
            header += 1
        while header < len(lines) and not lines[header].rstrip().endswith(("{", ";", ",", "}")):
            header += 1
        end = header
        if header < len(lines) and lines[header].rstrip().endswith("{"):
            end = next((closing for closing in range(header + 1, len(lines)) if lines[closing].rstrip() in (f"{indent}}}", f"{indent}}};")), None)
            if end is None:
                # 找不到同缩进的右括号就不跳过，多查一段只会多报，不会漏报。
                kept.append(line)
                index += 1
                continue
        index = end + 1
    return "\n".join(kept)


def spdx_ids(expression: str) -> set[str]:
    return {term.strip() for term in expression.split(" AND ")}


def crate_license(name: str) -> str:
    manifest = (ROOT / "crates" / name / "Cargo.toml").read_text(encoding="utf-8")
    check("license.workspace" not in manifest, f"crates/{name}/Cargo.toml inherits the workspace's GPL-3.0-only licence")
    match = re.search(r'^license = "([^"]+)"$', manifest, re.MULTILINE)
    check(match is not None, f"crates/{name}/Cargo.toml has no license field of its own")
    return match.group(1) if match else ""


def include_patterns(manifest: str) -> list[str]:
    match = re.search(r"^include = \[([^\]]*)\]$", manifest, re.MULTILINE)
    return re.findall(r'"([^"]+)"', match.group(1)) if match else []


def included(path: str, patterns: list[str]) -> bool:
    """Whether cargo's include globs take in path: `**/` spans any number of directories, a trailing `**` everything below, `*` stays within one directory."""
    for pattern in patterns:
        regex = "".join({"**/": "(?:.*/)?", "**": ".*", "*": "[^/]*"}.get(part, re.escape(part)) for part in re.split(r"(\*\*/|\*\*|\*)", pattern))
        if re.fullmatch(regex, path):
            return True
    return False


def own_version(name: str) -> str:
    manifest = (ROOT / "crates" / name / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "([^"]+)"$', manifest, re.MULTILINE)
    check(match is not None and re.search(r"^version\.workspace", manifest, re.MULTILINE) is None, f"crates/{name}/Cargo.toml inherits the workspace version instead of carrying its own")
    return match.group(1) if match else ""


def main() -> int:
    mpl = MPL_TEXT.read_text(encoding="utf-8")
    for name in ("engine", "path-trust"):
        licence = (ROOT / "crates" / name / "LICENSE").read_text(encoding="utf-8")
        check(licence.endswith(mpl), f"crates/{name}/LICENSE does not end with resources/licenses/MPL-2.0.txt verbatim")
        preamble = licence[: len(licence) - len(mpl)] if licence.endswith(mpl) else licence
        check(" ".join(EXHIBIT_A.split()) in " ".join(preamble.split()), f"crates/{name}/LICENSE does not open with the Exhibit A notice")
        check("Exhibit B is not attached" in preamble and "This Source Code Form is \"Incompatible With Secondary Licenses\"" not in preamble, f"crates/{name}/LICENSE must say Exhibit B is not attached, and must not attach it")
        first = (ROOT / "crates" / name / "src/lib.rs").read_text(encoding="utf-8").splitlines()[0]
        check(first == f"// {EXHIBIT_A}", f"crates/{name}/src/lib.rs does not open with the Exhibit A notice")

    check(crate_license("path-trust") == "MPL-2.0", "crates/path-trust/Cargo.toml must declare MPL-2.0")
    declared = spdx_ids(crate_license("engine"))
    expected = {"MPL-2.0"}.union(*EMBEDDED.values(), *ADAPTED.values())
    check(declared == expected, f"crates/engine/Cargo.toml declares {sorted(declared)}, the files inside the crate need {sorted(expected)}")
    check(not any("GPL" in term for term in declared), f"crates/engine/Cargo.toml declares a GPL or LGPL licence: {sorted(declared)}")

    source = ENGINE / "src"
    compiled = set()
    # 只看编进产品的代码：测试里读的文件（例如 diagnostics.rs 的测试读它自己的源码核对常量）不随二进制分发，不需要许可证条目。去掉测试代码的规则与下面检查 crate 外读取的那段相同。
    for path in sorted(source.rglob("*.rs")):
        if path.name.endswith("tests.rs") or "tests" in path.relative_to(source).parts[:-1]:
            continue
        for literal in re.findall(r'include_(?:str|bytes)!\(\s*"([^"]+)"', production_code(path.read_text(encoding="utf-8"))):
            target = (path.parent / literal).resolve()
            relative = target.relative_to(source.resolve()).as_posix() if target.is_relative_to(source.resolve()) else None
            # 引进来的 .rs 是本 crate 自己的源码（例如 diagnostics.rs 的测试读自身核对诊断码），归 crate 的 MPL-2.0 管，不是要登记的数据文件。
            if relative is None or relative.endswith(".rs") or any(marker in f"/{relative}" for marker in TEST_DATA):
                continue
            compiled.add(relative)
    unlisted = sorted(compiled - EMBEDDED.keys())
    check(not unlisted, f"crates/engine compiles in {unlisted}, which have no licence entry here and in crates/engine/README.md")
    readme = (ENGINE / "README.md").read_text(encoding="utf-8")
    for relative in [*EMBEDDED, *ADAPTED]:
        check((source / relative).is_file(), f"crates/engine/src/{relative} is listed but missing")
        check(f"src/{relative}" in readme or f"`{relative.rsplit('/', 1)[-1]}`" in readme, f"crates/engine/README.md does not name src/{relative}")
    # 非测试代码不许 include 到 crate 目录之外：tests 模块、#[cfg(test)] 的辅助项和 *tests.rs 里的读取由 cargo test 在仓库里跑，不进产品。只去掉 #[cfg(test)] 标着的那一项，不从第一个 #[cfg(test)] 一刀切到文件尾：octagram.rs 的测试辅助函数后面还有几百行产品代码。
    sample = "fn a() {}\n    #[cfg(test)]\n    fn helper(&self) -> usize {\n        include_str!(\"../test-only\").len()\n    }\n    fn shipped() {\n        include_str!(\"../shipped\");\n    }\n#[cfg(test)]\nmod tests {\n    const T: &str = include_str!(\"../tests-only\");\n}\n"
    check(re.findall(r'include_(?:str|bytes)!\(\s*"([^"]+)"', production_code(sample)) == ["../shipped"], "production_code() no longer keeps the production code after a #[cfg(test)] helper, or no longer drops the test-only items")
    for path in sorted(source.rglob("*.rs")):
        if path.name.endswith("tests.rs") or "tests" in path.relative_to(source).parts[:-1]:
            continue
        body = production_code(path.read_text(encoding="utf-8"))
        for literal in re.findall(r'include_(?:str|bytes)!\(\s*"([^"]+)"', body):
            target = (path.parent / literal).resolve()
            check(target.is_relative_to(ENGINE.resolve()), f"{path.relative_to(ROOT)} compiles in {literal} from outside crates/engine")

    octagram = (ROOT / "resources/licenses/librime-octagram-BSD-3-Clause.txt").read_text(encoding="utf-8")
    check("Copyright (c) 2008-2014, Susumu Yata" in octagram and "fc45469d6a4469b2c9f337b2e153983888c7826a" in octagram, "librime-octagram-BSD-3-Clause.txt lost the darts-clone terms appended to it")
    llvm = (ROOT / "resources/licenses" / LLVM_LICENCE).read_text(encoding="utf-8")
    check("---- LLVM Exceptions to the Apache 2.0 License ----" in llvm and "llvm-project/blob/4a1b95bda0c444798a5240fe924dd127b776d12d/libcxx/LICENSE.TXT" in llvm, f"{LLVM_LICENCE} is not libc++'s LICENSE.TXT with its pinned source")

    latex = (ROOT / "resources/licenses" / LATEX_LICENCE).read_text(encoding="utf-8")
    check(latex.endswith((ROOT / "plugins/phrase_table/LICENSE").read_text(encoding="utf-8")), f"{LATEX_LICENCE} does not end with plugins/phrase_table/LICENSE verbatim")

    for channel, names in CHANNELS.items():
        lines = live_lines(ROOT / channel)
        for name in names:
            check(any(name in line for line in lines), f"{channel} does not ship {name}")
    collector = (ROOT / COLLECTOR).read_text(encoding="utf-8")
    check('PROJECT_LICENSE = "GPL-3.0-only"' in collector and 'p.get("license") != PROJECT_LICENSE' in collector, f"{COLLECTOR} no longer collects the workspace members that are not GPL-3.0-only, so the engine's LICENSE drops out of rust-crates-NOTICES.txt")
    for document in INBOUND_DOCS:
        check(INBOUND in (ROOT / document).read_text(encoding="utf-8"), f"{document} does not state that contributions to the engine are licensed {INBOUND}")

    packaging = ROOT / "platforms/linux/packaging"
    spec = re.search(r"^License:\s*(.+)$", (packaging / "rpm/msime.spec").read_text(encoding="utf-8"), re.MULTILINE).group(1)
    missing = sorted(expected - spdx_ids(spec))
    check(not missing, f"rpm/msime.spec License lacks {missing}, which the engine compiles in")
    gentoo_names = {"MPL-2.0", "BSD", "BSD-2", "CC-BY-3.0", "Unicode-3.0", "Apache-2.0", "Apache-2.0-with-LLVM-exceptions", "MIT", "WTFPL-2"}
    for ebuild in ("gentoo/app-i18n/msime/msime.ebuild.in", "gentoo/app-i18n/msime/msime-9999.ebuild"):
        line = re.search(r'^LICENSE="([^"]*)"$', (packaging / ebuild).read_text(encoding="utf-8"), re.MULTILINE)
        listed = set(line.group(1).split()) if line else set()
        check(gentoo_names <= listed, f"{ebuild} LICENSE lacks {sorted(gentoo_names - listed)}")
    copyright_text = (packaging / "debian/copyright").read_text(encoding="utf-8")
    check(re.search(r"^Files: crates/engine/\* crates/path-trust/\*\n(?:.+\n)*?License: MPL-2.0$", copyright_text, re.MULTILINE) is not None, "debian/copyright has no MPL-2.0 stanza for crates/engine and crates/path-trust")
    # 录自 GPL-3.0 词库的金样和手写参考输出只给测试读，不在 MPL-2.0 之内；DEP-5 以最后一个匹配的段为准，所以这一段要排在 crates/engine/* 之后。
    fixtures = re.search(r"^Files: crates/engine/tests/golden/real/\* crates/engine/src/handwriting/testdata/\*\n(?:.+\n)*?License: GPL-3.0-only$", copyright_text, re.MULTILINE)
    check(fixtures is not None and fixtures.start() > copyright_text.index("Files: crates/engine/* crates/path-trust/*"), "debian/copyright does not keep the engine's test fixtures under GPL-3.0-only after the crates/engine/* stanza")
    nix = (ROOT / "platforms/linux/nix/fcitx5.nix").read_text(encoding="utf-8")
    # 后三项不是引擎的，是同一个派生编进或链接的 Wayland 协议代码与 Rust 依赖，rpm/msime.spec 的 License 也列着。
    for attribute in ("mpl20", "cc-by-30", "unicode-30", "bsd2", "bsd3", "wtfpl", "llvm-exception", "hpnd", "isc", "zlib"):
        check(f"lib.licenses.{attribute}\n" in nix, f"platforms/linux/nix/fcitx5.nix meta.license lacks lib.licenses.{attribute}")

    licences = ENGINE / "licenses"
    shipped = {path.name for path in licences.iterdir()} if licences.is_dir() else set()
    check(shipped == PACKAGED_LICENCES, f"crates/engine/licenses holds {sorted(shipped)}, the README table needs {sorted(PACKAGED_LICENCES)}")
    for name in sorted(shipped & PACKAGED_LICENCES):
        check((licences / name).read_bytes() == (ROOT / "resources/licenses" / name).read_bytes(), f"crates/engine/licenses/{name} differs from resources/licenses/{name}; copy the file again")
    named = set(re.findall(r"\]\(licenses/([^)]+)\)", readme))
    check(named == PACKAGED_LICENCES, f"the crates/engine/README.md table links {sorted(named)}, crates/engine/licenses holds {sorted(PACKAGED_LICENCES)}")
    engine_manifest = (ENGINE / "Cargo.toml").read_text(encoding="utf-8")
    patterns = include_patterns(engine_manifest)
    check(bool(patterns), "crates/engine/Cargo.toml has no include whitelist, so the test fixtures and examples would be published")
    for required in ["Cargo.toml", "LICENSE", "README.md", "src/lib.rs", *(f"src/{relative}" for relative in EMBEDDED), *(f"licenses/{name}" for name in PACKAGED_LICENCES)]:
        check(included(required, patterns), f"the include whitelist of crates/engine/Cargo.toml leaves out {required}")
    for excluded in UNPACKAGED:
        check(not included(excluded, patterns), f"the include whitelist of crates/engine/Cargo.toml takes in {excluded}")
    for name in ("engine", "path-trust"):
        manifest = (ROOT / "crates" / name / "Cargo.toml").read_text(encoding="utf-8")
        for field in ("readme", "keywords", "categories", "homepage", "documentation", "include"):
            check(re.search(rf"^{field} = ", manifest, re.MULTILINE) is not None, f"crates/{name}/Cargo.toml lacks the {field} crates.io shows")
        outside = re.findall(r"\]\((\.\./[^)]*)\)", (ROOT / "crates" / name / "README.md").read_text(encoding="utf-8"))
        check(not outside, f"crates/{name}/README.md links outside the crate by relative path, which breaks on crates.io: {outside}")
    path_trust = own_version("path-trust")
    own_version("engine")
    dependency = re.search(r"^msime-path-trust = \{([^}]*)\}$", engine_manifest, re.MULTILINE)
    check(dependency is not None and f'version = "{path_trust}"' in dependency.group(1), f"crates/engine/Cargo.toml must depend on msime-path-trust by version {path_trust} as well as by path, or it cannot be published")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"engine license: MPL-2.0 with {len(declared) - 1} embedded licences, {len(compiled)} compiled-in data files, {len(CHANNELS)} notice channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())

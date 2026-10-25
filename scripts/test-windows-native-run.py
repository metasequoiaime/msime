#!/usr/bin/env python3
"""Run the Windows test sources that do not actually need Windows.

Most of `platforms/windows/tests/` is policy: pure functions over contract
structs, with no Win32 call anywhere in the translation unit. On a machine with
no Windows and no Docker those tests had exactly one level of evidence - the
cross build linked them - and linking does not catch an assertion. That is the
whole gap this closes: the same sources, compiled with the host compiler and
actually executed.

Which ones qualify is discovered rather than listed. A source that compiles and
links on its own with the host compiler is one whose translation unit needs
nothing from Windows; anything that does not is skipped and stays covered by the
cross build alone. Nothing has to be added here when a test is added.

The sources in `HOST_DIFFERENCES` compile but do not pass here, or do not compile here for a reason that is the host's rather than the code's, and they are exclusions with reasons rather than failures. Every other failure is reported: these are the same assertions the Windows build would make.

A source that stops at a missing header is skipped only when the header is not in this repository - a Windows SDK or system header. A header the repository does have means the include path here is wrong, and skipping it would drop a portable test from the count without anyone noticing, which is how two of the TIP's tests went unrun.
"""

from __future__ import annotations

import functools
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = pathlib.Path(__file__).resolve().parent.parent
TESTS = ROOT / "platforms/windows/tests"
# The TIP has its own tests, under its own directory, and most of them are the same kind of thing:
# policy over contract structs with no Win32 call in the translation unit. They were outside this
# runner only because it looked in one place, so the DLL's own decisions - which Enter commits, how
# a candidate is owned - had the cross build as their only evidence here.
TSF_TESTS = ROOT / "platforms/windows/tsf/tests"
TEST_ROOTS = [TESTS, TSF_TESTS]
SRC = ROOT / "platforms/windows/src"

# Sources that build here but cannot pass here. Each is a property of the host,
# not of the code, and each is still executed by the cross build's own runners.
HOST_DIFFERENCES = {
    "runtime/aux_message.cpp": (
        "wchar_t is 4 bytes here and 2 on Windows, so the wire bytes a test "
        "builds from a wide literal are a different shape and the "
        "embedded-NUL case cannot be expressed"
    ),
    "ui/shell_surfaces.cpp": "asserts on Windows path and environment-block semantics",
}

# Sources whose `main` takes an argument the build system supplies.
ARGUMENTS = {
    "input/wubi_auto_commit.cpp": ["platforms/windows/tests/input/fixtures/msime-wubi-commit.db"],
    "input/wubi_commit_owner.cpp": ["platforms/windows/tests/input/fixtures/msime-wubi-commit-owner.db"],
    "input/backspace_selected_phrase.cpp": ["platforms/windows/tests/input/fixtures/msime-phrase-backspace.db"],
    "input/host_online_candidates.cpp": ["platforms/windows/tests/input/fixtures/msime-pinyin-online.db"],
    "core/installer_launch.cpp": ["platforms/windows/installer/msime_setup.iss"],
    "input/zhuyin_keys.cpp": ["platforms/windows/tests/input/fixtures/msime-zhuyin.db"],
    "input/stroke_keys.cpp": ["platforms/windows/tests/input/fixtures/msime-stroke.db"],
}

# 这些探针必须由专用系统验收提供真实产物，不能无参执行冒充单元测试。
BUILD_DRIVEN = {
    "runtime/windows7_import_load.cpp": "需要同架构产品目录、导入清单和隔离的插件冷装载子进程",
}

# The three online workers are the exception to "one translation unit": their
# class bodies live in a .cpp of their own, so a test of the queueing above them
# links that file too. Every one of those files reaches the shared host library
# for its request building, which is why this is a named list rather than a rule
# - linking arbitrary sources would drag the whole Windows build in. When the
# host library has not been built here the three are skipped exactly like any
# other source that needs more than itself.
# Values are (sources relative to platforms/windows, whether they need the shared host library).
COMPANIONS: dict[str, tuple[list[str], bool]] = {
    # 两份真实会话共同验证五笔自动上屏的文档写入归属。
    "input/wubi_auto_commit.cpp": (
        ["src/ipc/ReplyComposer.cpp", "src/ipc/ReplyCodec.cpp", "src/ipc/ServerSession.cpp",
         "src/input/ChineseTextConversion.cpp", "tsf/EngineSessionAdapter.cpp", "tsf/EngineResponse.cpp"],
        True,
    ),
    "input/wubi_commit_owner.cpp": (
        ["src/ipc/ReplyComposer.cpp", "src/ipc/ReplyCodec.cpp", "src/ipc/ServerSession.cpp",
         "src/input/ChineseTextConversion.cpp", "tsf/EngineSessionAdapter.cpp", "tsf/EngineResponse.cpp"],
        True,
    ),
    # Windows 的两份真实会话共同验证部分选词、普通退格与实时设置。
    "input/backspace_selected_phrase.cpp": (
        ["src/ipc/ReplyComposer.cpp", "src/ipc/ReplyCodec.cpp", "src/ipc/ServerSession.cpp",
         "src/input/ChineseTextConversion.cpp", "tsf/EngineSessionAdapter.cpp", "tsf/EngineResponse.cpp"],
        True,
    ),
    "candidate/cloud_candidate_worker.cpp": (["src/candidate/CloudCandidateWorker.cpp"], True),
    "candidate/ai_candidate_worker.cpp": (["src/candidate/AiCandidateWorker.cpp"], True),
    "candidate/translation_worker.cpp": (["src/candidate/TranslationWorker.cpp"], True),
    "core/uiless_candidates.cpp": (["src/ipc/ReplyCodec.cpp"], False),
    # 候选信箱发布页面时做繁简投影，需要真实的转换实现。
    "ui/candidate_render_request.cpp": (["src/input/ChineseTextConversion.cpp"], True),
    "core/candidate_continuation_pipe.cpp": (["src/ipc/ReplyCodec.cpp"], False),
    # 两份真实引擎会话通过外部候选选择继续组字。
    "input/host_online_candidates.cpp": (
        ["src/ipc/ServerSession.cpp", "src/input/ChineseTextConversion.cpp", "tsf/EngineSessionAdapter.cpp", "tsf/EngineResponse.cpp"], True,
    ),
    "input/candidate_continuation.cpp": (
        ["src/ipc/ReplyComposer.cpp", "src/ipc/ReplyCodec.cpp", "src/ipc/ServerSession.cpp",
         "src/input/ChineseTextConversion.cpp", "tsf/EngineSessionAdapter.cpp", "tsf/EngineResponse.cpp"],
        True,
    ),
    # Ctrl+Enter translation commit, through the reply composer against a real Engine session.
    "input/candidate_translation_commit.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # The Server side of the Korean scheme and its Hanja list, through the reply composer against a real Engine session.
    "input/korean_keys.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # The Server side of the Zhuyin scheme and its candidate list, against a real Engine session and a checked-in dictionary.
    "input/zhuyin_keys.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # The Server side of the Stroke scheme, against a real Engine session and a checked-in synthetic dictionary.
    "input/stroke_keys.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # The Server side of the Vietnamese scheme, against a real Engine session.
    "input/vietnamese_keys.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # 藏文方案的 Server 端，对真实 Engine 会话运行。
    "input/tibetan_keys.cpp": (
        [
            "src/ipc/ReplyComposer.cpp",
            "src/ipc/ReplyCodec.cpp",
            "src/ipc/ServerSession.cpp",
            "src/input/ChineseTextConversion.cpp",
        ],
        True,
    ),
    # Book-title nesting paid back after the TSF auto-closes, against a real Engine session.
    "input/paired_punctuation_balance.cpp": (
        ["src/ipc/ServerSession.cpp", "src/ipc/ReplyCodec.cpp", "src/input/ChineseTextConversion.cpp"],
        True,
    ),
    # The TIP's reply parser, which its own policy tests call. No host library: this is JSON in,
    # struct out.
    "tsf/input/raw_commit.cpp": (["tsf/EngineResponse.cpp"], False),
    "tsf/input/host_composition.cpp": (["tsf/EngineResponse.cpp"], False),
    # 按键分类快照读取宿主视图，需要链接同一个 JSON 解析器。
    "tsf/input/key_composition_state.cpp": (["tsf/EngineResponse.cpp"], False),
    "tsf/input/engine_response.cpp": (["tsf/EngineResponse.cpp"], False),
}
COMPANION_LIBRARIES = ["-lcurl", "-lsqlite3"]
COMPANION_FRAMEWORKS = [
    "CoreFoundation",
    "CoreText",
    "CoreGraphics",
    "CoreAudio",
    "AudioToolbox",
    "Security",
    "SystemConfiguration",
]


def host_library() -> pathlib.Path | None:
    """The shared host library the worker sources link against, if it is built."""
    for profile in ("debug", "release"):
        candidate = ROOT / "target" / profile / "libmsime_host_api.a"
        if candidate.exists():
            return candidate
    return None


def companion_flags(relative: str) -> list[str] | None:
    """Extra compiler arguments for a source that needs another translation unit, or None."""
    entry = COMPANIONS.get(relative)
    if not entry:
        return []
    companions, needs_library = entry
    windows = ROOT / "platforms/windows"
    flags = [str(windows / name) for name in companions]
    if not needs_library:
        return flags
    library = host_library()
    if library is None:
        return None
    flags += [str(library), *COMPANION_LIBRARIES]
    if sys.platform == "darwin":
        for framework in COMPANION_FRAMEWORKS:
            flags += ["-framework", framework]
    return flags

EXTRA_INCLUDES = ["/opt/homebrew/include", "/usr/local/include"]


def cmake_sources() -> set[str]:
    """Every test source the Windows build actually compiles.

    A file under `tests/` that no CMakeLists mentions is not a test of this
    build - `voice/voice_wire_peer.cpp` is a subprocess fixture driven by a Rust
    interop test, and running it as though it were a test reports a pass for
    something that never asserted anything.
    """
    mentioned: set[str] = set()
    for lists in sorted((ROOT / "platforms/windows").rglob("CMakeLists.txt")):
        text = lists.read_text(encoding="utf-8")
        for source in sources():
            relative = source.relative_to(ROOT / "platforms/windows").as_posix()
            if relative in text or source.name in text:
                mentioned.add(key_for(source))
    return mentioned


def sources() -> list[pathlib.Path]:
    """Every test source under either root, in a stable order."""
    return sorted(path for root in TEST_ROOTS for path in root.rglob("*.cpp"))


def key_for(source: pathlib.Path) -> str:
    """The name a source is known by here: relative to its own root, prefixed for the TIP's."""
    if TSF_TESTS in source.parents:
        return f"tsf/{source.relative_to(TSF_TESTS).as_posix()}"
    return source.relative_to(TESTS).as_posix()


def include_flags() -> list[str]:
    # Every source directory, the way the Windows build exposes them, plus the
    # contract and host headers. Derived so a new subdirectory needs no edit.
    directories = [SRC, *(path for path in sorted(SRC.iterdir()) if path.is_dir())]
    # The DLL/Server contract headers, which the Windows build puts on the global include path. Ahead of the TIP's directories, so the forwarding tsf/IPC/KeyEventSendResult.h does not shadow the definition it forwards to.
    directories += [ROOT / "platforms/windows/common"]
    directories += [TESTS / "core", ROOT / "shared/contracts", ROOT / "crates/host-api/include"]
    # Windows 宿主会继续解析到公共绘制头文件；与 CMake 的公开 include 路径一致。
    directories += [ROOT / "platforms/windows/msimeui/include"]
    # The TIP's own headers, for its own tests: its root and each of its source directories, which its CMakeLists hands to individual tests (`IPC` for passthrough statistics, `Global` for the smart punctuation fingerprint). Last, so a header both trees have still resolves to the host's copy as before.
    tsf = ROOT / "platforms/windows/tsf"
    directories += [tsf]
    if tsf.exists():
        directories += [path for path in sorted(tsf.iterdir()) if path.is_dir() and path.name != "tests"]
    flags = [f"-I{path}" for path in directories if path.exists()]
    flags += [f"-I{path}" for path in EXTRA_INCLUDES if pathlib.Path(path).exists()]
    # shared/contracts/msime_edition.h 要求构建选定一个版本；Windows 的 CMake 缺省编 full，这里照做。
    flags += ["-DMSIME_EDITION_FULL", "-DNOMINMAX"]
    if sys.platform == "win32":
        # 与原生 CMake 相同，避免 Windows SDK 的窄字符别名改变被测源码。
        flags += ["-DUNICODE", "-D_UNICODE"]
    return flags


# clang's `fatal error: 'X' file not found` and GCC's `fatal error: X: No such file or directory`.
MISSING_HEADER = re.compile(r"fatal error: '?([^':\s]+)'?:? (?:file not found|No such file or directory)")


@functools.lru_cache(maxsize=1)
def repository_files() -> tuple[str, ...]:
    listed = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True)
    return tuple(listed.stdout.splitlines())


def repository_header(header: str) -> str | None:
    """The tracked file an `#include` of this name would have found, if the repository has one."""
    return next(
        (path for path in repository_files() if path == header or path.endswith(f"/{header}")),
        None,
    )


def accepts_declspec(driver: str) -> bool:
    """这个 C++ 驱动认不认 -fdeclspec。"""
    if shutil.which(driver) is None:
        return False
    probe = subprocess.run(
        [driver, "-std=c++17", "-w", "-fdeclspec", "-fsyntax-only", "-x", "c++", "-"],
        input="int main() { return 0; }\n",
        capture_output=True,
        text=True,
    )
    return probe.returncode == 0


@functools.lru_cache(maxsize=1)
def compiler() -> str | None:
    """用来编译这些源文件的宿主 C++ 驱动，没有合适的返回 None。

    -fdeclspec 是 clang 的选项。这个 runner 写在 macOS 上，那里的 `c++` 就是 clang，
    但多数 Linux 发行版上它是 GCC，而 GCC 直接拒绝这个选项——于是每个源文件都停在第一
    行，整套测试被报成「全坏了」，正是这个 runner 存在的意义的反面。`c++` 是谁不该靠
    猜：探一次这个选项，默认驱动不认就改用 clang++。
    """
    for driver in (os.environ.get("CXX"), "c++", "clang++"):
        if driver and accepts_declspec(driver):
            return driver
    return None


def build(
    source: pathlib.Path, flags: list[str], workspace: pathlib.Path
) -> tuple[pathlib.Path | None, str]:
    """(executable, reason). A null executable with a reason is a failure to report."""
    relative = key_for(source)
    if relative in BUILD_DRIVEN:
        return None, ""
    binary = workspace / relative.replace("/", "_").removesuffix(".cpp")
    companions = companion_flags(relative)
    if companions is None:
        return None, ""
    environment = os.environ.copy()
    if sys.platform == "darwin":
        # 这些 Windows 策略测试在当前主机运行，不继承 macOS 产品的最低系统版本。
        # 例如浮点 `to_chars` 在 macOS 13.3 才可用，产品的 13.0 目标不应限制宿主测试。
        environment.pop("MACOSX_DEPLOYMENT_TARGET", None)
    # Compiling is CPU-bound and safe to do many at once.
    compiled = subprocess.run(
        # -fdeclspec: these sources are written for MSVC, and a `__declspec(dllexport)` on a
        # function is not something to work around - clang accepts it behind this flag, and
        # without it a source that is otherwise perfectly portable stops at its first line.
        # -std=c++17: the language the Windows build compiles these sources in (CMAKE_CXX_STANDARD in
        # platforms/windows/CMakeLists.txt). Under C++20 `path::u8string()` returns std::u8string
        # rather than std::string, so a fixture that puts one in a JSON document gets an array of
        # character codes and the code under test sees a type error the Windows build never has.
        [compiler(), "-std=c++17", "-w", "-fdeclspec", *flags, "-o", str(binary), str(source), *companions],
        capture_output=True,
        text=True,
        env=environment,
    )
    if compiled.returncode == 0:
        return binary, ""
    # Two ordinary reasons a test source does not build on its own here, both
    # of them "the Windows build owns this one":
    #   - it reaches for a Windows header, one this repository does not have;
    #   - it needs the other translation units the Windows build links it with,
    #     which shows up as undefined symbols.
    # Anything else is code that does not compile, and letting that quietly
    # leave the count is exactly how a suite goes missing - the failure this
    # runner exists to stop happening, so it is reported rather than skipped.
    ordinary = (
        "file not found",
        "No such file or directory",
        "Undefined symbols",
        "undefined reference",
    )
    # MSVC 链接器也会把诊断写到 stdout；错误码不受系统显示语言影响。
    diagnostic = compiled.stdout + compiled.stderr
    missing = MISSING_HEADER.search(diagnostic)
    if missing:
        found = repository_header(missing.group(1))
        if found:
            return None, f"cannot find {missing.group(1)}, which this repository has at {found}: the include path here is missing its directory"
    if any(marker in diagnostic for marker in ordinary) or re.search(
        r"\berror LNK(?:2001|2019|1120|1561):", diagnostic
    ):
        return None, ""
    # LLVM 在 Windows 上的同类链接错误也表示缺少独立翻译单元的依赖。
    if sys.platform == "win32" and any(
        line.startswith(("lld-link: error: undefined symbol:",
                         "lld-link: error: subsystem must be defined"))
        for line in diagnostic.splitlines()
    ):
        return None, ""
    if relative in HOST_DIFFERENCES:
        return None, ""
    first = next(
        (line for line in diagnostic.splitlines() if "error" in line),
        "did not compile",
    )
    return None, first[:200]


def execute(source: pathlib.Path, binary: pathlib.Path) -> tuple[str, str]:
    """Returns (outcome, detail) where outcome is passed / failed / excluded."""
    relative = key_for(source)
    if relative in HOST_DIFFERENCES:
        return "excluded", HOST_DIFFERENCES[relative]
    try:
        run = subprocess.run(
            [str(binary), *ARGUMENTS.get(relative, [])],
            capture_output=True,
            text=True,
            timeout=120,
            cwd=ROOT,
            # Never the caller's stdin. A fixture that reads frames from it
            # blocks forever on whatever the gate happened to be started with,
            # which is how this runner first hung.
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "failed", "timed out"
    if run.returncode == 0:
        return "passed", ""
    detail = (run.stderr.strip() or run.stdout.strip() or "no output").splitlines()
    return "failed", detail[0][:200] if detail else "no output"


def main() -> int:
    if compiler() is None:
        # 有编译器但它不认 -fdeclspec，与根本没有编译器是两回事，说清楚是哪一种，并把
        # 取得它的办法打出来——「跳过」不该让人以为这台机器已经覆盖到了。
        print("skipped: no host C++ compiler that accepts -fdeclspec"
              if shutil.which("c++") else "skipped: no host C++ compiler")
        print("  install clang to enable this gate here")
        return 0
    if not TESTS.exists():
        print("skipped: the Windows tests are not present")
        return 0
    flags = include_flags()
    built_by_cmake = cmake_sources()
    candidates = sources()
    selected = [source for source in candidates if key_for(source) in built_by_cmake]
    fixtures = len(candidates) - len(selected)
    with tempfile.TemporaryDirectory() as directory:
        workspace = pathlib.Path(directory)
        with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
            builds = list(pool.map(lambda source: build(source, flags, workspace), selected))
        # Run one at a time. Some of these wait on their own timers - a worker's
        # debounce window, a lease deadline - and a gate that reds because the
        # machine was busy is a gate people learn to skip. Serial execution costs
        # a couple of seconds and removes the whole class.
        outcomes = [
            (
                source,
                *(
                    execute(source, binary)
                    if binary
                    else (("failed", reason) if reason else ("skipped", ""))
                ),
            )
            for source, (binary, reason) in zip(selected, builds)
        ]

    failures = [
        (source.relative_to(ROOT).as_posix(), detail)
        for source, outcome, detail in outcomes
        if outcome == "failed"
    ]
    counts = {name: 0 for name in ("passed", "failed", "skipped", "excluded")}
    for _, outcome, _ in outcomes:
        counts[outcome] += 1

    for name, detail in failures:
        print(f"FAIL {name}: {detail}", file=sys.stderr)
    if failures:
        print(
            "\nThese are the assertions the Windows build makes, running here. A failure that is "
            "a property of this host rather than of the code belongs in HOST_DIFFERENCES with the "
            "reason written out.",
            file=sys.stderr,
        )
        return 1
    print(
        f"windows native run: {counts['passed']} passed, "
        f"{counts['excluded']} excluded by host differences, "
        f"{counts['skipped']} need the Windows build, "
        f"{fixtures} not tests"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

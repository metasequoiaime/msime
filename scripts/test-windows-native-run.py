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
    "runtime/session_pump.cpp": (
        "does not compile here: it takes a path's u8string(), which is "
        "std::u8string, where the Windows build's own conversion applies"
    ),
    "runtime/session_smoke.cpp": (
        "does not compile here: it calls preference_monitor_tests, declared "
        "only for the Windows build"
    ),
}

# Sources whose `main` takes an argument the build system supplies.
ARGUMENTS = {
    "core/installer_launch.cpp": ["platforms/windows/installer/msime_setup.iss"],
    "input/zhuyin_keys.cpp": ["platforms/windows/tests/input/fixtures/zhuyin.db"],
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
    "candidate/cloud_candidate_worker.cpp": (["src/candidate/CloudCandidateWorker.cpp"], True),
    "candidate/ai_candidate_worker.cpp": (["src/candidate/AiCandidateWorker.cpp"], True),
    "candidate/translation_worker.cpp": (["src/candidate/TranslationWorker.cpp"], True),
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
    # Book-title nesting paid back after the TSF auto-closes, against a real Engine session.
    "input/paired_punctuation_balance.cpp": (
        ["src/ipc/ServerSession.cpp", "src/ipc/ReplyCodec.cpp", "src/input/ChineseTextConversion.cpp"],
        True,
    ),
    # The TIP's reply parser, which its own policy tests call. No host library: this is JSON in,
    # struct out.
    "tsf/input/raw_commit.cpp": (["tsf/EngineResponse.cpp"], False),
    "tsf/input/engine_response.cpp": (["tsf/EngineResponse.cpp"], False),
}
COMPANION_LIBRARIES = ["-lcurl", "-lsqlite3"]
COMPANION_FRAMEWORKS = [
    "CoreFoundation",
    "CoreText",
    "CoreGraphics",
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
    # The TIP's own headers, for its own tests: its root and each of its source directories, which its CMakeLists hands to individual tests (`IPC` for passthrough statistics, `Global` for the smart punctuation fingerprint). Last, so a header both trees have still resolves to the host's copy as before.
    tsf = ROOT / "platforms/windows/tsf"
    directories += [tsf]
    if tsf.exists():
        directories += [path for path in sorted(tsf.iterdir()) if path.is_dir() and path.name != "tests"]
    flags = [f"-I{path}" for path in directories if path.exists()]
    flags += [f"-I{path}" for path in EXTRA_INCLUDES if pathlib.Path(path).exists()]
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
        [driver, "-std=c++20", "-w", "-fdeclspec", "-fsyntax-only", "-x", "c++", "-"],
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
    binary = workspace / relative.replace("/", "_").removesuffix(".cpp")
    companions = companion_flags(relative)
    if companions is None:
        return None, ""
    # Compiling is CPU-bound and safe to do many at once.
    compiled = subprocess.run(
        # -fdeclspec: these sources are written for MSVC, and a `__declspec(dllexport)` on a
        # function is not something to work around - clang accepts it behind this flag, and
        # without it a source that is otherwise perfectly portable stops at its first line.
        [compiler(), "-std=c++20", "-w", "-fdeclspec", *flags, "-o", str(binary), str(source), *companions],
        capture_output=True,
        text=True,
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
    missing = MISSING_HEADER.search(compiled.stderr)
    if missing:
        found = repository_header(missing.group(1))
        if found:
            return None, f"cannot find {missing.group(1)}, which this repository has at {found}: the include path here is missing its directory"
    if any(marker in compiled.stderr for marker in ordinary):
        return None, ""
    if relative in HOST_DIFFERENCES:
        return None, ""
    first = next(
        (line for line in compiled.stderr.splitlines() if "error" in line),
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

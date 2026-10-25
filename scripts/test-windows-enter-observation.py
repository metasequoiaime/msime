#!/usr/bin/env python3
"""编译执行生产回车观测片段，核对它交给 Server 的原文来源。"""
from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARNESS = r'''
#include "KeyCompositionState.h"
#include <codecvt>
#include <cstdio>
#include <locale>

using UINT = unsigned;
constexpr UINT VK_RETURN = 13;
msime::tsf::EngineResult snapshot;
bool parseOK = true;
namespace msime::tsf {
bool EngineSessionAdapter::parse_result(const std::string &, EngineResult *out, std::string *) {
    if (!parseOK) return false;
    *out = snapshot;
    return true;
}
}
namespace FanyUtils {
std::wstring string_to_wstring(const std::string &text) {
    return std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>>{}.from_bytes(text);
}
}
namespace GlobalIme { std::wstring word_for_creating_word; }
struct Buffer {
    std::wstring text;
    const wchar_t *Get() const { return text.data(); }
    std::size_t GetLength() const { return text.size(); }
};
using CStringRange = Buffer;
struct Host {
    bool active = true, readable = true;
    unsigned reads = 0;
    bool valid() const { return active; }
    bool view(std::string *out, std::string *) {
        ++reads;
        *out = "synthetic-snapshot";
        return readable;
    }
};
struct Engine {
    Buffer legacy;
    Host host;
    const Buffer &GetKeystrokeBuffer() const { return legacy; }
    Host *GetHostEngineAdapter() { return &host; }
};
bool composing = true;
bool _IsComposing() { return composing; }
std::wstring observation(Engine *_pCompositionProcessorEngine, UINT code = VK_RETURN) {
// 生产观测片段
    return localCommitObservation;
}
int failures = 0, checks = 0;
void check(bool ok, const char *message) {
    ++checks;
    if (!ok) { std::fprintf(stderr, "FAIL: %s\n", message); ++failures; }
}
void reset() {
    snapshot = {};
    snapshot.view.editing_text = "synthetic";
    snapshot.view.preedit = "display text with separators";
    parseOK = composing = true;
    GlobalIme::word_for_creating_word.clear();
}
int main() {
    reset();
    {
        Engine engine;
        check(observation(&engine) == L"synthetic", "empty legacy buffer must not omit the host raw composition");
    }
    {
        Engine engine; engine.legacy.text = L"stale";
        check(observation(&engine) == L"synthetic", "stale legacy buffer must not replace the host raw composition");
    }
    {
        Engine engine; engine.legacy.text = L"old";
        snapshot.view.phrase_prefix = "合成";
        GlobalIme::word_for_creating_word = L"旧词段";
        check(observation(&engine) == L"合成synthetic", "prefix and raw text must come from one host view");
        check(engine.host.reads == 1, "one observation reads one host snapshot");
    }
    reset();
    for (const bool readable : {false, true}) {
        Engine engine; engine.legacy.text = L"stale";
        engine.host.readable = readable; parseOK = false;
        check(observation(&engine).empty(), "host read or parse failure must not resurrect stale legacy text");
    }
    reset();
    for (const std::size_t length : {std::size_t{127}, std::size_t{128}}) {
        Engine engine;
        snapshot.view.editing_text.assign(length, 'x');
        check(observation(&engine).size() == (length == 127 ? 127 : 0), "observation uses the existing wire bound without truncation");
    }
    reset();
    {
        Engine engine;
        snapshot.view.editing_text.assign(125, 'x');
        snapshot.view.editing_text += "😀";
        check(observation(&engine).size() == 127, "wire length is measured in UTF-16 code units");
        snapshot.view.editing_text.insert(0, "x");
        check(observation(&engine).empty(), "surrogate pair cannot bypass the wire limit");
    }
    reset();
    {
        Engine engine; engine.host.active = false;
        engine.legacy.text = L"legacy";
        GlobalIme::word_for_creating_word = L"合成";
        check(observation(&engine) == L"合成legacy", "profiles without a host retain legacy raw text and prefix");
        check(engine.host.reads == 0, "legacy path does not query an invalid host");
    }
    reset();
    {
        Engine engine; snapshot.view.editing_text.clear();
        check(observation(&engine).empty(), "empty host state remains empty");
        snapshot.view.phrase_prefix = "合成";
        check(observation(&engine) == L"合成", "prefix-only host composition is preserved");
        composing = false;
        check(observation(&engine).empty(), "idle Enter carries no composition observation");
        composing = true;
        check(observation(&engine, 'A').empty(), "ordinary keys carry no Enter observation");
        check(observation(nullptr).empty(), "missing engine carries no observation");
    }
    std::printf("production Enter observation: %d checks, %d failures\n", checks, failures);
    return failures ? 1 : 0;
}
'''


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if not compiler:
        print("skipped: no C++ compiler for the Enter observation model")
        return 0
    source = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text(encoding="utf-8")
    start = source.index("        std::wstring localCommitObservation;")
    end = source.index("        // CandidateActive", start)
    harness = HARNESS.replace("// 生产观测片段", source[start:end])
    with tempfile.TemporaryDirectory(prefix="msime-enter-observation-") as temporary:
        directory = pathlib.Path(temporary)
        cpp, binary = directory / "observation.cpp", directory / "observation"
        cpp.write_text(harness, encoding="utf-8")
        built = subprocess.run([
            compiler, "-std=c++17", "-DMSIME_EDITION_FULL=1", "-Wall", "-Wextra", "-Werror",
            "-Wno-deprecated-declarations", "-I", str(ROOT / "platforms/windows/tsf"),
            "-I", str(ROOT / "crates/host-api/include"), str(cpp), "-o", str(binary),
        ], check=False, timeout=60)
        if built.returncode:
            return built.returncode
        return subprocess.run([str(binary)], check=False, timeout=30).returncode


if __name__ == "__main__":
    raise SystemExit(main())

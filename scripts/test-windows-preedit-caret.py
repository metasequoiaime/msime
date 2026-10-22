#!/usr/bin/env python3
"""执行生产预编辑绘制方法，核对宿主快照与 TSF 选区的接线。"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

HARNESS = r'''
#include <algorithm>
#include <atomic>
#include <codecvt>
#include <cstdio>
#include <cstdlib>
#include <cwchar>
#include <limits>
#include <locale>
#include <string>
#include <string_view>
#include <vector>
#include "KeyCompositionState.h"
#include "Composition/PreeditCaret.h"
#include "shared/input/CompositionDisplay.h"

#define _In_
using BOOL = int;
using HRESULT = int;
using UINT = unsigned;
using LONG = int;
using DWORD_PTR = std::size_t;
using TfEditCookie = int;
constexpr int TRUE = 1, FALSE = 0, S_OK = 0, E_FAIL = -1;
constexpr int TF_ANCHOR_START = 0, TF_ANCHOR_END = 1, TF_AE_NONE = 0;
constexpr int FANY_IME_NO_REQUEST_ID = 0, CP_UTF8 = 65001, MB_ERR_INVALID_CHARS = 8;
constexpr int ERROR_BROKEN_PIPE = 109;
bool SUCCEEDED(HRESULT hr) { return hr >= 0; }
bool FAILED(HRESULT hr) { return hr < 0; }
HRESULT HRESULT_FROM_WIN32(int) { return E_FAIL; }

namespace FanyUtils {
std::wstring string_to_wstring(const std::string &text) {
    // 即使 wchar_t 为 32 位，模型也使用 Windows 的 UTF-16 单元。
    return std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>>{}.from_bytes(text);
}
}
int MultiByteToWideChar(int, int, const char *text, int length, wchar_t *out, int capacity) {
    const auto wide = FanyUtils::string_to_wstring(std::string(text, static_cast<std::size_t>(length)));
    if (out) {
        if (capacity < static_cast<int>(wide.size())) return 0;
        std::copy(wide.begin(), wide.end(), out);
    }
    return static_cast<int>(wide.size());
}

namespace Global {
std::atomic<int> InputModeScheme{msime::windows::scheme::Quanpin};
bool IsUiLessMode() { return false; }
enum class DataFromServerMsgType { TransportUnavailable, UiLessComposition, Preedit };
}
namespace GlobalIme { std::wstring word_for_creating_word, pending_create_word_preedit; }
namespace GlobalSettings {
namespace TsfPreeditStyle {
const std::string Empty = "empty", Raw = "raw", Pinyin = "pinyin";
}
std::string style = TsfPreeditStyle::Raw;
const std::string &getTsfPreeditStyle() { return style; }
}
namespace FanyImeUiLessPage {
struct Payload { std::wstring preedit; bool hasPage = false; };
Payload Parse(const wchar_t *text) { return {text, false}; }
}
struct FanyImeNamedpipeDataToTsf {
    Global::DataFromServerMsgType msg_type = Global::DataFromServerMsgType::Preedit;
    const wchar_t *candidate_string = L"mo'xing";
} pipeReply;
FanyImeNamedpipeDataToTsf *TryReadDataFromServerPipeWithTimeout(uint64_t, bool) { return &pipeReply; }
struct PerfTimer { double ElapsedMs() const { return 0; } };

struct CStringRange {
    const wchar_t *text = L"";
    std::size_t size = 0;
    void Set(const wchar_t *value, std::size_t length) { text = value; size = length; }
    std::size_t GetLength() const { return size; }
    std::wstring ToWString() const { return {text, size}; }
};
template<class T> struct CMetasequoiaImeArray {
    std::vector<T> rows;
    T *Append() { rows.emplace_back(); return &rows.back(); }
    T *GetAt(std::size_t index) { return &rows.at(index); }
    std::size_t Count() const { return rows.size(); }
    void Clear() { rows.clear(); }
};
struct CCandidateListItem { bool _EngineHighlighted = false; };
struct Presenter {
    void _ApplyUiLessPayload(const FanyImeUiLessPage::Payload &) {}
    void _ClearList() {}
    void _SetText(CMetasequoiaImeArray<CCandidateListItem> *, BOOL) {}
    void _SetSelection(int) {}
    void _NotifyUiLessHost() {}
};

msime::tsf::EngineResult currentResult;
bool parseOK = true;
bool msime::tsf::EngineSessionAdapter::parse_result(const std::string &, EngineResult *result, std::string *) {
    *result = currentResult;
    return parseOK;
}
struct Host {
    bool active = true, readable = true;
    unsigned reads = 0;
    bool valid() const { return active; }
    bool view(std::string *raw, std::string *) { ++reads; *raw = "synthetic-view"; return readable; }
};
struct CCompositionProcessorEngine {
    Host host;
    std::wstring legacy, rendered;
    std::size_t caret = 0, renderedPrefix = 0;
    Host *GetHostEngineAdapter() { return &host; }
    void GetReadingStrings(CMetasequoiaImeArray<CStringRange> *out, BOOL *wildcard) {
        *wildcard = FALSE;
        if (!legacy.empty()) out->Append()->Set(legacy.c_str(), legacy.size());
    }
    CStringRange GetKeystrokeBuffer() { return {legacy.c_str(), legacy.size()}; }
    std::size_t GetCaretPosition() const { return caret; }
    void SetRenderedPreedit(std::wstring text, std::size_t prefix) { rendered = std::move(text); renderedPrefix = prefix; }
    void GetCandidateList(CMetasequoiaImeArray<CCandidateListItem> *, BOOL, BOOL) {}
};

// 模型只负责记录生产方法对 TSF 范围和选区的调用，不替代真实 COM 或编辑器。
struct ITfRange {
    LONG start = 2, end = 2;
    void Collapse(TfEditCookie, int anchor) { if (anchor == TF_ANCHOR_START) end = start; else start = end; }
    void ShiftEnd(TfEditCookie, LONG shift, LONG *shifted, void *) { end += shift; *shifted = shift; }
    void Release() {}
};
struct TF_SELECTION {
    ITfRange *range = nullptr;
    struct { int ase = 0; BOOL fInterimChar = FALSE; } style;
};
struct ITfContext {
    std::wstring document = L"4M";
    LONG selection = 2;
    unsigned selections = 0;
    HRESULT SetSelection(TfEditCookie, int count, TF_SELECTION *value) {
        if (count != 1) std::abort();
        selection = value->range->start;
        ++selections;
        return S_OK;
    }
};
struct Composition {
    ITfRange range;
    HRESULT GetRange(ITfRange **out) { *out = &range; return S_OK; }
};
struct CMetasequoiaIME {
    Composition composition;
    Composition *_pComposition = &composition;
    Presenter *_pCandidateListUIPresenter = nullptr;
    unsigned writes = 0;
    bool writeOK = true;
    HRESULT _AddComposingAndChar(TfEditCookie, ITfContext *context, CStringRange *text) {
        if (!writeOK) return E_FAIL;
        ++writes;
        context->document = L"4M" + text->ToWString();
        composition.range = {2, static_cast<LONG>(context->document.size())};
        return S_OK;
    }
    HRESULT _CreateAndStartCandidate(CCompositionProcessorEngine *, TfEditCookie, ITfContext *) { std::abort(); }
    void _DeleteCandidateList(BOOL, ITfContext *) { std::abort(); }
    HRESULT _HandleCompositionInputWorker(CCompositionProcessorEngine *, TfEditCookie, ITfContext *, uint64_t);
};

// 生产光标映射包装
// 生产完整绘制方法

void check(bool condition, const char *message) {
    if (!condition) { std::fprintf(stderr, "FAIL: %s\n", message); std::exit(1); }
}
struct Fixture {
    CCompositionProcessorEngine engine;
    CMetasequoiaIME ime;
    ITfContext context;
    Fixture() {
        currentResult = {};
        parseOK = true;
        Global::InputModeScheme = msime::windows::scheme::Quanpin;
        GlobalSettings::style = GlobalSettings::TsfPreeditStyle::Raw;
        GlobalIme::word_for_creating_word.clear();
        GlobalIme::pending_create_word_preedit.clear();
    }
    void hostText(const std::string &text, std::size_t caret) {
        currentResult.view.editing_text = text;
        currentResult.view.preedit = text;
        currentResult.view.caret = caret;
    }
    HRESULT render(uint64_t request = 0) {
        engine.host.reads = 0;
        const auto result = ime._HandleCompositionInputWorker(&engine, 1, &context, request);
        check(!engine.host.active || engine.host.reads == 1, "one authoritative host view per render");
        return result;
    }
};
int main() {
    // 已提交英文后立即逐键续打；旧缓冲为空或仍残留另一段原文都不能决定新选区。
    for (const auto &stale : {std::wstring{}, std::wstring(L"stale")}) {
        Fixture f;
        f.engine.legacy = stale;
        f.engine.caret = stale.size();
        std::string typed;
        for (const char key : std::string("moxing")) {
            typed += key;
            f.hostText(typed, typed.size());
            check(f.render() == S_OK, "consecutive host spelling renders");
            check(f.context.document == L"4M" + FanyUtils::string_to_wstring(typed), "new marked text follows committed English");
            check(f.context.selection == static_cast<LONG>(2 + typed.size()), "host caret follows every newly rendered letter");
        }
        f.hostText("moxing", 2);
        check(f.render() == S_OK && f.context.selection == 4, "host caret inside composition overrides stale caret");
        f.hostText("moxing", 999);
        check(f.render() == S_OK && f.context.selection == 8, "host caret is bounded by current spelling");
    }
    {
        Fixture f; f.hostText("moxing", 3);
        GlobalSettings::style = GlobalSettings::TsfPreeditStyle::Pinyin;
        check(f.render(1) == S_OK && f.context.document == L"4Mmo'xing" && f.context.selection == 6,
              "formatted preedit maps the host caret through separators");
    }
    for (const auto &prefix : {std::string("你"), std::string("\xF0\xA0\xAE\xB7")}) {
        Fixture f; f.hostText("hao", 1);
        currentResult.view.phrase_prefix = prefix;
        check(f.render() == S_OK, "partial phrase renders");
        const auto wide = FanyUtils::string_to_wstring(prefix);
        check(f.context.document == L"4M" + wide + L"hao" && f.context.selection == static_cast<LONG>(3 + wide.size()),
              "UTF-16 phrase prefix shifts the host caret");
    }
    {
        Fixture f; f.hostText("", 0);
        currentResult.view.phrase_prefix = "合成词";
        check(f.render() == S_OK && f.context.document == L"4M合成词" && f.context.selection == 5,
              "a prefix-only composition ends after its selected phrase");
    }
    {
        Fixture f; f.hostText("kanji", 5);
        Global::InputModeScheme = msime::windows::scheme::Japanese;
        currentResult.view.reading = "かんじ";
        check(f.render() == S_OK && f.context.document == L"4Mかんじ" && f.context.selection == 5,
              "Japanese reading keeps the caret at its displayed end");
        currentResult.view.caret = 2;
        check(f.render() == S_OK && f.context.document == L"4Mkanji" && f.context.selection == 4,
              "Japanese interior caret returns to the original letters");
    }
    {
        Fixture f; f.hostText("moxing", 6);
        GlobalSettings::style = GlobalSettings::TsfPreeditStyle::Empty;
        check(f.render() == S_OK && f.context.document == L"4M" && f.context.selection == 2,
              "hidden preedit has no visible caret offset");
    }
    {
        Fixture f; f.hostText("moxing", 6);
        currentResult.view.conversion = "模型";
        currentResult.view.conversion_focus_start = 1;
        check(f.render() == S_OK && f.context.document == L"4M模型" && f.context.selection == 3,
              "sentence editing retains its converted focus caret");
    }
    {
        Fixture f; f.hostText("nihao", 0);
        Global::InputModeScheme = msime::windows::scheme::Korean;
        check(f.render() == S_OK && f.context.selection == 7, "locked-caret schemes keep the caret at the end");
    }
    {
        Fixture f; f.engine.host.active = false;
        f.engine.legacy = L"legacy"; f.engine.caret = 2;
        check(f.render() == S_OK && f.context.document == L"4Mlegacy" && f.context.selection == 4,
              "profiles without a host retain the legacy caret");
    }
    for (const bool readable : {false, true}) {
        Fixture f; f.engine.legacy = L"stale";
        f.engine.host.readable = readable; parseOK = false;
        check(f.render() == E_FAIL && f.ime.writes == 0 && f.context.selections == 0,
              "failed host view cannot render stale legacy composition");
    }
    {
        Fixture f; f.hostText("moxing", 6); f.ime.writeOK = false;
        check(f.render() == E_FAIL && f.context.selections == 0, "failed text write does not place a caret");
    }
    std::puts("production preedit rendering: text and TSF selection share the host view");
}
'''


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if not compiler:
        print("skipped: no C++ compiler for the TSF rendering model")
        return 0
    source = (ROOT / "platforms/windows/tsf/Key/KeyHandler.cpp").read_text(encoding="utf-8")
    begin = source.index("HRESULT CMetasequoiaIME::_HandleCompositionInputWorker(")
    end = source.index("\n//+---------------------------------------------------------------------------", begin)
    worker = source[begin:end]
    wrapper = ""
    if "DWORD_PTR MapRawCaretToPreedit(" in source:
        begin = source.index("DWORD_PTR MapRawCaretToPreedit(")
        end = source.index("\n}", begin) + 2
        wrapper = source[begin:end]
    harness = HARNESS.replace("// 生产光标映射包装", wrapper).replace("// 生产完整绘制方法", worker)
    with tempfile.TemporaryDirectory(prefix="msime-tsf-preedit-") as temporary:
        directory = pathlib.Path(temporary)
        cpp, binary = directory / "render.cpp", directory / "render"
        cpp.write_text(harness, encoding="utf-8")
        built = subprocess.run([
            compiler, "-std=c++17", "-DMSIME_EDITION_FULL=1", "-Wall", "-Wextra", "-Werror",
            "-Wno-unused-variable", "-Wno-unused-but-set-variable", "-Wno-deprecated-declarations",
            "-I", str(ROOT),
            "-I", str(ROOT / "platforms/windows/tsf"),
            "-I", str(ROOT / "crates/host-api/include"), str(cpp), "-o", str(binary),
        ], check=False)
        if built.returncode:
            return built.returncode
        return subprocess.run([str(binary)], check=False, timeout=30).returncode


if __name__ == "__main__":
    raise SystemExit(main())

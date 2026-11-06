#!/usr/bin/env python3
"""编译执行真实的重复标点处理函数：武装在排队期间过期时，这颗键必须和没被认领时走同一条标点路径。"""
from __future__ import annotations
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TSF = ROOT / 'platforms/windows/tsf'

HARNESS = r'''
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cwctype>
#include <optional>
#include <string>
#include <unordered_set>
#include "PairedPunctuationHostPolicy.h"
#include "CandidatePunctuationKeyPolicy.h"

#define _In_
using WCHAR = wchar_t;
using BOOL = int;
using UINT = unsigned;
using LONG = long;
using ULONG = unsigned long;
using HRESULT = int;
using TfEditCookie = int;
using TfClientId = unsigned;
using HWND = void *;
constexpr HRESULT S_OK = 0, E_FAIL = -2, E_INVALIDARG = -3;
constexpr int CP_UTF8 = 0, MB_ERR_INVALID_CHARS = 0, ERROR_BROKEN_PIPE = 109, TF_DEFAULT_SELECTION = 0;
constexpr int TF_ANCHOR_START = 0, TF_ANCHOR_END = 1;
constexpr UINT VK_DECIMAL = 0x6E, VK_OEM_PERIOD = 0xBE, VK_OEM_1 = 0xBA, VK_OEM_COMMA = 0xBC;
constexpr uint64_t FANY_IME_NO_REQUEST_ID = 0;
bool SUCCEEDED(HRESULT hr) { return hr >= 0; }
bool FAILED(HRESULT hr) { return hr < 0; }
HRESULT HRESULT_FROM_WIN32(int code) { return -code; }
int MultiByteToWideChar(int, int, const char *, int, WCHAR *, int) { return 0; }
unsigned long long GetTickCount64() { return 1000; }
HWND GetForegroundWindow() { return nullptr; }
void DebugTsfSmartPunctuation(WCHAR, bool, bool) {}
void SendPairedPunctuationAutoClosedToServerViaNamedPipe(WCHAR) {}
WCHAR GetPairedPunctuationClosing(const std::wstring &) { return 0; }
struct PerfTimer { double ElapsedMs() const { return 0; } };

namespace Global {
std::atomic<bool> SmartPunctuationEnabled{true};
std::atomic<bool> SmartPunctuationDirectDigitEnabled{true};
std::atomic<bool> SmartPunctuationDirectLetterEnabled{true};
std::atomic<bool> PairedPunctuationEnabled{false};
std::wstring current_process_name = L"qq.exe";
const std::unordered_set<WCHAR> CommitWithHighlightedCandPunc = {L',', L'.', L';'};
WCHAR FullWidthCharTable[0x5f];
enum class DataFromServerMsgType { Normal, CommitExactText, TransportUnavailable };
}
struct FanyImeNamedpipeDataToTsf { Global::DataFromServerMsgType msg_type; const WCHAR *candidate_string; };
FanyImeNamedpipeDataToTsf *TryReadDataFromServerPipeWithTimeout(uint64_t) { return nullptr; }
struct FanyImeQuickPhraseCursorPipe {
    static std::optional<std::size_t> SuffixUnits(Global::DataFromServerMsgType, bool) { return std::nullopt; }
};
namespace msime::tsf {
struct EngineResult {
    bool handled = false, has_commit = false;
    std::string commit;
    std::optional<std::string> commit_cursor_suffix;
};
struct EngineSessionAdapter {
    static bool parse_result(const std::string &, EngineResult *, std::string *) { return false; }
};
}

struct CStringRange {
    std::wstring text;
    void Set(const WCHAR *s, std::size_t n) { text.assign(s, n); }
};
struct ITfRange {
    HRESULT Collapse(TfEditCookie, int) { return S_OK; }
    void Release() {}
};
struct TF_SELECTION { ITfRange *range = nullptr; };
struct ITfThreadMgr {};
// 没有选区的文档：原地改写走不通，读不回前文的宿主（QQ）也是这样。
struct ITfContext {
    HRESULT GetSelection(TfEditCookie, int, ULONG, TF_SELECTION *, ULONG *fetched) { *fetched = 0; return E_FAIL; }
    HRESULT SetSelection(TfEditCookie, ULONG, TF_SELECTION *) { return S_OK; }
};
HRESULT SafeRangeShiftStart(ITfRange *, TfEditCookie, LONG, LONG *) { return E_FAIL; }
HRESULT SafeRangeSetText(ITfRange *, TfEditCookie, int, const WCHAR *, LONG) { return E_FAIL; }

struct Host {
    int calls = 0;
    bool valid() const { return true; }
    bool punctuation(uint8_t, std::string *, std::string *) { ++calls; return false; }
};
class CCompositionProcessorEngine {
public:
    Host host;
    bool punctuationMode = true;
    bool doubleSingleByteMode = false;
    int getPunctuationCalls = 0;
    Host *GetHostEngineAdapter() { return &host; }
    BOOL GetPunctuationMode(ITfThreadMgr *, TfClientId) { return punctuationMode; }
    BOOL GetDoubleSingleByteMode(ITfThreadMgr *, TfClientId) { return doubleSingleByteMode; }
    BOOL IsPunctuation(WCHAR wch) { return GetPunctuation(wch) != nullptr; }
    BOOL IsDoubleSingleByte(WCHAR wch) { return wch >= 0x20 && wch < 0x7f; }
    void BalanceNestPairAfterAutoClose(WCHAR) {}
    const WCHAR *GetPunctuation(WCHAR key) {
        switch (key) {
        case L',': return L"，";
        case L'.': return L"。";
        case L':': return L"：";
        case L';': return L"；";
        default: return nullptr;
        }
    }
    static BOOL IsSmartAsciiPunctuationKey(WCHAR);
    std::wstring ResolvePunctuation(WCHAR, WCHAR);
};
struct CCandidateListUIPresenter;
enum CANDIDATE_MODE { CANDIDATE_NONE, CANDIDATE_ORIGINAL, CANDIDATE_INCREMENTAL };

class CMetasequoiaIME {
public:
    CCompositionProcessorEngine engine;
    CCompositionProcessorEngine *_pCompositionProcessorEngine = &engine;
    CCandidateListUIPresenter *_pCandidateListUIPresenter = nullptr;
    ITfThreadMgr *_pThreadMgr = nullptr;
    TfClientId _tfClientId = 0;
    CANDIDATE_MODE _candidateMode = CANDIDATE_NONE;

    // 文档：光标停在末尾。readable 为假时读不回前文（QQ 这类宿主），只能靠前文影子。
    std::wstring document;
    bool readable = false;
    std::wstring rewrites;
    WCHAR armedSpace = 0;
    int spaceArms = 0;

    WCHAR _smartPunctuationShadowChar = 0;
    bool _smartPunctuationShadowValid = false;
    WCHAR _smartPunctuationKey = 0;
    WCHAR _smartPunctuationPrecedingChar = 0;
    bool _smartPunctuationCommittedAscii = false;
    bool _smartPunctuationAsciiRejected = false;
    unsigned long long _smartPunctuationCommitTick = 0;
    uint64_t _smartPunctuationFocusToken = 0;
    HWND _smartPunctuationForegroundWindow = nullptr;
    bool _smartPunctuationSpaceArmed = false;
    WCHAR _smartPunctuationSpaceChinese = 0;
    WCHAR _smartPunctuationSpaceBeforeChar = 0;
    uint64_t _smartPunctuationSpaceFocusToken = 0;
    HWND _smartPunctuationSpaceForegroundWindow = nullptr;
    bool _smartPunctuationSpaceInterrupted = false;
    bool _smartPunctuationRevertArmed = false;
    WCHAR _smartPunctuationRevertAscii = 0;
    WCHAR _smartPunctuationRevertChinese = 0;
    WCHAR _smartPunctuationRevertBeforeChar = 0;
    uint64_t _smartPunctuationRevertFocusToken = 0;
    HWND _smartPunctuationRevertForegroundWindow = nullptr;
    unsigned long long _smartPunctuationRevertDeadline = 0;

    bool _IsComposing() const { return false; }
    uint64_t _CaptureFocusSessionToken() const { return 7; }
    bool _QueueRepeatedSmartPunctuationReplacement(WCHAR) { return false; }
    HRESULT _ApplyHostCompositionResult(TfEditCookie, ITfContext *, const msime::tsf::EngineResult &) { return S_OK; }
    HRESULT _CommitServerExactText(TfEditCookie, ITfContext *, const WCHAR *, std::optional<std::size_t>) { return S_OK; }
    bool _TryStepOverPairedPunctuation(TfEditCookie, ITfContext *, WCHAR) { return false; }
    HRESULT _InsertTextToComposition(TfEditCookie, ITfContext *, CStringRange *) { return E_FAIL; }
    HRESULT _AddComposingAndChar(TfEditCookie, ITfContext *, CStringRange *) { return E_FAIL; }
    HRESULT _AddCharAndFinalize(TfEditCookie, ITfContext *, CStringRange *text) { document += text->text; return S_OK; }
    void _HandleCompleteCommitFirst(TfEditCookie, ITfContext *) {}
    void _HandleComplete(TfEditCookie, ITfContext *) {}
    void _HandleCancel(TfEditCookie, ITfContext *) {}
    void _PushPairedPunctuation(WCHAR, WCHAR) {}
    void _QueuePairedPunctuationCaretMove(int) {}
    void _ArmSmartPunctuationSpace(WCHAR chinese, bool autoClosedPair, WCHAR) {
        ++spaceArms;
        armedSpace = autoClosedPair ? 0 : chinese;
    }
    WCHAR _GetPrecedingDocumentChar(TfEditCookie, ITfContext *) {
        return readable && !document.empty() ? document.back() : 0;
    }
    bool _SmartPunctuationFingerprintMatches(TfEditCookie, ITfContext *, WCHAR) { return true; }
    bool _QueueSmartPunctuationRewrite(WCHAR replacement) {
        if (document.empty()) return false;
        document.back() = replacement;
        rewrites.push_back(replacement);
        return true;
    }

    std::wstring _PrefetchPunctuationCommitText(UINT code, WCHAR wch);
    HRESULT _HandleCompositionPunctuation(TfEditCookie ec, ITfContext *pContext, UINT code, WCHAR wch,
                                          uint64_t requestId, const std::wstring &prefetchedText);
    HRESULT _HandleCompositionDoubleSingleByte(TfEditCookie ec, ITfContext *pContext, WCHAR wch);
    HRESULT _HandleSmartPunctuationRevert(TfEditCookie ec, ITfContext *pContext, UINT code, WCHAR wch);
    WCHAR _GetPrecedingCharForSmartPunctuation(TfEditCookie ec, ITfContext *pContext);
    std::wstring _ResolveSmartPunctuation(WCHAR wch, WCHAR precedingChar);
    void _ResetSmartPunctuationHistory();
    void _ClearSmartPunctuationSpace();
    void _ClearSmartPunctuationRevert();
    void _InvalidateSmartPunctuationShadow();
    void _NoteSmartPunctuationDocumentText(WCHAR wch);
};

// 生产代码
'''

TESTS = r'''
int checks = 0, failures = 0;
void check(bool condition, const char *what) {
    ++checks;
    if (!condition) { ++failures; std::fprintf(stderr, "FAIL: %s\n", what); }
}

// 「1.」已经在文档里，前文影子记着那个 `.`（空格转换把「。」改回 `.` 之后就是这样），重复标点的武装已过期。
void prepare(CMetasequoiaIME &ime, const std::wstring &document, bool readable, bool shadow) {
    ime.document = document;
    ime.readable = readable;
    ime._smartPunctuationShadowValid = shadow;
    ime._smartPunctuationShadowChar = shadow ? document.back() : 0;
}

// 不经过屏障时同一颗键的处理：_IsKeyEaten 分成 FUNCTION_PUNCTUATION，按键线程定下预取文本，编辑会话里调 _HandleCompositionPunctuation。
void normalPunctuation(CMetasequoiaIME &ime, UINT code, WCHAR wch) {
    ime._HandleCompositionPunctuation(0, nullptr, code, wch, FANY_IME_NO_REQUEST_ID,
                                      ime._PrefetchPunctuationCommitText(code, wch));
}

void expectSameAsNormal(const std::wstring &document, bool readable, bool shadow, UINT code, WCHAR wch,
                        const std::wstring &expected, const char *what) {
    ITfContext context;
    CMetasequoiaIME expired, normal;
    prepare(expired, document, readable, shadow);
    prepare(normal, document, readable, shadow);
    const HRESULT hr = expired._HandleSmartPunctuationRevert(0, &context, code, wch);
    normalPunctuation(normal, code, wch);
    check(hr == S_OK, what);
    check(expired.document == expected, what);
    check(expired.document == normal.document, what);
    check(expired._smartPunctuationShadowValid == normal._smartPunctuationShadowValid &&
              expired._smartPunctuationShadowChar == normal._smartPunctuationShadowChar,
          what);
    check(expired.spaceArms == normal.spaceArms && expired.armedSpace == normal.armedSpace, what);
    check(expired._smartPunctuationKey == normal._smartPunctuationKey &&
              expired._smartPunctuationPrecedingChar == normal._smartPunctuationPrecedingChar &&
              expired._smartPunctuationCommittedAscii == normal._smartPunctuationCommittedAscii,
          what);
}

int main() {
    for (int i = 0; i < 0x5f; ++i) Global::FullWidthCharTable[i] = static_cast<WCHAR>(i == 0 ? 0x3000 : 0xFF00 + i);

    // 1. 问题本身：「1.」之后再按 `.`，前文是 `.` 不是数字，正常决议出「。」。修复前这里写进的是 `.`，得到「1..」。
    {
        ITfContext context;
        CMetasequoiaIME ime;
        prepare(ime, L"1.", false, true);
        check(ime._HandleSmartPunctuationRevert(0, &context, VK_OEM_PERIOD, L'.') == S_OK, "过期的重复标点仍然处理成功");
        check(ime.document == L"1.。", "过期后按前文决议：「1.」后再按 . 得到「1.。」");
        check(ime._smartPunctuationShadowValid && ime._smartPunctuationShadowChar == L'。', "写入后前文影子记下「。」");
        check(ime.spaceArms == 1 && ime.armedSpace == L'。', "和普通标点一样武装空格转换");
        check(ime._smartPunctuationKey == L'.' && !ime._smartPunctuationCommittedAscii,
              "和普通标点一样记下重复标点的历史");
        check(ime.rewrites.empty(), "过期时不再改写前面那个标点");
    }
    expectSameAsNormal(L"1.", false, true, VK_OEM_PERIOD, L'.', L"1.。", "读不回前文的宿主：与普通标点路径一致");
    expectSameAsNormal(L"1.", true, false, VK_OEM_PERIOD, L'.', L"1.。", "读得回前文的宿主：与普通标点路径一致");
    // 2. 前文是数字时，普通路径直出 ASCII，过期路径也一样。
    expectSameAsNormal(L"x1", false, true, VK_OEM_PERIOD, L'.', L"x1.", "数字后：与普通路径一样直出 .");
    expectSameAsNormal(L"x1", false, true, VK_OEM_COMMA, L',', L"x1,", "数字后的逗号：直出 ,");
    // 3. 不在智能标点名单里的键按键线程预取的中文标点上屏。
    expectSameAsNormal(L"a;", false, true, VK_OEM_1, L';', L"a;；", "非智能标点键：出中文分号");
    // 4. 小键盘的点在中文标点下也出 ASCII，过期路径要拿到虚拟键才分得出来。
    expectSameAsNormal(L"中.", false, true, VK_DECIMAL, L'.', L"中..", "小键盘的点：照常出 ASCII");
    // 5. 智能标点总开关关掉时，按键线程预取中文标点。
    Global::SmartPunctuationEnabled = false;
    expectSameAsNormal(L"1.", false, true, VK_OEM_PERIOD, L'.', L"1.。", "智能标点关闭：出中文句号");
    Global::SmartPunctuationEnabled = true;

    // 6. 排队期间切成了英文标点：不再是标点键，开着全角就按全角写，否则原样写下并记进前文影子。
    {
        ITfContext context;
        CMetasequoiaIME ime;
        prepare(ime, L"1.", false, true);
        ime.engine.punctuationMode = false;
        ime.engine.doubleSingleByteMode = true;
        check(ime._HandleSmartPunctuationRevert(0, &context, VK_OEM_PERIOD, L'.') == S_OK, "全角：处理成功");
        check(ime.document == std::wstring(L"1.") + static_cast<WCHAR>(0xFF0E), "全角模式：写全角点");
    }
    {
        ITfContext context;
        CMetasequoiaIME ime;
        prepare(ime, L"1;", false, false);
        ime.engine.punctuationMode = false;
        check(ime._HandleSmartPunctuationRevert(0, &context, VK_OEM_1, L';') == S_OK, "英文标点：处理成功");
        check(ime.document == L"1;;", "英文标点：原样写下按的字");
        check(ime._smartPunctuationShadowValid && ime._smartPunctuationShadowChar == L';',
              "英文标点：写下的字记进前文影子");
        check(ime.spaceArms == 0, "英文标点：不武装空格转换");
    }

    // 7. 武装换成了别的键（同一次排队里又转换了另一个标点）：这颗键同样按普通标点处理。
    {
        ITfContext context;
        CMetasequoiaIME ime;
        prepare(ime, L"1.", false, true);
        ime._smartPunctuationRevertArmed = true;
        ime._smartPunctuationRevertAscii = L',';
        ime._smartPunctuationRevertChinese = L'，';
        check(ime._HandleSmartPunctuationRevert(0, &context, VK_OEM_PERIOD, L'.') == S_OK, "武装是别的键：处理成功");
        check(ime.document == L"1.。" && ime.rewrites.empty(), "武装是别的键：按普通标点出「。」，不改写前文");
        check(!ime._smartPunctuationRevertArmed, "武装是别的键：武装被消费");
    }

    // 8. 回归：窗口内的重复标点仍然把前面的 ASCII 换回中文，不多写一个字。
    {
        ITfContext context;
        CMetasequoiaIME ime;
        prepare(ime, L"1.", false, true);
        ime._smartPunctuationRevertArmed = true;
        ime._smartPunctuationRevertAscii = L'.';
        ime._smartPunctuationRevertChinese = L'。';
        check(ime._HandleSmartPunctuationRevert(0, &context, VK_OEM_PERIOD, L'.') == S_OK, "窗口内：处理成功");
        check(ime.document == L"1。" && ime.rewrites == L"。", "窗口内：把 . 换回「。」");
        check(ime._smartPunctuationShadowValid && ime._smartPunctuationShadowChar == L'。', "窗口内：前文影子是「。」");
        check(ime.spaceArms == 0 && !ime._smartPunctuationRevertArmed, "窗口内：不另外武装，武装被消费");
    }
    {
        ITfContext context;
        CMetasequoiaIME ime;
        check(ime._HandleSmartPunctuationRevert(0, &context, 0, 0) == E_INVALIDARG && ime.document.empty(),
              "没有字符的键仍然拒绝");
    }

    std::printf("smart punctuation revert expiry: %d checks, %d failures\n", checks, failures);
    return failures ? 1 : 0;
}
'''


def method(source: str, start: str, end: str) -> str:
    begin = source.index(start)
    return source[begin:source.index(end, begin)]


def main() -> int:
    compiler = shutil.which('clang++') or shutil.which('g++') or shutil.which('c++')
    if compiler is None:
        print('skipped: C++ compiler unavailable for Windows smart punctuation revert regression')
        return 0
    handler = (TSF / 'Key/KeyHandler.cpp').read_text(encoding='utf-8')
    composition = (TSF / 'Composition/Composition.cpp').read_text(encoding='utf-8')
    engine = (TSF / 'Composition/CompositionProcessorEngine.cpp').read_text(encoding='utf-8')
    separator = '\n//+---------------------------------------------------------------------------'
    body = method(handler, 'std::wstring CMetasequoiaIME::_PrefetchPunctuationCommitText(', separator)
    body += method(handler, 'HRESULT CMetasequoiaIME::_HandleCompositionPunctuation(', separator)
    body += method(handler, 'HRESULT CMetasequoiaIME::_HandleCompositionDoubleSingleByte(', separator)
    body += method(composition, 'WCHAR CMetasequoiaIME::_GetPrecedingCharForSmartPunctuation(',
                   '\nbool CMetasequoiaIME::_CanConvertSmartPunctuationSpace(')
    body += method(composition, 'void CMetasequoiaIME::_ClearSmartPunctuationSpace()',
                   '\nvoid CMetasequoiaIME::_ArmSmartPunctuationRevert(')
    body += method(composition, 'HRESULT CMetasequoiaIME::_HandleSmartPunctuationRevert(',
                   '\nWCHAR CMetasequoiaIME::_GetPairedPunctuationClosingFor(')
    body += method(composition, 'void CMetasequoiaIME::_ResetSmartPunctuationHistory()', '\n}\n') + '\n}\n'
    body += method(composition, 'void CMetasequoiaIME::_InvalidateSmartPunctuationShadow()',
                   '\nvoid CMetasequoiaIME::_UpdateSmartPunctuationShadow(')
    body += method(composition, 'void CMetasequoiaIME::_NoteSmartPunctuationDocumentText(',
                   '\nvoid CMetasequoiaIME::_NoteKeyForSmartPunctuation(')
    body += method(composition, 'std::wstring CMetasequoiaIME::_ResolveSmartPunctuation(', separator)
    body += method(engine, 'BOOL CCompositionProcessorEngine::IsSmartAsciiPunctuationKey(',
                   'void CCompositionProcessorEngine::BalanceNestPairAfterAutoClose(')
    # 接线：分派表把虚拟键交给这个函数，按键线程的标点预取也走同一个函数。
    category = (TSF / 'Key/KeyStateCategory.cpp').read_text(encoding='utf-8')
    assert '_HandleSmartPunctuationRevert(dto.ec, dto.pContext, dto.code, dto.wch)' in category
    sink = (TSF / 'Key/KeyEventSink.cpp').read_text(encoding='utf-8')
    assert 'std::wstring punctuationCommitText = _PrefetchPunctuationCommitText(code, wch);' in sink
    source = HARNESS.replace('// 生产代码', body) + TESTS
    with tempfile.TemporaryDirectory(prefix='msime-smart-punctuation-revert-') as temporary:
        unit = pathlib.Path(temporary) / 'revert.cpp'
        binary = pathlib.Path(temporary) / 'revert'
        unit.write_text(source, encoding='utf-8')
        # 生产函数里有只给 MSVC 计时日志用的局部变量，这里不当成错误。
        subprocess.run([compiler, '-std=c++17', '-Wall', '-Wextra', '-Werror', '-Wno-unused-variable',
                        '-Wno-unused-but-set-variable', '-Wno-unused-parameter',
                        '-I' + str(TSF / 'Global'), str(unit), '-o', str(binary)], check=True)
        return subprocess.run([str(binary)], check=False).returncode


if __name__ == '__main__':
    raise SystemExit(main())

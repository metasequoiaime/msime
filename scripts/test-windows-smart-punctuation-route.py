#!/usr/bin/env python3
"""执行 TSF 标点入口和实际智能标点决策，防止无前文调用抢先上屏。"""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
HARNESS = r'''
#include <atomic>
#include <cassert>
#include <cstdint>
#include <cstdio>
#include <string>
#include <optional>
using WCHAR = wchar_t;
using BOOL = bool;
using UINT = unsigned;
using TfEditCookie = int;
struct ITfContext {};
constexpr int S_OK = 0, CP_UTF8 = 0, MB_ERR_INVALID_CHARS = 0, VK_DECIMAL = 110;
namespace Global {
std::atomic<bool> SmartPunctuationEnabled{true};
std::atomic<bool> SmartPunctuationDirectDigitEnabled{true};
std::atomic<bool> SmartPunctuationDirectLetterEnabled{true};
}
struct Host {
    int calls = 0;
    bool returnsResult = false;
    bool valid() const { return true; }
    bool punctuation(uint8_t, std::string *, std::string *) { ++calls; return returnsResult; }
};
class CCompositionProcessorEngine {
public:
    Host host;
    Host *GetHostEngineAdapter() { return &host; }
    static BOOL IsSmartAsciiPunctuationKey(WCHAR);
    std::wstring ResolvePunctuation(WCHAR, WCHAR);
    const WCHAR *GetPunctuation(WCHAR key) {
        switch (key) {
        case L',': return L"，";
        case L'.': return L"。";
        case L':': return L"：";
        default: return L"？";
        }
    }
};
// 生产决策
namespace msime::tsf {
struct EngineResult {
    bool handled = false, has_commit = false;
    std::string commit;
    std::optional<std::string> commit_cursor_suffix;
};
EngineResult probeResult;
struct EngineSessionAdapter {
    static bool parse_result(const std::string &, EngineResult *out, std::string *) { *out = probeResult; return true; }
};
}
int MultiByteToWideChar(int, int, const char *, int, WCHAR *, int) { return 0; }
struct CStringRange { void Set(const WCHAR *, std::size_t) {} };
bool _QueueRepeatedSmartPunctuationReplacement(WCHAR) { return false; }
int _AddCharAndFinalize(TfEditCookie, ITfContext *, CStringRange *) { return S_OK; }
int cursorCommits = 0;
int _ApplyHostCompositionResult(TfEditCookie, ITfContext *, const msime::tsf::EngineResult &result) {
    assert(result.has_commit && result.commit_cursor_suffix);
    ++cursorCommits;
    return S_OK;
}
bool composing = false;
bool _IsComposing() { return composing; }
constexpr int CANDIDATE_NONE = 0;
int _candidateMode = CANDIDATE_NONE;
int entry(CCompositionProcessorEngine *pCompositionProcessorEngine, WCHAR wch,
           const std::wstring &prefetchedText = {}) {
    TfEditCookie ec = 0;
    UINT code = 0;
    ITfContext *pContext = nullptr;
// 生产入口
    return S_OK;
}
int main() {
    CCompositionProcessorEngine engine;
    for (const WCHAR key : {L',', L'.', L':'}) {
        entry(&engine, key);
        assert(engine.host.calls == 0 && "smart punctuation must reach the editor-context path");
        for (const WCHAR before : {L'1', L'9', L'a', L'Z'})
            assert(engine.ResolvePunctuation(key, before) == std::wstring(1, key));
        for (const WCHAR before : {WCHAR{0}, L'中', L' '})
            assert(engine.ResolvePunctuation(key, before) == engine.GetPunctuation(key));
        Global::SmartPunctuationDirectDigitEnabled = false;
        assert(engine.ResolvePunctuation(key, L'1') == engine.GetPunctuation(key));
        Global::SmartPunctuationDirectDigitEnabled = true;
        Global::SmartPunctuationDirectLetterEnabled = false;
        assert(engine.ResolvePunctuation(key, L'a') == engine.GetPunctuation(key));
        Global::SmartPunctuationDirectLetterEnabled = true;
        Global::SmartPunctuationEnabled = false;
        entry(&engine, key);
        assert(engine.host.calls == 0);
        assert(engine.ResolvePunctuation(key, L'1') == engine.GetPunctuation(key));
        Global::SmartPunctuationEnabled = true;
    }
    entry(&engine, L'?');
    assert(engine.host.calls == 1 && "other punctuation keeps its host route");
    entry(&engine, L'.', L"合成。");
    entry(&engine, L'?', L"合成？");
    assert(engine.host.calls == 1 && "prefetched commits must not dispatch punctuation twice");
    composing = true;
    entry(&engine, L'.');
    assert(engine.host.calls == 2 && "composition punctuation keeps the host session route");
    composing = false;
    _candidateMode = 1;
    entry(&engine, L',');
    assert(engine.host.calls == 3 && "candidate punctuation keeps its existing route");
    engine.host.returnsResult = true;
    for (const auto &text : {std::string{}, std::string{"synthetic"}}) {
        msime::tsf::probeResult.handled = msime::tsf::probeResult.has_commit = true;
        msime::tsf::probeResult.commit = text;
        msime::tsf::probeResult.commit_cursor_suffix = "";
        entry(&engine, L'?');
    }
    assert(cursorCommits == 2 && "explicit cursor commits, including empty text, use the exact editor route");
    std::puts("Windows TSF 标点入口：数字、字母、中文前文和三个开关的回归通过");
}
'''


def main() -> None:
    compiler = shutil.which('c++') or shutil.which('clang++') or shutil.which('g++')
    if not compiler:
        print('skipped: 没有可用的 C++ 编译器')
        return
    directory = ROOT / 'platforms/windows/tsf'
    handler = (directory / 'Key/KeyHandler.cpp').read_text(encoding='utf-8')
    start = handler.index('    std::wstring pendingPunctuationCommitText', handler.index('HRESULT CMetasequoiaIME::_HandleCompositionPunctuation('))
    end = handler.index('    double pipeReadElapsedMs', start)
    # 宿主接口在这里作为边界探针；实际断言的是生产入口有没有绕过文档前文流程。
    entry = handler[start:end]
    engine = (directory / 'Composition/CompositionProcessorEngine.cpp').read_text(encoding='utf-8')
    start = engine.index('BOOL CCompositionProcessorEngine::IsSmartAsciiPunctuationKey(')
    end = engine.index('void CCompositionProcessorEngine::BalanceNestPairAfterAutoClose(', start)
    source = HARNESS.replace('// 生产决策', engine[start:end]).replace('// 生产入口', entry)
    with tempfile.TemporaryDirectory(prefix='msime-smart-punctuation-route-') as temporary:
        code = Path(temporary) / 'route.cpp'
        executable = Path(temporary) / 'route'
        code.write_text(source, encoding='utf-8')
        subprocess.run([compiler, '-std=c++17', '-Wall', '-Wextra', '-Werror', str(code), '-o', str(executable)], check=True)
        subprocess.run([str(executable)], check=True)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""执行真实线程焦点消息分支，验证首次文档连接与延迟消息的所有权复核。"""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
HARNESS = r'''
#include <cassert>
#include <functional>
#include <cstdio>
using BOOL = int;
using HRESULT = int;
constexpr int FALSE = 0, S_OK = 0, E_FAIL = -1, WM_ThreadFocus = 5;
bool SUCCEEDED(int result) { return result >= 0; }
namespace Global { bool g_connected = false; }
struct ITfDocumentMgr { int releases = 0; void Release() { ++releases; } };
struct Thread {
    ITfDocumentMgr document;
    bool focused = true, hasDocument = true;
    int focusCalls = 0, documentCalls = 0;
    HRESULT firstResult = S_OK, secondResult = S_OK, documentResult = S_OK;
    std::function<void(int)> focusSideEffect;
    std::function<void()> documentSideEffect;
    HRESULT IsThreadFocus(BOOL *out) {
        ++focusCalls;
        if (focusSideEffect) focusSideEffect(focusCalls);
        *out = focused;
        return focusCalls == 1 ? firstResult : secondResult;
    }
    HRESULT GetFocus(ITfDocumentMgr **out) {
        ++documentCalls;
        if (documentSideEffect) documentSideEffect();
        *out = hasDocument && SUCCEEDED(documentResult) ? &document : nullptr;
        return documentResult;
    }
};
struct Ime {
    Thread *thread = nullptr;
    bool owner = true;
    int activations = 0, refreshes = 0, statuses = 0;
    Thread *_GetThreadMgr() { return thread; }
    void OnSetFocus(ITfDocumentMgr *document, void *) {
        assert(owner && !Global::g_connected && document == &thread->document);
        ++activations; Global::g_connected = true;
    }
    void _RefreshLanguageBarThemeIcons() { ++refreshes; }
};
bool IsNamedpipeFocusStateOwner(Ime *ime) { return ime->owner; }
void SendCurrentImeStatusSnapshot(Ime *ime) { ++ime->statuses; }
void dispatch(Ime *pIME) {
    switch (WM_ThreadFocus) {
// 生产消息分支
    }
}
struct Fixture {
    Thread thread;
    Ime ime;
    Fixture() { Global::g_connected = false; ime.thread = &thread; }
    void run() { dispatch(&ime); assert(ime.refreshes == ime.statuses); }
};
int main() {
    // 激活时尚未获得焦点，随后仅收到线程事件；重复事件不创建第二次会话。
    { Fixture f; f.run(); f.run(); assert(f.ime.activations == 1 && f.thread.document.releases == 1 && f.thread.focusCalls == 2); }
    { Fixture f; Global::g_connected = true; f.run(); assert(f.ime.activations == 0 && f.thread.focusCalls == 0); }
    { Fixture f; f.ime.owner = false; f.run(); assert(f.thread.focusCalls == 0); }
    { Fixture f; f.ime.thread = nullptr; f.run(); assert(f.ime.activations == 0); }
    { Fixture f; f.thread.firstResult = E_FAIL; f.run(); assert(f.thread.documentCalls == 0); }
    { Fixture f; f.thread.focused = false; f.run(); assert(f.thread.documentCalls == 0); }
    { Fixture f; f.thread.hasDocument = false; f.run(); assert(f.ime.activations == 0); }
    { Fixture f; f.thread.documentResult = E_FAIL; f.run(); assert(f.ime.activations == 0); }
    // 获取文档期间失焦、查询失败、owner 替换或其它回调已连接，不能重复激活。
    { Fixture f; f.thread.documentSideEffect = [&] { f.thread.focused = false; }; f.run(); assert(f.ime.activations == 0 && f.thread.document.releases == 1); }
    { Fixture f; f.thread.secondResult = E_FAIL; f.run(); assert(f.ime.activations == 0 && f.thread.document.releases == 1); }
    { Fixture f; f.thread.focusSideEffect = [&](int n) { if (n == 2) f.ime.owner = false; }; f.run(); assert(f.ime.activations == 0 && f.thread.document.releases == 1); }
    { Fixture f; f.thread.documentSideEffect = [] { Global::g_connected = true; }; f.run(); assert(f.ime.activations == 0 && f.thread.document.releases == 1); }
    { Fixture f; f.thread.hasDocument = false; f.run(); f.thread.hasDocument = true; f.run(); assert(f.ime.activations == 1); }
    std::puts("TSF 初次文档连接、重复事件和失效焦点的 13 项回归通过");
}
'''

def main() -> None:
    compiler = shutil.which('c++') or shutil.which('clang++') or shutil.which('g++')
    if not compiler:
        print('skipped: 没有可用的 C++ 编译器')
        return
    source = (ROOT / 'platforms/windows/tsf/IME/MetasequoiaIME.cpp').read_text()
    start = source.index('    case WM_ThreadFocus: {')
    end = source.index('    case WM_DrainDeferredKeyDown:', start)
    harness = HARNESS.replace('// 生产消息分支', source[start:end])
    with tempfile.TemporaryDirectory(prefix='msime-tsf-focus-') as directory:
        code = Path(directory) / 'focus.cpp'
        executable = Path(directory) / 'focus'
        code.write_text(harness)
        subprocess.run([compiler, '-std=c++17', '-Wall', '-Wextra', '-Werror', str(code), '-o', str(executable)], check=True)
        subprocess.run([str(executable)], check=True)

if __name__ == '__main__':
    main()

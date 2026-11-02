#!/usr/bin/env python3
"""执行生产 TSF 光标提交适配器，覆盖在线／离线焦点归属和重入边界。"""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
compiler = shutil.which('clang++') or shutil.which('c++') or shutil.which('g++')
if not compiler:
    print('skipped: 没有可用的 C++ 编译器')
    raise SystemExit(0)
source = (ROOT / 'platforms/windows/tsf/Composition/Composition.cpp').read_text()
begin = source.index('HRESULT CMetasequoiaIME::_CommitTextWithCursor(')
end = source.index('\nHRESULT CMetasequoiaIME::_AddCharAndFinalize(', begin)
unit = r'''
#include "platforms/windows/tsf/QuickPhraseCursorEdit.h"
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <memory>
#include <string>
#include <vector>
using HRESULT = long;
using LONG = int;
using ULONG = unsigned;
using TfEditCookie = int;
using BOOL = bool;
constexpr HRESULT S_OK=0, E_FAIL=-1, E_INVALIDARG=-2;
constexpr BOOL FALSE=false;
constexpr int TF_DEFAULT_SELECTION=0, TF_AE_NONE=0, TF_ANCHOR_START=0, TF_ANCHOR_END=1;
bool FAILED(HRESULT hr) { return hr<0; }
bool SUCCEEDED(HRESULT hr) { return hr>=0; }
std::function<void(const std::string&)> hook;
std::vector<std::string> operations;
void observe(const std::string &name) { operations.push_back(name); if(hook) hook(name); }
struct Document { std::wstring text=L"左旧拼写右"; std::size_t start=1,end=4; unsigned writes=0; };
struct ITfRange {
    Document *document;
    std::size_t start,end;
    int refs=1;
    ITfRange(Document *owner,std::size_t first,std::size_t last):document(owner),start(first),end(last){}
    void Release() { if(--refs==0) delete this; }
    HRESULT Clone(ITfRange **out) { *out=new ITfRange(document,start,end);observe("clone");return S_OK; }
    HRESULT Collapse(TfEditCookie,int anchor) {
        if(anchor==TF_ANCHOR_END) start=end; else end=start;
        observe("collapse");return S_OK;
    }
};
struct TF_SELECTION {
    ITfRange *range=nullptr;
    struct { int ase=0; BOOL fInterimChar=false; } style;
};
struct ITfContext {
    Document document;
    int refs=1;
    void AddRef() { ++refs; }
    void Release() { --refs;assert(refs>=1); }
    HRESULT GetSelection(TfEditCookie,int,int,TF_SELECTION *out,ULONG *count) {
        out->range=new ITfRange(&document,document.start,document.end);*count=1;
        observe("get-selection");return S_OK;
    }
    HRESULT SetSelection(TfEditCookie,int,TF_SELECTION *selection) {
        document.start=selection->range->start;document.end=selection->range->end;
        observe("set-selection");return S_OK;
    }
};
struct ITfDocumentMgr {
    ITfContext *top=nullptr;
    HRESULT GetTop(ITfContext **out) { *out=top;if(top)top->AddRef();observe("get-top");return S_OK; }
    void Release() {}
};
struct ThreadMgr {
    ITfDocumentMgr document;
    HRESULT GetFocus(ITfDocumentMgr **out) { *out=&document;observe("get-focus");return S_OK; }
};
struct ITfComposition {
    ITfContext *context;
    int refs=1;
    explicit ITfComposition(ITfContext *owner):context(owner){}
    void AddRef() { ++refs; }
    void Release() { --refs;assert(refs>=1); }
    HRESULT GetRange(ITfRange **out) {
        *out=new ITfRange(&context->document,1,4);observe("get-range");return S_OK;
    }
};
HRESULT SafeRangeSetText(ITfRange *range,TfEditCookie,int,const wchar_t *text,LONG count) {
    range->document->text.replace(range->start,range->end-range->start,text,static_cast<std::size_t>(count));
    range->end=range->start+static_cast<std::size_t>(count);++range->document->writes;
    observe("set-text");return S_OK;
}
HRESULT SafeRangeGetText(ITfRange *range,TfEditCookie,int,wchar_t *text,ULONG capacity,ULONG *fetched) {
    const auto part=range->document->text.substr(range->start,(std::min)(range->end-range->start,static_cast<std::size_t>(capacity)));
    std::copy(part.begin(),part.end(),text);*fetched=static_cast<ULONG>(part.size());
    observe("get-text");return S_OK;
}
HRESULT SafeRangeShiftStart(ITfRange *range,TfEditCookie,LONG count,LONG *shifted) {
    assert(count<=0 && range->start>=static_cast<std::size_t>(-count));
    range->start-=static_cast<std::size_t>(-count);*shifted=count;observe("shift");return S_OK;
}
namespace GlobalIme { std::wstring word_for_creating_word,pending_create_word_preedit; }
namespace Global { void ResetForcedOverlayAnchor() {} }
struct CMetasequoiaIME {
    ITfComposition *_pComposition=nullptr;
    ITfContext *_pContext=nullptr;
    ThreadMgr *_pThreadMgr=nullptr;
    uint64_t _deferredKeyFocusGeneration=1,_editorOperationSerial=0,epoch=1,focus=0;
    bool fallback=false,acknowledged=false;
    std::vector<int> _creatingWordRestoreHistory;
    bool _IsServerUnavailableFallbackActive() const { return fallback; }
    uint64_t _CaptureCompositionEpoch() const { return epoch; }
    uint64_t _CaptureFocusSessionToken() const { return focus; }
    bool _IsCompositionEpochCurrent(uint64_t value) const { return epoch==value; }
    bool _IsFocusSessionCurrent(uint64_t token,ITfContext *context=nullptr) const {
        if(!token || token!=focus || !acknowledged) return false;
        if(!context) return true;
        ITfDocumentMgr *document=nullptr; ITfContext *top=nullptr;
        _pThreadMgr->GetFocus(&document);document->GetTop(&top);
        const bool matches=top==context;if(top)top->Release();document->Release();return matches;
    }
    void _ClearPairedPunctuationStack() {}
    void _CancelPairedPunctuationCaretMove() {}
    void _ResetSmartPunctuationHistory() {}
    void _DeleteCandidateList(BOOL,ITfContext*) { observe("delete-candidates"); }
    HRESULT _TerminateComposition(TfEditCookie,ITfContext*) {
        _pComposition=nullptr;_pContext=nullptr;++epoch;observe("end-composition");return S_OK;
    }
    HRESULT _CommitTextWithCursor(TfEditCookie,ITfContext*,const std::wstring&,std::size_t,bool* = nullptr);
};
// 生产适配器
struct Fixture {
    ITfContext context,other;
    ITfComposition composition{&context};
    ThreadMgr thread;
    CMetasequoiaIME ime;
    Fixture(bool offline,bool composing,uint64_t token) {
        thread.document.top=&context;ime._pThreadMgr=&thread;ime.focus=token;ime.fallback=offline;
        ime.acknowledged=!offline;
        if(composing) { ime._pComposition=&composition;ime._pContext=&context; }
        hook={};operations.clear();
    }
    ~Fixture() { hook={};assert(context.refs==1 && other.refs==1 && composition.refs==1); }
};
int main() {
    const std::wstring text{L'前',0xd83c,0xdf32,L'后'};
    for(bool offline:{false,true}) for(bool composing:{false,true}) {
        // 离线时同时覆盖零 token 和重连失败留下的非零、未确认 token。
        for(uint64_t token: {uint64_t{0},uint64_t{17}}) {
            if(!offline && !token) continue;
            Fixture f(offline,composing,token);bool refresh=false;
            assert(f.ime._CommitTextWithCursor(0,&f.context,text,3,&refresh)==S_OK);
            assert(refresh && f.context.document.writes==1 && f.context.document.text==L"左"+text+L"右");
            assert(f.context.document.start==2 && f.context.document.end==2);
            assert(!f.ime._pComposition && !f.ime._pContext);
        }
    }
    for(bool offline:{false,true}) {
        Fixture f(offline,true,17); f.thread.document.top=&f.other;
        assert(FAILED(f.ime._CommitTextWithCursor(0,&f.context,text,3)) && !f.context.document.writes);
    }
    { Fixture f(false,true,17);f.ime._pContext=&f.other;
      assert(FAILED(f.ime._CommitTextWithCursor(0,&f.context,text,3)) && !f.context.document.writes); }
    for(const auto &stage:{"set-text","set-selection","delete-candidates","end-composition","get-text","shift"}) {
        Fixture f(true,true,17);bool fired=false,refresh=true;std::size_t stopped=0;
        hook=[&](const std::string &operation) {
            if(!fired && operation==stage) {
                fired=true;stopped=operations.size();++f.ime._editorOperationSerial;f.context.document.text=L"新的预编辑";
                f.context.document.start=f.context.document.end=1;
            }
        };
        assert(f.ime._CommitTextWithCursor(0,&f.context,text,3,&refresh)==S_OK);
        assert(fired && !refresh && f.context.document.writes==1 && f.context.document.text==L"新的预编辑");
        // 检查所有权时可以读焦点，重入后不得再写文档、清理组字或设置选区。
        assert(std::all_of(operations.begin()+static_cast<std::ptrdiff_t>(stopped),operations.end(),
            [](const std::string &operation) {return operation=="get-focus" || operation=="get-top";}));
    }
    for(bool reconnect:{false,true}) {
        Fixture f(true,true,17);bool refresh=true;
        hook=[&](const std::string &operation) {
            if(operation=="set-text") {
                if(reconnect) ++f.ime.focus; else f.ime.fallback=false;
            }
        };
        assert(f.ime._CommitTextWithCursor(0,&f.context,text,3,&refresh)==S_OK);
        assert(!refresh && f.context.document.writes==1);
        assert(std::count(operations.begin(),operations.end(),"set-selection")==0);
    }
    for(const auto &stage:{"get-focus","get-top","get-range"}) {
        Fixture f(true,true,17);bool fired=false;
        hook=[&](const std::string &operation) {
            if(!fired && operation==stage) {fired=true;++f.ime._deferredKeyFocusGeneration;}
        };
        assert(FAILED(f.ime._CommitTextWithCursor(0,&f.context,text,3)) && !f.context.document.writes);
    }
    std::cout << "TSF cursor adapter: online/offline focus, stale token, exact context and COM reentry passed\n";
}
'''
unit = unit.replace('// 生产适配器', source[begin:end])
with tempfile.TemporaryDirectory(prefix='msime-tsf-cursor-adapter-') as temporary:
    code = Path(temporary) / 'adapter.cpp'
    executable = Path(temporary) / 'adapter'
    code.write_text(unit)
    subprocess.run([compiler, '-std=c++17', '-Wall', '-Wextra', '-Werror', '-I', str(ROOT), str(code), '-o', str(executable)], check=True)
    subprocess.run([str(executable)], check=True)

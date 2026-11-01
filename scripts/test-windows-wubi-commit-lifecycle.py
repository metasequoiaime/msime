#!/usr/bin/env python3
"""执行真实的五笔 worker 窗口派发与收尾方法，替换 COM、窗口和文档边界。"""
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
if compiler is None:
    print("skipped: C++ compiler unavailable for Wubi worker lifecycle regression")
    raise SystemExit(0)
ime = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.cpp").read_text()
ime_header = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.h").read_text()
sink = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text()
handler = (ROOT / "platforms/windows/tsf/Key/KeyHandler.cpp").read_text()

def section(source, begin, end):
    start = source.index(begin)
    return source[start:source.index(end, start)]

unit = r'''
#include "platforms/windows/tsf/WubiCommitOwner.h"
#include "platforms/windows/tsf/PreferenceBarrier.h"
#include "platforms/windows/common/GroupedPanelKeyLease.h"
#include <atomic>
#include <deque>
#include <map>
#include <mutex>
#include <string>
#include <cstdio>
#include <new>
#include <utility>
using UINT=unsigned; using WPARAM=std::uintptr_t; using HRESULT=long;
using TfEditCookie=int; using HWND=int;
constexpr HRESULT S_OK=0, S_FALSE=1, TF_S_ASYNC=2, E_FAIL=-1;
constexpr UINT WM_CommitCandidateAndContinue=29, TIMER_PREFERENCE_TRANSITION=99;
void KillTimer(HWND, UINT) {}
constexpr int TF_ES_ASYNCDONTCARE=1, TF_ES_READWRITE=2;
#ifndef _Out_
#define _Out_
#endif
#ifndef _In_opt_
#define _In_opt_
#endif
#define STDMETHODIMP HRESULT
bool SUCCEEDED(HRESULT value) { return value>=0; }
bool FAILED(HRESULT value) { return value<0; }
uint64_t now=100;
uint64_t GetTickCount64() { return now; }
std::deque<UINT> posted;
bool postOk=true;
bool PostMessage(HWND, UINT, WPARAM value, int) { if (postOk) posted.push_back(value); return postOk; }
void ResetNamedpipeReplyState() {}
// 日志是隔离边界：补齐生产取消路径新增的诊断成员，不删改被提取的方法。
namespace Global { std::atomic<bool> TsfDiagnosticLogEnabled{false}; }
std::wstring BoolFlag(bool value) { return value ? L"1" : L"0"; }
void QueueTsfDiagnosticLog(const std::wstring &) {}
template<class... T> void DebugTsfIssue47(T...) {}
struct CMetasequoiaIME;
class CCommitCandidateAndContinueEditSession;
struct ITfContext {
    int refs=1;
    bool reject=false;
    CCommitCandidateAndContinueEditSession *pending=nullptr;
    void AddRef() { ++refs; }
    void Release() { --refs; }
    HRESULT RequestEditSession(int,CCommitCandidateAndContinueEditSession*,int,HRESULT*);
};
struct ITfDocumentMgr {
    ITfContext *context=nullptr;
    HRESULT GetTop(ITfContext **out) { *out=context; if(context)context->AddRef(); return S_OK; }
    void Release() {}
};
struct ThreadMgr {
    ITfDocumentMgr document;
    HRESULT GetFocus(ITfDocumentMgr **out) { *out=&document; return S_OK; }
};
struct Host { bool enabled=true;bool valid() const { return enabled; } };
struct Engine { Host host;Host *GetHostEngineAdapter() { return &host; } unsigned GetVirtualKeyLength() { return 0; } };
struct CEditSessionBase {
    CMetasequoiaIME *_pTextService;
    ITfContext *_pContext;
    CEditSessionBase(CMetasequoiaIME *ime,ITfContext *context):_pTextService(ime),_pContext(context){}
    virtual ~CEditSessionBase()=default;
    virtual HRESULT DoEditSession(TfEditCookie)=0;
    void Release() {}
};
struct CMetasequoiaIME {
    struct WorkerCandidateCommit {
        std::wstring text;
        uint64_t focusToken=0,compositionEpoch=0;
        std::optional<std::size_t> cursorSuffix = std::nullopt;
    };
    struct DeferredKeyDown { ITfContext *context=nullptr; WPARAM wParam=0; wchar_t translatedWch=0; uint64_t focusGeneration=0;
        struct { int Category=0,Function=0; } keyState; };
    std::atomic<bool> _localSessionResetPending{false};
    msime::tsf::PreferenceBarrier _preferenceBarrier;
    uint64_t _preferenceTransitionStarted=0;
    std::deque<FanyImePreferencePipe::Transition> _preferenceTransitions;
    std::atomic<bool> _preferenceNotificationPending{false};
    msime::tsf::WubiCommitOwner _wubiCommitOwner;
    msime::panel_key::Leases _groupedPanelLeases;
    std::deque<DeferredKeyDown> _deferredKeyDowns, _deferredAppliedPrefix;
    DeferredKeyDown _deferredKeyInFlight;
    bool _hasDeferredKeyInFlight=false;
    uint64_t _deferredKeyReplayToken=0,_deferredKeyFocusGeneration=1;
    bool _deferredKeyClassificationPending=false,_backspaceHoldArmed=false,_deferredKeyDrainPosted=false,
      _deferredKeyProjectionValid=false,_deferredProjectedImeOpen=false,_deferredProjectedPunctuationOpen=false,
      _deferredProjectedDoubleSingleByteOpen=false,_deferredProjectedCandidateActive=false,
      _deferredProjectedUnicodeMode=false,_deferredProjectedUrlMode=false,_deferredProjectedPhraseActive=false,
      _deferredProjectedKoreanHanjaListOpen=false,_shiftHotkeyArmed=false,_ctrlHotkeyArmed=false;
    size_t _deferredProjectedInputLength=0,_deferredProjectedCaret=0;
    std::wstring _deferredProjectedRawInput;
    Engine *_pCompositionProcessorEngine=nullptr;
    std::mutex _pendingCommitCandidateMutex;
    std::map<UINT,WorkerCandidateCommit> _pendingServerCommitMessages;
    WorkerCandidateCommit _pendingContinuationFailure;
    std::map<UINT,int> _pendingWorkerSwitchMessages;
    std::wstring _pendingServerCandidateString;
    bool _hasPendingServerCandidate=false;
    UINT _pendingServerCandidateMsgType=0;
    HWND _msgWndHandle=1;
    int _tfClientId=1, completed=0,dirty=0,writes=0;
    bool writeFails=false;
    uint64_t editorSerial=0;
    std::optional<std::size_t> cursorSeen;
    void _BeginEditorOperation() { ++editorSerial; }
    uint64_t focus=1,epoch=1;
    ThreadMgr thread;
    bool _IsComposing() { return true; }
    void _ClearApplicationKeyReplay() {}
    void _ClearAsyncKeyRequests() {}
    ThreadMgr *_GetThreadMgr() { return &thread; }
    bool _IsFocusSessionCurrent(uint64_t value,ITfContext *context=nullptr) const {
        return value==focus && (!context || context==thread.document.context); }
    bool _IsCompositionEpochCurrent(uint64_t value) const { return value==epoch; }
    REPLAY_CURRENT_DECLARATION
    void _CompleteDeferredKeyReplay(uint64_t token) {
        if (_hasDeferredKeyInFlight && token==_deferredKeyReplayToken) {
            ++completed; _hasDeferredKeyInFlight=false; _deferredKeyReplayToken=0;
            _deferredKeyInFlight.context->Release(); _deferredKeyInFlight={}; } }
    HRESULT _HandleCommitCandidateAndContinue(TfEditCookie,ITfContext*,const std::wstring&,
                                               std::optional<std::size_t> cursor) {
        cursorSeen=cursor; ++writes; ++epoch; return writeFails?E_FAIL:S_OK; }
    bool _IsWubiWorkerEditCurrent(uint64_t,uint64_t,ITfContext*) const;
    bool _FinishWubiLocalEdit(uint64_t,bool);
    void _FinishWubiWorkerEdit(uint64_t,bool);
    void _PollWubiCommit();
    void _ClearPendingIpcRequests();
    void _ClearDeferredKeyDowns();
    bool _PeekServerCandidateCommit(UINT,WorkerCandidateCommit&);
    bool _TakeServerCandidateCommit(UINT,WorkerCandidateCommit&);
    void dispatch(UINT);
};
namespace Global { namespace DataFromServerMsgType { constexpr UINT OutofRange=0; } }
void MarkNamedpipeSessionDirtyForOwner(CMetasequoiaIME *ime) { ++ime->dirty; }
'''
unit = unit.replace('REPLAY_CURRENT_DECLARATION', section(
    ime_header, '    bool _IsDeferredKeyReplayCurrent(', '\n    bool _IsDeferredApplicationTextCurrent('))
unit += section(sink, 'bool CMetasequoiaIME::_IsDeferredKeyReplayCurrent(', '\nvoid CMetasequoiaIME::_RetryDeferredKeyReplay(')
unit += section(ime, 'class CCommitCandidateAndContinueEditSession :', '\nclass CDeferredApplicationTextEditSession')
unit += r'''
HRESULT ITfContext::RequestEditSession(int,CCommitCandidateAndContinueEditSession *edit,int,HRESULT *result) {
    if (reject) { *result=E_FAIL; return E_FAIL; }
    pending=edit; *result=TF_S_ASYNC; return S_OK;
}
'''
unit += section(ime,'bool CMetasequoiaIME::_IsWubiWorkerEditCurrent(', '\nuint64_t CMetasequoiaIME::_CaptureFocusSessionToken(')
unit += section(ime,'bool CMetasequoiaIME::_PeekServerCandidateCommit(', '\nbool CMetasequoiaIME::_PostWorkerCompartmentSwitch(')
unit += section(sink,'void CMetasequoiaIME::_ClearDeferredKeyDowns(', '\nvoid CMetasequoiaIME::_CompleteDeferredKeyReplay(')
case = section(ime, '    case WM_CommitCandidateAndContinue: {', '    case WM_InsertText:')
unit += 'void CMetasequoiaIME::dispatch(UINT wParam) { auto *pIME=this; switch(WM_CommitCandidateAndContinue) {\n' + case + '\n} }\n'
selection = section(handler, '        if (!isCovered)\n', '\n    // An initialized host')
unit += '#define DebugTsfIssue47(...) ((void)0)\nHRESULT selection_outside_result() { HRESULT workerResult=S_OK; const bool isCovered=false; if (true) {\n' + selection + '\nExit: return workerResult; }\n#undef DebugTsfIssue47\n'
unit += r'''
int checks=0,failures=0;
void check(bool value,const char *message) { ++checks; if(!value) { ++failures;std::fprintf(stderr,"FAIL: %s\n",message); } }
struct Fixture {
    CMetasequoiaIME ime;
    ITfContext context;
    Engine engine;
    Fixture() { ime._pCompositionProcessorEngine=&engine;ime.thread.document.context=&context; }
    void arm(uint64_t token=11) {
        ime._hasDeferredKeyInFlight=true; ime._deferredKeyReplayToken=token;
        ime._deferredKeyInFlight.context=&context; context.AddRef();
        ime._deferredKeyInFlight.focusGeneration=ime._deferredKeyFocusGeneration;
        ime._wubiCommitOwner.begin(token,now);
    }
    void result() { msime::tsf::EngineResult result;result.handled=result.has_commit=true;
        check(ime._wubiCommitOwner.observe(result),"local auto commit is deferred"); }
    void finish(uint64_t token=11) { check(ime._FinishWubiLocalEdit(token,true),"local edit retains FIFO token"); }
    void payload(UINT message=17) { ime._pendingServerCommitMessages.emplace(message,CMetasequoiaIME::WorkerCandidateCommit{L"合成",ime.focus,ime.epoch}); }
    void execute() { auto *edit=context.pending;context.pending=nullptr;check(edit!=nullptr,"worker edit requested");
        if(edit) { edit->DoEditSession(0);delete edit; } }
    ~Fixture() { delete context.pending; posted.clear();postOk=true;now=100; }
};
int main() {
    check(selection_outside_result()==S_FALSE,"selection outside composition does not report a locally applied character");
    { Fixture f;f.arm();f.payload();f.ime.dispatch(17);
      check(!f.ime._FinishWubiLocalEdit(11,false) && f.ime._hasDeferredKeyInFlight &&
            !f.ime._wubiCommitOwner.active() && f.ime.completed==0,
            "unapplied early worker leaves original failure callback responsible for retry");
      f.ime._CompleteDeferredKeyReplay(11); }
    { Fixture f;f.arm();f.payload();f.ime.dispatch(17);
      check(!f.context.pending,"early worker never requests COM before local character");
      f.result();f.finish();check(posted.size()==1,"early worker scheduled after local edit");
      if(!posted.empty())f.ime.dispatch(posted.front());
      check(f.ime._hasDeferredKeyInFlight,"FIFO stays owned while async worker waits");f.execute();
      check(f.ime.writes==1 && f.ime.completed==1 && !f.ime._hasDeferredKeyInFlight,"worker writes once and releases FIFO"); }
    { Fixture f;f.arm();f.result();f.finish();f.ime._ClearPendingIpcRequests();
      check(!f.ime._hasDeferredKeyInFlight && !f.ime._wubiCommitOwner.active() && f.context.refs==1,
            "disconnect/reset releases the retained token and its context"); }
    { Fixture f;f.arm();f.ime._ClearPendingIpcRequests();
      check(f.ime._hasDeferredKeyInFlight && !f.ime._wubiCommitOwner.active(),
            "reset before local character leaves original retry callback owning its token");
      f.ime._CompleteDeferredKeyReplay(11); }
    { Fixture f;f.arm(22);f.ime.dispatch(17);
      check(f.ime._wubiCommitOwner.current(22) && !f.ime._wubiCommitOwner.has_early_worker() && f.ime.dirty==0,
            "old posted message without a payload cannot alter the new owner"); }
    { Fixture f;f.arm(22);f.payload();f.ime._pendingServerCommitMessages[17].compositionEpoch=0;f.ime.dispatch(17);
      check(!f.ime._wubiCommitOwner.has_early_worker() && f.ime.dirty==0,"old epoch payload cannot alter new owner"); }
    { Fixture f;f.arm();f.result();f.finish();f.payload();f.ime.dispatch(17);
      f.ime._ClearPendingIpcRequests();f.arm(22);f.execute();
      check(f.ime.writes==0 && f.ime._wubiCommitOwner.current(22),"cancelled async edit cannot write or retire replacement token"); }
    { Fixture f;f.arm();f.result();f.finish();f.context.reject=true;f.payload();f.ime.dispatch(17);
      check(f.ime.dirty==1 && !f.ime._hasDeferredKeyInFlight,"refused worker edit resets without replaying character"); }
    { Fixture f;f.arm();f.result();f.finish();now=2100;f.ime._PollWubiCommit();
      check(f.ime.dirty==1 && !f.ime._hasDeferredKeyInFlight,"missing worker timeout releases blocked FIFO"); }
    { Fixture f;f.arm();f.result();f.finish();f.payload();f.ime.dispatch(17);f.ime.writeFails=true;f.execute();
      check(f.ime.writes==1 && f.ime.dirty==1 && !f.ime._hasDeferredKeyInFlight,"partial document failure never replays or rewrites"); }
    { Fixture f;f.payload();f.ime.dispatch(17);
      check(!f.context.pending && f.ime.writes==0,"modern worker without retained ownership never writes"); }
    { Fixture f;f.engine.host.enabled=false;f.payload();f.ime.dispatch(17);f.execute();
      check(f.ime.writes==1,"legacy host without an adapter keeps its original worker route"); }
    for (std::size_t suffix : {std::size_t{0}, std::size_t{3}}) {
        Fixture f; f.arm(); f.result(); f.finish(); f.payload();
        f.ime._pendingServerCommitMessages[17].cursorSuffix=suffix;
        f.ime.dispatch(17);
        check(f.ime.editorSerial==0,"queueing does not take document ownership");
        f.execute();
        check(f.ime.cursorSeen==suffix && f.ime.editorSerial==1 && f.ime.writes==1 && f.ime.completed==1,
              "cursor metadata and explicit zero survive worker queue and granted edit session");
    }
    std::printf("Wubi worker lifecycle: %d checks, %d failures\n",checks,failures);return failures?1:0;
}
'''
with tempfile.TemporaryDirectory(prefix="msime-wubi-lifecycle-") as temporary:
    source = pathlib.Path(temporary) / "probe.cpp"
    binary = pathlib.Path(temporary) / "probe"
    source.write_text(unit)
    subprocess.run([compiler,"-std=c++17","-Wall","-Wextra","-Werror", "-DMSIME_EDITION_FULL", "-I",str(ROOT),
                    "-I",str(ROOT / "crates/host-api/include"),str(source),"-o",str(binary)],check=True)
    raise SystemExit(subprocess.run([str(binary)]).returncode)

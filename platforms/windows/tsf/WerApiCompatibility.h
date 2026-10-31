#pragma once
#include <windows.h>
#include <werapi.h>
#include <type_traits>

namespace msime::tsf::wer {
// 微软 SDK 的三种 WER 回调均为 cdecl；MinGW 的 WINAPI 声明在 x86 会破坏调用栈。
using ExceptionEvent = HRESULT (__cdecl *)(PVOID, const PWER_RUNTIME_EXCEPTION_INFORMATION,
    PBOOL, PWSTR, PDWORD, PDWORD);
using ExceptionSignature = HRESULT (__cdecl *)(PVOID, const PWER_RUNTIME_EXCEPTION_INFORMATION,
    DWORD, PWSTR, PDWORD, PWSTR, PDWORD);
using DebuggerLaunch = HRESULT (__cdecl *)(PVOID, const PWER_RUNTIME_EXCEPTION_INFORMATION,
    PBOOL, PWSTR, PDWORD, PBOOL);
#ifndef __MINGW32__
static_assert(std::is_same_v<ExceptionEvent, PFN_WER_RUNTIME_EXCEPTION_EVENT>);
static_assert(std::is_same_v<ExceptionSignature, PFN_WER_RUNTIME_EXCEPTION_EVENT_SIGNATURE>);
static_assert(std::is_same_v<DebuggerLaunch, PFN_WER_RUNTIME_EXCEPTION_DEBUGGER_LAUNCH>);
#endif
}

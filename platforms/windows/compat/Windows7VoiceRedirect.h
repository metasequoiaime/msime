#pragma once
#include "Windows7VoiceApi.h"

// 仅在固定上游源码的四个翻译单元中强制包含，SDK 声明完成后才重定向调用。
inline HANDLE WINAPI msime_win7_create_file(LPCWSTR name, DWORD access, DWORD share, DWORD disposition,
                                            LPCREATEFILE2_EXTENDED_PARAMETERS parameters) {
  return msime::windows::compat::voice_api().create_file(name, access, share, disposition, parameters);
}
inline VOID WINAPI msime_win7_precise_time(LPFILETIME output) {
  msime::windows::compat::voice_api().system_time(output);
}
inline BOOL WINAPI msime_win7_mitigation(HANDLE process, PROCESS_MITIGATION_POLICY policy, PVOID output, SIZE_T size) {
  return msime::windows::compat::voice_api().process_mitigation(process, policy, output, size);
}
inline HRESULT WINAPI msime_win7_remove_backslash(PWSTR path, size_t capacity) {
  return msime::windows::compat::voice_api().path_remove_backslash(path, capacity);
}
inline HRESULT WINAPI msime_win7_remove_file_spec(PWSTR path, size_t capacity) {
  return msime::windows::compat::voice_api().path_remove_file_spec(path, capacity);
}
inline HRESULT WINAPI msime_win7_create_factory(UINT flags, REFIID iid, void** output) {
  return msime::windows::compat::voice_api().create_factory(flags, iid, output);
}
#define CreateFile2 msime_win7_create_file
#define GetSystemTimePreciseAsFileTime msime_win7_precise_time
#define GetProcessMitigationPolicy msime_win7_mitigation
#define PathCchRemoveBackslash msime_win7_remove_backslash
#define PathCchRemoveFileSpec msime_win7_remove_file_spec
#define CreateDXGIFactory2 msime_win7_create_factory

#include <windows.h>
#include <sherpa-onnx/c-api/c-api.h>
#include <algorithm>
#include <cassert>
#include <cstdio>
#include <cstring>
#include <cwchar>
#include <string>

namespace {
template <typename Function>
Function load(HMODULE module, const char* name) {
  const auto address = GetProcAddress(module, name);
  if (!address) std::fprintf(stderr, "Missing recognition API: %s\n", name);
  assert(address);
  return reinterpret_cast<Function>(reinterpret_cast<void*>(address));
}
}

int wmain(int count, wchar_t** arguments) {
  assert(count == 2);
  // CMake 会传入正斜杠；加载器的相邻依赖搜索要求标准 Windows 绝对路径。
  std::wstring location(arguments[1]);
  std::replace(location.begin(), location.end(), L'/', L'\\');
  const HMODULE module = LoadLibraryExW(location.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
  if (!module) { std::fprintf(stderr, "Sherpa DLL load failed: %lu\n", GetLastError()); return 1; }
  // 共享识别适配器依赖的全部在线、离线和 VAD 入口都必须存在。
  for (const char* name : {
      "SherpaOnnxCreateOnlineRecognizer", "SherpaOnnxDestroyOnlineRecognizer",
      "SherpaOnnxCreateOnlineStream", "SherpaOnnxCreateOnlineStreamWithHotwords",
      "SherpaOnnxDestroyOnlineStream", "SherpaOnnxOnlineStreamAcceptWaveform",
      "SherpaOnnxIsOnlineStreamReady", "SherpaOnnxDecodeOnlineStream",
      "SherpaOnnxGetOnlineStreamResult", "SherpaOnnxDestroyOnlineRecognizerResult",
      "SherpaOnnxOnlineStreamReset", "SherpaOnnxOnlineStreamInputFinished", "SherpaOnnxOnlineStreamIsEndpoint",
      "SherpaOnnxCreateOfflineRecognizer", "SherpaOnnxDestroyOfflineRecognizer",
      "SherpaOnnxCreateOfflineStream", "SherpaOnnxCreateOfflineStreamWithHotwords",
      "SherpaOnnxDestroyOfflineStream", "SherpaOnnxAcceptWaveformOffline", "SherpaOnnxDecodeOfflineStream",
      "SherpaOnnxGetOfflineStreamResult", "SherpaOnnxDestroyOfflineRecognizerResult",
      "SherpaOnnxCreateVoiceActivityDetector", "SherpaOnnxDestroyVoiceActivityDetector",
      "SherpaOnnxVoiceActivityDetectorAcceptWaveform", "SherpaOnnxVoiceActivityDetectorEmpty",
      "SherpaOnnxVoiceActivityDetectorDetected", "SherpaOnnxVoiceActivityDetectorPop",
      "SherpaOnnxVoiceActivityDetectorFront", "SherpaOnnxDestroySpeechSegment", "SherpaOnnxVoiceActivityDetectorFlush"})
    assert(GetProcAddress(module, name));
#define MSIME_LOAD(name) const auto name = load<decltype(&::name)>(module, #name)
  MSIME_LOAD(SherpaOnnxGetVersionStr);
  MSIME_LOAD(SherpaOnnxGetOnnxruntimeVersionStr);
  MSIME_LOAD(SherpaOnnxCreateCircularBuffer);
  MSIME_LOAD(SherpaOnnxDestroyCircularBuffer);
  MSIME_LOAD(SherpaOnnxCircularBufferPush);
  MSIME_LOAD(SherpaOnnxCircularBufferGet);
  MSIME_LOAD(SherpaOnnxCircularBufferFree);
  MSIME_LOAD(SherpaOnnxCircularBufferPop);
  MSIME_LOAD(SherpaOnnxCircularBufferSize);
  MSIME_LOAD(SherpaOnnxCircularBufferHead);
#undef MSIME_LOAD
  assert(std::strcmp(SherpaOnnxGetVersionStr(), "1.13.8") == 0);
  // 不允许拾取其他目录里恰好能加载的现代运行库。
  wchar_t recognition_path[32768]{};
  wchar_t onnx_path[32768]{};
  const DWORD recognition_length = GetModuleFileNameW(module, recognition_path, 32768);
  const DWORD onnx_length = GetModuleFileNameW(GetModuleHandleW(L"onnxruntime.dll"), onnx_path, 32768);
  assert(recognition_length && recognition_length < 32768 && onnx_length && onnx_length < 32768);
  const std::wstring recognition(recognition_path, recognition_length);
  const auto adjacent = recognition.substr(0, recognition.find_last_of(L'\\') + 1) + L"onnxruntime.dll";
  std::fwprintf(stderr, L"Recognition: %ls\nONNX: %ls\n", recognition_path, onnx_path);
  assert(_wcsicmp(adjacent.c_str(), onnx_path) == 0);
  assert(std::strcmp(SherpaOnnxGetOnnxruntimeVersionStr(), "1.28.2") == 0);
  // 使用合成音频样本跨过实际 DLL 的分配、读写和释放边界。
  const auto* buffer = SherpaOnnxCreateCircularBuffer(16);
  assert(buffer);
  const float samples[]{0.25f, -0.5f, 0.75f, -1.0f};
  SherpaOnnxCircularBufferPush(buffer, samples, 4);
  assert(SherpaOnnxCircularBufferSize(buffer) == 4 && SherpaOnnxCircularBufferHead(buffer) == 0);
  const auto* copied = SherpaOnnxCircularBufferGet(buffer, 0, 4);
  assert(copied && std::memcmp(copied, samples, sizeof(samples)) == 0);
  SherpaOnnxCircularBufferFree(copied);
  SherpaOnnxCircularBufferPop(buffer, 2);
  assert(SherpaOnnxCircularBufferSize(buffer) == 2 && SherpaOnnxCircularBufferHead(buffer) == 2);
  SherpaOnnxDestroyCircularBuffer(buffer);
  assert(FreeLibrary(module));
  std::puts("Win7 Sherpa runtime: versions, recognition exports, adjacent ONNX and real buffer lifecycle passed");
}

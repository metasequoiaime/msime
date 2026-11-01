#include "SystemSpeech.h"

#include <windows.h>
#include <sapi.h>
#include <wrl/client.h>

namespace msime::windows {
namespace {
using Microsoft::WRL::ComPtr;

// 与 SystemAsrStream.cpp 相同，类和接口 id 直接写出，不依赖 sapi.lib / libsapi.a 是否带着它们。数值与 sapi.h 的 DEFINE_GUID 相同。
constexpr GUID kClsidSpVoice{0x96749377, 0x3391, 0x11d2, {0x9e, 0xe3, 0x00, 0xc0, 0x4f, 0x79, 0x73, 0x96}};
constexpr GUID kIidSpVoice{0x6c44df74, 0x72b9, 0x4992, {0xa1, 0xec, 0xef, 0x99, 0x6e, 0x04, 0x22, 0xd4}};
constexpr GUID kClsidSpObjectTokenCategory{0xa910187f, 0x0c7a, 0x45ac, {0x92, 0xcc, 0x59, 0xed, 0xaf, 0xb7, 0x7b, 0x53}};
constexpr GUID kIidSpObjectTokenCategory{0x2d3d3845, 0x39af, 0x4850, {0xbb, 0xf9, 0x40, 0xb4, 0x97, 0x80, 0x01, 0x1d}};

constexpr const wchar_t *kUnavailable = L"系统没有可用的朗读语音";
constexpr const wchar_t *kFailed = L"无法朗读";

struct ComApartment {
  HRESULT result = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  ComApartment() = default;
  ComApartment(const ComApartment &) = delete;
  ComApartment &operator=(const ComApartment &) = delete;
  ~ComApartment() {
    if (SUCCEEDED(result))
      CoUninitialize();
  }
  bool usable() const { return SUCCEEDED(result) || result == RPC_E_CHANGED_MODE; }
};

// 装着的英文声音，先美式再英式；都没有时返回空，调用方用系统默认的声音（中文系统上常常只有中文声音，读英文单词能听懂）。
ComPtr<ISpObjectToken> english_voice() {
  ComPtr<ISpObjectTokenCategory> category;
  if (FAILED(CoCreateInstance(kClsidSpObjectTokenCategory, nullptr, CLSCTX_INPROC_SERVER,
                              kIidSpObjectTokenCategory,
                              reinterpret_cast<void **>(category.GetAddressOf()))) ||
      !category || FAILED(category->SetId(SPCAT_VOICES, FALSE)))
    return {};
  for (const wchar_t *required : {L"Language=409", L"Language=809"}) {
    ComPtr<IEnumSpObjectTokens> tokens;
    ComPtr<ISpObjectToken> token;
    ULONG fetched = 0;
    if (SUCCEEDED(category->EnumTokens(required, nullptr, tokens.GetAddressOf())) && tokens &&
        tokens->Next(1, token.GetAddressOf(), &fetched) == S_OK && fetched == 1 && token)
      return token;
  }
  return {};
}
} // namespace

std::optional<std::wstring> speak_english(const std::string &text) {
  if (text.empty())
    return std::wstring(kFailed);
  const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                        static_cast<int>(text.size()), nullptr, 0);
  if (count <= 0)
    return std::wstring(kFailed);
  std::wstring wide(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()),
                          wide.data(), count) != count)
    return std::wstring(kFailed);
  ComApartment apartment;
  if (!apartment.usable())
    return std::wstring(kFailed);
  ComPtr<ISpVoice> voice;
  if (FAILED(CoCreateInstance(kClsidSpVoice, nullptr, CLSCTX_INPROC_SERVER, kIidSpVoice,
                              reinterpret_cast<void **>(voice.GetAddressOf()))) ||
      !voice)
    return std::wstring(kUnavailable);
  if (const auto token = english_voice())
    (void)voice->SetVoice(token.Get());
  // 同步读完再返回：声音对象随这次调用释放，异步读会被截断。文字按纯文本读，不当作 SAPI 的 XML 标记。
  // 一个声音都没装时创建照样成功，读的时候才失败，所以读失败也说成没有可用的声音。
  if (FAILED(voice->Speak(wide.c_str(), SPF_PURGEBEFORESPEAK | SPF_IS_NOT_XML, nullptr)))
    return std::wstring(kUnavailable);
  return std::nullopt;
}
} // namespace msime::windows

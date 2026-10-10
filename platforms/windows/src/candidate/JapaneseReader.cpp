#ifndef NOMINMAX
#define NOMINMAX
#endif
#include "JapaneseReader.h"
#include "JapaneseRomaji.h"

#include <windows.h>
#include <objbase.h>

#include <cstddef>
#include <vector>

namespace msime::windows {
namespace {
// msime.h 的声明照抄过来：MinGW 不带这个头文件。布局按 Windows SDK 的 msime.h，它对整个文件用 #pragma pack(1)；WDD 里六个一位的标志和十位的填充在 MSVC 和 MinGW（默认 -mms-bitfields）下都正好占一个 WORD，这里写成一个 WORD。
#pragma pack(push, 1)
struct WordDescriptor {
  WORD wDispPos;
  WORD wReadPos;
  WORD cchDisp;
  WORD cchRead;
  DWORD reserved;
  WORD nPos;
  WORD flags;
  void *pReserved;
};
struct MorphResult {
  DWORD dwSize;
  WCHAR *pwchOutput;
  WORD cchOutput;
  WCHAR *pwchRead;
  WORD cchRead;
  WORD *pchInputPos;
  WORD *pchOutputIdxWDD;
  WORD *pchReadIdxWDD;
  WORD *paMonoRubyPos;
  WordDescriptor *pWDD;
  INT cWDD;
  void *pPrivate;
};
#pragma pack(pop)
static_assert(sizeof(WordDescriptor) == 16 + sizeof(void *), "WDD layout of msime.h");
static_assert(offsetof(MorphResult, cchOutput) == 4 + sizeof(void *), "MORRSLT layout of msime.h");
static_assert(offsetof(MorphResult, cchRead) == 6 + 2 * sizeof(void *), "MORRSLT layout of msime.h");
static_assert(offsetof(MorphResult, cWDD) == 8 + 7 * sizeof(void *), "MORRSLT layout of msime.h");

// IFELanguage，方法顺序与 msime.h 的 DECLARE_INTERFACE_(IFELanguage, IUnknown) 相同。
struct FeLanguage : IUnknown {
  virtual HRESULT STDMETHODCALLTYPE Open() = 0;
  virtual HRESULT STDMETHODCALLTYPE Close() = 0;
  virtual HRESULT STDMETHODCALLTYPE GetJMorphResult(DWORD request, DWORD mode, INT length, const WCHAR *input,
                                                    DWORD *info, MorphResult **result) = 0;
  virtual HRESULT STDMETHODCALLTYPE GetConversionModeCaps(DWORD *caps) = 0;
  virtual HRESULT STDMETHODCALLTYPE GetPhonetic(BSTR string, LONG start, LONG length, BSTR *phonetic) = 0;
  virtual HRESULT STDMETHODCALLTYPE GetConversion(BSTR string, LONG start, LONG length, BSTR *result) = 0;
};

// IID_IFELanguage {019F7152-E6DB-11d0-83C3-00C04FDDB82E}
constexpr GUID kFeLanguageIid = {0x019f7152, 0xe6db, 0x11d0, {0x83, 0xc3, 0x00, 0xc0, 0x4f, 0xdd, 0xb8, 0x2e}};
// FELANG_REQ_REV：由文字反查读音。FELANG_CMODE_HIRAGANAOUT（0）| FELANG_CMODE_AUTOMATIC | FELANG_CMODE_NOINVISIBLECHAR。
constexpr DWORD kRequestReverse = 0x00030000;
constexpr DWORD kReverseMode = 0x08000000 | 0x40000000;
// 释义的第一个词很短；更长的文字不交给 IFELanguage，免得它回 E_LARGEINPUT 或在工作线程上耗时。
constexpr size_t kMaximumInput = 64;
constexpr size_t kMaximumCache = 1024;

FeLanguage *language_of(void *pointer) { return static_cast<FeLanguage *>(pointer); }
} // namespace

JapaneseReader::~JapaneseReader() { close(); }

void JapaneseReader::close() {
  if (auto *language = language_of(language_)) {
    language->Close();
    language->Release();
    language_ = nullptr;
  }
  if (uninitialize_) {
    CoUninitialize();
    uninitialize_ = false;
  }
  attempted_ = false;
}

bool JapaneseReader::open() {
  if (attempted_)
    return language_ != nullptr;
  attempted_ = true;
  const HRESULT initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  if (SUCCEEDED(initialized))
    uninitialize_ = true;
  else if (initialized != RPC_E_CHANGED_MODE)
    return false;
  CLSID clsid{};
  if (FAILED(CLSIDFromProgID(L"MSIME.Japan", &clsid)))
    return false;
  void *created = nullptr;
  if (FAILED(CoCreateInstance(clsid, nullptr, CLSCTX_ALL, kFeLanguageIid, &created)) || !created)
    return false;
  auto *language = static_cast<FeLanguage *>(created);
  if (FAILED(language->Open())) {
    language->Release();
    return false;
  }
  language_ = language;
  return true;
}

std::string JapaneseReader::read(const std::string &term) {
  const auto text = japanese_utf16(term);
  if (text.empty())
    return {};
  if (text.size() <= kMaximumInput && open()) {
    const std::wstring input(text.begin(), text.end());
    std::vector<DWORD> info(input.size(), 0);
    MorphResult *result = nullptr;
    const HRESULT hr = language_of(language_)->GetJMorphResult(kRequestReverse, kReverseMode,
                                                               static_cast<INT>(input.size()), input.c_str(),
                                                               info.data(), &result);
    // 反查时 pwchOutput 是平假名读音，pwchRead（即 pwchComp）是原文：写法取原文、读音取输出（japanese_reverse_words）。
    std::vector<JapaneseWord> words;
    if (hr == S_OK && result && result->pwchOutput && result->pwchRead && result->pWDD && result->cWDD > 0) {
      std::vector<JapaneseMorphWord> morphs;
      morphs.reserve(static_cast<size_t>(result->cWDD));
      for (INT index = 0; index < result->cWDD; ++index) {
        const auto &word = result->pWDD[index];
        morphs.push_back({word.wDispPos, word.cchDisp, word.wReadPos, word.cchRead});
      }
      const std::u16string comp(result->pwchRead, result->pwchRead + result->cchRead);
      const std::u16string output(result->pwchOutput, result->pwchOutput + result->cchOutput);
      words = japanese_reverse_words(comp, output, morphs);
    }
    if (result)
      CoTaskMemFree(result);
    if (!words.empty())
      if (auto value = japanese_romaji(words); !value.empty())
        return value;
  }
  // 没有日语输入法，或它这次答不出来：只由假名组成的文字按假名直接读，整段当一个词。
  if (japanese_kana_only(text))
    return japanese_romaji({{text, text}});
  return {};
}

std::string JapaneseReader::romaji(const std::string &term) {
  if (term.empty())
    return {};
  if (const auto found = cache_.find(term); found != cache_.end())
    return found->second;
  if (cache_.size() >= kMaximumCache)
    cache_.clear();
  auto value = read(term);
  cache_.emplace(term, value);
  return value;
}
} // namespace msime::windows

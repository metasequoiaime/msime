#pragma once

#include <algorithm>
#include <cctype>
#include <chrono>
#include <optional>
#include <string>
#include <string_view>
#include <unordered_set>
#include <utility>
#include <vector>

namespace msime::windows {

// 账号释义缓存按认证账号隔离；同一个候选词对不同账号可能有不同回答。
inline std::string account_gloss_cache_scope(std::string_view user_id) {
  return std::string("account:") +
         (user_id.empty() ? std::string("anonymous") : std::string(user_id));
}

// 水杉账号的候选释义接口（服务端的 /v1/translate），和 macOS 输入法、Linux provider 经 msime_client_account 的 translate 操作、Android 与 iOS 键盘经 msime_client_account_gloss 问的是同一个：一次最多 32 个中文词、一种目标语言，源语言固定是中文。Windows 走 msime_client_account_gloss：路径、请求体和对回复的解读都由 Rust 给出，HTTP 和令牌在这里，所以这里只有服务的来源和本地预先筛词用的上限。
inline constexpr std::string_view account_gloss_origin = "https://api.msime.app";
inline constexpr size_t account_gloss_maximum_words = 32;
inline constexpr size_t account_gloss_maximum_word_bytes = 2048;
// 取不到任何会话时补注册本机匿名账号（macOS 的 BackendAnonymousAccount.ensureSignedIn），失败后隔多久再试。注册最多阻塞约一分钟，它跑在分离的线程上（Server 退出时不等它），但翻译线程平时要等它的结果，离线时不能每一页都试一次。
inline constexpr auto account_registration_retry = std::chrono::minutes(10);

// 一个候选是否可以问账号：共享层给每个候选算好的 online_gloss（只有中文候选，且不是表情、颜文字）。
struct AccountGlossCandidate {
  std::string text;
  bool online_gloss = false;
};

// 这一页要发给账号的词：可以问账号、本机词典还没有答上（answered 里带非空释义的不算）、也不在缓存里（known 返回真）的候选，按页上的顺序，同一个词只问一次。和 macOS synchronizeAccountGloss 一样，本机词典先答，账号只补它们留下的空。
template <typename Known>
std::vector<std::string> account_gloss_words(
    const std::vector<AccountGlossCandidate> &candidates,
    const std::vector<std::pair<std::string, std::string>> &answered, Known &&known) {
  std::vector<std::string> words;
  std::unordered_set<std::string> seen;
  for (const auto &candidate : candidates) {
    if (!candidate.online_gloss || candidate.text.empty() ||
        candidate.text.size() > account_gloss_maximum_word_bytes)
      continue;
    const bool local = std::any_of(answered.begin(), answered.end(), [&](const auto &entry) {
      return entry.first == candidate.text && !entry.second.empty();
    });
    if (local || known(candidate.text) || !seen.insert(candidate.text).second)
      continue;
    words.push_back(candidate.text);
    if (words.size() == account_gloss_maximum_words)
      break;
  }
  return words;
}

// 接口的 target_lang 是大写的语言代码（EN、JA）。代码只能是 ASCII 字母和连字符，空的或超过 16 字节的不发。
inline std::optional<std::string> account_gloss_target(std::string_view language) {
  if (language.empty() || language.size() > 16 || language == "zh")
    return std::nullopt;
  std::string target;
  target.reserve(language.size());
  for (const unsigned char ch : language) {
    if (!std::isalpha(ch) && ch != '-')
      return std::nullopt;
    target.push_back(static_cast<char>(std::toupper(ch)));
  }
  return target;
}

// Rust 解读过的回复（msime_client_account_gloss 的 parse，每项是释义或 null）换成每个词的释义：条数必须和问的词一样多，否则整批作废（返回空），这一页的词留到下次再问。null 换成空字符串，表示账号对这个词没有释义，调用方据此记一条否定缓存。规范化（制表符和换行折成空格、去首尾空白、与原文相同或含控制字符的置 null、超过 4096 字节的置 null）都在 Rust，一条坏值不再让整批作废。
inline std::optional<std::vector<std::string>> account_gloss_values(
    const std::vector<std::string> &words, std::vector<std::optional<std::string>> translations) {
  if (translations.size() != words.size())
    return std::nullopt;
  std::vector<std::string> values;
  values.reserve(translations.size());
  for (auto &translation : translations)
    values.push_back(translation ? std::move(*translation) : std::string{});
  return values;
}
} // namespace msime::windows

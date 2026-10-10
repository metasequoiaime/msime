#include "AccountGlossPolicy.h"
#include "CandidateTranslationPolicy.h"
#include "TranslationWorker.h"
#include "CandidateHttpPolicy.h"
#include "JapaneseReader.h"
#include "TranslationDisplay.h"

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
#include <ctime>
#include <memory>
#include <stdexcept>
#include <system_error>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace msime::windows {
namespace {
constexpr auto kDebounce = std::chrono::milliseconds(500);
constexpr size_t kMaximumQueryBytes = 65536;
constexpr size_t kMaximumResponseBytes = 1024 * 1024;
constexpr long kCustomTranslationTimeoutMs = 2500;
constexpr long kTencentTranslationTimeoutMs = 2000;
constexpr long kNiuTransTranslationTimeoutMs = 2500;
// 账号释义由服务端的模型回答，比机器翻译接口慢；macOS 等 30 秒，这里只有一个翻译线程，等太久会让后面的页都排着，所以取 10 秒。
constexpr long kAccountTranslationTimeoutMs = 10000;
constexpr auto kTranslationBatchBudget = std::chrono::seconds(6);
constexpr auto kNegativeTranslationTtl = std::chrono::minutes(8);
constexpr size_t kMaximumTranslationCacheEntries = 4096;

struct HttpResponse {
  std::string body;
  std::function<bool()> cancelled;
};

size_t write_response(char *data, size_t size, size_t count, void *context) {
  auto &response = *static_cast<HttpResponse *>(context);
  if (response.cancelled && response.cancelled())
    return 0;
  if (size > kMaximumResponseBytes ||
      response.body.size() > kMaximumResponseBytes ||
      (size != 0 &&
       count > (kMaximumResponseBytes - response.body.size()) / size))
    return 0;
  response.body.append(data, size * count);
  return size * count;
}

int transfer_progress(void *context, curl_off_t, curl_off_t, curl_off_t,
                      curl_off_t) {
  const auto &cancelled = *static_cast<const std::function<bool()> *>(context);
  return cancelled && cancelled() ? 1 : 0;
}

std::optional<nlohmann::json> host_value(char *raw) {
  auto value = msime::host_api::own_string(raw);
  if (!value)
    return std::nullopt;
  try {
    const auto document = nlohmann::json::parse(value.get());
    if (!document.value("ok", false) || !document.contains("value"))
      return std::nullopt;
    return document.at("value");
  } catch (...) {
    return std::nullopt;
  }
}

std::optional<nlohmann::json> query_document(const std::string &query) {
  if (query.empty() || query.size() > kMaximumQueryBytes)
    return std::nullopt;
  try {
    auto document = nlohmann::json::parse(query);
    if (!document.is_object() || !document.contains("generation") ||
        !document.at("generation").is_number_unsigned() ||
        !document.contains("target_language") ||
        !document.at("target_language").is_string() ||
        !document.contains("candidates") ||
        !document.at("candidates").is_array() ||
        document.at("candidates").empty() ||
        document.at("candidates").size() > 9)
      return std::nullopt;
    for (const auto &candidate : document.at("candidates"))
      if (!candidate.is_object() || !candidate.contains("text") ||
          !candidate.at("text").is_string() ||
          candidate.at("text").get<std::string>().empty() ||
          candidate.at("text").get<std::string>().size() > 4096)
        return std::nullopt;
    return document;
  } catch (...) {
    return std::nullopt;
  }
}

std::optional<std::string> http_request(const nlohmann::json &descriptor,
                                        const std::function<bool()> &cancelled,
                                        bool allow_http, long timeout_ms,
                                        long *status_out = nullptr) {
  if (status_out)
    *status_out = 0;
  try {
    if (!descriptor.is_object() || !descriptor.at("url").is_string())
      return std::nullopt;
    const auto url = descriptor.at("url").get<std::string>();
    if (!valid_candidate_url(url, allow_http))
      return std::nullopt;
    const auto headers = descriptor.value("headers", nlohmann::json::object());
    if (!headers.is_object())
      return std::nullopt;
    static std::once_flag curl_once;
    std::call_once(curl_once, [] { curl_global_init(CURL_GLOBAL_DEFAULT); });
    std::unique_ptr<CURL, decltype(&curl_easy_cleanup)> curl(curl_easy_init(),
                                                             curl_easy_cleanup);
    if (!curl)
      return std::nullopt;
    curl_slist *raw_headers = nullptr;
    std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)> request_headers(
        nullptr, curl_slist_free_all);
    for (auto it = headers.begin(); it != headers.end(); ++it) {
      if (!valid_candidate_header_name(it.key()) || !it.value().is_string() ||
          it.key().size() > 128 ||
          it.value().get<std::string>().size() > 8192 ||
          !valid_candidate_header_value(it.value().get<std::string>()))
        return std::nullopt;
      const auto line = it.key() + ": " + it.value().get<std::string>();
      raw_headers = curl_slist_append(raw_headers, line.c_str());
      if (!raw_headers)
        return std::nullopt;
      request_headers.reset(raw_headers);
    }
    HttpResponse response{{}, cancelled};
    curl_easy_setopt(curl.get(), CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl.get(), CURLOPT_PROTOCOLS_STR,
                     allow_http ? "http,https" : "https");
    curl_easy_setopt(curl.get(), CURLOPT_FOLLOWLOCATION, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT_MS, 2500L);
    curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT_MS, timeout_ms);
    curl_easy_setopt(curl.get(), CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(curl.get(), CURLOPT_USERAGENT, "MSIME-Client/1.0");
    curl_easy_setopt(curl.get(), CURLOPT_HTTPHEADER, request_headers.get());
    curl_easy_setopt(curl.get(), CURLOPT_POST, 1L);
    curl_easy_setopt(curl.get(), CURLOPT_WRITEFUNCTION, write_response);
    curl_easy_setopt(curl.get(), CURLOPT_WRITEDATA, &response);
    curl_easy_setopt(curl.get(), CURLOPT_NOPROGRESS, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFOFUNCTION, transfer_progress);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFODATA, &cancelled);
    std::string body;
    if (descriptor.contains("body_utf8")) {
      if (!descriptor.at("body_utf8").is_string())
        return std::nullopt;
      body = descriptor.at("body_utf8").get<std::string>();
    } else if (descriptor.contains("body")) {
      body = descriptor.at("body").dump();
    } else {
      return std::nullopt;
    }
    if (body.size() > kMaximumResponseBytes)
      return std::nullopt;
    curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDS, body.data());
    curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDSIZE_LARGE,
                     static_cast<curl_off_t>(body.size()));
    const auto result = curl_easy_perform(curl.get());
    long status = 0;
    curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &status);
    if (status_out && result == CURLE_OK)
      *status_out = status;
    if (result != CURLE_OK || status < 200 || status >= 300 ||
        (cancelled && cancelled()))
      return std::nullopt;
    return std::move(response.body);
  } catch (...) {
    return std::nullopt;
  }
}

std::optional<std::string>
custom_translation(const nlohmann::json &config, const nlohmann::json &item,
                   const std::function<bool()> &cancelled) {
  const auto request =
      nlohmann::json{{"config", config},
                     {"text", item.at("key")},
                     {"source_language", item.at("source_language")},
                     {"target_language", item.at("target_language")}};
  const auto bytes = request.dump();
  auto descriptor = host_value(msime_client_custom_translation_http_request(
      reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
  if (!descriptor || descriptor->is_null() || cancelled())
    return std::nullopt;
  // The source Windows client permits an explicitly configured HTTP custom
  // translator (useful for a local service).  Keep cloud providers HTTPS-only.
  auto body =
      http_request(*descriptor, cancelled, true, kCustomTranslationTimeoutMs);
  if (!body || cancelled())
    return std::nullopt;
  auto parsed = host_value(msime_client_parse_custom_translation_response(
      reinterpret_cast<const uint8_t *>(body->data()), body->size()));
  if (!parsed || !parsed->is_string() || parsed->get<std::string>().empty())
    return std::nullopt;
  return parsed->get<std::string>();
}

void append_tencent_group(const nlohmann::json &config,
                          const std::vector<nlohmann::json> &items,
                          std::string &translations,
                          const std::function<bool()> &cancelled) {
  if (items.empty() || cancelled())
    return;
  nlohmann::json texts = nlohmann::json::array();
  for (const auto &item : items)
    texts.push_back(item.at("key"));
  const auto request =
      nlohmann::json{{"config", config},
                     {"texts", texts},
                     {"source_language", items.front().at("source_language")},
                     {"target_language", items.front().at("target_language")},
                     {"timestamp", static_cast<int64_t>(std::time(nullptr))}};
  const auto bytes = request.dump();
  auto descriptor = host_value(msime_client_tencent_translation_http_request(
      reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
  if (!descriptor || descriptor->is_null())
    return;
  auto body =
      http_request(*descriptor, cancelled, false, kTencentTranslationTimeoutMs);
  if (!body || cancelled())
    return;
  auto parsed = host_value(msime_client_parse_tencent_translation_response(
      reinterpret_cast<const uint8_t *>(body->data()), body->size(),
      items.size()));
  if (!parsed || !parsed->is_array() || parsed->size() != items.size())
    return;
  try {
    auto output = nlohmann::json::parse(translations);
    for (size_t i = 0; i < items.size(); ++i)
      if (!parsed->at(i).is_null() && parsed->at(i).is_string())
        output.push_back(
            {{"text", items[i].at("text")}, {"translation", parsed->at(i)}});
    translations = output.dump();
  } catch (...) {
  }
}

std::string millisecond_timestamp() {
  const auto now = std::chrono::system_clock::now().time_since_epoch();
  return std::to_string(
      std::chrono::duration_cast<std::chrono::milliseconds>(now).count());
}

void append_niutrans_item(const nlohmann::json &config,
                          const nlohmann::json &item, std::string &translations,
                          const std::function<bool()> &cancelled) {
  if (cancelled())
    return;
  const auto timestamp = millisecond_timestamp();
  const auto request =
      nlohmann::json{{"config", config},
                     {"text", item.at("key")},
                     {"source_language", item.at("source_language")},
                     {"target_language", item.at("target_language")},
                     {"timestamp", timestamp}};
  const auto bytes = request.dump();
  auto descriptor = host_value(msime_client_niutrans_translation_http_request(
      reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
  if (!descriptor || descriptor->is_null() || cancelled())
    return;
  auto body = http_request(*descriptor, cancelled, false,
                           kNiuTransTranslationTimeoutMs);
  if (!body || cancelled())
    return;
  auto parsed = host_value(msime_client_parse_niutrans_translation_response(
      reinterpret_cast<const uint8_t *>(body->data()), body->size()));
  if (!parsed || !parsed->is_string() || parsed->get<std::string>().empty())
    return;
  try {
    auto output = nlohmann::json::parse(translations);
    output.push_back({{"text", item.at("text")}, {"translation", *parsed}});
    translations = output.dump();
  } catch (...) {
  }
}

std::mutex &account_directory_mutex() {
  static std::mutex mutex;
  return mutex;
}

std::string &account_directory_storage() {
  static std::string directory;
  return directory;
}

std::string account_directory() {
  std::lock_guard lock(account_directory_mutex());
  return account_directory_storage();
}

// 取账号令牌：设置应用登录的账号优先，没有登录时用本机匿名账号（msime_client_account_access_token）。两种会话都没有时（安装后首次注册失败、文件被删）补注册匿名账号再取一次，和 macOS BackendCandidateGloss.translationSession 一样；补注册失败后 account_registration_retry 之内不再试。令牌只在内存里交给请求头，不写日志。
std::optional<std::string> account_access_token(const std::string &directory,
                                                const std::string &rejected) {
  static std::mutex registration_mutex;
  static std::optional<std::chrono::steady_clock::time_point> last_registration;
  auto request = nlohmann::json{{"directory", directory}};
  if (!rejected.empty())
    request["rejected_token"] = rejected;
  std::string error;
  const auto ask = [&]() -> std::optional<std::string> {
    error.clear();
    const auto bytes = request.dump();
    auto raw = msime::host_api::own_string(msime_client_account_access_token(
        reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
    if (!raw)
      return std::nullopt;
    try {
      const auto document = nlohmann::json::parse(raw.get());
      if (!document.value("ok", false)) {
        if (document.contains("error") && document.at("error").is_string())
          error = document.at("error").get<std::string>();
        return std::nullopt;
      }
      const auto &value = document.at("value");
      if (!value.is_object() || !value.contains("access_token") ||
          !value.at("access_token").is_string() ||
          value.at("access_token").get<std::string>().empty())
        return std::nullopt;
      return value.at("access_token").get<std::string>();
    } catch (...) {
      return std::nullopt;
    }
  };
  if (auto token = ask())
    return token;
  if (error != "account_unauthorized")
    return std::nullopt;
  {
    std::lock_guard lock(registration_mutex);
    const auto now = std::chrono::steady_clock::now();
    if (last_registration && now < *last_registration + account_registration_retry)
      return std::nullopt;
    last_registration = now;
  }
  msime::host_api::discard_string(msime_client_ensure_anonymous_account(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
  request.erase("rejected_token");
  return ask();
}

// 在一条分离的线程上取账号令牌（account_access_token），翻译线程每 50 毫秒看一次 stopping：补注册匿名账号会阻塞约一分钟，刷新令牌也要联网，都不能让 TranslationWorker::stop() 的 join 跟着等，Server 退出时不等它，由那条线程自己跑完后退出，和 server_main 启动时注册匿名账号的线程一样。线程起不来时这一页不问账号。
std::optional<std::string> account_access_token_detached(const std::string &directory,
                                                         const std::string &rejected,
                                                         const std::function<bool()> &stopping) {
  struct Pending {
    std::mutex mutex;
    std::condition_variable done;
    bool finished = false;
    std::optional<std::string> token;
  };
  auto pending = std::make_shared<Pending>();
  try {
    std::thread([pending, directory, rejected] {
      auto token = account_access_token(directory, rejected);
      std::lock_guard lock(pending->mutex);
      pending->token = std::move(token);
      pending->finished = true;
      pending->done.notify_all();
    }).detach();
  } catch (const std::system_error &) {
    return std::nullopt;
  }
  std::unique_lock lock(pending->mutex);
  while (!pending->done.wait_for(lock, std::chrono::milliseconds(50), [&] { return pending->finished; }))
    if (stopping())
      return std::nullopt;
  return std::move(pending->token);
}

void persist_english_glosses(const nlohmann::json &query,
                             const std::string &translations) noexcept;

// 「水杉账号」的候选释义（macOS BackendCandidateGloss + InputController synchronizeAccountGloss）：translations 是本机词典已经答上的结果，账号只问它们留下的、共享层标了 online_gloss 的中文候选，一次一个 POST，一种目标语言（两种目标语言时上层按语言各调一次）。回复按语言和词缓存，账号没有释义的词记否定缓存，八分钟内不再问；请求失败（离线、限流）什么也不记，下一页再问。请求只在 Server 退出时中止，换页不中止：macOS 也让在途的请求跑完，它的回答进缓存，用户退回这一页或再打这个词时直接用。返回空表示这一页已被更新的请求取代。
std::optional<std::string> account_glosses(
    const nlohmann::json &query, std::string translations,
    std::unordered_map<std::string, std::string> &cache,
    std::unordered_map<std::string, std::chrono::steady_clock::time_point> &negative,
    const std::function<bool()> &cancelled, const std::function<bool()> &stopping) {
  const auto directory = account_directory();
  const auto language = query.at("target_language").get<std::string>();
  const auto target = account_gloss_target(language);
  if (directory.empty() || !target)
    return translations;
  auto output = nlohmann::json::parse(translations);
  if (!output.is_array())
    return translations;
  std::vector<std::pair<std::string, std::string>> answered;
  for (const auto &entry : output)
    if (entry.is_object())
      answered.emplace_back(entry.value("text", std::string{}),
                            entry.value("translation", std::string{}));
  std::vector<AccountGlossCandidate> candidates;
  for (const auto &candidate : query.at("candidates"))
    candidates.push_back({candidate.at("text").get<std::string>(),
                          candidate.value("online_gloss", false)});
  const auto cache_id = [&](const std::string &word) {
    return nlohmann::json{{"provider", "account"}, {"target_language", language}, {"key", word}}
        .dump();
  };
  // 缓存里已有的释义先放进去，不等网络。
  for (const auto &candidate : candidates) {
    if (!candidate.online_gloss)
      continue;
    const bool local = std::any_of(answered.begin(), answered.end(), [&](const auto &entry) {
      return entry.first == candidate.text && !entry.second.empty();
    });
    if (local)
      continue;
    if (const auto cached = cache.find(cache_id(candidate.text)); cached != cache.end()) {
      output.push_back({{"text", candidate.text}, {"translation", cached->second}});
      answered.emplace_back(candidate.text, cached->second);
    }
  }
  const auto now = std::chrono::steady_clock::now();
  const auto words = account_gloss_words(candidates, answered, [&](const std::string &word) {
    const auto found = negative.find(cache_id(word));
    return found != negative.end() && found->second > now;
  });
  if (words.empty() || cancelled())
    return cancelled() ? std::nullopt : std::optional<std::string>(output.dump());
  auto token = account_access_token_detached(directory, {}, stopping);
  if (!token)
    return output.dump();
  const nlohmann::json body{{"texts", words}, {"source_lang", "ZH"}, {"target_lang", *target}};
  long status = 0;
  std::optional<std::string> response;
  for (int attempt = 0; attempt < 2 && token; ++attempt) {
    const nlohmann::json descriptor{
        {"url", std::string(account_gloss_url)},
        {"headers", {{"Authorization", "Bearer " + *token}, {"Content-Type", "application/json"}}},
        {"body", body}};
    response = http_request(descriptor, stopping, false, kAccountTranslationTimeoutMs, &status);
    // 服务端以 401 拒绝了这个令牌（过期、被吊销）：带上它再取一次，共享层会强制刷新，刷新不了时退回匿名账号。
    if (response || status != 401 || stopping())
      break;
    token = account_access_token_detached(directory, *token, stopping);
  }
  if (!response || stopping())
    return cancelled() ? std::nullopt : std::optional<std::string>(output.dump());
  std::optional<std::vector<std::string>> values;
  try {
    const auto document = nlohmann::json::parse(*response);
    const auto code = document.is_object() ? document.value("code", nlohmann::json(nullptr))
                                           : nlohmann::json(nullptr);
    const bool ok = (code.is_number_integer() && code.get<int64_t>() == 200) ||
                    (code.is_string() && code.get<std::string>() == "200");
    if (ok && document.contains("data") && document.at("data").is_array()) {
      std::vector<std::string> data;
      bool strings = true;
      for (const auto &value : document.at("data")) {
        if (!value.is_string()) {
          strings = false;
          break;
        }
        data.push_back(value.get<std::string>());
      }
      if (strings)
        values = account_gloss_values(words, std::move(data));
    }
  } catch (...) {
  }
  if (!values)
    return cancelled() ? std::nullopt : std::optional<std::string>(output.dump());
  auto learned = nlohmann::json::array();
  for (size_t index = 0; index < words.size(); ++index) {
    const auto id = cache_id(words[index]);
    const auto &value = (*values)[index];
    if (value.empty()) {
      if (cache.find(id) == cache.end()) {
        if (negative.size() >= kMaximumTranslationCacheEntries)
          negative.clear();
        negative[id] = std::chrono::steady_clock::now() + kNegativeTranslationTtl;
      }
      continue;
    }
    if (cache.size() >= kMaximumTranslationCacheEntries)
      cache.clear();
    cache[id] = value;
    negative.erase(id);
    output.push_back({{"text", words[index]}, {"translation", value}});
    learned.push_back({{"text", words[index]}, {"translation", value}});
  }
  // 英文释义存进本机学到的释义表，和其他服务一样（persist_english_glosses 只在目标语言是英文、查询带着用户目录时写）。问账号的都是本机词典没答上的词，存进去不会盖掉随包的释义。
  if (!learned.empty())
    persist_english_glosses(query, learned.dump());
  if (cancelled())
    return std::nullopt;
  return output.dump();
}

void persist_english_glosses(const nlohmann::json &query,
                             const std::string &translations) noexcept {
  try {
    if (query.value("target_language", std::string{}) != "en")
      return;
    const auto user_data = query.value("user_data", std::string{});
    if (user_data.empty())
      return;
    const auto values = nlohmann::json::parse(translations);
    if (!values.is_array() || values.empty())
      return;
    const auto request = nlohmann::json{
        {"target_language", "en"},
        {"translations",
         values}}.dump();
    msime::host_api::discard_string(msime_client_translation_gloss_save(
        reinterpret_cast<const uint8_t *>(request.data()), request.size(),
        reinterpret_cast<const uint8_t *>(user_data.data()), user_data.size()));
  } catch (...) {
    // Persistence is an optional display-data side effect. A missing or
    // temporarily unavailable user dictionary must not hide translations.
  }
}

// 只请求一种目标语言的结果只有一行释义：来源自带的 U+2028、U+2029 折成空格，免得候选窗读回时多出一行。两种目标语言的结果由 join_translation_lines 拼好，行分隔是有意的，不动。
void flatten_single_target(const std::string &query_bytes,
                           TranslationWorker::Result &result) {
  const auto query = query_document(query_bytes);
  if (!query)
    return;
  const auto targets = query->value("target_languages", nlohmann::json::array());
  if (targets.is_array() && targets.size() > 1)
    return;
  auto values = nlohmann::json::parse(result.translations, nullptr, false);
  if (!values.is_array())
    return;
  for (auto &value : values)
    if (value.is_object() && value.contains("translation") &&
        value.at("translation").is_string())
      value["translation"] =
          flatten_translation_line(value.at("translation").get<std::string>());
  result.translations = values.dump();
}
// 日文行的罗马字由本线程上的 IFELanguage 读出（JapaneseReader.h）：TranslationWorker::run 在工作线程的栈上持有一个读音器，经这个指针交给本线程上的读音查询，run 返回时它随栈析构关闭，COM 套间跟着线程。不在工作线程上（指针为空）时不读罗马字。这里不放 thread_local 的读音器对象本身：它的析构要等线程退出回调，那时持着加载器锁，不能 CoUninitialize；MinGW 构建的 thread_local 析构在 Wine 下还会跳到空地址崩溃（windows-translation-worker 测试）。
thread_local JapaneseReader *active_japanese_reader = nullptr;
// 这一页候选的读音和整句逐词拆解，和 macOS 的 synchronizePronunciation、synchronizeGlossBreakdowns 一样：读音只在打开「显示读音」时问，英文释义行（英文候选则是它自己）整行发给共享读音表，日文释义行的第一个词交给本机的微软日语输入法读成罗马字；拆解在查询带 gloss_breakdown 时问（候选翻译或离线英文释义打开、目标语言里有英文，和 macOS 的 currentGlossRequest 一样），2 到 32 个汉字的候选发给共享拆解表。两张表都装在资源目录旁边，没装时共享层回答空列表，不是错误。都在本线程上读本机文件，不联网。
CandidateReadings candidate_readings(const std::string &query_bytes,
                                     const std::string &translations,
                                     const std::function<bool()> &cancelled) {
  CandidateReadings readings;
  const auto query = query_document(query_bytes);
  if (!query)
    return readings;
  const auto resources = query->value("resources", nlohmann::json(nullptr));
  if (!resources.is_string() || resources.get<std::string>().empty())
    return readings;
  const auto resource_path = resources.get<std::string>();
  const auto generation = query->at("generation").get<uint64_t>();
  // 候选文字和它眼下的释义，按页上的顺序。
  std::vector<std::pair<std::string, std::string>> page;
  {
    std::unordered_map<std::string, std::string> glossed;
    const auto values = nlohmann::json::parse(translations, nullptr, false);
    if (values.is_array())
      for (const auto &value : values)
        if (value.is_object() && value.contains("text") && value.at("text").is_string() &&
            value.contains("translation") && value.at("translation").is_string())
          glossed.emplace(value.at("text").get<std::string>(),
                          value.at("translation").get<std::string>());
    for (const auto &candidate : query->at("candidates")) {
      const auto text = candidate.at("text").get<std::string>();
      const auto found = glossed.find(text);
      page.emplace_back(text, found == glossed.end() ? std::string{} : found->second);
    }
  }
  if (query->value("candidate_pronunciation", false)) {
    std::vector<std::string> targets;
    const auto languages = query->value("target_languages", nlohmann::json::array());
    if (languages.is_array())
      for (const auto &language : languages)
        if (language.is_string())
          targets.push_back(language.get<std::string>());
    if (targets.empty())
      targets.push_back(query->at("target_language").get<std::string>());
    auto items = nlohmann::json::array();
    std::unordered_set<std::string> asked;
    for (const auto &[text, translation] : page)
      for (auto &english : gloss_english_texts(text, translation_lines(translation), targets))
        if (asked.insert(english).second)
          items.push_back({{"text", english}, {"language", "en"}});
    std::unordered_map<std::string, std::string> answered;
    if (!items.empty() && !cancelled()) {
      const auto request =
          nlohmann::json{{"generation", generation}, {"items", std::move(items)}}.dump();
      if (const auto value = host_value(msime_client_pronunciation_request(
              reinterpret_cast<const uint8_t *>(request.data()), request.size(),
              reinterpret_cast<const uint8_t *>(resource_path.data()), resource_path.size()));
          value && value->is_object())
        for (const auto &entry : value->value("pronunciations", nlohmann::json::array()))
          if (entry.is_object() && entry.contains("text") && entry.at("text").is_string() &&
              entry.contains("pronunciation") && entry.at("pronunciation").is_string())
            answered.emplace(entry.at("text").get<std::string>(),
                             entry.at("pronunciation").get<std::string>());
    }
    const auto english = [&](const std::string &text) {
      const auto found = answered.find(text);
      return found == answered.end() ? std::string{} : found->second;
    };
    const auto japanese = [&](const std::string &term) {
      if (cancelled() || !active_japanese_reader)
        return std::string{};
      return active_japanese_reader->romaji(term);
    };
    for (const auto &[text, translation] : page) {
      const auto lines =
          gloss_pronunciation_lines(text, translation_lines(translation), targets, english, japanese);
      if (lines.empty())
        continue;
      std::string joined;
      for (size_t index = 0; index < lines.size(); ++index) {
        if (index)
          joined.push_back('\n');
        joined.append(lines[index]);
      }
      auto &reading = readings[text];
      reading.translation = translation;
      reading.pronunciation = std::move(joined);
    }
  }
  if (query->value("gloss_breakdown", false) && !cancelled()) {
    auto sentences = nlohmann::json::array();
    std::unordered_set<std::string> asked;
    for (const auto &[text, translation] : page)
      if (gloss_sentence_candidate(text) && asked.insert(text).second)
        sentences.push_back(text);
    if (!sentences.empty()) {
      const auto request =
          nlohmann::json{{"generation", generation}, {"texts", std::move(sentences)}}.dump();
      if (const auto value = host_value(msime_client_gloss_breakdown_request(
              reinterpret_cast<const uint8_t *>(request.data()), request.size(),
              reinterpret_cast<const uint8_t *>(resource_path.data()), resource_path.size()));
          value && value->is_object())
        for (const auto &entry : value->value("breakdowns", nlohmann::json::array()))
          if (entry.is_object() && entry.contains("text") && entry.at("text").is_string() &&
              entry.contains("breakdown") && entry.at("breakdown").is_string() &&
              !entry.at("breakdown").get<std::string>().empty())
            readings[entry.at("text").get<std::string>()].breakdown =
                entry.at("breakdown").get<std::string>();
    }
  }
  return readings;
}
} // namespace

TranslationWorker::TranslationWorker(Completed completed)
    : TranslationWorker(std::move(completed), Translator{}) {}

TranslationWorker::TranslationWorker(Completed completed, Translator translator)
    : completed_(std::move(completed)), translate_(std::move(translator)) {
  if (!completed_)
    throw std::invalid_argument("Missing translation completion");
  if (!translate_)
    translate_ = [this](const FocusLease &lease, const std::string &query,
                        const std::function<bool()> &cancelled) {
      auto result = translate(lease, query, cancelled);
      if (result) {
        flatten_single_target(query, *result);
        result->readings = candidate_readings(query, result->translations, cancelled);
      } else if (!cancelled()) {
        // 这一页没有任何释义，整句候选仍可能有逐词拆解：只带读音和拆解回去，释义是空列表，SessionController 见到它就不交给会话。
        auto readings = candidate_readings(query, "[]", cancelled);
        const auto document = query_document(query);
        if (!readings.empty() && document && !cancelled())
          result = Result{lease, document->at("generation").get<uint64_t>(), "[]",
                          std::move(readings)};
      }
      return result;
    };
  worker_ = std::thread([this] { run(); });
}

TranslationWorker::~TranslationWorker() { stop(); }

void TranslationWorker::set_account_directory(std::string directory) {
  std::lock_guard lock(account_directory_mutex());
  account_directory_storage() = std::move(directory);
}

bool TranslationWorker::submit(const FocusLease &lease, std::string query) {
  if (!lease.epoch || !lease.token || query.empty() ||
      query.size() > kMaximumQueryBytes)
    return false;
  std::lock_guard lock(mutex_);
  if (stopping_)
    return false;
  const uint64_t serial = ++next_serial_;
  pending_ = Request{lease, std::move(query), serial};
  latest_serial_.store(serial, std::memory_order_release);
  wake_.notify_one();
  return true;
}

void TranslationWorker::clear_cache() {
  clear_cache_requested_.store(true, std::memory_order_release);
  latest_serial_.fetch_add(1, std::memory_order_acq_rel);
  {
    std::lock_guard lock(mutex_);
    pending_.reset();
  }
  wake_.notify_one();
}

void TranslationWorker::request_stop() {
  stopping_.store(true, std::memory_order_release);
  {
    std::lock_guard lock(mutex_);
    pending_.reset();
  }
  wake_.notify_all();
}

void TranslationWorker::stop() {
  std::lock_guard join(join_mutex_);
  request_stop();
  if (worker_.joinable()) {
    if (worker_.get_id() == std::this_thread::get_id())
      throw std::logic_error("Cannot join translation worker from itself");
    worker_.join();
  }
}

bool TranslationWorker::cancelled(uint64_t serial) const noexcept {
  return stopping_.load(std::memory_order_acquire) ||
         latest_serial_.load(std::memory_order_acquire) != serial;
}

std::optional<TranslationWorker::Result>
TranslationWorker::translate(const FocusLease &lease, const std::string &query_bytes,
                             const std::function<bool()> &cancelled) {
  const auto document = query_document(query_bytes);
  if (!document || cancelled())
    return std::nullopt;
  try {
    const auto &query = *document;
    const auto generation = query.at("generation").get<uint64_t>();

    // `/fy` (command mode): one English sentence for the service the user selected, into the query's own target language, whatever the gloss switches say (command_translation_item in CandidateTranslationPolicy.h). The selected service alone is asked, in the same precedence as below, with no packaged gloss and no gloss cache, and its answer goes back through apply_translations, which makes it the command's first row.
    if (query.value("sentence", false)) {
      const auto &candidates = query.at("candidates");
      std::vector<std::string> texts;
      texts.reserve(candidates.size());
      for (const auto &candidate : candidates)
        texts.push_back(candidate.at("text").get<std::string>());
      const auto command = msime::windows::command_translation_item(
          true, texts, query.at("target_language").get<std::string>());
      if (!command)
        return std::nullopt;
      const nlohmann::json item{{"text", command->text},
                                {"key", command->text},
                                {"source_language", command->source_language},
                                {"target_language", command->target_language}};
      auto translations = nlohmann::json::array().dump();
      const auto niutrans = query.value("niutrans", nlohmann::json(nullptr));
      const auto custom =
          query.value("custom_translation", nlohmann::json(nullptr));
      const auto tencent = query.value("tencent_tmt", nlohmann::json(nullptr));
      if (niutrans.is_object() && niutrans.value("enabled", false)) {
        append_niutrans_item(niutrans, item, translations, cancelled);
      } else if (custom.is_object() && custom.value("enabled", false)) {
        if (auto value = custom_translation(custom, item, cancelled))
          translations = nlohmann::json::array(
                             {{{"text", command->text}, {"translation", *value}}})
                             .dump();
      } else if (tencent.is_object() && tencent.value("enabled", false)) {
        append_tencent_group(tencent, std::vector<nlohmann::json>{item},
                             translations, cancelled);
      }
      if (cancelled() || nlohmann::json::parse(translations).empty())
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, translations};
    }

    // 宿主只有一个候选释义字段，而偏好可以要两种目标语言。每种语言单独翻译，再按偏好顺序每种语言一行拼起来（join_translation_lines，行间是 U+2028），和 macOS 一样第 N 行始终是第 N 种目标语言，某种语言没有释义时留空行。递归调用只带一种目标语言，所以每一行走的服务、缓存、取消和持久化路径都和单语言时完全相同。
    const auto target_languages = query.value("target_languages",
                                              nlohmann::json::array());
    if (target_languages.is_array() && target_languages.size() > 1) {
      // 候选文字按第一次出现的顺序排，每个候选一组按目标语言排列的行。
      std::vector<std::pair<std::string, std::vector<std::string>>> merged;
      std::unordered_map<std::string, size_t> positions;
      size_t line = 0;
      for (const auto &target : target_languages) {
        if (!target.is_string() || target.get<std::string>().empty())
          continue;
        const size_t column = line++;
        auto single = query;
        single["target_language"] = target;
        single["target_languages"] = nlohmann::json::array({target});
        // Packaged glosses are specifically English-target data. A secondary
        // non-English row must never display that gloss as its translation.
        single["english_gloss"] =
            query.value("english_gloss", false) && target == "en";
        const auto bytes = single.dump();
        auto child = translate(lease, bytes, cancelled);
        if (!child)
          continue;
        try {
          const auto values = nlohmann::json::parse(child->translations);
          if (!values.is_array())
            continue;
          for (const auto &value : values) {
            if (!value.is_object() || !value.contains("text") ||
                !value.at("text").is_string() ||
                !value.contains("translation") ||
                !value.at("translation").is_string())
              continue;
            const auto text = value.at("text").get<std::string>();
            const auto translation = value.at("translation").get<std::string>();
            if (translation.empty())
              continue;
            const auto [it, inserted] = positions.emplace(text, merged.size());
            if (inserted)
              merged.push_back({text, {}});
            auto &lines = merged.at(it->second).second;
            if (lines.size() <= column)
              lines.resize(column + 1);
            // 同一种语言对同一个候选答了两次时只留第一次。
            if (lines[column].empty())
              lines[column] = translation;
          }
        } catch (...) {
          continue;
        }
      }
      auto joined = nlohmann::json::array();
      for (const auto &[text, lines] : merged) {
        auto translation = join_translation_lines(lines);
        if (!translation.empty())
          joined.push_back({{"text", text}, {"translation", std::move(translation)}});
      }
      if (cancelled() || joined.empty())
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, joined.dump()};
    }

    // The packaged glosses for this page: English without a target language,
    // or one installed non-English dictionary with it. Nothing here reaches
    // the network, and a page with no dictionary entry is not a failure.
    const auto packaged_glosses =
        [&](const std::string &target) -> std::optional<nlohmann::json> {
      const auto resources = query.value("resources", std::string{});
      if (resources.empty())
        return std::nullopt;
      auto gloss_request = nlohmann::json{{"generation", generation}};
      const auto user_data = query.value("user_data", std::string{});
      if (!user_data.empty())
        gloss_request["user_data"] = user_data;
      if (!target.empty())
        gloss_request["target_language"] = target;
      auto gloss_candidates = nlohmann::json::array();
      for (const auto &candidate : query.at("candidates"))
        gloss_candidates.push_back(
            {{"text", candidate.at("text")}, {"source", 0}});
      gloss_request["candidates"] = std::move(gloss_candidates);
      const auto gloss_bytes = gloss_request.dump();
      auto glossed = host_value(msime_client_candidate_gloss_request(
          reinterpret_cast<const uint8_t *>(gloss_bytes.data()),
          gloss_bytes.size(),
          reinterpret_cast<const uint8_t *>(resources.data()),
          resources.size()));
      if (glossed && glossed->is_object() &&
          glossed->value("translations", nlohmann::json::array()).is_array())
        return glossed->at("translations");
      return std::nullopt;
    };

    // A non-English target with its offline dictionary installed. The shared
    // query ranks the user's own translator above that dictionary, so the
    // providers are asked first, through this same function with the
    // dictionary taken out of the query, and the dictionary fills only the
    // candidates they left (fill_offline_glosses in
    // CandidateTranslationPolicy.h).
    const auto target = query.at("target_language").get<std::string>();
    const auto offline_languages =
        query.value("offline_gloss_languages", nlohmann::json::array());
    if (target != "en" && offline_languages.is_array() &&
        std::find(offline_languages.begin(), offline_languages.end(),
                  target) != offline_languages.end()) {
      auto online_query = query;
      online_query.erase("offline_gloss_languages");
      const auto offline = packaged_glosses(target);
      // 水杉账号只补本机词典留下的空（macOS synchronizeAccountGloss）：词典已经答上的候选不再发给账号。用户自己的翻译服务排在词典前面，照旧整页都问。
      if (query.value("translation_account", false) && offline && offline->is_array()) {
        std::unordered_set<std::string> known;
        for (const auto &entry : *offline)
          if (entry.is_object() && !entry.value("translation", std::string{}).empty())
            known.insert(entry.value("text", std::string{}));
        auto remaining = nlohmann::json::array();
        for (const auto &candidate : query.at("candidates"))
          if (known.find(candidate.at("text").get<std::string>()) == known.end())
            remaining.push_back(candidate);
        online_query["candidates"] = std::move(remaining);
      }
      std::optional<TranslationWorker::Result> online;
      if (!online_query.at("candidates").empty())
        online = translate(lease, online_query.dump(), cancelled);
      if (cancelled())
        return std::nullopt;
      std::vector<std::pair<std::string, std::string>> answered;
      std::vector<std::pair<std::string, std::string>> dictionary;
      const auto entries = [](const nlohmann::json &values, auto &into) {
        if (!values.is_array())
          return;
        into.reserve(values.size());
        for (const auto &entry : values)
          if (entry.is_object())
            into.emplace_back(entry.value("text", std::string{}),
                              entry.value("translation", std::string{}));
      };
      if (online) {
        const auto parsed = nlohmann::json::parse(online->translations);
        if (parsed.is_array())
          answered.reserve(parsed.size());
        entries(parsed, answered);
      }
      if (offline) {
        if (offline->is_array())
          dictionary.reserve(offline->size());
        entries(*offline, dictionary);
      }
      msime::windows::fill_offline_glosses(answered, dictionary);
      auto merged = nlohmann::json::array();
      for (const auto &[text, translation] : answered)
        if (!text.empty() && !translation.empty())
          merged.push_back({{"text", text}, {"translation", translation}});
      if (merged.empty())
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, merged.dump()};
    }

    auto translations = nlohmann::json::array().dump();
    // The offline English gloss comes from a packaged dictionary, so it is
    // resolved before any provider is consulted and never reaches the network.
    // It is also the only source available when no online provider is
    // configured, which is the usual case.
    if (query.value("english_gloss", false)) {
      auto glossed = packaged_glosses(std::string{});
      if (cancelled())
        return std::nullopt;
      if (glossed)
        translations = glossed->dump();
      // No gloss for this page. Fall through: an online provider may still be
      // configured, and a page with no dictionary entry is not a failure.
    }
    // 用户选了「水杉账号」（共享层已经判过候选释义开着、没有用户自己的服务在前）：本机英文释义之外的中文候选问账号。不走下面的翻译计划：账号只翻译中文候选，英文候选的反向翻译是用户自己服务的事。
    if (query.value("translation_account", false)) {
      auto answered = account_glosses(query, translations, translation_cache_,
                                      translation_negative_cache_, cancelled, [this] {
                                        return stopping_.load(std::memory_order_acquire);
                                      });
      if (!answered || cancelled() || *answered == "[]")
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, std::move(*answered)};
    }
    const auto plan_request = nlohmann::json{
        {"target_language", query.at("target_language")},
        {"candidates", [&] {
           auto candidates = nlohmann::json::array();
           for (const auto &candidate : query.at("candidates"))
             candidates.push_back(
                 {{"text", candidate.at("text")}, {"source", 0}});
           return candidates;
         }()}};
    const auto plan_bytes = plan_request.dump();
    auto plan = host_value(msime_client_custom_translation_plan(
        reinterpret_cast<const uint8_t *>(plan_bytes.data()),
        plan_bytes.size()));
    if (!plan || !plan->is_array() || cancelled())
      return std::nullopt;

    // Keep local dictionary hits and ask an online provider only for misses.
    // This mirrors the source worker's local-first merge behavior instead of
    // treating one offline hit as a complete page. The rule itself is
    // `untranslated_texts` in CandidateTranslationPolicy.h, which is where it
    // can be read and tested without a provider, a page or a session.
    std::vector<std::pair<std::string, std::string>> answered;
    try {
      const auto parsed = nlohmann::json::parse(translations);
      if (parsed.is_array())
        answered.reserve(parsed.size());
      for (const auto &entry : parsed)
        if (entry.is_object())
          answered.emplace_back(entry.value("text", std::string{}),
                                entry.value("translation", std::string{}));
    } catch (...) {
      return std::nullopt;
    }
    std::unordered_set<std::string> translated_texts;
    for (const auto &entry : answered)
      if (!entry.first.empty())
        translated_texts.insert(entry.first);
    if (plan->empty()) {
      if (translated_texts.empty())
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, translations};
    }

    // Translation results are valid across candidate generations. Cache each
    // item independently so one provider miss does not suppress retries for
    // unrelated candidates. Keys deliberately exclude credentials and the
    // generation.
    std::string provider_scope;
    const auto local_result =
        [&]() -> std::optional<TranslationWorker::Result> {
      if (translations == "[]")
        return std::nullopt;
      return TranslationWorker::Result{lease, generation, translations};
    };
    const auto niutrans = query.value("niutrans", nlohmann::json(nullptr));
    const auto custom =
        query.value("custom_translation", nlohmann::json(nullptr));
    if (niutrans.is_object() && niutrans.value("enabled", false)) {
      // NiuTrans app_id identifies the account used for the request. Keep it
      // in the cache scope so switching accounts cannot reuse another
      // account's glosses; the secret itself never enters the cache key.
      provider_scope =
          "niutrans:" + niutrans.value("app_id", std::string{});
    } else if (custom.is_object() && custom.value("enabled", false)) {
      provider_scope = "custom:" + custom.value("endpoint", std::string{});
    } else {
      const auto tencent = query.value("tencent_tmt", nlohmann::json(nullptr));
      if (!tencent.is_object() || !tencent.value("enabled", false))
        return local_result();
      provider_scope = "tencent";
    }
    const auto target_language = query.at("target_language");
    const auto item_cache_id = [&](const nlohmann::json &item) {
      return nlohmann::json{
          {"provider", provider_scope},
          {"target_language", target_language},
          {"key", item.at("key")},
          {"direction", item.at("source_language").get<std::string>() + ">" +
                            item.at("target_language").get<std::string>()},
          {"source_language", item.at("source_language")},
          {"item_target_language", item.at("target_language")}}
          .dump();
    };
    std::vector<std::string> planned;
    planned.reserve(plan->size());
    for (const auto &item : *plan)
      planned.push_back(item.at("text").get<std::string>());
    const auto wanted = msime::windows::untranslated_texts(answered, planned);
    const std::unordered_set<std::string> wanted_texts(wanted.begin(),
                                                       wanted.end());
    std::vector<nlohmann::json> pending;
    pending.reserve(plan->size());
    for (const auto &item : *plan) {
      if (wanted_texts.find(item.at("text").get<std::string>()) ==
          wanted_texts.end())
        continue;
      const auto cache_id = item_cache_id(item);
      if (const auto cached = translation_cache_.find(cache_id);
          cached != translation_cache_.end()) {
        auto output = nlohmann::json::parse(translations);
        output.push_back(
            {{"text", item.at("text")}, {"translation", cached->second}});
        translations = output.dump();
        translated_texts.insert(item.at("text").get<std::string>());
        continue;
      }
      const auto negative = translation_negative_cache_.find(cache_id);
      if (negative != translation_negative_cache_.end()) {
        if (negative->second > std::chrono::steady_clock::now())
          continue;
        translation_negative_cache_.erase(negative);
      }
      pending.push_back(item);
    }

    const auto provider_deadline =
        std::chrono::steady_clock::now() + kTranslationBatchBudget;
    std::unordered_set<std::string> attempted_cache_ids;

    if (niutrans.is_object() && niutrans.value("enabled", false)) {
      for (const auto &item : pending) {
        if (cancelled() ||
            std::chrono::steady_clock::now() >= provider_deadline)
          break;
        attempted_cache_ids.insert(item_cache_id(item));
        append_niutrans_item(niutrans, item, translations, cancelled);
      }
    } else if (custom.is_object() && custom.value("enabled", false)) {
      for (const auto &item : pending) {
        if (cancelled() ||
            std::chrono::steady_clock::now() >= provider_deadline)
          break;
        attempted_cache_ids.insert(item_cache_id(item));
        if (auto value = custom_translation(custom, item, cancelled)) {
          auto output = nlohmann::json::parse(translations);
          output.push_back(
              {{"text", item.at("text")}, {"translation", *value}});
          translations = output.dump();
        }
      }
    } else {
      const auto tencent = query.value("tencent_tmt", nlohmann::json(nullptr));
      if (!tencent.is_object() || !tencent.value("enabled", false))
        return std::nullopt;
      std::unordered_map<std::string, std::vector<nlohmann::json>> groups;
      groups.reserve(pending.size());
      for (const auto &item : pending)
        groups[item.at("source_language").get<std::string>() + "\n" +
               item.at("target_language").get<std::string>()]
            .push_back(item);
      for (const auto &[key, items] : groups) {
        (void)key;
        if (cancelled() ||
            std::chrono::steady_clock::now() >= provider_deadline)
          break;
        for (const auto &item : items)
          attempted_cache_ids.insert(item_cache_id(item));
        append_tencent_group(tencent, items, translations, cancelled);
      }
    }
    if (cancelled())
      return std::nullopt;
    const auto output = nlohmann::json::parse(translations);
    for (const auto &item : pending) {
      const auto cache_id = item_cache_id(item);
      if (attempted_cache_ids.find(cache_id) == attempted_cache_ids.end())
        continue;
      const auto found =
          std::find_if(output.begin(), output.end(), [&](const auto &entry) {
            return entry.is_object() && entry.value("text", std::string{}) ==
                                            item.at("text").get<std::string>();
          });
      if (found != output.end() &&
          found->value("translation", std::string{}) != std::string{}) {
        const auto value = found->at("translation").get<std::string>();
        if (translation_cache_.size() >= kMaximumTranslationCacheEntries)
          translation_cache_.clear();
        translation_cache_[cache_id] = value;
        translation_negative_cache_.erase(cache_id);
        persist_english_glosses(
            nlohmann::json{
                {"target_language", target_language},
                {"user_data", query.value("user_data", std::string{})}},
            nlohmann::json::array(
                {{{"text", item.at("text")}, {"translation", value}}})
                .dump());
      } else {
        if (translation_negative_cache_.size() >=
            kMaximumTranslationCacheEntries)
          translation_negative_cache_.clear();
        translation_negative_cache_[cache_id] =
            std::chrono::steady_clock::now() + kNegativeTranslationTtl;
      }
    }
    if (output.empty())
      return std::nullopt;
    return TranslationWorker::Result{lease, generation, output.dump()};
  } catch (...) {
    return std::nullopt;
  }
}

void TranslationWorker::run() noexcept {
  // 日文读音用的 IFELanguage 和本线程的 COM 由这个栈上的读音器持有，run 返回时先清空指针，再由读音器的析构关闭它们，都在线程函数里，不在线程退出回调里（见 active_japanese_reader）。
  JapaneseReader japanese_reader;
  active_japanese_reader = &japanese_reader;
  struct ClearActiveJapaneseReader {
    ~ClearActiveJapaneseReader() { active_japanese_reader = nullptr; }
  } clear_active_japanese_reader;
  for (;;) {
    Request request;
    {
      std::unique_lock lock(mutex_);
      wake_.wait(lock, [&] {
        return stopping_.load(std::memory_order_acquire) ||
               pending_.has_value() ||
               clear_cache_requested_.load(std::memory_order_acquire);
      });
      if (stopping_.load(std::memory_order_acquire))
        return;
      if (clear_cache_requested_.exchange(false, std::memory_order_acq_rel)) {
        translation_cache_.clear();
        translation_negative_cache_.clear();
      }
      if (!pending_)
        continue;
      request = std::move(*pending_);
      pending_.reset();
      for (;;) {
        const auto deadline = std::chrono::steady_clock::now() + kDebounce;
        if (!wake_.wait_until(lock, deadline, [&] {
              return stopping_.load(std::memory_order_acquire) ||
                     pending_.has_value();
            }))
          break;
        if (stopping_.load(std::memory_order_acquire))
          return;
        request = std::move(*pending_);
        pending_.reset();
      }
    }
    try {
      const auto serial = request.serial;
      const auto cancel = [this, serial] { return cancelled(serial); };
      auto result = translate_(request.lease, request.query, cancel);
      if (result && !cancel())
        completed_(std::move(*result));
    } catch (...) {
      // Translation is optional and must never stop input.
    }
  }
}
} // namespace msime::windows

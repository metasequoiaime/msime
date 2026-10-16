#include "AiCandidateWorker.h"
#include "AiEndpointPolicy.h"
#include "CandidateHttpPolicy.h"

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
#include <functional>
#include <memory>
#include <mutex>
#include <stdexcept>

namespace msime::windows {
namespace {
// The reference debounces AI requests harder than cloud ones: each is a paid
// model call, and a user mid-word would otherwise spend several.
constexpr auto kDebounce = std::chrono::milliseconds(650);
constexpr size_t kMaximumQueryBytes = 16384;
constexpr size_t kMaximumResponseBytes = 1024 * 1024;
constexpr uint8_t kDefaultLimit = 3;
constexpr size_t kMaximumCacheEntries = 4096;

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

// The session has already built the authenticated HTTP descriptor. Keep the
// response limit from the query that will receive the parsed candidates.
std::optional<uint8_t> ai_limit(const std::string &query) {
  try {
    const auto parsed = nlohmann::json::parse(query, nullptr, false);
    if (parsed.is_discarded() || !parsed.is_object())
      return std::nullopt;
    if (!parsed.value("ai_eligible", false))
      return std::nullopt;
    const auto config = parsed.value("ai_assistant", nlohmann::json::object());
    if (!config.is_object() || !config.value("enabled", false))
      return std::nullopt;
    const auto candidate_limit = config.value("candidate_limit", 3);
    return static_cast<uint8_t>(candidate_limit >= 1 && candidate_limit <= 10
                                    ? candidate_limit
                                    : kDefaultLimit);
  } catch (...) {
    return std::nullopt;
  }
}

// 这次查询有没有向 AI 发问：查询符合条件且 AI 开着。只有发问了的查询，空回答才要交给 owner，由它告诉 Host API 这次没有回答（#6888）。
bool ai_requested(const std::string &query) {
  try {
    const auto parsed = nlohmann::json::parse(query, nullptr, false);
    if (parsed.is_discarded() || !parsed.is_object() ||
        !parsed.value("ai_eligible", false))
      return false;
    const auto config = parsed.value("ai_assistant", nlohmann::json::object());
    return config.is_object() && config.value("enabled", false);
  } catch (...) {
    return false;
  }
}

std::optional<std::string> ai_cache_key(const std::string &query,
                                        const std::string &descriptor) {
  try {
    const auto parsed = nlohmann::json::parse(query, nullptr, false);
    if (parsed.is_discarded() || !parsed.is_object())
      return std::nullopt;
    const auto config = parsed.value("ai_assistant", nlohmann::json::object());
    const auto segments =
        parsed.value("pinyin_segments", nlohmann::json::array());
    if (!config.is_object() || !config.value("enabled", false) ||
        !segments.is_array() || segments.empty())
      return std::nullopt;
    // The provider is asked for this many candidates. Keep it in the cache
    // identity: reusing a three-candidate answer after the user raises the
    // preference to ten would silently hide the newly requested rows.
    const auto configured_limit = config.value("candidate_limit", 3);
    const auto candidate_limit = configured_limit >= 1 && configured_limit <= 10
                                     ? configured_limit
                                     : 3;
    // Context and prompt settings change the model's answer. Keep a digest
    // of the context rather than retaining the private text in the cache key.
    // Session and generation stay out so an unchanged request can be reused
    // after a candidate refresh.
    const auto context = parsed.value("ai_context", std::string{});
    // The descriptor contains the endpoint and credential headers resolved by
    // the host session. Hash it into the identity without retaining secrets in
    // the cache key, so a credential or endpoint change cannot reuse an older
    // provider answer for the same pinyin.
    return nlohmann::json{{"descriptor_hash", std::hash<std::string>{}(descriptor)},
                          {"provider", config.value("provider", std::string{})},
                          {"endpoint", config.value("endpoint", std::string{})},
                          {"model", config.value("model", std::string{})},
                          {"candidate_limit", candidate_limit},
                          {"prompt_id", config.value("prompt_id", std::string{})},
                          {"prompt_custom_1",
                           config.value("prompt_custom_1", std::string{})},
                          {"prompt_custom_2",
                           config.value("prompt_custom_2", std::string{})},
                          {"prompt_custom_3",
                           config.value("prompt_custom_3", std::string{})},
                          {"ai_context_hash", std::hash<std::string>{}(context)},
                          {"pinyin_segments", segments}}
        .dump();
  } catch (...) {
    return std::nullopt;
  }
}

std::optional<std::string> https_post(const nlohmann::json &descriptor,
                                      const std::function<bool()> &cancelled) {
  try {
    if (!descriptor.is_object() || !descriptor.at("url").is_string())
      return std::nullopt;
    const auto url = descriptor.at("url").get<std::string>();
    // 本机和局域网的接口（如 LM Studio）可以用 http，其余只走 https；规则见 AiEndpointPolicy.h。
    const auto protocol = std::string(ai_endpoint_protocol(url));
    if (protocol.empty())
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
    curl_easy_setopt(curl.get(), CURLOPT_PROTOCOLS_STR, protocol.c_str());
    if (ai_endpoint_connects_directly(url))
      curl_easy_setopt(curl.get(), CURLOPT_NOPROXY, "*");
    // 请求描述带着凭据，跟随重定向就等于把它交给响应指定的任意主机；不跟随也保证局域网的 http 请求不会被转到公网主机。
    curl_easy_setopt(curl.get(), CURLOPT_FOLLOWLOCATION, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT_MS, 2500L);
    // The reference budgets 8 s for a model call; a shorter cap would abandon
    // answers that were still on their way.
    curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT_MS, 8000L);
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
    if (descriptor.contains("body_utf8") &&
        descriptor.at("body_utf8").is_string())
      body = descriptor.at("body_utf8").get<std::string>();
    else if (descriptor.contains("body"))
      body = descriptor.at("body").dump();
    else
      return std::nullopt;
    if (body.size() > kMaximumResponseBytes)
      return std::nullopt;
    curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDS, body.data());
    curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDSIZE_LARGE,
                     static_cast<curl_off_t>(body.size()));
    const auto result = curl_easy_perform(curl.get());
    long status = 0;
    curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &status);
    if (result != CURLE_OK || status < 200 || status >= 300 ||
        (cancelled && cancelled()))
      return std::nullopt;
    return std::move(response.body);
  } catch (...) {
    return std::nullopt;
  }
}
} // namespace

AiCandidateWorker::AiCandidateWorker(Completed completed)
    : AiCandidateWorker(std::move(completed), Fetcher{}) {}

AiCandidateWorker::AiCandidateWorker(Completed completed, Fetcher fetcher)
    : completed_(std::move(completed)), fetch_(std::move(fetcher)) {
  if (!completed_)
    throw std::invalid_argument("Missing AI candidate completion");
  if (!fetch_)
    fetch_ = [](const std::string &query, const std::string &descriptor,
                const std::function<bool()> &cancelled) {
      return fetch(query, descriptor, cancelled);
    };
  worker_ = std::thread([this] { run(); });
}

AiCandidateWorker::~AiCandidateWorker() { stop(); }

bool AiCandidateWorker::submit(const FocusLease &lease, std::string query,
                               std::string descriptor) {
  if (!lease.epoch || !lease.token || query.empty() ||
      query.size() > kMaximumQueryBytes || descriptor.size() > 131072)
    return false;
  std::lock_guard lock(mutex_);
  if (stopping_)
    return false;
  const uint64_t serial = ++next_serial_;
  pending_ = Request{lease, std::move(query), std::move(descriptor), serial};
  latest_serial_.store(serial, std::memory_order_release);
  wake_.notify_one();
  return true;
}

void AiCandidateWorker::request_stop() {
  stopping_.store(true, std::memory_order_release);
  {
    std::lock_guard lock(mutex_);
    pending_.reset();
  }
  wake_.notify_all();
}

void AiCandidateWorker::stop() {
  std::lock_guard join(join_mutex_);
  request_stop();
  if (worker_.joinable()) {
    if (worker_.get_id() == std::this_thread::get_id())
      throw std::logic_error("Cannot join AI candidate worker from itself");
    worker_.join();
  }
}

bool AiCandidateWorker::cancelled(uint64_t serial) const noexcept {
  return stopping_.load(std::memory_order_acquire) ||
         latest_serial_.load(std::memory_order_acquire) != serial;
}

std::vector<std::string>
AiCandidateWorker::fetch(const std::string &query,
                         const std::string &descriptor,
                         const std::function<bool()> &is_cancelled) {
  const auto limit = ai_limit(query);
  if (!limit || descriptor.empty() || (is_cancelled && is_cancelled()))
    return {};
  const auto parsed_descriptor = nlohmann::json::parse(descriptor, nullptr, false);
  if (parsed_descriptor.is_discarded())
    return {};
  const auto body = https_post(parsed_descriptor, is_cancelled);
  if (!body || body->empty() || (is_cancelled && is_cancelled()))
    return {};
  auto raw = msime::host_api::own_string(
      msime_client_parse_ai_response(
          reinterpret_cast<const uint8_t *>(body->data()), body->size(), *limit));
  if (!raw)
    return {};
  try {
    const auto parsed = nlohmann::json::parse(raw.get(), nullptr, false);
    if (parsed.is_discarded() || !parsed.value("ok", false) ||
        !parsed.at("value").is_array())
      return {};
    std::vector<std::string> candidates;
    candidates.reserve(*limit);
    for (const auto &entry : parsed.at("value")) {
      if (!entry.is_string())
        continue;
      auto text = entry.get<std::string>();
      if (text.empty() || text.size() > 512)
        continue;
      candidates.push_back(std::move(text));
      if (candidates.size() >= *limit)
        break;
    }
    return candidates;
  } catch (...) {
    return {};
  }
}

void AiCandidateWorker::run() {
  while (true) {
    Request request;
    {
      std::unique_lock lock(mutex_);
      wake_.wait(lock, [this] { return stopping_ || pending_.has_value(); });
      if (stopping_)
        return;
      request = *pending_;
      pending_.reset();
    }
    // Debounce: a newer keystroke during the wait replaces this request rather
    // than spending a model call on a prefix the user has already left.
    {
      std::unique_lock lock(mutex_);
      wake_.wait_for(lock, kDebounce, [this, &request] {
        return stopping_ ||
               latest_serial_.load(std::memory_order_acquire) != request.serial;
      });
      if (stopping_ ||
          latest_serial_.load(std::memory_order_acquire) != request.serial)
        continue;
    }
    const auto is_cancelled = [this, serial = request.serial] {
      return cancelled(serial);
    };
    std::vector<std::string> candidates;
    bool cached = false;
    const auto cache_key = ai_cache_key(request.query, request.descriptor);
    {
      std::lock_guard lock(mutex_);
      if (cache_key) {
        const auto found = candidate_cache_.find(*cache_key);
        if (found != candidate_cache_.end()) {
          candidates = found->second;
          cached = true;
        }
      }
    }
    if (!cached) {
      try {
        if (!request.descriptor.empty())
          candidates = fetch_(request.query, request.descriptor, is_cancelled);
      } catch (...) {
        failed_.store(true, std::memory_order_release);
        candidates.clear();
      }
      if (is_cancelled())
        continue;
      // Only successful responses are cacheable.  Keeping an empty result
      // would permanently suppress retries for this prefix: a transient
      // provider failure (or a provider that was temporarily unavailable)
      // would then be mistaken for a valid answer until the query changed.
      if (!candidates.empty()) {
        std::lock_guard lock(mutex_);
        if (cache_key) {
          if (candidate_cache_.size() >= kMaximumCacheEntries)
            candidate_cache_.clear();
          candidate_cache_[*cache_key] = candidates;
        }
      }
    }
    if (is_cancelled())
      continue;
    // 空回答（失败、被拒、模型没给词）也交给 owner：它告诉 Host API 这次没有回答（#6888）。没向 AI 发问的查询什么都不交。
    if (candidates.empty() && !ai_requested(request.query))
      continue;
    try {
      completed_(Result{request.lease, request.query, std::move(candidates)});
    } catch (...) {
      failed_.store(true, std::memory_order_release);
    }
  }
}
} // namespace msime::windows

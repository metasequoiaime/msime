#include "AiCandidateWorker.h"
#include "CandidateHttpPolicy.h"

#include "msime_client.h"

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
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

// Build the AI request from the online query the Engine already produced.
// Returns nothing when the query is not eligible, which is the ordinary case:
// AI assistance off, no credentials, or a prefix too short to ask about.
std::optional<nlohmann::json> ai_descriptor(const std::string &query,
                                            uint8_t &limit) {
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
    limit = static_cast<uint8_t>(candidate_limit >= 1 && candidate_limit <= 10
                                     ? candidate_limit
                                     : kDefaultLimit);
    nlohmann::json request{
        {"config", config},
        {"input",
         {{"segmented_pinyin",
           parsed.value("segmented_pinyin", nlohmann::json::array())},
          {"context", parsed.value("ai_context", std::string{})},
          {"candidate_limit", limit}}}};
    const auto serialized = request.dump();
    if (serialized.size() > 65536)
      return std::nullopt;
    std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
        msime_client_ai_http_request(
            reinterpret_cast<const uint8_t *>(serialized.data()),
            serialized.size()),
        msime_client_string_free);
    if (!raw)
      return std::nullopt;
    const auto response = nlohmann::json::parse(raw.get(), nullptr, false);
    if (response.is_discarded() || !response.value("ok", false) ||
        !response.at("value").is_object())
      return std::nullopt;
    return response.at("value");
  } catch (...) {
    return std::nullopt;
  }
}

std::optional<std::string> ai_cache_key(const std::string &query) {
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
    // Match the source worker's cache identity. Deliberately omit token,
    // prompt, context, session, and generation so no secrets are retained and
    // an unchanged prefix can be reused after a candidate refresh.
    return nlohmann::json{{"provider", config.value("provider", std::string{})},
                          {"endpoint", config.value("endpoint", std::string{})},
                          {"model", config.value("model", std::string{})},
                          {"candidate_limit", candidate_limit},
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
    if (!valid_candidate_url(url))
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
      if (!it.value().is_string() || it.key().size() > 128 ||
          it.value().get<std::string>().size() > 8192)
        return std::nullopt;
      const auto line = it.key() + ": " + it.value().get<std::string>();
      raw_headers = curl_slist_append(raw_headers, line.c_str());
      if (!raw_headers)
        return std::nullopt;
      request_headers.reset(raw_headers);
    }
    HttpResponse response{{}, cancelled};
    curl_easy_setopt(curl.get(), CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl.get(), CURLOPT_PROTOCOLS_STR, "https");
    // The descriptor carries credentials; a redirect would hand them to
    // whatever host the response names.
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
    fetch_ = [](const std::string &query,
                const std::function<bool()> &cancelled) {
      return fetch(query, cancelled);
    };
  worker_ = std::thread([this] { run(); });
}

AiCandidateWorker::~AiCandidateWorker() { stop(); }

bool AiCandidateWorker::submit(const FocusLease &lease, std::string query) {
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
                         const std::function<bool()> &is_cancelled) {
  uint8_t limit = kDefaultLimit;
  const auto descriptor = ai_descriptor(query, limit);
  if (!descriptor || (is_cancelled && is_cancelled()))
    return {};
  const auto body = https_post(*descriptor, is_cancelled);
  if (!body || body->empty() || (is_cancelled && is_cancelled()))
    return {};
  std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
      msime_client_parse_ai_response(
          reinterpret_cast<const uint8_t *>(body->data()), body->size(), limit),
      msime_client_string_free);
  if (!raw)
    return {};
  try {
    const auto parsed = nlohmann::json::parse(raw.get(), nullptr, false);
    if (parsed.is_discarded() || !parsed.value("ok", false) ||
        !parsed.at("value").is_array())
      return {};
    std::vector<std::string> candidates;
    for (const auto &entry : parsed.at("value")) {
      if (!entry.is_string())
        continue;
      auto text = entry.get<std::string>();
      if (text.empty() || text.size() > 512)
        continue;
      candidates.push_back(std::move(text));
      if (candidates.size() >= limit)
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
    const auto cache_key = ai_cache_key(request.query);
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
        candidates = fetch_(request.query, is_cancelled);
      } catch (...) {
        failed_.store(true, std::memory_order_release);
        continue;
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
    if (candidates.empty() || is_cancelled())
      continue;
    try {
      completed_(Result{request.lease, request.query, std::move(candidates)});
    } catch (...) {
      failed_.store(true, std::memory_order_release);
    }
  }
}
} // namespace msime::windows

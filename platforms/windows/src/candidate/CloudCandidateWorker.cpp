#include "CloudCandidateWorker.h"
#include "CandidateHttpPolicy.h"

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
#include <memory>
#include <stdexcept>

namespace msime::windows
{
namespace
{
constexpr auto kDebounce = std::chrono::milliseconds(500);
constexpr size_t kMaximumQueryBytes = MSIME_ONLINE_QUERY_MAX_BYTES;
constexpr size_t kMaximumResponseBytes = 256 * 1024;

struct Response
{
    std::string body;
    std::function<bool()> cancelled;
};

size_t write_response(char *data, size_t size, size_t count, void *context)
{
    auto &response = *static_cast<Response *>(context);
    if (response.cancelled && response.cancelled())
        return 0;
    if (size > kMaximumResponseBytes || response.body.size() > kMaximumResponseBytes ||
        (size != 0 && count > (kMaximumResponseBytes - response.body.size()) / size))
        return 0;
    response.body.append(data, size * count);
    return size * count;
}

int transfer_progress(void *context, curl_off_t, curl_off_t, curl_off_t, curl_off_t)
{
    const auto &cancelled = *static_cast<const std::function<bool()> *>(context);
    return cancelled && cancelled() ? 1 : 0;
}

std::string request_url(const std::string &query)
{
    auto raw = msime::host_api::own_string(
        msime_client_cloud_request_url(reinterpret_cast<const uint8_t *>(query.data()), query.size()));
    if (!raw)
        return {};
    try
    {
        const auto document = nlohmann::json::parse(raw.get());
        if (!document.at("ok").get<bool>() || !document.at("value").is_string())
            return {};
        const auto url = document.at("value").get<std::string>();
        if (!valid_candidate_url(url))
            return {};
        return url;
    }
    catch (...)
    {
        return {};
    }
}

} // namespace

CloudCandidateWorker::CloudCandidateWorker(Completed completed)
    : CloudCandidateWorker(std::move(completed), Fetcher{})
{
}

CloudCandidateWorker::CloudCandidateWorker(Completed completed, Fetcher fetcher)
    : completed_(std::move(completed)), fetch_(std::move(fetcher))
{
    if (!completed_)
        throw std::invalid_argument("Missing cloud candidate completion");
    if (!fetch_)
        fetch_ = [](const std::string &query, const std::function<bool()> &cancelled) {
            return fetch(query, cancelled);
        };
    worker_ = std::thread([this] { run(); });
}

CloudCandidateWorker::~CloudCandidateWorker()
{
    stop();
}

bool CloudCandidateWorker::submit(const FocusLease &lease, std::string query)
{
    if (!lease.epoch || !lease.token || query.empty() || query.size() > kMaximumQueryBytes)
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

void CloudCandidateWorker::request_stop()
{
    stopping_.store(true, std::memory_order_release);
    {
        std::lock_guard lock(mutex_);
        pending_.reset();
    }
    wake_.notify_all();
}

void CloudCandidateWorker::stop()
{
    std::lock_guard join(join_mutex_);
    request_stop();
    if (worker_.joinable())
    {
        if (worker_.get_id() == std::this_thread::get_id())
            throw std::logic_error("Cannot join cloud candidate worker from itself");
        worker_.join();
    }
}

bool CloudCandidateWorker::cancelled(uint64_t serial) const noexcept
{
    return stopping_.load(std::memory_order_acquire) ||
           latest_serial_.load(std::memory_order_acquire) != serial;
}

std::string CloudCandidateWorker::fetch(const std::string &query,
                                        const std::function<bool()> &cancelled)
{
    nlohmann::json descriptor;
    try {
        const auto document = nlohmann::json::parse(query);
        if (document.contains("cloud_request") && document.at("cloud_request").is_object())
            descriptor = document.at("cloud_request");
        else
            descriptor = {{"url", request_url(query)}, {"method", "GET"}, {"headers", nlohmann::json::object()},
                          {"body_utf8", ""}, {"timeout_ms", MSIME_CLOUD_REQUEST_TIMEOUT_MS},
                          {"connect_timeout_ms", MSIME_CLOUD_CONNECT_TIMEOUT_MS}};
        if (descriptor.contains("cached_response"))
            return descriptor.at("cached_response").get<std::string>();
    } catch (...) { return {}; }
    const auto url = descriptor.value("url", std::string{});
    const auto method = descriptor.value("method", std::string{});
    const auto timeout = descriptor.value("timeout_ms", 0L);
    const auto connect_timeout = descriptor.value("connect_timeout_ms", 0L);
    if (!valid_candidate_url(url, true) || (method != "GET" && method != "POST") ||
        timeout < 100 || timeout > 10000 || connect_timeout <= 0 || connect_timeout > timeout ||
        (cancelled && cancelled())) return {};

    static std::once_flag curl_once;
    std::call_once(curl_once, [] { curl_global_init(CURL_GLOBAL_DEFAULT); });
    std::unique_ptr<CURL, decltype(&curl_easy_cleanup)> curl(curl_easy_init(), curl_easy_cleanup);
    if (!curl)
        return {};

    Response response{{}, cancelled};
    curl_easy_setopt(curl.get(), CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl.get(), CURLOPT_PROTOCOLS_STR, url.rfind("https://", 0) == 0 ? "https" : "http");
    if (url.rfind("http://", 0) == 0) curl_easy_setopt(curl.get(), CURLOPT_PROXY, "");
    curl_easy_setopt(curl.get(), CURLOPT_FOLLOWLOCATION, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT_MS,
                     connect_timeout);
    curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT_MS, timeout);
    curl_easy_setopt(curl.get(), CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(curl.get(), CURLOPT_USERAGENT, "MSIME-Client/1.0");
    curl_easy_setopt(curl.get(), CURLOPT_WRITEFUNCTION, write_response);
    curl_easy_setopt(curl.get(), CURLOPT_WRITEDATA, &response);
    curl_easy_setopt(curl.get(), CURLOPT_NOPROGRESS, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFOFUNCTION, transfer_progress);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFODATA, &cancelled);
    const auto headers = descriptor.value("headers", nlohmann::json::object());
    if (!headers.is_object() || headers.size() > 18) return {};
    curl_slist *raw_headers = nullptr;
    for (auto it = headers.begin(); it != headers.end(); ++it) {
        if (!it.value().is_string() || !valid_candidate_header_name(it.key()) ||
            !valid_candidate_header_value(it.value().get<std::string>())) {
            curl_slist_free_all(raw_headers); return {};
        }
        const auto line = it.key() + ": " + it.value().get<std::string>();
        auto *next = curl_slist_append(raw_headers, line.c_str());
        if (!next) { curl_slist_free_all(raw_headers); return {}; }
        raw_headers = next;
    }
    std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)> request_headers(raw_headers, curl_slist_free_all);
    curl_easy_setopt(curl.get(), CURLOPT_HTTPHEADER, request_headers.get());
    const auto body = descriptor.value("body_utf8", std::string{});
    if (body.size() > 8192) return {};
    if (method == "POST") {
        curl_easy_setopt(curl.get(), CURLOPT_POST, 1L);
        curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDS, body.data());
        curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDSIZE_LARGE, static_cast<curl_off_t>(body.size()));
    }
    const auto result = curl_easy_perform(curl.get());
    long status = 0;
    curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &status);
    if (result != CURLE_OK || (status < 200 || status >= 300) || (cancelled && cancelled()))
        return {};
    return std::move(response.body);
}

void CloudCandidateWorker::run() noexcept
{
    for (;;)
    {
        Request request;
        {
            std::unique_lock lock(mutex_);
            wake_.wait(lock, [&] { return stopping_.load(std::memory_order_acquire) || pending_.has_value(); });
            if (stopping_.load(std::memory_order_acquire))
                return;
            request = std::move(*pending_);
            pending_.reset();
            for (;;)
            {
                const auto deadline = std::chrono::steady_clock::now() + kDebounce;
                if (!wake_.wait_until(lock, deadline, [&] {
                        return stopping_.load(std::memory_order_acquire) || pending_.has_value();
                    }))
                    break;
                if (stopping_.load(std::memory_order_acquire))
                    return;
                request = std::move(*pending_);
                pending_.reset();
            }
        }

        try
        {
            const auto cancelled = [this, serial = request.serial] { return this->cancelled(serial); };
            auto body = fetch_(request.query, cancelled);
            if (body.empty() || cancelled())
                continue;
            completed_(Result{std::move(request.lease), std::move(request.query), std::move(body)});
        }
        catch (...)
        {
            // Network/provider failures are optional and must never stop input.
        }
    }
}
} // namespace msime::windows

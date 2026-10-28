// 真实 macOS 适配、偏好、队列和异常处理器；仅替换网络投递，不访问产品端点。
#include <cstdlib>
#define MSIME_USAGE_REPORTING_TEST_SUPPORT std::getenv("MSIME_TEST_REPORTING_SUPPORT")
#import "../../src/core/UsageReporting.mm"
#include "../../../common/CrashRecorder.h"
#include "../../../common/HostApiString.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <condition_variable>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <thread>
#include <sys/mman.h>

namespace {
using Json = nlohmann::json;
std::filesystem::path deniedPreferences;
std::mutex sendingMutex;
std::condition_variable sendingChanged;
bool blockSending = false;
bool sendingEntered = false;
bool sendingReleased = false;
Json sent;

Json result(char *raw) {
    auto owned = msime::host_api::own_string(raw);
    if (!owned) throw std::runtime_error("synthetic empty host response");
    const auto value = Json::parse(owned.get());
    if (!value.value("ok", false)) throw std::runtime_error("synthetic rejected host response");
    return value.at("value");
}

void save(const std::filesystem::path &directory, bool enabled) {
    const auto path = directory.string();
    auto snapshot = result(msime_client_load_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size()));
    snapshot["preferences"]["usage_reporting"] = enabled;
    const auto document = snapshot.dump();
    result(msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size(), snapshot.at("revision").get<uint64_t>(),
        reinterpret_cast<const uint8_t *>(document.data()), document.size()));
}

Json captureStatus(const std::filesystem::path &directory) {
    std::ifstream file(directory / "telemetry-session.json");
    return Json{{"armed", msime::crash::is_armed()}, {"directory", directory.string()},
                {"marker", file ? Json::parse(file) : Json::object()}};
}
}

extern "C" char *msime_test_telemetry_flush(const uint8_t *body, size_t size) {
    const auto request = Json::parse(body, body + size);
    {
        std::unique_lock lock(sendingMutex);
        if (blockSending) {
            sent = request;
            sendingEntered = true;
            sendingChanged.notify_all();
            sendingChanged.wait(lock, [] { return sendingReleased; });
        } else {
            const auto consent = Json{{"directory", request.at("preferences_directory")}}.dump();
            return msime_client_telemetry_consent(reinterpret_cast<const uint8_t *>(consent.data()), consent.size());
        }
    }
    const auto consent = Json{{"directory", deniedPreferences.string()}}.dump();
    return msime_client_telemetry_consent(reinterpret_cast<const uint8_t *>(consent.data()), consent.size());
}

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    @autoreleasepool {
        const std::string mode = argv[1];
        const std::filesystem::path requestedDirectory = argv[2];
        const std::filesystem::path preferences = argv[3];
        if (mode == "collect") {
            msime::telemetry::begin({"macos", "9.9.9", requestedDirectory, std::nullopt,
                                    preferences, "collector", true});
            msime::telemetry::end();
            return 0;
        }
        if (mode != "observe" && mode != "observe-standalone") return 2;
        const auto support = requestedDirectory.parent_path().string();
        setenv("MSIME_TEST_REPORTING_SUPPORT", support.c_str(), 1);
        deniedPreferences = preferences.parent_path() / "denied-preferences";
        save(deniedPreferences, false);
        NSString *preferencesDirectory = [NSString stringWithUTF8String:preferences.c_str()];
        if (mode == "observe-standalone") MSIMESettingsCrashReportingStart(preferencesDirectory);
        else MSIMEUsageReportingStart(preferencesDirectory);
        dispatch_sync(gQueue, ^{});
        const std::filesystem::path directory = MSIMEUsageReportingDirectory().UTF8String;
        std::thread network;
        std::cout << std::unitbuf << captureStatus(directory).dump() << '\n';
        std::string command;
        while (std::getline(std::cin, command)) {
            if (command == "fault") {
                auto *page = mmap(nullptr, static_cast<size_t>(getpagesize()), PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
                if (page == MAP_FAILED) return 3;
                *static_cast<volatile char *>(page) = 7;
                return 4;
            }
            if (command == "exception") @throw [NSException exceptionWithName:@"SyntheticException"
                                                                      reason:@"synthetic-private-input" userInfo:nil];
            if (command == "disable" || command == "enable") {
                save(preferences, command == "enable");
                std::cout << "saved\n";
            } else if (command == "status") {
                std::cout << captureStatus(directory).dump() << '\n';
            } else if (command == "flush-start") {
                {
                    std::lock_guard guard(sendingMutex);
                    blockSending = true;
                }
                network = std::thread([] { @autoreleasepool { FlushNow(); } });
                std::unique_lock lock(sendingMutex);
                if (!sendingChanged.wait_for(lock, std::chrono::seconds(10), [] { return sendingEntered; }))
                    std::exit(7);
                std::cout << sent.dump() << '\n';
            } else if (command == "flush-release") {
                {
                    std::lock_guard guard(sendingMutex);
                    sendingReleased = true;
                }
                sendingChanged.notify_all();
                network.join();
                std::cout << captureStatus(directory).dump() << '\n';
            } else if (command == "observer-stop") {
                MSIMEUsageReportingStop();
                std::cout << captureStatus(directory).dump() << '\n';
            } else if (command == "stop") {
                break;
            } else {
                return 2;
            }
        }
        if (network.joinable()) return 8;
        MSIMEUsageReportingStop();
        std::cout << captureStatus(directory).dump() << '\n';
    }
    return 0;
}

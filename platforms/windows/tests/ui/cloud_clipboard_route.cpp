#include "../../settings/CloudClipboardRoute.h"

#include <cassert>
#include <string>

namespace {
struct Response {
  bool ok;
  std::wstring value;
};
}

int main() {
  using namespace msime::settings;
  for (const auto source : {L"account", L"relay", L"unknown", L""}) {
    for (const bool source_ok : {false, true}) {
      for (const bool backend_ok : {false, true}) {
        int relay_calls = 0, account_calls = 0, invalid_calls = 0, decode_calls = 0;
        const auto response = cloud_clipboard_request(
            [=] { return Response{source_ok, source}; },
            [&](Response const &value) { ++decode_calls; return value.value; },
            [&] { ++relay_calls; return Response{backend_ok, L"relay_response"}; },
            [&] { ++account_calls; return Response{backend_ok, L"account_response"}; },
            [&] { ++invalid_calls; return Response{false, L"invalid_source"}; });
        if (!source_ok) {
          assert(!response.ok && response.value == source);
          assert(decode_calls == 0 && relay_calls == 0 && account_calls == 0 && invalid_calls == 0);
        } else if (std::wstring_view(source) == L"relay") {
          assert(response.ok == backend_ok && response.value == L"relay_response");
          assert(relay_calls == 1 && account_calls == 0 && invalid_calls == 0);
        } else if (std::wstring_view(source) == L"account") {
          assert(response.ok == backend_ok && response.value == L"account_response");
          assert(relay_calls == 0 && account_calls == 1 && invalid_calls == 0);
        } else {
          assert(!response.ok && response.value == L"invalid_source");
          assert(relay_calls == 0 && account_calls == 0 && invalid_calls == 1);
        }
      }
    }
  }
  assert(cloud_clipboard_failure(L"relay_unauthorized") == CloudClipboardState::relay_unauthorized);
  assert(cloud_clipboard_failure(L"unauthorized") == CloudClipboardState::signed_out);
  assert(cloud_clipboard_failure(L"account_unauthorized") == CloudClipboardState::signed_out);
  assert(cloud_clipboard_failure(L"clipboard_device_pending") == CloudClipboardState::pending);
  assert(cloud_clipboard_failure(L"clipboard_sync_disabled") == CloudClipboardState::disabled);
  assert(cloud_clipboard_failure(L"relay_unavailable") == CloudClipboardState::unavailable);
}

#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../core/BoundedCliInput.h"
#include <array>
#include <iostream>
#include <nlohmann/json.hpp>
#include <string>

int main(int argc, char **argv) {
  if (argc != 2 || argv[1][0] != '/') return 2;
  std::array<char, 12001> input;
  const auto input_size = msime::linux_host::read_bounded_cli_input(std::cin, input);
  if (!input_size) return 2;
  const size_t size = *input_size;
  try {
    const auto request = nlohmann::json{
        {"directory", argv[1]}, {"text", std::string(input.data(), size)}}.dump();
    auto result = msime::host_api::own_string(
        msime_client_capture_clipboard_history(
            reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    if (!result) return 1;
    const auto document = nlohmann::json::parse(result.get());
    if (!document.value("ok", false)) return 1;
    return document.at("value").value("captured", false) ? 0 : 3;
  } catch (...) { return 1; }
}

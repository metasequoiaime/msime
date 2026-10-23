#include "VoiceProviders.h"
#include <exception>
#include <string>

int main(int argc, char **argv) {
  if (argc != 5) return 2;
  const bool expected = std::string(argv[4]) == "success";
  try {
    const auto cancelled = std::make_shared<std::atomic_bool>(false);
    const auto result = std::string(argv[1]) == "asr"
        ? msime::voice::recognize_cloud_asr({0.0f}, argv[2], argv[3],
              "fixture", "synthetic-token", "zh-CN", cancelled)
        : msime::voice::polish_cloud_text("synthetic transcript", argv[2],
              argv[3], "fixture", "synthetic-token", "fixture prompt", "disabled", cancelled);
    return expected && result == "synthetic result" ? 0 : 1;
  } catch (const std::exception &) {
    // Do not print response bodies or transport diagnostics.
    return expected ? 1 : 0;
  }
}

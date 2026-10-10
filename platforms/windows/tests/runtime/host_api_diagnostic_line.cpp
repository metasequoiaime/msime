#include "HostApiDiagnosticLine.h"

#include <cstdio>
#include <string>

namespace {
int failures = 0;

void require(bool value, const char *what) {
  if (!value) {
    std::fprintf(stderr, "host api diagnostic line: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  using msime::windows::host_api_diagnostic_line;
  using msime::windows::host_api_diagnostic_line_limit;
  require(host_api_diagnostic_line("sound pack not loaded: typewriter (C:\\Users\\me\\pack.toml): missing") ==
              "host_api: sound pack not loaded",
          "only the category before the first colon is kept");
  require(host_api_diagnostic_line("audio device not opened") == "host_api: uncategorized",
          "a line without a colon is not trusted");
  require(host_api_diagnostic_line(nullptr) == "host_api: uncategorized", "a null line is uncategorized");
  require(host_api_diagnostic_line(": detail") == "host_api: ", "an empty category stays empty");
  const std::string long_category(500, 'x');
  const auto capped = host_api_diagnostic_line((long_category + ": detail").c_str());
  require(capped.size() == host_api_diagnostic_line_limit, "the event is capped like macOS");
  require(capped.find("detail") == std::string::npos, "the detail never leaks past the cap");

  if (failures == 0)
    std::puts("host api diagnostic line passed");
  return failures == 0 ? 0 : 1;
}

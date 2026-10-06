#pragma once

#include <cstdlib>
#include <string>

#include "../core/LinuxEdition.h"

inline std::string msime_cli_provider_socket(int argc, char **argv,
                                             const char *environment,
                                             const char *default_name) {
  if (argc == 2)
    return argv[1][0] == '/' ? std::string(argv[1]) : std::string{};
  if (argc != 1)
    return {};
  if (const auto *value = std::getenv(environment); value && *value)
    return value[0] == '/' ? std::string(value) : std::string{};
  if (const auto *runtime = std::getenv("XDG_RUNTIME_DIR"); runtime && runtime[0] == '/')
    return std::string(runtime) + "/" MSIME_EDITION_CLIENT_DIRECTORY "/" + default_name;
  return {};
}

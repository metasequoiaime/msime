#include "RerankModelRoot.h"

#include <filesystem>
#include <iostream>
#include <stdexcept>

namespace {
void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}
} // namespace

int main() {
  namespace fs = std::filesystem;
  try {
    using msime::windows::name_rerank_models;
    const auto server = fs::temp_directory_path() / "msime-rerank-model-root" / "server";
    const auto data = server.parent_path() / "data";
    // A session after an online dictionary update: resources is a generation under DataDir and the helpcode fallback is DataDir itself, so neither parent is the Server directory the installer put rerank-model and rerank-model-small in.
    const nlohmann::json base = {{"api_version", 1},
                                 {"resources", (data / "dictionary-updates" / "generation").u8string()},
                                 {"helpcode_fallback_resources", data.u8string()}};
    auto options = base;
    name_rerank_models(options, server);
    require(options.at("rerank_models") == server.u8string(),
            "The Server directory was not named as the rerank model root");

    // A name already in the options wins; a relative directory and a document that is not an object are left alone.
    auto named = base;
    named["rerank_models"] = "C:\\elsewhere";
    name_rerank_models(named, server);
    require(named.at("rerank_models") == "C:\\elsewhere", "An explicit name was replaced");
    auto relative = base;
    name_rerank_models(relative, fs::path("server"));
    require(!relative.contains("rerank_models"), "A relative directory was named");
    nlohmann::json not_object = nlohmann::json::array();
    name_rerank_models(not_object, server);
    require(not_object.is_array() && not_object.empty(), "A non-object document changed");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Windows rerank model root checks passed\n";
  return 0;
}

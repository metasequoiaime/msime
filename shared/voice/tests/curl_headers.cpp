#include "../CurlHeaders.h"

#include <stdexcept>
#include <string>
#include <vector>

int main() {
  msime::voice::CurlHeaderList headers(nullptr, curl_slist_free_all);
  if (!msime::voice::append_curl_header(headers, "Authorization: Bearer synthetic") ||
      !msime::voice::append_curl_header(headers, "Content-Type: application/json") ||
      !msime::voice::append_curl_header(headers, "X-Synthetic: 1"))
    throw std::runtime_error("synthetic header append failed");

  std::vector<std::string> lines;
  for (const auto *item = headers.get(); item; item = item->next)
    lines.emplace_back(item->data);
  if (lines != std::vector<std::string>{"Authorization: Bearer synthetic",
                                       "Content-Type: application/json", "X-Synthetic: 1"})
    throw std::runtime_error("curl header list lost an item");
  return 0;
}

#include "../../TsfHostDelayLoad.h"

#include <cassert>

int main() {
  using msime::windows::host_library_path;

  assert(host_library_path(L"C:\\Windows\\System32\\IME\\MetasequoiaImeTsf.dll",
                           L"msime_host_api.dll") ==
         L"C:\\Windows\\System32\\IME\\msime_host_api.dll");
  assert(host_library_path(L"C:\\ime\\MetasequoiaImeTsf.dll",
                           L"msime_host_api_wubi.dll") ==
         L"C:\\ime\\msime_host_api_wubi.dll");
  for (const auto *invalid : {L"", L"MetasequoiaImeTsf.dll",
                              L"relative\\MetasequoiaImeTsf.dll",
                              L"C:relative\\MetasequoiaImeTsf.dll"}) {
    assert(host_library_path(invalid, L"msime_host_api.dll").empty());
  }
  assert(host_library_path(L"\\\\server\\share\\MetasequoiaImeTsf.dll",
                           L"msime_host_api.dll") ==
         L"\\\\server\\share\\msime_host_api.dll");
  for (const auto *invalid : {L"", L"..\\msime_host_api.dll",
                              L"C:\\msime_host_api.dll", L"other/msime_host_api.dll"}) {
    assert(host_library_path(L"C:\\ime\\MetasequoiaImeTsf.dll", invalid).empty());
  }
}

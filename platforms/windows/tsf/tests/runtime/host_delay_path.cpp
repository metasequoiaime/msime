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
  assert(host_library_path(L"MetasequoiaImeTsf.dll", L"msime_host_api.dll") ==
         L"msime_host_api.dll");
}

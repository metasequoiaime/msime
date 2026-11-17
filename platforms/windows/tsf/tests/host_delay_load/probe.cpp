#include "TsfHostImports.h"

extern "C" __declspec(dllimport) int msime_test_delay_one();
extern "C" __declspec(dllimport) int msime_test_delay_two();

extern "C" HRESULT msime_test_probe() {
  const auto result = msime::tsf::LoadHostImports();
  if (FAILED(result)) return result;
  return msime_test_delay_one() + msime_test_delay_two() == 42 ? S_OK : E_FAIL;
}

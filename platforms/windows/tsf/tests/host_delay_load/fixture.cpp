extern "C" __declspec(dllexport) int msime_test_delay_one() { return 19; }
#ifndef MSIME_TEST_MISSING_EXPORT
extern "C" __declspec(dllexport) int msime_test_delay_two() { return 23; }
#endif

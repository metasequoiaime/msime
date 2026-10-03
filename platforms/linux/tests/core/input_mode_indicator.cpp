#include "../src/core/InputModeIndicator.h"

#include <cassert>

int main() {
  using msime::linux_host::input_mode_indicator;
  using Indicator = msime::linux_host::InputModeIndicator;

  assert(input_mode_indicator(true, "quanpin", false) == Indicator::Chinese);
  assert(input_mode_indicator(true, "shuangpin", false) == Indicator::Chinese);
  assert(input_mode_indicator(true, "wubi", false) == Indicator::Chinese);
  assert(input_mode_indicator(true, "japanese", false) == Indicator::Japanese);
  assert(input_mode_indicator(true, "korean", false) == Indicator::Korean);
  assert(input_mode_indicator(true, "cantonese", false) == Indicator::Cantonese);
  assert(input_mode_indicator(true, "zhuyin", false) == Indicator::Zhuyin);
  assert(input_mode_indicator(true, "vietnamese", false) == Indicator::Vietnamese);
  // Stroke is a Chinese scheme shown as itself, like Cantonese and Zhuyin.
  assert(input_mode_indicator(true, "stroke", false) == Indicator::Stroke);
  assert(input_mode_indicator(false, "stroke", false) == Indicator::English);
  assert(input_mode_indicator(true, "stroke", true) == Indicator::CapsLock);
  assert(input_mode_indicator(false, "quanpin", false) == Indicator::English);
  assert(input_mode_indicator(false, "zhuyin", false) == Indicator::English);
  assert(input_mode_indicator(false, "vietnamese", false) == Indicator::English);
  assert(input_mode_indicator(true, "vietnamese", true) == Indicator::CapsLock);
  // Direct input with the Japanese or Korean scheme selected types English.
  assert(input_mode_indicator(false, "japanese", false) == Indicator::English);
  assert(input_mode_indicator(false, "korean", false) == Indicator::English);
  // CapsLock wins in every mode, as on the Windows language bar.
  assert(input_mode_indicator(true, "quanpin", true) == Indicator::CapsLock);
  assert(input_mode_indicator(true, "japanese", true) == Indicator::CapsLock);
  assert(input_mode_indicator(true, "korean", true) == Indicator::CapsLock);
  assert(input_mode_indicator(false, "quanpin", true) == Indicator::CapsLock);
  return 0;
}

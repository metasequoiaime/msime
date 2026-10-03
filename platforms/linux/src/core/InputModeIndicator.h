#pragma once

#include <string_view>

namespace msime::linux_host {

// What the panel shows for the input method, following the Windows language bar (LanguageBar.cpp): CapsLock outranks everything, because letters then reach the editor as capitals whichever mode is on; every scheme other than the three Chinese ones of the base set is shown only while conversion is on, since direct input with it selected is plain English typing. Each host draws these with its own symbols.
enum class InputModeIndicator { Chinese, English, Japanese, Korean, Cantonese, Zhuyin, Vietnamese, Tibetan, Stroke, CapsLock };

// `scheme` is the preferences scheme id the Engine runs ("quanpin", "japanese", "korean", ...; see effective_input_scheme); Cantonese, Zhuyin and Stroke, though Chinese, are shown as themselves, and every other id is one of the Chinese schemes of the base set.
inline InputModeIndicator input_mode_indicator(bool input_enabled, std::string_view scheme,
                                               bool caps_lock) {
  if (caps_lock) return InputModeIndicator::CapsLock;
  if (!input_enabled) return InputModeIndicator::English;
  if (scheme == "japanese") return InputModeIndicator::Japanese;
  if (scheme == "korean") return InputModeIndicator::Korean;
  if (scheme == "cantonese") return InputModeIndicator::Cantonese;
  if (scheme == "zhuyin") return InputModeIndicator::Zhuyin;
  if (scheme == "vietnamese") return InputModeIndicator::Vietnamese;
  if (scheme == "tibetan") return InputModeIndicator::Tibetan;
  if (scheme == "stroke") return InputModeIndicator::Stroke;
  return InputModeIndicator::Chinese;
}

}  // namespace msime::linux_host

#include "../../../../shared/voice/HelperCrashCapture.h"
#include "../core/AuxiliaryCrashCapture.h"

namespace msime::voice {

struct HelperCrashCapture::State {
  msime::linux_host::AuxiliaryCrashCapture capture{"voice-local"};
};

HelperCrashCapture::HelperCrashCapture() noexcept {
  try {
    state_ = std::make_unique<State>();
  } catch (...) {
  }
}

HelperCrashCapture::~HelperCrashCapture() = default;

} // namespace msime::voice

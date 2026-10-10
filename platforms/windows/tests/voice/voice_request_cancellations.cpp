#include "../../src/voice/VoiceRequestCancellation.h"

#include <cstdlib>
#include <memory>
#include <vector>

using msime::windows::prune_completed_voice_requests;

static void require(bool value) {
  if (!value)
    std::abort();
}

int main() {
  auto completed = std::make_shared<std::atomic_bool>(false);
  auto active = std::make_shared<std::atomic_bool>(false);
  std::vector<std::shared_ptr<std::atomic_bool>> requests{completed, active};
  completed.reset();

  prune_completed_voice_requests(requests);

  require(requests.size() == 1);
  require(requests.front() == active);
}
